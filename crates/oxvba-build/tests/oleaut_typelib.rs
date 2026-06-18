#![cfg(target_os = "windows")]

use std::path::{Path, PathBuf};

use oxvba_build::{
    ComClassDescriptor, ComEventDescriptor, ComImplementedInterfaceDescriptor,
    ComImplementedInterfaceMethodDescriptor, ComInvokeKind, ComMemberDescriptor, ComParamType,
    ComServerDescriptor, ComWireType, compile_typelib, emit_typelib_with_oleaut, generate_idl,
};

#[test]
fn oleaut_typelib_emits_loadable_dispatch_server_shape() {
    let temp = TestDir::new("oleaut_typelib_emits_loadable_dispatch_server_shape");
    let descriptor = dispatch_descriptor();
    let tlb_path = temp.path.join("DemoServer.oleaut.tlb");

    emit_typelib_with_oleaut(&descriptor, &tlb_path).expect("OleAut emitter should write TLB");

    let shape = loaded_shape(&tlb_path);
    assert!(shape.members.iter().any(|member| {
        member.name == "Add"
            && member.token == 1
            && member.invoke_kind == "Method"
            && member.parameter_types == ["Long", "Long"]
            && member.return_type.as_deref() == Some("Long")
    }));
    assert!(shape.members.iter().any(|member| {
        member.name == "Fire"
            && member.token == 2
            && member.invoke_kind == "Method"
            && member.parameter_types == ["Long"]
            && member.return_type.is_none()
    }));
    assert!(shape.members.iter().any(|member| {
        member.name.eq_ignore_ascii_case("Value")
            && member.token == 3
            && member.invoke_kind == "PropertyGet"
            && member.parameter_types.is_empty()
            && member.return_type.as_deref() == Some("Long")
    }));
    assert!(shape.events.iter().any(|event| event.name == "Changed"));
}

#[test]
#[ignore = "requires midl.exe and Windows SDK IDL includes; used as the MIDL oracle while refining OleAut emission"]
fn oleaut_typelib_shape_matches_midl_for_dispatch_server_slice() {
    let temp = TestDir::new("oleaut_typelib_shape_matches_midl_for_dispatch_server_slice");
    let descriptor = dispatch_descriptor();
    let idl_path = temp.path.join("DemoServer.idl");
    let midl_tlb_path = temp.path.join("DemoServer.midl.tlb");
    let oleaut_tlb_path = temp.path.join("DemoServer.oleaut.tlb");

    std::fs::write(&idl_path, generate_idl(&descriptor)).expect("write IDL oracle input");
    compile_typelib(&idl_path, &midl_tlb_path).expect("MIDL should compile oracle TLB");
    emit_typelib_with_oleaut(&descriptor, &oleaut_tlb_path)
        .expect("OleAut emitter should write comparison TLB");

    assert_eq!(loaded_shape(&oleaut_tlb_path), loaded_shape(&midl_tlb_path));
}

#[test]
fn oleaut_typelib_emits_loadable_dual_and_imported_interface_shape() {
    let temp = TestDir::new("oleaut_typelib_emits_loadable_dual_and_imported_interface_shape");
    let descriptor = dual_and_imported_interface_descriptor();
    let tlb_path = temp.path.join("DemoServer.oleaut.tlb");

    emit_typelib_with_oleaut(&descriptor, &tlb_path).expect("OleAut emitter should write TLB");

    let shape = loaded_shape(&tlb_path);
    assert!(shape.members.iter().any(|member| {
        member.name == "Ping"
            && member.token == 1
            && member.invoke_kind == "Method"
            && member.parameter_types.is_empty()
            && member.return_type.as_deref() == Some("Long")
    }));
    assert!(shape.members.iter().any(|member| {
        member.name == "AddPair"
            && member.token == 2
            && member.invoke_kind == "Method"
            && member.parameter_types == ["Long", "Long"]
            && member.return_type.as_deref() == Some("Long")
    }));
    assert!(shape.members.iter().any(|member| {
        member.name == "GetCustomUI"
            && member.token == 1
            && member.invoke_kind == "Method"
            && member.parameter_types == ["String"]
            && member.return_type.as_deref() == Some("String")
    }));
    assert!(shape.members.iter().any(|member| {
        member.name == "ConnectData"
            && member.parameter_wire_types
                == [
                    "Automation(Long)",
                    "SafeArrayVariant",
                    "Automation(ByRefBoolean)",
                ]
            && member.return_wire_type.as_deref() == Some("Automation(Variant)")
    }));
    assert!(shape.members.iter().any(|member| {
        member.name == "RefreshData"
            && member.parameter_wire_types == ["Automation(ByRefLong)"]
            && member.return_wire_type.as_deref() == Some("ByRefSafeArrayVariant")
    }));
}

