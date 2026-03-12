use crate::RuntimeValue;

pub const ARRAY_TAG_BASE: i32 = -1_000_000_000;
pub const ARRAY_TAG_LIMIT: i32 = ARRAY_TAG_BASE + 1_000_000;
const DISPATCH_ARRAY_PAYLOAD_BASE: i32 = 20_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeArray {
    pub dimensions: u8,
    pub len: usize,
    pub elements: Option<Vec<RuntimeValue>>,
}

impl SafeArray {
    pub fn vector(len: usize) -> Self {
        Self {
            dimensions: 1,
            len,
            elements: None,
        }
    }

    pub fn from_values(values: Vec<RuntimeValue>) -> Self {
        Self {
            dimensions: 1,
            len: values.len(),
            elements: Some(values),
        }
    }

    pub fn effective_len(&self) -> usize {
        self.elements
            .as_ref()
            .map(|values| values.len())
            .unwrap_or(self.len)
    }
}

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
    if array.dimensions == 0 {
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
    match i32::try_from(array.len) {
        Ok(len) => DISPATCH_ARRAY_PAYLOAD_BASE.saturating_add(len),
        Err(_) => DISPATCH_ARRAY_PAYLOAD_BASE,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ARRAY_TAG_BASE, array_len_from_tag, array_tag_from_safe_array, marshal_dispatch_argument,
        safe_array_from_tag,
    };
    use crate::RuntimeValue;

    #[test]
    fn safe_array_tag_roundtrip_for_vector_shape() {
        let tag = ARRAY_TAG_BASE + 3;
        let array = safe_array_from_tag(tag).expect("array tag should decode");
        assert_eq!(array.len, 3);
        assert_eq!(array.dimensions, 1);
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
        let array = super::SafeArray::from_values(vec![RuntimeValue::I32(4), RuntimeValue::I32(9)]);
        assert_eq!(array.dimensions, 1);
        assert_eq!(array.len, 2);
        assert_eq!(array.effective_len(), 2);
        assert_eq!(
            array.elements,
            Some(vec![RuntimeValue::I32(4), RuntimeValue::I32(9)])
        );
        assert_eq!(array_tag_from_safe_array(&array), Some(ARRAY_TAG_BASE + 2));
    }
}
