use crate::{
    Variant, VariantCore,
    bstr::BStr,
    safe_array::{SafeArray, SafeArrayBound},
};
use core::{ptr, slice};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VbaRecordFieldKind {
    Variant,
    Integer,
    Long,
    LongLong,
    LongPtr,
    Byte,
    Single,
    Double,
    Currency,
    Date,
    String,
    Boolean,
    Record(Arc<VbaRecordLayout>),
    FixedArray {
        element: Box<VbaRecordFieldKind>,
        len: usize,
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
    pub name: Option<String>,
    pub kind: VbaRecordFieldKind,
    pub offset: usize,
    pub size: usize,
    pub align: usize,
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

impl VbaRecordLayout {
    pub fn new(fields: Vec<VbaRecordFieldSpec>) -> Result<Self, String> {
        let mut offset = 0usize;
        let mut record_align = 1usize;
        let mut layouts = Vec::with_capacity(fields.len());

        for field in fields {
            let (size, align) = field.kind.storage_shape()?;
            offset = align_to(offset, align);
            layouts.push(VbaRecordFieldLayout {
                name: field.name,
                kind: field.kind,
                offset,
                size,
                align,
            });
            offset = offset
                .checked_add(size)
                .ok_or_else(|| "VBA record layout size overflow".to_string())?;
            record_align = record_align.max(align);
        }

        Ok(Self {
            fields: layouts,
            size: align_to(offset, record_align),
            align: record_align,
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
}

impl VbaRecordFieldKind {
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
            Self::LongPtr => pointer_shape,
            Self::Byte => (core::mem::size_of::<u8>(), core::mem::align_of::<u8>()),
            Self::Single => (core::mem::size_of::<f32>(), core::mem::align_of::<f32>()),
            Self::Double | Self::Currency | Self::Date => {
                (core::mem::size_of::<f64>(), core::mem::align_of::<f64>())
            }
            Self::String => pointer_shape,
            Self::Boolean => (core::mem::size_of::<i16>(), core::mem::align_of::<i16>()),
            Self::Record(layout) => (layout.size(), layout.align()),
            Self::FixedArray { element, len } => {
                if *len == 0 {
                    return Err(
                        "VBA fixed-array record field must have at least one element".into(),
                    );
                }
                let (element_size, element_align) = element.storage_shape()?;
                let stride = align_to(element_size, element_align);
                (
                    stride
                        .checked_mul(*len)
                        .ok_or_else(|| "VBA fixed-array record field size overflow".to_string())?,
                    element_align,
                )
            }
        };
        Ok(shape)
    }

    fn validate_byref_as_any_native_abi(&self, path: &str) -> Result<(), String> {
        match self {
            Self::Integer
            | Self::Long
            | Self::LongLong
            | Self::LongPtr
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
            Self::Variant => {
                Err("Variant fields carry nested VARIANT state, not plain native bytes".into())
            }
        }
    }
}

impl VbaRecord {
    pub fn new_default(layout: Arc<VbaRecordLayout>) -> Result<Self, String> {
        if layout.align() > core::mem::align_of::<u64>() {
            return Err(format!(
                "VBA record layout alignment {} exceeds native buffer alignment {}",
                layout.align(),
                core::mem::align_of::<u64>()
            ));
        }
        let words = layout.size().div_ceil(core::mem::size_of::<u64>());
        let mut record = Self {
            layout,
            data: vec![0; words],
        };
        let fields = record.layout.fields().to_vec();
        for field in &fields {
            // SAFETY: the buffer is sized to `layout.size()`, and each field offset
            // and recursive fixed-array stride was computed by `VbaRecordLayout`.
            unsafe {
                init_field_at(record.field_mut_ptr(field), &field.kind)?;
            }
        }
        Ok(record)
    }

    pub fn layout(&self) -> &Arc<VbaRecordLayout> {
        &self.layout
    }

    pub fn data_ptr(&self) -> *const u8 {
        self.data.as_ptr().cast()
    }

    pub fn data_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr().cast()
    }

    pub fn field_ptr(&self, field: &VbaRecordFieldLayout) -> *const u8 {
        unsafe { self.data_ptr().add(field.offset) }
    }

    pub fn field_mut_ptr(&mut self, field: &VbaRecordFieldLayout) -> *mut u8 {
        unsafe { self.data_mut_ptr().add(field.offset) }
    }

    pub fn field_bytes(&self, index: usize) -> Option<&[u8]> {
        let field = self.layout.fields().get(index)?;
        // SAFETY: the layout field is within the owned buffer by construction.
        Some(unsafe { slice::from_raw_parts(self.data_ptr().add(field.offset), field.size) })
    }

    pub fn read_field_variant(&self, index: usize) -> Result<Variant, String> {
        let field = self
            .layout
            .fields()
            .get(index)
            .ok_or_else(|| format!("record field {index} out of range"))?;
        // SAFETY: the field pointer is in range and aligned for `field.kind`.
        unsafe { read_field_variant_at(self.field_ptr(field), &field.kind) }
    }

    pub fn write_field_variant(&mut self, index: usize, value: &Variant) -> Result<(), String> {
        let field = self
            .layout
            .fields()
            .get(index)
            .cloned()
            .ok_or_else(|| format!("record field {index} out of range"))?;
        // SAFETY: the field pointer is in range and aligned for `field.kind`.
        unsafe { write_field_variant_at(self.field_mut_ptr(&field), &field.kind, value) }
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
        unsafe { clone_record_from_ptr(src, layout) }
    }

    /// Clone this record into uninitialized raw storage described by its layout.
    ///
    /// # Safety
    /// `dst` must point to writable, properly aligned, uninitialized storage of at
    /// least `self.layout().size()` bytes. The caller becomes responsible for
    /// eventually dropping the initialized raw record with [`Self::drop_raw`].
    pub unsafe fn clone_into_raw(&self, dst: *mut u8) -> Result<(), String> {
        unsafe { clone_record_into_ptr(self.data_ptr(), dst, self.layout()) }
    }

    /// Drop a raw record payload initialized according to `layout`.
    ///
    /// # Safety
    /// `ptr` must point to a live record payload initialized according to `layout`,
    /// and this function must be called at most once for that payload.
    pub unsafe fn drop_raw(ptr: *mut u8, layout: &VbaRecordLayout) {
        for field in layout.fields() {
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
        unsafe { Self::clone_from_raw(words.as_ptr().cast(), layout) }
    }
}

impl Clone for VbaRecord {
    fn clone(&self) -> Self {
        let mut clone = Self {
            layout: self.layout.clone(),
            data: vec![0; self.data.len()],
        };
        let fields = self.layout.fields().to_vec();
        for field in &fields {
            // SAFETY: source and destination are distinct buffers with the same
            // descriptor-backed layout.
            unsafe {
                clone_field_at(
                    self.field_ptr(field),
                    clone.field_mut_ptr(field),
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

unsafe fn init_field_at(ptr: *mut u8, kind: &VbaRecordFieldKind) -> Result<(), String> {
    match kind {
        VbaRecordFieldKind::Variant => unsafe { ptr.cast::<Variant>().write(Variant::empty()) },
        VbaRecordFieldKind::String => unsafe { ptr.cast::<*mut u16>().write(ptr::null_mut()) },
        VbaRecordFieldKind::Record(layout) => {
            for field in layout.fields() {
                unsafe { init_field_at(ptr.add(field.offset), &field.kind)? };
            }
        }
        VbaRecordFieldKind::FixedArray { element, len } => {
            let (element_size, element_align) = element.storage_shape()?;
            let stride = align_to(element_size, element_align);
            for i in 0..*len {
                unsafe { init_field_at(ptr.add(i * stride), element)? };
            }
        }
        _ => {}
    }
    Ok(())
}

unsafe fn clone_field_at(
    src: *const u8,
    dst: *mut u8,
    kind: &VbaRecordFieldKind,
) -> Result<(), String> {
    match kind {
        VbaRecordFieldKind::Variant => {
            let value = unsafe { &*src.cast::<Variant>() };
            unsafe { dst.cast::<Variant>().write(value.clone()) };
        }
        VbaRecordFieldKind::String => {
            let raw = unsafe { *src.cast::<*mut u16>() };
            let cloned = clone_bstr_raw(raw)?;
            unsafe { dst.cast::<*mut u16>().write(cloned) };
        }
        VbaRecordFieldKind::Record(layout) => {
            for field in layout.fields() {
                unsafe {
                    clone_field_at(src.add(field.offset), dst.add(field.offset), &field.kind)?
                };
            }
        }
        VbaRecordFieldKind::FixedArray { element, len } => {
            let (element_size, element_align) = element.storage_shape()?;
            let stride = align_to(element_size, element_align);
            for i in 0..*len {
                unsafe { clone_field_at(src.add(i * stride), dst.add(i * stride), element)? };
            }
        }
        _ => {
            let (size, _) = kind.storage_shape()?;
            unsafe { ptr::copy_nonoverlapping(src, dst, size) };
        }
    }
    Ok(())
}

unsafe fn drop_field_at(ptr: *mut u8, kind: &VbaRecordFieldKind) {
    match kind {
        VbaRecordFieldKind::Variant => unsafe { ptr::drop_in_place(ptr.cast::<Variant>()) },
        VbaRecordFieldKind::String => {
            let raw = unsafe { *ptr.cast::<*mut u16>() };
            if !raw.is_null() {
                // SAFETY: string fields own the BSTR pointer stored in the record slot.
                let _ = unsafe { BStr::from_raw_bstr(raw) };
            }
        }
        VbaRecordFieldKind::Record(layout) => {
            for field in layout.fields() {
                unsafe { drop_field_at(ptr.add(field.offset), &field.kind) };
            }
        }
        VbaRecordFieldKind::FixedArray { element, len } => {
            let Ok((element_size, element_align)) = element.storage_shape() else {
                return;
            };
            let stride = align_to(element_size, element_align);
            for i in 0..*len {
                unsafe { drop_field_at(ptr.add(i * stride), element) };
            }
        }
        _ => {}
    }
}

unsafe fn read_field_variant_at(
    ptr: *const u8,
    kind: &VbaRecordFieldKind,
) -> Result<Variant, String> {
    let value = match kind {
        VbaRecordFieldKind::Variant => unsafe { (&*ptr.cast::<Variant>()).clone() },
        VbaRecordFieldKind::Integer => Variant::from_i16(unsafe { *ptr.cast::<i16>() }),
        VbaRecordFieldKind::Long => Variant::from_i32(unsafe { *ptr.cast::<i32>() }),
        VbaRecordFieldKind::LongLong => Variant::from_i64(unsafe { *ptr.cast::<i64>() }),
        VbaRecordFieldKind::LongPtr => {
            if core::mem::size_of::<usize>() == 8 {
                Variant::from_i64(unsafe { *ptr.cast::<isize>() as i64 })
            } else {
                Variant::from_i32(unsafe { *ptr.cast::<isize>() as i32 })
            }
        }
        VbaRecordFieldKind::Byte => Variant::from_u8(unsafe { *ptr.cast::<u8>() }),
        VbaRecordFieldKind::Single => Variant::from_f32(unsafe { *ptr.cast::<f32>() }),
        VbaRecordFieldKind::Double => Variant::from_f64(unsafe { *ptr.cast::<f64>() }),
        VbaRecordFieldKind::Currency => {
            Variant::from_currency_scaled_i64(unsafe { *ptr.cast::<i64>() })
        }
        VbaRecordFieldKind::Date => Variant::from_date_f64(unsafe { *ptr.cast::<f64>() }),
        VbaRecordFieldKind::String => {
            let raw = unsafe { *ptr.cast::<*mut u16>() };
            if raw.is_null() {
                Variant::from_string(BStr::empty())
            } else {
                let text = unsafe { borrow_bstr_raw(raw) };
                let value = Variant::from_string(text.clone());
                core::mem::forget(text);
                value
            }
        }
        VbaRecordFieldKind::Boolean => Variant::from_bool(unsafe { *ptr.cast::<i16>() != 0 }),
        VbaRecordFieldKind::Record(layout) => {
            Variant::from_vba_record(unsafe { clone_record_from_ptr(ptr, layout.clone())? })
        }
        VbaRecordFieldKind::FixedArray { element, len } => {
            let (element_size, element_align) = element.storage_shape()?;
            let stride = align_to(element_size, element_align);
            let mut values = Vec::with_capacity(*len);
            for i in 0..*len {
                values.push(unsafe { read_field_variant_at(ptr.add(i * stride), element)? });
            }
            // A UDT fixed-array field is inherently fixed-size: surface
            // `FADF_FIXEDSIZE` so `Erase` of the materialized member array resets
            // its elements (and is then written back into the inline storage)
            // rather than deallocating — which the inline field cannot represent.
            Variant::from_safearray(
                SafeArray::from_variants_nd(
                    vec![SafeArrayBound {
                        lower: 0,
                        count: u32::try_from(*len).map_err(
                            |_| "fixed-array record field length exceeds SAFEARRAY capacity",
                        )?,
                    }],
                    values,
                )
                .with_fixed_size(true),
            )
        }
    };
    Ok(value)
}

unsafe fn write_field_variant_at(
    ptr: *mut u8,
    kind: &VbaRecordFieldKind,
    value: &Variant,
) -> Result<(), String> {
    match kind {
        VbaRecordFieldKind::Variant => unsafe {
            ptr.cast::<Variant>().drop_in_place();
            ptr.cast::<Variant>().write(value.clone());
        },
        VbaRecordFieldKind::Integer => {
            let value = value
                .as_i16()
                .ok_or_else(|| "Integer record field requires Integer value".to_string())?;
            unsafe { ptr.cast::<i16>().write(value) };
        }
        VbaRecordFieldKind::Long => {
            let value = value
                .as_i32()
                .ok_or_else(|| "Long record field requires Long value".to_string())?;
            unsafe { ptr.cast::<i32>().write(value) };
        }
        VbaRecordFieldKind::LongLong => {
            let value = value
                .as_i64()
                .ok_or_else(|| "LongLong record field requires LongLong value".to_string())?;
            unsafe { ptr.cast::<i64>().write(value) };
        }
        VbaRecordFieldKind::LongPtr => {
            if core::mem::size_of::<usize>() == 8 {
                let value = value
                    .as_i64()
                    .ok_or_else(|| "LongPtr record field requires LongLong value".to_string())?;
                unsafe { ptr.cast::<isize>().write(value as isize) };
            } else {
                let value = value
                    .as_i32()
                    .ok_or_else(|| "LongPtr record field requires Long value".to_string())?;
                unsafe { ptr.cast::<isize>().write(value as isize) };
            }
        }
        VbaRecordFieldKind::Byte => {
            let value = value
                .as_u8()
                .or_else(|| value.as_i32().and_then(|value| u8::try_from(value).ok()))
                .or_else(|| value.as_i16().and_then(|value| u8::try_from(value).ok()))
                .ok_or_else(|| "Byte record field requires Byte value".to_string())?;
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
            unsafe { ptr.cast::<f64>().write(value) };
        }
        VbaRecordFieldKind::Currency => {
            let value = value
                .as_currency_scaled_i64()
                .ok_or_else(|| "Currency record field requires Currency value".to_string())?;
            unsafe { ptr.cast::<i64>().write(value) };
        }
        VbaRecordFieldKind::Date => {
            let value = value
                .as_date_f64()
                .ok_or_else(|| "Date record field requires Date value".to_string())?;
            unsafe { ptr.cast::<f64>().write(value) };
        }
        VbaRecordFieldKind::String => {
            let Some(text) = value.as_bstr() else {
                return Err("String record field requires String value".to_string());
            };
            let raw = text.raw_bstr();
            core::mem::forget(text);
            unsafe { drop_field_at(ptr, kind) };
            unsafe { ptr.cast::<*mut u16>().write(raw) };
        }
        VbaRecordFieldKind::Boolean => {
            let value = value
                .as_bool()
                .ok_or_else(|| "Boolean record field requires Boolean value".to_string())?;
            unsafe { ptr.cast::<i16>().write(if value { -1 } else { 0 }) };
        }
        VbaRecordFieldKind::Record(layout) => {
            let Some(source) = value.as_vba_record() else {
                return Err("nested record field requires VBA record value".to_string());
            };
            if source.layout().as_ref() != layout.as_ref() {
                return Err("nested record field layout mismatch".to_string());
            }
            unsafe {
                drop_field_at(ptr, kind);
                clone_record_into_ptr(source.data_ptr(), ptr, layout)?;
            }
        }
        VbaRecordFieldKind::FixedArray { element, len } => {
            let Some(array) = value.as_safearray() else {
                return Err("fixed-array record field assignment requires an array value".into());
            };
            let values = array.variant_elements().ok_or_else(|| {
                "fixed-array record field assignment requires materialized array elements"
                    .to_string()
            })?;
            if values.len() != *len {
                return Err(format!(
                    "fixed-array record field assignment requires {len} elements, got {}",
                    values.len()
                ));
            }
            let (element_size, element_align) = element.storage_shape()?;
            let stride = align_to(element_size, element_align);
            for (i, value) in values.iter().enumerate() {
                unsafe { write_field_variant_at(ptr.add(i * stride), element, value)? };
            }
        }
    }
    Ok(())
}

unsafe fn clone_record_from_ptr(
    src: *const u8,
    layout: Arc<VbaRecordLayout>,
) -> Result<VbaRecord, String> {
    let mut record = VbaRecord::new_default(layout.clone())?;
    unsafe { clone_record_into_ptr(src, record.data_mut_ptr(), &layout)? };
    Ok(record)
}

unsafe fn clone_record_into_ptr(
    src: *const u8,
    dst: *mut u8,
    layout: &VbaRecordLayout,
) -> Result<(), String> {
    for field in layout.fields() {
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
    let text = unsafe { borrow_bstr_raw(raw) };
    let cloned = text.clone_raw_bstr();
    core::mem::forget(text);
    cloned
}

fn align_to(value: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

#[cfg(test)]
mod tests {
    use super::{
        VbaRecord, VbaRecordFieldKind as Kind, VbaRecordFieldSpec as Field, VbaRecordLayout,
    };
    use crate::{Variant, bstr::BStr};
    use std::sync::Arc;

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
                    len: 8,
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
                    len: 4,
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
    fn native_record_defaults_and_reads_scalar_fields_from_buffer() {
        let layout = Arc::new(
            VbaRecordLayout::new(vec![
                Field::named("Id", Kind::Long),
                Field::named(
                    "Bytes",
                    Kind::FixedArray {
                        element: Box::new(Kind::Byte),
                        len: 4,
                    },
                ),
            ])
            .expect("layout"),
        );
        let mut record = VbaRecord::new_default(layout).expect("record");
        let fields = record.layout().fields().to_vec();

        unsafe {
            record.field_mut_ptr(&fields[0]).cast::<i32>().write(1234);
            core::ptr::copy_nonoverlapping(
                [1u8, 2, 3, 4].as_ptr(),
                record.field_mut_ptr(&fields[1]),
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
                    len: 4,
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
    fn native_record_clone_deep_copies_bstr_and_variant_fields() {
        let layout = Arc::new(
            VbaRecordLayout::new(vec![
                Field::named("Text", Kind::String),
                Field::named("Value", Kind::Variant),
            ])
            .expect("layout"),
        );
        let mut record = VbaRecord::new_default(layout).expect("record");
        let fields = record.layout().fields().to_vec();
        let bstr = BStr::from("alpha");
        let raw_bstr = bstr.clone_raw_bstr().expect("clone bstr");

        unsafe {
            record
                .field_mut_ptr(&fields[0])
                .cast::<*mut u16>()
                .write(raw_bstr);
            record
                .field_mut_ptr(&fields[1])
                .cast::<Variant>()
                .drop_in_place();
            record
                .field_mut_ptr(&fields[1])
                .cast::<Variant>()
                .write(Variant::from_string("payload"));
        }

        let clone = record.clone();
        let original_raw = unsafe { *record.field_ptr(&fields[0]).cast::<*mut u16>() };
        let clone_raw = unsafe { *clone.field_ptr(&fields[0]).cast::<*mut u16>() };

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
