//! .def file generation for Windows DLL exports.

use oxvba_project::NativeExportDescriptor;

/// Generate a Windows `.def` file for the given exports.
pub fn generate_def_file(project_name: &str, exports: &[NativeExportDescriptor]) -> String {
    let mut def = format!("LIBRARY {project_name}\nEXPORTS\n");

    for export in exports {
        def.push_str("    ");
        def.push_str(&export.exported_name);
        if let Some(ordinal) = export.ordinal {
            def.push_str(&format!(" @{ordinal}"));
        }
        def.push('\n');
    }

    def
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_project::CallingConvention;

    #[test]
    fn def_file_format() {
        let exports = vec![
            NativeExportDescriptor {
                exported_name: "CalcSum".to_string(),
                module_name: "Math".to_string(),
                procedure_name: "Sum".to_string(),
                calling_convention: CallingConvention::Stdcall,
                ordinal: Some(1),
                kind: None,
                param_types: None,
                return_type: None,
                category: None,
                description: None,
                argument_descriptions: None,
            },
            NativeExportDescriptor {
                exported_name: "DoWork".to_string(),
                module_name: "Mod1".to_string(),
                procedure_name: "DoWork".to_string(),
                calling_convention: CallingConvention::Stdcall,
                ordinal: None,
                kind: None,
                param_types: None,
                return_type: None,
                category: None,
                description: None,
                argument_descriptions: None,
            },
        ];

        let def = generate_def_file("MathLib", &exports);
        assert!(def.starts_with("LIBRARY MathLib\nEXPORTS\n"));
        assert!(def.contains("    CalcSum @1\n"));
        assert!(def.contains("    DoWork\n"));
    }
}