#[test]
#[ignore = "requires midl.exe and Windows SDK IDL includes; used as the MIDL oracle while refining OleAut emission"]
fn oleaut_typelib_shape_matches_midl_for_dual_and_imported_interface_slice() {
    let temp =
        TestDir::new("oleaut_typelib_shape_matches_midl_for_dual_and_imported_interface_slice");
    let descriptor = dual_and_imported_interface_descriptor();
    let idl_path = temp.path.join("DemoServer.idl");
    let midl_tlb_path = temp.path.join("DemoServer.midl.tlb");
    let oleaut_tlb_path = temp.path.join("DemoServer.oleaut.tlb");

    std::fs::write(&idl_path, generate_idl(&descriptor)).expect("write IDL oracle input");
    compile_typelib(&idl_path, &midl_tlb_path).expect("MIDL should compile oracle TLB");
    emit_typelib_with_oleaut(&descriptor, &oleaut_tlb_path)
        .expect("OleAut emitter should write comparison TLB");

    assert_eq!(loaded_shape(&oleaut_tlb_path), loaded_shape(&midl_tlb_path));
}

fn dispatch_descriptor() -> ComServerDescriptor {
    ComServerDescriptor {
        project_name: "DemoServer".to_string(),
        libid: "11111111-2222-3333-4444-555555555555".to_string(),
        version_major: 1,
        version_minor: 0,
        classes: vec![ComClassDescriptor {
            class_name: "Calculator".to_string(),
            description: Some("Calculator class".to_string()),
            prog_id: "DemoServer.Calculator".to_string(),
            creatable: true,
            clsid: "11111111-2222-3333-4444-555555555556".to_string(),
            default_interface_name: "ICalculator".to_string(),
            default_interface_iid: "11111111-2222-3333-4444-555555555557".to_string(),
            source_interface_name: Some("_CalculatorEvents".to_string()),
            source_interface_iid: Some("11111111-2222-3333-4444-555555555558".to_string()),
            implemented_interfaces: Vec::new(),
            members: vec![
                ComMemberDescriptor {
                    name: "Add".to_string(),
                    dispid: 1,
                    vtable_slot: None,
                    invoke_kind: ComInvokeKind::Method,
                    is_default: false,
                    parameter_names: vec!["a".to_string(), "b".to_string()],
                    parameter_types: vec![ComParamType::Long, ComParamType::Long],
                    parameter_optional: vec![false, false],
                    return_type: Some(ComParamType::Long),
                },
                ComMemberDescriptor {
                    name: "Fire".to_string(),
                    dispid: 2,
                    vtable_slot: None,
                    invoke_kind: ComInvokeKind::Method,
                    is_default: false,
                    parameter_names: vec!["value".to_string()],
                    parameter_types: vec![ComParamType::Long],
                    parameter_optional: vec![false],
                    return_type: None,
                },
                ComMemberDescriptor {
                    name: "Value".to_string(),
                    dispid: 3,
                    vtable_slot: None,
                    invoke_kind: ComInvokeKind::PropertyGet,
                    is_default: false,
                    parameter_names: Vec::new(),
                    parameter_types: Vec::new(),
                    parameter_optional: Vec::new(),
                    return_type: Some(ComParamType::Long),
                },
            ],
            events: vec![ComEventDescriptor {
                name: "Changed".to_string(),
                dispid: 10,
                callback_arity: 1,
            }],
        }],
    }
}

