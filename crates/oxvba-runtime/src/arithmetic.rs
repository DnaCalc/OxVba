use crate::{Variant, coerce::coerce_to, variant::VarType};

/// VBA **Variant** integer-arithmetic promotion: compute in the operands' integer
/// width and widen to `Double` only when the result leaves that width. `Integer`-pair
/// arithmetic promotes to `Long`; `Long`/`LongLong` pairs stay in place unless they
/// overflow. Used by the widening regime ([`oxvba_bundle::NumericMode::Widening`]); the
/// *fixed* regime is handled in the VM with an Overflow error instead of widening.
fn widen_double(lhs: &Variant, rhs: &Variant, f: impl Fn(f64, f64) -> f64) -> Result<Variant, String> {
    let l = coerce_to(lhs, VarType::Double)?;
    let r = coerce_to(rhs, VarType::Double)?;
    Ok(Variant::from_f64(f(l.as_f64().unwrap_or(0.0), r.as_f64().unwrap_or(0.0))))
}

fn i32_or_double(v: i64) -> Variant {
    if (i64::from(i32::MIN)..=i64::from(i32::MAX)).contains(&v) {
        Variant::from_i32(v as i32)
    } else {
        Variant::from_f64(v as f64)
    }
}

fn i64_or_double(v: Option<i64>, lossy: impl Fn() -> f64) -> Variant {
    match v {
        Some(n) => Variant::from_i64(n),
        None => Variant::from_f64(lossy()),
    }
}

pub fn add(lhs: &Variant, rhs: &Variant) -> Result<Variant, String> {
    match (lhs.vtype(), rhs.vtype()) {
        (VarType::Integer, VarType::Integer) => {
            Ok(Variant::from_i32(i32::from(lhs.as_i16().unwrap_or(0)) + i32::from(rhs.as_i16().unwrap_or(0))))
        }
        (VarType::Long, VarType::Long) => {
            Ok(i32_or_double(i64::from(lhs.as_i32().unwrap_or(0)) + i64::from(rhs.as_i32().unwrap_or(0))))
        }
        (VarType::LongLong, VarType::LongLong) => Ok(i64_or_double(
            lhs.as_i64().unwrap_or(0).checked_add(rhs.as_i64().unwrap_or(0)),
            || lhs.as_i64().unwrap_or(0) as f64 + rhs.as_i64().unwrap_or(0) as f64,
        )),
        _ => widen_double(lhs, rhs, |a, b| a + b),
    }
}

pub fn sub(lhs: &Variant, rhs: &Variant) -> Result<Variant, String> {
    match (lhs.vtype(), rhs.vtype()) {
        (VarType::Integer, VarType::Integer) => {
            Ok(Variant::from_i32(i32::from(lhs.as_i16().unwrap_or(0)) - i32::from(rhs.as_i16().unwrap_or(0))))
        }
        (VarType::Long, VarType::Long) => {
            Ok(i32_or_double(i64::from(lhs.as_i32().unwrap_or(0)) - i64::from(rhs.as_i32().unwrap_or(0))))
        }
        (VarType::LongLong, VarType::LongLong) => Ok(i64_or_double(
            lhs.as_i64().unwrap_or(0).checked_sub(rhs.as_i64().unwrap_or(0)),
            || lhs.as_i64().unwrap_or(0) as f64 - rhs.as_i64().unwrap_or(0) as f64,
        )),
        _ => widen_double(lhs, rhs, |a, b| a - b),
    }
}

pub fn mul(lhs: &Variant, rhs: &Variant) -> Result<Variant, String> {
    match (lhs.vtype(), rhs.vtype()) {
        (VarType::Integer, VarType::Integer) => {
            Ok(i32_or_double(i64::from(lhs.as_i16().unwrap_or(0)) * i64::from(rhs.as_i16().unwrap_or(0))))
        }
        (VarType::Long, VarType::Long) => {
            Ok(i32_or_double(i64::from(lhs.as_i32().unwrap_or(0)) * i64::from(rhs.as_i32().unwrap_or(0))))
        }
        (VarType::LongLong, VarType::LongLong) => Ok(i64_or_double(
            lhs.as_i64().unwrap_or(0).checked_mul(rhs.as_i64().unwrap_or(0)),
            || lhs.as_i64().unwrap_or(0) as f64 * rhs.as_i64().unwrap_or(0) as f64,
        )),
        _ => widen_double(lhs, rhs, |a, b| a * b),
    }
}

