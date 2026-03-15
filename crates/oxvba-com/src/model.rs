pub const DISPATCH_INVOKE_MISSING_ARG_TOKEN: i32 = i32::MIN + 2_048;

use oxvba_runtime::{
    F64Value, ObjectHandle, RuntimeValue,
    bstr::BStr,
    safe_array::{
        SafeArray, array_tag_from_safe_array, marshal_dispatch_argument, safe_array_from_tag,
    },
    value_tags::{EMPTY_TAG, NULL_TAG, error_code_from_tag, is_error_tag},
};

macro_rules! define_token {
    ($name:ident) => {
        #[repr(transparent)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(i32);

        impl $name {
            pub const fn new(raw: i32) -> Self {
                Self(raw)
            }

            pub const fn raw(self) -> i32 {
                self.0
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

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "{}", self.raw())
            }
        }
    };
}

define_token!(ComObjectToken);
define_token!(ComMemberToken);
define_token!(ComSubscriptionToken);
define_token!(ComCallbackToken);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComObjectTransportKind {
    Projection,
    NativeDispatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComObjectDescriptor {
    pub object: ObjectHandle,
    pub prog_id_name: String,
    pub transport: ComObjectTransportKind,
    pub supports_events: bool,
    pub known_member_tokens: Vec<ComMemberToken>,
    pub known_event_tokens: Vec<ComMemberToken>,
    pub default_member_token: Option<ComMemberToken>,
    pub default_member_name: Option<String>,
    pub typelib_cache_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComInvokeKind {
    Method,
    PropertyGet,
    PropertyPut,
    PropertyPutRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComValue {
    Empty,
    Null,
    ErrorCode(i32),
    Bool(bool),
    I32(i32),
    F64(F64Value),
    String(BStr),
    ArrayIntent(SafeArray),
    ObjectHandle(ObjectHandle),
}

impl ComValue {
    pub fn from_runtime_value(value: &RuntimeValue) -> Self {
        match value {
            RuntimeValue::Empty => Self::Empty,
            RuntimeValue::Null => Self::Null,
            RuntimeValue::ErrorCode(code) => Self::ErrorCode(*code),
            RuntimeValue::Bool(value) => Self::Bool(*value),
            RuntimeValue::I32(value) => Self::I32(*value),
            RuntimeValue::F64(value) => Self::F64(*value),
            RuntimeValue::String(value) => Self::String(value.clone()),
            RuntimeValue::ArrayIntent(array) => Self::ArrayIntent(array.clone()),
            RuntimeValue::ObjectHandle(handle) => Self::ObjectHandle(*handle),
            RuntimeValue::BindingHandle(handle) => Self::I32(handle.raw()),
        }
    }

    pub fn from_runtime_token(value: i32) -> Self {
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

    pub fn to_runtime_value(&self) -> RuntimeValue {
        match self {
            Self::Empty => RuntimeValue::Empty,
            Self::Null => RuntimeValue::Null,
            Self::ErrorCode(code) => RuntimeValue::ErrorCode(*code),
            Self::Bool(value) => RuntimeValue::Bool(*value),
            Self::I32(value) => RuntimeValue::I32(*value),
            Self::F64(value) => RuntimeValue::F64(*value),
            Self::String(value) => RuntimeValue::String(value.clone()),
            Self::ArrayIntent(array) => RuntimeValue::ArrayIntent(array.clone()),
            Self::ObjectHandle(handle) => RuntimeValue::ObjectHandle(*handle),
        }
    }

    pub fn to_runtime_token(&self) -> Result<i32, String> {
        self.to_runtime_value().to_legacy_i32()
    }

    pub fn to_legacy_dispatch_token(&self) -> Result<i32, String> {
        match self {
            Self::ArrayIntent(array) => {
                let token = array_tag_from_safe_array(array).ok_or_else(|| {
                    "unsupported array intent for legacy dispatch transport".to_string()
                })?;
                Ok(marshal_dispatch_argument(token))
            }
            _ => self.to_runtime_token(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComInvokeArg {
    pub value: Option<ComValue>,
    pub name: Option<String>,
}

impl ComInvokeArg {
    pub fn positional(value: i32) -> Self {
        Self {
            value: Some(ComValue::from_runtime_token(value)),
            name: None,
        }
    }

    pub fn named(value: i32, name: impl Into<String>) -> Self {
        Self {
            value: Some(ComValue::from_runtime_token(value)),
            name: Some(name.into()),
        }
    }

    pub fn positional_value(value: ComValue) -> Self {
        Self {
            value: Some(value),
            name: None,
        }
    }

    pub fn named_value(value: ComValue, name: impl Into<String>) -> Self {
        Self {
            value: Some(value),
            name: Some(name.into()),
        }
    }

    pub fn omitted() -> Self {
        Self {
            value: None,
            name: None,
        }
    }

    pub fn omitted_named(name: impl Into<String>) -> Self {
        Self {
            value: None,
            name: Some(name.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComInvokeRequest {
    pub object: ObjectHandle,
    pub member: ComMemberToken,
    pub args: Vec<ComInvokeArg>,
    pub invoke_kind_hint: Option<ComInvokeKind>,
}

impl ComInvokeRequest {
    pub fn new(object: ObjectHandle, member: ComMemberToken, args: Vec<ComInvokeArg>) -> Self {
        Self {
            object,
            member,
            args,
            invoke_kind_hint: None,
        }
    }

    pub fn legacy(object: i32, member: i32, arg: i32) -> Self {
        let args = if arg == DISPATCH_INVOKE_MISSING_ARG_TOKEN {
            Vec::new()
        } else {
            vec![ComInvokeArg::positional(arg)]
        };
        Self::new(ObjectHandle::new(object), ComMemberToken::new(member), args)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComCallbackPayload {
    pub callback: ComCallbackToken,
    pub subscription: ComSubscriptionToken,
    pub object: ObjectHandle,
    pub event: ComMemberToken,
    pub args: Vec<ComValue>,
}

#[cfg(test)]
mod tests {
    use super::{ComMemberToken, ComValue};
    use oxvba_runtime::{
        F64Value, ObjectHandle, RuntimeValue,
        bstr::BStr,
        safe_array::{ARRAY_TAG_BASE, SafeArray},
        value_tags::{EMPTY_TAG, NULL_TAG, error_tag_from_code},
    };

    #[test]
    fn com_value_from_runtime_token_preserves_array_null_error_shape() {
        assert_eq!(ComValue::from_runtime_token(EMPTY_TAG), ComValue::Empty);
        assert_eq!(ComValue::from_runtime_token(NULL_TAG), ComValue::Null);
        assert_eq!(
            ComValue::from_runtime_token(error_tag_from_code(17)),
            ComValue::ErrorCode(17)
        );
        assert_eq!(
            ComValue::from_runtime_token(ARRAY_TAG_BASE + 3),
            ComValue::ArrayIntent(SafeArray::vector(3))
        );
    }

    #[test]
    fn com_value_array_intent_roundtrips_to_runtime_tag() {
        let value = ComValue::ArrayIntent(SafeArray::vector(4));
        assert_eq!(
            value.to_runtime_token().expect("array token"),
            ARRAY_TAG_BASE + 4
        );
        assert_eq!(
            value
                .to_legacy_dispatch_token()
                .expect("legacy dispatch token"),
            20_004
        );
    }

    #[test]
    fn com_value_roundtrips_runtime_value_shape() {
        let value = ComValue::ArrayIntent(SafeArray::vector(5));
        assert_eq!(
            value.to_runtime_value(),
            RuntimeValue::ArrayIntent(SafeArray::vector(5))
        );
        assert_eq!(
            ComValue::from_runtime_value(&RuntimeValue::Bool(true)),
            ComValue::Bool(true)
        );
        assert_eq!(
            ComValue::from_runtime_value(&RuntimeValue::F64(F64Value::from_f64(3.5))),
            ComValue::F64(F64Value::from_f64(3.5))
        );
        assert_eq!(
            ComValue::from_runtime_value(&RuntimeValue::String(BStr("ABC".to_string()))),
            ComValue::String(BStr("ABC".to_string()))
        );
        assert_eq!(
            ComValue::String(BStr("ABC".to_string())).to_runtime_value(),
            RuntimeValue::String(BStr("ABC".to_string()))
        );
        assert_eq!(
            ComValue::F64(F64Value::from_f64(3.5)).to_runtime_value(),
            RuntimeValue::F64(F64Value::from_f64(3.5))
        );
        assert_eq!(
            ComValue::from_runtime_value(&RuntimeValue::ObjectHandle(ObjectHandle::new(1234))),
            ComValue::ObjectHandle(ObjectHandle::new(1234))
        );
        assert_eq!(
            ComValue::ObjectHandle(ObjectHandle::new(1234)).to_runtime_value(),
            RuntimeValue::ObjectHandle(ObjectHandle::new(1234))
        );
        assert!(
            ComValue::String(BStr("ABC".to_string()))
                .to_runtime_token()
                .is_err()
        );
    }

    #[test]
    fn com_value_preserves_safe_array_payload_shape() {
        let value = ComValue::ArrayIntent(SafeArray::from_values(vec![
            RuntimeValue::I32(4),
            RuntimeValue::String(BStr("A".to_string())),
        ]));
        assert_eq!(
            value.to_runtime_value(),
            RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                RuntimeValue::I32(4),
                RuntimeValue::String(BStr("A".to_string())),
            ]))
        );
        assert_eq!(
            value.to_runtime_token().expect("array tag"),
            ARRAY_TAG_BASE + 2
        );
    }

    #[test]
    fn com_member_token_roundtrips_raw_dispid() {
        let token = ComMemberToken::new(11);
        assert_eq!(token.raw(), 11);
        assert_eq!(i32::from(token), 11);
    }
}
