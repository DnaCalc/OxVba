pub const DISPATCH_INVOKE_MISSING_ARG_TOKEN: i32 = i32::MIN + 2_048;

use oxvba_runtime::{
    CurrencyValue, Decimal96, F64Value, ObjectRef, Variant,
    bstr::BStr,
    safe_array::{SafeArray, array_tag_from_safe_array, marshal_dispatch_argument},
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
    pub object: ObjectRef,
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
    I64(i64),
    F64(F64Value),
    Decimal(Decimal96),
    Currency(CurrencyValue),
    String(BStr),
    ArrayIntent(SafeArray),
    Object(ObjectRef),
}

impl ComValue {
    /// Converts a retained runtime [`Variant`] into the shared COM semantic
    /// carrier.
    pub fn from_variant(value: &Variant) -> Result<Self, String> {
        Ok(match value.vtype() {
            oxvba_runtime::VarType::Empty => Self::Empty,
            oxvba_runtime::VarType::Null => Self::Null,
            oxvba_runtime::VarType::Integer => Self::I32(
                value
                    .as_i16()
                    .ok_or_else(|| "invalid Integer VARIANT payload".to_string())?
                    as i32,
            ),
            oxvba_runtime::VarType::Long => Self::I32(
                value
                    .as_i32()
                    .ok_or_else(|| "invalid Long VARIANT payload".to_string())?,
            ),
            oxvba_runtime::VarType::LongLong => Self::I64(
                value
                    .as_i64()
                    .ok_or_else(|| "invalid LongLong VARIANT payload".to_string())?,
            ),
            oxvba_runtime::VarType::Byte => Self::I32(i32::from(
                value
                    .as_u8()
                    .ok_or_else(|| "invalid Byte VARIANT payload".to_string())?,
            )),
            oxvba_runtime::VarType::Single => Self::F64(F64Value::from_single_f64(
                value
                    .as_f32()
                    .ok_or_else(|| "invalid Single VARIANT payload".to_string())?
                    as f64,
            )),
            oxvba_runtime::VarType::Double => Self::F64(F64Value::from_f64(
                value
                    .as_f64()
                    .ok_or_else(|| "invalid Double VARIANT payload".to_string())?,
            )),
            oxvba_runtime::VarType::Decimal => Self::Decimal(
                value
                    .as_decimal96()
                    .ok_or_else(|| "invalid Decimal VARIANT payload".to_string())?,
            ),
            oxvba_runtime::VarType::Currency => Self::Currency(CurrencyValue::from_scaled_i64(
                value
                    .as_currency_scaled_i64()
                    .ok_or_else(|| "invalid Currency VARIANT payload".to_string())?,
            )),
            oxvba_runtime::VarType::Date => Self::F64(F64Value::from_date_f64(
                value
                    .as_date_f64()
                    .ok_or_else(|| "invalid Date VARIANT payload".to_string())?,
            )),
            oxvba_runtime::VarType::String => Self::String(
                value
                    .as_bstr()
                    .ok_or_else(|| "invalid String VARIANT payload".to_string())?,
            ),
            oxvba_runtime::VarType::Boolean => Self::Bool(
                value
                    .as_bool()
                    .ok_or_else(|| "invalid Boolean VARIANT payload".to_string())?,
            ),
            oxvba_runtime::VarType::Error => Self::ErrorCode(
                value
                    .as_error_code()
                    .ok_or_else(|| "invalid Error VARIANT payload".to_string())?,
            ),
            oxvba_runtime::VarType::Object => Self::Object(
                value
                    .as_object_ref()
                    .ok_or_else(|| "invalid Object VARIANT payload".to_string())?,
            ),
            oxvba_runtime::VarType::ArrayVariant => Self::ArrayIntent(
                value
                    .as_safearray()
                    .ok_or_else(|| "invalid SAFEARRAY VARIANT payload".to_string())?,
            ),
        })
    }

    /// Compatibility projection from scalar i32 transport into the
    /// shared COM semantic carrier.
    pub fn from_runtime_token(value: i32) -> Self {
        crate::compat::com_value_from_runtime_token(value)
    }

