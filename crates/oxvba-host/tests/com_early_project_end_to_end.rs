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
fn run_project_windows_hosted_error(manifest: &ProjectManifest, enable_jit: bool) -> String {
    let mut engine = Engine::new(HostConfig {
        enable_jit,
        root_object_name: None,
    });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let err = engine
        .execute_project_with_snapshot_phased(manifest)
        .expect_err("project should fail deterministically");
    assert_eq!(err.phase(), DiagnosticPhase::Runtime);
    err.message().to_string()
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
fn early_bound_project_executes_imported_call_statements_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.Count()
Call obj.Exists(42)
Call obj.Lookup(42)
Call obj.Value()
Call obj(42)
afterValue = 19
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(19),
        "Call-form imported positional method/property/default-member invokes should execute without degrading the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_call_statement_subset_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.Count()
Call obj.Exists(42)
Call obj.Lookup(42)
Call obj.Value()
Call obj(42)
afterValue = 19
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported Call-form positional member invokes"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_named_argument_call_statements() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.SumPair(rhs := 14, lhs := 3)
Call obj.LookupPair(rhs := 9, lhs := 5)
Call obj(value := 41)
afterValue = 29
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(29),
        "Call-form imported named-argument method/property/default-member invokes should execute without degrading metadata-backed canonicalization"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_call_statements_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.SumPair(rhs := 14, lhs := 3)
Call obj.LookupPair(rhs := 9, lhs := 5)
Call obj(value := 41)
afterValue = 29
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported Call-form named-argument member invokes"
    );
}

#[test]
fn early_bound_project_executes_imported_no_paren_call_statements_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.Count
Call obj.Exists 42
Call obj.Lookup 42
Call obj.Value
Call obj 42
afterValue = 43
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(43),
        "no-paren Call-form imported positional method/property/default-member invokes should execute on the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_no_paren_call_statement_subset_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.Count
Call obj.Exists 42
Call obj.Lookup 42
Call obj.Value
Call obj 42
afterValue = 43
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported no-paren Call-form positional member invokes"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_no_paren_named_argument_call_statements() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.SumPair rhs := 14, lhs := 3
Call obj.LookupPair rhs := 9, lhs := 5
Call obj value := 41
afterValue = 47
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(47),
        "no-paren Call-form imported named-argument method/property/default-member invokes should execute on the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_no_paren_named_argument_call_statements_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.SumPair rhs := 14, lhs := 3
Call obj.LookupPair rhs := 9, lhs := 5
Call obj value := 41
afterValue = 47
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported no-paren Call-form named-argument member invokes"
    );
}

#[test]
fn early_bound_project_executes_imported_statement_context_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.Count()
obj.Exists(42)
obj.Lookup(42)
obj.Value()
obj(42)
afterValue = 31
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(31),
        "statement-context imported positional method/property/default-member invokes should execute without degrading the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_statement_context_subset_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.Count()
obj.Exists(42)
obj.Lookup(42)
obj.Value()
obj(42)
afterValue = 31
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported statement-context positional member invokes"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_named_argument_statement_context() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.SumPair(rhs := 14, lhs := 3)
obj.LookupPair(rhs := 9, lhs := 5)
obj(value := 41)
afterValue = 37
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(37),
        "statement-context imported named-argument method/property/default-member invokes should execute without degrading metadata-backed canonicalization"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_statement_context_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.SumPair(rhs := 14, lhs := 3)
obj.LookupPair(rhs := 9, lhs := 5)
obj(value := 41)
afterValue = 37
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported statement-context named-argument member invokes"
    );
}

#[test]
fn early_bound_project_executes_imported_no_paren_statement_context_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.Exists 42
obj.Lookup 42
obj 42
afterValue = 53
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(53),
        "no-paren statement-context imported positional method/property/default-member invokes should execute on the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_no_paren_statement_context_subset_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.Exists 42
obj.Lookup 42
obj 42
afterValue = 53
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported no-paren statement-context positional member invokes"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_no_paren_named_argument_statement_context() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.SumPair rhs := 14, lhs := 3
obj.LookupPair rhs := 9, lhs := 5
obj value := 41
afterValue = 59
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(59),
        "no-paren statement-context imported named-argument method/property/default-member invokes should execute on the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_no_paren_named_argument_statement_context_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.SumPair rhs := 14, lhs := 3
