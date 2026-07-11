use crate::{
    Variant, VariantCore,
    bstr::{BStr, borrow_raw_bstr},
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

/// OxVba verifier/security limit for recursive record-layout metadata.
///
/// VBA rejects direct and indirect self-reference, but its published
/// documentation does not define a maximum finite nesting depth. This cap is a
/// resource-safety admission policy, not a VBA semantic limit.
pub const MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH: usize = 64;

/// Maximum aggregate metadata nodes visited while sealing one record layout.
///
/// This is derived from the 64 KiB non-zero field budget times the graph-depth
/// cap, so a layout satisfying both primary bounds cannot be rejected merely
/// for using a wide graph. It also bounds validation work for a hostile DAG.
pub const MAX_VBA_RECORD_LAYOUT_GRAPH_VISITS: usize =
    MAX_VBA_RECORD_SIZE * MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH;

#[cfg(test)]
std::thread_local! {
    static FIELD_POINTER_PROJECTIONS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    static RECORD_BUFFER_EVENTS: core::cell::Cell<(usize, usize)> = const { core::cell::Cell::new((0, 0)) };
    static LAYOUT_FIELD_TABLE_RESERVATIONS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    static OWNING_BOUNDARY_FAILURE: core::cell::Cell<Option<(usize, OwningFailureMode)>> = const { core::cell::Cell::new(None) };
    static OWNING_BOUNDARY_TRACE: core::cell::RefCell<Vec<&'static str>> = const { core::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OwningFailureMode {
    Error,
    Panic,
}

#[cfg(test)]
pub(crate) fn inject_owning_boundary_failure(nth: usize, mode: OwningFailureMode) {
    OWNING_BOUNDARY_TRACE.with(|trace| trace.borrow_mut().clear());
    OWNING_BOUNDARY_FAILURE.with(|failure| failure.set(Some((nth, mode))));
}

#[cfg(test)]
pub(crate) fn clear_owning_boundary_failure() {
    OWNING_BOUNDARY_FAILURE.with(|failure| failure.set(None));
}

#[cfg(test)]
pub(crate) fn take_owning_boundary_trace() -> Vec<&'static str> {
    OWNING_BOUNDARY_TRACE.with(|trace| core::mem::take(&mut *trace.borrow_mut()))
}

#[cfg(test)]
pub(crate) fn owning_boundary(name: &'static str) -> Result<(), String> {
    OWNING_BOUNDARY_TRACE.with(|trace| trace.borrow_mut().push(name));
    let failure = OWNING_BOUNDARY_FAILURE.with(|state| match state.get() {
        Some((0, mode)) => {
            state.set(None);
            Some(mode)
        }
        Some((remaining, mode)) => {
            state.set(Some((remaining - 1, mode)));
            None
        }
        None => None,
    });
    match failure {
        Some(OwningFailureMode::Error) => Err(format!(
            "injected owning clone/allocation failure at {name}"
        )),
        Some(OwningFailureMode::Panic) => {
            panic!("injected owning clone/allocation panic at {name}")
        }
        None => Ok(()),
    }
}

