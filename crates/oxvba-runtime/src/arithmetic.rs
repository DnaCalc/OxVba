use crate::{Variant, coerce::coerce_to, variant::VarType};

pub fn add(lhs: &Variant, rhs: &Variant) -> Result<Variant, String> {
    match (lhs.vtype, rhs.vtype) {
        (VarType::Integer, VarType::Integer) => {
            let sum = lhs.as_i16().unwrap_or(0) as i32 + rhs.as_i16().unwrap_or(0) as i32;
            Ok(Variant::from_i32(sum))
        }
        (VarType::Long, VarType::Long) => {
            let sum = lhs.as_i32().unwrap_or(0) as i64 + rhs.as_i32().unwrap_or(0) as i64;
            if sum > i32::MAX as i64 || sum < i32::MIN as i64 {
                Ok(Variant::from_f64(sum as f64))
            } else {
                Ok(Variant::from_i32(sum as i32))
            }
        }
        _ => {
            let l = coerce_to(lhs, VarType::Double)?;
            let r = coerce_to(rhs, VarType::Double)?;
            Ok(Variant::from_f64(
                l.as_f64().unwrap_or(0.0) + r.as_f64().unwrap_or(0.0),
            ))
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
            prop_assert_eq!(result.vtype, VarType::Long);
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
                prop_assert_eq!(result.vtype, VarType::Double,
                    "overflow sum {} should promote to Double", sum);
                prop_assert_eq!(result.as_f64(), Some(sum as f64));
            } else {
                prop_assert_eq!(result.vtype, VarType::Long,
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
            prop_assert_eq!(result_ab.vtype, result_ba.vtype);
            match result_ab.vtype {
                VarType::Long => prop_assert_eq!(result_ab.as_i32(), result_ba.as_i32()),
                VarType::Double => prop_assert_eq!(result_ab.as_f64(), result_ba.as_f64()),
                _ => prop_assert!(false, "unexpected result type {:?}", result_ab.vtype),
            }
        }
    }
}
