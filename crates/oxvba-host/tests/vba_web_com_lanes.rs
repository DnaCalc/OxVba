//! VBA-Web-shaped runtime/host/COM progression lanes.
//!
//! These tests are intentionally smaller than the COM matrix. They prove the corpus
//! harness can graduate from pure helpers into policy-owned COM object-model use
//! without requiring live COM in default CI.

use std::sync::{Arc, Mutex};

use oxvba_com::{
    OptionalParamDefault, PortableComProjection, SourceTypeKind, TypeLibMemberInvokeKind,
    TypeLibMetadataBlob, TypeLibParamType, TypeLibResolveRequest, TypeLibResolvedIdentity,
    TypeLibWireType,
    platform::portable::{PortableDispatch, PortableObjectFactory, PortableObjectResult},
};
use oxvba_hal::model::HostPolicy;
use oxvba_hal::{adapters::builder::HostBuilder, model::native_host_profile};
use oxvba_host::{Engine, HostConfig, HostProfileProvider};
use oxvba_runtime::Variant;
use oxvba_symbol::TypeLibResolver;
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectReference, SymbolProjectManifest,
};

fn run_source_with_policy(
    source: &str,
    policy: HostPolicy,
) -> Result<Vec<oxvba_runtime::Variant>, String> {
    let mut engine = Engine::new(HostConfig::vm3());
    engine.set_host_policy(policy);
    engine
        .execute_source_with_variant_snapshot_clean(source)
        .map_err(|d| format!("{:?}: {}", d.phase(), d.message()))
}

fn first_i32(values: &[oxvba_runtime::Variant]) -> Option<i32> {
    values.iter().find_map(|value| value.as_i32())
}

fn app_member(
    name: &str,
    token: i32,
    parameter_names: Vec<&str>,
    parameter_optional: Vec<bool>,
    parameter_types: Vec<TypeLibParamType>,
    return_type: Option<TypeLibParamType>,
) -> oxvba_com::TypeLibMemberMetadata {
    let parameter_optional_defaults = parameter_optional
        .iter()
        .map(|optional| {
            if *optional {
                OptionalParamDefault::OptionalVariant
            } else {
                OptionalParamDefault::Required
            }
        })
        .collect::<Vec<_>>();
    oxvba_com::TypeLibMemberMetadata {
        name: name.to_string(),
        token,
        vtable_slot: None,
        requires_argument: !parameter_names.is_empty(),
        invoke_kind: TypeLibMemberInvokeKind::Method,
        parameter_names: parameter_names.into_iter().map(str::to_string).collect(),
        parameter_optional,
        parameter_optional_defaults,
        is_default_member: false,
        parameter_wire_types: parameter_types
            .iter()
            .cloned()
            .map(TypeLibWireType::Automation)
            .collect(),
        parameter_iids: vec![None; parameter_types.len()],
        parameter_types,
        return_wire_type: return_type.map(TypeLibWireType::Automation),
        return_type,
        callconv_is_stdcall: false,
        is_dual: true,
        interface_iid: None,
        source_typekind: Some(SourceTypeKind::Dispatch),
        vtable_slot_bound: None,
    }
}

fn property_get_member(
    name: &str,
    token: i32,
    return_type: TypeLibParamType,
    return_wire_type: TypeLibWireType,
    is_default_member: bool,
) -> oxvba_com::TypeLibMemberMetadata {
    oxvba_com::TypeLibMemberMetadata {
        name: name.to_string(),
        token,
        vtable_slot: None,
        requires_argument: false,
        invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
        parameter_names: Vec::new(),
        parameter_optional: Vec::new(),
        parameter_optional_defaults: Vec::new(),
        is_default_member,
        parameter_wire_types: Vec::new(),
        parameter_iids: Vec::new(),
        parameter_types: Vec::new(),
        return_wire_type: Some(return_wire_type),
        return_type: Some(return_type),
        callconv_is_stdcall: false,
        is_dual: true,
        interface_iid: None,
        source_typekind: Some(SourceTypeKind::Dispatch),
        vtable_slot_bound: None,
    }
}

struct ApplicationTypeLibs;

