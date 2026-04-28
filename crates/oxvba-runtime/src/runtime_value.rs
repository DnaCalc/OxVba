use crate::{
    bstr::BStr, decimal::Decimal96, object_ref::ObjectRef, safe_array::SafeArray, variant::Variant,
};

macro_rules! define_i32_handle {
    ($name:ident) => {
        #[repr(transparent)]
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

define_i32_handle!(BindingHandle);
define_i32_handle!(DynLinkSymbol);

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
pub enum F64Subtype {
    Single,
    #[default]
    Double,
    Date,
}

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
pub struct F64Value {
    bits: u64,
    subtype: F64Subtype,
}

impl F64Value {
    pub const fn from_bits(bits: u64) -> Self {
        Self {
            bits,
            subtype: F64Subtype::Double,
        }
    }

    pub const fn from_bits_with_subtype(bits: u64, subtype: F64Subtype) -> Self {
        Self { bits, subtype }
    }

    pub fn from_f64(value: f64) -> Self {
        Self::from_bits_with_subtype(value.to_bits(), F64Subtype::Double)
    }

    pub fn from_single_f64(value: f64) -> Self {
        Self::from_bits_with_subtype(value.to_bits(), F64Subtype::Single)
    }

    pub fn from_date_f64(value: f64) -> Self {
        Self::from_bits_with_subtype(value.to_bits(), F64Subtype::Date)
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }

    pub const fn subtype(self) -> F64Subtype {
        self.subtype
    }

    pub fn as_f64(self) -> f64 {
        f64::from_bits(self.bits)
    }
}

impl core::fmt::Display for F64Value {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_f64())
    }
}

#[repr(transparent)]
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
pub struct CurrencyValue(i64);

impl CurrencyValue {
    pub const SCALE: i64 = 10_000;

    /// Minimum representable Currency value: -922,337,203,685,477.5808
    pub const MIN_SCALED: i64 = i64::MIN;

    /// Maximum representable Currency value:  922,337,203,685,477.5807
    pub const MAX_SCALED: i64 = i64::MAX;

    pub const fn from_scaled_i64(scaled: i64) -> Self {
        Self(scaled)
    }

    pub const fn scaled_i64(self) -> i64 {
        self.0
    }

    /// Validates that a floating-point value is within the Currency range
    /// before scaling.  Returns `Err` on overflow.
    pub fn validate_from_f64(value: f64) -> Result<Self, String> {
        let scaled = value * Self::SCALE as f64;
        if scaled < Self::MIN_SCALED as f64 || scaled > Self::MAX_SCALED as f64 || scaled.is_nan() {
            return Err(format!("Currency overflow: {value}"));
        }
        Ok(Self(scaled as i64))
    }
}

/// VBA Date serial range: 100-Jan-1 (serial −657434) to 9999-Dec-31 (serial 2958465).
pub fn validate_date_range(serial: f64) -> Result<f64, String> {
    const DATE_MIN: f64 = -657_434.0;
    const DATE_MAX: f64 = 2_958_465.0;
    if serial.is_nan() || !(DATE_MIN..=DATE_MAX).contains(&serial) {
        return Err(format!("Date overflow: {serial}"));
    }
    Ok(serial)
}

impl core::fmt::Display for CurrencyValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let scaled = i128::from(self.0);
        let negative = scaled < 0;
        let magnitude = if negative { -scaled } else { scaled };
        let whole = magnitude / i128::from(Self::SCALE);
        let fractional = (magnitude % i128::from(Self::SCALE)) as u16;
        if negative {
            write!(f, "-")?;
        }
        if fractional == 0 {
            return write!(f, "{whole}");
        }
        let mut fractional_text = format!("{fractional:04}");
        while fractional_text.ends_with('0') {
            fractional_text.pop();
        }
        write!(f, "{whole}.{fractional_text}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RuntimeValue {
    #[default]
    Empty,
    Null,
    ErrorCode(i32),
    I32(i32),
    I64(i64),
    F64(F64Value),
    Decimal(Decimal96),
    Currency(CurrencyValue),
    Bool(bool),
    String(BStr),
    ArrayIntent(SafeArray),
    Object(ObjectRef),
    BindingHandle(BindingHandle),
}

impl RuntimeValue {
    /// Compatibility bridge from the legacy semantic carrier into the retained
    /// runtime [`Variant`].
    ///
    /// New value-model code should construct `Variant` values directly.
    pub fn to_variant(&self) -> Result<Variant, String> {
        Variant::try_from_runtime_value(self)
    }

