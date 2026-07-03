//! Typed arithmetic kernels shared by vm3 support code and the JIT ABI shims.

use crate::arith::ArithError;

pub const CURRENCY_SCALE: i128 = 10_000;
pub const CURRENCY_MIN: i128 = i64::MIN as i128;
pub const CURRENCY_MAX: i128 = i64::MAX as i128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedIntBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

pub fn checked_i64_binop(op: CheckedIntBinOp, lhs: i64, rhs: i64) -> Result<i64, ArithError> {
    match op {
        CheckedIntBinOp::Add => checked_i64_add(lhs, rhs),
        CheckedIntBinOp::Sub => checked_i64_sub(lhs, rhs),
        CheckedIntBinOp::Mul => checked_i64_mul(lhs, rhs),
        CheckedIntBinOp::Div => {
            if rhs == 0 {
                Err(ArithError::div_by_zero())
            } else {
                lhs.checked_div(rhs).ok_or_else(ArithError::overflow)
            }
        }
        CheckedIntBinOp::Rem => {
            if rhs == 0 {
                Err(ArithError::div_by_zero())
            } else {
                lhs.checked_rem(rhs).ok_or_else(ArithError::overflow)
            }
        }
    }
}

pub fn checked_i64_add(lhs: i64, rhs: i64) -> Result<i64, ArithError> {
    lhs.checked_add(rhs).ok_or_else(ArithError::overflow)
}

pub fn checked_i64_sub(lhs: i64, rhs: i64) -> Result<i64, ArithError> {
    lhs.checked_sub(rhs).ok_or_else(ArithError::overflow)
}

pub fn checked_i64_mul(lhs: i64, rhs: i64) -> Result<i64, ArithError> {
    lhs.checked_mul(rhs).ok_or_else(ArithError::overflow)
}

pub fn checked_i64_neg(value: i64) -> Result<i64, ArithError> {
    value.checked_neg().ok_or_else(ArithError::overflow)
}

pub fn narrow_i64_to_i32(value: i64) -> Result<i32, ArithError> {
    i32::try_from(value).map_err(|_| ArithError::overflow())
}

pub fn narrow_i64_to_i16(value: i64) -> Result<i16, ArithError> {
    i16::try_from(value).map_err(|_| ArithError::overflow())
}

pub fn narrow_i64_to_u8(value: i64) -> Result<u8, ArithError> {
    u8::try_from(value).map_err(|_| ArithError::overflow())
}

pub fn currency_add_scaled(lhs: i64, rhs: i64) -> Result<i64, ArithError> {
    currency_add_scaled_i128(i128::from(lhs), i128::from(rhs))
}

pub fn currency_sub_scaled(lhs: i64, rhs: i64) -> Result<i64, ArithError> {
    currency_sub_scaled_i128(i128::from(lhs), i128::from(rhs))
}

pub fn currency_checked_scaled_i128(scaled: i128) -> Result<i64, ArithError> {
    if !(CURRENCY_MIN..=CURRENCY_MAX).contains(&scaled) {
        return Err(ArithError::overflow());
    }
    Ok(scaled as i64)
}

pub fn currency_scale_integer_to_scaled(value: i128) -> Result<i128, ArithError> {
    value
        .checked_mul(CURRENCY_SCALE)
        .filter(|scaled| (CURRENCY_MIN..=CURRENCY_MAX).contains(scaled))
        .ok_or_else(ArithError::overflow)
}

pub fn currency_add_scaled_i128(lhs: i128, rhs: i128) -> Result<i64, ArithError> {
    currency_checked_scaled_i128(lhs.checked_add(rhs).ok_or_else(ArithError::overflow)?)
}

pub fn currency_sub_scaled_i128(lhs: i128, rhs: i128) -> Result<i64, ArithError> {
    currency_checked_scaled_i128(lhs.checked_sub(rhs).ok_or_else(ArithError::overflow)?)
}

pub fn currency_mul_scaled_i128(lhs: i128, rhs: i128) -> Result<i64, ArithError> {
    let product = lhs.checked_mul(rhs).ok_or_else(ArithError::overflow)?;
    currency_checked_scaled_i128(div_round_ties_even(product, CURRENCY_SCALE))
}

pub fn currency_from_f64(value: f64) -> Result<i64, ArithError> {
    let scaled = (value * 10_000.0).round_ties_even();
    if scaled.is_finite() && scaled >= i64::MIN as f64 && scaled <= i64::MAX as f64 {
        Ok(scaled as i64)
    } else {
        Err(ArithError::overflow())
    }
}

pub fn div_round_ties_even(numerator: i128, denominator: i128) -> i128 {
    debug_assert!(denominator > 0);
    let negative = numerator < 0;
    let magnitude = if negative { -numerator } else { numerator };
    let quotient = magnitude / denominator;
    let remainder = magnitude % denominator;
    let twice_remainder = remainder * 2;
    let rounded =
        if twice_remainder > denominator || (twice_remainder == denominator && quotient % 2 != 0) {
            quotient + 1
        } else {
            quotient
        };
    if negative { -rounded } else { rounded }
}

pub fn f64_add(lhs: f64, rhs: f64) -> f64 {
    lhs + rhs
}

