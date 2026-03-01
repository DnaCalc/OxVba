pub const EMPTY_TAG: i32 = 0;
pub const NULL_TAG: i32 = -1;

pub const ERROR_TAG_BASE: i32 = -900_000_000;
pub const ERROR_TAG_LIMIT: i32 = -899_000_001;
const ERROR_TAG_SPAN: i32 = ERROR_TAG_LIMIT - ERROR_TAG_BASE;

pub fn error_tag_from_code(code: i32) -> i32 {
    let normalized = code.saturating_abs().min(ERROR_TAG_SPAN);
    ERROR_TAG_BASE.saturating_add(normalized)
}

pub fn is_error_tag(value: i32) -> bool {
    (ERROR_TAG_BASE..=ERROR_TAG_LIMIT).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::{
        EMPTY_TAG, ERROR_TAG_BASE, ERROR_TAG_LIMIT, NULL_TAG, error_tag_from_code, is_error_tag,
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
}