impl TypeLibResolver for ApplicationTypeLibs {
    fn resolve(&self, request: &TypeLibResolveRequest) -> Option<TypeLibMetadataBlob> {
        if request
            .requested_coclass
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case("Workbooks"))
        {
            return Some(TypeLibMetadataBlob {
                identity: TypeLibResolvedIdentity {
                    reference_name: "Excel".into(),
                    requested_coclass: Some("Workbooks".into()),
                    importlib: "excel".into(),
                    libid: None,
                    major_version: 1,
                    minor_version: 0,
                    lcid: None,
                    cache_key: "vba-web-portable-workbooks".into(),
                },
                activation_prog_id: None,
                member_name_to_token: vec![("Item".into(), 0), ("Count".into(), 21)],
                members: vec![
                    property_get_member(
                        "Item",
                        0,
                        TypeLibParamType::Long,
                        TypeLibWireType::Automation(TypeLibParamType::Long),
                        true,
                    ),
                    property_get_member(
                        "Count",
                        21,
                        TypeLibParamType::Long,
                        TypeLibWireType::Automation(TypeLibParamType::Long),
                        false,
                    ),
                ],
                events: Vec::new(),
                coclass_names: vec!["Workbooks".into()],
            });
        }
        Some(TypeLibMetadataBlob {
            identity: TypeLibResolvedIdentity {
                reference_name: "Excel".into(),
                requested_coclass: Some("Application".into()),
                importlib: "excel".into(),
                libid: None,
                major_version: 1,
                minor_version: 0,
                lcid: None,
                cache_key: "vba-web-portable-application".into(),
            },
            activation_prog_id: Some("Excel.Application".into()),
            member_name_to_token: vec![
                ("Run".into(), 10),
                ("OnTime".into(), 11),
                ("Workbooks".into(), 20),
            ],
            members: vec![
                app_member(
                    "Run",
                    10,
                    vec!["Macro", "Arg1"],
                    vec![false, true],
                    vec![TypeLibParamType::Variant, TypeLibParamType::Variant],
                    Some(TypeLibParamType::Variant),
                ),
                app_member(
                    "OnTime",
                    11,
                    vec!["EarliestTime", "Procedure", "LatestTime", "Schedule"],
                    vec![false, false, true, true],
                    vec![
                        TypeLibParamType::Variant,
                        TypeLibParamType::String,
                        TypeLibParamType::Variant,
                        TypeLibParamType::Variant,
                    ],
                    None,
                ),
                property_get_member(
                    "Workbooks",
                    20,
                    TypeLibParamType::Object,
                    TypeLibWireType::InterfacePointer {
                        name: "Workbooks".into(),
                    },
                    false,
                ),
            ],
            events: Vec::new(),
            coclass_names: vec!["Application".into()],
        })
    }
}

struct RecordingApplicationFactory {
    calls: Arc<Mutex<Vec<String>>>,
}

impl PortableObjectFactory for RecordingApplicationFactory {
    fn create(&self) -> Box<dyn PortableDispatch> {
        Box::new(RecordingApplication {
            calls: self.calls.clone(),
        })
    }
}

struct RecordingApplication {
    calls: Arc<Mutex<Vec<String>>>,
}

impl PortableDispatch for RecordingApplication {
    fn invoke(&self, member: &str, args: &[Variant]) -> Result<Variant, String> {
        self.calls
            .lock()
            .map_err(|_| "call log poisoned".to_string())?
            .push(format!("{member}:{}", args.len()));
        match member {
            "Run" => Ok(Variant::from_i32(42)),
            "OnTime" => Ok(Variant::empty()),
            other => Err(format!("unexpected Application member `{other}`")),
        }
    }

    fn get(&self, member: &str) -> Result<Variant, String> {
        Err(format!("unexpected Application property get `{member}`"))
    }

    fn put(&self, member: &str, _value: Variant) -> Result<(), String> {
        Err(format!("unexpected Application property put `{member}`"))
    }

    fn member_names(&self) -> Vec<String> {
        vec!["Run".into(), "OnTime".into(), "Workbooks".into()]
    }

    fn get_object(&self, member: &str) -> Option<Result<PortableObjectResult, String>> {
        if member.eq_ignore_ascii_case("Workbooks") {
            self.calls
                .lock()
                .map_err(|_| "call log poisoned".to_string())
                .map(|mut calls| calls.push("Application.Workbooks:get-object".into()))
                .ok();
            return Some(Ok(PortableObjectResult::new(
                "Excel.Workbooks",
                Box::new(RecordingWorkbooks {
                    calls: self.calls.clone(),
                }),
            )));
        }
        None
    }
}

struct RecordingWorkbooks {
    calls: Arc<Mutex<Vec<String>>>,
}

impl PortableDispatch for RecordingWorkbooks {
    fn invoke(&self, member: &str, args: &[Variant]) -> Result<Variant, String> {
        Err(format!(
            "unexpected Workbooks invoke `{member}` with {} args",
            args.len()
        ))
    }

