use crate::{RuntimeValue, Variant};
use core::ptr::NonNull;

pub const ARRAY_TAG_BASE: i32 = -1_000_000_000;
pub const ARRAY_TAG_LIMIT: i32 = ARRAY_TAG_BASE + 1_000_000;
const DISPATCH_ARRAY_PAYLOAD_BASE: i32 = 20_000;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafeArrayBound {
    pub count: u32,
    pub lower: i32,
}

#[repr(C)]
struct RawSafeArray {
    c_dims: u16,
    f_features: u16,
    cb_elements: u32,
    c_locks: u32,
    pv_data: *mut core::ffi::c_void,
    rgsabound: [SafeArrayBound; 1],
}

#[repr(transparent)]
pub struct SafeArray(NonNull<RawSafeArray>);

unsafe impl Send for SafeArray {}
unsafe impl Sync for SafeArray {}

fn bounds_layout(dimensions: usize) -> Result<std::alloc::Layout, String> {
    if dimensions == 0 {
        return Err("SAFEARRAY must have at least one dimension".to_string());
    }
    let header = std::alloc::Layout::new::<RawSafeArray>();
    let extra = dimensions
        .checked_sub(1)
        .ok_or_else(|| "SAFEARRAY dimension underflow".to_string())?;
    let extra_bounds = std::alloc::Layout::array::<SafeArrayBound>(extra)
        .map_err(|_| "SAFEARRAY bounds layout overflow".to_string())?;
    header
        .extend(extra_bounds)
        .map(|(layout, _)| layout.pad_to_align())
        .map_err(|_| "SAFEARRAY header layout overflow".to_string())
}

fn default_bounds_for_len(len: usize) -> Result<Vec<SafeArrayBound>, String> {
    Ok(vec![SafeArrayBound {
        count: u32::try_from(len).map_err(|_| {
            format!("SAFEARRAY length {len} exceeds supported u32 element capacity")
        })?,
        lower: 0,
    }])
}

fn bounds_total_len(bounds: &[SafeArrayBound]) -> Result<usize, String> {
    let mut total = 1usize;
    for bound in bounds {
        total = total
            .checked_mul(bound.count as usize)
            .ok_or_else(|| "SAFEARRAY total element count overflowed".to_string())?;
    }
    Ok(total)
}

fn alloc_header(bounds: &[SafeArrayBound], pv_data: *mut core::ffi::c_void) -> Result<NonNull<RawSafeArray>, String> {
    let layout = bounds_layout(bounds.len())?;
    let raw = unsafe { std::alloc::alloc_zeroed(layout) }.cast::<RawSafeArray>();
    let Some(raw) = NonNull::new(raw) else {
        return Err("failed to allocate SAFEARRAY header".to_string());
    };
    unsafe {
        let header = raw.as_ptr();
        (*header).c_dims = u16::try_from(bounds.len())
            .map_err(|_| "SAFEARRAY dimension count exceeds u16 capacity".to_string())?;
        (*header).f_features = 0;
        (*header).cb_elements = u32::try_from(core::mem::size_of::<Variant>())
            .expect("canonical Variant size should fit u32");
        (*header).c_locks = 0;
        (*header).pv_data = pv_data;
        let dst = core::ptr::addr_of_mut!((*header).rgsabound).cast::<SafeArrayBound>();
        core::ptr::copy_nonoverlapping(bounds.as_ptr(), dst, bounds.len());
    }
    Ok(raw)
}

fn variants_to_boxed_slice(values: Vec<RuntimeValue>) -> Result<Box<[Variant]>, String> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        out.push(Variant::try_from_runtime_value(&value)?);
    }
    Ok(out.into_boxed_slice())
}

impl SafeArray {
    fn from_bounds_and_runtime_values(
        bounds: Vec<SafeArrayBound>,
        values: Option<Vec<RuntimeValue>>,
    ) -> Result<Self, String> {
        let expected_len = bounds_total_len(&bounds)?;
        let pv_data = match values {
            Some(values) => {
                if values.len() != expected_len {
                    return Err(format!(
                        "SAFEARRAY payload length {} does not match shape length {}",
                        values.len(),
                        expected_len
                    ));
                }
                let boxed = variants_to_boxed_slice(values)?;
                if boxed.is_empty() {
                    core::ptr::null_mut()
                } else {
                    Box::into_raw(boxed).cast::<Variant>().cast()
                }
            }
            None => core::ptr::null_mut(),
        };
        let header = alloc_header(&bounds, pv_data)?;
        Ok(Self(header))
    }

    pub fn vector(len: usize) -> Self {
        Self::from_bounds_and_runtime_values(
            default_bounds_for_len(len).expect("vector bounds should fit SAFEARRAY capacity"),
            None,
        )
        .expect("shape-only SAFEARRAY allocation should succeed")
    }

    pub fn from_values(values: Vec<RuntimeValue>) -> Self {
        let len = values.len();
        Self::from_bounds_and_runtime_values(
            default_bounds_for_len(len).expect("value bounds should fit SAFEARRAY capacity"),
            Some(values),
        )
        .expect("SAFEARRAY payload allocation should succeed for supported canonical values")
    }

