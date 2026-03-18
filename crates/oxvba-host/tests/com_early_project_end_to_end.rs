use oxvba_compiler::{
    ModuleKind, ProjectKind, ProjectManifest, ProjectReference, ReferenceKind,
    module_unit_from_source,
};
use oxvba_hal::model::HostPolicy;
use oxvba_host::engine::DiagnosticPhase;
use oxvba_host::{Engine, HostConfig};
use oxvba_runtime::{ObjectHandle, RuntimeValue};

fn manifest_with_typelib(main_source: &str) -> ProjectManifest {
    let main_module = module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
        .expect("main module should parse");
    ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    }
}

#[cfg(target_os = "windows")]
fn run_project_windows_hosted(manifest: &ProjectManifest, enable_jit: bool) -> Vec<RuntimeValue> {
    let mut engine = Engine::new(HostConfig {
        enable_jit,
        root_object_name: None,
    });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .execute_project_with_snapshot_phased(manifest)
        .expect("project should execute")
}

#[cfg(target_os = "windows")]
fn expect_object_handle(value: &RuntimeValue) -> ObjectHandle {
    match value {
        RuntimeValue::ObjectHandle(handle) => *handle,
        other => panic!("expected object handle, got {:?}", other),
    }
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_with_typed_declarations_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim countValue
Dim existsValue
Dim lookupValue
countValue = obj.Count()
existsValue = obj.Exists(42)
lookupValue = obj.Lookup(42)
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(7),
        "Count should map through early-bind rewrite lane"
    );
    assert_eq!(
        out[2],
        RuntimeValue::Bool(true),
        "Exists(42) should map through early-bind rewrite lane"
    );
    assert_eq!(
        out[3],
        RuntimeValue::I32(1_042),
        "Lookup(42) should map through metadata-backed property-get lane"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_vm_jit_snapshots_match_for_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim countValue
Dim existsValue
Dim lookupValue
countValue = obj.Count()
existsValue = obj.Exists(41)
lookupValue = obj.Lookup(41)
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for early-binding subset"
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_unsupported_member_shape() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Call obj.SetValue(9)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("unsupported member shape should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-MEMBER-SHAPE-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_unsupported_member() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim value
value = obj.UnknownMember()
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("unsupported member should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_wrong_arity() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim value
value = obj.Exists()
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong-arity member should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_and_late_dispatch_paths_can_mix_in_one_project() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim a
Dim b
a = obj.Count()
b = DispatchInvoke(obj, "Exists", 42)
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(out[1], RuntimeValue::I32(7));
    assert_eq!(out[2], RuntimeValue::Bool(true));
}