    fn get(&self, member: &str) -> Result<Variant, String> {
        self.calls
            .lock()
            .map_err(|_| "call log poisoned".to_string())?
            .push(format!("Workbooks.{member}:get"));
        match member {
            "Count" => Ok(Variant::from_i32(42)),
            "Item" => Ok(Variant::from_i32(1)),
            other => Err(format!("unexpected Workbooks property get `{other}`")),
        }
    }

    fn put(&self, member: &str, _value: Variant) -> Result<(), String> {
        Err(format!("unexpected Workbooks property put `{member}`"))
    }

    fn member_names(&self) -> Vec<String> {
        vec!["Count".into(), "Item".into()]
    }
}

fn application_host_manifest() -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Main"),
            source: "Public verdict As Long\n\
                     Sub Main()\n\
                     Dim r As Variant\n\
                     r = Application.Run(\"MacroName\", 1)\n\
                     Application.OnTime 0, \"MacroName\"\n\
                     If r = 42 Then verdict = 1\n\
                     End Sub\n"
                .into(),
        }],
        references: vec![ProjectReference::HostInjected {
            referenced_project_name: "Excel".into(),
        }],
        reference_projects: Vec::new(),
        conditional_constants: Default::default(),
        conditional_compilation_target: Default::default(),
    }
}

#[test]
fn vba_web_dictionary_createobject_is_policy_gated() {
    let mut policy = HostPolicy::deterministic_runtime();
    policy.allow_com_activation = false;
    let err = run_source_with_policy(
        "Sub Main()\n\
         Dim d As Object\n\
         Set d = CreateObject(\"Scripting.Dictionary\")\n\
         End Sub\n",
        policy,
    )
    .expect_err("COM activation must be denied by host policy");
    assert!(
        err.contains("PolicyDenied") || err.contains("policy") || err.contains("denied"),
        "unexpected diagnostic: {err}"
    );
}

#[test]
fn host_injected_application_run_and_ontime_execute_through_portable_host_root() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let projection = Arc::new(PortableComProjection::new());
    projection.register_object(
        "Excel.Application",
        Arc::new(RecordingApplicationFactory {
            calls: calls.clone(),
        }),
    );

    let host = HostBuilder::new()
        .profile(native_host_profile())
        .policy(HostPolicy::interactive_dev())
        .portable_objects(projection)
        .build();
    let manifest = application_host_manifest();

    let program = oxvba_bind::bind_program(&manifest, &ApplicationTypeLibs).expect("bind");
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate");
    let vm = oxvba_vm3::Vm3::run(&oxp, &*host).expect("run");

    assert_eq!(vm.slot(0), Some(Variant::from_i32(1)));
    assert_eq!(
        calls.lock().expect("call log").as_slice(),
        ["Run:2".to_string(), "OnTime:2".to_string()]
    );
}

#[test]
fn engine_preserves_portable_com_projection_across_policy_rebuild() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let projection = Arc::new(PortableComProjection::new());
    projection.register_object(
        "Excel.Application",
        Arc::new(RecordingApplicationFactory {
            calls: calls.clone(),
        }),
    );

    let profile = HostProfileProvider::new()
        .with_portable_com_projection(projection)
        .with_host_policy(HostPolicy::interactive_dev());
    let engine = Engine::new(HostConfig::vm3()).with_host_profile_provider(profile);
    let values = engine
        .execute_source_with_variant_snapshot_clean(
            "Public verdict As Long\n\
             Sub Main()\n\
             Dim app As Object\n\
             Set app = CreateObject(\"Excel.Application\")\n\
             verdict = app.Run(\"MacroName\", 1)\n\
             End Sub\n",
        )
        .expect("portable CreateObject dispatch should run through Engine");

    assert_eq!(first_i32(&values), Some(42));
    assert_eq!(calls.lock().expect("call log").as_slice(), ["Run:2"]);
}

#[test]
fn engine_executes_host_injected_application_through_portable_host_root() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let projection = Arc::new(PortableComProjection::new());
    projection.register_object(
        "Excel.Application",
        Arc::new(RecordingApplicationFactory {
            calls: calls.clone(),
        }),
    );

    let profile = HostProfileProvider::new()
        .with_typelib_resolver(Arc::new(ApplicationTypeLibs))
        .with_portable_com_projection(projection);
    let engine = Engine::new(HostConfig::vm3()).with_host_profile_provider(profile);
    let manifest = application_host_manifest();

    let values = engine
        .execute_manifest_with_variant_snapshot(&manifest)
        .expect("host-injected Application should execute through Engine");

    assert_eq!(first_i32(&values), Some(1));
    assert_eq!(
        calls.lock().expect("call log").as_slice(),
        ["Run:2".to_string(), "OnTime:2".to_string()]
    );
}