    pub fn from_values_nd(bounds: Vec<SafeArrayBound>, values: Vec<RuntimeValue>) -> Self {
        Self::from_bounds_and_runtime_values(bounds, Some(values))
            .expect("SAFEARRAY nd payload allocation should succeed for supported canonical values")
    }

    pub fn from_shape(bounds: Vec<SafeArrayBound>) -> Result<Self, String> {
        Self::from_bounds_and_runtime_values(bounds, None)
    }

    pub fn from_shape_and_values(
        bounds: Vec<SafeArrayBound>,
        values: Vec<RuntimeValue>,
    ) -> Result<Self, String> {
        Self::from_bounds_and_runtime_values(bounds, Some(values))
    }

    pub fn dimensions(&self) -> u8 {
        unsafe { (*self.0.as_ptr()).c_dims as u8 }
    }

    fn raw_bounds(&self) -> Vec<SafeArrayBound> {
        let dims = self.dimensions() as usize;
        if dims == 0 {
            return Vec::new();
        }
        let ptr = unsafe { core::ptr::addr_of!((*self.0.as_ptr()).rgsabound).cast::<SafeArrayBound>() };
        unsafe { core::slice::from_raw_parts(ptr, dims) }.to_vec()
    }

    pub fn len(&self) -> usize {
        bounds_total_len(&self.raw_bounds()).unwrap_or(0)
    }

    pub fn effective_len(&self) -> usize {
        self.len()
    }

    pub fn bounds(&self) -> Option<Vec<SafeArrayBound>> {
        let bounds = self.raw_bounds();
        if bounds.is_empty() {
            return None;
        }
        let dims = self.dimensions() as usize;
        if dims == 1 && bounds[0].lower == 0 {
            None
        } else {
            Some(bounds)
        }
    }

    fn bounds_for_shape(&self) -> Vec<SafeArrayBound> {
        self.bounds()
            .unwrap_or_else(|| default_bounds_for_len(self.len()).unwrap_or_default())
    }

    fn variant_slice(&self) -> Option<&[Variant]> {
        let data = unsafe { (*self.0.as_ptr()).pv_data.cast::<Variant>() };
        if data.is_null() {
            return None;
        }
        Some(unsafe { core::slice::from_raw_parts(data, self.len()) })
    }

    pub fn elements(&self) -> Option<Vec<RuntimeValue>> {
        self.variant_slice().map(|values| {
            values
                .iter()
                .map(|value| {
                    value
                        .to_runtime_value()
                        .expect("SAFEARRAY canonical Variant payload should decode into RuntimeValue")
                })
                .collect()
        })
    }

    pub fn replace_elements(&self, values: Vec<RuntimeValue>) -> Result<Self, String> {
        Self::from_shape_and_values(self.bounds_for_shape(), values)
    }
}

impl Clone for SafeArray {
    fn clone(&self) -> Self {
        match self.elements() {
            Some(values) => Self::from_shape_and_values(self.bounds_for_shape(), values)
                .expect("cloning canonical SAFEARRAY with values should succeed"),
            None => Self::from_shape(self.bounds_for_shape())
                .expect("cloning shape-only SAFEARRAY should succeed"),
        }
    }
}

impl Drop for SafeArray {
    fn drop(&mut self) {
        let len = self.len();
        let data = unsafe { (*self.0.as_ptr()).pv_data.cast::<Variant>() };
        if !data.is_null() && len > 0 {
            let raw_slice = core::ptr::slice_from_raw_parts_mut(data, len);
            unsafe {
                drop(Box::from_raw(raw_slice));
            }
        }
        if let Ok(layout) = bounds_layout(self.dimensions() as usize) {
            unsafe { std::alloc::dealloc(self.0.as_ptr().cast::<u8>(), layout) };
        }
    }
}

impl core::fmt::Debug for SafeArray {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SafeArray")
            .field("dimensions", &self.dimensions())
            .field("len", &self.len())
            .field("bounds", &self.bounds())
            .field("elements", &self.elements())
            .finish()
    }
}

impl PartialEq for SafeArray {
    fn eq(&self, other: &Self) -> bool {
        self.dimensions() == other.dimensions()
            && self.len() == other.len()
            && self.bounds() == other.bounds()
            && self.elements() == other.elements()
    }
}

impl Eq for SafeArray {}

pub fn is_array_tag(value: i32) -> bool {
    (ARRAY_TAG_BASE..=ARRAY_TAG_LIMIT).contains(&value)
}

pub fn array_len_from_tag(value: i32) -> Option<usize> {
    if !is_array_tag(value) {
        return None;
    }
    let count = value.checked_sub(ARRAY_TAG_BASE)?;
    usize::try_from(count).ok()
}

pub fn safe_array_from_tag(value: i32) -> Option<SafeArray> {
    array_len_from_tag(value).map(SafeArray::vector)
}

