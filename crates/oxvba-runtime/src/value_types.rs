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

#[cfg(test)]
mod tests {
    use super::{CurrencyValue, F64Subtype, F64Value};

    #[test]
    fn f64_value_preserves_bit_stable_shape() {
        let value = F64Value::from_date_f64(-12.5);
        assert_eq!(value.as_f64(), -12.5);
        assert_eq!(value.subtype(), F64Subtype::Date);
        assert_eq!(
            F64Value::from_bits_with_subtype(value.bits(), value.subtype()),
            value
        );
    }

    #[test]
    fn currency_value_preserves_exact_scaled_shape() {
        let value = CurrencyValue::from_scaled_i64(-42_500);
        assert_eq!(value.scaled_i64(), -42_500);
        assert_eq!(value.to_string(), "-4.25");
        assert_eq!(CurrencyValue::from_scaled_i64(3_210_000).to_string(), "321");
    }
}

#[cfg(test)]
mod proptests {
    use super::CurrencyValue;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_currency_display_roundtrip(scaled: i64) {
            let cv = CurrencyValue::from_scaled_i64(scaled);
            // The display implementation must not panic for any i64 input.
            let text = cv.to_string();
            prop_assert!(!text.is_empty(), "display produced empty string for scaled={}", scaled);
        }
    }
}