#[cfg(not(test))]
#[inline]
pub(crate) fn owning_boundary(_name: &'static str) -> Result<(), String> {
    Ok(())
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

#[cfg(test)]
pub(crate) fn record_buffer_event_counts() -> (usize, usize) {
    RECORD_BUFFER_EVENTS.with(core::cell::Cell::get)
}

#[cfg(test)]
fn note_layout_field_table_reservation() {
    LAYOUT_FIELD_TABLE_RESERVATIONS.with(|count| count.set(count.get() + 1));
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
    graph_depth: usize,
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
        let (size, align, graph_depth) = validate_record_shape(&fields)?;

        let mut offset = 0usize;
        let mut layouts = Vec::new();
        #[cfg(test)]
        note_layout_field_table_reservation();
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
            graph_depth,
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

    pub fn graph_depth(&self) -> usize {
        self.graph_depth
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
            Self::FixedArray { element, bounds } => {
                reject_nested_fixed_array_element(element)?;
                element
                    .file_len()?
                    .checked_mul(fixed_array_total_len(bounds)?)
                    .ok_or_else(|| "VBA fixed-array record file length overflow".to_string())?
            }
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
                reject_nested_fixed_array_element(element)?;
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
        owning_boundary("record-buffer-allocation")?;
        let mut data = Vec::new();
        data.try_reserve_exact(words).map_err(|_| {
            format!(
                "VBA record buffer allocation failed for {} bytes",
                layout.size()
            )
        })?;
        data.resize(words, 0);
        let record = Self { layout, data };
        crate::live_counters::record_buffer_allocated();
        #[cfg(test)]
        note_record_buffer_allocation();
        // Every admitted field kind has a valid all-zero default: numeric/fixed
        // storage is zero, String is a null BSTR, Variant is VT_EMPTY (tag zero),
        // and Record/FixedArray recurse over those same defaults. The complete
        // buffer is therefore initialized before `record` can escape, with no
        // fallible per-field phase that could leave a partial owner.
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
        let field = self.checked_field_by_index(index)?;
        // SAFETY: the field pointer is in range and aligned for `field.kind`.
        unsafe { read_field_variant_at(self.field_ptr_by_index(index)?, &field.kind) }
    }

    pub fn write_field_variant(&mut self, index: usize, value: &Variant) -> Result<(), String> {
        let layout = Arc::clone(&self.layout);
        let field = layout
            .fields()
            .get(index)
            .ok_or_else(|| format!("record field {index} out of range"))?;
        // SAFETY: the field pointer is in range and aligned for `field.kind`.
        unsafe { write_field_variant_at(self.field_mut_ptr_by_index(index)?, &field.kind, value) }
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
        let field = self.checked_field_by_index(index)?;
        match &field.kind {
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
        let field = self.checked_field_by_index(index)?;
        match &field.kind {
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
        let layout = Arc::clone(&self.layout);
        let field = layout
            .fields()
            .get(index)
            .ok_or_else(|| format!("record field {index} out of range"))?;
        match &field.kind {
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
        let replacement = self.try_clone()?;
        // SAFETY: the caller supplied writable, aligned, uninitialized storage;
        // `replacement` is a completely initialized same-layout owner. Moving its
        // bytes touches `dst` only after every fallible clone has succeeded.
        unsafe { replacement.move_into_raw(dst) };
        Ok(())
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

    pub(crate) fn try_clone(&self) -> Result<Self, String> {
        let layout = Arc::clone(&self.layout);
        let mut clone = Self::new_default(Arc::clone(&layout))?;
        for (index, field) in layout.fields().iter().enumerate() {
            // SAFETY: source and destination are distinct, fully initialized
            // same-layout buffers. Each owning replacement prepares before commit,
            // so `clone` remains a valid default/partial-success owner on failure.
            unsafe {
                clone_field_into_live(
                    self.field_ptr_by_index(index)?,
                    clone.field_mut_ptr_by_index(index)?,
                    &field.kind,
                )?;
            }
        }
        Ok(clone)
    }

    /// Swap this owned record payload with a live inline/raw payload of the same layout.
    ///
    /// # Safety
    /// `other` must point to a distinct, writable, fully initialized payload laid out by
    /// `self.layout()`. After return, `self` owns the prior `other` payload and the caller
    /// owns the prior `self` payload at `other`.
    pub(crate) unsafe fn swap_with_raw(&mut self, other: *mut u8) {
        let size = self.layout().size();
        let owned = self.data_mut_ptr();
        // SAFETY: caller guarantees distinct same-layout initialized payloads;
        // byte-wise swap transfers every owning field exactly once and cannot fail.
        unsafe { ptr::swap_nonoverlapping(owned, other, size) };
    }

    /// Move this complete payload into uninitialized raw storage without dropping fields.
    ///
    /// # Safety
    /// `dst` must be distinct, writable, aligned storage of at least `layout.size()` bytes.
    unsafe fn move_into_raw(self, dst: *mut u8) {
        let this = core::mem::ManuallyDrop::new(self);
        // SAFETY: caller guarantees the distinct destination extent. `this` is a
        // complete initialized owner, and copying the exact layout size transfers
        // its complete byte image before any backing allocation is released.
        unsafe { ptr::copy_nonoverlapping(this.data_ptr(), dst, this.layout().size()) };
        // SAFETY: `this` will not run Drop. Read out the two ordinary owner fields
        // exactly once so their backing allocations/references are released without
        // dropping the record payload whose ownership just moved to `dst`.
        let data = unsafe { ptr::read(&this.data) };
        // SAFETY: same ManuallyDrop ownership transfer as for `data` above.
        let layout = unsafe { ptr::read(&this.layout) };
        drop(data);
        drop(layout);
        crate::live_counters::record_buffer_freed();
        #[cfg(test)]
        note_record_buffer_free();
    }
}

impl Clone for VbaRecord {
    fn clone(&self) -> Self {
        self.try_clone()
            .expect("VBA record deep clone should succeed")
    }
}

impl Drop for VbaRecord {
    fn drop(&mut self) {
        let data_ptr = self.data.as_mut_ptr().cast::<u8>();
        for field in self.layout.fields() {
            // SAFETY: each initialized field is dropped exactly once while the owning
            // record buffer is still live.
            unsafe {
                drop_field_at(data_ptr.add(field.offset), &field.kind);
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

/// Complete initialized owner for one inline field while a replacement is prepared.
///
/// Every admitted field kind has a valid all-zero default (the same invariant used by
/// `VbaRecord::new_default`). Drop therefore always sees a complete live field, including
/// when a later fallible clone returns or unwinds. `commit` swaps once and makes this guard
/// own the old destination for ordinary cleanup.
struct OwnedFieldBuffer<'a> {
    kind: &'a VbaRecordFieldKind,
    size: usize,
    words: Vec<u64>,
}

impl<'a> OwnedFieldBuffer<'a> {
    fn new_default(kind: &'a VbaRecordFieldKind) -> Result<Self, String> {
        let (size, align) = kind.storage_shape()?;
        if align > core::mem::align_of::<u64>() {
            return Err(format!(
                "record field alignment {align} exceeds transactional buffer alignment"
            ));
        }
        owning_boundary("field-buffer-allocation")?;
        let word_count = size.div_ceil(core::mem::size_of::<u64>());
        let mut words = Vec::new();
        words.try_reserve_exact(word_count).map_err(|_| {
            format!("transactional record field allocation failed for {size} bytes")
        })?;
        words.resize(word_count, 0);
        Ok(Self { kind, size, words })
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.words.as_mut_ptr().cast()
    }

    unsafe fn clone_from_raw(src: *const u8, kind: &'a VbaRecordFieldKind) -> Result<Self, String> {
        let mut replacement = Self::new_default(kind)?;
        let dst = replacement.as_mut_ptr();
        match kind {
            VbaRecordFieldKind::Record(layout) => {
                for field in layout.fields() {
                    // SAFETY: the sealed nested layout proves each matching source
                    // and destination sub-field extent is in bounds and distinct.
                    unsafe {
                        clone_field_into_live(
                            src.add(field.offset),
                            dst.add(field.offset),
                            &field.kind,
                        )?
                    };
                }
            }
            VbaRecordFieldKind::FixedArray { element, bounds } => {
                let len = fixed_array_total_len(bounds)?;
                let (element_size, element_align) = element.storage_shape()?;
                let stride = checked_align_to(element_size, element_align)?;
                for index in 0..len {
                    // SAFETY: `index < len` and the sealed array extent uses this
                    // exact stride, selecting matching distinct live elements.
                    unsafe {
                        clone_field_into_live(
                            src.add(index * stride),
                            dst.add(index * stride),
                            element,
                        )?
                    };
                }
            }
            _ => {
                // SAFETY: caller supplies a live source of `kind`; the replacement
                // buffer is a distinct, complete all-zero default of the same kind.
                unsafe { clone_field_into_live(src, dst, kind)? };
            }
        }
        Ok(replacement)
    }

    /// Swap the prepared value into `dst`; this guard then owns the previous value.
    ///
    /// # Safety
    /// `dst` must be a distinct, writable, initialized field of `self.kind`.
    unsafe fn commit(mut self, dst: *mut u8) {
        let size = self.size;
        let src = self.as_mut_ptr();
        // SAFETY: caller guarantees distinct same-kind fields of `self.size` bytes.
        unsafe { ptr::swap_nonoverlapping(src, dst, size) };
        // `self` drops the old destination after the slot already owns the replacement.
    }
}

impl Drop for OwnedFieldBuffer<'_> {
    fn drop(&mut self) {
        let kind = self.kind;
        let ptr = self.as_mut_ptr();
        // SAFETY: construction starts with a complete valid all-zero default;
        // transactional writes preserve validity, and commit only swaps another
        // complete live field into this buffer.
        unsafe { drop_field_at(ptr, kind) };
    }
}

/// Clone `src` transactionally over a distinct initialized `dst` field.
///
/// # Safety
/// Both pointers must reference distinct live fields initialized for `kind`.
unsafe fn clone_field_into_live(
    src: *const u8,
    dst: *mut u8,
    kind: &VbaRecordFieldKind,
) -> Result<(), String> {
    match kind {
        VbaRecordFieldKind::Variant => {
            owning_boundary("variant-clone")?;
            // SAFETY: `src` holds a live `Variant` per the contract; borrowing it
            // shared to clone does not move or invalidate the source slot.
            let value = unsafe { &*src.cast::<Variant>() };
            let replacement = value.try_clone()?;
            // SAFETY: replacement is complete before mutation; `dst` is a live
            // Variant slot. Replace makes the new slot valid before old Drop runs.
            let previous = unsafe { dst.cast::<Variant>().replace(replacement) };
            drop(previous);
        }
        VbaRecordFieldKind::String => {
            // SAFETY: `src` holds the initialized BSTR carrier pointer for this field.
            let raw = unsafe { *src.cast::<*mut u16>() };
            let cloned = clone_bstr_raw(raw)?;
            // SAFETY: cloned is fully owned before mutation and `dst` is a live
            // pointer carrier. Replace keeps the slot initialized throughout.
            let previous = unsafe { dst.cast::<*mut u16>().replace(cloned) };
            if !previous.is_null() {
                // SAFETY: the previous live String field owned this BSTR exactly once.
                drop(unsafe { BStr::from_raw_bstr(previous) });
            }
        }
        VbaRecordFieldKind::Record(_) | VbaRecordFieldKind::FixedArray { .. } => {
            // SAFETY: forwards this function's live-source contract. Preparation
            // completes recursively in an owned guard before the one-shot swap.
            let replacement = unsafe { OwnedFieldBuffer::clone_from_raw(src, kind)? };
            // SAFETY: `dst` is the distinct live same-kind destination.
            unsafe { replacement.commit(dst) };
        }
        _ => {
            let (size, _) = kind.storage_shape()?;
            // SAFETY: Copy scalar fields may overwrite the live destination; both
            // slots are `size` bytes and non-overlapping per the contract.
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
        VbaRecordFieldKind::Variant => unsafe { &*ptr.cast::<Variant>() }.try_clone()?,
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
                // SAFETY: the record keeps this raw BSTR live for the duration
                // of the destructor-free borrowed view and owned clone.
                Variant::from_string(unsafe { borrow_raw_bstr(raw) }.try_to_owned()?)
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
        VbaRecordFieldKind::Variant => {
            owning_boundary("variant-assignment-clone")?;
            let replacement = value.try_clone()?;
            // SAFETY: replacement is complete before mutation and `ptr` is a live
            // Variant slot. Replace commits once before the prior owner drops.
            let previous = unsafe { ptr.cast::<Variant>().replace(replacement) };
            drop(previous);
        }
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
            if value.vtype() != crate::VarType::String {
                return Err("String record field requires String value".to_string());
            }
            owning_boundary("string-assignment-clone")?;
            let text = value
                .try_as_bstr()?
                .expect("String Variant must retain its BSTR payload");
            let raw = text.into_raw_bstr();
            // SAFETY: the replacement BSTR is fully owned before mutation and `ptr`
            // is a live pointer carrier. Replace keeps the slot initialized.
            let previous = unsafe { ptr.cast::<*mut u16>().replace(raw) };
            if !previous.is_null() {
                // SAFETY: the previous live String field owned this BSTR once.
                drop(unsafe { BStr::from_raw_bstr(previous) });
            }
        }
        VbaRecordFieldKind::FixedString { len } => {
            if value.vtype() == crate::VarType::Null {
                return Err("Invalid use of Null".to_string());
            }
            let Some(text) = value.try_as_bstr()? else {
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
            let Some(source) = value.vba_record_ref() else {
                return Err("nested record field requires VBA record value".to_string());
            };
            if source.layout().as_ref() != layout.as_ref() {
                return Err("nested record field layout mismatch".to_string());
            }
            // SAFETY: source is a live same-layout record and `ptr` is the live
            // destination. The guard finishes all fallible cloning before commit.
            let replacement = unsafe { OwnedFieldBuffer::clone_from_raw(source.data_ptr(), kind)? };
            // SAFETY: `ptr` is the distinct live nested-record destination.
            unsafe { replacement.commit(ptr) };
        }
        VbaRecordFieldKind::FixedArray { element, bounds } => {
            let len = fixed_array_total_len(bounds)?;
            if value.vtype() != crate::VarType::ArrayVariant {
                return Err("fixed-array record field assignment requires an array value".into());
            }
            owning_boundary("fixed-array-source-clone")?;
            let Some(array) = value.try_as_safearray()? else {
                return Err(
                    "fixed-array record field assignment requires an allocated array".into(),
                );
            };
            owning_boundary("fixed-array-elements-clone")?;
            let values = array.try_variant_elements()?.ok_or_else(|| {
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
            let mut replacement = OwnedFieldBuffer::new_default(kind)?;
            let replacement_ptr = replacement.as_mut_ptr();
            for (i, value) in values.iter().enumerate() {
                // SAFETY: `i < len` and `stride` selects the live default `i`-th
                // element in the replacement guard. Each element write is itself
                // transactional; failure leaves the guard fully droppable.
                unsafe { write_field_variant_at(replacement_ptr.add(i * stride), element, value)? };
            }
            // SAFETY: `ptr` is the live fixed-array field; all replacement elements
            // are complete, so one swap commits the whole field.
            unsafe { replacement.commit(ptr) };
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
    for field in layout.fields() {
        // SAFETY: source is live per the caller contract; destination is a distinct
        // complete all-zero default. Each field transaction leaves `record` valid
        // if a later boundary returns or unwinds.
        unsafe {
            clone_field_into_live(
                src.add(field.offset),
                record.data_mut_ptr().add(field.offset),
                &field.kind,
            )?
        };
    }
    Ok(record)
}

fn clone_bstr_raw(raw: *mut u16) -> Result<*mut u16, String> {
    if raw.is_null() {
        return Ok(ptr::null_mut());
    }
    owning_boundary("bstr-clone")?;
    // SAFETY: `raw` is the live BSTR owned by the record slot for this call;
    // BorrowedBStr has no destructor and cannot consume it on error or unwind.
    unsafe { borrow_raw_bstr(raw) }.clone_raw_bstr()
}

fn validate_record_shape(fields: &[VbaRecordFieldSpec]) -> Result<(usize, usize, usize), String> {
    if fields.is_empty() {
        return Err("VBA record layout must contain at least one field".to_string());
    }
    if fields.len() > MAX_VBA_RECORD_SIZE {
        return Err(format!(
            "VBA record layout has {} fields; the sealed field table limit is {}",
            fields.len(),
            MAX_VBA_RECORD_SIZE
        ));
    }
    let graph_depth = validate_layout_graph(fields)?;

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
    Ok((size, record_align, graph_depth))
}

fn validate_layout_graph(fields: &[VbaRecordFieldSpec]) -> Result<usize, String> {
    let mut ancestors = [ptr::null(); MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH];
    let mut visits = 0usize;
    validate_layout_fields_graph(
        fields,
        MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH,
        &mut ancestors,
        0,
        &mut visits,
    )
}

fn validate_layout_fields_graph(
    fields: &[VbaRecordFieldSpec],
    remaining_depth: usize,
    ancestors: &mut [*const VbaRecordLayout; MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH],
    ancestor_len: usize,
    visits: &mut usize,
) -> Result<usize, String> {
    let mut graph_depth = 0usize;
    for field in fields {
        graph_depth = graph_depth.max(validate_field_kind_graph(
            &field.kind,
            remaining_depth,
            ancestors,
            ancestor_len,
            visits,
        )?);
    }
    Ok(graph_depth)
}

fn validate_sealed_layout_fields_graph(
    fields: &[VbaRecordFieldLayout],
    remaining_depth: usize,
    ancestors: &mut [*const VbaRecordLayout; MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH],
    ancestor_len: usize,
    visits: &mut usize,
) -> Result<usize, String> {
    let mut graph_depth = 0usize;
    for field in fields {
        graph_depth = graph_depth.max(validate_field_kind_graph(
            &field.kind,
            remaining_depth,
            ancestors,
            ancestor_len,
            visits,
        )?);
    }
    Ok(graph_depth)
}

fn validate_field_kind_graph(
    kind: &VbaRecordFieldKind,
    remaining_depth: usize,
    ancestors: &mut [*const VbaRecordLayout; MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH],
    ancestor_len: usize,
    visits: &mut usize,
) -> Result<usize, String> {
    *visits = visits
        .checked_add(1)
        .ok_or_else(|| "VBA record layout graph visit count overflow".to_string())?;
    if *visits > MAX_VBA_RECORD_LAYOUT_GRAPH_VISITS {
        return Err(format!(
            "VBA record layout graph exceeds the OxVba validation budget of {MAX_VBA_RECORD_LAYOUT_GRAPH_VISITS} nodes"
        ));
    }
    if remaining_depth == 0 {
        return Err(format!(
            "VBA record layout graph depth exceeds the OxVba safety limit of {MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH}"
        ));
    }

    match kind {
        VbaRecordFieldKind::Record(layout) => {
            let layout_ptr = Arc::as_ptr(layout);
            if ancestors[..ancestor_len].contains(&layout_ptr) {
                return Err("VBA record layout graph contains a recursive record cycle".to_string());
            }
            if ancestor_len >= ancestors.len() {
                return Err(format!(
                    "VBA record layout graph depth exceeds the OxVba safety limit of {MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH}"
                ));
            }
            ancestors[ancestor_len] = layout_ptr;
            let child_depth = validate_sealed_layout_fields_graph(
                layout.fields(),
                remaining_depth - 1,
                ancestors,
                ancestor_len + 1,
                visits,
            )?;
            Ok(1 + child_depth)
        }
        VbaRecordFieldKind::FixedArray { element, .. } => {
            reject_nested_fixed_array_element(element)?;
            Ok(1 + validate_field_kind_graph(
                element,
                remaining_depth - 1,
                ancestors,
                ancestor_len,
                visits,
            )?)
        }
        _ => Ok(1),
    }
}

fn reject_nested_fixed_array_element(element: &VbaRecordFieldKind) -> Result<(), String> {
    if matches!(element, VbaRecordFieldKind::FixedArray { .. }) {
        return Err("VBA record fixed-array element cannot itself be a fixed array".to_string());
    }
    Ok(())
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
        FIELD_POINTER_PROJECTIONS, LAYOUT_FIELD_TABLE_RESERVATIONS,
        MAX_VBA_RECORD_FIXED_ARRAY_RANK, MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH, MAX_VBA_RECORD_SIZE,
        OwningFailureMode, RECORD_BUFFER_EVENTS, VbaRecord, VbaRecordFieldHandle,
        VbaRecordFieldKind as Kind, VbaRecordFieldSpec as Field, VbaRecordLayout, checked_align_to,
        clear_owning_boundary_failure, inject_owning_boundary_failure, take_owning_boundary_trace,
        validate_field_kind_graph,
    };
    use crate::live_counters::thread_live_handle_counts;
    use crate::safe_array::{SafeArray, SafeArrayBound};
    use crate::{ObjectRef, Variant, bstr::BStr};
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::Arc,
    };

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

    fn layout_field_table_reservations() -> usize {
        LAYOUT_FIELD_TABLE_RESERVATIONS.with(core::cell::Cell::get)
    }

    fn record_bytes(record: &VbaRecord) -> Vec<u8> {
        // SAFETY: `data_ptr` exposes this record's live payload and `memory_len`
        // is its exact initialized layout extent.
        unsafe { core::slice::from_raw_parts(record.data_ptr(), record.memory_len()) }.to_vec()
    }

    fn owned_inner_layout() -> Arc<VbaRecordLayout> {
        Arc::new(
            VbaRecordLayout::new(vec![
                Field::named("Text", Kind::String),
                Field::named("Value", Kind::Variant),
            ])
            .expect("owning inner layout"),
        )
    }

    fn owned_inner_record(layout: &Arc<VbaRecordLayout>, text: &str, identity: i32) -> VbaRecord {
        let mut record = VbaRecord::new_default(Arc::clone(layout)).expect("inner record");
        record
            .write_field_variant(0, &Variant::from_string(text))
            .expect("inner String write");
        record
            .write_field_variant(
                1,
                &Variant::from_object_ref(ObjectRef::from_compat_identity(identity)),
            )
            .expect("inner Variant write");
        record
    }

    fn assert_owned_inner(record: &VbaRecord, text: &str, identity: i32) {
        assert_eq!(
            record
                .read_field_variant(0)
                .expect("inner String read")
                .as_bstr()
                .expect("inner String value")
                .as_str(),
            text
        );
        assert_eq!(
            record
                .read_field_variant(1)
                .expect("inner Variant read")
                .as_object_ref()
                .expect("inner Object value")
                .compat_identity(),
            identity
        );
    }

    fn assert_record_variant(value: &Variant, text: &str, identity: i32) {
        assert_owned_inner(
            value.vba_record_ref().expect("VBA record Variant"),
            text,
            identity,
        );
    }

    fn assert_injected_field_write_sweep(
        label: &str,
        make_target: impl Fn() -> VbaRecord,
        field: usize,
        replacement: &Variant,
        assert_old: impl Fn(&VbaRecord),
        assert_new: impl Fn(&VbaRecord),
    ) {
        let mut successful = make_target();
        take_owning_boundary_trace();
        successful
            .write_field_variant(field, replacement)
            .expect("uninjected field replacement");
        let success_trace = take_owning_boundary_trace();
        assert!(
            !success_trace.is_empty(),
            "transactional {label} write must expose at least one failure boundary"
        );
        assert_new(&successful);
        drop(successful);

        for mode in [OwningFailureMode::Error, OwningFailureMode::Panic] {
            for nth in 0..success_trace.len() {
                let mut target = make_target();
                let before = record_bytes(&target);
                inject_owning_boundary_failure(nth, mode);
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    target.write_field_variant(field, replacement)
                }));
                clear_owning_boundary_failure();
                let failure_trace = take_owning_boundary_trace();
                assert_eq!(
                    failure_trace.as_slice(),
                    &success_trace[..=nth],
                    "injected {mode:?} failure did not reach the expected {label} boundary {nth}"
                );
                match (mode, outcome) {
                    (OwningFailureMode::Error, Ok(Err(error))) => assert!(
                        error.contains("injected owning clone/allocation failure"),
                        "unexpected injected error: {error}"
                    ),
                    (OwningFailureMode::Error, Err(_)) => {
                        panic!("injected {label} error unexpectedly unwound")
                    }
                    (OwningFailureMode::Panic, Err(_)) => {}
                    (OwningFailureMode::Error, Ok(Ok(()))) => {
                        panic!("injected owning error unexpectedly committed")
                    }
                    (OwningFailureMode::Panic, Ok(result)) => {
                        panic!("injected owning panic did not unwind: {result:?}")
                    }
                }
                assert_eq!(
                    record_bytes(&target),
                    before,
                    "{label} destination bytes changed after injected {mode:?} boundary {nth}"
                );
                assert_old(&target);
            }
        }
        take_owning_boundary_trace();
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
    fn vba_record_layout_sealing_reuses_same_arc_handle_across_record_lifetimes() {
        let before_buffers = record_buffer_events();
        let layout = Arc::new(
            VbaRecordLayout::new(vec![Field::named("Value", Kind::Long)]).expect("layout"),
        );
        let mut record_a = VbaRecord::new_default(Arc::clone(&layout)).expect("record A");
        let mut record_b = VbaRecord::new_default(layout).expect("record B");
        let handle = record_a.field_handle(0).expect("shared-layout handle");

        record_a
            .write_field_variant(0, &Variant::from_i32(11))
            .expect("write A");
        record_b
            .write_field_variant(0, &Variant::from_i32(22))
            .expect("write B");
        let ptr_a = record_a.field_ptr(&handle).expect("A pointer");
        let ptr_b = record_b.field_ptr(&handle).expect("B pointer");
        assert_ne!(ptr_a, ptr_b);
        assert_eq!(
            record_a.read_field_variant(0).expect("read A").as_i32(),
            Some(11)
        );
        assert_eq!(
            record_b.read_field_variant(0).expect("read B").as_i32(),
            Some(22)
        );

        drop(record_a);
        assert_eq!(
            record_b.field_ptr(&handle).expect("B pointer after A drop"),
            ptr_b
        );
        // SAFETY: `handle` is still bound to record B's exact layout Arc and the
        // checked projection proves this is its aligned Long field.
        unsafe {
            record_b
                .field_mut_ptr(&handle)
                .expect("B mutable pointer after A drop")
                .cast::<i32>()
                .write(33);
        }
        assert_eq!(
            record_b
                .read_field_variant(0)
                .expect("read B after A drop")
                .as_i32(),
            Some(33)
        );

        drop(record_b);
        drop(handle);
        let after_buffers = record_buffer_events();
        assert_eq!(after_buffers.0 - before_buffers.0, 2);
        assert_eq!(after_buffers.1 - before_buffers.1, 2);
    }

    #[test]
    fn vba_record_layout_sealing_rejects_hostile_shape_inputs_before_record_allocation() {
        let before_buffers = record_buffer_events();
        let before_projection = pointer_projection_count();
        let before_field_tables = layout_field_table_reservations();

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
        assert_eq!(
            VbaRecordLayout::new(Vec::new()).expect_err("empty layout"),
            "VBA record layout must contain at least one field"
        );

        let nested_array = Kind::FixedArray {
            element: Box::new(Kind::FixedArray {
                element: Box::new(Kind::Byte),
                bounds: bounds(0, 2),
            }),
            bounds: bounds(0, 2),
        };
        assert_eq!(
            VbaRecordLayout::new(vec![Field::named("Nested", nested_array)])
                .expect_err("array-of-array shape"),
            "VBA record fixed-array element cannot itself be a fixed array"
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
        assert_eq!(layout_field_table_reservations(), before_field_tables);
    }

    #[test]
    fn vba_record_layout_sealing_enforces_bounded_graph_depth_before_allocation() {
        let cycle_layout = Arc::new(
            VbaRecordLayout::new(vec![Field::named("CycleProbe", Kind::Byte)])
                .expect("cycle probe layout"),
        );
        let mut simulated_ancestors = [core::ptr::null(); MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH];
        simulated_ancestors[0] = Arc::as_ptr(&cycle_layout);
        let mut simulated_visits = 0usize;
        assert_eq!(
            validate_field_kind_graph(
                &Kind::Record(cycle_layout),
                MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH,
                &mut simulated_ancestors,
                1,
                &mut simulated_visits,
            )
            .expect_err("simulated recursive layout graph"),
            "VBA record layout graph contains a recursive record cycle"
        );

        let mut boundary = Arc::new(
            VbaRecordLayout::new(vec![Field::named("Leaf", Kind::Byte)]).expect("depth-one layout"),
        );
        for level in 2..=MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH {
            boundary = Arc::new(
                VbaRecordLayout::new(vec![Field::named(
                    format!("Level{level}"),
                    Kind::Record(boundary),
                )])
                .expect("layout at safety boundary"),
            );
        }
        assert_eq!(boundary.graph_depth(), MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH);

        let before_field_tables = layout_field_table_reservations();
        let before_buffers = record_buffer_events();
        let before_projection = pointer_projection_count();
        assert_eq!(
            VbaRecordLayout::new(vec![Field::named(
                "TooDeep",
                Kind::Record(Arc::clone(&boundary)),
            )])
            .expect_err("depth boundary plus one"),
            format!(
                "VBA record layout graph depth exceeds the OxVba safety limit of {MAX_VBA_RECORD_LAYOUT_GRAPH_DEPTH}"
            )
        );
        assert_eq!(layout_field_table_reservations(), before_field_tables);
        assert_eq!(record_buffer_events(), before_buffers);
        assert_eq!(pointer_projection_count(), before_projection);

        let record = VbaRecord::new_default(boundary).expect("boundary record");
        let handle = record.field_handle(0).expect("boundary field handle");
        assert!(record.field_ptr(&handle).is_ok());
        drop(record);
        drop(handle);
        let after_buffers = record_buffer_events();
        assert_eq!(after_buffers.0 - before_buffers.0, 1);
        assert_eq!(after_buffers.1 - before_buffers.1, 1);
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
    fn vba_record_transactional_write_preserves_every_owning_destination_until_commit() {
        let before_buffers = record_buffer_events();
        {
            let variant_layout = Arc::new(
                VbaRecordLayout::new(vec![Field::named("Value", Kind::Variant)])
                    .expect("Variant field layout"),
            );
            let variant_replacement = Variant::from_safearray(SafeArray::from_variants(vec![
                Variant::from_string("new payload"),
                Variant::from_object_ref(ObjectRef::from_compat_identity(22)),
            ]));
            assert_injected_field_write_sweep(
                "Variant",
                || {
                    let mut target =
                        VbaRecord::new_default(Arc::clone(&variant_layout)).expect("target");
                    target
                        .write_field_variant(
                            0,
                            &Variant::from_object_ref(ObjectRef::from_compat_identity(11)),
                        )
                        .expect("old Variant");
                    target
                },
                0,
                &variant_replacement,
                |target| {
                    assert_eq!(
                        target
                            .read_field_variant(0)
                            .expect("old Variant read")
                            .as_object_ref()
                            .expect("old Object")
                            .compat_identity(),
                        11
                    );
                },
                |target| {
                    let value = target.read_field_variant(0).expect("new Variant read");
                    let elements = value
                        .as_safearray()
                        .expect("new SAFEARRAY")
                        .variant_elements()
                        .expect("new SAFEARRAY values");
                    assert_eq!(
                        elements[0].as_bstr().expect("new BSTR element").as_str(),
                        "new payload"
                    );
                    assert_eq!(
                        elements[1]
                            .as_object_ref()
                            .expect("new Object element")
                            .compat_identity(),
                        22
                    );
                },
            );

            let string_layout = Arc::new(
                VbaRecordLayout::new(vec![Field::named("Text", Kind::String)])
                    .expect("String field layout"),
            );
            let string_replacement = Variant::from_string("new text");
            assert_injected_field_write_sweep(
                "String",
                || {
                    let mut target =
                        VbaRecord::new_default(Arc::clone(&string_layout)).expect("target");
                    target
                        .write_field_variant(0, &Variant::from_string("old text"))
                        .expect("old String");
                    target
                },
                0,
                &string_replacement,
                |target| {
                    assert_eq!(
                        target
                            .read_field_variant(0)
                            .expect("old String read")
                            .as_bstr()
                            .expect("old String")
                            .as_str(),
                        "old text"
                    );
                },
                |target| {
                    assert_eq!(
                        target
                            .read_field_variant(0)
                            .expect("new String read")
                            .as_bstr()
                            .expect("new String")
                            .as_str(),
                        "new text"
                    );
                },
            );

            let inner_layout = owned_inner_layout();
            let nested_layout = Arc::new(
                VbaRecordLayout::new(vec![Field::named(
                    "Nested",
                    Kind::Record(Arc::clone(&inner_layout)),
                )])
                .expect("nested-record layout"),
            );
            let nested_replacement =
                Variant::from_vba_record(owned_inner_record(&inner_layout, "new nested", 32));
            assert_injected_field_write_sweep(
                "nested Record",
                || {
                    let mut target =
                        VbaRecord::new_default(Arc::clone(&nested_layout)).expect("target");
                    target
                        .write_field_variant(
                            0,
                            &Variant::from_vba_record(owned_inner_record(
                                &inner_layout,
                                "old nested",
                                31,
                            )),
                        )
                        .expect("old nested record");
                    target
                },
                0,
                &nested_replacement,
                |target| {
                    assert_record_variant(
                        &target.read_field_variant(0).expect("old nested read"),
                        "old nested",
                        31,
                    );
                },
                |target| {
                    assert_record_variant(
                        &target.read_field_variant(0).expect("new nested read"),
                        "new nested",
                        32,
                    );
                },
            );

            let fixed_layout = Arc::new(
                VbaRecordLayout::new(vec![Field::named(
                    "Items",
                    Kind::FixedArray {
                        element: Box::new(Kind::Record(Arc::clone(&inner_layout))),
                        bounds: bounds(-1, 2),
                    },
                )])
                .expect("fixed nested-record array layout"),
            );
            let fixed_replacement = Variant::from_safearray(SafeArray::from_variants(vec![
                Variant::from_vba_record(owned_inner_record(&inner_layout, "new zero", 42)),
                Variant::from_vba_record(owned_inner_record(&inner_layout, "new one", 43)),
            ]));
            assert_injected_field_write_sweep(
                "fixed array of Record",
                || {
                    let mut target =
                        VbaRecord::new_default(Arc::clone(&fixed_layout)).expect("target");
                    target
                        .write_array_field_element(
                            0,
                            0,
                            &Variant::from_vba_record(owned_inner_record(
                                &inner_layout,
                                "old zero",
                                40,
                            )),
                        )
                        .expect("old fixed element zero");
                    target
                        .write_array_field_element(
                            0,
                            1,
                            &Variant::from_vba_record(owned_inner_record(
                                &inner_layout,
                                "old one",
                                41,
                            )),
                        )
                        .expect("old fixed element one");
                    target
                },
                0,
                &fixed_replacement,
                |target| {
                    assert_record_variant(
                        &target
                            .read_array_field_element(0, 0)
                            .expect("old fixed read")
                            .expect("old fixed zero"),
                        "old zero",
                        40,
                    );
                    assert_record_variant(
                        &target
                            .read_array_field_element(0, 1)
                            .expect("old fixed read")
                            .expect("old fixed one"),
                        "old one",
                        41,
                    );
                },
                |target| {
                    assert_record_variant(
                        &target
                            .read_array_field_element(0, 0)
                            .expect("new fixed read")
                            .expect("new fixed zero"),
                        "new zero",
                        42,
                    );
                    assert_record_variant(
                        &target
                            .read_array_field_element(0, 1)
                            .expect("new fixed read")
                            .expect("new fixed one"),
                        "new one",
                        43,
                    );
                },
            );

            let raw_layout = Arc::new(
                VbaRecordLayout::new(vec![
                    Field::named("Text", Kind::String),
                    Field::named("Value", Kind::Variant),
                    Field::named("Nested", Kind::Record(Arc::clone(&inner_layout))),
                    Field::named(
                        "Items",
                        Kind::FixedArray {
                            element: Box::new(Kind::Record(Arc::clone(&inner_layout))),
                            bounds: bounds(0, 2),
                        },
                    ),
                ])
                .expect("raw clone layout"),
            );
            let mut source = VbaRecord::new_default(Arc::clone(&raw_layout)).expect("raw source");
            source
                .write_field_variant(0, &Variant::from_string("raw text"))
                .expect("raw String");
            source
                .write_field_variant(
                    1,
                    &Variant::from_safearray(SafeArray::from_variants(vec![
                        Variant::from_string("raw payload"),
                        Variant::from_object_ref(ObjectRef::from_compat_identity(51)),
                    ])),
                )
                .expect("raw Variant");
            source
                .write_field_variant(
                    2,
                    &Variant::from_vba_record(owned_inner_record(&inner_layout, "raw nested", 52)),
                )
                .expect("raw nested");
            source
                .write_field_variant(
                    3,
                    &Variant::from_safearray(SafeArray::from_variants(vec![
                        Variant::from_vba_record(owned_inner_record(&inner_layout, "raw zero", 53)),
                        Variant::from_vba_record(owned_inner_record(&inner_layout, "raw one", 54)),
                    ])),
                )
                .expect("raw fixed array");

            let words = raw_layout.size().div_ceil(core::mem::size_of::<u64>());
            let sentinel = 0xA5A5_A5A5_A5A5_A5A5u64;
            let mut successful_raw = vec![sentinel; words];
            take_owning_boundary_trace();
            // SAFETY: `successful_raw` is aligned writable storage covering the
            // complete record layout and is uninitialized from the record's view.
            unsafe { source.clone_into_raw(successful_raw.as_mut_ptr().cast()) }
                .expect("uninjected raw clone");
            let raw_success_trace = take_owning_boundary_trace();
            assert!(!raw_success_trace.is_empty());
            // SAFETY: the preceding clone fully initialized this same-layout raw slot.
            let cloned = unsafe {
                VbaRecord::clone_from_raw(successful_raw.as_ptr().cast(), Arc::clone(&raw_layout))
            }
            .expect("materialize successful raw clone");
            assert_eq!(
                cloned
                    .read_field_variant(0)
                    .expect("raw String read")
                    .as_bstr()
                    .expect("raw String")
                    .as_str(),
                "raw text"
            );
            assert_record_variant(
                &cloned.read_field_variant(2).expect("raw nested read"),
                "raw nested",
                52,
            );
            assert_record_variant(
                &cloned
                    .read_array_field_element(3, 1)
                    .expect("raw fixed read")
                    .expect("raw fixed one"),
                "raw one",
                54,
            );
            drop(cloned);
            // SAFETY: successful_raw contains one live raw record and is dropped once.
            unsafe { VbaRecord::drop_raw(successful_raw.as_mut_ptr().cast(), &raw_layout) };

            for mode in [OwningFailureMode::Error, OwningFailureMode::Panic] {
                for nth in 0..raw_success_trace.len() {
                    let mut raw = vec![sentinel; words];
                    let before = raw.clone();
                    inject_owning_boundary_failure(nth, mode);
                    let outcome = catch_unwind(AssertUnwindSafe(|| {
                        // SAFETY: raw is aligned writable storage of sufficient extent.
                        unsafe { source.clone_into_raw(raw.as_mut_ptr().cast()) }
                    }));
                    clear_owning_boundary_failure();
                    let failure_trace = take_owning_boundary_trace();
                    assert_eq!(failure_trace.as_slice(), &raw_success_trace[..=nth]);
                    match (mode, outcome) {
                        (OwningFailureMode::Error, Ok(Err(error))) => assert!(
                            error.contains("injected owning clone/allocation failure"),
                            "unexpected raw-clone error: {error}"
                        ),
                        (OwningFailureMode::Error, Err(_)) => {
                            panic!("injected raw-clone error unexpectedly unwound")
                        }
                        (OwningFailureMode::Panic, Err(_)) => {}
                        (OwningFailureMode::Error, Ok(Ok(()))) => {
                            panic!("injected raw-clone error unexpectedly committed")
                        }
                        (OwningFailureMode::Panic, Ok(result)) => {
                            panic!("injected raw-clone panic did not unwind: {result:?}")
                        }
                    }
                    assert_eq!(
                        raw, before,
                        "partial raw construction escaped after {mode:?} boundary {nth}"
                    );
                }
            }
        }
        clear_owning_boundary_failure();
        take_owning_boundary_trace();
        let after_buffers = record_buffer_events();
        assert_eq!(
            after_buffers.0 - before_buffers.0,
            after_buffers.1 - before_buffers.1,
            "all successful, error, and unwind paths must balance record owners"
        );
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

    #[test]
    fn borrowed_carrier_unwind_safety_record_read_and_raw_clone_preserve_source() {
        let handles_before = thread_live_handle_counts();
        {
            let layout = Arc::new(
                VbaRecordLayout::new(vec![
                    Field::named("Text", Kind::String),
                    Field::named("Value", Kind::Variant),
                ])
                .expect("borrowed-carrier record layout"),
            );
            let source_bytes = vec![0x41, 0x00, 0x00, 0x42, 0x7f];
            let mut source = VbaRecord::new_default(Arc::clone(&layout)).expect("source record");
            source
                .write_field_variant(0, &Variant::from_bstr_bytes(&source_bytes))
                .expect("odd-byte String field");
            source
                .write_field_variant(
                    1,
                    &Variant::from_safearray(SafeArray::from_variants(vec![
                        Variant::from_string("nested text"),
                        Variant::from_object_ref(ObjectRef::from_compat_identity(521)),
                    ])),
                )
                .expect("owning Variant field");
            let source_image = record_bytes(&source);

            for mode in [OwningFailureMode::Error, OwningFailureMode::Panic] {
                inject_owning_boundary_failure(0, mode);
                let outcome = catch_unwind(AssertUnwindSafe(|| source.read_field_variant(0)));
                clear_owning_boundary_failure();
                assert_eq!(take_owning_boundary_trace(), vec!["borrowed-bstr-clone"]);
                match (mode, outcome) {
                    (OwningFailureMode::Error, Ok(Err(error))) => assert!(
                        error.contains("injected owning clone/allocation failure"),
                        "unexpected record String-read error: {error}"
                    ),
                    (OwningFailureMode::Panic, Err(_)) => {}
                    (OwningFailureMode::Error, Err(_)) => {
                        panic!("fallible record String read unexpectedly unwound")
                    }
                    (OwningFailureMode::Error, Ok(Ok(_))) => {
                        panic!("injected record String-read error unexpectedly succeeded")
                    }
                    (OwningFailureMode::Panic, Ok(result)) => {
                        panic!("injected record String-read panic did not unwind: {result:?}")
                    }
                }
                assert_eq!(record_bytes(&source), source_image);
                assert_eq!(
                    source
                        .read_field_variant(0)
                        .expect("String source remains readable")
                        .string_bytes(),
                    Some(source_bytes.clone())
                );
                take_owning_boundary_trace();

                // Allocation is followed by the record field's logical clone
                // boundary; the third boundary is inside the destructor-free view.
                inject_owning_boundary_failure(2, mode);
                let outcome = catch_unwind(AssertUnwindSafe(|| source.try_clone()));
                clear_owning_boundary_failure();
                assert_eq!(
                    take_owning_boundary_trace(),
                    vec![
                        "record-buffer-allocation",
                        "bstr-clone",
                        "borrowed-bstr-clone"
                    ]
                );
                match (mode, outcome) {
                    (OwningFailureMode::Error, Ok(Err(error))) => assert!(
                        error.contains("injected owning clone/allocation failure"),
                        "unexpected record clone error: {error}"
                    ),
                    (OwningFailureMode::Panic, Err(_)) => {}
                    (OwningFailureMode::Error, Err(_)) => {
                        panic!("fallible record clone unexpectedly unwound")
                    }
                    (OwningFailureMode::Error, Ok(Ok(_))) => {
                        panic!("injected record clone error unexpectedly succeeded")
                    }
                    (OwningFailureMode::Panic, Ok(result)) => {
                        panic!("injected record clone panic did not unwind: {result:?}")
                    }
                }
                assert_eq!(record_bytes(&source), source_image);

                inject_owning_boundary_failure(2, mode);
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    // SAFETY: `source` is live and `layout` is its exact sealed layout.
                    unsafe { VbaRecord::clone_from_raw(source.data_ptr(), Arc::clone(&layout)) }
                }));
                clear_owning_boundary_failure();
                assert_eq!(
                    take_owning_boundary_trace(),
                    vec![
                        "record-buffer-allocation",
                        "bstr-clone",
                        "borrowed-bstr-clone"
                    ]
                );
                match (mode, outcome) {
                    (OwningFailureMode::Error, Ok(Err(error))) => assert!(
                        error.contains("injected owning clone/allocation failure"),
                        "unexpected raw record clone error: {error}"
                    ),
                    (OwningFailureMode::Panic, Err(_)) => {}
                    (OwningFailureMode::Error, Err(_)) => {
                        panic!("fallible raw record clone unexpectedly unwound")
                    }
                    (OwningFailureMode::Error, Ok(Ok(_))) => {
                        panic!("injected raw record clone error unexpectedly succeeded")
                    }
                    (OwningFailureMode::Panic, Ok(result)) => {
                        panic!("injected raw record clone panic did not unwind: {result:?}")
                    }
                }
                assert_eq!(record_bytes(&source), source_image);
                assert_eq!(
                    source
                        .read_field_variant(1)
                        .expect("Variant source remains readable")
                        .safearray_element(1)
                        .expect("SAFEARRAY carrier")
                        .expect("Object element remains readable")
                        .as_object_ref()
                        .expect("Object element")
                        .compat_identity(),
                    521
                );
                take_owning_boundary_trace();
            }
        }
        clear_owning_boundary_failure();
        take_owning_boundary_trace();
        assert_eq!(
            thread_live_handle_counts(),
            handles_before,
            "record read/raw-clone failures and final source drop must balance all handles"
        );
    }
}