pub fn array_tag_from_safe_array(array: &SafeArray) -> Option<i32> {
    if array.dimensions() == 0 {
        return None;
    }
    let len_i32 = i32::try_from(array.effective_len()).ok()?;
    ARRAY_TAG_BASE
        .checked_add(len_i32)
        .filter(|v| *v <= ARRAY_TAG_LIMIT)
}

pub fn marshal_dispatch_argument(value: i32) -> i32 {
    let Some(array) = safe_array_from_tag(value) else {
        return value;
    };
    match i32::try_from(array.len()) {
        Ok(len) => DISPATCH_ARRAY_PAYLOAD_BASE.saturating_add(len),
        Err(_) => DISPATCH_ARRAY_PAYLOAD_BASE,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ARRAY_TAG_BASE, array_len_from_tag, array_tag_from_safe_array, marshal_dispatch_argument,
        safe_array_from_tag, SafeArray, SafeArrayBound,
    };
    use crate::RuntimeValue;

    #[test]
    fn safe_array_tag_roundtrip_for_vector_shape() {
        let tag = ARRAY_TAG_BASE + 3;
        let array = safe_array_from_tag(tag).expect("array tag should decode");
        assert_eq!(array.len(), 3);
        assert_eq!(array.dimensions(), 1);
        assert_eq!(array_tag_from_safe_array(&array), Some(tag));
    }

    #[test]
    fn marshal_dispatch_argument_distinguishes_array_tags() {
        assert_eq!(marshal_dispatch_argument(9), 9);
        assert_eq!(marshal_dispatch_argument(ARRAY_TAG_BASE + 4), 20_004);
        assert_eq!(array_len_from_tag(ARRAY_TAG_BASE + 2), Some(2));
    }

    #[test]
    fn safe_array_from_values_preserves_owned_payload_shape() {
        let array = SafeArray::from_values(vec![RuntimeValue::I32(4), RuntimeValue::I32(9)]);
        assert_eq!(array.dimensions(), 1);
        assert_eq!(array.len(), 2);
        assert_eq!(array.effective_len(), 2);
        assert_eq!(
            array.elements(),
            Some(vec![RuntimeValue::I32(4), RuntimeValue::I32(9)])
        );
        assert_eq!(array_tag_from_safe_array(&array), Some(ARRAY_TAG_BASE + 2));
    }

    #[test]
    fn safe_array_from_values_nd_preserves_multi_dimensional_shape() {
        let bounds = vec![
            SafeArrayBound { lower: 1, count: 3 },
            SafeArrayBound { lower: 1, count: 2 },
        ];
        let values = vec![
            RuntimeValue::I32(1),
            RuntimeValue::I32(2),
            RuntimeValue::I32(3),
            RuntimeValue::I32(4),
            RuntimeValue::I32(5),
            RuntimeValue::I32(6),
        ];
        let array = SafeArray::from_values_nd(bounds.clone(), values.clone());
        assert_eq!(array.dimensions(), 2);
        assert_eq!(array.len(), 6);
        assert_eq!(array.effective_len(), 6);
        assert_eq!(array.bounds().as_ref(), Some(&bounds));
        assert_eq!(array.elements().as_ref(), Some(&values));
    }

    #[test]
    fn safe_array_vector_has_no_bounds_metadata() {
        let array = SafeArray::vector(5);
        assert_eq!(array.dimensions(), 1);
        assert_eq!(array.bounds(), None);
        assert_eq!(array.elements(), None);
    }

    #[test]
    fn safe_array_replace_elements_preserves_shape() {
        let shape = SafeArray::from_shape(vec![
            SafeArrayBound { lower: 1, count: 2 },
            SafeArrayBound { lower: 4, count: 2 },
        ])
        .expect("shape");
        let replaced = shape
            .replace_elements(vec![
                RuntimeValue::I32(1),
                RuntimeValue::I32(2),
                RuntimeValue::I32(3),
                RuntimeValue::I32(4),
            ])
            .expect("replace");
        assert_eq!(replaced.bounds(), shape.bounds());
        assert_eq!(replaced.elements().expect("elements").len(), 4);
    }
}

#[cfg(test)]
mod proptests {
    use super::{ARRAY_TAG_BASE, ARRAY_TAG_LIMIT, array_tag_from_safe_array, safe_array_from_tag};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_safe_array_tag_roundtrip(len in 0..=1_000_000usize) {
            let tag = ARRAY_TAG_BASE + len as i32;
            prop_assert!((ARRAY_TAG_BASE..=ARRAY_TAG_LIMIT).contains(&tag));

            let array = safe_array_from_tag(tag)
                .expect("tag in valid range should decode to SafeArray");
            prop_assert_eq!(array.len(), len);
            prop_assert_eq!(array.dimensions(), 1);

            let recovered_tag = array_tag_from_safe_array(&array)
                .expect("decoded SafeArray should encode back to a tag");
            prop_assert_eq!(recovered_tag, tag);
        }
    }
}
