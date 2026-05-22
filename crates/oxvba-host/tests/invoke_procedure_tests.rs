//! Tests for Engine::invoke_procedure_with_variants, create_class_instance, invoke_member_on_object_with_variants.

use std::collections::BTreeMap;

use oxvba_compiler::{
    DeclareParamType, ModuleAttributes, ModuleKind, ModuleUnit, OxBundle, ProjectKind,
    ProjectManifest, ProjectReference, ReferenceKind, compile_project,
};
use oxvba_hal::model::HostPolicy;
use oxvba_host::{
    Engine, HostConfig, HostUdfCallContext, HostUdfTypeMapEvidence, HostUdfTypedValue,
};
use oxvba_runtime::{VarType, Variant, bstr::BStr};

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
        .invoke_procedure_with_variants(&mut session, "Mod1", "DoWork", &[])
        .unwrap();

    assert_eq!(result, Variant::empty());
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
        .invoke_procedure_with_variants(&mut session, "Mod1", "GetValue", &[])
        .unwrap();

    // The return value should be 42
    assert_eq!(result.as_i32(), Some(42));
}

#[test]
fn invoke_function_clears_return_slot_between_repeated_calls() {
    let source = "Public Function Accum() As Long\nAccum = Accum + 1\nEnd Function\n";
    let manifest = make_manifest(vec![make_module("Mod1", source)]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();

    let first = engine
        .invoke_procedure_with_variants(&mut session, "Mod1", "Accum", &[])
        .unwrap();
    let second = engine
        .invoke_procedure_with_variants(&mut session, "Mod1", "Accum", &[])
        .unwrap();

    assert_eq!(first, Variant::from_i32(1));
    assert_eq!(second, Variant::from_i32(1));
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

    let result = engine.invoke_procedure_with_variants(&mut session, "Mod1", "TakeTwo", &[]);
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

    let result = engine.invoke_procedure_with_variants(&mut session, "Mod1", "NonExistent", &[]);
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
        .invoke_procedure_with_variants(&mut session, "Mod1", "Double", &[Variant::from_i32(5)])
        .unwrap();
    assert_eq!(r1.as_i32(), Some(10));

    let r2 = engine
        .invoke_procedure_with_variants(&mut session, "Mod1", "Triple", &[Variant::from_i32(7)])
        .unwrap();
    assert_eq!(r2.as_i32(), Some(21));
}

#[test]
fn host_udf_catalog_exposes_public_procedural_functions_with_policy() {
    let manifest = make_manifest(vec![
        make_module(
            "Main",
            concat!(
                "Public Function HostAdd(a As Long, b As Long) As Long\n",
                "HostAdd = a + b\n",
                "End Function\n",
                "Public Sub Helper()\n",
                "End Sub\n",
                "Private Function Hidden() As Long\n",
                "Hidden = 1\n",
                "End Function\n"
            ),
        ),
        make_class_module(
            "Widget",
            "Public Function ClassAdd(a As Long, b As Long) As Long\nClassAdd = a + b\nEnd Function\n",
        ),
    ]);

    let engine = Engine::default();
    let session = engine.compile_and_prepare_session(&manifest).unwrap();
    let catalog = engine.host_udf_catalog(&session);

    assert_eq!(catalog.functions.len(), 1);
    let host_add = &catalog.functions[0];
    assert_eq!(host_add.module_name, "main");
    assert_eq!(host_add.procedure_name, "hostadd");
    assert_eq!(
        host_add.stable_host_call_id,
        "host-call:testproj:main:hostadd:function"
    );
    assert_eq!(
        host_add.registration_identity.source_system,
        "oxvba-project"
    );
    assert!(
        host_add
            .registration_identity
            .source_fingerprint
            .starts_with("oxvba-fp-")
    );
    assert!(
        host_add
            .registration_identity
            .unregister_key
            .contains(&host_add.callable_metadata.descriptor_fingerprint)
    );
    assert_eq!(host_add.callable_metadata.worksheet_visible_name, "hostadd");
    assert_eq!(host_add.callable_metadata.export_kind, "function");
    assert_eq!(host_add.callable_metadata.arity, 2);
    assert_eq!(
        host_add.callable_metadata.parameter_type_text,
        vec![Some("Long".to_string()), Some("Long".to_string())]
    );
    assert_eq!(
        host_add.callable_metadata.return_type_text,
        Some("Long".to_string())
    );
    assert_eq!(
        host_add.invocation_target.stable_host_call_id,
        host_add.stable_host_call_id
    );
    assert_eq!(
        host_add.invocation_target.runtime_profile,
        "prepared-project-session"
    );
    assert_eq!(
        host_add.invocation_target.argument_conversion_lane,
        "variant-host-udf-arguments"
    );
    assert_eq!(
        host_add.invocation_target.result_conversion_lane,
        "variant-host-udf-result"
    );
    assert_eq!(
        host_add.invocation_target.diagnostic_projection_lane,
        "phase-diagnostic"
    );
    assert_eq!(host_add.arguments.len(), 2);
    assert_eq!(host_add.arguments[0].name.as_deref(), Some("a"));
    assert_eq!(
        host_add.arguments[0].value_type,
        Some(DeclareParamType::Long)
    );
    assert_eq!(host_add.arguments[1].name.as_deref(), Some("b"));
    assert_eq!(
        host_add.arguments[1].value_type,
        Some(DeclareParamType::Long)
    );
    assert_eq!(host_add.return_type, Some(DeclareParamType::Long));
    assert!(!host_add.volatile);
    assert_eq!(host_add.dependency_policy, "explicit-arguments-only");
    assert_eq!(host_add.side_effect_policy, "no-host-side-effects");
    assert_eq!(
        host_add.thread_safety_policy,
        "single-threaded-vba-compatible"
    );
    assert_eq!(
        host_add.allowed_contexts,
        vec![
            "worksheet-cell".to_string(),
            "host-formula-evaluator".to_string()
        ]
    );
    assert_eq!(
        host_add.capability_constraints.allowed_contexts,
        host_add.allowed_contexts
    );
    assert_eq!(
        host_add.capability_constraints.supported_value_subsets,
        vec![
            "variant-scalar-first-tier".to_string(),
            "typed-double-first-slice".to_string()
        ]
    );
    assert_eq!(
        host_add.change_signal_inputs,
        vec![
            "project-load".to_string(),
            "project-unload".to_string(),
            "module-edit".to_string(),
            "function-admission-change".to_string(),
            "descriptor-fingerprint-change".to_string()
        ]
    );
}

#[test]
fn host_udf_invoke_runs_public_function_with_caller_context() {
    let manifest = make_manifest(vec![make_module(
        "Main",
        "Public Function HostAdd(a As Long, b As Long) As Long\nHostAdd = a + b\nEnd Function\n",
    )]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();
    let catalog = engine.host_udf_catalog(&session);
    let host_add = &catalog.functions[0];

    let result = engine
        .invoke_host_udf_with_variants(
            &mut session,
            &host_add.stable_host_call_id,
            HostUdfCallContext::new()
                .with_caller("Sheet1!A1")
                .with_locale(1033)
                .with_dependency("Sheet1!B1:C1")
                .with_volatile_requested(true),
            &[Variant::from_i32(2), Variant::from_i32(5)],
        )
        .unwrap();

    assert_eq!(result.value.as_i32(), Some(7));
    assert_eq!(result.caller.as_deref(), Some("Sheet1!A1"));
    assert!(result.volatile_requested);
    assert_eq!(result.dependency_tokens, vec!["Sheet1!B1:C1".to_string()]);
}

#[test]
fn typed_host_udf_signature_and_invoke_admit_double_first_slice() {
    let manifest = make_manifest(vec![make_module(
        "Main",
        concat!(
            "Public Function AddThem(val1 As Double, val2 As Double) As Double\n",
            "AddThem = val1 + val2\n",
            "End Function\n"
        ),
    )]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();
    let catalog = engine.host_udf_catalog(&session);
    let add_them = &catalog.functions[0];

    let signature = engine
        .host_udf_typed_signature(
            &session,
            &add_them.stable_host_call_id,
            HostUdfTypeMapEvidence::ExcelObserved,
        )
        .unwrap();
    assert_eq!(signature.procedure_name, "addthem");
    assert_eq!(signature.return_type, DeclareParamType::Double);
    assert_eq!(signature.evidence, HostUdfTypeMapEvidence::ExcelObserved);
    assert_eq!(signature.parameters.len(), 2);
    assert_eq!(signature.parameters[0].name.as_deref(), Some("val1"));
    assert_eq!(signature.parameters[0].vba_type, DeclareParamType::Double);
    assert_eq!(signature.parameters[1].name.as_deref(), Some("val2"));
    assert_eq!(signature.parameters[1].vba_type, DeclareParamType::Double);

    let result = engine
        .invoke_host_udf_typed(
            &mut session,
            &signature,
            HostUdfCallContext::new().with_caller("Sheet1!A1"),
            &[
                HostUdfTypedValue::Double(2.0),
                HostUdfTypedValue::Double(3.0),
            ],
        )
        .unwrap();

    assert_eq!(result.value, HostUdfTypedValue::Double(5.0));
    assert_eq!(result.caller.as_deref(), Some("Sheet1!A1"));
}

#[test]
fn typed_host_udf_signature_rejects_non_double_first_slice() {
    let manifest = make_manifest(vec![make_module(
        "Main",
        "Public Function AddLongs(a As Long, b As Long) As Long\nAddLongs = a + b\nEnd Function\n",
    )]);

    let engine = Engine::default();
    let session = engine.compile_and_prepare_session(&manifest).unwrap();
    let catalog = engine.host_udf_catalog(&session);
    let add_longs = &catalog.functions[0];

    let err = engine
        .host_udf_typed_signature(
            &session,
            &add_longs.stable_host_call_id,
            HostUdfTypeMapEvidence::ExcelObserved,
        )
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("not admitted by the typed first slice"),
        "got: {err}"
    );
}

#[test]
fn typed_host_udf_invoke_rejects_rejected_type_map_evidence() {
    let manifest = make_manifest(vec![make_module(
        "Main",
        "Public Function AddThem(a As Double, b As Double) As Double\nAddThem = a + b\nEnd Function\n",
    )]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();
    let catalog = engine.host_udf_catalog(&session);
    let add_them = &catalog.functions[0];

    let signature = engine
        .host_udf_typed_signature(
            &session,
            &add_them.stable_host_call_id,
            HostUdfTypeMapEvidence::Rejected,
        )
        .unwrap();

    let err = engine
        .invoke_host_udf_typed(
            &mut session,
            &signature,
            HostUdfCallContext::new(),
            &[
                HostUdfTypedValue::Double(2.0),
                HostUdfTypedValue::Double(3.0),
            ],
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("marked rejected"), "got: {err}");
}

#[test]
fn host_udf_invoke_rejects_sub_exports() {
    let manifest = make_manifest(vec![make_module("Main", "Public Sub Helper()\nEnd Sub\n")]);

    let engine = Engine::default();
    let mut session = engine.compile_and_prepare_session(&manifest).unwrap();
    assert!(engine.host_udf_catalog(&session).functions.is_empty());

    let err = engine
        .invoke_host_udf_with_variants(
            &mut session,
            "host-call:testproj:main:helper:sub",
            HostUdfCallContext::new(),
            &[],
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("host UDF function not found"), "got: {err}");
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
        .invoke_procedure_with_variants(&mut session, "Main", "Main", &[])
        .unwrap();

    assert_eq!(result, Variant::from_string(BStr::from("41,42,")));
}

#[cfg(target_os = "windows")]
fn run_imported_com_newenum_foreach(enable_jit: bool) -> Variant {
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
        .invoke_procedure_with_variants(&mut session, "Main", "Main", &[])
        .unwrap()
}

#[cfg(target_os = "windows")]
fn run_imported_com_newenum_foreach_bundle(enable_jit: bool) -> Variant {
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
        .invoke_procedure_with_variants(&mut session, "Main", "Main", &[])
        .unwrap()
}

#[cfg(target_os = "windows")]
#[test]
fn invoke_function_foreach_over_imported_com_newenum_executes() {
    let result = run_imported_com_newenum_foreach(false);

    assert_eq!(result, Variant::from_string(BStr::from("41,42,")));
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
    assert_eq!(vm, Variant::from_string(BStr::from("41,42,")));
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
    assert_eq!(vm, Variant::from_string(BStr::from("41,42,")));
}

#[cfg(target_os = "windows")]
fn run_registered_testdispatch_foreach(enable_jit: bool) -> Variant {
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
        .invoke_procedure_with_variants(&mut session, "Main", "Main", &[])
        .unwrap()
}

#[cfg(target_os = "windows")]
fn run_registered_testdispatch_foreach_bundle(enable_jit: bool) -> Variant {
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
        .invoke_procedure_with_variants(&mut session, "Main", "Main", &[])
        .unwrap()
}

#[cfg(target_os = "windows")]
#[test]
fn invoke_function_foreach_over_registered_testdispatch_executes() {
    let result = run_registered_testdispatch_foreach(false);

    assert_eq!(result, Variant::from_string(BStr::from("41,42,")));
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
    assert_eq!(vm, Variant::from_string(BStr::from("41,42,")));
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
    assert_eq!(vm, Variant::from_string(BStr::from("41,42,")));
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

    let result = engine.invoke_member_on_object_with_variants(
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
        .invoke_member_on_object_with_variants(
            &mut session,
            handle.clone(),
            "Add",
            &[Variant::from_i32(10), Variant::from_i32(32)],
        )
        .unwrap();

    assert_eq!(result.as_i32(), Some(42));

    let variant_result = engine
        .invoke_member_on_object_with_variants(
            &mut session,
            handle,
            "Add",
            &[Variant::from_i32(10), Variant::from_i32(32)],
        )
        .unwrap();
    assert_eq!(variant_result.vtype(), VarType::Long);
    assert_eq!(variant_result.as_i32(), Some(42));
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
        .invoke_member_on_object_with_variants(&mut session, handle.clone(), "Increment", &[])
        .unwrap();
    assert_eq!(result, Variant::empty());

    // Invoke GetCount (Function — should return 1 after one increment)
    let result = engine
        .invoke_member_on_object_with_variants(&mut session, handle, "GetCount", &[])
        .unwrap();
    assert_eq!(result.as_i32(), Some(1));
}