fn dual_and_imported_interface_descriptor() -> ComServerDescriptor {
    ComServerDescriptor {
        project_name: "DemoServer".to_string(),
        libid: "21111111-2222-3333-4444-555555555555".to_string(),
        version_major: 1,
        version_minor: 0,
        classes: vec![
            ComClassDescriptor {
                class_name: "Pinger".to_string(),
                description: Some("Pinger class".to_string()),
                prog_id: "DemoServer.Pinger".to_string(),
                creatable: true,
                clsid: "21111111-2222-3333-4444-555555555556".to_string(),
                default_interface_name: "IPinger".to_string(),
                default_interface_iid: "21111111-2222-3333-4444-555555555557".to_string(),
                source_interface_name: None,
                source_interface_iid: None,
                implemented_interfaces: Vec::new(),
                members: vec![
                    ComMemberDescriptor {
                        name: "Ping".to_string(),
                        dispid: 1,
                        vtable_slot: Some(7),
                        invoke_kind: ComInvokeKind::Method,
                        is_default: false,
                        parameter_names: Vec::new(),
                        parameter_types: Vec::new(),
                        parameter_optional: Vec::new(),
                        return_type: Some(ComParamType::Long),
                    },
                    ComMemberDescriptor {
                        name: "AddPair".to_string(),
                        dispid: 2,
                        vtable_slot: Some(8),
                        invoke_kind: ComInvokeKind::Method,
                        is_default: false,
                        parameter_names: vec!["a".to_string(), "b".to_string()],
                        parameter_types: vec![ComParamType::Long, ComParamType::Long],
                        parameter_optional: vec![false, false],
                        return_type: Some(ComParamType::Long),
                    },
                ],
                events: Vec::new(),
            },
            ComClassDescriptor {
                class_name: "RibbonAddin".to_string(),
                description: Some("Ribbon add-in class".to_string()),
                prog_id: "DemoServer.RibbonAddin".to_string(),
                creatable: true,
                clsid: "21111111-2222-3333-4444-555555555558".to_string(),
                default_interface_name: "IRibbonAddin".to_string(),
                default_interface_iid: "21111111-2222-3333-4444-555555555559".to_string(),
                source_interface_name: None,
                source_interface_iid: None,
                implemented_interfaces: vec![
                    ComImplementedInterfaceDescriptor {
                        name: "IRibbonExtensibility".to_string(),
                        iid: "000C0396-0000-0000-C000-000000000046".to_string(),
                        methods: vec![ComImplementedInterfaceMethodDescriptor {
                            name: "GetCustomUI".to_string(),
                            vba_name: "IRibbonExtensibility_GetCustomUI".to_string(),
                            dispid: 1,
                            vtable_slot: Some(7),
                            invoke_kind: ComInvokeKind::Method,
                            parameter_names: vec!["RibbonID".to_string()],
                            parameter_types: vec![ComParamType::String],
                            parameter_wire_types: vec![ComWireType::Automation(
                                ComParamType::String,
                            )],
                            parameter_optional: vec![false],
                            return_type: Some(ComParamType::String),
                            return_wire_type: Some(ComWireType::Automation(ComParamType::String)),
                        }],
                    },
                    ComImplementedInterfaceDescriptor {
                        name: "IRtdServer".to_string(),
                        iid: "EC0E6191-DB51-11D3-8F3E-00C04F3651B8".to_string(),
                        methods: vec![
                            ComImplementedInterfaceMethodDescriptor {
                                name: "ServerStart".to_string(),
                                vba_name: "IRtdServer_ServerStart".to_string(),
                                dispid: 10,
                                vtable_slot: Some(7),
                                invoke_kind: ComInvokeKind::Method,
                                parameter_names: vec!["CallbackObject".to_string()],
                                parameter_types: vec![ComParamType::Object],
                                parameter_wire_types: vec![ComWireType::InterfacePointer {
                                    name: "IRTDUpdateEvent".to_string(),
                                    iid: "A43788C1-D91B-11D3-8F39-00C04F3651B8".to_string(),
                                }],
                                parameter_optional: vec![false],
                                return_type: Some(ComParamType::Long),
                                return_wire_type: Some(ComWireType::Automation(ComParamType::Long)),
                            },
                            ComImplementedInterfaceMethodDescriptor {
                                name: "ConnectData".to_string(),
                                vba_name: "IRtdServer_ConnectData".to_string(),
                                dispid: 11,
                                vtable_slot: Some(8),
                                invoke_kind: ComInvokeKind::Method,
                                parameter_names: vec![
                                    "TopicID".to_string(),
                                    "Strings".to_string(),
                                    "GetNewValues".to_string(),
                                ],
                                parameter_types: vec![
                                    ComParamType::Long,
                                    ComParamType::Variant,
                                    ComParamType::ByRefBoolean,
                                ],
                                parameter_wire_types: vec![
                                    ComWireType::Automation(ComParamType::Long),
                                    ComWireType::SafeArrayVariant,
                                    ComWireType::Automation(ComParamType::ByRefBoolean),
                                ],
                                parameter_optional: vec![false, false, false],
                                return_type: Some(ComParamType::Variant),
                                return_wire_type: Some(ComWireType::Automation(
                                    ComParamType::Variant,
                                )),
                            },
                            ComImplementedInterfaceMethodDescriptor {
                                name: "RefreshData".to_string(),
                                vba_name: "IRtdServer_RefreshData".to_string(),
                                dispid: 12,
                                vtable_slot: Some(9),
                                invoke_kind: ComInvokeKind::Method,
                                parameter_names: vec!["TopicCount".to_string()],
                                parameter_types: vec![ComParamType::ByRefLong],
                                parameter_wire_types: vec![ComWireType::Automation(
                                    ComParamType::ByRefLong,
                                )],
                                parameter_optional: vec![false],
                                return_type: Some(ComParamType::Variant),
                                return_wire_type: Some(ComWireType::ByRefSafeArrayVariant),
                            },
                        ],
                    },
                ],
                members: Vec::new(),
                events: Vec::new(),
            },
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadedShape {
    members: Vec<LoadedMember>,
    events: Vec<LoadedEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct LoadedMember {
    name: String,
    token: i32,
    invoke_kind: String,
    parameter_types: Vec<String>,
    parameter_wire_types: Vec<String>,
    return_type: Option<String>,
    return_wire_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct LoadedEvent {
    name: String,
    token: i32,
    callback_arity: u8,
}

fn loaded_shape(path: &Path) -> LoadedShape {
    let path_text = path.display().to_string();
    let ptlib =
        oxvba_com::windows_typelib_loader::load_typelib_from_path(&path_text).expect("load TLB");
    let result = {
        let mut members = oxvba_com::windows_typelib_loader::enumerate_typelib_members(ptlib)
            .expect("enumerate members")
            .into_iter()
            .filter(|member| {
                !matches!(
                    member.name.as_str(),
                    "QueryInterface"
                        | "AddRef"
                        | "Release"
                        | "GetTypeInfoCount"
                        | "GetTypeInfo"
                        | "GetIDsOfNames"
                        | "Invoke"
                )
            })
            .map(|member| LoadedMember {
                name: member.name,
                token: member.token,
                invoke_kind: format!("{:?}", member.invoke_kind),
                parameter_types: member
                    .parameter_types
                    .into_iter()
                    .map(|param| format!("{param:?}"))
                    .collect(),
                parameter_wire_types: member
                    .parameter_wire_types
                    .into_iter()
                    .map(|param| format!("{param:?}"))
                    .collect(),
                return_type: member.return_type.map(|param| format!("{param:?}")),
                return_wire_type: member.return_wire_type.map(|param| format!("{param:?}")),
            })
            .collect::<Vec<_>>();
        members.sort();
        members.dedup();
        let mut events = oxvba_com::windows_typelib_loader::enumerate_typelib_events(ptlib)
            .expect("enumerate events")
            .into_iter()
            .map(|event| LoadedEvent {
                name: event.name,
                token: event.token,
                callback_arity: event.callback_arity,
            })
            .collect::<Vec<_>>();
        events.sort();
        events.dedup();
        LoadedShape { members, events }
    };
    // SAFETY: ptlib was returned by load_typelib_from_path above and is released exactly once here.
    unsafe { oxvba_com::windows_typelib_loader::release_typelib(ptlib) };
    result
}

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let unique = format!(
            "oxvba_build_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path).expect("create test dir");
        Self { path }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