obj.LookupPair rhs := 9, lhs := 5
obj value := 41
afterValue = 59
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported no-paren statement-context named-argument member invokes"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_reports_runtime_error_for_imported_raise_exception_call_statement() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Call obj.RaiseException()
End Sub
"#,
    );

    let vm = run_project_windows_hosted_error(&manifest, false);
    let jit = run_project_windows_hosted_error(&manifest, true);
    assert!(
        vm.contains("com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;")
            && jit.contains(
                "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;"
            ),
        "expected stable imported exception prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
    assert!(
        vm.contains("excep_source=\"OxVba.TestDispatch\"")
            && vm.contains("excep_description=\"controlled dispatch exception\"")
            && jit.contains("excep_source=\"OxVba.TestDispatch\"")
            && jit.contains("excep_description=\"controlled dispatch exception\""),
        "expected imported EXCEPINFO source/description across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_reports_runtime_error_for_imported_raise_exception_statement_context() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
obj.RaiseException()
End Sub
"#,
    );

    let vm = run_project_windows_hosted_error(&manifest, false);
    let jit = run_project_windows_hosted_error(&manifest, true);
    assert!(
        vm.contains("com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;")
            && jit.contains(
                "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;"
            ),
        "expected stable imported exception prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
    assert!(
        vm.contains("excep_source=\"OxVba.TestDispatch\"")
            && vm.contains("excep_description=\"controlled dispatch exception\"")
            && jit.contains("excep_source=\"OxVba.TestDispatch\"")
            && jit.contains("excep_description=\"controlled dispatch exception\""),
        "expected imported EXCEPINFO source/description across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_reports_runtime_error_for_imported_no_paren_raise_exception_call_statement()
{
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Call obj.RaiseException
End Sub
"#,
    );

    let vm = run_project_windows_hosted_error(&manifest, false);
    let jit = run_project_windows_hosted_error(&manifest, true);
    assert!(
        vm.contains("com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;")
            && jit.contains(
                "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;"
            ),
        "expected stable imported exception prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
    assert!(
        vm.contains("excep_source=\"OxVba.TestDispatch\"")
            && vm.contains("excep_description=\"controlled dispatch exception\"")
            && jit.contains("excep_source=\"OxVba.TestDispatch\"")
            && jit.contains("excep_description=\"controlled dispatch exception\""),
        "expected imported EXCEPINFO source/description across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_reports_runtime_error_for_imported_no_paren_raise_exception_statement_context()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
obj.RaiseException
End Sub
"#,
    );

    let vm = run_project_windows_hosted_error(&manifest, false);
    let jit = run_project_windows_hosted_error(&manifest, true);
    assert!(
        vm.contains("com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;")
            && jit.contains(
                "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;"
            ),
        "expected stable imported exception prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
    assert!(
        vm.contains("excep_source=\"OxVba.TestDispatch\"")
            && vm.contains("excep_description=\"controlled dispatch exception\"")
            && jit.contains("excep_source=\"OxVba.TestDispatch\"")
            && jit.contains("excep_description=\"controlled dispatch exception\""),
        "expected imported EXCEPINFO source/description across VM/JIT, got vm={vm:?} jit={jit:?}"
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

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_named_argument_property_putref_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim other As New OxVba.TestDispatch
Dim otherCount
Dim afterSetIndexedValueRef
otherCount = DispatchInvoke(other, "Count")
Set obj.SetIndexedValueRef(lhs := 8) = other
afterSetIndexedValueRef = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(
        expect_object_handle(&out[0]).raw() >= 20_001,
        "receiver should remain a controlled object handle"
    );
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert_eq!(
        out[2],
        RuntimeValue::I32(7),
        "controlled property-putref object lane should interrogate the bound object deterministically"
    );
    assert_eq!(
        out[3],
        RuntimeValue::I32(408_007),
        "named-argument property-putref assignment should preserve index and bounded object-derived token"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_property_putref_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim other As New OxVba.TestDispatch
Dim otherCount
Dim afterSetIndexedValueRef
otherCount = DispatchInvoke(other, "Count")
Set obj.SetIndexedValueRef(lhs := 8) = other
afterSetIndexedValueRef = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported named-argument property-putref assignment syntax"
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
fn early_bound_project_executes_imported_zero_arg_method_read_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim implicitValue
Dim explicitValue
implicitValue = obj.Ping
Let explicitValue = obj.Ping
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(123),
        "imported zero-arg method read-assignment should lower through metadata-backed invoke syntax"
    );
    assert_eq!(
        out[2],
        RuntimeValue::I32(123),
        "explicit Let should preserve imported zero-arg method read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_zero_arg_method_read_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim implicitValue
Dim explicitValue
implicitValue = obj.Ping
Let explicitValue = obj.Ping
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported zero-arg method read-assignment syntax"
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
fn early_bound_project_executes_imported_parenthesized_zero_arg_property_get_read_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim implicitValue
Dim explicitValue
obj.SetValue = 9
implicitValue = obj.Value()
Let explicitValue = obj.Value()
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(9),
        "imported parenthesized zero-arg property-get read-assignment should lower through metadata-backed getter syntax"
    );
    assert_eq!(
        out[2],
        RuntimeValue::I32(9),
        "explicit Let should preserve imported parenthesized zero-arg property-get read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_parenthesized_zero_arg_property_get_read_assignment_vm_jit_snapshots_match()
{
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim implicitValue
Dim explicitValue
obj.SetValue = 9
implicitValue = obj.Value()
Let explicitValue = obj.Value()
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported parenthesized zero-arg property-get read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_object_property_get_read_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.SelfDispatch
Set childUnknown = obj.SelfUnknown
wrappedDispatch = obj.SelfDispatch
Let wrappedUnknown = obj.SelfUnknown
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert!(expect_object_handle(&out[3]).raw() >= 20_001);
    assert!(expect_object_handle(&out[4]).raw() >= 20_001);
    assert_eq!(
        out[5],
        RuntimeValue::I32(7),
        "direct imported object-valued property-get should preserve VT_DISPATCH rebinding on Object targets"
    );
    assert_eq!(
        out[6],
        RuntimeValue::I32(7),
        "direct imported object-valued property-get should preserve VT_UNKNOWN rebinding on Object targets"
    );
    assert_eq!(
        out[7],
        RuntimeValue::I32(7),
        "direct imported object-valued property-get should preserve VT_DISPATCH rebinding on Variant targets"
    );
    assert_eq!(
        out[8],
        RuntimeValue::I32(7),
        "direct imported object-valued property-get should preserve VT_UNKNOWN rebinding on explicit-Let Variant targets"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_object_property_get_read_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.SelfDispatch
Set childUnknown = obj.SelfUnknown
wrappedDispatch = obj.SelfDispatch
Let wrappedUnknown = obj.SelfUnknown
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported object-valued property-get read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_parenthesized_object_property_get_read_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.SelfDispatch()
Set childUnknown = obj.SelfUnknown()
wrappedDispatch = obj.SelfDispatch()
Let wrappedUnknown = obj.SelfUnknown()
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert!(expect_object_handle(&out[3]).raw() >= 20_001);
    assert!(expect_object_handle(&out[4]).raw() >= 20_001);
    assert_eq!(
        out[5],
        RuntimeValue::I32(7),
        "parenthesized imported object-valued property-get should preserve VT_DISPATCH rebinding on Object targets"
    );
    assert_eq!(
        out[6],
        RuntimeValue::I32(7),
        "parenthesized imported object-valued property-get should preserve VT_UNKNOWN rebinding on Object targets"
    );
    assert_eq!(
        out[7],
        RuntimeValue::I32(7),
        "parenthesized imported object-valued property-get should preserve VT_DISPATCH rebinding on Variant targets"
    );
    assert_eq!(
        out[8],
        RuntimeValue::I32(7),
        "parenthesized imported object-valued property-get should preserve VT_UNKNOWN rebinding on explicit-Let Variant targets"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_parenthesized_object_property_get_read_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.SelfDispatch()
Set childUnknown = obj.SelfUnknown()
wrappedDispatch = obj.SelfDispatch()
Let wrappedUnknown = obj.SelfUnknown()
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for parenthesized imported object-valued property-get read-assignment syntax"
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
fn early_bound_project_executes_imported_explicit_let_named_argument_calls() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim sumPair
Dim lookupPair
Dim echoValue
Let sumPair = obj.SumPair(rhs := 14, lhs := 3)
Let lookupPair = obj.LookupPair(rhs := 9, lhs := 5)
Let echoValue = obj(value := 41)
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(3_014),
        "explicit Let imported method call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
    assert_eq!(
        out[2],
        RuntimeValue::I32(205_009),
        "explicit Let imported property-get call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
    assert_eq!(
        out[3],
        RuntimeValue::I32(41),
        "explicit Let imported default-member call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_explicit_let_named_argument_calls_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim sumPair
Dim lookupPair
Dim echoValue
Let sumPair = obj.SumPair(rhs := 14, lhs := 3)
Let lookupPair = obj.LookupPair(rhs := 9, lhs := 5)
Let echoValue = obj(value := 41)
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for explicit Let imported named-argument calls"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_explicit_let_positional_calls() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim countValue
Dim existsValue
Dim lookupValue
Dim echoValue
Let countValue = obj.Count()
Let existsValue = obj.Exists(42)
Let lookupValue = obj.Lookup(42)
Let echoValue = obj(42)
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(7),
        "explicit Let imported zero-arg call should preserve metadata-backed lowering"
    );
    assert_eq!(
        out[2],
        RuntimeValue::Bool(true),
        "explicit Let imported method call should preserve metadata-backed lowering"
    );
    assert_eq!(
        out[3],
        RuntimeValue::I32(1_042),
        "explicit Let imported property-get call should preserve metadata-backed lowering"
    );
    assert_eq!(
        out[4],
        RuntimeValue::I32(42),
        "explicit Let imported default-member call should preserve metadata-backed lowering"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_explicit_let_positional_calls_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim countValue
Dim existsValue
Dim lookupValue
Dim echoValue
Let countValue = obj.Count()
Let existsValue = obj.Exists(42)
Let lookupValue = obj.Lookup(42)
Let echoValue = obj(42)
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for explicit Let imported positional calls"
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

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_object_result_assignment_intents() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.ReturnSelfDispatch()
Set childUnknown = obj.ReturnSelfUnknown()
wrappedDispatch = obj.ReturnSelfDispatch()
Let wrappedUnknown = obj.ReturnSelfUnknown()
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert!(expect_object_handle(&out[3]).raw() >= 20_001);
    assert!(expect_object_handle(&out[4]).raw() >= 20_001);
    assert_eq!(
        out[5],
        RuntimeValue::I32(7),
        "explicit Set should preserve VT_DISPATCH object-result rebinding on Object targets"
    );
    assert_eq!(
        out[6],
        RuntimeValue::I32(7),
        "explicit Set should preserve VT_UNKNOWN object-result rebinding on Object targets"
    );
    assert_eq!(
        out[7],
        RuntimeValue::I32(7),
        "implicit Variant-target assignment should preserve VT_DISPATCH object-result rebinding"
    );
    assert_eq!(
        out[8],
        RuntimeValue::I32(7),
        "explicit Let Variant-target assignment should preserve VT_UNKNOWN object-result rebinding"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_zero_arg_object_result_assignment_intents_without_parentheses()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.ReturnSelfDispatch
Set childUnknown = obj.ReturnSelfUnknown
wrappedDispatch = obj.ReturnSelfDispatch
Let wrappedUnknown = obj.ReturnSelfUnknown
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert!(expect_object_handle(&out[3]).raw() >= 20_001);
    assert!(expect_object_handle(&out[4]).raw() >= 20_001);
    assert_eq!(
        out[5],
        RuntimeValue::I32(7),
        "explicit Set should preserve VT_DISPATCH zero-arg method rebinding on Object targets without parentheses"
    );
    assert_eq!(
        out[6],
        RuntimeValue::I32(7),
        "explicit Set should preserve VT_UNKNOWN zero-arg method rebinding on Object targets without parentheses"
    );
    assert_eq!(
        out[7],
        RuntimeValue::I32(7),
        "implicit Variant-target assignment should preserve VT_DISPATCH zero-arg method rebinding without parentheses"
    );
    assert_eq!(
        out[8],
        RuntimeValue::I32(7),
        "explicit Let Variant-target assignment should preserve VT_UNKNOWN zero-arg method rebinding without parentheses"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_zero_arg_object_result_assignment_intents_without_parentheses_vm_jit_snapshots_match()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.ReturnSelfDispatch
Set childUnknown = obj.ReturnSelfUnknown
wrappedDispatch = obj.ReturnSelfDispatch
Let wrappedUnknown = obj.ReturnSelfUnknown
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported zero-arg object-result assignment intents without parentheses"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_object_result_assignment_intents_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.ReturnSelfDispatch()
Set childUnknown = obj.ReturnSelfUnknown()
wrappedDispatch = obj.ReturnSelfDispatch()
Let wrappedUnknown = obj.ReturnSelfUnknown()
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported object-result assignment-intent lanes"
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

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_property_putref_assignments_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim other As New OxVba.TestDispatch
Dim otherCount
Dim afterSetValueRef
otherCount = DispatchInvoke(other, "Count")
Set obj.SetValueRef = other
afterSetValueRef = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(
        expect_object_handle(&out[0]).raw() >= 20_001,
        "receiver should remain a controlled object handle"
    );
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert_eq!(
        out[2],
        RuntimeValue::I32(7),
        "controlled property-putref object lane should interrogate the bound object deterministically"
    );
    assert_eq!(
        out[3],
        RuntimeValue::I32(100_007),
        "property-putref assignment should preserve bounded object-derived token on the deterministic setter lane"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_property_putref_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim other As New OxVba.TestDispatch
Dim otherCount
Dim afterSetValueRef
otherCount = DispatchInvoke(other, "Count")
Set obj.SetValueRef = other
afterSetValueRef = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported property-putref assignment syntax"
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
fn early_bound_project_reports_compile_error_for_imported_withevents_source() {
    let class_module = module_unit_from_source(
        "Sink",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Sink"
Private WithEvents src As OxVba.TestEventServer
Public Sub Attach()
End Sub
"#,
    )
    .expect("class module should parse");
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
End Sub
"#,
    )
    .expect("main module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("imported WithEvents source should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-WITHEVENTS-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_unqualified_imported_withevents_source() {
    let class_module = module_unit_from_source(
        "Sink",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Sink"
Private WithEvents src As TestEventServer
Public Sub Attach()
End Sub
"#,
    )
    .expect("class module should parse");
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
End Sub
"#,
    )
    .expect("main module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("unqualified imported WithEvents source should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-WITHEVENTS-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_unqualified_imported_type_declaration() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As TestDispatch
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("unqualified imported type declaration should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-UNQUALIFIED-TYPE-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_unqualified_imported_as_new_declaration() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New TestDispatch
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("unqualified imported As New declaration should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-UNQUALIFIED-TYPE-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_imported_module_scope_declaration() {
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Private obj As OxVba.TestDispatch
Public Sub Main()
End Sub
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("module-scope imported declaration should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-MODULE-DECL-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_unqualified_imported_module_scope_declaration() {
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Private obj As TestDispatch
Public Sub Main()
End Sub
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("unqualified module-scope imported declaration should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-MODULE-DECL-UNSUPPORTED"),
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

#[test]
fn early_bound_project_reports_compile_error_for_wrong_default_member_arity_no_paren_call_statement()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Call obj
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong zero-arg no-paren Call default-member arity should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_wrong_default_member_arity_no_paren_statement() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
obj
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong zero-arg no-paren statement default-member arity should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_missing_default_member_no_paren_call_statement() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchNoDefault
Call obj 41
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("missing no-paren Call default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-NOT-FOUND"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_missing_default_member_no_paren_statement() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchNoDefault
obj 41
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("missing no-paren statement default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-NOT-FOUND"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_ambiguous_default_member_no_paren_call_statement()
{
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchAmbiguousDefault
Call obj 41
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("ambiguous no-paren Call default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-AMBIGUOUS"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_ambiguous_default_member_no_paren_statement() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchAmbiguousDefault
obj 41
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("ambiguous no-paren statement default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-AMBIGUOUS"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_wrong_default_member_arity_parenthesized_call_statement()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Call obj()
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong parenthesized Call default-member arity should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_wrong_default_member_arity_parenthesized_statement()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
obj()
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong parenthesized statement default-member arity should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_missing_default_member_parenthesized_call_statement()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchNoDefault
Call obj(41)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("missing parenthesized Call default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-NOT-FOUND"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_missing_default_member_parenthesized_statement() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchNoDefault
obj(41)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("missing parenthesized statement default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-NOT-FOUND"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_ambiguous_default_member_parenthesized_call_statement()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchAmbiguousDefault
Call obj(41)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("ambiguous parenthesized Call default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-AMBIGUOUS"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_ambiguous_default_member_parenthesized_statement()
{
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchAmbiguousDefault
obj(41)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("ambiguous parenthesized statement default member should fail");
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
