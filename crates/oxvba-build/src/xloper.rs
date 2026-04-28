//! XLOPER12 FFI types and marshaling for Excel XLL add-ins.

// Excel XLOPER12 type constants
pub const XL_TYPE_NUM: u32 = 0x0001;
pub const XL_TYPE_STR: u32 = 0x0002;
pub const XL_TYPE_BOOL: u32 = 0x0004;
pub const XL_TYPE_REF: u32 = 0x0008;
pub const XL_TYPE_ERR: u32 = 0x0010;
pub const XL_TYPE_FLOW: u32 = 0x0020;
pub const XL_TYPE_MULTI: u32 = 0x0040;
pub const XL_TYPE_MISSING: u32 = 0x0080;
pub const XL_TYPE_NIL: u32 = 0x0100;
pub const XL_TYPE_SREF: u32 = 0x0400;
pub const XL_TYPE_INT: u32 = 0x0800;
pub const XL_BIT_XL_FREE: u32 = 0x1000;
pub const XL_BIT_DLL_FREE: u32 = 0x4000;

/// Map the generated XLL wrapper ABI to an Excel type registration letter.
///
/// The current generated exports receive arguments as `LPXLOPER12` and return
/// an owned `LPXLOPER12`, so every registered argument and return uses `Q`
/// regardless of the VBA scalar type carried inside the XLOPER12 value.
pub fn declare_param_type_to_excel_letter(_ty: &oxvba_compiler::DeclareParamType) -> &'static str {
    "Q"
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
        s.push('Q');
    }
    // Parameter types
    for param in param_types {
        s.push_str(declare_param_type_to_excel_letter(param));
    }
    s
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
        assert_eq!(s, "QQQ");
    }

    #[test]
    fn type_string_for_long_sub() {
        let s = build_type_string(&[DeclareParamType::Long], None);
        assert_eq!(s, "QQ");
    }

    #[test]
    fn type_string_for_string_function() {
        let s = build_type_string(&[DeclareParamType::String], Some(&DeclareParamType::String));
        assert_eq!(s, "QQ");
    }

    #[test]
    fn excel_letter_mapping() {
        assert_eq!(
            declare_param_type_to_excel_letter(&DeclareParamType::Double),
            "Q"
        );
        assert_eq!(
            declare_param_type_to_excel_letter(&DeclareParamType::Long),
            "Q"
        );
        assert_eq!(
            declare_param_type_to_excel_letter(&DeclareParamType::Boolean),
            "Q"
        );
        assert_eq!(
            declare_param_type_to_excel_letter(&DeclareParamType::String),
            "Q"
        );
    }
}
