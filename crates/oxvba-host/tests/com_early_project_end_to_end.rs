use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use oxvba_compiler::{
    ModuleKind, OxBundle, ProjectKind, ProjectManifest, ProjectReference, ReferenceKind,
    compile_project, module_unit_from_source,
};
use oxvba_hal::{
    adapters::builder::HostBuilder,
    model::{ComInvocationStrategy, HostPolicy, native_host_profile},
};
use oxvba_host::engine::DiagnosticPhase;
use oxvba_host::{Engine, HostConfig};
use oxvba_project::load_basproj;
use oxvba_runtime::{ObjectRef, RuntimeInterfaceId, Variant};
use oxvba_vm::{Vm, VmExecutionPackage};

fn canonical_snapshot_objects() -> &'static Mutex<HashMap<i32, ObjectRef>> {
    static CANONICAL: OnceLock<Mutex<HashMap<i32, ObjectRef>>> = OnceLock::new();
    CANONICAL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn canonicalize_variant(value: Variant) -> Variant {
    if let Some(object) = value.as_object_ref() {
        let raw = object.raw();
        let canonical = canonical_snapshot_objects()
            .lock()
            .expect("canonical object snapshot map should not be poisoned")
            .entry(raw)
            .or_insert_with(|| object.clone())
            .clone();
        return Variant::from_object_ref(canonical);
    }
    if let Some(array) = value.as_safearray() {
        let array = match array.variant_elements() {
            Some(elements) => array
                .replace_variant_elements(elements.into_iter().map(canonicalize_variant).collect())
                .expect("canonical snapshot array rewrite should preserve SAFEARRAY shape"),
            None => array,
        };
        return Variant::from_safearray(array);
    }
    value
}

fn canonicalize_snapshot(values: Vec<Variant>) -> Vec<Variant> {
    values.into_iter().map(canonicalize_variant).collect()
}

fn manifest_with_reference(referenced_project_name: &str, main_source: &str) -> ProjectManifest {
    let main_module = module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
        .expect("main module should parse");
    ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module],
        references: vec![ProjectReference {
            referenced_project_name: referenced_project_name.to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    }
}

fn manifest_with_typelib(main_source: &str) -> ProjectManifest {
    manifest_with_reference("OxVba", main_source)
}

#[cfg(target_os = "windows")]
fn run_project_windows_hosted(manifest: &ProjectManifest, enable_jit: bool) -> Vec<Variant> {
    run_project_windows_hosted_with_policy(manifest, enable_jit, HostPolicy::interactive_dev())
}

#[cfg(target_os = "windows")]
fn run_project_windows_hosted_with_policy(
    manifest: &ProjectManifest,
    enable_jit: bool,
    policy: HostPolicy,
) -> Vec<Variant> {
    let _ = enable_jit;
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(policy);
    canonicalize_snapshot(
        engine
            .execute_project_with_variant_snapshot_phased(manifest)
            .expect("project should execute"),
    )
}

#[cfg(target_os = "windows")]
fn run_project_windows_hosted_error(manifest: &ProjectManifest, enable_jit: bool) -> String {
    let _ = enable_jit;
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let err = engine
        .execute_project_with_variant_snapshot_phased(manifest)
        .expect_err("project should fail deterministically");
    format!("{:?}: {}", err.phase(), err.message())
}

fn expect_object_handle(value: &Variant) -> ObjectRef {
    value
        .as_object_ref()
        .unwrap_or_else(|| panic!("expected object handle, got {:?}", value))
}

fn assert_same_object_identity(values: &[Variant], indices: &[usize], context: &str) {
    let first = expect_object_handle(&values[indices[0]]);
    for index in indices.iter().copied().skip(1) {
        let next = expect_object_handle(&values[index]);
        assert_eq!(
            first, next,
            "{context}: expected identical retained ObjectRef identity at indices {indices:?}, got values={values:?}"
        );
    }
}

#[test]
fn pure_oxvba_class_object_exposes_runtime_descriptor_metadata() {
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim widget As New Widget
Dim valueOut
valueOut = widget.Value
End Sub
"#,
    )
    .expect("main module should parse");
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Public Property Get Value()
Value = 41
End Property
Attribute Value.VB_UserMemId = 0
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let compiled = compile_project(&manifest).expect("project should compile");
    let mut vm = oxvba_vm::Vm::new(
        HostBuilder::new()
            .profile(native_host_profile())
            .policy(HostPolicy::deterministic_runtime())
            .build(),
    );
    vm.set_project_dynamic_objects(compiled.project_dynamic_objects.clone());
    let widget = vm
        .project_dynamic_object_ref(1)
        .expect("compiled New Widget should register a project dynamic object");
    let descriptor = widget.class_descriptor();
    assert_eq!(descriptor.name, "projecta.widget");
    let dispatch = widget
        .query_interface_descriptor(RuntimeInterfaceId::IDispatch)
        .expect("pure OxVba class object should expose descriptor-backed dispatch metadata");
    assert!(dispatch.dual_dispatch);
    let value = dispatch
        .members
        .iter()
        .find(|member| member.name.eq_ignore_ascii_case("Value"))
        .expect("Value member should be represented in descriptor metadata");
    assert_eq!(value.dispatch_id, 0);
    assert!(value.is_default_member);
    assert_eq!(value.arity, 0);
    let engine = Engine::new(HostConfig { enable_jit: false });
    let out = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("project should execute");
    assert_eq!(out[1], Variant::from_i32(41));
}

#[test]
fn pure_oxvba_class_no_paren_read_invokes_parameterless_function() {
    // VBA: `x = obj.Foo` (no parentheses) calls a *parameterless* Function, it is not only
    // a Property Get read. Regression for F3 get-or-call: the no-paren member-read rewrite
    // must probe a parameterless Function after Property Get, not silently yield Empty.
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim widget As New Widget
Dim valueOut
valueOut = widget.GetScore
End Sub
"#,
    )
    .expect("main module should parse");
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Public Function GetScore() As Long
GetScore = 42
End Function
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig { enable_jit: false });
    let out = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("project should execute");
    assert!(
        out.contains(&Variant::from_i32(42)),
        "no-paren read of a parameterless Function should invoke it; out={out:?}"
    );
}

#[test]
fn pure_oxvba_class_explicit_set_new_instantiates_and_dispatches() {
    // `Dim w As Widget : Set w = New Widget` (explicit project-class instantiation into a
    // typed receiver) must instantiate and then dispatch. Regression for F3a; combined with
    // the no-paren read (F3b) this is the everyday `Set`-then-use pattern.
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim w As Widget
Dim valueOut
Set w = New Widget
valueOut = w.GetScore
End Sub
"#,
    )
    .expect("main module should parse");
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Public Function GetScore() As Long
GetScore = 42
End Function
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig { enable_jit: false });
    let out = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("project should execute");
    assert!(
        out.contains(&Variant::from_i32(42)),
        "explicit `Set w = New Widget` then `w.GetScore` should yield 42; out={out:?}"
    );
}

#[test]
fn pure_oxvba_class_set_new_into_object_variable_instantiates_and_dispatches() {
    // `Dim c As Object : Set c = New Widget` — late-bound receiver. Regression for F3c(a):
    // the instance handle is lowered through `__oxvba_project_instance(...)` so it
    // type-checks into an `Object`-typed slot (a bare integer literal was rejected as
    // `cannot assign Long to Object`). At runtime the slot still holds the `i32` handle,
    // so the late-bound project-dynamic dispatch resolves `GetScore` to 42.
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim c As Object
Dim valueOut
Set c = New Widget
valueOut = c.GetScore
End Sub
"#,
    )
    .expect("main module should parse");
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Public Function GetScore() As Long
GetScore = 42
End Function
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig { enable_jit: false });
    let out = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("project should execute");
    assert!(
        out.contains(&Variant::from_i32(42)),
        "`Dim c As Object : Set c = New Widget` then `c.GetScore` should yield 42; out={out:?}"
    );
}

#[test]
fn pure_oxvba_class_new_instance_is_a_reference_counted_object() {
    // The instance handle is gone from the value model: `New <ProjectClass>` now yields a
    // real reference-counted Object reference in the slot, not an integer. `IsObject(c)`
    // therefore returns True (VBA-correct), and the aliasing `Set d = c` shares the same
    // reference so a mutation through one variable is visible through the other.
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim c As Widget
Dim d As Widget
Dim aliased
Set c = New Widget
Set d = c
c.SetScore 99
aliased = d.GetScore
End Sub
"#,
    )
    .expect("main module should parse");
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Private mScore As Long
Public Sub SetScore(ByVal value As Long)
mScore = value
End Sub
Public Function GetScore() As Long
GetScore = mScore
End Function
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig { enable_jit: false });
    let out = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("project should execute");
    // `New Widget` now lands a real Object reference in the slot (not an integer handle).
    assert!(
        out.iter().any(|v| v.as_object_ref().is_some()),
        "`Set c = New Widget` should leave a real Object reference in the slot, not an integer; out={out:?}"
    );
    // `Set d = c` shares that reference, so c.SetScore 99 is visible through d.GetScore.
    assert!(
        out.contains(&Variant::from_i32(99)),
        "`Set d = c` should alias the same instance, so d.GetScore reflects c.SetScore 99; out={out:?}"
    );
}

