use crate::{
    bstr::BStr,
    safe_array::{SafeArray, array_tag_from_safe_array, safe_array_from_tag},
    value_tags::{EMPTY_TAG, NULL_TAG, error_code_from_tag, error_tag_from_code, is_error_tag},
};

macro_rules! define_i32_handle {
    ($name:ident) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Default,
            rkyv::Archive,
            rkyv::Serialize,
            rkyv::Deserialize,
            serde::Serialize,
            serde::Deserialize,
        )]
        pub struct $name(i32);

        impl $name {
            pub const fn new(raw: i32) -> Self {
                Self(raw)
            }

            pub const fn raw(self) -> i32 {
                self.0
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<i32> for $name {
            fn from(value: i32) -> Self {
                Self::new(value)
            }
        }

        impl From<$name> for i32 {
            fn from(value: $name) -> Self {
                value.raw()
            }
        }
    };
}

define_i32_handle!(ObjectHandle);
define_i32_handle!(BindingHandle);
define_i32_handle!(DynLinkSymbol);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RuntimeValue {
    #[default]
    Empty,
    Null,
    ErrorCode(i32),
    I32(i32),
    Bool(bool),
    String(BStr),
    ArrayIntent(SafeArray),
    ObjectHandle(ObjectHandle),
}

impl RuntimeValue {
    pub fn from_legacy_i32(value: i32) -> Self {
        if value == EMPTY_TAG {
            return Self::Empty;
        }
        if value == NULL_TAG {
            return Self::Null;
        }
        if is_error_tag(value) {
            return Self::ErrorCode(error_code_from_tag(value).unwrap_or(0));
        }
        if let Some(array) = safe_array_from_tag(value) {
            return Self::ArrayIntent(array);
        }
        Self::I32(value)
    }

    pub fn to_legacy_i32(&self) -> Result<i32, String> {
        match self {
            Self::Empty => Ok(EMPTY_TAG),
            Self::Null => Ok(NULL_TAG),
            Self::ErrorCode(code) => Ok(error_tag_from_code(*code)),
            Self::I32(value) => Ok(*value),
            Self::Bool(value) => Ok(i32::from(*value)),
            Self::ArrayIntent(array) => array_tag_from_safe_array(array).ok_or_else(|| {
                "array intent cannot be represented in current legacy slot tag".to_string()
            }),
            Self::ObjectHandle(handle) => Ok(handle.raw()),
            Self::String(_) => {
                Err("string cannot be represented in current legacy i32 slot lane".to_string())
            }
        }
    }

    pub fn as_i32_lossy(&self) -> Option<i32> {
        self.to_legacy_i32().ok()
    }
}

impl From<i32> for RuntimeValue {
    fn from(value: i32) -> Self {
        Self::from_legacy_i32(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        safe_array::{ARRAY_TAG_BASE, SafeArray},
        value_tags::{EMPTY_TAG, NULL_TAG, error_tag_from_code},
    };

    use super::{ObjectHandle, RuntimeValue};

    #[test]
    fn runtime_value_from_legacy_i32_preserves_tagged_shapes() {
        assert_eq!(
            RuntimeValue::from_legacy_i32(EMPTY_TAG),
            RuntimeValue::Empty
        );
        assert_eq!(RuntimeValue::from_legacy_i32(NULL_TAG), RuntimeValue::Null);
        assert_eq!(
            RuntimeValue::from_legacy_i32(error_tag_from_code(17)),
            RuntimeValue::ErrorCode(17)
        );
        assert_eq!(
            RuntimeValue::from_legacy_i32(ARRAY_TAG_BASE + 3),
            RuntimeValue::ArrayIntent(SafeArray::vector(3))
        );
    }

    #[test]
    fn runtime_value_roundtrips_legacy_i32_subset() {
        let value = RuntimeValue::ArrayIntent(SafeArray::vector(4));
        assert_eq!(
            value.to_legacy_i32().expect("array tag"),
            ARRAY_TAG_BASE + 4
        );
        assert_eq!(
            RuntimeValue::Bool(true).to_legacy_i32().expect("bool tag"),
            1
        );
        assert!(
            RuntimeValue::String(crate::bstr::BStr("ABC".to_string()))
                .to_legacy_i32()
                .is_err()
        );
    }

    #[test]
    fn runtime_value_object_handles_preserve_legacy_shape() {
        assert_eq!(
            RuntimeValue::ObjectHandle(ObjectHandle::new(42))
                .to_legacy_i32()
                .expect("object handle"),
            42
        );
    }
}
