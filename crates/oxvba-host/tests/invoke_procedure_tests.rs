//! Tests for Engine::invoke_procedure, create_class_instance, invoke_member_on_object.

use std::collections::BTreeMap;

use oxvba_compiler::{
    ModuleAttributes, ModuleKind, ModuleUnit, OxBundle, ProjectKind, ProjectManifest,
    ProjectReference, ReferenceKind, compile_project,
};
use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig};
use oxvba_runtime::{RuntimeValue, bstr::BStr};

fn make_manifest(modules: Vec<ModuleUnit>) -> ProjectManifest {
    ProjectManifest {
        project_name: "TestProj".to_string(),
        project_kind: ProjectKind::Library,
        modules,
        references: vec![],
        reference_projects: vec![],
        conditional_constants: BTreeMap::new(),
    }
}

fn make_source_manifest_with_reference(
    reference_name: &str,
    modules: Vec<ModuleUnit>,
) -> ProjectManifest {
    ProjectManifest {
        project_name: "TestProj".to_string(),
        project_kind: ProjectKind::Source,
        modules,
        references: vec![ProjectReference {
            referenced_project_name: reference_name.to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: vec![],
        conditional_constants: BTreeMap::new(),
    }
}

fn make_module(name: &str, source: &str) -> ModuleUnit {
    ModuleUnit {
        module_name: name.to_string(),
        module_kind: ModuleKind::Procedural,
        attributes: ModuleAttributes {
            vb_name: name.to_string(),
            ..Default::default()
        },
        source: source.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Invoke Sub (no return value)
// ---------------------------------------------------------------------------

#[test]
fn invoke_sub_no_return() {
    let source = "Public Sub DoWork()\nEnd Sub\n";
    let manifest = make_manifest(vec![make_module("Mod1", source)]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();

    let result = engine
        .invoke_procedure(&mut session, "Mod1", "DoWork", &[])
        .unwrap();

    assert_eq!(result, RuntimeValue::Empty);
}

// ---------------------------------------------------------------------------
// Invoke Function (with return value)
// ---------------------------------------------------------------------------

#[test]
fn invoke_function_returns_value() {
    let source = "Public Function GetValue() As Long\nGetValue = 42\nEnd Function\n";
    let manifest = make_manifest(vec![make_module("Mod1", source)]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();

    let result = engine
        .invoke_procedure(&mut session, "Mod1", "GetValue", &[])
        .unwrap();

    // The return value should be 42
    match result {
        RuntimeValue::I32(v) => assert_eq!(v, 42),
        other => panic!("Expected I32(42), got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Invoke with wrong arity
// ---------------------------------------------------------------------------

#[test]
fn invoke_wrong_arity_is_error() {
    let source = "Public Sub TakeTwo(a, b)\nEnd Sub\n";
    let manifest = make_manifest(vec![make_module("Mod1", source)]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();

    let result = engine.invoke_procedure(&mut session, "Mod1", "TakeTwo", &[]);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("arity mismatch"), "got: {err}");
}

// ---------------------------------------------------------------------------
// Invoke missing procedure
// ---------------------------------------------------------------------------

#[test]
fn invoke_missing_procedure_is_error() {
    let source = "Public Sub Hello()\nEnd Sub\n";
    let manifest = make_manifest(vec![make_module("Mod1", source)]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();

    let result = engine.invoke_procedure(&mut session, "Mod1", "NonExistent", &[]);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("procedure not found"), "got: {err}");
}

// ---------------------------------------------------------------------------
// Multiple invocations on same session
// ---------------------------------------------------------------------------

#[test]
fn multiple_invocations_on_same_session() {
    let source = concat!(
        "Public Function Double(x As Long) As Long\nDouble = x * 2\nEnd Function\n",
        "Public Function Triple(x As Long) As Long\nTriple = x * 3\nEnd Function\n",
    );
    let manifest = make_manifest(vec![make_module("Mod1", source)]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();

    let r1 = engine
        .invoke_procedure(&mut session, "Mod1", "Double", &[RuntimeValue::I32(5)])
        .unwrap();
    match r1 {
        RuntimeValue::I32(v) => assert_eq!(v, 10),
        other => panic!("Expected I32(10), got {other:?}"),
    }

    let r2 = engine
        .invoke_procedure(&mut session, "Mod1", "Triple", &[RuntimeValue::I32(7)])
        .unwrap();
    match r2 {
        RuntimeValue::I32(v) => assert_eq!(v, 21),
        other => panic!("Expected I32(21), got {other:?}"),
    }
}

#[test]
fn invoke_function_foreach_over_project_newenum_array_executes() {
    let manifest = make_manifest(vec![
        make_module(
            "Main",
            concat!(
                "Public Sub Bootstrap()\n",
                "End Sub\n",
                "Public Function Main() As String\n",
                "Dim widget As New Widget\n",
                "Dim item\n",
                "Dim valueOut\n",
                "For Each item In widget\n",
                "valueOut = valueOut & CStr(item) & \",\"\n",
                "Next item\n",
                "Main = valueOut\n",
                "End Function\n"
            ),
        ),
        make_class_module(
            "Widget",
            concat!(
                "Public Property Get NewEnum() As Variant\n",
                "NewEnum = Array(41, 42)\n",
                "End Property\n",
                "Attribute NewEnum.VB_UserMemId = -4\n",
                "Attribute NewEnum.VB_MemberFlags = \"40\"\n"
            ),
        ),
    ]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();
    let result = engine
        .invoke_procedure(&mut session, "Main", "Main", &[])
        .unwrap();

    assert_eq!(result, RuntimeValue::String(BStr("41,42,".to_string())));
}

#[cfg(target_os = "windows")]
fn run_imported_com_newenum_foreach(enable_jit: bool) -> RuntimeValue {
    let manifest = make_source_manifest_with_reference(
        "OxVba",
        vec![make_module(
            "Main",
            concat!(
                "Public Function Main() As String\n",
                "Dim obj As New OxVba.TestDispatch\n",
                "Dim item\n",
                "Dim valueOut\n",
                "For Each item In obj\n",
                "valueOut = valueOut & CStr(item) & \",\"\n",
                "Next item\n",
                "Main = valueOut\n",
                "End Function\n"
            ),
        )],
    );

    let mut engine = Engine::new(HostConfig {
        enable_jit,
        root_object_name: None,
    });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();
    engine
        .invoke_procedure(&mut session, "Main", "Main", &[])
        .unwrap()
}

#[cfg(target_os = "windows")]
fn run_imported_com_newenum_foreach_bundle(enable_jit: bool) -> RuntimeValue {
    let manifest = make_source_manifest_with_reference(
        "OxVba",
        vec![make_module(
            "Main",
            concat!(
                "Public Function Main() As String\n",
                "Dim obj As New OxVba.TestDispatch\n",
                "Dim item\n",
                "Dim valueOut\n",
                "For Each item In obj\n",
                "valueOut = valueOut & CStr(item) & \",\"\n",
                "Next item\n",
                "Main = valueOut\n",
                "End Function\n"
            ),
        )],
    );

    let compiled = compile_project(&manifest).unwrap();
    let bundle = OxBundle::from_compiled_project(&compiled, &manifest.project_name);
    let mut engine = Engine::new(HostConfig {
        enable_jit,
        root_object_name: None,
    });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let mut session = engine
        .compile_and_prepare_session_from_bundle(&bundle)
        .unwrap();
    engine
        .invoke_procedure(&mut session, "Main", "Main", &[])
        .unwrap()
}

#[cfg(target_os = "windows")]
#[test]
fn invoke_function_foreach_over_imported_com_newenum_executes() {
    let result = run_imported_com_newenum_foreach(false);

    assert_eq!(result, RuntimeValue::String(BStr("41,42,".to_string())));
}

#[cfg(target_os = "windows")]
#[test]
fn invoke_function_foreach_over_imported_com_newenum_vm_jit_snapshots_match() {
    let vm = run_imported_com_newenum_foreach(false);
    let jit = run_imported_com_newenum_foreach(true);

    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported COM NewEnum direct session invocation"
    );
    assert_eq!(vm, RuntimeValue::String(BStr("41,42,".to_string())));
}

#[cfg(target_os = "windows")]
#[test]
fn invoke_function_foreach_over_imported_com_newenum_bundle_vm_jit_snapshots_match() {
    let vm = run_imported_com_newenum_foreach_bundle(false);
    let jit = run_imported_com_newenum_foreach_bundle(true);

    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported COM NewEnum bundle session invocation"
    );
    assert_eq!(vm, RuntimeValue::String(BStr("41,42,".to_string())));
}

#[cfg(target_os = "windows")]
fn run_registered_testdispatch_foreach(enable_jit: bool) -> RuntimeValue {
    let manifest = make_manifest(vec![make_module(
        "Main",
        concat!(
            "Public Function Main() As String\n",
            "Dim obj\n",
            "Dim item\n",
            "Dim valueOut\n",
            "obj = CreateObject(\"OxVba.TestDispatch\")\n",
            "For Each item In obj\n",
            "valueOut = valueOut & CStr(item) & \",\"\n",
            "Next item\n",
            "Main = valueOut\n",
            "End Function\n"
        ),
    )]);

    let mut engine = Engine::new(HostConfig {
        enable_jit,
        root_object_name: None,
    });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();
    engine
        .invoke_procedure(&mut session, "Main", "Main", &[])
        .unwrap()
}

#[cfg(target_os = "windows")]
fn run_registered_testdispatch_foreach_bundle(enable_jit: bool) -> RuntimeValue {
    let manifest = make_manifest(vec![make_module(
        "Main",
        concat!(
            "Public Function Main() As String\n",
            "Dim obj\n",
            "Dim item\n",
            "Dim valueOut\n",
            "obj = CreateObject(\"OxVba.TestDispatch\")\n",
            "For Each item In obj\n",
            "valueOut = valueOut & CStr(item) & \",\"\n",
            "Next item\n",
            "Main = valueOut\n",
            "End Function\n"
        ),
    )]);

    let compiled = compile_project(&manifest).unwrap();
    let bundle = OxBundle::from_compiled_project(&compiled, &manifest.project_name);
    let mut engine = Engine::new(HostConfig {
        enable_jit,
        root_object_name: None,
    });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let mut session = engine
        .compile_and_prepare_session_from_bundle(&bundle)
        .unwrap();
    engine
        .invoke_procedure(&mut session, "Main", "Main", &[])
        .unwrap()
}

#[cfg(target_os = "windows")]
#[test]
fn invoke_function_foreach_over_registered_testdispatch_executes() {
    let result = run_registered_testdispatch_foreach(false);

    assert_eq!(result, RuntimeValue::String(BStr("41,42,".to_string())));
}

#[cfg(target_os = "windows")]
#[test]
fn invoke_function_foreach_over_registered_testdispatch_vm_jit_snapshots_match() {
    let vm = run_registered_testdispatch_foreach(false);
    let jit = run_registered_testdispatch_foreach(true);

    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for registered OxVba.TestDispatch direct session invocation"
    );
    assert_eq!(vm, RuntimeValue::String(BStr("41,42,".to_string())));
}

#[cfg(target_os = "windows")]
#[test]
fn invoke_function_foreach_over_registered_testdispatch_bundle_vm_jit_snapshots_match() {
    let vm = run_registered_testdispatch_foreach_bundle(false);
    let jit = run_registered_testdispatch_foreach_bundle(true);

    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for registered OxVba.TestDispatch bundle session invocation"
    );
    assert_eq!(vm, RuntimeValue::String(BStr("41,42,".to_string())));
}

// ---------------------------------------------------------------------------
// Create class instance: missing class returns error
// ---------------------------------------------------------------------------

#[test]
fn create_class_instance_missing_class_is_error() {
    let source = "Public Sub Hello()\nEnd Sub\n";
    let manifest = make_manifest(vec![make_module("Mod1", source)]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();

    let result = engine.create_class_instance(&mut session, "NonExistent");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("class not found"), "got: {err}");
}

// ---------------------------------------------------------------------------
// Invoke member on object: missing object returns error
// ---------------------------------------------------------------------------

#[test]
fn invoke_member_missing_object_is_error() {
    let source = "Public Sub Hello()\nEnd Sub\n";
    let manifest = make_manifest(vec![make_module("Mod1", source)]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();

    let result = engine.invoke_member_on_object(
        &mut session,
        oxvba_runtime::ObjectRef::from_compat_identity(999),
        "DoWork",
        &[],
    );
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not found"), "got: {err}");
}

// ---------------------------------------------------------------------------
// End-to-end class instantiation + member invocation with actual return values
// ---------------------------------------------------------------------------

fn make_class_module(name: &str, source: &str) -> ModuleUnit {
    ModuleUnit {
        module_name: name.to_string(),
        module_kind: ModuleKind::Class,
        attributes: ModuleAttributes {
            vb_name: name.to_string(),
            vb_exposed: true,
            vb_creatable: true,
            ..Default::default()
        },
        source: source.to_string(),
    }
}

#[test]
fn create_class_and_invoke_member_returns_value() {
    let class_source = concat!(
        "Public Function Add(a As Long, b As Long) As Long\n",
        "Add = a + b\n",
        "End Function\n",
    );

    // The compiler only emits dynamic object routes for classes that are
    // referenced via `New ClassName` in the source. A driver module that
    // uses `Dim x As New Calculator` causes the binding to be registered.
    let driver_source = concat!(
        "Public Sub Main()\n",
        "Dim c As New Calculator\n",
        "End Sub\n",
    );

    let manifest = make_manifest(vec![
        make_module("Driver", driver_source),
        make_class_module("Calculator", class_source),
    ]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();

    let handle = engine
        .create_class_instance(&mut session, "Calculator")
        .unwrap();

    let result = engine
        .invoke_member_on_object(
            &mut session,
            handle,
            "Add",
            &[RuntimeValue::I32(10), RuntimeValue::I32(32)],
        )
        .unwrap();

    match result {
        RuntimeValue::I32(v) => assert_eq!(v, 42),
        other => panic!("Expected I32(42), got {other:?}"),
    }
}

#[test]
fn create_class_and_invoke_sub_member() {
    let class_source = concat!(
        "Private mCount As Long\n",
        "Public Sub Increment()\nmCount = mCount + 1\nEnd Sub\n",
        "Public Function GetCount() As Long\nGetCount = mCount\nEnd Function\n",
    );

    // Driver uses `New Counter` to trigger dynamic object route registration
    let driver_source = concat!("Public Sub Main()\n", "Dim c As New Counter\n", "End Sub\n",);

    let manifest = make_manifest(vec![
        make_module("Driver", driver_source),
        make_class_module("Counter", class_source),
    ]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();

    let handle = engine
        .create_class_instance(&mut session, "Counter")
        .unwrap();

    // Invoke Increment (Sub — returns Empty)
    let result = engine
        .invoke_member_on_object(&mut session, handle.clone(), "Increment", &[])
        .unwrap();
    assert_eq!(result, RuntimeValue::Empty);

    // Invoke GetCount (Function — should return 1 after one increment)
    let result = engine
        .invoke_member_on_object(&mut session, handle, "GetCount", &[])
        .unwrap();
    match result {
        RuntimeValue::I32(v) => assert_eq!(v, 1),
        other => panic!("Expected I32(1), got {other:?}"),
    }
}
