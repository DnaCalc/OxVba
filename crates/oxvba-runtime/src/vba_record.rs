use crate::{
    Variant, VariantCore,
    bstr::BStr,
    safe_array::{SafeArray, SafeArrayBound},
};
use core::{ptr, slice};
use std::sync::Arc;

/// VBA rejects a user-defined type whose packed native payload exceeds 64 KiB.
///
/// Procedure-local declarations have an additional 32 KiB compiler rule; this
/// runtime limit is the context-independent upper bound for the type itself.
pub const MAX_VBA_RECORD_SIZE: usize = 64 * 1024;

/// VBA arrays admit at most 60 dimensions.
pub const MAX_VBA_RECORD_FIXED_ARRAY_RANK: usize = 60;

#[cfg(test)]
std::thread_local! {
    static FIELD_POINTER_PROJECTIONS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    static RECORD_BUFFER_EVENTS: core::cell::Cell<(usize, usize)> = const { core::cell::Cell::new((0, 0)) };
}

#[cfg(test)]
fn note_field_pointer_projection() {
    FIELD_POINTER_PROJECTIONS.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
fn note_record_buffer_allocation() {
    RECORD_BUFFER_EVENTS.with(|events| {
        let (allocated, freed) = events.get();
        events.set((allocated + 1, freed));
    });
}

#[cfg(test)]
fn note_record_buffer_free() {
    RECORD_BUFFER_EVENTS.with(|events| {
        let (allocated, freed) = events.get();
        events.set((allocated, freed + 1));
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VbaRecordFieldKind {
    Variant,
    Integer,
    Long,
    LongLong,
    Byte,
    Single,
    Double,
    Currency,
    Date,
    String,
    FixedString {
        len: usize,
    },
    Boolean,
    Record(Arc<VbaRecordLayout>),
    FixedArray {
        element: Box<VbaRecordFieldKind>,
        bounds: Vec<SafeArrayBound>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbaRecordFieldSpec {
    pub name: Option<String>,
    pub kind: VbaRecordFieldKind,
}

impl VbaRecordFieldSpec {
    pub fn anonymous(kind: VbaRecordFieldKind) -> Self {
        Self { name: None, kind }
    }

    pub fn named(name: impl Into<String>, kind: VbaRecordFieldKind) -> Self {
        Self {
            name: Some(name.into()),
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbaRecordFieldLayout {
    name: Option<String>,
    kind: VbaRecordFieldKind,
    offset: usize,
    size: usize,
    align: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbaRecordLayout {
    fields: Vec<VbaRecordFieldLayout>,
    size: usize,
    align: usize,
}

pub struct VbaRecord {
    layout: Arc<VbaRecordLayout>,
    data: Vec<u64>,
}

/// An unforgeable field selection bound to one runtime layout instance.
///
/// A handle can be reused across records only when they share the same
/// [`Arc<VbaRecordLayout>`]. Passing a handle from an independently constructed
/// (even structurally equal) layout is rejected before a record pointer is
/// formed.
#[derive(Clone)]
pub struct VbaRecordFieldHandle {
    layout: Arc<VbaRecordLayout>,
    index: usize,
}

impl VbaRecordFieldLayout {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn kind(&self) -> &VbaRecordFieldKind {
        &self.kind
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn align(&self) -> usize {
        self.align
    }
}

impl VbaRecordFieldHandle {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn name(&self) -> Option<&str> {
        self.layout.fields[self.index].name()
    }

    pub fn kind(&self) -> &VbaRecordFieldKind {
        self.layout.fields[self.index].kind()
    }

    pub fn offset(&self) -> usize {
        self.layout.fields[self.index].offset()
    }

    pub fn size(&self) -> usize {
        self.layout.fields[self.index].size()
    }

    pub fn align(&self) -> usize {
        self.layout.fields[self.index].align()
    }
}

impl core::fmt::Debug for VbaRecordFieldHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VbaRecordFieldHandle")
            .field("index", &self.index)
            .field("name", &self.name())
            .field("offset", &self.offset())
            .field("size", &self.size())
            .field("align", &self.align())
            .finish()
    }
}

impl PartialEq for VbaRecordFieldHandle {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && Arc::ptr_eq(&self.layout, &other.layout)
    }
}

impl Eq for VbaRecordFieldHandle {}

impl VbaRecordLayout {
    pub fn new(fields: Vec<VbaRecordFieldSpec>) -> Result<Self, String> {
        let (size, align) = validate_record_shape(&fields)?;

        let mut offset = 0usize;
        let mut layouts = Vec::new();
        layouts
            .try_reserve_exact(fields.len())
            .map_err(|_| "VBA record field table allocation failed".to_string())?;

        for field in fields {
            let (field_size, field_align) = field.kind.storage_shape()?;
            offset = checked_align_to(offset, field_align)?;
            layouts.push(VbaRecordFieldLayout {
                name: field.name,
                kind: field.kind,
                offset,
                size: field_size,
                align: field_align,
            });
            offset = offset
                .checked_add(field_size)
                .ok_or_else(|| "VBA record layout size overflow".to_string())?;
        }

        debug_assert_eq!(checked_align_to(offset, align), Ok(size));

        Ok(Self {
            fields: layouts,
            size,
            align,
        })
    }

    pub fn fields(&self) -> &[VbaRecordFieldLayout] {
        &self.fields
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn align(&self) -> usize {
        self.align
    }

    pub fn field_handle(self: &Arc<Self>, index: usize) -> Option<VbaRecordFieldHandle> {
        self.fields.get(index)?;
        Some(VbaRecordFieldHandle {
            layout: Arc::clone(self),
            index,
        })
    }

    pub fn file_len(&self) -> Result<usize, String> {
        self.fields.iter().try_fold(0usize, |total, field| {
            total
                .checked_add(field.kind.file_len()?)
                .ok_or_else(|| "VBA record file length overflow".to_string())
        })
    }

    pub fn validate_byref_as_any_native_abi(&self) -> Result<(), String> {
        for field in self.fields() {
            let name = field
                .name
                .as_deref()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("@{}", field.offset));
            field
                .kind
                .validate_byref_as_any_native_abi(&name)
                .map_err(|reason| {
                    format!(
                        "record field `{name}` is not supported for native ByRef As Any: {reason}"
                    )
                })?;
        }
        Ok(())
    }

    pub fn supports_lset_byte_overlay(&self) -> bool {
        self.fields()
            .iter()
            .all(|field| field.kind.supports_lset_byte_overlay())
    }
}

impl VbaRecordFieldKind {
    pub fn file_len(&self) -> Result<usize, String> {
        let len = match self {
            Self::Variant => {
                return Err("Variant record field file length is not implemented".to_string());
            }
            Self::String => {
                return Err(
                    "variable-length String record field file length is not implemented"
                        .to_string(),
                );
            }
            Self::FixedString { len } => *len,
            Self::Record(layout) => layout.file_len()?,
            Self::FixedArray { element, bounds } => element
                .file_len()?
                .checked_mul(fixed_array_total_len(bounds)?)
                .ok_or_else(|| "VBA fixed-array record file length overflow".to_string())?,
            _ => self.storage_shape()?.0,
        };
        Ok(len)
    }

    pub fn storage_shape(&self) -> Result<(usize, usize), String> {
        let pointer_shape = (
            core::mem::size_of::<*mut core::ffi::c_void>(),
            core::mem::align_of::<*mut core::ffi::c_void>(),
        );
        let shape = match self {
            Self::Variant => (
                core::mem::size_of::<VariantCore>(),
                core::mem::align_of::<VariantCore>(),
            ),
            Self::Integer => (core::mem::size_of::<i16>(), core::mem::align_of::<i16>()),
            Self::Long => (core::mem::size_of::<i32>(), core::mem::align_of::<i32>()),
            Self::LongLong => (core::mem::size_of::<i64>(), core::mem::align_of::<i64>()),
            Self::Byte => (core::mem::size_of::<u8>(), core::mem::align_of::<u8>()),
            Self::Single => (core::mem::size_of::<f32>(), core::mem::align_of::<f32>()),
            Self::Double | Self::Currency | Self::Date => {
                (core::mem::size_of::<f64>(), core::mem::align_of::<f64>())
            }
            Self::String => pointer_shape,
            Self::FixedString { len } => {
                if *len == 0 {
                    return Err(
                        "VBA fixed-string record field must have at least one character"
                            .to_string(),
                    );
                }
                let size = len
                    .checked_mul(core::mem::size_of::<u16>())
                    .ok_or_else(|| "VBA fixed-string record field size overflow".to_string())?;
                validate_record_size(size)?;
                (size, 1)
            }
            Self::Boolean => (core::mem::size_of::<i16>(), core::mem::align_of::<i16>()),
            Self::Record(layout) => (layout.size(), layout.align()),
            Self::FixedArray { element, bounds } => {
                let len = fixed_array_total_len(bounds)?;
                let (element_size, element_align) = element.storage_shape()?;
                if element_size == 0 {
                    return Err("VBA fixed-array record element cannot be zero-sized".to_string());
                }
                let stride = checked_align_to(element_size, element_align)?;
                let size = stride
                    .checked_mul(len)
                    .ok_or_else(|| "VBA fixed-array record field size overflow".to_string())?;
                validate_record_size(size)?;
                (size, element_align)
            }
        };
        if shape.0 == 0 {
            return Err("VBA record field cannot be zero-sized".to_string());
        }
        validate_record_size(shape.0)?;
        Ok(shape)
    }

    fn validate_byref_as_any_native_abi(&self, path: &str) -> Result<(), String> {
        match self {
            Self::Integer
            | Self::Long
            | Self::LongLong
            | Self::Byte
            | Self::Single
            | Self::Double
            | Self::Currency
            | Self::Date
            | Self::Boolean => Ok(()),
            Self::Record(layout) => {
                for field in layout.fields() {
                    let child = field
                        .name
                        .as_deref()
                        .map(|name| format!("{path}.{name}"))
                        .unwrap_or_else(|| format!("{path}.@{}", field.offset));
                    field.kind.validate_byref_as_any_native_abi(&child)?;
                }
                Ok(())
            }
            Self::FixedArray { element, .. } => {
                element.validate_byref_as_any_native_abi(&format!("{path}[]"))
            }
            Self::String => {
                Err("String fields carry owned BSTR pointers, not plain native bytes".into())
            }
            Self::FixedString { .. } => Ok(()),
            Self::Variant => {
                Err("Variant fields carry nested VARIANT state, not plain native bytes".into())
            }
        }
    }

    fn supports_lset_byte_overlay(&self) -> bool {
        match self {
            Self::Variant | Self::String => false,
            Self::Record(layout) => layout.supports_lset_byte_overlay(),
            Self::FixedArray { element, .. } => element.supports_lset_byte_overlay(),
            Self::Integer
            | Self::Long
            | Self::LongLong
            | Self::Byte
            | Self::Single
            | Self::Double
            | Self::Currency
            | Self::Date
            | Self::FixedString { .. }
            | Self::Boolean => true,
        }
    }
}

impl VbaRecord {
    pub fn new_default(layout: Arc<VbaRecordLayout>) -> Result<Self, String> {
        validate_record_size(layout.size())?;
        validate_storage_alignment(layout.align())?;
        let word_size = core::mem::size_of::<u64>();
        let words = layout
            .size()
            .checked_add(word_size - 1)
            .ok_or_else(|| "VBA record buffer word count overflow".to_string())?
            / word_size;
        let mut data = Vec::new();
        data.try_reserve_exact(words).map_err(|_| {
            format!(
                "VBA record buffer allocation failed for {} bytes",
                layout.size()
            )
        })?;
        data.resize(words, 0);
        let mut record = Self { layout, data };
        crate::live_counters::record_buffer_allocated();
        #[cfg(test)]
        note_record_buffer_allocation();
        let fields = record.layout.fields().to_vec();
        for (index, field) in fields.iter().enumerate() {
            // SAFETY: the buffer is sized to `layout.size()`, and each field offset
            // and recursive fixed-array stride was computed by `VbaRecordLayout`.
            unsafe {
                init_field_at(record.field_mut_ptr_by_index(index)?, &field.kind)?;
            }
        }
        Ok(record)
    }

    pub fn layout(&self) -> &Arc<VbaRecordLayout> {
        &self.layout
    }

    pub fn file_len(&self) -> Result<usize, String> {
        self.layout.file_len()
    }

    pub fn memory_len(&self) -> usize {
        self.layout.size()
    }

    pub fn data_ptr(&self) -> *const u8 {
        self.data.as_ptr().cast()
    }

    pub fn data_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr().cast()
    }

    pub fn field_handle(&self, index: usize) -> Option<VbaRecordFieldHandle> {
        self.layout.field_handle(index)
    }

    /// Return this record's pointer for an owner-bound field handle.
    ///
    /// Layout identity, index, kind shape, extent and alignment are validated
    /// before pointer arithmetic. Dereferencing or casting the returned raw
    /// pointer remains the caller's unsafe operation.
    pub fn field_ptr(&self, field: &VbaRecordFieldHandle) -> Result<*const u8, String> {
        self.ensure_handle_belongs(field)?;
        self.field_ptr_by_index(field.index)
    }

    /// Return this record's mutable pointer for an owner-bound field handle.
    ///
    /// The same validation as [`Self::field_ptr`] happens before pointer
    /// arithmetic; `&mut self` supplies exclusive record access.
    pub fn field_mut_ptr(&mut self, field: &VbaRecordFieldHandle) -> Result<*mut u8, String> {
        self.ensure_handle_belongs(field)?;
        self.field_mut_ptr_by_index(field.index)
    }

    pub fn field_bytes(&self, index: usize) -> Option<&[u8]> {
        let size = self.checked_field_by_index(index).ok()?.size;
        let ptr = self.field_ptr_by_index(index).ok()?;
        // SAFETY: `field_ptr_by_index` validated that the complete field extent is
        // inside this live record buffer.
        Some(unsafe { slice::from_raw_parts(ptr, size) })
    }

    pub fn read_field_variant(&self, index: usize) -> Result<Variant, String> {
        let kind = self.checked_field_by_index(index)?.kind.clone();
        // SAFETY: the field pointer is in range and aligned for `field.kind`.
        unsafe { read_field_variant_at(self.field_ptr_by_index(index)?, &kind) }
    }

    pub fn write_field_variant(&mut self, index: usize, value: &Variant) -> Result<(), String> {
        let kind = self.checked_field_by_index(index)?.kind.clone();
        // SAFETY: the field pointer is in range and aligned for `field.kind`.
        unsafe { write_field_variant_at(self.field_mut_ptr_by_index(index)?, &kind, value) }
    }

    fn ensure_handle_belongs(&self, field: &VbaRecordFieldHandle) -> Result<(), String> {
        if !Arc::ptr_eq(&self.layout, &field.layout) {
            return Err("record field handle belongs to a different layout".to_string());
        }
        if field.index >= self.layout.fields.len() {
            return Err(format!(
                "record field handle index {} is out of range",
                field.index
            ));
        }
        Ok(())
    }

    fn checked_field_by_index(&self, index: usize) -> Result<&VbaRecordFieldLayout, String> {
        let field = self
            .layout
            .fields
            .get(index)
            .ok_or_else(|| format!("record field {index} out of range"))?;
        let (expected_size, expected_align) = field.kind.storage_shape()?;
        validate_storage_alignment(field.align)?;
        if field.size != expected_size || field.align != expected_align {
            return Err(format!(
                "record field {index} shape does not match its sealed kind"
            ));
        }
        if field.offset % field.align != 0 {
            return Err(format!(
                "record field {index} offset {} is not aligned to {}",
                field.offset, field.align
            ));
        }
        let end = field
            .offset
            .checked_add(field.size)
            .ok_or_else(|| format!("record field {index} extent overflow"))?;
        if end > self.layout.size {
            return Err(format!(
                "record field {index} extent {end} exceeds layout size {}",
                self.layout.size
            ));
        }
        let buffer_bytes = self
            .data
            .len()
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or_else(|| "record buffer byte length overflow".to_string())?;
        if self.layout.size > buffer_bytes || end > buffer_bytes {
            return Err(format!(
                "record field {index} extent is outside its owned buffer"
            ));
        }
        Ok(field)
    }

    fn field_ptr_by_index(&self, index: usize) -> Result<*const u8, String> {
        let offset = self.checked_field_by_index(index)?.offset;
        #[cfg(test)]
        note_field_pointer_projection();
        // SAFETY: `checked_field_by_index` proved the offset and full field extent
        // are inside the live `Vec<u64>` buffer, and that the offset has the kind's
        // required alignment relative to the `u64`-aligned base.
        Ok(unsafe { self.data.as_ptr().cast::<u8>().add(offset) })
    }

    fn field_mut_ptr_by_index(&mut self, index: usize) -> Result<*mut u8, String> {
        let offset = self.checked_field_by_index(index)?.offset;
        #[cfg(test)]
        note_field_pointer_projection();
        // SAFETY: the same extent/alignment proof as `field_ptr_by_index` applies;
        // `&mut self` supplies exclusive access to the buffer.
        Ok(unsafe { self.data.as_mut_ptr().cast::<u8>().add(offset) })
    }

    pub fn lset_from(&mut self, source: &VbaRecord) -> Result<(), String> {
        if !self.layout.supports_lset_byte_overlay() || !source.layout.supports_lset_byte_overlay()
        {
            return Err("Type mismatch".to_string());
        }
        let len = self.memory_len().min(source.memory_len());
        // SAFETY: both pointers address live record buffers sized by their layouts.
        // `ptr::copy` permits overlap, so `LSet a = a` is a no-op rather than UB.
        unsafe { ptr::copy(source.data_ptr(), self.data_mut_ptr(), len) };
        Ok(())
    }

    pub fn array_field_bounds_len(
        &self,
        index: usize,
    ) -> Result<Option<(Vec<SafeArrayBound>, usize)>, String> {
        let kind = self.checked_field_by_index(index)?.kind.clone();
        match &kind {
            VbaRecordFieldKind::Variant => {
                // SAFETY: this field is a live Variant slot in this record layout.
                let value = unsafe { &*self.field_ptr_by_index(index)?.cast::<Variant>() };
                Ok(value.safearray_bounds_len())
            }
            VbaRecordFieldKind::FixedArray { bounds, .. } => {
                Ok(Some((bounds.clone(), fixed_array_total_len(bounds)?)))
            }
            _ => Ok(None),
        }
    }

    pub fn read_array_field_element(
        &self,
        index: usize,
        flat: usize,
    ) -> Result<Option<Variant>, String> {
        let kind = self.checked_field_by_index(index)?.kind.clone();
        match &kind {
            VbaRecordFieldKind::Variant => {
                // SAFETY: this field is a live Variant slot in this record layout.
                let value = unsafe { &*self.field_ptr_by_index(index)?.cast::<Variant>() };
                Ok(value.safearray_element(flat).transpose()?)
            }
            VbaRecordFieldKind::FixedArray { element, bounds } => {
                let len = fixed_array_total_len(bounds)?;
                if flat >= len {
                    return Err("fixed-array record field index out of range".to_string());
                }
                let (element_size, element_align) = element.storage_shape()?;
                let stride = checked_align_to(element_size, element_align)?;
                // SAFETY: `flat < len`; `stride` is the element storage size and
                // `field_ptr_by_index(index)` is the validated fixed-array base.
                Ok(Some(unsafe {
                    read_field_variant_at(
                        self.field_ptr_by_index(index)?.add(flat * stride),
                        element,
                    )?
                }))
            }
            _ => Ok(None),
        }
    }

    pub fn write_array_field_element(
        &mut self,
        index: usize,
        flat: usize,
        value: &Variant,
    ) -> Result<Option<()>, String> {
        let kind = self.checked_field_by_index(index)?.kind.clone();
        match &kind {
            VbaRecordFieldKind::Variant => {
                // SAFETY: this field is a live Variant slot and `&mut self` proves
                // exclusive access to it.
                let field_value =
                    unsafe { &mut *self.field_mut_ptr_by_index(index)?.cast::<Variant>() };
                if field_value.safearray_bounds_len().is_none() {
                    return Ok(None);
                }
                field_value.set_safearray_element(flat, value)?;
                Ok(Some(()))
            }
            VbaRecordFieldKind::FixedArray { element, bounds } => {
                let len = fixed_array_total_len(bounds)?;
                if flat >= len {
                    return Err("fixed-array record field index out of range".to_string());
                }
                let (element_size, element_align) = element.storage_shape()?;
                let stride = checked_align_to(element_size, element_align)?;
                // SAFETY: `flat < len`; `stride` is the element storage size and
                // `field_mut_ptr_by_index(index)` is the validated field base.
                unsafe {
                    write_field_variant_at(
                        self.field_mut_ptr_by_index(index)?.add(flat * stride),
                        element,
                        value,
                    )?;
                }
                Ok(Some(()))
            }
            _ => Ok(None),
        }
    }

    /// Clone a record value from raw storage described by `layout`.
    ///
    /// # Safety
    /// `src` must point to a live record payload initialized according to `layout`
    /// for the duration of this call.
    pub unsafe fn clone_from_raw(
        src: *const u8,
        layout: Arc<VbaRecordLayout>,
    ) -> Result<Self, String> {
        // SAFETY: forwards this method's `# Safety` contract — `src` points to a
        // live record payload initialized according to `layout` for this call.
        unsafe { clone_record_from_ptr(src, layout) }
    }

    /// Clone this record into uninitialized raw storage described by its layout.
    ///
    /// # Safety
    /// `dst` must point to writable, properly aligned, uninitialized storage of at
    /// least `self.layout().size()` bytes. The caller becomes responsible for
    /// eventually dropping the initialized raw record with [`Self::drop_raw`].
    pub unsafe fn clone_into_raw(&self, dst: *mut u8) -> Result<(), String> {
        // SAFETY: forwards this method's `# Safety` contract — `dst` is writable,
        // aligned, uninitialized storage of at least `self.layout().size()` bytes;
        // `self.data_ptr()` is this record's own live payload with the same layout.
        unsafe { clone_record_into_ptr(self.data_ptr(), dst, self.layout()) }
    }

    /// Drop a raw record payload initialized according to `layout`.
    ///
    /// # Safety
    /// `ptr` must point to a live record payload initialized according to `layout`,
    /// and this function must be called at most once for that payload.
    pub unsafe fn drop_raw(ptr: *mut u8, layout: &VbaRecordLayout) {
        for field in layout.fields() {
            // SAFETY: per this method's `# Safety` contract `ptr` is a live payload
            // laid out by `layout`, so `field.offset` is in bounds; each field is
            // dropped exactly once because the caller promises a single `drop_raw`.
            unsafe { drop_field_at(ptr.add(field.offset), &field.kind) };
        }
    }

    pub fn clone_into_native_words(&self) -> Result<Vec<u64>, String> {
        self.layout().validate_byref_as_any_native_abi()?;
        let mut words = vec![0; self.layout().size().div_ceil(core::mem::size_of::<u64>())];
        // SAFETY: `Vec<u64>` gives native word alignment, the buffer length covers
        // the descriptor size, and eligibility excludes owning fields that would
        // need non-trivial initialization/cleanup during plain native staging.
        unsafe { self.clone_into_raw(words.as_mut_ptr().cast())? };
        Ok(words)
    }

    /// Clone a record value out of a native `u64` word buffer staged for ByRef
    /// As Any (the inverse of [`Self::clone_into_native_words`]).
    ///
    /// # Safety
    /// `words` must hold a live native-ABI image of a record laid out by `layout`
    /// (e.g. as written back by native code through [`Self::clone_into_native_words`]):
    /// every plain-native field initialized, for the duration of this call.
    pub unsafe fn clone_from_native_words(
        words: &[u64],
        layout: Arc<VbaRecordLayout>,
    ) -> Result<Self, String> {
        layout.validate_byref_as_any_native_abi()?;
        let required_words = layout.size().div_ceil(core::mem::size_of::<u64>());
        if words.len() < required_words {
            return Err(format!(
                "native record buffer has {} words but layout requires {}",
                words.len(),
                required_words
            ));
        }
        // SAFETY: `validate_byref_as_any_native_abi` accepted `layout` (plain native
        // fields only), the length check above guarantees the `u64` buffer covers
        // `layout.size()`, and `Vec<u64>` storage satisfies the record's alignment.
        unsafe { Self::clone_from_raw(words.as_ptr().cast(), layout) }
    }
}

impl Clone for VbaRecord {
    fn clone(&self) -> Self {
        let mut clone = Self {
            layout: self.layout.clone(),
            data: vec![0; self.data.len()],
        };
        crate::live_counters::record_buffer_allocated();
        #[cfg(test)]
        note_record_buffer_allocation();
        let fields = self.layout.fields().to_vec();
        for (index, field) in fields.iter().enumerate() {
            // SAFETY: source and destination are distinct buffers with the same
            // descriptor-backed layout.
            unsafe {
                clone_field_at(
                    self.field_ptr_by_index(index)
                        .expect("sealed source record field"),
                    clone
                        .field_mut_ptr_by_index(index)
                        .expect("sealed destination record field"),
                    &field.kind,
                )
                .expect("VBA record deep clone should succeed");
            }
        }
        clone
    }
}

impl Drop for VbaRecord {
    fn drop(&mut self) {
        let fields = self.layout.fields().to_vec();
        for field in &fields {
            // SAFETY: each initialized field is dropped exactly once while the owning
            // record buffer is still live.
            unsafe {
                let ptr = self.data_mut_ptr().add(field.offset);
                drop_field_at(ptr, &field.kind);
            }
        }
        crate::live_counters::record_buffer_freed();
        #[cfg(test)]
        note_record_buffer_free();
    }
}

impl core::fmt::Debug for VbaRecord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VbaRecord")
            .field("layout", &self.layout)
            .field("data_ptr", &self.data_ptr())
            .finish()
    }
}

/// # Safety
/// `ptr` must reference an uninitialized field slot sized and aligned for `kind`
/// (i.e. a slot inside a buffer laid out by [`VbaRecordLayout`]).
unsafe fn init_field_at(ptr: *mut u8, kind: &VbaRecordFieldKind) -> Result<(), String> {
    match kind {
        // SAFETY: per the contract the slot is sized/aligned for a `Variant`; the
        // slot is uninitialized so `write` overwrites without dropping garbage.
        VbaRecordFieldKind::Variant => unsafe { ptr.cast::<Variant>().write(Variant::empty()) },
        // SAFETY: the slot is the pointer-sized/aligned BSTR carrier; writing the
        // null sentinel initializes it as the empty-string state.
        VbaRecordFieldKind::String => unsafe { ptr.cast::<*mut u16>().write(ptr::null_mut()) },
        VbaRecordFieldKind::Record(layout) => {
            for field in layout.fields() {
                // SAFETY: `field.offset` lies within this nested record's slot (the
                // offsets were produced by the same layout pass that sized it).
                unsafe { init_field_at(ptr.add(field.offset), &field.kind)? };
            }
        }
        VbaRecordFieldKind::FixedArray { element, bounds } => {
            let len = fixed_array_total_len(bounds)?;
            let (element_size, element_align) = element.storage_shape()?;
            let stride = checked_align_to(element_size, element_align)?;
            for i in 0..len {
                // SAFETY: `i < len` and `stride` is the per-element storage size, so
                // `i * stride` indexes the `i`-th element slot inside the array field.
                unsafe { init_field_at(ptr.add(i * stride), element)? };
            }
        }
        _ => {}
    }
    Ok(())
}

/// # Safety
/// `src` must reference a live field initialized for `kind`, and `dst` an
/// uninitialized field slot of the same `kind`; the two slots must not overlap.
unsafe fn clone_field_at(
    src: *const u8,
    dst: *mut u8,
    kind: &VbaRecordFieldKind,
) -> Result<(), String> {
    match kind {
        VbaRecordFieldKind::Variant => {
            // SAFETY: `src` holds a live `Variant` per the contract; borrowing it
            // shared to clone does not move or invalidate the source slot.
            let value = unsafe { &*src.cast::<Variant>() };
            // SAFETY: `dst` is an uninitialized `Variant` slot; `write` stores the
            // deep clone without dropping the (uninitialized) destination.
            unsafe { dst.cast::<Variant>().write(value.clone()) };
        }
        VbaRecordFieldKind::String => {
            // SAFETY: `src` holds the initialized BSTR carrier pointer for this field.
            let raw = unsafe { *src.cast::<*mut u16>() };
            let cloned = clone_bstr_raw(raw)?;
            // SAFETY: `dst` is the uninitialized BSTR carrier slot; store the owned clone.
            unsafe { dst.cast::<*mut u16>().write(cloned) };
        }
        VbaRecordFieldKind::Record(layout) => {
            for field in layout.fields() {
                // SAFETY: `field.offset` indexes the same sub-field in both the live
                // `src` and uninitialized `dst` nested records (identical layouts).
                unsafe {
                    clone_field_at(src.add(field.offset), dst.add(field.offset), &field.kind)?
                };
            }
        }
        VbaRecordFieldKind::FixedArray { element, bounds } => {
            let len = fixed_array_total_len(bounds)?;
            let (element_size, element_align) = element.storage_shape()?;
            let stride = checked_align_to(element_size, element_align)?;
            for i in 0..len {
                // SAFETY: `i < len` and `stride` is the element storage size, so
                // `i * stride` selects matching element slots in `src` and `dst`.
                unsafe { clone_field_at(src.add(i * stride), dst.add(i * stride), element)? };
            }
        }
        _ => {
            let (size, _) = kind.storage_shape()?;
            // SAFETY: scalar (Copy) fields: both slots are `size` bytes for `kind`
            // and are non-overlapping distinct record buffers per the contract.
            unsafe { ptr::copy_nonoverlapping(src, dst, size) };
        }
    }
    Ok(())
}

/// # Safety
/// `ptr` must reference a field initialized for `kind`, and this must be the
/// single drop of that field (no later read/drop of the same slot).
unsafe fn drop_field_at(ptr: *mut u8, kind: &VbaRecordFieldKind) {
    match kind {
        // SAFETY: the slot holds a live `Variant`; `drop_in_place` runs its
        // destructor exactly once per this function's single-drop contract.
        VbaRecordFieldKind::Variant => unsafe { ptr::drop_in_place(ptr.cast::<Variant>()) },
        VbaRecordFieldKind::String => {
            // SAFETY: the slot holds the field's BSTR carrier pointer.
            let raw = unsafe { *ptr.cast::<*mut u16>() };
            if !raw.is_null() {
                // SAFETY: string fields own the BSTR pointer stored in the record slot.
                let _ = unsafe { BStr::from_raw_bstr(raw) };
            }
        }
        VbaRecordFieldKind::Record(layout) => {
            for field in layout.fields() {
                // SAFETY: `field.offset` is in bounds of this nested record slot; each
                // sub-field is dropped once as part of this single record drop.
                unsafe { drop_field_at(ptr.add(field.offset), &field.kind) };
            }
        }
        VbaRecordFieldKind::FixedArray { element, bounds } => {
            let Ok((element_size, element_align)) = element.storage_shape() else {
                return;
            };
            let Ok(len) = fixed_array_total_len(bounds) else {
                return;
            };
            let Ok(stride) = checked_align_to(element_size, element_align) else {
                return;
            };
            for i in 0..len {
                // SAFETY: `i < len` and `stride` is the element storage size, so each
                // `i * stride` selects a distinct live element slot dropped once.
                unsafe { drop_field_at(ptr.add(i * stride), element) };
            }
        }
        _ => {}
    }
}

/// # Safety
/// `ptr` must reference a live field initialized for and aligned to `kind`.
unsafe fn read_field_variant_at(
    ptr: *const u8,
    kind: &VbaRecordFieldKind,
) -> Result<Variant, String> {
    // Each typed read below is sound because the contract guarantees the slot at
    // `ptr` holds a value of exactly this `kind`, aligned for its storage type.
    let value = match kind {
        // SAFETY: slot holds a live `Variant`; shared-borrow + clone leaves it intact.
        VbaRecordFieldKind::Variant => unsafe { (&*ptr.cast::<Variant>()).clone() },
        // SAFETY: slot is an aligned `i16`.
        VbaRecordFieldKind::Integer => Variant::from_i16(unsafe { *ptr.cast::<i16>() }),
        // SAFETY: slot is an aligned `i32`.
        VbaRecordFieldKind::Long => Variant::from_i32(unsafe { *ptr.cast::<i32>() }),
        // SAFETY: slot is an aligned `i64`.
        VbaRecordFieldKind::LongLong => Variant::from_i64(unsafe { *ptr.cast::<i64>() }),
        // SAFETY: slot is a `u8`.
        VbaRecordFieldKind::Byte => Variant::from_u8(unsafe { *ptr.cast::<u8>() }),
        // SAFETY: slot is an aligned `f32`.
        VbaRecordFieldKind::Single => Variant::from_f32(unsafe { *ptr.cast::<f32>() }),
        // SAFETY: slot is an aligned `f64`.
        VbaRecordFieldKind::Double => Variant::from_f64(unsafe { *ptr.cast::<f64>() }),
        VbaRecordFieldKind::Currency => {
            // SAFETY: Currency is stored as a scaled `i64`; slot is an aligned `i64`.
            Variant::from_currency_scaled_i64(unsafe { *ptr.cast::<i64>() })
        }
        // SAFETY: Date is stored as an `f64` serial; slot is an aligned `f64`.
        VbaRecordFieldKind::Date => Variant::from_date_f64(unsafe { *ptr.cast::<f64>() }),
        VbaRecordFieldKind::String => {
            // SAFETY: slot holds the field's BSTR carrier pointer.
            let raw = unsafe { *ptr.cast::<*mut u16>() };
            if raw.is_null() {
                Variant::from_string(BStr::empty())
            } else {
                // SAFETY: `raw` is the live BSTR owned by the record slot; we borrow
                // it without taking ownership and `forget` the wrapper so the slot
                // keeps its pointer (see `borrow_bstr_raw`).
                let text = unsafe { borrow_bstr_raw(raw) };
                let value = Variant::from_string(text.clone());
                core::mem::forget(text);
                value
            }
        }
        VbaRecordFieldKind::FixedString { len } => {
            let mut units = Vec::with_capacity(*len);
            for i in 0..*len {
                // SAFETY: fixed-string fields are byte-packed in VBA records, so the
                // u16 code units may be unaligned. `read_unaligned` copies each unit.
                units.push(unsafe { ptr::read_unaligned(ptr.add(i * 2).cast::<u16>()) });
            }
            Variant::from_string(BStr::from_utf16_units(&units)?)
        }
        // SAFETY: Boolean is stored as a VBA `i16` (0 / -1); slot is an aligned `i16`.
        VbaRecordFieldKind::Boolean => Variant::from_bool(unsafe { *ptr.cast::<i16>() != 0 }),
        VbaRecordFieldKind::Record(layout) => {
            // SAFETY: the nested record slot at `ptr` is a live payload laid out by
            // `layout`; `clone_record_from_ptr` deep-copies it without consuming it.
            Variant::from_vba_record(unsafe { clone_record_from_ptr(ptr, layout.clone())? })
        }
        VbaRecordFieldKind::FixedArray { element, bounds } => {
            let len = fixed_array_total_len(bounds)?;
            let (element_size, element_align) = element.storage_shape()?;
            let stride = checked_align_to(element_size, element_align)?;
            let mut values = Vec::with_capacity(len);
            for i in 0..len {
                // SAFETY: `i < len` and `stride` is the element storage size, so
                // `i * stride` selects the live `i`-th element slot of this array.
                values.push(unsafe { read_field_variant_at(ptr.add(i * stride), element)? });
            }
            // A UDT fixed-array field is inherently fixed-size: surface
            // `FADF_FIXEDSIZE` so `Erase` of the materialized member array resets
            // its elements (and is then written back into the inline storage)
            // rather than deallocating — which the inline field cannot represent.
            Variant::from_safearray(
                SafeArray::from_variants_nd(bounds.clone(), values).with_fixed_size(true),
            )
        }
    };
    Ok(value)
}

/// # Safety
/// `ptr` must reference a field slot sized and aligned for `kind` that is already
/// initialized for that `kind` (so owning slots can be dropped before overwrite).
unsafe fn write_field_variant_at(
    ptr: *mut u8,
    kind: &VbaRecordFieldKind,
    value: &Variant,
) -> Result<(), String> {
    match kind {
        // SAFETY: the slot holds a live `Variant`; drop it once, then write the new
        // clone into the now-uninitialized slot — both ops target the same valid slot.
        VbaRecordFieldKind::Variant => unsafe {
            ptr.cast::<Variant>().drop_in_place();
            ptr.cast::<Variant>().write(value.clone());
        },
        VbaRecordFieldKind::Integer => {
            let value = value
                .as_i16()
                .ok_or_else(|| "Integer record field requires Integer value".to_string())?;
            // SAFETY: slot is an aligned `i16` (Copy scalar — direct overwrite).
            unsafe { ptr.cast::<i16>().write(value) };
        }
        VbaRecordFieldKind::Long => {
            let value = value
                .as_i32()
                .ok_or_else(|| "Long record field requires Long value".to_string())?;
            // SAFETY: slot is an aligned `i32` (Copy scalar — direct overwrite).
            unsafe { ptr.cast::<i32>().write(value) };
        }
        VbaRecordFieldKind::LongLong => {
            let value = value
                .as_i64()
                .ok_or_else(|| "LongLong record field requires LongLong value".to_string())?;
            // SAFETY: slot is an aligned `i64` (Copy scalar — direct overwrite).
            unsafe { ptr.cast::<i64>().write(value) };
        }
        VbaRecordFieldKind::Byte => {
            let value = value
                .as_u8()
                .or_else(|| value.as_i32().and_then(|value| u8::try_from(value).ok()))
                .or_else(|| value.as_i16().and_then(|value| u8::try_from(value).ok()))
                .ok_or_else(|| "Byte record field requires Byte value".to_string())?;
            // SAFETY: slot is a `u8` (Copy scalar — direct overwrite).
            unsafe { ptr.cast::<u8>().write(value) };
        }
        VbaRecordFieldKind::Single => {
            let value = value
                .as_f32()
                .or_else(|| value.as_f64().map(|value| value as f32))
                .or_else(|| value.as_i32().map(|value| value as f32))
                .or_else(|| value.as_i16().map(|value| value as f32))
                .or_else(|| value.as_u8().map(|value| value as f32))
                .ok_or_else(|| "Single record field requires Single value".to_string())?;
            // SAFETY: slot is an aligned `f32` (Copy scalar — direct overwrite).
            unsafe { ptr.cast::<f32>().write(value) };
        }
        VbaRecordFieldKind::Double => {
            let value = value
                .as_f64()
                .or_else(|| value.as_f32().map(f64::from))
                .or_else(|| value.as_i32().map(f64::from))
                .or_else(|| value.as_i16().map(f64::from))
                .or_else(|| value.as_u8().map(f64::from))
                .ok_or_else(|| "Double record field requires Double value".to_string())?;
            // SAFETY: slot is an aligned `f64` (Copy scalar — direct overwrite).
            unsafe { ptr.cast::<f64>().write(value) };
        }
        VbaRecordFieldKind::Currency => {
            let value = value
                .as_currency_scaled_i64()
                .ok_or_else(|| "Currency record field requires Currency value".to_string())?;
            // SAFETY: Currency stores a scaled `i64`; slot is an aligned `i64`.
            unsafe { ptr.cast::<i64>().write(value) };
        }
        VbaRecordFieldKind::Date => {
            let value = value
                .as_date_f64()
                .ok_or_else(|| "Date record field requires Date value".to_string())?;
            // SAFETY: Date stores an `f64` serial; slot is an aligned `f64`.
            unsafe { ptr.cast::<f64>().write(value) };
        }
        VbaRecordFieldKind::String => {
            let Some(text) = value.as_bstr() else {
                return Err("String record field requires String value".to_string());
            };
            let raw = text.raw_bstr();
            core::mem::forget(text);
            // SAFETY: `ptr` is the live String field — drop its current BSTR first so
            // it is not leaked, then store the owned BSTR we took from `value` above.
            unsafe { drop_field_at(ptr, kind) };
            // SAFETY: slot is the pointer-sized/aligned BSTR carrier, now uninitialized.
            unsafe { ptr.cast::<*mut u16>().write(raw) };
        }
        VbaRecordFieldKind::FixedString { len } => {
            if value.vtype() == crate::VarType::Null {
                return Err("Invalid use of Null".to_string());
            }
            let Some(text) = value.as_bstr() else {
                return Err("FixedString record field requires String value".to_string());
            };
            let mut units = text.to_utf16_units();
            if units.len() > *len {
                units.truncate(*len);
            } else {
                while units.len() < *len {
                    units.push(0x20);
                }
            }
            for (i, unit) in units.into_iter().enumerate() {
                // SAFETY: fixed-string fields are byte-packed in VBA records, so the
                // u16 code units may be unaligned. `write_unaligned` stores each unit.
                unsafe { ptr::write_unaligned(ptr.add(i * 2).cast::<u16>(), unit) };
            }
        }
        VbaRecordFieldKind::Boolean => {
            let value = value
                .as_bool()
                .ok_or_else(|| "Boolean record field requires Boolean value".to_string())?;
            // SAFETY: Boolean stores a VBA `i16` (-1/0); slot is an aligned `i16`.
            unsafe { ptr.cast::<i16>().write(if value { -1 } else { 0 }) };
        }
        VbaRecordFieldKind::Record(layout) => {
            let Some(source) = value.as_vba_record() else {
                return Err("nested record field requires VBA record value".to_string());
            };
            if source.layout().as_ref() != layout.as_ref() {
                return Err("nested record field layout mismatch".to_string());
            }
            // SAFETY: `ptr` is the live nested-record slot — drop its current fields,
            // then deep-copy `source` (verified same `layout`) into the freed slot.
            unsafe {
                drop_field_at(ptr, kind);
                clone_record_into_ptr(source.data_ptr(), ptr, layout)?;
            }
        }
        VbaRecordFieldKind::FixedArray { element, bounds } => {
            let len = fixed_array_total_len(bounds)?;
            let Some(array) = value.as_safearray() else {
                return Err("fixed-array record field assignment requires an array value".into());
            };
            let values = array.variant_elements().ok_or_else(|| {
                "fixed-array record field assignment requires materialized array elements"
                    .to_string()
            })?;
            if values.len() != len {
                return Err(format!(
                    "fixed-array record field assignment requires {len} elements, got {}",
                    values.len()
                ));
            }
            let (element_size, element_align) = element.storage_shape()?;
            let stride = checked_align_to(element_size, element_align)?;
            for (i, value) in values.iter().enumerate() {
                // SAFETY: `i < len` (length checked above) and `stride` is the element
                // storage size, so `i * stride` targets the live `i`-th element slot.
                unsafe { write_field_variant_at(ptr.add(i * stride), element, value)? };
            }
        }
    }
    Ok(())
}

/// # Safety
/// `src` must reference a live record payload initialized according to `layout`.
unsafe fn clone_record_from_ptr(
    src: *const u8,
    layout: Arc<VbaRecordLayout>,
) -> Result<VbaRecord, String> {
    let mut record = VbaRecord::new_default(layout.clone())?;
    // SAFETY: `src` is the live source payload (caller contract); `record` is a
    // freshly default-initialized destination with the identical `layout`.
    unsafe { clone_record_into_ptr(src, record.data_mut_ptr(), &layout)? };
    Ok(record)
}

/// # Safety
/// `src` must reference a live payload laid out by `layout`, and `dst` a distinct,
/// default-initialized payload of the same `layout`.
unsafe fn clone_record_into_ptr(
    src: *const u8,
    dst: *mut u8,
    layout: &VbaRecordLayout,
) -> Result<(), String> {
    for field in layout.fields() {
        // SAFETY: `field.offset` is in bounds of both same-layout payloads, and the
        // src/dst records are distinct buffers so the field slots do not overlap.
        unsafe { clone_field_at(src.add(field.offset), dst.add(field.offset), &field.kind)? };
    }
    Ok(())
}

unsafe fn borrow_bstr_raw(raw: *mut u16) -> BStr {
    // SAFETY: the caller guarantees `raw` is a live BSTR owned elsewhere. The
    // wrapper must be forgotten before it would drop.
    unsafe { BStr::from_raw_bstr(raw) }
}

fn clone_bstr_raw(raw: *mut u16) -> Result<*mut u16, String> {
    if raw.is_null() {
        return Ok(ptr::null_mut());
    }
    // SAFETY: `raw` is non-null (checked above) and is the live BSTR owned by the
    // record slot; we only borrow it to clone, then `forget` the borrowed wrapper
    // below so ownership stays with the original slot.
    let text = unsafe { borrow_bstr_raw(raw) };
    let cloned = text.clone_raw_bstr();
    core::mem::forget(text);
    cloned
}

fn validate_record_shape(fields: &[VbaRecordFieldSpec]) -> Result<(usize, usize), String> {
    if fields.len() > MAX_VBA_RECORD_SIZE {
        return Err(format!(
            "VBA record layout has {} fields; the sealed field table limit is {}",
            fields.len(),
            MAX_VBA_RECORD_SIZE
        ));
    }

    let mut offset = 0usize;
    let mut record_align = 1usize;
    for field in fields {
        let (size, align) = field.kind.storage_shape()?;
        validate_storage_alignment(align)?;
        offset = checked_align_to(offset, align)?;
        offset = offset
            .checked_add(size)
            .ok_or_else(|| "VBA record layout size overflow".to_string())?;
        validate_record_size(offset)?;
        record_align = record_align.max(align);
    }

    let size = checked_align_to(offset, record_align)?;
    validate_record_size(size)?;
    Ok((size, record_align))
}

fn validate_record_size(size: usize) -> Result<(), String> {
    if size > MAX_VBA_RECORD_SIZE {
        return Err(format!(
            "VBA record layout size {size} exceeds the 64 KiB limit ({MAX_VBA_RECORD_SIZE} bytes)"
        ));
    }
    Ok(())
}

fn validate_storage_alignment(align: usize) -> Result<(), String> {
    if align == 0 || !align.is_power_of_two() {
        return Err(format!(
            "VBA record field alignment {align} is not a non-zero power of two"
        ));
    }
    if align > core::mem::align_of::<u64>() {
        return Err(format!(
            "VBA record field alignment {align} exceeds native buffer alignment {}",
            core::mem::align_of::<u64>()
        ));
    }
    Ok(())
}

fn fixed_array_total_len(bounds: &[SafeArrayBound]) -> Result<usize, String> {
    if bounds.is_empty() {
        return Err("VBA fixed-array record field must have at least one dimension".into());
    }
    if bounds.len() > MAX_VBA_RECORD_FIXED_ARRAY_RANK {
        return Err(format!(
            "VBA fixed-array record field rank {} exceeds the {MAX_VBA_RECORD_FIXED_ARRAY_RANK}-dimension limit",
            bounds.len()
        ));
    }
    bounds.iter().try_fold(1usize, |total, bound| {
        if bound.count == 0 {
            return Err("VBA fixed-array record field must have at least one element".into());
        }
        total
            .checked_mul(bound.count as usize)
            .ok_or_else(|| "VBA fixed-array record field size overflow".to_string())
    })
}

fn checked_align_to(value: usize, align: usize) -> Result<usize, String> {
    if align == 0 || !align.is_power_of_two() {
        return Err(format!(
            "VBA record alignment {align} is not a non-zero power of two"
        ));
    }
    let mask = align - 1;
    value
        .checked_add(mask)
        .map(|value| value & !mask)
        .ok_or_else(|| "VBA record layout alignment overflow".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        FIELD_POINTER_PROJECTIONS, MAX_VBA_RECORD_FIXED_ARRAY_RANK, MAX_VBA_RECORD_SIZE,
        RECORD_BUFFER_EVENTS, VbaRecord, VbaRecordFieldHandle, VbaRecordFieldKind as Kind,
        VbaRecordFieldSpec as Field, VbaRecordLayout, checked_align_to,
    };
    use crate::safe_array::SafeArrayBound;
    use crate::{Variant, bstr::BStr};
    use std::sync::Arc;

    fn bounds(lower: i32, len: usize) -> Vec<SafeArrayBound> {
        vec![SafeArrayBound {
            lower,
            count: u32::try_from(len).expect("test bound fits u32"),
        }]
    }

    fn pointer_projection_count() -> usize {
        FIELD_POINTER_PROJECTIONS.with(core::cell::Cell::get)
    }

    fn record_buffer_events() -> (usize, usize) {
        RECORD_BUFFER_EVENTS.with(core::cell::Cell::get)
    }

    #[test]
    fn vba_record_layout_sealing_rejects_forged_and_cross_layout_handles_before_projection() {
        let before_buffers = record_buffer_events();
        let layout_a = Arc::new(
            VbaRecordLayout::new(vec![Field::named("Value", Kind::Long)]).expect("first layout"),
        );
        let layout_b = Arc::new(
            VbaRecordLayout::new(vec![Field::named("Value", Kind::Long)])
                .expect("structurally equal second layout"),
        );
        let mut record = VbaRecord::new_default(layout_a.clone()).expect("record");
        let other_handle = layout_b.field_handle(0).expect("other handle");
        let forged_handle = VbaRecordFieldHandle {
            layout: layout_a.clone(),
            index: usize::MAX,
        };

        let before_projection = pointer_projection_count();
        assert_eq!(
            record
                .field_ptr(&other_handle)
                .expect_err("cross-layout handle"),
            "record field handle belongs to a different layout"
        );
        assert_eq!(pointer_projection_count(), before_projection);
        assert_eq!(
            record
                .field_mut_ptr(&forged_handle)
                .expect_err("forged index"),
            format!("record field handle index {} is out of range", usize::MAX)
        );
        assert_eq!(pointer_projection_count(), before_projection);
        assert!(layout_a.field_handle(usize::MAX).is_none());

        let own_handle = record.field_handle(0).expect("own handle");
        let ptr = record.field_ptr(&own_handle).expect("validated pointer");
        assert_eq!(ptr as usize % own_handle.align(), 0);
        assert_eq!(pointer_projection_count(), before_projection + 1);

        drop(record);
        let after_buffers = record_buffer_events();
        assert_eq!(after_buffers.0 - before_buffers.0, 1);
        assert_eq!(after_buffers.1 - before_buffers.1, 1);
    }

    #[test]
    fn vba_record_layout_sealing_rejects_hostile_shape_inputs_before_record_allocation() {
        let before_buffers = record_buffer_events();
        let before_projection = pointer_projection_count();

        assert_eq!(
            VbaRecordLayout::new(vec![Field::named(
                "Text",
                Kind::FixedString { len: usize::MAX },
            )])
            .expect_err("fixed string multiplication overflow"),
            "VBA fixed-string record field size overflow"
        );
        assert_eq!(
            VbaRecordLayout::new(vec![Field::named("Text", Kind::FixedString { len: 0 })])
                .expect_err("zero-size fixed string"),
            "VBA fixed-string record field must have at least one character"
        );

        let excessive_rank =
            vec![SafeArrayBound { count: 1, lower: 0 }; MAX_VBA_RECORD_FIXED_ARRAY_RANK + 1];
        assert_eq!(
            VbaRecordLayout::new(vec![Field::named(
                "Values",
                Kind::FixedArray {
                    element: Box::new(Kind::Byte),
                    bounds: excessive_rank,
                },
            )])
            .expect_err("rank limit"),
            format!(
                "VBA fixed-array record field rank {} exceeds the {MAX_VBA_RECORD_FIXED_ARRAY_RANK}-dimension limit",
                MAX_VBA_RECORD_FIXED_ARRAY_RANK + 1
            )
        );

        let overflowing_bounds = vec![
            SafeArrayBound {
                count: u32::MAX,
                lower: 0,
            };
            3
        ];
        assert_eq!(
            VbaRecordLayout::new(vec![Field::named(
                "Values",
                Kind::FixedArray {
                    element: Box::new(Kind::Byte),
                    bounds: overflowing_bounds,
                },
            )])
            .expect_err("element-count overflow"),
            "VBA fixed-array record field size overflow"
        );

        let over_limit_units = MAX_VBA_RECORD_SIZE / core::mem::size_of::<u16>() + 1;
        assert!(
            VbaRecordLayout::new(vec![Field::named(
                "Text",
                Kind::FixedString {
                    len: over_limit_units,
                },
            )])
            .expect_err("64 KiB record limit")
            .contains("exceeds the 64 KiB limit")
        );
        assert_eq!(
            checked_align_to(usize::MAX, 8).expect_err("alignment overflow"),
            "VBA record layout alignment overflow"
        );
        assert_eq!(
            checked_align_to(8, 3).expect_err("invalid alignment"),
            "VBA record alignment 3 is not a non-zero power of two"
        );

        assert_eq!(pointer_projection_count(), before_projection);
        assert_eq!(record_buffer_events(), before_buffers);
    }

    #[test]
    fn vba_record_layout_sealing_preserves_nested_fixed_array_alignment_and_extents() {
        let before_buffers = record_buffer_events();
        let inner = Arc::new(
            VbaRecordLayout::new(vec![
                Field::named("Tag", Kind::Byte),
                Field::named("Value", Kind::LongLong),
            ])
            .expect("inner layout"),
        );
        let outer = Arc::new(
            VbaRecordLayout::new(vec![
                Field::named("Prefix", Kind::Byte),
                Field::named("Nested", Kind::Record(inner.clone())),
                Field::named(
                    "Items",
                    Kind::FixedArray {
                        element: Box::new(Kind::Record(inner)),
                        bounds: bounds(-1, 2),
                    },
                ),
                Field::named("Tail", Kind::FixedString { len: 3 }),
            ])
            .expect("outer layout"),
        );
        let record = VbaRecord::new_default(outer.clone()).expect("outer record");

        let mut prior_end = 0usize;
        for index in 0..outer.fields().len() {
            let descriptor = &outer.fields()[index];
            let handle = record.field_handle(index).expect("field handle");
            assert_eq!(handle.index(), index);
            assert_eq!(handle.offset(), descriptor.offset());
            assert_eq!(handle.size(), descriptor.size());
            assert_eq!(handle.kind(), descriptor.kind());
            assert!(handle.offset() >= prior_end);
            assert_eq!(handle.offset() % handle.align(), 0);
            assert!(handle.offset() + handle.size() <= outer.size());
            let ptr = record.field_ptr(&handle).expect("aligned field pointer");
            assert_eq!(ptr as usize % handle.align(), 0);
            prior_end = handle.offset() + handle.size();
        }

        let materialized = record
            .read_array_field_element(2, 1)
            .expect("fixed-array read")
            .expect("fixed-array element");
        assert!(materialized.as_vba_record().is_some());

        drop(materialized);
        drop(record);
        let after_buffers = record_buffer_events();
        assert_eq!(
            after_buffers.0 - before_buffers.0,
            after_buffers.1 - before_buffers.1
        );
    }

    #[test]
    fn vba_record_layout_sealing_allows_maximum_bounded_allocation_and_balances() {
        let before_buffers = record_buffer_events();
        let units = MAX_VBA_RECORD_SIZE / core::mem::size_of::<u16>();
        let layout = Arc::new(
            VbaRecordLayout::new(vec![Field::named("Text", Kind::FixedString { len: units })])
                .expect("maximum layout"),
        );
        assert_eq!(layout.size(), MAX_VBA_RECORD_SIZE);

        let record = VbaRecord::new_default(layout).expect("bounded fallible allocation");
        assert_eq!(record.memory_len(), MAX_VBA_RECORD_SIZE);
        assert_eq!(
            record.field_bytes(0).expect("field extent").len(),
            MAX_VBA_RECORD_SIZE
        );
        drop(record);

        let after_buffers = record_buffer_events();
        assert_eq!(after_buffers.0 - before_buffers.0, 1);
        assert_eq!(after_buffers.1 - before_buffers.1, 1);
    }

    #[test]
    fn guid_shaped_udt_layout_matches_native_offsets() {
        let layout = VbaRecordLayout::new(vec![
            Field::named("Data1", Kind::Long),
            Field::named("Data2", Kind::Integer),
            Field::named("Data3", Kind::Integer),
            Field::named(
                "Data4",
                Kind::FixedArray {
                    element: Box::new(Kind::Byte),
                    bounds: bounds(0, 8),
                },
            ),
        ])
        .expect("layout");

        let offsets: Vec<_> = layout.fields().iter().map(|field| field.offset).collect();
        assert_eq!(offsets, vec![0, 4, 6, 8]);
        assert_eq!(layout.size(), 16);
        assert_eq!(layout.align(), core::mem::align_of::<i32>());
        assert_eq!(layout.fields()[3].size, 8);
    }

    #[test]
    fn nested_records_align_to_their_payload() {
        let inner = VbaRecordLayout::new(vec![
            Field::named("Flag", Kind::Boolean),
            Field::named("Value", Kind::Long),
        ])
        .expect("inner");
        let layout = VbaRecordLayout::new(vec![
            Field::named("Tag", Kind::Byte),
            Field::named("Inner", Kind::Record(std::sync::Arc::new(inner))),
            Field::named("Tail", Kind::Integer),
        ])
        .expect("layout");

        assert_eq!(layout.fields()[0].offset, 0);
        assert_eq!(layout.fields()[1].offset, 4);
        assert_eq!(layout.fields()[2].offset, 12);
        assert_eq!(layout.size(), 16);
    }

    #[test]
    fn byref_as_any_native_abi_admits_nested_fixed_array_records() {
        let inner = Arc::new(
            VbaRecordLayout::new(vec![
                Field::named("Flag", Kind::Boolean),
                Field::named("Value", Kind::Long),
            ])
            .expect("inner"),
        );
        let layout = VbaRecordLayout::new(vec![
            Field::named("Tag", Kind::Integer),
            Field::named("Inner", Kind::Record(inner)),
            Field::named(
                "Bytes",
                Kind::FixedArray {
                    element: Box::new(Kind::Byte),
                    bounds: bounds(0, 4),
                },
            ),
            Field::named("Total", Kind::Currency),
        ])
        .expect("layout");

        layout
            .validate_byref_as_any_native_abi()
            .expect("plain native fields should be eligible for ByRef As Any");
    }

    #[test]
    fn byref_as_any_native_abi_rejects_owning_record_fields() {
        let with_string =
            VbaRecordLayout::new(vec![Field::named("Text", Kind::String)]).expect("layout");
        let string_error = with_string
            .validate_byref_as_any_native_abi()
            .expect_err("String record fields are not plain native bytes");
        assert!(string_error.contains("Text"));
        assert!(string_error.contains("String fields"));

        let with_variant =
            VbaRecordLayout::new(vec![Field::named("Value", Kind::Variant)]).expect("layout");
        let variant_error = with_variant
            .validate_byref_as_any_native_abi()
            .expect_err("Variant record fields are not plain native bytes");
        assert!(variant_error.contains("Value"));
        assert!(variant_error.contains("Variant fields"));
    }

    #[test]
    fn pointer_and_variant_fields_use_runtime_carrier_shapes() {
        let layout = VbaRecordLayout::new(vec![
            Field::named("B", Kind::Byte),
            Field::named("S", Kind::String),
            Field::named("V", Kind::Variant),
        ])
        .expect("layout");

        assert_eq!(layout.fields()[0].offset, 0);
        assert_eq!(
            layout.fields()[1].offset,
            core::mem::align_of::<*mut core::ffi::c_void>()
        );
        assert_eq!(
            layout.fields()[2].offset % core::mem::align_of::<crate::VariantCore>(),
            0
        );
        assert_eq!(
            layout.fields()[2].size,
            core::mem::size_of::<crate::VariantCore>()
        );
    }

    #[test]
    fn fixed_string_fields_are_inline_byte_packed_utf16() {
        let layout = VbaRecordLayout::new(vec![
            Field::named("B", Kind::Byte),
            Field::named("Name", Kind::FixedString { len: 5 }),
            Field::named("Tail", Kind::Integer),
        ])
        .expect("layout");

        assert_eq!(
            layout
                .fields()
                .iter()
                .map(|field| (field.offset, field.size, field.align))
                .collect::<Vec<_>>(),
            vec![(0, 1, 1), (1, 10, 1), (12, 2, 2)]
        );
        assert_eq!(layout.size(), 14);
        assert_eq!(layout.file_len().expect("file len"), 8);
    }

    #[test]
    fn fixed_string_fields_default_to_nuls_and_assign_with_spaces() {
        let layout = Arc::new(
            VbaRecordLayout::new(vec![Field::named("Name", Kind::FixedString { len: 5 })])
                .expect("layout"),
        );
        let mut record = VbaRecord::new_default(layout).expect("record");

        assert_eq!(
            record
                .read_field_variant(0)
                .expect("default")
                .as_bstr()
                .expect("string")
                .to_utf16_units(),
            vec![0, 0, 0, 0, 0]
        );

        record
            .write_field_variant(0, &Variant::from_string("ab"))
            .expect("write short");
        assert_eq!(
            record
                .read_field_variant(0)
                .expect("short")
                .as_bstr()
                .expect("string")
                .as_str(),
            "ab   "
        );

        record
            .write_field_variant(0, &Variant::from_string("abcdef"))
            .expect("write long");
        assert_eq!(
            record
                .read_field_variant(0)
                .expect("long")
                .as_bstr()
                .expect("string")
                .as_str(),
            "abcde"
        );
    }

    #[test]
    fn native_record_defaults_and_reads_scalar_fields_from_buffer() {
        let layout = Arc::new(
            VbaRecordLayout::new(vec![
                Field::named("Id", Kind::Long),
                Field::named(
                    "Bytes",
                    Kind::FixedArray {
                        element: Box::new(Kind::Byte),
                        bounds: bounds(0, 4),
                    },
                ),
            ])
            .expect("layout"),
        );
        let mut record = VbaRecord::new_default(layout).expect("record");
        let fields = [
            record.field_handle(0).expect("id field"),
            record.field_handle(1).expect("bytes field"),
        ];

        // SAFETY: `fields` are this record's own layout fields, so `field_mut_ptr`
        // yields in-bounds slots: a `Long` (i32) slot and a 4-byte fixed-array slot.
        unsafe {
            record
                .field_mut_ptr(&fields[0])
                .expect("id field pointer")
                .cast::<i32>()
                .write(1234);
            core::ptr::copy_nonoverlapping(
                [1u8, 2, 3, 4].as_ptr(),
                record
                    .field_mut_ptr(&fields[1])
                    .expect("bytes field pointer"),
                4,
            );
        }

        assert_eq!(
            record.read_field_variant(0).expect("id").as_i32(),
            Some(1234)
        );
        assert_eq!(record.field_bytes(1).expect("bytes"), &[1, 2, 3, 4]);
    }

    #[test]
    fn fixed_array_record_field_projects_to_array_and_writes_back_inline_storage() {
        let layout = Arc::new(
            VbaRecordLayout::new(vec![Field::named(
                "Bytes",
                Kind::FixedArray {
                    element: Box::new(Kind::Byte),
                    bounds: bounds(1, 4),
                },
            )])
            .expect("layout"),
        );
        let mut record = VbaRecord::new_default(layout).expect("record");

        let mut array = record
            .read_field_variant(0)
            .expect("fixed array projection")
            .as_safearray()
            .expect("array projection");
        assert_eq!(array.bounds(), Some(bounds(1, 4)));
        array
            .set_variant_element(1, &Variant::from_u8(0xAB))
            .expect("set array element");
        array
            .set_variant_element(3, &Variant::from_u8(0xCD))
            .expect("set array element");
        record
            .write_field_variant(0, &Variant::from_safearray(array))
            .expect("write fixed array projection back");

        assert_eq!(record.field_bytes(0).expect("bytes"), &[0, 0xAB, 0, 0xCD]);
    }

    #[test]
    fn lset_record_overlay_copies_prefix_and_preserves_target_tail() {
        let target_layout = Arc::new(
            VbaRecordLayout::new(vec![Field::named("Text", Kind::FixedString { len: 4 })])
                .expect("target layout"),
        );
        let source_layout = Arc::new(
            VbaRecordLayout::new(vec![Field::named("Text", Kind::FixedString { len: 2 })])
                .expect("source layout"),
        );
        let mut target = VbaRecord::new_default(target_layout).expect("target");
        let mut source = VbaRecord::new_default(source_layout).expect("source");
        target
            .write_field_variant(0, &Variant::from_string("zzzz"))
            .expect("target text");
        source
            .write_field_variant(0, &Variant::from_string("xy"))
            .expect("source text");

        target.lset_from(&source).expect("lset");

        assert_eq!(
            target
                .read_field_variant(0)
                .expect("target after lset")
                .as_bstr()
                .expect("string")
                .as_str(),
            "xyzz"
        );
    }

    #[test]
    fn lset_record_overlay_truncates_longer_source() {
        let target_layout = Arc::new(
            VbaRecordLayout::new(vec![Field::named("Text", Kind::FixedString { len: 2 })])
                .expect("target layout"),
        );
        let source_layout = Arc::new(
            VbaRecordLayout::new(vec![Field::named("Text", Kind::FixedString { len: 4 })])
                .expect("source layout"),
        );
        let mut target = VbaRecord::new_default(target_layout).expect("target");
        let mut source = VbaRecord::new_default(source_layout).expect("source");
        source
            .write_field_variant(0, &Variant::from_string("wxyz"))
            .expect("source text");

        target.lset_from(&source).expect("lset");

        assert_eq!(
            target
                .read_field_variant(0)
                .expect("target after lset")
                .as_bstr()
                .expect("string")
                .as_str(),
            "wx"
        );
    }

    #[test]
    fn lset_record_overlay_reinterprets_same_size_storage() {
        let target_layout = Arc::new(
            VbaRecordLayout::new(vec![
                Field::named("I", Kind::Integer),
                Field::named("B1", Kind::Byte),
                Field::named("B2", Kind::Byte),
            ])
            .expect("target layout"),
        );
        let source_layout = Arc::new(
            VbaRecordLayout::new(vec![Field::named("L", Kind::Long)]).expect("source layout"),
        );
        let mut target = VbaRecord::new_default(target_layout).expect("target");
        let mut source = VbaRecord::new_default(source_layout).expect("source");
        source
            .write_field_variant(0, &Variant::from_i32(0x0403_0201))
            .expect("source long");

        target.lset_from(&source).expect("lset");

        assert_eq!(
            target.read_field_variant(0).expect("integer").as_i16(),
            Some(513)
        );
        assert_eq!(target.read_field_variant(1).expect("b1").as_u8(), Some(3));
        assert_eq!(target.read_field_variant(2).expect("b2").as_u8(), Some(4));
    }

    #[test]
    fn lset_record_overlay_rejects_owning_fields() {
        let layout = Arc::new(
            VbaRecordLayout::new(vec![Field::named("Text", Kind::String)]).expect("layout"),
        );
        let mut target = VbaRecord::new_default(layout.clone()).expect("target");
        let source = VbaRecord::new_default(layout).expect("source");

        let err = target
            .lset_from(&source)
            .expect_err("variable strings are not LSet byte-overlay compatible");
        assert_eq!(err, "Type mismatch");
    }

    #[test]
    fn native_record_clone_deep_copies_bstr_and_variant_fields() {
        let layout = Arc::new(
            VbaRecordLayout::new(vec![
                Field::named("Text", Kind::String),
                Field::named("Value", Kind::Variant),
            ])
            .expect("layout"),
        );
        let mut record = VbaRecord::new_default(layout).expect("record");
        let fields = [
            record.field_handle(0).expect("text field"),
            record.field_handle(1).expect("value field"),
        ];
        let bstr = BStr::from("alpha");
        let raw_bstr = bstr.clone_raw_bstr().expect("clone bstr");

        // SAFETY: `fields[0]` is the String (BSTR-carrier) slot and `fields[1]` the
        // Variant slot of this record. We hand the String slot an owned BSTR, and
        // drop the default-initialized Variant before writing the replacement.
        unsafe {
            record
                .field_mut_ptr(&fields[0])
                .expect("text field pointer")
                .cast::<*mut u16>()
                .write(raw_bstr);
            record
                .field_mut_ptr(&fields[1])
                .expect("value field pointer")
                .cast::<Variant>()
                .drop_in_place();
            record
                .field_mut_ptr(&fields[1])
                .expect("value field pointer")
                .cast::<Variant>()
                .write(Variant::from_string("payload"));
        }

        let clone = record.clone();
        // SAFETY: `fields[0]` is the String slot in both records; read its BSTR
        // carrier pointer (without taking ownership) to compare original vs clone.
        let original_raw = unsafe {
            *record
                .field_ptr(&fields[0])
                .expect("text field pointer")
                .cast::<*mut u16>()
        };
        // SAFETY: same as above, reading the cloned record's String-slot pointer.
        let clone_raw = unsafe {
            *clone
                .field_ptr(&fields[0])
                .expect("clone text field pointer")
                .cast::<*mut u16>()
        };

        assert!(!original_raw.is_null());
        assert!(!clone_raw.is_null());
        assert_ne!(original_raw, clone_raw);
        assert_eq!(
            clone
                .read_field_variant(0)
                .expect("text")
                .as_bstr()
                .map(|text| text.as_str()),
            Some("alpha".to_string())
        );
        assert_eq!(
            clone
                .read_field_variant(1)
                .expect("variant")
                .as_bstr()
                .map(|text| text.as_str()),
            Some("payload".to_string())
        );
    }
}
