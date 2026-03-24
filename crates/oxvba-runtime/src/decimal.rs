#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Decimal96 {
    pub lo: u32,
    pub mid: u32,
    pub hi: u32,
    pub scale_sign: u16,
}

impl Decimal96 {
    pub const SIGN_MASK: u16 = 0x8000;

    pub const fn from_parts(lo: u32, mid: u32, hi: u32, scale: u8, negative: bool) -> Self {
        let sign = if negative { Self::SIGN_MASK } else { 0 };
        Self {
            lo,
            mid,
            hi,
            scale_sign: sign | (scale as u16),
        }
    }

    pub const fn from_scale_sign(lo: u32, mid: u32, hi: u32, scale_sign: u16) -> Self {
        Self {
            lo,
            mid,
            hi,
            scale_sign,
        }
    }

    pub const fn scale(self) -> u8 {
        (self.scale_sign & 0x00FF) as u8
    }

    pub const fn is_negative(self) -> bool {
        (self.scale_sign & Self::SIGN_MASK) != 0
    }

    pub const fn magnitude_u128(self) -> u128 {
        (self.lo as u128) | ((self.mid as u128) << 32) | ((self.hi as u128) << 64)
    }
}

impl core::fmt::Display for Decimal96 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let scale = u32::from(self.scale());
        if scale > 28 {
            return write!(
                f,
                "<decimal:lo={};mid={};hi={};scale_sign={}>",
                self.lo, self.mid, self.hi, self.scale_sign
            );
        }
        let magnitude = self.magnitude_u128();
        if magnitude == 0 {
            return write!(f, "0");
        }
        let divisor = 10u128.pow(scale);
        let whole = magnitude / divisor;
        let fractional = magnitude % divisor;
        if self.is_negative() {
            write!(f, "-")?;
        }
        if scale == 0 || fractional == 0 {
            return write!(f, "{whole}");
        }
        let mut fractional_text = format!("{fractional:0width$}", width = scale as usize);
        while fractional_text.ends_with('0') {
            fractional_text.pop();
        }
        write!(f, "{whole}.{fractional_text}")
    }
}

#[cfg(test)]
mod tests {
    use super::Decimal96;

    #[test]
    fn decimal96_preserves_exact_parts() {
        let value = Decimal96::from_parts(0x5566_7788, 0x1122_3344, 0x99AA_BBCC, 4, true);
        assert_eq!(value.lo, 0x5566_7788);
        assert_eq!(value.mid, 0x1122_3344);
        assert_eq!(value.hi, 0x99AA_BBCC);
        assert_eq!(value.scale(), 4);
        assert!(value.is_negative());
        assert_eq!(
            Decimal96::from_scale_sign(value.lo, value.mid, value.hi, value.scale_sign),
            value
        );
    }

    #[test]
    fn decimal96_display_trims_fractional_trailing_zeroes() {
        assert_eq!(
            Decimal96::from_parts(1_250_000, 0, 0, 4, false).to_string(),
            "125"
        );
        assert_eq!(
            Decimal96::from_parts(123_450, 0, 0, 3, false).to_string(),
            "123.45"
        );
        assert_eq!(
            Decimal96::from_parts(42_500, 0, 0, 4, true).to_string(),
            "-4.25"
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::Decimal96;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_decimal96_from_parts_accessor_roundtrip(
            lo: u32,
            mid: u32,
            hi: u32,
            scale in 0u8..=28u8,
            negative: bool,
        ) {
            let d = Decimal96::from_parts(lo, mid, hi, scale, negative);
            prop_assert_eq!(d.lo, lo);
            prop_assert_eq!(d.mid, mid);
            prop_assert_eq!(d.hi, hi);
            prop_assert_eq!(d.scale(), scale);
            prop_assert_eq!(d.is_negative(), negative);
        }
    }
}
