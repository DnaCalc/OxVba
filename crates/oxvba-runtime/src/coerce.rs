use crate::variant::{VarType, Variant};

pub fn coerce_to(value: &Variant, target: VarType) -> Result<Variant, String> {
    if value.vtype == target {
        return Ok(value.clone());
    }

    match (value.vtype, target) {
        (VarType::Integer, VarType::Long) => {
            Ok(Variant::from_i32(value.as_i16().unwrap_or(0) as i32))
        }
        (VarType::Integer, VarType::Double) => {
            Ok(Variant::from_f64(value.as_i16().unwrap_or(0) as f64))
        }
        (VarType::Long, VarType::Double) => {
            Ok(Variant::from_f64(value.as_i32().unwrap_or(0) as f64))
        }
        (VarType::Boolean, VarType::Long) => {
            let n = if value.as_bool().unwrap_or(false) {
                -1
            } else {
                0
            };
            Ok(Variant::from_i32(n))
        }
        (_, VarType::String) => {
            let text = match value.vtype {
                VarType::Integer => value.as_i16().unwrap_or(0).to_string(),
                VarType::Long => value.as_i32().unwrap_or(0).to_string(),
                VarType::Double => value.as_f64().unwrap_or(0.0).to_string(),
                VarType::Boolean => {
                    if value.as_bool().unwrap_or(false) {
                        "True".to_string()
                    } else {
                        "False".to_string()
                    }
                }
                _ => {
                    return Err(format!(
                        "unsupported coercion to String from {:?}",
                        value.vtype
                    ));
                }
            };
            Variant::from_inline_ascii(&text)
                .ok_or_else(|| "string coercion exceeds inline threshold".to_string())
        }
        _ => Err(format!(
            "unsupported coercion from {:?} to {:?}",
            value.vtype, target
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::coerce_to;
    use crate::{VarType, Variant};

    #[test]
    fn integer_to_long() {
        let input = Variant::from_i16(7);
        let out = coerce_to(&input, VarType::Long).expect("coercion should succeed");
        assert_eq!(out.as_i32(), Some(7));
    }

    #[test]
    fn bool_to_long() {
        let input = Variant::from_bool(true);
        let out = coerce_to(&input, VarType::Long).expect("coercion should succeed");
        assert_eq!(out.as_i32(), Some(-1));
    }
}
