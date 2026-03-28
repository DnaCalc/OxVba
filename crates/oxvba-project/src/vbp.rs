//! VB6 `.vbp` project file parser and converter to `.basproj` XML.

use std::path::Path;

use crate::{BasProjError, LoadedProject, load_basproj_from_str};

/// Parsed VBP project file.
#[derive(Debug, Clone)]
pub struct VbpProject {
    pub project_type: String,
    pub project_name: String,
    pub startup: Option<String>,
    pub modules: Vec<VbpModule>,
    pub classes: Vec<VbpClass>,
    pub references: Vec<VbpReference>,
}

#[derive(Debug, Clone)]
pub struct VbpModule {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct VbpClass {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct VbpReference {
    pub guid: String,
    pub version: String,
    pub name: String,
}

/// Parse a VB6 `.vbp` file content into a `VbpProject`.
pub fn parse_vbp(content: &str) -> Result<VbpProject, String> {
    let mut project_type = "Exe".to_string();
    let mut project_name = String::new();
    let mut startup = None;
    let mut modules = Vec::new();
    let mut classes = Vec::new();
    let mut references = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('[') {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            match key {
                "Type" => {
                    project_type = match value {
                        "Exe" => "Exe".to_string(),
                        "OleDll" => "Library".to_string(),
                        "OleExe" => "ComServer".to_string(),
                        "Control" => "Library".to_string(),
                        _ => value.to_string(),
                    };
                }
                "Name" => {
                    // Strip quotes
                    project_name = value.trim_matches('"').to_string();
                }
                "Startup" => {
                    let v = value.trim_matches('"');
                    if !v.is_empty() && v != "(None)" {
                        startup = Some(v.to_string());
                    }
                }
                "Module" => {
                    // Format: Module=ModuleName; path\to\file.bas
                    if let Some((name, path)) = value.split_once(';') {
                        modules.push(VbpModule {
                            name: name.trim().to_string(),
                            path: path.trim().to_string(),
                        });
                    }
                }
                "Class" => {
                    // Format: Class=ClassName; path\to\file.cls
                    if let Some((name, path)) = value.split_once(';') {
                        classes.push(VbpClass {
                            name: name.trim().to_string(),
                            path: path.trim().to_string(),
                        });
                    }
                }
                "Reference" => {
                    // Format: Reference=*\G{GUID}#major.minor#lcid#path#name
                    if let Some(ref_data) = parse_vbp_reference(value) {
                        references.push(ref_data);
                    }
                }
                "Form" => {
                    return Err(format!(
                        "VBP-E-UNSUPPORTED-FORM: `{line}` is not supported in VBP-S0"
                    ));
                }
                "UserControl" => {
                    return Err(format!(
                        "VBP-E-UNSUPPORTED-USERCONTROL: `{line}` is not supported in VBP-S0"
                    ));
                }
                "PropertyPage" => {
                    return Err(format!(
                        "VBP-E-UNSUPPORTED-PROPERTYPAGE: `{line}` is not supported in VBP-S0"
                    ));
                }
                _ => {} // Ignore other keys for now.
            }
        }
    }

    if project_name.is_empty() {
        return Err("VBP file missing Name property".to_string());
    }

