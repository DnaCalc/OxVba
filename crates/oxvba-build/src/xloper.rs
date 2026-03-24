//! XLOPER12 FFI types and marshaling for Excel XLL add-ins.

// Excel XLOPER12 type constants
pub const XL_TYPE_NUM: u32 = 0x0001;
pub const XL_TYPE_STR: u32 = 0x0002;
pub const XL_TYPE_BOOL: u32 = 0x0004;
pub const XL_TYPE_ERR: u32 = 0x0010;
pub const XL_TYPE_INT: u32 = 0x0020;
pub const XL_TYPE_MISSING: u32 = 0x0080;
pub const XL_TYPE_NIL: u32 = 0x0100;

/// Map a `DeclareParamType` to an Excel type registration letter.
pub fn declare_param_type_to_excel_letter(ty: &oxvba_compiler::DeclareParamType) -> &'static str {
    use oxvba_compiler::DeclareParamType;
    match ty {
        DeclareParamType::Double => "B",
        DeclareParamType::Single => "E",
        DeclareParamType::Long => "J",
        DeclareParamType::Integer => "I",
        DeclareParamType::Boolean => "A",
        DeclareParamType::String => "C%",
        DeclareParamType::Currency => "J",
        DeclareParamType::Date => "B",
        DeclareParamType::Byte => "I",
        DeclareParamType::LongLong => "J",
        DeclareParamType::LongPtr => "J",
        DeclareParamType::Variant => "Q",
        DeclareParamType::Any => "Q",
    }
}

/// Build the Excel type string for a function registration.
pub fn build_type_string(
    param_types: &[oxvba_compiler::DeclareParamType],
    return_type: Option<&oxvba_compiler::DeclareParamType>,
) -> String {
    let mut s = String::new();
    // Return type first
    if let Some(ret) = return_type {
        s.push_str(declare_param_type_to_excel_letter(ret));
    } else {
        s.push('J'); // Default to Long for void/Sub
    }
    // Parameter types
    for param in param_types {
        s.push_str(declare_param_type_to_excel_letter(param));
    }
    s
}

/// Generate Rust code for RuntimeValue → XLOPER12 marshaling.
pub fn generate_marshal_to_xloper(var_name: &str) -> String {
    format!(
        r#"match {var_name} {{
    RuntimeValue::I32(v) => xloper12_int(v),
    RuntimeValue::F64(v) => xloper12_num(v.as_f64()),
    RuntimeValue::Bool(v) => xloper12_bool(v),
    RuntimeValue::String(s) => xloper12_str(&s.0),
    RuntimeValue::Empty => xloper12_nil(),
    _ => xloper12_nil(),
}}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_compiler::DeclareParamType;

    #[test]
    fn type_string_for_double_function() {
        let s = build_type_string(
            &[DeclareParamType::Double, DeclareParamType::Double],
            Some(&DeclareParamType::Double),
        );
        assert_eq!(s, "BBB");
    }

    #[test]
    fn type_string_for_long_sub() {
        let s = build_type_string(&[DeclareParamType::Long], None);
        assert_eq!(s, "JJ");
    }

    #[test]
    fn type_string_for_string_function() {
        let s = build_type_string(&[DeclareParamType::String], Some(&DeclareParamType::String));
        assert_eq!(s, "C%C%");
    }

    #[test]
    fn excel_letter_mapping() {
        assert_eq!(
            declare_param_type_to_excel_letter(&DeclareParamType::Double),
            "B"
        );
        assert_eq!(
            declare_param_type_to_excel_letter(&DeclareParamType::Long),
            "J"
        );
        assert_eq!(
            declare_param_type_to_excel_letter(&DeclareParamType::Boolean),
            "A"
        );
        assert_eq!(
            declare_param_type_to_excel_letter(&DeclareParamType::String),
            "C%"
        );
    }
}