    /// Converts the shared COM semantic carrier into a retained runtime
    /// [`Variant`].
    pub fn to_variant(&self) -> Result<Variant, String> {
        Ok(match self {
            Self::Empty => Variant::empty(),
            Self::Null => Variant::null(),
            Self::ErrorCode(code) => Variant::from_error_code(*code),
            Self::Bool(value) => Variant::from_bool(*value),
            Self::I32(value) => Variant::from_i32(*value),
            Self::I64(value) => Variant::from_i64(*value),
            Self::F64(value) => match value.subtype() {
                oxvba_runtime::F64Subtype::Single => Variant::from_f32(value.as_f64() as f32),
                oxvba_runtime::F64Subtype::Double => Variant::from_f64(value.as_f64()),
                oxvba_runtime::F64Subtype::Date => Variant::from_date_f64(value.as_f64()),
            },
            Self::Decimal(value) => Variant::from_decimal96(*value),
            Self::Currency(value) => Variant::from_currency_scaled_i64(value.scaled_i64()),
            Self::String(value) => Variant::from_string(value.clone()),
            Self::ArrayIntent(array) => Variant::from_safearray(array.clone()),
            Self::Object(handle) => Variant::from_object_ref(handle.clone()),
        })
    }

    /// Compatibility projection into scalar i32 transport.
    pub fn to_runtime_token(&self) -> Result<i32, String> {
        crate::compat::com_value_to_runtime_token(self)
    }

    /// Compatibility projection into legacy dispatch-token transport.
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
pub struct ComInvokeValue {
    value: Variant,
}

impl ComInvokeValue {
    /// Builds an invoke value from the shared COM carrier, retaining the exact
    /// runtime [`Variant`] payload.
    pub fn from_com_value(value: ComValue) -> Self {
        Self {
            value: value
                .to_variant()
                .expect("COM invoke values must be exact VARIANT-compatible"),
        }
    }

    /// Projects the retained invoke `Variant` back into the shared COM carrier.
    pub fn to_com_value(&self) -> ComValue {
        ComValue::from_variant(&self.value).expect("COM invoke VARIANT must project to ComValue")
    }

    /// Compatibility projection into scalar i32 transport.
    pub fn to_runtime_token(&self) -> Result<i32, String> {
        self.to_com_value().to_runtime_token()
    }

    /// Compatibility projection into legacy dispatch-token transport.
    pub fn to_legacy_dispatch_token(&self) -> Result<i32, String> {
        self.to_com_value().to_legacy_dispatch_token()
    }

