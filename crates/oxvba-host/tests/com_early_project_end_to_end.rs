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
Dim echoValue
countValue = obj.Count()
existsValue = obj.Exists(42)
lookupValue = obj.Lookup(42)
echoValue = obj(42)
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
    assert_eq!(
        out[4],
        RuntimeValue::I32(42),
        "obj(42) should map through metadata-backed default-member lane"
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
Dim echoValue
countValue = obj.Count()
existsValue = obj.Exists(41)
lookupValue = obj.Lookup(41)
echoValue = obj(41)
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

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_property_put_assignments_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterSetValue
Dim afterSetIndexedValue
obj.SetValue = 9
afterSetValue = DispatchInvoke(obj, "Value")
obj.SetIndexedValue(7) = 11
afterSetIndexedValue = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(9),
        "imported property-put assignment should lower into the deterministic setter lane"
    );
    assert_eq!(
        out[2],
        RuntimeValue::I32(307_011),
        "imported indexed property-put assignment should preserve index and value"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_property_put_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterSetValue
Dim afterSetIndexedValue
obj.SetValue = 9
afterSetValue = DispatchInvoke(obj, "Value")
obj.SetIndexedValue(7) = 11
afterSetIndexedValue = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported property-put assignment subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_named_argument_property_put_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterSetIndexedValue
obj.SetIndexedValue(lhs := 7) = 11
afterSetIndexedValue = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(307_011),
        "imported named-argument property-put assignment should preserve metadata-backed parameter naming"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_property_put_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterSetIndexedValue
obj.SetIndexedValue(lhs := 7) = 11
afterSetIndexedValue = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported named-argument property-put assignment syntax"
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_named_argument_property_putref_assignment_shape() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim other As New OxVba.TestDispatch
Set obj.SetIndexedValueRef(lhs := 8) = other
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("named-argument property-putref assignment should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-MEMBER-SHAPE-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_zero_arg_property_get_read_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim implicitValue
Dim explicitValue
obj.SetValue = 9
implicitValue = obj.Value
Let explicitValue = obj.Value
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(9),
        "imported zero-arg property-get read-assignment should lower through metadata-backed getter syntax"
    );
    assert_eq!(
        out[2],
        RuntimeValue::I32(9),
        "explicit Let should preserve imported zero-arg property-get read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_zero_arg_property_get_read_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim implicitValue
Dim explicitValue
obj.SetValue = 9
implicitValue = obj.Value
Let explicitValue = obj.Value
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported zero-arg property-get read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_object_result_member_calls() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim child As Object
Dim wrapped
Dim childCount
Dim wrappedCount
Set child = obj.ReturnSelfDispatch()
wrapped = obj.ReturnSelfUnknown()
childCount = DispatchInvoke(child, "Count")
wrappedCount = DispatchInvoke(wrapped, "Count")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert_eq!(
        out[3],
        RuntimeValue::I32(7),
        "VT_DISPATCH imported member result should rebind into an invokable object handle"
    );
    assert_eq!(
        out[4],
        RuntimeValue::I32(7),
        "VT_UNKNOWN imported member result should rebind through IDispatch into an invokable object handle"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_named_argument_calls() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim sumPair
Dim lookupPair
Dim echoValue
sumPair = obj.SumPair(rhs := 14, lhs := 3)
lookupPair = obj.LookupPair(rhs := 9, lhs := 5)
echoValue = obj(value := 41)
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(3_014),
        "imported method call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
    assert_eq!(
        out[2],
        RuntimeValue::I32(205_009),
        "imported property-get call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
    assert_eq!(
        out[3],
        RuntimeValue::I32(41),
        "imported default-member call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_calls_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim sumPair
Dim lookupPair
Dim echoValue
sumPair = obj.SumPair(rhs := 14, lhs := 3)
lookupPair = obj.LookupPair(rhs := 9, lhs := 5)
echoValue = obj(value := 41)
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported named-argument calls"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_object_result_member_calls_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim child As Object
Dim wrapped
Dim childCount
Dim wrappedCount
Set child = obj.ReturnSelfDispatch()
wrapped = obj.ReturnSelfUnknown()
childCount = DispatchInvoke(child, "Count")
wrappedCount = DispatchInvoke(wrapped, "Count")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported object-result member calls"
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_missing_member() {
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
        .expect_err("missing member should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-NOT-FOUND"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_unsupported_property_putref_assignment_shape() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim other As New OxVba.TestDispatch
Set obj.SetValueRef = other
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("unsupported property-putref assignment should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-MEMBER-SHAPE-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_wrong_property_put_arity() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
obj.SetIndexedValue = 11
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong property-put arity should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
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

#[test]
fn early_bound_project_reports_compile_error_for_wrong_default_member_arity() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim value
value = obj()
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong default-member arity should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_missing_default_member() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchNoDefault
Dim value
value = obj(41)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("missing default member should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-NOT-FOUND"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_ambiguous_default_member() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchAmbiguousDefault
Dim value
value = obj(41)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("ambiguous default member should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-AMBIGUOUS"),
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
