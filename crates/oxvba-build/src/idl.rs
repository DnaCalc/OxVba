//! IDL (Interface Definition Language) source generation.

use oxvba_project::{ComClassExportDescriptor, DispatchMemberInfo};

/// Generate IDL source for MIDL compilation to produce a .tlb type library.
pub fn generate_idl(project_name: &str, classes: &[ComClassExportDescriptor]) -> String {
    let mut idl = String::new();

    let lib_uuid = deterministic_uuid(project_name, "__typelib__");
    idl.push_str(&format!(
        r#"// Auto-generated IDL for {project_name}
// Compile with: midl /tlb {project_name}.tlb {project_name}.idl

import "oaidl.idl";
import "ocidl.idl";

[
    uuid({lib_uuid}),
    version(1.0),
    helpstring("{project_name} Type Library")
]
library {project_name}Lib
{{
    importlib("stdole2.tlb");

"#
    ));

    for class in classes {
        idl.push_str(&generate_class_idl(project_name, class));
    }

    idl.push_str("};\n");
    idl
}

fn generate_class_idl(project_name: &str, class: &ComClassExportDescriptor) -> String {
    let class_name = &class.class_name;
    let description = class.description.as_deref().unwrap_or(class_name);

    let iface_uuid = deterministic_uuid(project_name, &format!("I{class_name}"));
    let coclass_uuid = deterministic_uuid(project_name, class_name);

    let mut idl = String::new();

    // Dispatch interface
    idl.push_str(&format!(
        r#"    [
        uuid({iface_uuid}),
        dual,
        oleautomation,
        helpstring("{description} Interface")
    ]
    interface I{class_name} : IDispatch
    {{
"#
    ));

    for (i, member) in class.members.iter().enumerate() {
        let dispid = i + 1;
        idl.push_str(&generate_member_idl(member, dispid));
    }

    idl.push_str("    };\n\n");

    // Coclass
    idl.push_str(&format!(
        r#"    [
        uuid({coclass_uuid}),
        helpstring("{description}")
    ]
    coclass {class_name}
    {{
        [default] interface I{class_name};
    }};

"#
    ));

    idl
}

fn generate_member_idl(member: &DispatchMemberInfo, dispid: usize) -> String {
    let name = &member.member_name;
    let kind = match member.kind {
        oxvba_compiler::ProjectDynamicMemberKind::PropertyGet => "propget",
        oxvba_compiler::ProjectDynamicMemberKind::PropertyLet
        | oxvba_compiler::ProjectDynamicMemberKind::PropertySet => "propput",
        _ => "",
    };

    let is_function = matches!(
        member.kind,
        oxvba_compiler::ProjectDynamicMemberKind::Function
            | oxvba_compiler::ProjectDynamicMemberKind::PropertyGet
    );

    let params: Vec<String> = (0..member.param_count)
        .map(|i| format!("[in] VARIANT arg{i}"))
        .collect();

    let attr = if kind.is_empty() {
        format!("[id({dispid})]")
    } else {
        format!("[id({dispid}), {kind}]")
    };

    if is_function {
        format!(
            "        {attr} HRESULT {name}({params}[out, retval] VARIANT* pRetVal);\n",
            params = if params.is_empty() {
                String::new()
            } else {
                format!("{}, ", params.join(", "))
            },
        )
    } else {
        format!("        {attr} HRESULT {name}({});\n", params.join(", "))
    }
}

/// Generate a deterministic UUID v5-style identifier from project + component names.
///
/// Uses a simple FNV-1a-style hash to produce a reproducible 128-bit value,
/// formatted as a UUID with the version nibble set to 5 and variant bits to RFC 4122.
pub fn deterministic_uuid(namespace: &str, name: &str) -> String {
    // OxVBA namespace seed (arbitrary fixed value for deterministic generation)
    const NAMESPACE_SEED: u128 = 0x4f58_5642_4100_0000_0000_0000_0000_0001; // "OXVBA..."

    let input = format!("{namespace}\0{name}");
    let mut hash: u128 = NAMESPACE_SEED;

    // FNV-1a-inspired mixing for 128-bit output
    for byte in input.bytes() {
        hash ^= byte as u128;
        hash = hash.wrapping_mul(0x0100_0000_01b3_0000_0000_0000_0000_001d);
    }

    // Set version 5 (bits 48-51) and variant RFC 4122 (bits 64-65)
    let mut bytes = hash.to_be_bytes();
    bytes[6] = (bytes[6] & 0x0F) | 0x50; // version 5
    bytes[8] = (bytes[8] & 0x3F) | 0x80; // variant RFC 4122

    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u16::from_be_bytes([bytes[4], bytes[5]]),
        u16::from_be_bytes([bytes[6], bytes[7]]),
        u16::from_be_bytes([bytes[8], bytes[9]]),
        u64::from_be_bytes([
            0, 0, bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
        ]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idl_output_structure() {
        let classes = vec![ComClassExportDescriptor {
            class_name: "Calculator".to_string(),
            prog_id: Some("TestApp.Calculator".to_string()),
            instancing: None,
            description: Some("A calculator".to_string()),
            members: vec![
                DispatchMemberInfo {
                    member_name: "Add".to_string(),
                    kind: oxvba_compiler::ProjectDynamicMemberKind::Function,
                    param_count: 2,
                },
                DispatchMemberInfo {
                    member_name: "Clear".to_string(),
                    kind: oxvba_compiler::ProjectDynamicMemberKind::Method,
                    param_count: 0,
                },
            ],
        }];

        let idl = generate_idl("TestApp", &classes);
        assert!(idl.contains("library TestAppLib"));
        assert!(idl.contains("interface ICalculator : IDispatch"));
        assert!(idl.contains("coclass Calculator"));
        assert!(idl.contains("HRESULT Add"));
        assert!(idl.contains("[out, retval] VARIANT* pRetVal"));
        assert!(idl.contains("HRESULT Clear()"));
    }

    #[test]
    fn idl_uuids_are_deterministic_and_not_zero() {
        let classes = vec![ComClassExportDescriptor {
            class_name: "Foo".to_string(),
            prog_id: None,
            instancing: None,
            description: None,
            members: vec![],
        }];

        let idl = generate_idl("MyProject", &classes);
        assert!(!idl.contains("00000000-0000-0000-0000-000000000000"));

        // Same input produces same output
        let idl2 = generate_idl("MyProject", &classes);
        assert_eq!(idl, idl2);
    }

    #[test]
    fn deterministic_uuid_format() {
        let uuid = deterministic_uuid("Test", "Component");
        // 8-4-4-4-12
        assert_eq!(uuid.len(), 36);
        assert_eq!(&uuid[8..9], "-");
        assert_eq!(&uuid[13..14], "-");
        assert_eq!(&uuid[18..19], "-");
        assert_eq!(&uuid[23..24], "-");
        // Version nibble should be 5
        assert_eq!(&uuid[14..15], "5");
    }
}
