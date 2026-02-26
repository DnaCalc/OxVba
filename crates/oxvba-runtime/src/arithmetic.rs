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
