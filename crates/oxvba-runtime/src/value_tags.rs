pub const EMPTY_TAG: i32 = 0;
pub const NULL_TAG: i32 = -1;

pub const ERROR_TAG_BASE: i32 = -900_000_000;
pub const ERROR_TAG_LIMIT: i32 = -899_000_001;
const ERROR_TAG_SPAN: i32 = ERROR_TAG_LIMIT - ERROR_TAG_BASE;

pub fn error_tag_from_code(code: i32) -> i32 {
    let normalized = code.saturating_abs().min(ERROR_TAG_SPAN);
    ERROR_TAG_BASE.saturating_add(normalized)
}

pub fn error_code_from_tag(value: i32) -> Option<i32> {
    if !is_error_tag(value) {
        return None;
    }
    value.checked_sub(ERROR_TAG_BASE)
}

pub fn is_error_tag(value: i32) -> bool {
    (ERROR_TAG_BASE..=ERROR_TAG_LIMIT).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::{
        EMPTY_TAG, ERROR_TAG_BASE, ERROR_TAG_LIMIT, NULL_TAG, error_code_from_tag,
        error_tag_from_code, is_error_tag,
    };

    #[test]
    fn empty_and_null_tags_are_stable() {
        assert_eq!(EMPTY_TAG, 0);
        assert_eq!(NULL_TAG, -1);
    }

    #[test]
    fn cverr_tag_encoding_is_in_reserved_error_range() {
        let tag = error_tag_from_code(17);
        assert!(is_error_tag(tag));
        assert_eq!(tag, ERROR_TAG_BASE + 17);
        assert!(!is_error_tag(NULL_TAG));
        assert!(!is_error_tag(EMPTY_TAG));
    }

    #[test]
    fn cverr_tag_encoding_clamps_large_codes() {
        let tag = error_tag_from_code(i32::MAX);
        assert_eq!(tag, ERROR_TAG_LIMIT);
        assert!(is_error_tag(tag));
    }

    #[test]
    fn cverr_tag_roundtrips_back_to_code() {
        let tag = error_tag_from_code(17);
        assert_eq!(error_code_from_tag(tag), Some(17));
        assert_eq!(error_code_from_tag(NULL_TAG), None);
        assert_eq!(error_code_from_tag(EMPTY_TAG), None);
    }
}

#[cfg(test)]
mod proptests {
    use super::{ERROR_TAG_BASE, ERROR_TAG_LIMIT, error_code_from_tag, error_tag_from_code};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_error_tag_encoding_always_in_reserved_band(code: i32) {
            let tag = error_tag_from_code(code);
            prop_assert!(
                (ERROR_TAG_BASE..=ERROR_TAG_LIMIT).contains(&tag),
                "tag {} from code {} is outside the reserved error band {}..={}",
                tag, code, ERROR_TAG_BASE, ERROR_TAG_LIMIT,
            );
        }

        #[test]
        fn prop_error_tag_roundtrip(code in 0..=999_999i32) {
            let tag = error_tag_from_code(code);
            let recovered = error_code_from_tag(tag);
            prop_assert_eq!(
                recovered,
                Some(code.abs()),
                "roundtrip failed: code={}, tag={}, recovered={:?}",
                code, tag, recovered,
            );
        }
    }
}