#[test]
fn engine_project_closure_executes_host_injected_application_through_portable_host_root() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let projection = Arc::new(PortableComProjection::new());
    projection.register_object(
        "Excel.Application",
        Arc::new(RecordingApplicationFactory {
            calls: calls.clone(),
        }),
    );

    let profile = HostProfileProvider::new()
        .with_typelib_resolver(Arc::new(ApplicationTypeLibs))
        .with_portable_com_projection(projection);
    let engine = Engine::new(HostConfig::vm3()).with_host_profile_provider(profile);
    let manifest = application_host_manifest();
    let values = engine
        .execute_project_closure_with_variant_snapshot(&[manifest])
        .expect("host-injected Application should execute through Engine project closure");

    assert_eq!(first_i32(&values), Some(1));
    assert_eq!(
        calls.lock().expect("call log").as_slice(),
        ["Run:2".to_string(), "OnTime:2".to_string()]
    );
}

#[test]
fn engine_executes_host_returned_com_object_chain_through_portable_projection() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let projection = Arc::new(PortableComProjection::new());
    projection.register_object(
        "Excel.Application",
        Arc::new(RecordingApplicationFactory {
            calls: calls.clone(),
        }),
    );

    let profile = HostProfileProvider::new()
        .with_typelib_resolver(Arc::new(ApplicationTypeLibs))
        .with_portable_com_projection(projection);
    let engine = Engine::new(HostConfig::vm3()).with_host_profile_provider(profile);
    let manifest = SymbolProjectManifest {
        project_name: "Proj".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "Main".into(),
            module_kind: ModuleKind::Procedural,
            attributes: ModuleAttributes::named("Main"),
            source: "Public verdict As Long\n\
                     Sub Main()\n\
                     Dim n As Long\n\
                     n = Application.Workbooks.Count\n\
                     n = n + Application.Workbooks\n\
                     If n = 43 Then verdict = 1\n\
                     End Sub\n"
                .into(),
        }],
        references: vec![ProjectReference::HostInjected {
            referenced_project_name: "Excel.Application".into(),
        }],
        reference_projects: Vec::new(),
        conditional_constants: Default::default(),
        conditional_compilation_target: Default::default(),
    };

    let values = engine
        .execute_manifest_with_variant_snapshot(&manifest)
        .expect("host-returned COM object chain should execute through portable projection");

    assert_eq!(first_i32(&values), Some(1));
    assert_eq!(
        calls.lock().expect("call log").as_slice(),
        [
            "Application.Workbooks:get-object".to_string(),
            "Workbooks.Count:get".to_string(),
            "Application.Workbooks:get-object".to_string(),
            "Workbooks.Item:get".to_string(),
        ]
    );
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "live COM; requires Scripting.Dictionary registration"]
fn vba_web_dictionary_late_bound_smoke_executes_when_available() {
    let source = "Public verdict As Long\n\
         Sub Main()\n\
         Dim d As Object\n\
         Set d = CreateObject(\"Scripting.Dictionary\")\n\
         d.Add \"status\", 200\n\
         d.Add \"body\", \"ok\"\n\
         verdict = d.Count * 1000\n\
         If d.Exists(\"status\") Then verdict = verdict + d(\"status\")\n\
         If d(\"body\") = \"ok\" Then verdict = verdict + 1\n\
         End Sub\n";
    let values = run_source_with_policy(source, HostPolicy::interactive_dev())
        .expect("live Scripting.Dictionary smoke should run when registered");
    assert_eq!(first_i32(&values), Some(2201), "snapshot: {values:?}");
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "live COM; requires WinHttpRequest registration; no network I/O"]
fn vba_web_winhttp_activation_and_setup_smoke_executes_when_available() {
    let source = "Public verdict As Long\n\
         Sub Main()\n\
         Dim req As Object\n\
         Set req = CreateObject(\"WinHttp.WinHttpRequest.5.1\")\n\
         req.SetTimeouts 1, 1, 1, 1\n\
         req.Open \"GET\", \"https://example.test/\", False\n\
         verdict = 1\n\
         End Sub\n";
    let values = run_source_with_policy(source, HostPolicy::interactive_dev())
        .expect("live WinHttpRequest activation/setup smoke should run when registered");
    assert_eq!(first_i32(&values), Some(1), "snapshot: {values:?}");
}
