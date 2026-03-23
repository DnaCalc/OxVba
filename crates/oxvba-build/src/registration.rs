//! COM server registry code generation.

use oxvba_project::ComClassExportDescriptor;

/// Generate Rust source for DllRegisterServer registry writes.
pub fn generate_registration_code(
    project_name: &str,
    dll_name: &str,
    classes: &[ComClassExportDescriptor],
) -> String {
    let mut code = String::new();

    for class in classes {
        let class_name = &class.class_name;
        let default_prog_id = format!("{project_name}.{class_name}");
        let prog_id = class.prog_id.as_deref().unwrap_or(&default_prog_id);
        let description = class.description.as_deref().unwrap_or(class_name);

        code.push_str(&format!(
            r#"// Register {class_name}
// HKCR\CLSID\{{clsid}}\InprocServer32 = {dll_name}
// HKCR\CLSID\{{clsid}}\InprocServer32\ThreadingModel = Apartment
// HKCR\CLSID\{{clsid}}\ProgID = {prog_id}
// HKCR\{prog_id} = {description}
// HKCR\{prog_id}\CLSID = {{clsid}}

"#
        ));
    }

    code
}

/// Generate Rust source for DllUnregisterServer registry cleanup.
pub fn generate_unregistration_code(
    project_name: &str,
    classes: &[ComClassExportDescriptor],
) -> String {
    let mut code = String::new();

    for class in classes {
        let class_name = &class.class_name;
        let default_prog_id = format!("{project_name}.{class_name}");
        let prog_id = class.prog_id.as_deref().unwrap_or(&default_prog_id);

        code.push_str(&format!(
            "// Unregister {class_name}: delete HKCR\\CLSID\\{{clsid}}, HKCR\\{prog_id}\n"
        ));
    }

    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_code_contains_prog_id() {
        let classes = vec![oxvba_project::ComClassExportDescriptor {
            class_name: "Widget".to_string(),
            prog_id: Some("TestApp.Widget".to_string()),
            instancing: None,
            description: Some("A widget".to_string()),
            members: vec![],
        }];

        let code = generate_registration_code("TestApp", "TestApp.dll", &classes);
        assert!(code.contains("TestApp.Widget"));
        assert!(code.contains("ThreadingModel = Apartment"));
    }
}