#[test]
fn pure_oxvba_class_distinct_new_instances_have_separate_state() {
    // Each `New Widget` is now a distinct IUnknown instance with its own field state, so two
    // instances do not alias (the prior compile-time-handle model shared one). `Set c = a`
    // still aliases a (shared reference). Proves the per-instance identity switch.
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim a As Widget
Dim b As Widget
Dim c As Widget
Dim aScore
Dim bScore
Dim cScore
Set a = New Widget
Set b = New Widget
Set c = a
a.SetScore 10
b.SetScore 20
c.SetScore 30
aScore = a.GetScore
bScore = b.GetScore
cScore = c.GetScore
End Sub
"#,
    )
    .expect("main module should parse");
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Private mScore As Long
Public Sub SetScore(ByVal value As Long)
mScore = value
End Sub
Public Function GetScore() As Long
GetScore = mScore
End Function
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig { enable_jit: false });
    let out = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("project should execute");
    // b is a distinct instance, so b.GetScore = 20 (not clobbered by a/c).
    assert!(
        out.contains(&Variant::from_i32(20)),
        "distinct instance b should keep its own score 20; out={out:?}"
    );
    // c aliases a, so c.SetScore 30 is visible through a → both read 30.
    assert!(
        out.iter().filter(|v| **v == Variant::from_i32(30)).count() >= 2,
        "c aliases a, so a.GetScore and c.GetScore both = 30; out={out:?}"
    );
}

#[test]
fn pure_oxvba_class_value_read_of_sub_is_expected_function_or_variable() {
    // F3c diagnostic-parity: `x = obj.SomeSub` reads a Sub in a value context. VBA raises a
    // compile-time error "Expected Function or variable"; OxVBA must raise an equivalent
    // diagnostic at the same point rather than silently yielding Empty.
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim w As New Widget
Dim valueOut
valueOut = w.DoThing
End Sub
"#,
    )
    .expect("main module should parse");
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Public Sub DoThing()
End Sub
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect_err("reading a Sub in a value context should be a compile error");
    assert!(
        err.message()
            .contains("PMR-E-MEMBER-READ-EXPECTED-FUNCTION-OR-VARIABLE"),
        "expected Sub-in-value-context diagnostic; got {:?}: {}",
        err.phase(),
        err.message()
    );
}

#[test]
fn pure_oxvba_class_value_read_of_required_arg_function_is_argument_not_optional() {
    // F3c diagnostic-parity: `x = obj.NeedsArg` reads, without arguments, a Function that has
    // a required parameter. VBA raises a compile-time error "Argument not optional"; OxVBA
    // must raise an equivalent diagnostic rather than silently yielding Empty.
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim w As New Widget
Dim valueOut
valueOut = w.NeedsArg
End Sub
"#,
    )
    .expect("main module should parse");
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Public Function NeedsArg(ByVal factor As Long) As Long
NeedsArg = factor * 2
End Function
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect_err("reading a required-arg Function without arguments should be a compile error");
    assert!(
        err.message()
            .contains("PMR-E-MEMBER-READ-ARGUMENT-NOT-OPTIONAL"),
        "expected argument-not-optional diagnostic; got {:?}: {}",
        err.phase(),
        err.message()
    );
}

#[test]
fn pure_oxvba_class_statement_sub_call_and_all_optional_function_read_are_not_diagnosed() {
    // Negative guard for the F3c diagnostics: a *statement-form* Sub call (`obj.DoThing`) is a
    // valid call and must not be diagnosed (it routes to member dispatch). And a no-paren value
    // read of a Function whose parameters are all Optional (`x = obj.OptScore`) is get-or-called,
    // not flagged "Argument not optional" — covered by the relaxed no-arg-callable probe.
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim w As New Widget
Dim valueOut
w.DoThing
valueOut = w.OptScore
End Sub
"#,
    )
    .expect("main module should parse");
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Public Sub DoThing()
End Sub
Public Function OptScore(Optional ByVal bonus As Long) As Long
OptScore = 7 + bonus
End Function
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig { enable_jit: false });
    let out = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("statement-form Sub call and all-optional Function read should both be valid");
    assert!(
        out.contains(&Variant::from_i32(7)),
        "all-optional Function read should get-or-call and yield 7; out={out:?}"
    );
}

#[test]
fn pure_oxvba_variant_receiver_uses_descriptor_cache_for_default_indexed_and_properties() {
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim widget As New Widget
Dim child As New Child
Dim indexedOut
Dim defaultOut
Dim ignored
Dim ignoredSet
Dim afterLet
Dim afterSet
indexedOut = DispatchInvoke(widget, "Value", 5)
defaultOut = widget(5)
ignored = DispatchInvoke(widget, "Stored", 46)
afterLet = DispatchInvoke(widget, "Observe")
ignoredSet = DispatchInvoke(widget, "Kid", child)
afterSet = DispatchInvoke(widget, "Observe")
End Sub
"#,
    )
    .expect("main module should parse");
    let widget_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Private stored
Public Sub Class_Initialize()
stored = 3
End Sub
Public Property Get Value(ByVal index)
Value = stored + index
End Property
Attribute Value.VB_UserMemId = 0
Public Property Let Stored(ByVal n)
stored = n
End Property
Public Property Set Kid(ByRef target)
stored = 17
End Property
Public Property Get Observe()
Observe = stored
End Property
"#,
    )
    .expect("widget module should parse");
    let child_module = module_unit_from_source(
        "Child",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Child"
Public Property Get Value()
Value = 1
End Property
"#,
    )
    .expect("child module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, widget_module, child_module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let compiled = compile_project(&manifest).expect("project should compile");
    let widget_handle = compiled
        .project_dynamic_objects
        .iter()
        .find(|route| route.module_name.eq_ignore_ascii_case("Widget"))
        .expect("Widget route should exist")
        .object_handle;
    let mut vm = Vm::new(
        HostBuilder::new()
            .profile(native_host_profile())
            .policy(HostPolicy::deterministic_runtime())
            .build(),
    );
    vm.set_project_procedure_runtime_metadata(compiled.procedure_runtime_metadata.clone());
    vm.set_project_com_withevents_routes(compiled.project_com_withevents_routes.clone());
    vm.set_project_dynamic_objects(compiled.project_dynamic_objects.clone());
    assert!(
        vm.resolve_project_dynamic_unhinted_dispatch_plan_for_test(widget_handle, "Value", 1)
            .is_some(),
        "indexed Value get should have a unique descriptor plan"
    );
    assert!(
        vm.resolve_project_dynamic_unhinted_dispatch_plan_for_test(widget_handle, "Stored", 1)
            .is_some(),
        "Property Let should have a unique unhinted descriptor plan by arity"
    );
    assert!(
        vm.resolve_project_dynamic_unhinted_dispatch_plan_for_test(widget_handle, "Observe", 0)
            .is_some(),
        "Property Get should have a unique unhinted descriptor plan by arity"
    );
    let child_set_plan =
        vm.resolve_project_dynamic_unhinted_dispatch_plan_for_test(widget_handle, "Kid", 1);
    assert!(
        child_set_plan.is_some(),
        "Property Set should have a unique unhinted descriptor plan by arity; route={:?}",
        compiled.project_dynamic_objects
    );
    assert!(
        vm.project_dynamic_dispatch_cache_len_for_test(widget_handle) >= 4,
        "descriptor cache should retain unhinted Value get, Stored let, Kid set, and Observe get plans"
    );
    let bundle = OxBundle::from_compiled_project_with_manifest(&compiled, &manifest);
    let package = VmExecutionPackage::from_bundle(&bundle);
    vm.execute_package(&package)
        .expect("project should execute pure OxVba indexed/property paths");
    let snapshot = vm.snapshot_variants(compiled.bytecode.slot_count);
    assert!(
        snapshot.contains(&Variant::from_i32(8)),
        "indexed default get should return stored + index"
    );
    assert!(
        snapshot.contains(&Variant::from_i32(46)),
        "indexed default let should update stored through the dynamic descriptor path"
    );
    assert!(
        snapshot.contains(&Variant::from_i32(17)),
        "property set should update stored through the dynamic descriptor path"
    );
    assert!(
        vm.project_dynamic_dispatch_cache_len_for_test(widget_handle) >= 4,
        "descriptor cache should remain populated after project execution"
    );
}

#[test]
fn pure_oxvba_interface_receiver_executes_through_project_descriptor_shape() {
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim widget As New Widget
Dim iface As IWidget
Dim valueOut
Set iface = widget
valueOut = iface.Value(5)
End Sub
"#,
    )
    .expect("main module should parse");
    let interface_module = module_unit_from_source(
        "IWidget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "IWidget"
Public Property Get Value(ByVal index)
End Property
"#,
    )
    .expect("interface module should parse");
    let widget_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Implements IWidget
Private Property Get IWidget_Value(ByVal index)
IWidget_Value = index + 7
End Property
"#,
    )
    .expect("widget module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, interface_module, widget_module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig { enable_jit: false });
    let out = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("pure OxVba interface receiver project should execute");
    assert!(
        out.contains(&Variant::from_i32(12)),
        "interface receiver call should return implementation result; out={out:?}"
    );
}