    /// Retained value-model payload for COM invocation.
    pub fn variant(&self) -> &Variant {
        &self.value
    }
}

impl From<ComValue> for ComInvokeValue {
    fn from(value: ComValue) -> Self {
        Self::from_com_value(value)
    }
}

impl From<ComInvokeValue> for ComValue {
    fn from(value: ComInvokeValue) -> Self {
        value.to_com_value()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComInvokeArg {
    pub value: Option<ComInvokeValue>,
    pub name: Option<String>,
}

impl ComInvokeArg {
    /// Compatibility positional argument from scalar i32 transport.
    ///
    /// New value-model call sites should prefer [`Self::positional_value`].
    pub fn positional(value: i32) -> Self {
        Self {
            value: Some(ComValue::from_runtime_token(value).into()),
            name: None,
        }
    }

    /// Compatibility named argument from scalar i32 transport.
    ///
    /// New value-model call sites should prefer [`Self::named_value`].
    pub fn named(value: i32, name: impl Into<String>) -> Self {
        Self {
            value: Some(ComValue::from_runtime_token(value).into()),
            name: Some(name.into()),
        }
    }

    /// Retained value-model positional argument.
    pub fn positional_value(value: ComValue) -> Self {
        Self {
            value: Some(value.into()),
            name: None,
        }
    }

    /// Retained value-model named argument.
    pub fn named_value(value: ComValue, name: impl Into<String>) -> Self {
        Self {
            value: Some(value.into()),
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
    pub object: ObjectRef,
    pub member: ComMemberToken,
    pub args: Vec<ComInvokeArg>,
    pub invoke_kind_hint: Option<ComInvokeKind>,
}

impl ComInvokeRequest {
    pub fn new(object: ObjectRef, member: ComMemberToken, args: Vec<ComInvokeArg>) -> Self {
        Self {
            object,
            member,
            args,
            invoke_kind_hint: None,
        }
    }

    /// Compatibility request constructor from legacy object/member/argument
    /// tokens.
    pub fn legacy(object: i32, member: i32, arg: i32) -> Self {
        let args = if arg == DISPATCH_INVOKE_MISSING_ARG_TOKEN {
            Vec::new()
        } else {
            vec![ComInvokeArg::positional(arg)]
        };
        Self::new(
            ObjectRef::from_compat_identity(object),
            ComMemberToken::new(member),
            args,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComCallbackValue {
    value: Variant,
}

impl ComCallbackValue {
    /// Builds a callback value from the shared COM carrier, retaining the exact
    /// runtime [`Variant`] payload.
    pub fn from_com_value(value: ComValue) -> Self {
        Self {
            value: value
                .to_variant()
                .expect("COM callback values must be exact VARIANT-compatible"),
        }
    }

    /// Projects the retained callback `Variant` back into the shared COM
    /// carrier.
    pub fn to_com_value(&self) -> ComValue {
        ComValue::from_variant(&self.value).expect("COM callback VARIANT must project to ComValue")
    }

    /// Retained value-model payload for COM callbacks.
    pub fn variant(&self) -> &Variant {
        &self.value
    }
}

impl From<ComValue> for ComCallbackValue {
    fn from(value: ComValue) -> Self {
        Self::from_com_value(value)
    }
}

impl From<ComCallbackValue> for ComValue {
    fn from(value: ComCallbackValue) -> Self {
        value.to_com_value()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComCallbackPayload {
    pub callback: ComCallbackToken,
    pub subscription: ComSubscriptionToken,
    pub object: ObjectRef,
    pub event: ComMemberToken,
    pub args: Vec<ComCallbackValue>,
}

#[cfg(test)]
mod tests {
    use super::{ComCallbackValue, ComInvokeArg, ComMemberToken, ComValue};
    use oxvba_runtime::{
        CurrencyValue, Decimal96, F64Value, ObjectRef, VarType, Variant, bstr::BStr,
        safe_array::SafeArray,
    };

    #[test]
    fn com_value_from_runtime_token_is_plain_i32() {
        assert_eq!(ComValue::from_runtime_token(0), ComValue::I32(0));
        assert_eq!(ComValue::from_runtime_token(-1), ComValue::I32(-1));
    }

    #[test]
    fn com_value_array_intent_projects_to_length_token() {
        let value = ComValue::ArrayIntent(SafeArray::vector(4));
        assert_eq!(value.to_runtime_token().expect("array token"), 4);
        assert_eq!(
            value
                .to_legacy_dispatch_token()
                .expect("legacy dispatch token"),
            20_004
        );
    }

    #[test]
    fn com_value_roundtrips_variant_shape() {
        let value = ComValue::ArrayIntent(SafeArray::vector(5));
        assert_eq!(
            value.to_variant().expect("array variant"),
            Variant::from_safearray(SafeArray::vector(5))
        );
        assert_eq!(
            ComValue::from_variant(&Variant::from_bool(true)).expect("bool variant"),
            ComValue::Bool(true)
        );
        assert_eq!(
            ComValue::from_variant(&Variant::from_f64(3.5)).expect("f64 variant"),
            ComValue::F64(F64Value::from_f64(3.5))
        );
        assert_eq!(
            ComValue::from_variant(&Variant::from_decimal96(Decimal96::from_parts(
                123_450, 0, 0, 3, false
            )))
            .expect("decimal variant"),
            ComValue::Decimal(Decimal96::from_parts(123_450, 0, 0, 3, false))
        );
        assert_eq!(
            ComValue::from_variant(&Variant::from_currency_scaled_i64(125_000))
                .expect("currency variant"),
            ComValue::Currency(CurrencyValue::from_scaled_i64(125_000))
        );
        assert_eq!(
            ComValue::from_variant(&Variant::from_string(BStr::from("ABC")))
                .expect("string variant"),
            ComValue::String(BStr::from("ABC"))
        );
        assert_eq!(
            ComValue::String(BStr::from("ABC"))
                .to_variant()
                .expect("string variant"),
            Variant::from_string(BStr::from("ABC"))
        );
        assert_eq!(
            ComValue::F64(F64Value::from_f64(3.5))
                .to_variant()
                .expect("f64 variant"),
            Variant::from_f64(3.5)
        );
        assert_eq!(
            ComValue::Decimal(Decimal96::from_parts(123_450, 0, 0, 3, true))
                .to_variant()
                .expect("decimal variant"),
            Variant::from_decimal96(Decimal96::from_parts(123_450, 0, 0, 3, true))
        );
        assert_eq!(
            ComValue::Currency(CurrencyValue::from_scaled_i64(-42_500))
                .to_variant()
                .expect("currency variant"),
            Variant::from_currency_scaled_i64(-42_500)
        );
        let object_value = ComValue::from_variant(&Variant::from_object_ref(
            ObjectRef::from_compat_identity(1234),
        ))
        .expect("object variant");
        let ComValue::Object(object_ref) = object_value else {
            panic!("expected ObjectRef-backed COM value");
        };
        assert_eq!(object_ref.raw(), 1234);
        let roundtripped = ComValue::Object(ObjectRef::from_compat_identity(1234))
            .to_variant()
            .expect("object variant");
        let object_ref = roundtripped
            .as_object_ref()
            .expect("expected object-ref variant carrier");
        assert_eq!(object_ref.raw(), 1234);
        assert!(
            ComValue::String(BStr::from("ABC"))
                .to_runtime_token()
                .is_err()
        );
    }

    #[test]
    fn com_value_preserves_safe_array_payload_shape() {
        let value = ComValue::ArrayIntent(SafeArray::from_variants(vec![
            Variant::from_i32(4),
            Variant::from_string(BStr::from("A")),
        ]));
        assert_eq!(
            value.to_variant().expect("safe array variant"),
            Variant::from_safearray(SafeArray::from_variants(vec![
                Variant::from_i32(4),
                Variant::from_string(BStr::from("A")),
            ]))
        );
        assert_eq!(value.to_runtime_token().expect("array token"), 2);
    }

    #[test]
    fn com_value_i64_roundtrips_variant() {
        let large: i64 = 5_000_000_000;
        assert_eq!(
            ComValue::from_variant(&Variant::from_i64(large)).expect("i64 variant"),
            ComValue::I64(large)
        );
        assert_eq!(
            ComValue::I64(large).to_variant().expect("i64 variant"),
            Variant::from_i64(large)
        );
        // I64 that fits in i32 stays I64 (no auto-narrowing at ComValue level)
        assert_eq!(
            ComValue::from_variant(&Variant::from_i64(42)).expect("i64 variant"),
            ComValue::I64(42)
        );
    }

    #[test]
    fn com_value_i64_to_legacy_token_narrows_when_possible() {
        // Fits in i32
        assert_eq!(ComValue::I64(42).to_runtime_token().expect("fits i32"), 42);
        // Does not fit
        assert!(ComValue::I64(5_000_000_000).to_runtime_token().is_err());
    }

    #[test]
    fn com_invoke_arg_retains_payload_as_variant() {
        let arg = ComInvokeArg::positional_value(ComValue::I32(7));
        let value = arg.value.expect("invoke arg should retain a value");
        assert_eq!(value.variant().vtype(), VarType::Long);
        assert_eq!(value.to_com_value(), ComValue::I32(7));
    }

    #[test]
    fn com_callback_value_retains_payload_as_variant() {
        let value = ComCallbackValue::from_com_value(ComValue::I32(7));
        assert_eq!(value.variant().vtype(), VarType::Long);
        assert_eq!(value.to_com_value(), ComValue::I32(7));
    }

    #[test]
    fn com_member_token_roundtrips_raw_dispid() {
        let token = ComMemberToken::new(11);
        assert_eq!(token.raw(), 11);
        assert_eq!(i32::from(token), 11);
    }
}