    /// Compatibility projection from the retained runtime [`Variant`] into the
    /// legacy semantic carrier.
    pub fn from_variant(value: &Variant) -> Result<Self, String> {
        value.to_runtime_value()
    }

    pub fn as_i32_lossy(&self) -> Option<i32> {
        match self {
            Self::Empty => Some(0),
            Self::Null | Self::String(_) | Self::ArrayIntent(_) => None,
            Self::I32(value) => Some(*value),
            Self::I64(value) => i32::try_from(*value).ok(),
            Self::F64(value) => Some(value.as_f64() as i32),
            Self::Decimal(_) => None,
            Self::Currency(value) => Some((value.scaled_i64() / 10_000) as i32),
            Self::Bool(value) => Some(i32::from(*value)),
            Self::ErrorCode(code) => Some(*code),
            Self::Object(handle) => Some(handle.raw()),
            Self::BindingHandle(handle) => Some(handle.raw()),
        }
    }
}

impl From<i32> for RuntimeValue {
    fn from(value: i32) -> Self {
        Self::I32(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::safe_array::SafeArray;

    use super::{CurrencyValue, F64Subtype, F64Value, ObjectRef, RuntimeValue};
    use crate::decimal::Decimal96;

    #[test]
    fn runtime_value_from_i32_is_plain_long() {
        assert_eq!(RuntimeValue::from(0), RuntimeValue::I32(0));
        assert_eq!(RuntimeValue::from(-1), RuntimeValue::I32(-1));
    }

    #[test]
    fn runtime_value_f64_preserves_bit_stable_shape() {
        let value = F64Value::from_date_f64(-12.5);
        assert_eq!(value.as_f64(), -12.5);
        assert_eq!(value.subtype(), F64Subtype::Date);
        assert_eq!(
            F64Value::from_bits_with_subtype(value.bits(), value.subtype()),
            value
        );
    }

    #[test]
    fn runtime_value_currency_preserves_exact_scaled_shape() {
        let value = CurrencyValue::from_scaled_i64(-42_500);
        assert_eq!(value.scaled_i64(), -42_500);
        assert_eq!(value.to_string(), "-4.25");
        assert_eq!(CurrencyValue::from_scaled_i64(3_210_000).to_string(), "321");
    }

    #[test]
    fn runtime_value_decimal_preserves_exact_shape() {
        let value = Decimal96::from_parts(123_450, 0, 0, 3, true);
        assert_eq!(value.scale(), 3);
        assert!(value.is_negative());
        assert_eq!(value.to_string(), "-123.45");
        assert_eq!(
            RuntimeValue::Decimal(value),
            RuntimeValue::Decimal(Decimal96::from_scale_sign(123_450, 0, 0, value.scale_sign))
        );
    }

    #[test]
    fn runtime_value_variant_bridge_roundtrips_extended_shapes() {
        let string_value = RuntimeValue::String(crate::bstr::BStr::from("abc"));
        assert_eq!(
            RuntimeValue::from_variant(&string_value.to_variant().expect("string variant"))
                .expect("string roundtrip"),
            string_value
        );
        let object_value = RuntimeValue::Object(ObjectRef::from_compat_identity(42));
        let roundtripped =
            RuntimeValue::from_variant(&object_value.to_variant().expect("object variant"))
                .expect("object roundtrip");
        let RuntimeValue::Object(object_ref) = roundtripped else {
            panic!("expected canonical object-ref runtime carrier");
        };
        assert_eq!(object_ref.raw(), 42);
        let object_ref_value = RuntimeValue::Object(ObjectRef::from_compat_identity(42));
        let roundtripped =
            RuntimeValue::from_variant(&object_ref_value.to_variant().expect("object-ref variant"))
                .expect("object-ref roundtrip");
        let RuntimeValue::Object(object_ref) = roundtripped else {
            panic!("expected object-ref runtime carrier");
        };
        assert_eq!(object_ref.raw(), 42);
        let array_value = RuntimeValue::ArrayIntent(SafeArray::vector(3));
        assert_eq!(
            RuntimeValue::from_variant(&array_value.to_variant().expect("array variant"))
                .expect("array roundtrip"),
            array_value
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::{CurrencyValue, RuntimeValue};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_runtime_value_from_i32_is_plain_i32(v: i32) {
            prop_assert_eq!(RuntimeValue::from(v), RuntimeValue::I32(v));
        }

        #[test]
        fn prop_currency_display_roundtrip(scaled: i64) {
            let cv = CurrencyValue::from_scaled_i64(scaled);
            // The display implementation must not panic for any i64 input.
            let text = cv.to_string();
            prop_assert!(!text.is_empty(), "display produced empty string for scaled={}", scaled);
        }
    }
}