#[cfg(target_os = "windows")]
fn registered_scripting_dictionary_available() -> bool {
    let manifest = manifest_with_reference(
        "Scripting",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New Scripting.Dictionary
Dim countValue
countValue = obj.Count
End Sub
"#,
    );
    std::panic::catch_unwind(|| run_project_windows_hosted(&manifest, false))
        .ok()
        .and_then(|snapshot| snapshot.first().cloned())
        .map(|value| expect_object_handle(&value).raw() >= 20_001)
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn registered_scripting_filesystemobject_available() -> bool {
    let manifest = manifest_with_reference(
        "Scripting",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New Scripting.FileSystemObject
Dim extValue
extValue = obj.GetExtensionName("C:\temp\demo.txt")
End Sub
"#,
    );
    std::panic::catch_unwind(|| run_project_windows_hosted(&manifest, false))
        .ok()
        .and_then(|snapshot| snapshot.first().cloned())
        .map(|value| expect_object_handle(&value).raw() >= 20_001)
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn ado_typelib_path(file_name: &str) -> Option<String> {
    let candidates = [
        std::env::var_os("ProgramFiles").map(std::path::PathBuf::from),
        std::env::var_os("ProgramFiles(x86)").map(std::path::PathBuf::from),
        std::env::var_os("CommonProgramFiles").map(std::path::PathBuf::from),
        std::env::var_os("CommonProgramFiles(x86)").map(std::path::PathBuf::from),
    ];
    candidates
        .into_iter()
        .flatten()
        .flat_map(|root| {
            [
                root.join("Common Files")
                    .join("System")
                    .join("ado")
                    .join(file_name),
                root.join("System").join("ado").join(file_name),
                root.join("ado").join(file_name),
            ]
        })
        .find(|path| path.exists())
        .map(|path| path.to_string_lossy().into_owned())
}

#[cfg(target_os = "windows")]
fn registered_access_jet_ado_available() -> Option<(String, String)> {
    let adodb = ado_typelib_path("msado15.dll")?;
    let adox = ado_typelib_path("msadox.dll")?;
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-access-jet-ado-availability",
        r#"
Attribute VB_Name = "Main"
Public Sub Main()
Dim cn As New ADODB.Connection
Dim stateValue
stateValue = cn.State
End Sub
"#,
        &[BasprojComRefSpec {
            include: "ADODB",
            guid: None,
            major: None,
            minor: None,
            lcid: None,
            importlib: Some(adodb.as_str()),
        }],
    );
    if std::panic::catch_unwind(|| run_project_windows_hosted(&loaded.manifest, false)).is_err() {
        return None;
    }
    Some((adodb, adox))
}

#[cfg(target_os = "windows")]
fn registered_testdispatch_available() -> bool {
    let manifest = manifest_with_reference(
        "OxVba",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj
obj = CreateObject("OxVba.TestDispatch")
End Sub
"#,
    );
    std::panic::catch_unwind(|| run_project_windows_hosted(&manifest, false))
        .ok()
        .and_then(|snapshot| snapshot.first().cloned())
        .map(|value| expect_object_handle(&value).raw() >= 20_001)
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn registered_testeventserver_available() -> bool {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestEventServer
Dim value
value = obj.Ping()
End Sub
"#,
    );
    std::panic::catch_unwind(|| run_project_windows_hosted(&manifest, false))
        .ok()
        .and_then(|snapshot| snapshot.first().cloned())
        .map(|value| expect_object_handle(&value).raw() >= 20_001)
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
struct BasprojComRefSpec<'a> {
    include: &'a str,
    guid: Option<&'a str>,
    major: Option<u16>,
    minor: Option<u16>,
    lcid: Option<u32>,
    importlib: Option<&'a str>,
}

#[cfg(target_os = "windows")]
fn workspace_root_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
        })
}

