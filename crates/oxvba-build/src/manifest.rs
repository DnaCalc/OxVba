//! Registry-free COM activation manifest (SxS) generation.

use oxvba_project::ComClassExportDescriptor;

/// Generate a Side-by-Side (SxS) activation manifest for registry-free COM.
pub fn generate_sxs_manifest(
    project_name: &str,
    classes: &[ComClassExportDescriptor],
) -> String {
    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="utf-8"?>"#);
    xml.push('\n');
    xml.push_str(r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">"#);
    xml.push('\n');
    xml.push_str(&format!(
        r#"  <assemblyIdentity type="win32" name="{project_name}" version="1.0.0.0" />"#
    ));
    xml.push('\n');

    xml.push_str(&format!(
        r#"  <file name="{project_name}.dll">"#
    ));
    xml.push('\n');

    for class in classes {
        let prog_id = class
            .prog_id
            .as_deref()
            .unwrap_or(&class.class_name);
        let description = class
            .description
            .as_deref()
            .unwrap_or("");

        xml.push_str(&format!(
            r#"    <comClass clsid="{{00000000-0000-0000-0000-000000000000}}" progid="{prog_id}" threadingModel="Apartment" description="{description}" />"#
        ));
        xml.push('\n');
    }

    xml.push_str("  </file>\n");
    xml.push_str("</assembly>\n");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sxs_manifest_structure() {
        let classes = vec![
            oxvba_project::ComClassExportDescriptor {
                class_name: "Widget".to_string(),
                prog_id: Some("TestApp.Widget".to_string()),
                instancing: None,
                description: Some("A widget".to_string()),
                members: vec![],
            },
            oxvba_project::ComClassExportDescriptor {
                class_name: "Factory".to_string(),
                prog_id: None,
                instancing: None,
                description: None,
                members: vec![],
            },
        ];

        let xml = generate_sxs_manifest("TestApp", &classes);
        assert!(xml.contains("<?xml version"));
        assert!(xml.contains("<assembly"));
        assert!(xml.contains("TestApp.dll"));
        assert!(xml.contains("comClass"));
        assert!(xml.contains("TestApp.Widget"));
        assert!(xml.contains("threadingModel=\"Apartment\""));
    }
}