pub fn neg(v: &Variant) -> Result<Variant, String> {
    match v.vtype() {
        VarType::Integer => Ok(Variant::from_i32(-i32::from(v.as_i16().unwrap_or(0)))),
        VarType::Long => Ok(i32_or_double(-i64::from(v.as_i32().unwrap_or(0)))),
        VarType::LongLong => {
            Ok(i64_or_double(v.as_i64().unwrap_or(0).checked_neg(), || -(v.as_i64().unwrap_or(0) as f64)))
        }
        _ => {
            let d = coerce_to(v, VarType::Double)?;
            Ok(Variant::from_f64(-d.as_f64().unwrap_or(0.0)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::add;
    use crate::Variant;

    #[test]
    fn integer_add_promotes_to_long() {
        let lhs = Variant::from_i16(10);
        let rhs = Variant::from_i16(12);
        let out = add(&lhs, &rhs).expect("add should succeed");
        assert_eq!(out.as_i32(), Some(22));
    }

    #[test]
    fn mixed_add_uses_double() {
        let lhs = Variant::from_i32(10);
        let rhs = Variant::from_f64(0.5);
        let out = add(&lhs, &rhs).expect("add should succeed");
        assert_eq!(out.as_f64(), Some(10.5));
    }
}

#[cfg(test)]
mod proptests {
    use super::add;
    use crate::Variant;
    use crate::variant::VarType;
    use proptest::prelude::*;

    proptest! {
        /// Integer+Integer must promote to Long and produce the correct sum.
        #[test]
        fn prop_integer_add_promotes_to_long(a: i16, b: i16) {
            let lhs = Variant::from_i16(a);
            let rhs = Variant::from_i16(b);
            let result = add(&lhs, &rhs).expect("Integer+Integer should succeed");
            prop_assert_eq!(result.vtype(), VarType::Long);
            prop_assert_eq!(result.as_i32(), Some(a as i32 + b as i32));
        }

        /// Long+Long must produce Long when the sum fits, or Double on overflow.
        #[test]
        fn prop_long_add_overflow_promotes_to_double(a: i32, b: i32) {
            let lhs = Variant::from_i32(a);
            let rhs = Variant::from_i32(b);
            let result = add(&lhs, &rhs).expect("Long+Long should succeed");
            let sum = a as i64 + b as i64;
            if sum > i32::MAX as i64 || sum < i32::MIN as i64 {
                prop_assert_eq!(result.vtype(), VarType::Double,
                    "overflow sum {} should promote to Double", sum);
                prop_assert_eq!(result.as_f64(), Some(sum as f64));
            } else {
                prop_assert_eq!(result.vtype(), VarType::Long,
                    "non-overflow sum {} should stay Long", sum);
                prop_assert_eq!(result.as_i32(), Some(sum as i32));
            }
        }

        /// Integer addition must be commutative.
        #[test]
        fn prop_integer_add_commutative(a: i16, b: i16) {
            let result_ab = add(&Variant::from_i16(a), &Variant::from_i16(b))
                .expect("add should succeed");
            let result_ba = add(&Variant::from_i16(b), &Variant::from_i16(a))
                .expect("add should succeed");
            prop_assert_eq!(result_ab.as_i32(), result_ba.as_i32(),
                "commutativity failed for {} + {}", a, b);
        }

        /// Long addition must be commutative.
        #[test]
        fn prop_long_add_commutative(a: i32, b: i32) {
            let result_ab = add(&Variant::from_i32(a), &Variant::from_i32(b))
                .expect("add should succeed");
            let result_ba = add(&Variant::from_i32(b), &Variant::from_i32(a))
                .expect("add should succeed");
            // Compare data bytes since result could be Long or Double
            prop_assert_eq!(result_ab.vtype(), result_ba.vtype());
            match result_ab.vtype() {
                VarType::Long => prop_assert_eq!(result_ab.as_i32(), result_ba.as_i32()),
                VarType::Double => prop_assert_eq!(result_ab.as_f64(), result_ba.as_f64()),
                _ => prop_assert!(false, "unexpected result type {:?}", result_ab.vtype()),
            }
        }
    }
}