    Ok(VbpProject {
        project_type,
        project_name,
        startup,
        modules,
        classes,
        references,
    })
}

fn parse_vbp_reference(value: &str) -> Option<VbpReference> {
    // Format: *\G{GUID}#major.minor#lcid#path#name
    let value = value.strip_prefix("*\\G")?;
    let parts: Vec<&str> = value.splitn(5, '#').collect();
    if parts.len() < 5 {
        return None;
    }
    Some(VbpReference {
        guid: parts[0].to_string(),
        version: parts[1].to_string(),
        name: parts[4].to_string(),
    })
}

/// Generate `.basproj` XML from a parsed `VbpProject`.
pub fn generate_basproj_from_vbp(vbp: &VbpProject) -> Result<String, String> {
    let mut xml = String::new();
    xml.push_str("<Project Sdk=\"OxVba.Sdk/0.1.0\">\n");

    xml.push_str("  <PropertyGroup>\n");
    xml.push_str(&format!(
        "    <OutputType>{}</OutputType>\n",
        vbp.project_type
    ));
    xml.push_str(&format!(
        "    <ProjectName>{}</ProjectName>\n",
        xml_escape(&vbp.project_name)
    ));
    if let Some(startup) = normalize_vbp_startup_entry_point(vbp.startup.as_deref())? {
        xml.push_str(&format!(
            "    <EntryPoint>{}</EntryPoint>\n",
            xml_escape(startup)
        ));
    }
    xml.push_str("  </PropertyGroup>\n");

    if !vbp.modules.is_empty() || !vbp.classes.is_empty() {
        xml.push_str("  <ItemGroup>\n");
        for m in &vbp.modules {
            // Convert backslashes to forward slashes
            let path = m.path.replace('\\', "/");
            xml.push_str(&format!(
                "    <Module Include=\"{}\" />\n",
                xml_escape(&path)
            ));
        }
        for c in &vbp.classes {
            let path = c.path.replace('\\', "/");
            xml.push_str(&format!(
                "    <ClassModule Include=\"{}\" />\n",
                xml_escape(&path)
            ));
        }
        xml.push_str("  </ItemGroup>\n");
    }

    if !vbp.references.is_empty() {
        xml.push_str("  <ItemGroup>\n");
        for r in &vbp.references {
            xml.push_str(&format!(
                "    <COMReference Include=\"{}\">\n",
                xml_escape(&r.name)
            ));
            xml.push_str(&format!("      <Guid>{}</Guid>\n", xml_escape(&r.guid)));
            if let Some((major, minor)) = r.version.split_once('.') {
                xml.push_str(&format!("      <VersionMajor>{major}</VersionMajor>\n"));
                xml.push_str(&format!("      <VersionMinor>{minor}</VersionMinor>\n"));
            }
            xml.push_str("    </COMReference>\n");
        }
        xml.push_str("  </ItemGroup>\n");
    }

    xml.push_str("</Project>\n");
    Ok(xml)
}

/// Load a `.vbp` file from disk through the VBP-S0 adapter and produce a `LoadedProject`.
pub fn load_vbp(path: &Path) -> Result<LoadedProject, BasProjError> {
    let content = std::fs::read_to_string(path).map_err(|source| BasProjError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let project_dir = path.parent().unwrap_or_else(|| Path::new("."));
    load_vbp_from_str(&content, project_dir)
}

/// Load a `.vbp` string through the VBP-S0 adapter and produce a `LoadedProject`.
pub fn load_vbp_from_str(content: &str, project_dir: &Path) -> Result<LoadedProject, BasProjError> {
    let vbp = parse_vbp(content).map_err(BasProjError::VbpParse)?;
    let xml = generate_basproj_from_vbp(&vbp).map_err(BasProjError::VbpUnsupported)?;
    load_basproj_from_str(&xml, project_dir)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn normalize_vbp_startup_entry_point(startup: Option<&str>) -> Result<Option<&str>, String> {
    let Some(startup) = startup else {
        return Ok(None);
    };
    let startup = startup.trim();
    if startup.is_empty() || startup.eq_ignore_ascii_case("(None)") {
        return Ok(None);
    }
    if startup.eq_ignore_ascii_case("Sub Main") {
        return Ok(None);
    }
    let Some((module_name, procedure_name)) = startup.split_once('.') else {
        return Err(format!(
            "unsupported Startup value `{startup}`; VBP-S0 supports only `Sub Main` or `Module.Procedure`"
        ));
    };
    if !is_valid_identifier(module_name.trim()) || !is_valid_identifier(procedure_name.trim()) {
        return Err(format!(
            "unsupported Startup value `{startup}`; VBP-S0 supports only `Sub Main` or `Module.Procedure`"
        ));
    }
    Ok(Some(startup))
}

fn is_valid_identifier(identifier: &str) -> bool {
    let mut chars = identifier.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_VBP: &str = r#"Type=Exe
Reference=*\G{00020430-0000-0000-C000-000000000046}#2.0#0#C:\WINDOWS\system32\stdole2.tlb#OLE Automation
Reference=*\G{420B2830-E718-11CF-893D-00A0C9054228}#1.0#0#C:\WINDOWS\system32\scrrun.dll#Microsoft Scripting Runtime
Module=Module1; Module1.bas
Module=Utils; src\Utils.bas
Class=Calculator; Calculator.cls
Name="TestProject"
Startup="Sub Main"
"#;

    #[test]
    fn parse_vbp_basic() {
        let vbp = parse_vbp(SAMPLE_VBP).unwrap();
        assert_eq!(vbp.project_name, "TestProject");
        assert_eq!(vbp.project_type, "Exe");
        assert_eq!(vbp.startup.as_deref(), Some("Sub Main"));
        assert_eq!(vbp.modules.len(), 2);
        assert_eq!(vbp.modules[0].name, "Module1");
        assert_eq!(vbp.modules[0].path, "Module1.bas");
        assert_eq!(vbp.modules[1].name, "Utils");
        assert_eq!(vbp.modules[1].path, "src\\Utils.bas");
        assert_eq!(vbp.classes.len(), 1);
        assert_eq!(vbp.classes[0].name, "Calculator");
        assert_eq!(vbp.references.len(), 2);
        assert_eq!(vbp.references[0].name, "OLE Automation");
        assert_eq!(vbp.references[1].name, "Microsoft Scripting Runtime");
    }

    #[test]
    fn generate_basproj_from_vbp_produces_valid_xml() {
        let vbp = parse_vbp(SAMPLE_VBP).unwrap();
        let xml = generate_basproj_from_vbp(&vbp).expect("valid vbp should generate xml");

        assert!(xml.contains("<OutputType>Exe</OutputType>"));
        assert!(xml.contains("<ProjectName>TestProject</ProjectName>"));
        assert!(!xml.contains("<EntryPoint>Sub Main</EntryPoint>"));
        assert!(xml.contains("<Module Include=\"Module1.bas\""));
        assert!(xml.contains("<Module Include=\"src/Utils.bas\""));
        assert!(xml.contains("<ClassModule Include=\"Calculator.cls\""));
        assert!(xml.contains("<COMReference Include=\"OLE Automation\">"));
    }

    #[test]
    fn generate_basproj_from_vbp_preserves_explicit_module_procedure_startup() {
        let vbp = VbpProject {
            project_type: "Exe".to_string(),
            project_name: "TestProject".to_string(),
            startup: Some("Module1.Main".to_string()),
            modules: vec![VbpModule {
                name: "Module1".to_string(),
                path: "Module1.bas".to_string(),
            }],
            classes: Vec::new(),
            references: Vec::new(),
        };

        let xml = generate_basproj_from_vbp(&vbp).expect("valid vbp should generate xml");
        assert!(xml.contains("<EntryPoint>Module1.Main</EntryPoint>"));
    }

    #[test]
    fn generate_basproj_from_vbp_rejects_unsupported_startup_object() {
        let vbp = VbpProject {
            project_type: "Exe".to_string(),
            project_name: "TestProject".to_string(),
            startup: Some("Form1".to_string()),
            modules: vec![VbpModule {
                name: "Module1".to_string(),
                path: "Module1.bas".to_string(),
            }],
            classes: Vec::new(),
            references: Vec::new(),
        };

        let err = generate_basproj_from_vbp(&vbp)
            .expect_err("startup object should be rejected in VBP-S0");
        assert!(err.contains("unsupported Startup value"), "got: {err}");
    }

    #[test]
    fn parse_vbp_missing_name_is_error() {
        let content = "Type=Exe\nModule=Mod1; Mod1.bas\n";
        let result = parse_vbp(content);
        assert!(result.is_err());
    }

    #[test]
    fn parse_vbp_dll_type() {
        let content = "Type=OleDll\nName=\"MyLib\"\n";
        let vbp = parse_vbp(content).unwrap();
        assert_eq!(vbp.project_type, "Library");
    }

    #[test]
    fn parse_vbp_rejects_unsupported_form_key() {
        let content = "Type=Exe\nName=\"MyApp\"\nForm=frmMain; frmMain.frm\n";
        let err = parse_vbp(content).expect_err("Form should be rejected in VBP-S0");
        assert!(err.contains("VBP-E-UNSUPPORTED-FORM"), "got: {err}");
    }
}