#[cfg(target_os = "windows")]
fn run_wrapped_com_server_build(
    server_basproj_path: &std::path::Path,
    dll_path: &std::path::Path,
) -> Result<(), String> {
    let output = std::process::Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("-p")
        .arg("oxvba-cli")
        .arg("--bin")
        .arg("oxvba-cli")
        .arg("--")
        .arg("build")
        .arg(server_basproj_path)
        .arg("-o")
        .arg(dll_path)
        .current_dir(workspace_root_dir())
        .output()
        .map_err(|err| format!("failed to start wrapped COM server build command: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "wrapped COM server build failed with status {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn run_regsvr32(args: &[&str], dll_path: &std::path::Path) -> Result<(), String> {
    let output = std::process::Command::new("regsvr32")
        .args(args)
        .arg(dll_path)
        .output()
        .map_err(|err| format!("failed to start regsvr32: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "regsvr32 {:?} failed for {} with status {}: {}",
            args,
            dll_path.display(),
            output.status,
            stderr.trim()
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
struct RegisteredComServerGuard {
    dll_path: std::path::PathBuf,
}

#[cfg(target_os = "windows")]
impl RegisteredComServerGuard {
    fn register(dll_path: std::path::PathBuf) -> Result<Self, String> {
        run_regsvr32(&["/s"], &dll_path)?;
        Ok(Self { dll_path })
    }
}

#[cfg(target_os = "windows")]
impl Drop for RegisteredComServerGuard {
    fn drop(&mut self) {
        let _ = run_regsvr32(&["/s", "/u"], &self.dll_path);
    }
}

#[cfg(target_os = "windows")]
fn load_typelib_basproj_with_ref_specs(
    temp_leaf: &str,
    main_source: &str,
    com_refs: &[BasprojComRefSpec<'_>],
) -> oxvba_project::LoadedProject {
    let unique_leaf = format!(
        "{}-{}-{:?}-{}",
        temp_leaf,
        std::process::id(),
        std::thread::current().id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    );
    let temp_root = std::env::current_dir()
        .expect("cwd")
        .join("temp")
        .join(unique_leaf);
    std::fs::create_dir_all(&temp_root).expect("create temp root");

    let basproj_path = temp_root.join("ProjectA.basproj");
    let main_path = temp_root.join("Main.bas");
    std::fs::write(&main_path, main_source).expect("write main module");

    let mut basproj = String::from(
        "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n\
  <PropertyGroup>\n\
    <OutputType>Exe</OutputType>\n\
    <ProjectName>ProjectA</ProjectName>\n\
    <EntryPoint>Main.Main</EntryPoint>\n\
  </PropertyGroup>\n\
  <ItemGroup>\n\
    <Module Include=\"Main.bas\" />\n\
  </ItemGroup>\n\
  <ItemGroup>\n",
    );
    for com_ref in com_refs {
        basproj.push_str(&format!(
            "    <COMReference Include=\"{}\">\n",
            com_ref.include
        ));
        if let Some(guid) = com_ref.guid {
            basproj.push_str(&format!("      <Guid>{guid}</Guid>\n"));
        }
        if let Some(major) = com_ref.major {
            basproj.push_str(&format!("      <VersionMajor>{major}</VersionMajor>\n"));
        }
        if let Some(minor) = com_ref.minor {
            basproj.push_str(&format!("      <VersionMinor>{minor}</VersionMinor>\n"));
        }
        if let Some(lcid) = com_ref.lcid {
            basproj.push_str(&format!("      <Lcid>{lcid}</Lcid>\n"));
        }
        if let Some(importlib) = com_ref.importlib {
            basproj.push_str(&format!("      <ImportLib>{importlib}</ImportLib>\n"));
        }
        basproj.push_str("    </COMReference>\n");
    }
    basproj.push_str("  </ItemGroup>\n</Project>\n");

    std::fs::write(&basproj_path, basproj).expect("write basproj");
    load_basproj(&basproj_path).expect("basproj should load")
}

#[cfg(target_os = "windows")]
fn load_typelib_basproj_with_refs(
    temp_leaf: &str,
    main_source: &str,
    com_refs: &[(&str, &str, u16, u16, u32)],
) -> oxvba_project::LoadedProject {
    let specs = com_refs
        .iter()
        .map(|(include, guid, major, minor, lcid)| BasprojComRefSpec {
            include,
            guid: Some(*guid),
            major: Some(*major),
            minor: Some(*minor),
            lcid: Some(*lcid),
            importlib: Some(match *include {
                "OxVba" => "OxVba.TestEventServer.tlb",
                "OxVbaAlt" => "OxVba.TestEventServerAlt.tlb",
                "OxVbaAlt2" => "OxVba.TestEventServerAlt2.tlb",
                other => panic!("unexpected COMReference include `{other}`"),
            }),
        })
        .collect::<Vec<_>>();
    load_typelib_basproj_with_ref_specs(temp_leaf, main_source, &specs)
}

#[cfg(target_os = "windows")]
#[test]
fn wrapped_com_server_build_register_and_early_bind_interface_addthem() {
    let unique = format!(
        "wrapped-com-server-imycalc-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    );
    let temp_root = std::env::current_dir()
        .expect("cwd")
        .join("temp")
        .join(unique);
    std::fs::create_dir_all(&temp_root).expect("create temp root");

    let interface_source = r#"
Attribute VB_Name = "IMyCalc"
Option Explicit
Public Function AddThem(ByVal leftValue As Double, ByVal rightValue As Double) As Double
End Function
"#;
    let class_source = r#"
Attribute VB_Name = "MyCalc"
Option Explicit
Implements IMyCalc
Private Function IMyCalc_AddThem(ByVal leftValue As Double, ByVal rightValue As Double) As Double
IMyCalc_AddThem = leftValue + rightValue
End Function
"#;
    std::fs::write(temp_root.join("IMyCalc.cls"), interface_source).expect("write IMyCalc.cls");
    std::fs::write(temp_root.join("MyCalc.cls"), class_source).expect("write MyCalc.cls");

    let server_basproj_path = temp_root.join("MyCalcServer.basproj");
    let server_basproj = r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>ComServer</OutputType>
    <BuildTarget>WrappedComServer</BuildTarget>
    <ProjectName>MyCalcServer</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <ClassModule Include="IMyCalc.cls" />
    <ClassModule Include="MyCalc.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <ProgId>MyCalcServerLib.MyCalc</ProgId>
    </ClassModule>
  </ItemGroup>
</Project>
"#;
    std::fs::write(&server_basproj_path, server_basproj).expect("write server basproj");

    let server_dll_path = temp_root.join("MyCalcServer.dll");
    run_wrapped_com_server_build(&server_basproj_path, &server_dll_path)
        .expect("build wrapped COM server");
    let server_tlb_path = server_dll_path.with_extension("tlb");
    assert!(
        server_dll_path.exists(),
        "expected wrapped COM server DLL {}",
        server_dll_path.display()
    );
    assert!(
        server_tlb_path.exists(),
        "expected wrapped COM server TypeLib {}",
        server_tlb_path.display()
    );

    let tlb_identity = oxvba_com::TypeLibResolvedIdentity {
        reference_name: "MyCalcServer".to_string(),
        requested_coclass: Some("MyCalc".to_string()),
        importlib: server_tlb_path.to_string_lossy().to_string(),
        libid: None,
        major_version: 1,
        minor_version: 0,
        lcid: None,
        cache_key: "wrapped-com-server-imycalc-signature".to_string(),
    };
    let raw_tlib = oxvba_com::windows_typelib_loader::load_typelib_from_path(
        &server_tlb_path.to_string_lossy(),
    )
    .expect("load generated wrapped COM server typelib");
    let tlb_metadata =
        oxvba_com::windows_typelib_loader::build_metadata_blob_from_typelib(raw_tlib, tlb_identity)
            .expect("read generated wrapped COM server typelib metadata");
    unsafe { oxvba_com::windows_typelib_loader::release_typelib(raw_tlib) };
    let add_them = tlb_metadata
        .members
        .iter()
        .find(|member| member.name.eq_ignore_ascii_case("AddThem"))
        .expect("generated typelib should expose AddThem");
    assert_eq!(
        add_them.parameter_types,
        vec![
            oxvba_com::TypeLibParamType::Double,
            oxvba_com::TypeLibParamType::Double,
        ],
        "AddThem should expose two Double inputs, not generic VARIANTs"
    );
    assert_eq!(
        add_them.return_type,
        Some(oxvba_com::TypeLibParamType::Double),
        "AddThem should expose a Double retval, not a generic VARIANT"
    );

    let registration_guard =
        RegisteredComServerGuard::register(server_dll_path.clone()).expect("register DLL");

    let consumer_source = r#"
Attribute VB_Name = "Main"
Public Sub Main()
Dim calc As IMyCalc
Dim resultValue
Set calc = New MyCalc
resultValue = calc.AddThem(1.25, 2.5)
Debug.Print resultValue
End Sub
"#;
    let consumer_temp_leaf = format!(
        "basproj-early-bound-wrapped-com-imycalc-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    );
    let importlib = server_tlb_path.to_string_lossy().to_string();
    let consumer_loaded = load_typelib_basproj_with_ref_specs(
        &consumer_temp_leaf,
        consumer_source,
        &[BasprojComRefSpec {
            include: "MyCalcServer",
            guid: None,
            major: None,
            minor: None,
            lcid: None,
            importlib: Some(importlib.as_str()),
        }],
    );

    let snapshot = run_project_windows_hosted(&consumer_loaded.manifest, false);
    assert!(
        snapshot.contains(&Variant::from_f64(3.75)),
        "expected AddThem result in snapshot after early-bound IMyCalc call; snapshot={snapshot:?}"
    );

    drop(registration_guard);
    let _ = std::fs::remove_dir_all(&temp_root);
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
        Variant::from_i32(7),
        "Count should map through early-bind rewrite lane"
    );
    assert_eq!(
        out[2],
        Variant::from_bool(true),
        "Exists(42) should map through early-bind rewrite lane"
    );
    assert_eq!(
        out[3],
        Variant::from_i32(1_042),
        "Lookup(42) should map through metadata-backed property-get lane"
    );
    assert_eq!(
        out[4],
        Variant::from_i32(42),
        "obj(42) should map through metadata-backed default-member lane"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_newenum_foreach_transport() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim item
Dim valueOut
For Each item In obj
    valueOut = valueOut & CStr(item) & ","
Next item
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[2],
        Variant::from_string(oxvba_runtime::bstr::BStr::from("41,42,")),
        "imported COM NewEnum VT_UNKNOWN/IEnumVARIANT transport should materialize through the runtime For Each lane"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_imported_newenum_foreach_vm_repeat_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim item
Dim valueOut
For Each item In obj
    valueOut = valueOut & CStr(item) & ","
Next item
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported COM NewEnum/IEnumVARIANT For Each transport"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_registered_scripting_dictionary_anchor() {
    if !registered_scripting_dictionary_available() {
        return;
    }
    let manifest = manifest_with_reference(
        "Scripting",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New Scripting.Dictionary
Dim countValue
countValue = obj.Count
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(out[1], Variant::from_i32(0));
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_registered_scripting_dictionary_member_subset() {
    if !registered_scripting_dictionary_available() {
        return;
    }
    let manifest = manifest_with_reference(
        "Scripting",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New Scripting.Dictionary
Dim countValue
Dim existsValue
Call obj.Add("a", 1)
countValue = obj.Count
existsValue = obj.Exists("a")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    let obj = expect_object_handle(&out[0]);
    assert!(obj.raw() >= 20_001);
    let dispatch = obj
        .query_interface_descriptor(RuntimeInterfaceId::IDispatch)
        .expect("registered Scripting.Dictionary object should carry typelib descriptor metadata");
    assert!(
        dispatch.members.iter().any(|member| {
            member.name.eq_ignore_ascii_case("Count") && !member.is_default_member
        }),
        "Scripting.Dictionary descriptor should expose Count metadata"
    );
    assert!(
        dispatch.members.iter().any(|member| {
            member.name.eq_ignore_ascii_case("Exists") && member.params.len() == 1
        }),
        "Scripting.Dictionary descriptor should expose Exists parameter metadata"
    );
    assert_eq!(out[1], Variant::from_i32(1));
    assert_eq!(out[2], Variant::from_bool(true));
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_registered_scripting_filesystemobject_member_subset() {
    if !registered_scripting_filesystemobject_available() {
        return;
    }
    let manifest = manifest_with_reference(
        "Scripting",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New Scripting.FileSystemObject
Dim extValue
Dim baseValue
extValue = obj.GetExtensionName("C:\temp\demo.txt")
baseValue = obj.GetBaseName("C:\temp\demo.txt")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        Variant::from_string(oxvba_runtime::bstr::BStr::from("txt"))
    );
    assert_eq!(
        out[2],
        Variant::from_string(oxvba_runtime::bstr::BStr::from("demo"))
    );
}

#[cfg(target_os = "windows")]
#[test]
fn mixed_bound_project_executes_registered_access_jet_ado_database_subset() {
    let Some((adodb_importlib, adox_importlib)) = registered_access_jet_ado_available() else {
        return;
    };

    let unique = format!(
        "access-jet-mixed-bound-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    );
    let temp_root = std::env::current_dir()
        .expect("cwd")
        .join("temp")
        .join(unique);
    std::fs::create_dir_all(&temp_root).expect("create temp root");
    let db_path = temp_root.join("ShowcaseJetMixed.accdb");
    let connection = format!(
        "Provider=Microsoft.ACE.OLEDB.12.0;Data Source={}",
        db_path.to_string_lossy()
    );
    let escaped_connection = connection.replace('"', "\"\"");

    let main_source = format!(
        r#"
Attribute VB_Name = "Main"
Public Sub Main()
Dim catalog As New ADOX.Catalog
Dim cn As New ADODB.Connection
Dim rs
Dim fieldName
Dim fieldScore
Dim nameValue
Dim scoreValue
Call DispatchInvoke(catalog, "Create", "{connection}")
Call cn.Open("{connection}", "", "", 0)
Call cn.Execute("CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)", 0, 0)
Call cn.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)", 0, 0)
Call cn.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)", 0, 0)
rs = cn.Execute("SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2", 0, 0)
fieldName = DispatchInvoke(rs, "Fields", "Name")
fieldScore = DispatchInvoke(rs, "Fields", "Score")
nameValue = DispatchInvoke(fieldName, "Value")
scoreValue = DispatchInvoke(fieldScore, "Value")
End Sub
"#,
        connection = escaped_connection
    );

    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-access-jet-mixed-bound",
        &main_source,
        &[
            BasprojComRefSpec {
                include: "ADODB",
                guid: None,
                major: None,
                minor: None,
                lcid: None,
                importlib: Some(adodb_importlib.as_str()),
            },
            BasprojComRefSpec {
                include: "ADOX",
                guid: None,
                major: None,
                minor: None,
                lcid: None,
                importlib: Some(adox_importlib.as_str()),
            },
        ],
    );

    let out = run_project_windows_hosted(&loaded.manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert_eq!(
        out[5],
        Variant::from_string(oxvba_runtime::bstr::BStr::from("Grace"))
    );
    assert_eq!(out[6], Variant::from_i32(99));
    assert!(
        db_path.exists(),
        "mixed-bound Access/Jet database should be created"
    );

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[cfg(target_os = "windows")]
#[test]
fn strict_early_bound_project_executes_registered_access_jet_ado_database_subset() {
    let Some((adodb_importlib, adox_importlib)) = registered_access_jet_ado_available() else {
        return;
    };

    let unique = format!(
        "access-jet-strict-early-bound-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    );
    let temp_root = std::env::current_dir()
        .expect("cwd")
        .join("temp")
        .join(unique);
    std::fs::create_dir_all(&temp_root).expect("create temp root");
    let db_path = temp_root.join("ShowcaseJetStrictEarly.accdb");
    let connection = format!(
        "Provider=Microsoft.ACE.OLEDB.12.0;Data Source={}",
        db_path.to_string_lossy()
    );
    let escaped_connection = connection.replace('"', "\"\"");

    let main_source = format!(
        r#"
Attribute VB_Name = "Main"
Public Sub Main()
Dim catalog As New ADOX.Catalog
Dim cn As New ADODB.Connection
Dim rs As ADODB.Recordset
Dim fieldName As ADODB.Field
Dim fieldScore As ADODB.Field
Dim bangFieldName As ADODB.Field
Dim nameValue
Dim scoreValue
Dim bangNameValue
Dim bangScoreValue
Call catalog.Create("{connection}")
Call cn.Open("{connection}", "", "", 0)
Call cn.Execute("CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)", 0, 0)
Call cn.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)", 0, 0)
Call cn.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)", 0, 0)
Set rs = cn.Execute("SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2", 0, 0)
Set fieldName = rs.Fields("Name")
Set fieldScore = rs.Fields("Score")
Set bangFieldName = rs!Name
nameValue = fieldName.Value
scoreValue = fieldScore.Value
bangNameValue = rs!Name
bangScoreValue = rs!Score
End Sub
"#,
        connection = escaped_connection
    );

    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-access-jet-strict-early-bound",
        &main_source,
        &[
            BasprojComRefSpec {
                include: "ADODB",
                guid: None,
                major: None,
                minor: None,
                lcid: None,
                importlib: Some(adodb_importlib.as_str()),
            },
            BasprojComRefSpec {
                include: "ADOX",
                guid: None,
                major: None,
                minor: None,
                lcid: None,
                importlib: Some(adox_importlib.as_str()),
            },
        ],
    );

    let out = run_project_windows_hosted(&loaded.manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert!(expect_object_handle(&out[5]).raw() >= 20_001);
    assert_eq!(
        out[6],
        Variant::from_string(oxvba_runtime::bstr::BStr::from("Grace"))
    );
    assert_eq!(out[7], Variant::from_i32(99));
    assert_eq!(
        out[8],
        Variant::from_string(oxvba_runtime::bstr::BStr::from("Grace"))
    );
    assert_eq!(out[9], Variant::from_i32(99));
    assert!(
        db_path.exists(),
        "strict early-bound Access/Jet database should be created"
    );

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[cfg(target_os = "windows")]
fn dao_acedao_typelib_path() -> Option<String> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(var).map(std::path::PathBuf::from) {
            for ver in ["16", "15", "14"] {
                let office = format!("Office{ver}");
                // Click-to-Run installs nest the binaries under `root`; MSI installs do not.
                candidates.push(base.join("Microsoft Office").join("root").join(&office));
                candidates.push(base.join("Microsoft Office").join(&office));
            }
        }
    }
    for var in ["CommonProgramFiles", "CommonProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(var).map(std::path::PathBuf::from) {
            for ver in ["OFFICE16", "OFFICE15", "OFFICE14"] {
                candidates.push(base.join("Microsoft Shared").join(ver));
            }
        }
    }
    candidates
        .into_iter()
        .map(|dir| dir.join("ACEDAO.DLL"))
        .find(|path| path.exists())
        .map(|path| path.to_string_lossy().into_owned())
}

/// Returns the ACE DAO (`ACEDAO.DLL`) typelib path when both the DAO `DBEngine` COM class
/// activates and the typelib binary is locatable, else `None` so DAO tests skip cleanly on
/// machines without the Access Database Engine. DAO is exercised in addition to ADO because
/// its objects are dispinterfaces obtained from method calls (not coclasses), which stresses
/// interface-scoped binding and the get-or-call dispatch path differently than ADO does.
#[cfg(target_os = "windows")]
fn registered_access_jet_dao_available() -> Option<String> {
    let acedao = dao_acedao_typelib_path()?;
    let main_module = module_unit_from_source(
        "Main",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "Main"
Public Sub Main()
Dim engine
Set engine = CreateObject("DAO.DBEngine.120")
End Sub
"#,
    )
    .ok()?;
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };
    let available = std::panic::catch_unwind(|| run_project_windows_hosted(&manifest, false))
        .ok()
        .and_then(|snapshot| snapshot.first().cloned())
        .map(|value| expect_object_handle(&value).raw() >= 20_001)
        .unwrap_or(false);
    available.then_some(acedao)
}

#[cfg(target_os = "windows")]
fn access_jet_dao_temp_db(leaf: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let unique = format!(
        "{leaf}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    );
    let temp_root = std::env::current_dir()
        .expect("cwd")
        .join("temp")
        .join(unique);
    std::fs::create_dir_all(&temp_root).expect("create temp root");
    let db_path = temp_root.join("ShowcaseJetDao.accdb");
    (temp_root, db_path)
}

#[cfg(target_os = "windows")]
fn dao_com_ref(dao_importlib: &str) -> [BasprojComRefSpec<'_>; 1] {
    [BasprojComRefSpec {
        include: "DAO",
        guid: None,
        major: None,
        minor: None,
        lcid: None,
        importlib: Some(dao_importlib),
    }]
}

// Late-bound DAO: untyped variables, `CreateObject`, and `DispatchInvoke` by member name.
// The DAO DBEngine CLSID carries no `TypeLib` registry association, so member dispids are
// resolved dynamically via GetIDsOfNames; the get-or-call dispatch must still pick method
// vs property-get correctly for the strict Jet engine.
#[cfg(target_os = "windows")]
#[test]
fn late_bound_project_executes_registered_access_jet_dao_database_subset() {
    let Some(_dao_importlib) = registered_access_jet_dao_available() else {
        return;
    };
    let (temp_root, db_path) = access_jet_dao_temp_db("access-jet-dao-late-bound");
    let db = db_path.to_string_lossy();
    let main_source = format!(
        r#"
Attribute VB_Name = "Main"
Public Sub Main()
Dim engine
Dim db
Dim rs
Dim nameValue
Dim scoreValue
Set engine = CreateObject("DAO.DBEngine.120")
Set db = DispatchInvoke(engine, "CreateDatabase", "{db}", ";LANGID=0x0409;CP=1252;COUNTRY=0", 128)
Call DispatchInvoke(db, "Execute", "CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)")
Call DispatchInvoke(db, "Execute", "INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)")
Call DispatchInvoke(db, "Execute", "INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)")
Set rs = DispatchInvoke(db, "OpenRecordset", "SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2")
nameValue = DispatchInvoke(DispatchInvoke(rs, "Fields", "Name"), "Value")
scoreValue = DispatchInvoke(DispatchInvoke(rs, "Fields", "Score"), "Value")
End Sub
"#,
        db = db
    );

    let main_module = module_unit_from_source("Main", ModuleKind::Procedural, &main_source)
        .expect("late-bound DAO main module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert_eq!(
        out[3],
        Variant::from_string(oxvba_runtime::bstr::BStr::from("Grace"))
    );
    assert_eq!(out[4], Variant::from_i32(99));
    assert!(db_path.exists(), "late-bound DAO database should be created");

    let _ = std::fs::remove_dir_all(&temp_root);
}

// Mixed DAO: typed `As DAO.*` declarations early-bind `CreateDatabase`/`Execute`/`OpenRecordset`
// (resolved against the scoped DAO `DBEngine`/`Database` interfaces), while the recordset
// `Fields("...").Value` chain stays on the late-bound `DispatchInvoke` escape hatch.
#[cfg(target_os = "windows")]
#[test]
fn mixed_bound_project_executes_registered_access_jet_dao_database_subset() {
    let Some(dao_importlib) = registered_access_jet_dao_available() else {
        return;
    };
    let (temp_root, db_path) = access_jet_dao_temp_db("access-jet-dao-mixed-bound");
    let db = db_path.to_string_lossy();
    let main_source = format!(
        r#"
Attribute VB_Name = "Main"
Public Sub Main()
Dim engine As DAO.DBEngine
Dim db As DAO.Database
Dim rs As DAO.Recordset
Dim nameValue
Dim scoreValue
Set engine = CreateObject("DAO.DBEngine.120")
Set db = engine.CreateDatabase("{db}", ";LANGID=0x0409;CP=1252;COUNTRY=0", 128)
Call db.Execute("CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)", 128)
Call db.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)", 128)
Call db.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)", 128)
Set rs = db.OpenRecordset("SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2", 4, 0, 4)
nameValue = DispatchInvoke(DispatchInvoke(rs, "Fields", "Name"), "Value")
scoreValue = DispatchInvoke(DispatchInvoke(rs, "Fields", "Score"), "Value")
End Sub
"#,
        db = db
    );

    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-access-jet-dao-mixed-bound",
        &main_source,
        &dao_com_ref(&dao_importlib),
    );

    let out = run_project_windows_hosted(&loaded.manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert_eq!(
        out[3],
        Variant::from_string(oxvba_runtime::bstr::BStr::from("Grace"))
    );
    assert_eq!(out[4], Variant::from_i32(99));
    assert!(db_path.exists(), "mixed-bound DAO database should be created");

    let _ = std::fs::remove_dir_all(&temp_root);
}

// Strict early-bound DAO: no `DispatchInvoke` in the source. `DAO.DBEngine`, `DAO.Database`,
// `DAO.Recordset`, and `DAO.Field` are imported types; `rs.Fields("Name").Value` and the
// `rs!Name`/`rs!Score` bang accessors all lower through the metadata-backed COM bridge.
#[cfg(target_os = "windows")]
#[test]
fn strict_early_bound_project_executes_registered_access_jet_dao_database_subset() {
    let Some(dao_importlib) = registered_access_jet_dao_available() else {
        return;
    };
    let (temp_root, db_path) = access_jet_dao_temp_db("access-jet-dao-strict-early-bound");
    let db = db_path.to_string_lossy();
    let main_source = format!(
        r#"
Attribute VB_Name = "Main"
Public Sub Main()
Dim engine As DAO.DBEngine
Dim db As DAO.Database
Dim rs As DAO.Recordset
Dim fieldName As DAO.Field
Dim fieldScore As DAO.Field
Dim nameValue
Dim scoreValue
Dim bangNameValue
Dim bangScoreValue
Set engine = CreateObject("DAO.DBEngine.120")
Set db = engine.CreateDatabase("{db}", ";LANGID=0x0409;CP=1252;COUNTRY=0", 128)
Call db.Execute("CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)", 128)
Call db.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)", 128)
Call db.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)", 128)
Set rs = db.OpenRecordset("SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2", 4, 0, 4)
Set fieldName = rs.Fields("Name")
Set fieldScore = rs.Fields("Score")
nameValue = fieldName.Value
scoreValue = fieldScore.Value
bangNameValue = rs!Name
bangScoreValue = rs!Score
End Sub
"#,
        db = db
    );

    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-access-jet-dao-strict-early-bound",
        &main_source,
        &dao_com_ref(&dao_importlib),
    );

    let out = run_project_windows_hosted(&loaded.manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert!(expect_object_handle(&out[3]).raw() >= 20_001);
    assert!(expect_object_handle(&out[4]).raw() >= 20_001);
    assert_eq!(
        out[5],
        Variant::from_string(oxvba_runtime::bstr::BStr::from("Grace"))
    );
    assert_eq!(out[6], Variant::from_i32(99));
    assert_eq!(
        out[7],
        Variant::from_string(oxvba_runtime::bstr::BStr::from("Grace"))
    );
    assert_eq!(out[8], Variant::from_i32(99));
    assert!(
        db_path.exists(),
        "strict early-bound DAO database should be created"
    );

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_registered_scripting_dictionary_member_subset_prefer_vtable_matches_dispatch()
 {
    if !registered_scripting_dictionary_available() {
        return;
    }
    let manifest = manifest_with_reference(
        "Scripting",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New Scripting.Dictionary
Dim countValue
Dim existsValue
Call obj.Add("a", 1)
countValue = obj.Count
existsValue = obj.Exists("a")
End Sub
"#,
    );

    let dispatch = run_project_windows_hosted(&manifest, false);
    let mut policy = HostPolicy::interactive_dev();
    policy.com_invocation_strategy = ComInvocationStrategy::PreferVtable;
    let vtable = run_project_windows_hosted_with_policy(&manifest, false, policy);
    assert_eq!(dispatch, vtable);
    assert!(expect_object_handle(&vtable[0]).raw() >= 20_001);
    assert_eq!(vtable[1], Variant::from_i32(1));
    assert_eq!(vtable[2], Variant::from_bool(true));
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_registered_testdispatch_foreach_transport() {
    if !registered_testdispatch_available() {
        return;
    }
    let manifest = manifest_with_reference(
        "OxVba",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj
Dim item
Dim valueOut
obj = CreateObject("OxVba.TestDispatch")
For Each item In obj
    valueOut = valueOut & CStr(item) & ","
Next item
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[2],
        Variant::from_string(oxvba_runtime::bstr::BStr::from("41,42,")),
        "registered OxVba.TestDispatch For Each transport should materialize through the runtime lane"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_registered_testdispatch_foreach_vm_repeat_snapshots_match() {
    if !registered_testdispatch_available() {
        return;
    }
    let manifest = manifest_with_reference(
        "OxVba",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj
Dim item
Dim valueOut
obj = CreateObject("OxVba.TestDispatch")
For Each item In obj
    valueOut = valueOut & CStr(item) & ","
Next item
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for registered OxVba.TestDispatch For Each transport"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_registered_testeventserver_ping() {
    if !registered_testeventserver_available() {
        return;
    }
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestEventServer
Dim value
value = obj.Ping()
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(out[1], Variant::from_i32(42));
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_registered_testeventserver_ping_prefer_vtable_matches_dispatch() {
    if !registered_testeventserver_available() {
        return;
    }
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestEventServer
Dim value
value = obj.Ping()
End Sub
"#,
    );

    let dispatch = run_project_windows_hosted(&manifest, false);
    let mut policy = HostPolicy::interactive_dev();
    policy.com_invocation_strategy = ComInvocationStrategy::PreferVtable;
    let vtable = run_project_windows_hosted_with_policy(&manifest, false, policy);
    assert_eq!(dispatch, vtable);
    assert!(expect_object_handle(&vtable[0]).raw() >= 20_001);
    assert_eq!(vtable[1], Variant::from_i32(42));
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_executes_registered_testeventserver_ping() {
    if !registered_testeventserver_available() {
        return;
    }

    let loaded = load_typelib_basproj_with_refs(
        "basproj-typelib-oracle-test",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New OxVba.TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[("OxVba", "{E2A30001-0001-0001-0001-000000000001}", 1, 0, 0)],
    );
    assert!(
        loaded.manifest.references.iter().any(|reference| reference
            .referenced_project_name
            .eq_ignore_ascii_case("OxVba")),
        "expected loaded manifest to retain the OxVba typelib reference"
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("loaded basproj should compile through the OxVba typelib lane");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserver\")")
            && out.contains("value = dispatchinvoke(obj, 104)"),
        "expected loaded basproj binding to lower through the registered OxVba typelib dispatch lane, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_prefers_first_typelib_reference_for_unqualified_testeventserver() {
    let loaded = load_typelib_basproj_with_refs(
        "basproj-typelib-order-a",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            ("OxVba", "{E2A30001-0001-0001-0001-000000000001}", 1, 0, 0),
            (
                "OxVbaAlt",
                "{E2A30001-0001-0001-0001-000000000101}",
                1,
                0,
                0,
            ),
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("first reference should drive unqualified imported binding");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserver_testeventserver_ping(obj)"),
        "expected first reference to lower to the base synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_prefers_reversed_first_typelib_reference_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_refs(
        "basproj-typelib-order-b",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            (
                "OxVbaAlt",
                "{E2A30001-0001-0001-0001-000000000101}",
                1,
                0,
                0,
            ),
            ("OxVba", "{E2A30001-0001-0001-0001-000000000001}", 1, 0, 0),
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("reversed first reference should drive unqualified imported binding");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserveralt_testeventserver_ping(obj)"),
        "expected reversed first reference to lower to the alt synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_prefers_first_of_three_typelib_references_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_refs(
        "basproj-typelib-order-three-a",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            ("OxVba", "{E2A30001-0001-0001-0001-000000000001}", 1, 0, 0),
            (
                "OxVbaAlt",
                "{E2A30001-0001-0001-0001-000000000101}",
                1,
                0,
                0,
            ),
            (
                "OxVbaAlt2",
                "{E2A30001-0001-0001-0001-000000000201}",
                1,
                0,
                0,
            ),
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("first of three references should drive unqualified imported binding");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserver_testeventserver_ping(obj)"),
        "expected first of three references to lower to the base synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_prefers_middle_first_of_three_typelib_references_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_refs(
        "basproj-typelib-order-three-b",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            (
                "OxVbaAlt",
                "{E2A30001-0001-0001-0001-000000000101}",
                1,
                0,
                0,
            ),
            ("OxVba", "{E2A30001-0001-0001-0001-000000000001}", 1, 0, 0),
            (
                "OxVbaAlt2",
                "{E2A30001-0001-0001-0001-000000000201}",
                1,
                0,
                0,
            ),
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("alt first of three references should drive unqualified imported binding");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserveralt_testeventserver_ping(obj)"),
        "expected first of three references to lower to the alt synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_prefers_third_variant_when_first_of_three_typelib_references_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_refs(
        "basproj-typelib-order-three-c",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            (
                "OxVbaAlt2",
                "{E2A30001-0001-0001-0001-000000000201}",
                1,
                0,
                0,
            ),
            ("OxVba", "{E2A30001-0001-0001-0001-000000000001}", 1, 0, 0),
            (
                "OxVbaAlt",
                "{E2A30001-0001-0001-0001-000000000101}",
                1,
                0,
                0,
            ),
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("alt2 first of three references should drive unqualified imported binding");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserveralt2_testeventserver_ping(obj)"),
        "expected first of three references to lower to the alt2 synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_reports_unresolved_typelib_libid_identity() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-libid-unresolved",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[BasprojComRefSpec {
            include: "OxVbaMissing",
            guid: Some("{E2A30001-0001-0001-0001-000000009999}"),
            major: Some(1),
            minor: Some(0),
            lcid: Some(0),
            importlib: None,
        }],
    );

    let err = run_project_windows_hosted_error(&loaded.manifest, false);
    assert!(
        err.contains("PMR-E-TYPELIB-LIBID-UNRESOLVED"),
        "expected unresolved LIBID diagnostic, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_reports_unresolved_typelib_importlib_identity() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-importlib-unresolved",
        concat!(
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[BasprojComRefSpec {
            include: "OxVbaMissingFile",
            guid: None,
            major: None,
            minor: None,
            lcid: None,
            importlib: Some("C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\NoSuchTypeLib.tlb"),
        }],
    );

    let err = run_project_windows_hosted_error(&loaded.manifest, false);
    assert!(
        err.contains("PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"),
        "expected unresolved importlib diagnostic, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_mixed_broken_base_then_valid_alt_reports_unresolved_importlib() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-mixed-base-missing-alt-valid",
        concat!(
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
        ],
    );

    let err = run_project_windows_hosted_error(&loaded.manifest, false);
    assert!(
        err.contains("PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"),
        "expected unresolved importlib diagnostic, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_mixed_broken_alt_then_valid_base_reports_unresolved_importlib() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-mixed-alt-missing-base-valid",
        concat!(
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAltMissing",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
        ],
    );

    let err = run_project_windows_hosted_error(&loaded.manifest, false);
    assert!(
        err.contains("PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"),
        "expected unresolved importlib diagnostic, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_mixed_broken_base_then_valid_alt_then_valid_alt2_reports_unresolved_importlib()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-mixed-base-missing-alt-valid-alt2-valid",
        concat!(
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt2",
                guid: Some("{E2A30001-0001-0001-0001-000000000201}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt2.tlb"),
            },
        ],
    );

    let err = run_project_windows_hosted_error(&loaded.manifest, false);
    assert!(
        err.contains("PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"),
        "expected unresolved importlib diagnostic, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_mixed_broken_alt2_then_valid_base_then_valid_alt_reports_unresolved_importlib()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-mixed-alt2-missing-base-valid-alt-valid",
        concat!(
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAlt2Missing",
                guid: Some("{E2A30001-0001-0001-0001-000000000201}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt2.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
        ],
    );

    let err = run_project_windows_hosted_error(&loaded.manifest, false);
    assert!(
        err.contains("PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"),
        "expected unresolved importlib diagnostic, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_broken_base_then_valid_alt_qualified_target_resolves_alt_binding() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-broken-base-valid-alt-qualified-alt",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As OxVba_TestEventServerAlt.TestEventServer\n",
            "Dim value\n",
            "Set obj = New OxVba_TestEventServerAlt.TestEventServer\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest).expect(
        "qualified later-valid alt reference should compile despite earlier broken reference",
    );
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserveralt\")"),
        "expected qualified later-valid alt binding to lower to the alt ProgID, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_broken_alt_then_valid_base_qualified_target_resolves_base_binding() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-broken-alt-valid-base-qualified-base",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As OxVba_TestEventServer.TestEventServer\n",
            "Dim value\n",
            "Set obj = New OxVba_TestEventServer.TestEventServer\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAltMissing",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest).expect(
        "qualified later-valid base reference should compile despite earlier broken reference",
    );
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserver\")"),
        "expected qualified later-valid base binding to lower to the base ProgID, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_valid_base_then_broken_alt_resolves_qualified_base_binding() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-valid-base-broken-alt-qualified-base",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As OxVba_TestEventServer.TestEventServer\n",
            "Dim value\n",
            "Set obj = New OxVba_TestEventServer.TestEventServer\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAltMissing",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt.tlb",
                ),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("qualified valid base reference should compile despite later broken reference");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserver\")"),
        "expected qualified valid base binding to lower to the base ProgID, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_valid_alt_then_broken_base_resolves_qualified_alt_binding() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-valid-alt-broken-base-qualified-alt",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As OxVba_TestEventServerAlt.TestEventServer\n",
            "Dim value\n",
            "Set obj = New OxVba_TestEventServerAlt.TestEventServer\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("qualified valid alt reference should compile despite later broken reference");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserveralt\")"),
        "expected qualified valid alt binding to lower to the alt ProgID, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_valid_base_then_broken_alt_prefers_base_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-valid-base-broken-alt-unqualified",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAltMissing",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt.tlb",
                ),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest).expect(
        "valid first reference should drive unqualified binding despite later broken reference",
    );
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserver_testeventserver_ping(obj)"),
        "expected unqualified valid-first binding to lower to the base synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_valid_alt_then_broken_base_prefers_alt_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-valid-alt-broken-base-unqualified",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest).expect(
        "valid first alt reference should drive unqualified binding despite later broken reference",
    );
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserveralt_testeventserver_ping(obj)"),
        "expected unqualified valid-first binding to lower to the alt synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_valid_base_then_broken_alt_then_valid_alt2_prefers_base_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-valid-base-broken-alt-valid-alt2-unqualified",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAltMissing",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt2",
                guid: Some("{E2A30001-0001-0001-0001-000000000201}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt2.tlb"),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("valid first reference should drive unqualified binding despite later broken middle reference");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserver_testeventserver_ping(obj)"),
        "expected unqualified valid-first binding to lower to the base synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_valid_alt2_then_broken_base_then_valid_alt_prefers_alt2_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-valid-alt2-broken-base-valid-alt-unqualified",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAlt2",
                guid: Some("{E2A30001-0001-0001-0001-000000000201}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt2.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("valid first alt2 reference should drive unqualified binding despite later broken middle reference");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserveralt2_testeventserver_ping(obj)"),
        "expected unqualified valid-first binding to lower to the alt2 synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_broken_base_then_valid_alt_then_valid_alt2_qualified_target_resolves_alt2_binding()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-broken-base-valid-alt-valid-alt2-qualified-alt2",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As OxVbaAlt2.TestEventServer\n",
            "Dim value\n",
            "Set obj = New OxVbaAlt2.TestEventServer\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt2",
                guid: Some("{E2A30001-0001-0001-0001-000000000201}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt2.tlb"),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest).expect(
        "qualified later-valid alt2 reference should compile despite earlier broken reference",
    );
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserveralt2\")"),
        "expected qualified later-valid alt2 binding to lower to the alt2 ProgID, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_broken_alt2_then_valid_base_then_valid_alt_qualified_target_resolves_alt_binding()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-broken-alt2-valid-base-valid-alt-qualified-alt",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As OxVbaAlt.TestEventServer\n",
            "Dim value\n",
            "Set obj = New OxVbaAlt.TestEventServer\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAlt2Missing",
                guid: Some("{E2A30001-0001-0001-0001-000000000201}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt2.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest).expect(
        "qualified later-valid alt reference should compile despite earlier broken alt2 reference",
    );
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserveralt\")"),
        "expected qualified later-valid alt binding to lower to the alt ProgID, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_registered_testeventserver_withevents_callback_invokes_handler_body() {
    if !registered_testeventserver_available() {
        return;
    }

    let class_module = module_unit_from_source(
        "Sink",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Sink"
Private WithEvents src As OxVba.TestEventServer

Private Sub Class_Initialize()
    Set src = New OxVba.TestEventServer
    Call src.FireValueChanged(7)
End Sub

Private Sub src_OnValueChanged(ByVal value)
    Err.Raise 77
End Sub

Public Function Touch() As Long
    Touch = 1
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
Dim s As New Sink
Dim touched
touched = s.Touch()
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

    let err = run_project_windows_hosted_error(&manifest, false);
    assert!(
        err.contains("runtime error: 77"),
        "expected callback handler Err.Raise 77, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_registered_testeventserver_withevents_callback_preserves_value_payload() {
    if !registered_testeventserver_available() {
        return;
    }

    let class_module = module_unit_from_source(
        "Sink",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Sink"
Private WithEvents src As OxVba.TestEventServer

Private Sub Class_Initialize()
    Set src = New OxVba.TestEventServer
    Call src.FireValueChanged(7)
End Sub

Private Sub src_OnValueChanged(ByVal value)
    If value = 7 Then
        Err.Raise 7007
    Else
        Err.Raise 7999
    End If
End Sub

Public Function Touch() As Long
    Touch = 1
End Function
"#,
    )
    .expect("class module should parse");
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
    Dim s As New Sink
    Dim touched
    touched = s.Touch()
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

    let err = run_project_windows_hosted_error(&manifest, false);
    assert!(
        err.contains("runtime error: 7007"),
        "expected callback payload encoded in Err.Raise 7007, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_vm_repeat_snapshots_match_for_subset() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for early-binding subset"
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
        Variant::from_i32(19),
        "Call-form imported positional method/property/default-member invokes should execute without degrading the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_call_statement_subset_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported Call-form positional member invokes"
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
        Variant::from_i32(29),
        "Call-form imported named-argument method/property/default-member invokes should execute without degrading metadata-backed canonicalization"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_call_statements_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported Call-form named-argument member invokes"
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
        Variant::from_i32(43),
        "no-paren Call-form imported positional method/property/default-member invokes should execute on the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_no_paren_call_statement_subset_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported no-paren Call-form positional member invokes"
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
        Variant::from_i32(47),
        "no-paren Call-form imported named-argument method/property/default-member invokes should execute on the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_no_paren_named_argument_call_statements_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported no-paren Call-form named-argument member invokes"
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
        Variant::from_i32(31),
        "statement-context imported positional method/property/default-member invokes should execute without degrading the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_statement_context_subset_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported statement-context positional member invokes"
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
        Variant::from_i32(37),
        "statement-context imported named-argument method/property/default-member invokes should execute without degrading metadata-backed canonicalization"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_statement_context_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported statement-context named-argument member invokes"
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
        Variant::from_i32(53),
        "no-paren statement-context imported positional method/property/default-member invokes should execute on the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_no_paren_statement_context_subset_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported no-paren statement-context positional member invokes"
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
        Variant::from_i32(59),
        "no-paren statement-context imported named-argument method/property/default-member invokes should execute on the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_no_paren_named_argument_statement_context_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported no-paren statement-context named-argument member invokes"
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
    let repeat = run_project_windows_hosted_error(&manifest, true);
    assert!(
        vm.contains("com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;")
            && repeat.contains(
                "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;"
            ),
        "expected stable imported exception prefix across VM repeat, got vm={vm:?} repeat={repeat:?}"
    );
    assert!(
        vm.contains("excep_source=\"OxVba.TestDispatch\"")
            && vm.contains("excep_description=\"controlled dispatch exception\"")
            && repeat.contains("excep_source=\"OxVba.TestDispatch\"")
            && repeat.contains("excep_description=\"controlled dispatch exception\""),
        "expected imported EXCEPINFO source/description across VM repeat, got vm={vm:?} repeat={repeat:?}"
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
    let repeat = run_project_windows_hosted_error(&manifest, true);
    assert!(
        vm.contains("com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;")
            && repeat.contains(
                "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;"
            ),
        "expected stable imported exception prefix across VM repeat, got vm={vm:?} repeat={repeat:?}"
    );
    assert!(
        vm.contains("excep_source=\"OxVba.TestDispatch\"")
            && vm.contains("excep_description=\"controlled dispatch exception\"")
            && repeat.contains("excep_source=\"OxVba.TestDispatch\"")
            && repeat.contains("excep_description=\"controlled dispatch exception\""),
        "expected imported EXCEPINFO source/description across VM repeat, got vm={vm:?} repeat={repeat:?}"
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
    let repeat = run_project_windows_hosted_error(&manifest, true);
    assert!(
        vm.contains("com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;")
            && repeat.contains(
                "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;"
            ),
        "expected stable imported exception prefix across VM repeat, got vm={vm:?} repeat={repeat:?}"
    );
    assert!(
        vm.contains("excep_source=\"OxVba.TestDispatch\"")
            && vm.contains("excep_description=\"controlled dispatch exception\"")
            && repeat.contains("excep_source=\"OxVba.TestDispatch\"")
            && repeat.contains("excep_description=\"controlled dispatch exception\""),
        "expected imported EXCEPINFO source/description across VM repeat, got vm={vm:?} repeat={repeat:?}"
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
    let repeat = run_project_windows_hosted_error(&manifest, true);
    assert!(
        vm.contains("com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;")
            && repeat.contains(
                "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;"
            ),
        "expected stable imported exception prefix across VM repeat, got vm={vm:?} repeat={repeat:?}"
    );
    assert!(
        vm.contains("excep_source=\"OxVba.TestDispatch\"")
            && vm.contains("excep_description=\"controlled dispatch exception\"")
            && repeat.contains("excep_source=\"OxVba.TestDispatch\"")
            && repeat.contains("excep_description=\"controlled dispatch exception\""),
        "expected imported EXCEPINFO source/description across VM repeat, got vm={vm:?} repeat={repeat:?}"
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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
        Variant::from_i32(9),
        "imported property-put assignment should lower into the deterministic setter lane"
    );
    assert_eq!(
        out[2],
        Variant::from_i32(307_011),
        "imported indexed property-put assignment should preserve index and value"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_property_put_assignment_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported property-put assignment subset"
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
        Variant::from_i32(307_011),
        "imported named-argument property-put assignment should preserve metadata-backed parameter naming"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_property_put_assignment_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported named-argument property-put assignment syntax"
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
        Variant::from_i32(7),
        "controlled property-putref object lane should interrogate the bound object deterministically"
    );
    assert_eq!(
        out[3],
        Variant::from_i32(408_007),
        "named-argument property-putref assignment should preserve index and bounded object-derived token"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_property_putref_assignment_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported named-argument property-putref assignment syntax"
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
        Variant::from_i32(9),
        "imported zero-arg property-get read-assignment should lower through metadata-backed getter syntax"
    );
    assert_eq!(
        out[2],
        Variant::from_i32(9),
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
        Variant::from_i32(123),
        "imported zero-arg method read-assignment should lower through metadata-backed invoke syntax"
    );
    assert_eq!(
        out[2],
        Variant::from_i32(123),
        "explicit Let should preserve imported zero-arg method read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_zero_arg_method_read_assignment_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported zero-arg method read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_zero_arg_property_get_read_assignment_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported zero-arg property-get read-assignment syntax"
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
        Variant::from_i32(9),
        "imported parenthesized zero-arg property-get read-assignment should lower through metadata-backed getter syntax"
    );
    assert_eq!(
        out[2],
        Variant::from_i32(9),
        "explicit Let should preserve imported parenthesized zero-arg property-get read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_parenthesized_zero_arg_property_get_read_assignment_vm_repeat_snapshots_match()
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported parenthesized zero-arg property-get read-assignment syntax"
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
        Variant::from_i32(7),
        "direct imported object-valued property-get should preserve VT_DISPATCH rebinding on Object targets"
    );
    assert_eq!(
        out[6],
        Variant::from_i32(7),
        "direct imported object-valued property-get should preserve VT_UNKNOWN rebinding on Object targets"
    );
    assert_eq!(
        out[7],
        Variant::from_i32(7),
        "direct imported object-valued property-get should preserve VT_DISPATCH rebinding on Variant targets"
    );
    assert_eq!(
        out[8],
        Variant::from_i32(7),
        "direct imported object-valued property-get should preserve VT_UNKNOWN rebinding on explicit-Let Variant targets"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_object_property_get_read_assignment_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported object-valued property-get read-assignment syntax"
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
        Variant::from_i32(7),
        "parenthesized imported object-valued property-get should preserve VT_DISPATCH rebinding on Object targets"
    );
    assert_eq!(
        out[6],
        Variant::from_i32(7),
        "parenthesized imported object-valued property-get should preserve VT_UNKNOWN rebinding on Object targets"
    );
    assert_eq!(
        out[7],
        Variant::from_i32(7),
        "parenthesized imported object-valued property-get should preserve VT_DISPATCH rebinding on Variant targets"
    );
    assert_eq!(
        out[8],
        Variant::from_i32(7),
        "parenthesized imported object-valued property-get should preserve VT_UNKNOWN rebinding on explicit-Let Variant targets"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_parenthesized_object_property_get_read_assignment_vm_repeat_snapshots_match()
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for parenthesized imported object-valued property-get read-assignment syntax"
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
        Variant::from_i32(7),
        "VT_DISPATCH imported member result should rebind into an invokable object handle"
    );
    assert_eq!(
        out[4],
        Variant::from_i32(7),
        "VT_UNKNOWN imported member result should rebind through IDispatch into an invokable object handle"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_reuses_imported_object_identity_for_dispatch_and_unknown_results() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim firstDispatch As Object
Dim secondDispatch As Object
Dim firstUnknown
Dim secondUnknown
Set firstDispatch = obj.ReturnSelfDispatch()
Set secondDispatch = obj.ReturnSelfDispatch()
firstUnknown = obj.ReturnSelfUnknown()
secondUnknown = obj.ReturnSelfUnknown()
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_same_object_identity(
        &vm,
        &[1, 2, 3, 4],
        "VM imported repeated object-result identity",
    );
    assert_same_object_identity(
        &repeat,
        &[1, 2, 3, 4],
        "repeat imported repeated object-result identity",
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
        Variant::from_i32(3_014),
        "imported method call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
    assert_eq!(
        out[2],
        Variant::from_i32(205_009),
        "imported property-get call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
    assert_eq!(
        out[3],
        Variant::from_i32(41),
        "imported default-member call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_calls_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported named-argument calls"
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
        Variant::from_i32(3_014),
        "explicit Let imported method call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
    assert_eq!(
        out[2],
        Variant::from_i32(205_009),
        "explicit Let imported property-get call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
    assert_eq!(
        out[3],
        Variant::from_i32(41),
        "explicit Let imported default-member call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_explicit_let_named_argument_calls_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for explicit Let imported named-argument calls"
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
        Variant::from_i32(7),
        "explicit Let imported zero-arg call should preserve metadata-backed lowering"
    );
    assert_eq!(
        out[2],
        Variant::from_bool(true),
        "explicit Let imported method call should preserve metadata-backed lowering"
    );
    assert_eq!(
        out[3],
        Variant::from_i32(1_042),
        "explicit Let imported property-get call should preserve metadata-backed lowering"
    );
    assert_eq!(
        out[4],
        Variant::from_i32(42),
        "explicit Let imported default-member call should preserve metadata-backed lowering"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_explicit_let_positional_calls_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for explicit Let imported positional calls"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_object_result_member_calls_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported object-result member calls"
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
        Variant::from_i32(7),
        "explicit Set should preserve VT_DISPATCH object-result rebinding on Object targets"
    );
    assert_eq!(
        out[6],
        Variant::from_i32(7),
        "explicit Set should preserve VT_UNKNOWN object-result rebinding on Object targets"
    );
    assert_eq!(
        out[7],
        Variant::from_i32(7),
        "implicit Variant-target assignment should preserve VT_DISPATCH object-result rebinding"
    );
    assert_eq!(
        out[8],
        Variant::from_i32(7),
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
        Variant::from_i32(7),
        "explicit Set should preserve VT_DISPATCH zero-arg method rebinding on Object targets without parentheses"
    );
    assert_eq!(
        out[6],
        Variant::from_i32(7),
        "explicit Set should preserve VT_UNKNOWN zero-arg method rebinding on Object targets without parentheses"
    );
    assert_eq!(
        out[7],
        Variant::from_i32(7),
        "implicit Variant-target assignment should preserve VT_DISPATCH zero-arg method rebinding without parentheses"
    );
    assert_eq!(
        out[8],
        Variant::from_i32(7),
        "explicit Let Variant-target assignment should preserve VT_UNKNOWN zero-arg method rebinding without parentheses"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_zero_arg_object_result_assignment_intents_without_parentheses_vm_repeat_snapshots_match()
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported zero-arg object-result assignment intents without parentheses"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_object_result_assignment_intents_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported object-result assignment-intent lanes"
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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
        Variant::from_i32(7),
        "controlled property-putref object lane should interrogate the bound object deterministically"
    );
    assert_eq!(
        out[3],
        Variant::from_i32(100_007),
        "property-putref assignment should preserve bounded object-derived token on the deterministic setter lane"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_property_putref_assignment_vm_repeat_snapshots_match() {
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
    let repeat = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, repeat,
        "VM repeat snapshots should match for imported property-putref assignment syntax"
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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
fn early_bound_project_accepts_imported_withevents_source() {
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_withevents_source() {
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_type_declaration() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As TestDispatch
End Sub
"#,
    );

    let engine = Engine::new(HostConfig { enable_jit: false });
    engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_as_new_declaration() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New TestDispatch
End Sub
"#,
    );

    let engine = Engine::new(HostConfig { enable_jit: false });
    engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_imported_module_scope_declaration() {
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_imported_procedure_param_type_signature() {
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Public Sub Observe(ByVal obj As OxVba.TestDispatch)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_imported_implements_directive() {
    let class_module = module_unit_from_source(
        "ThingImpl",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "ThingImpl"
Implements OxVba.TestDispatch
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_implements_directive() {
    let class_module = module_unit_from_source(
        "ThingImpl",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "ThingImpl"
Implements TestDispatch
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_imported_event_declaration_type() {
    let class_module = module_unit_from_source(
        "Emitter",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Emitter"
Public Event Changed(ByVal value As OxVba.TestDispatch)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_event_declaration_type() {
    let class_module = module_unit_from_source(
        "Emitter",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Emitter"
Public Event Changed(ByVal value As TestDispatch)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_procedure_return_type_signature() {
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Public Function Make() As TestDispatch
End Function
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_module_scope_declaration() {
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("should compile");
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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

    let engine = Engine::new(HostConfig { enable_jit: false });
    let err = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
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
    assert_eq!(out[1], Variant::from_i32(7));
    assert_eq!(out[2], Variant::from_bool(true));
}
