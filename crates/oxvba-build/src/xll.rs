//! XLL entry point and function registration code generation.

use oxvba_project::NativeExportDescriptor;

use crate::xloper;

/// Generate Rust source code for an XLL add-in shim.
pub fn generate_xll_shim(
    project_name: &str,
    oxb_path: &str,
    exports: &[NativeExportDescriptor],
) -> String {
    let mut source = format!(
        r#"//! Auto-generated OxVBA XLL add-in for project "{project_name}".

#![allow(non_snake_case)]

use std::sync::OnceLock;

const BUNDLE_BYTES: &[u8] = include_bytes!("{oxb_path}");

"#
    );

    // xlAutoOpen
    source.push_str(&generate_xl_auto_open(project_name, exports));

    // xlAutoClose
    source.push_str(
        r#"
#[no_mangle]
pub extern "system" fn xlAutoClose() -> i32 {
    // Cleanup: unregister functions
    1
}

"#,
    );

    // xlAutoFree12
    source.push_str(
        r#"#[no_mangle]
pub extern "system" fn xlAutoFree12(p: *mut u8) {
    // Free XLOPER12 memory allocated by the add-in
    if !p.is_null() {
        unsafe { drop(Box::from_raw(p)); }
    }
}

"#,
    );

    source
}

fn generate_xl_auto_open(
    project_name: &str,
    exports: &[NativeExportDescriptor],
) -> String {
    let mut source = String::from(
        r#"#[no_mangle]
pub extern "system" fn xlAutoOpen() -> i32 {
    // Register exported functions with Excel
"#,
    );

    for export in exports {
        let name = &export.exported_name;
        let param_types = export.param_types.as_deref().unwrap_or(&[]);
        let return_type = export.return_type.as_ref().and_then(|r| r.as_ref());
        let type_string = xloper::build_type_string(param_types, return_type);

        source.push_str(&format!(
            "    // Register: {name} type=\"{type_string}\" category=\"{project_name}\"\n"
        ));
    }

    source.push_str("    1\n}\n");
    source
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_compiler::DeclareParamType;
    use oxvba_project::CallingConvention;

    #[test]
    fn xll_shim_has_required_entry_points() {
        let exports = vec![NativeExportDescriptor {
            exported_name: "MyFunc".to_string(),
            module_name: "Mod1".to_string(),
            procedure_name: "MyFunc".to_string(),
            calling_convention: CallingConvention::Stdcall,
            ordinal: None,
            kind: Some(oxvba_compiler::ExportKind::Function),
            param_types: Some(vec![DeclareParamType::Double]),
            return_type: Some(Some(DeclareParamType::Double)),
            category: None,
            description: None,
            argument_descriptions: None,
        }];

        let source = generate_xll_shim("TestAddin", "test.oxb", &exports);
        assert!(source.contains("xlAutoOpen"));
        assert!(source.contains("xlAutoClose"));
        assert!(source.contains("xlAutoFree12"));
        assert!(source.contains("MyFunc"));
        assert!(source.contains("TestAddin"));
    }

    #[test]
    fn xll_registration_type_string() {
        let exports = vec![NativeExportDescriptor {
            exported_name: "Add".to_string(),
            module_name: "Math".to_string(),
            procedure_name: "Add".to_string(),
            calling_convention: CallingConvention::Stdcall,
            ordinal: None,
            kind: Some(oxvba_compiler::ExportKind::Function),
            param_types: Some(vec![DeclareParamType::Double, DeclareParamType::Double]),
            return_type: Some(Some(DeclareParamType::Double)),
            category: None,
            description: None,
            argument_descriptions: None,
        }];

        let source = generate_xll_shim("Math", "math.oxb", &exports);
        assert!(source.contains("type=\"BBB\""));
    }
}