pub fn f64_sub(lhs: f64, rhs: f64) -> f64 {
    lhs - rhs
}

pub fn f64_mul(lhs: f64, rhs: f64) -> f64 {
    lhs * rhs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arith;
    use oxvba_bundle::{NumericCoerceTarget, NumericMode};
    use oxvba_runtime::Variant;
    use proptest::prelude::*;

    fn assert_i32_equivalent(
        typed: Result<i32, ArithError>,
        variant: Result<Variant, ArithError>,
    ) -> Result<(), TestCaseError> {
        match (typed, variant) {
            (Ok(lhs), Ok(rhs)) => {
                prop_assert_eq!(rhs.as_i32(), Some(lhs));
                Ok(())
            }
            (Err(lhs), Err(rhs)) => {
                prop_assert_eq!(lhs.code, rhs.code);
                Ok(())
            }
            (lhs, rhs) => Err(TestCaseError::fail(format!(
                "typed/Variant mismatch: typed={lhs:?}, variant={rhs:?}"
            ))),
        }
    }

    fn assert_currency_equivalent(
        typed: Result<i64, ArithError>,
        variant: Result<Variant, ArithError>,
    ) -> Result<(), TestCaseError> {
        match (typed, variant) {
            (Ok(lhs), Ok(rhs)) => {
                prop_assert_eq!(rhs.as_currency_scaled_i64(), Some(lhs));
                Ok(())
            }
            (Err(lhs), Err(rhs)) => {
                prop_assert_eq!(lhs.code, rhs.code);
                Ok(())
            }
            (lhs, rhs) => Err(TestCaseError::fail(format!(
                "typed/Variant mismatch: typed={lhs:?}, variant={rhs:?}"
            ))),
        }
    }

    #[test]
    fn checked_integer_overflow_reports_vba_overflow() {
        let err = checked_i64_add(i64::MAX, 1).expect_err("overflow");
        assert_eq!(err.code, 6);
    }

    #[test]
    fn narrowing_reports_vba_overflow() {
        let err = narrow_i64_to_i32(i64::from(i32::MAX) + 1).expect_err("overflow");
        assert_eq!(err.code, 6);
    }

    proptest! {
        #[test]
        fn checked_i32_add_matches_variant_facade(lhs in any::<i32>(), rhs in any::<i32>()) {
            let typed = checked_i64_add(i64::from(lhs), i64::from(rhs))
                .and_then(narrow_i64_to_i32);
            let variant = arith::add(
                &Variant::from_i32(lhs),
                &Variant::from_i32(rhs),
                NumericMode::Checked(NumericCoerceTarget::Long),
            );
            assert_i32_equivalent(typed, variant)?;
        }

        #[test]
        fn checked_i32_sub_matches_variant_facade(lhs in any::<i32>(), rhs in any::<i32>()) {
            let typed = checked_i64_sub(i64::from(lhs), i64::from(rhs))
                .and_then(narrow_i64_to_i32);
            let variant = arith::sub(
                &Variant::from_i32(lhs),
                &Variant::from_i32(rhs),
                NumericMode::Checked(NumericCoerceTarget::Long),
            );
            assert_i32_equivalent(typed, variant)?;
        }

        #[test]
        fn checked_i32_mul_matches_variant_facade(lhs in any::<i32>(), rhs in any::<i32>()) {
            let typed = checked_i64_mul(i64::from(lhs), i64::from(rhs))
                .and_then(narrow_i64_to_i32);
            let variant = arith::mul(
                &Variant::from_i32(lhs),
                &Variant::from_i32(rhs),
                NumericMode::Checked(NumericCoerceTarget::Long),
            );
            assert_i32_equivalent(typed, variant)?;
        }

        #[test]
        fn currency_add_matches_variant_facade(lhs in any::<i64>(), rhs in any::<i64>()) {
            let typed = currency_add_scaled(lhs, rhs);
            let variant = arith::add(
                &Variant::from_currency_scaled_i64(lhs),
                &Variant::from_currency_scaled_i64(rhs),
                NumericMode::Checked(NumericCoerceTarget::Currency),
            );
            assert_currency_equivalent(typed, variant)?;
        }

        #[test]
        fn currency_sub_matches_variant_facade(lhs in any::<i64>(), rhs in any::<i64>()) {
            let typed = currency_sub_scaled(lhs, rhs);
            let variant = arith::sub(
                &Variant::from_currency_scaled_i64(lhs),
                &Variant::from_currency_scaled_i64(rhs),
                NumericMode::Checked(NumericCoerceTarget::Currency),
            );
            assert_currency_equivalent(typed, variant)?;
        }

        #[test]
        fn currency_mul_matches_variant_facade(lhs in any::<i64>(), rhs in any::<i64>()) {
            let typed = currency_mul_scaled_i128(i128::from(lhs), i128::from(rhs));
            let variant = arith::mul(
                &Variant::from_currency_scaled_i64(lhs),
                &Variant::from_currency_scaled_i64(rhs),
                NumericMode::Checked(NumericCoerceTarget::Currency),
            );
            assert_currency_equivalent(typed, variant)?;
        }
    }
}
