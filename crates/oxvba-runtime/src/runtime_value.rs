use crate::{
    bstr::BStr,
    decimal::Decimal96,
    safe_array::{SafeArray, array_tag_from_safe_array, safe_array_from_tag},
    value_tags::{EMPTY_TAG, NULL_TAG, error_code_from_tag, error_tag_from_code, is_error_tag},
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

define_i32_handle!(ObjectHandle);
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
    if serial.is_nan() || serial < DATE_MIN || serial > DATE_MAX {
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
    ObjectHandle(ObjectHandle),
    BindingHandle(BindingHandle),
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
            Self::I64(value) => i32::try_from(*value).map_err(|_| {
                format!("i64 value {value} cannot be represented in current legacy i32 slot lane")
            }),
            Self::F64(_) => {
                Err("f64 cannot be represented in current legacy i32 slot lane".to_string())
            }
            Self::Decimal(_) => {
                Err("decimal cannot be represented in current legacy i32 slot lane".to_string())
            }
            Self::Currency(_) => {
                Err("currency cannot be represented in current legacy i32 slot lane".to_string())
            }
            Self::Bool(value) => Ok(i32::from(*value)),
            Self::ArrayIntent(array) => array_tag_from_safe_array(array).ok_or_else(|| {
                "array intent cannot be represented in current legacy slot tag".to_string()
            }),
            Self::ObjectHandle(handle) => Ok(handle.raw()),
            Self::BindingHandle(handle) => Ok(handle.raw()),
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

    use super::{BindingHandle, CurrencyValue, F64Subtype, F64Value, ObjectHandle, RuntimeValue};
    use crate::decimal::Decimal96;

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
        assert!(
            RuntimeValue::F64(F64Value::from_f64(3.5))
                .to_legacy_i32()
                .is_err()
        );
        assert!(
            RuntimeValue::Decimal(Decimal96::from_parts(123_450, 0, 0, 3, false))
                .to_legacy_i32()
                .is_err()
        );
        assert!(
            RuntimeValue::Currency(CurrencyValue::from_scaled_i64(125_000))
                .to_legacy_i32()
                .is_err()
        );
    }

    #[test]
    fn runtime_value_array_payload_still_projects_legacy_shape_by_length() {
        let value = RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
            RuntimeValue::I32(4),
            RuntimeValue::I32(9),
        ]));
        assert_eq!(
            value.to_legacy_i32().expect("array tag"),
            ARRAY_TAG_BASE + 2
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

    #[test]
    fn runtime_value_binding_handles_preserve_legacy_shape() {
        assert_eq!(
            RuntimeValue::BindingHandle(BindingHandle::new(77))
                .to_legacy_i32()
                .expect("binding handle"),
            77
        );
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
    fn runtime_value_i64_to_legacy_i32_narrows_when_fits() {
        assert_eq!(RuntimeValue::I64(42).to_legacy_i32().expect("fits i32"), 42);
        assert_eq!(
            RuntimeValue::I64(-100).to_legacy_i32().expect("fits i32"),
            -100
        );
        assert_eq!(
            RuntimeValue::I64(i32::MAX as i64)
                .to_legacy_i32()
                .expect("fits i32"),
            i32::MAX
        );
    }

    #[test]
    fn runtime_value_i64_to_legacy_i32_rejects_overflow() {
        assert!(RuntimeValue::I64(5_000_000_000).to_legacy_i32().is_err());
        assert!(RuntimeValue::I64(i64::MIN).to_legacy_i32().is_err());
    }
}

#[cfg(test)]
mod proptests {
    use super::{CurrencyValue, RuntimeValue};
    use crate::safe_array::is_array_tag;
    use crate::value_tags::{EMPTY_TAG, NULL_TAG, is_error_tag};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_runtime_value_legacy_i32_roundtrip(v: i32) {
            // Values that land in reserved tag bands (empty, null, error, array)
            // get decoded into a different RuntimeValue variant, so they won't
            // roundtrip as I32. We only test the plain-integer domain.
            prop_assume!(v != EMPTY_TAG);
            prop_assume!(v != NULL_TAG);
            prop_assume!(!is_error_tag(v));
            prop_assume!(!is_array_tag(v));

            let rt = RuntimeValue::from_legacy_i32(v);
            let back = rt.to_legacy_i32().expect("plain i32 should roundtrip");
            prop_assert_eq!(back, v);
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
