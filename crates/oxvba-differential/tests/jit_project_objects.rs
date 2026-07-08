use std::collections::BTreeMap;

use oxvba_differential::{Executor, RunOutcome, canon, run_modules, run_project_closure};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectReference,
    ReferencedProjectManifest, SymbolProjectManifest,
};

fn module(name: &str, kind: ModuleKind, src: &str) -> ModuleUnit {
    ModuleUnit {
        module_name: name.to_string(),
        module_kind: kind,
        attributes: ModuleAttributes::named(name),
        source: src.to_string(),
    }
}

fn proc_module(name: &str, src: &str) -> ModuleUnit {
    module(name, Procedural, src)
}

fn predeclared_class_module(name: &str, src: &str) -> ModuleUnit {
    let mut module = module(name, Class, src);
    module.attributes.vb_predeclared_id = true;
    module.attributes.vb_exposed = true;
    module.attributes.vb_creatable = true;
    module
}

fn exposed_class_module(name: &str, src: &str) -> ModuleUnit {
    let mut module = module(name, Class, src);
    module.attributes.vb_exposed = true;
    module.attributes.vb_creatable = true;
    module
}

fn project(modules: Vec<ModuleUnit>) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: "VBAProject".to_string(),
        project_kind: ProjectKind::Source,
        modules,
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn referenced(project_name: &str, modules: Vec<ModuleUnit>) -> ReferencedProjectManifest {
    ReferencedProjectManifest {
        project_name: project_name.to_string(),
        project_kind: ProjectKind::Library,
        modules,
    }
}

fn project_with_refs(
    name: &str,
    modules: Vec<ModuleUnit>,
    refs: Vec<ReferencedProjectManifest>,
) -> SymbolProjectManifest {
    let references = refs
        .iter()
        .map(|reference| ProjectReference::Project {
            referenced_project_name: reference.project_name.clone(),
        })
        .collect();
    SymbolProjectManifest {
        project_name: name.to_string(),
        project_kind: ProjectKind::Source,
        modules,
        references,
        reference_projects: refs,
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn library_project(reference: &ReferencedProjectManifest) -> SymbolProjectManifest {
    SymbolProjectManifest {
        project_name: reference.project_name.clone(),
        project_kind: ProjectKind::Library,
        modules: reference.modules.clone(),
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn assert_completed_with_i32(label: &str, outcome: RunOutcome, expected: i32) {
    assert!(
        outcome.unsupported.is_none(),
        "{label} should execute the project-object oracle case: {outcome:?}"
    );
    assert!(
        outcome
            .handle_balance
            .is_some_and(oxvba_runtime::HandleBalance::is_zero),
        "{label} handle imbalance: {:?}",
        outcome.handle_balance
    );
    assert_eq!(
        outcome
            .result
            .unwrap_or_else(|err| panic!("{label} should complete: {err}"))
            .first(),
        Some(&canon(&Variant::from_i32(expected)))
    );
}

fn assert_completed_prefix_i32(label: &str, outcome: RunOutcome, expected: &[i32]) {
    assert!(
        outcome.unsupported.is_none(),
        "{label} should execute the project-object oracle case: {outcome:?}"
    );
    assert!(
        outcome
            .handle_balance
            .is_some_and(oxvba_runtime::HandleBalance::is_zero),
        "{label} handle imbalance: {:?}",
        outcome.handle_balance
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("{label} should complete: {err}"));
    for (index, expected) in expected.iter().enumerate() {
        assert_eq!(
            values.get(index),
            Some(&canon(&Variant::from_i32(*expected))),
            "{label} snapshot mismatch at index {index}: {values:?}"
        );
    }
}

#[test]
fn jit_project_class_new_property_get_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  r = w.Value\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Private m As Long\nPrivate Sub Class_Initialize()\n  m = 42\nEnd Sub\nPublic Property Get Value() As Long\n  Value = m\nEnd Property\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 42);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 42);
}

#[test]
fn jit_project_typed_local_is_nothing_matches_vm3_without_construction() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim w As Widget\n  If w Is Nothing Then\n    r = 11\n  Else\n    r = 13\n  End If\nEnd Sub\n",
        ),
        ("Widget", Class, "' project class marker\n"),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 11);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 11);
}

#[test]
fn jit_project_dim_as_new_is_nothing_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nPublic initCount As Long\nSub Main()\n  Dim w As New Widget\n  If w Is Nothing Then\n    r = 41\n  Else\n    r = 43 + initCount\n  End If\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Private Sub Class_Initialize()\n  Main.initCount = Main.initCount + 1\nEnd Sub\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 44);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 44);
}

#[test]
fn jit_project_dim_as_new_set_nothing_before_access_is_lazy_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nPublic initCount As Long\nSub Main()\n  Dim c As New Counter\n  Set c = Nothing\n  r = initCount\nEnd Sub\n",
        ),
        (
            "Counter",
            Class,
            "Private Sub Class_Initialize()\n  Main.initCount = Main.initCount + 1\nEnd Sub\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 0);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 0);
}

#[test]
fn jit_project_dim_as_new_reinstantiates_after_set_nothing_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nPublic initCount As Long\nSub Main()\n  Dim c As New Counter\n  If c Is Nothing Then r = r + 100\n  Set c = Nothing\n  If c Is Nothing Then r = r + 1000\n  r = r + initCount\nEnd Sub\n",
        ),
        (
            "Counter",
            Class,
            "Private Sub Class_Initialize()\n  Main.initCount = Main.initCount + 1\nEnd Sub\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 2);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 2);
}

#[test]
fn jit_project_field_as_new_reinstantiates_after_set_nothing_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nPublic initCount As Long\nSub Main()\n  Dim h As Holder\n  Set h = New Holder\n  h.Touch\n  h.Clear\n  h.Touch\n  r = r + initCount\nEnd Sub\n",
        ),
        (
            "Holder",
            Class,
            "Private child As New Counter\nPublic Sub Touch()\n  If child Is Nothing Then\n    Main.r = Main.r + 100\n  Else\n    Main.r = Main.r + 1\n  End If\nEnd Sub\nPublic Sub Clear()\n  Set child = Nothing\nEnd Sub\n",
        ),
        (
            "Counter",
            Class,
            "Private Sub Class_Initialize()\n  Main.initCount = Main.initCount + 1\nEnd Sub\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 4);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 4);
}

#[test]
fn jit_project_typed_null_set_assignment_matches_vm3_without_construction() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim a As Widget\n  Dim b As Widget\n  Set b = a\n  If b Is Nothing Then\n    r = 21\n  Else\n    r = 22\n  End If\n  Set b = Nothing\n  If Not (b Is Nothing) Then\n    r = 23\n  End If\nEnd Sub\n",
        ),
        ("Widget", Class, "' project class marker\n"),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 21);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 21);
}

#[test]
fn jit_project_live_object_identity_and_set_assignment_match_vm3() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim a As Widget\n  Dim b As Widget\n  Dim o As Object\n  Dim v As Variant\n  Set a = New Widget\n  Set b = a\n  Set o = b\n  Set v = o\n  If a Is b Then r = r + 1\n  If b Is o Then r = r + 10\n  If o Is v Then r = r + 100\n  Set b = Nothing\n  If a Is Nothing Then r = r + 1000\n  If b Is Nothing Then r = r + 10000\nEnd Sub\n",
        ),
        ("Widget", Class, "' project class marker\n"),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 10111);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 10111);
}

#[test]
fn jit_set_object_from_scalar_raises_object_required_without_vm_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  On Error Resume Next\n  Dim o As Object\n  Set o = 1\n  r = Err.Number\nEnd Sub\n",
        ),
        ("Widget", Class, "' project class marker\n"),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 424);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 424);
}

#[test]
fn jit_let_object_from_nothing_raises_object_variable_not_set_without_vm_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  On Error Resume Next\n  Dim o As Object\n  o = Nothing\n  r = Err.Number\nEnd Sub\n",
        ),
        ("Widget", Class, "' project class marker\n"),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 91);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 91);
}

#[test]
fn jit_is_operator_variant_scalars_raise_object_required_without_vm_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  On Error Resume Next\n  Dim a As Variant\n  Dim b As Variant\n  Dim c As Boolean\n  a = 1\n  b = 2\n  c = (a Is b)\n  r = Err.Number\nEnd Sub\n",
        ),
        ("Widget", Class, "' project class marker\n"),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 424);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 424);
}

#[test]
fn jit_project_member_dispatch_on_unset_object_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  On Error Resume Next\n  Dim w As Widget\n  r = w.Value\n  r = Err.Number\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Public Property Get Value() As Long\n  Value = 5\nEnd Property\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 91);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 91);
}

#[test]
fn jit_project_method_named_dispatch_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  r = w.Pick(second:=20, first:=10)\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Public Function Pick(ByVal first As Long, ByVal second As Long) As Long\n  Pick = first\nEnd Function\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 10);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 10);
}

#[test]
fn jit_project_untyped_variant_named_method_dispatch_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim c\n  Set c = New Calc\n  r = c.Add(2, 3)\nEnd Sub\n",
        ),
        (
            "Calc",
            Class,
            "Public Function Add(ByVal a As Long, ByVal b As Long) As Long\n  Add = a + b\nEnd Function\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 5);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 5);
}

#[test]
fn jit_project_object_named_method_dispatch_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim c As Object\n  Set c = New Calc\n  r = c.Add(4, 6)\nEnd Sub\n",
        ),
        (
            "Calc",
            Class,
            "Public Function Add(ByVal a As Long, ByVal b As Long) As Long\n  Add = a + b\nEnd Function\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 10);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 10);
}

#[test]
fn jit_call_by_name_project_method_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim c As Object\n  Set c = New Calc\n  r = CallByName(c, \"Add\", vbMethod, 7, 8)\nEnd Sub\n",
        ),
        (
            "Calc",
            Class,
            "Public Function Add(ByVal a As Long, ByVal b As Long) As Long\n  Add = a + b\nEnd Function\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 15);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 15);
}

#[test]
fn jit_call_by_name_project_property_get_let_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim c As Object\n  Set c = New Calc\n  CallByName c, \"Value\", vbLet, 42\n  r = CallByName(c, \"Value\", vbGet)\nEnd Sub\n",
        ),
        (
            "Calc",
            Class,
            "Private m As Long\nPublic Property Get Value() As Long\n  Value = m\nEnd Property\nPublic Property Let Value(ByVal v As Long)\n  m = v\nEnd Property\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 42);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 42);
}

#[test]
fn jit_call_by_name_invalid_calltype_raises_error_5_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  On Error Resume Next\n  Dim c As Object\n  Set c = New Calc\n  r = CallByName(c, \"Add\", 99, 1, 2)\n  r = Err.Number\nEnd Sub\n",
        ),
        (
            "Calc",
            Class,
            "Public Function Add(ByVal a As Long, ByVal b As Long) As Long\n  Add = a + b\nEnd Function\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 5);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 5);
}

#[test]
fn jit_project_method_optional_dispatch_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  r = w.DefaultBonus()\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Public Function DefaultBonus(Optional ByVal bonus As Long = 5) As Long\n  DefaultBonus = bonus\nEnd Function\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 5);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 5);
}

#[test]
fn jit_project_method_paramarray_dispatch_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  r = w.Second(4, 9)\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Public Function Second(ParamArray xs() As Variant) As Long\n  Second = xs(1)\nEnd Function\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 9);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 9);
}

#[test]
fn jit_project_method_byref_alias_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim w As Widget\n  Dim n As Long\n  Set w = New Widget\n  n = 4\n  w.Bump n\n  r = n\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Public Sub Bump(ByRef value As Long)\n  value = value + 3\nEnd Sub\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 7);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 7);
}

#[test]
fn jit_project_indexed_property_get_let_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  w.Value(3) = 10\n  r = w.Value(2)\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Private m As Long\nPublic Property Get Value(ByVal i As Long) As Long\n  Value = m\nEnd Property\nPublic Property Let Value(ByVal i As Long, ByVal v As Long)\n  m = i\nEnd Property\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 3);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 3);
}

#[test]
fn jit_project_property_set_dispatch_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim b As Box\n  Dim t As Thing\n  Dim got As Thing\n  Set b = New Box\n  Set t = New Thing\n  Set b.Item(3) = t\n  Set got = b.Item(2)\n  r = got.GetVal()\nEnd Sub\n",
        ),
        (
            "Box",
            Class,
            "Private stored As Thing\nPublic Property Get Item(ByVal i As Long) As Thing\n  Set Item = stored\nEnd Property\nPublic Property Set Item(ByVal i As Long, ByVal v As Thing)\n  Set stored = v\nEnd Property\n",
        ),
        (
            "Thing",
            Class,
            "Public Function GetVal() As Long\n  GetVal = 23\nEnd Function\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 23);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 23);
}

#[test]
fn jit_project_object_default_member_get_let_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim o As Object\n  Set o = New Widget\n  o(i := 3) = 10\n  r = o(i := 2)\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Private m As Long\nPublic Property Get Value(ByVal i As Long) As Long\n  Value = m\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Let Value(ByVal i As Long, ByVal v As Long)\n  m = i\nEnd Property\nAttribute Value.VB_UserMemId = 0\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 3);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 3);
}

#[test]
fn jit_project_variant_default_member_get_let_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim v As Variant\n  Set v = New Widget\n  v(4) = 10\n  r = v(1)\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Private m As Long\nPublic Property Get Value(ByVal i As Long) As Long\n  Value = m\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Property Let Value(ByVal i As Long, ByVal v As Long)\n  m = i\nEnd Property\nAttribute Value.VB_UserMemId = 0\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 4);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 4);
}

#[test]
fn jit_project_variant_default_member_property_set_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim v As Variant\n  Dim t As Thing\n  Dim got As Thing\n  Set v = New Box\n  Set t = New Thing\n  Set v(3) = t\n  Set got = v(2)\n  r = got.GetVal()\nEnd Sub\n",
        ),
        (
            "Box",
            Class,
            "Private stored As Thing\nPublic Property Get Item(ByVal i As Long) As Thing\n  Set Item = stored\nEnd Property\nAttribute Item.VB_UserMemId = 0\nPublic Property Set Item(ByVal i As Long, ByVal v As Thing)\n  Set stored = v\nEnd Property\nAttribute Item.VB_UserMemId = 0\n",
        ),
        (
            "Thing",
            Class,
            "Public Function GetVal() As Long\n  GetVal = 31\nEnd Function\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 31);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 31);
}

#[test]
fn jit_project_object_default_member_property_set_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim b As Object\n  Dim t As Thing\n  Dim got As Thing\n  Set b = New Box\n  Set t = New Thing\n  Set b(3) = t\n  Set got = b(2)\n  r = got.GetVal()\nEnd Sub\n",
        ),
        (
            "Box",
            Class,
            "Private stored As Thing\nPublic Property Get Item(ByVal i As Long) As Thing\n  Set Item = stored\nEnd Property\nAttribute Item.VB_UserMemId = 0\nPublic Property Set Item(ByVal i As Long, ByVal v As Thing)\n  Set stored = v\nEnd Property\nAttribute Item.VB_UserMemId = 0\n",
        ),
        (
            "Thing",
            Class,
            "Public Function GetVal() As Long\n  GetVal = 29\nEnd Function\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 29);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 29);
}

#[test]
fn jit_project_typeof_unset_object_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim w As Widget\n  If TypeOf w Is Widget Then\n    r = 7\n  Else\n    r = 3\n  End If\nEnd Sub\n",
        ),
        ("Widget", Class, "' project class marker\n"),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 3);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 3);
}

#[test]
fn jit_project_typeof_live_object_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  If TypeOf w Is Widget Then\n    r = 7\n  Else\n    r = 3\n  End If\nEnd Sub\n",
        ),
        ("Widget", Class, "' project class marker\n"),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 7);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 7);
}

#[test]
fn jit_project_typeof_nothing_matches_vm3_without_descriptors() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  If TypeOf Nothing Is Widget Then\n    r = 7\n  Else\n    r = 3\n  End If\nEnd Sub\n",
        ),
        ("Widget", Class, "' project class marker\n"),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 3);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 3);
}

#[test]
fn jit_project_typename_unset_object_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim w As Widget\n  If TypeName(w) = \"Nothing\" Then\n    r = 31\n  Else\n    r = 37\n  End If\nEnd Sub\n",
        ),
        ("Widget", Class, "' project class marker\n"),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 31);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 31);
}

#[test]
fn jit_project_typename_live_object_matches_vm3_without_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  If TypeName(w) = \"Widget\" Then\n    r = 37\n  Else\n    r = 31\n  End If\nEnd Sub\n",
        ),
        ("Widget", Class, "' project class marker\n"),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 37);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_with_i32("JIT", jit, 37);
}

#[test]
fn jit_predeclared_default_instance_matches_vm3_without_fallback() {
    let app = project(vec![
        proc_module(
            "Main",
            "Public r As Variant\nPublic initCount As Long\nSub Main()\n  r = Counter.Id\n  r = Counter.Id\nEnd Sub\n",
        ),
        predeclared_class_module(
            "Counter",
            "Private n As Long\nPrivate Sub Class_Initialize()\n  Main.initCount = Main.initCount + 1\n  n = Main.initCount\nEnd Sub\nPublic Property Get Id() As Long\n  Id = n\nEnd Property\n",
        ),
    ]);

    let vm3 = run_project_closure(Executor::Vm3, &[app.clone()]);
    assert_completed_prefix_i32("VM3", vm3, &[1, 1]);

    let jit = run_project_closure(Executor::Jit, &[app]);
    assert_completed_prefix_i32("JIT", jit, &[1, 1]);
}

#[test]
fn jit_predeclared_statement_method_uses_default_instance_without_fallback() {
    let app = project(vec![
        proc_module(
            "Main",
            "Public r As Variant\nPublic initCount As Long\nPublic called As Long\nSub Main()\n  Counter.Bump\n  Counter.Bump\n  r = Counter.Id\nEnd Sub\n",
        ),
        predeclared_class_module(
            "Counter",
            "Private n As Long\nPrivate Sub Class_Initialize()\n  Main.initCount = Main.initCount + 1\n  n = Main.initCount\nEnd Sub\nPublic Sub Bump()\n  Main.called = Main.called + 10\nEnd Sub\nPublic Property Get Id() As Long\n  Id = n\nEnd Property\n",
        ),
    ]);

    let vm3 = run_project_closure(Executor::Vm3, &[app.clone()]);
    assert_completed_prefix_i32("VM3", vm3, &[1, 1, 20]);

    let jit = run_project_closure(Executor::Jit, &[app]);
    assert_completed_prefix_i32("JIT", jit, &[1, 1, 20]);
}

#[test]
fn jit_predeclared_set_nothing_resets_default_instance_without_fallback() {
    let app = project(vec![
        proc_module(
            "Main",
            "Public r As Variant\nPublic initCount As Long\nSub Main()\n  Dim beforeId As Variant\n  beforeId = Counter.Id\n  Set Counter = Nothing\n  r = Counter.Id\nEnd Sub\n",
        ),
        predeclared_class_module(
            "Counter",
            "Private n As Long\nPrivate Sub Class_Initialize()\n  Main.initCount = Main.initCount + 1\n  n = Main.initCount\nEnd Sub\nPublic Property Get Id() As Long\n  Id = n\nEnd Property\n",
        ),
    ]);

    let vm3 = run_project_closure(Executor::Vm3, &[app.clone()]);
    assert_completed_prefix_i32("VM3", vm3, &[2, 2]);

    let jit = run_project_closure(Executor::Jit, &[app]);
    assert_completed_prefix_i32("JIT", jit, &[2, 2]);
}

#[test]
fn jit_predeclared_set_new_replaces_default_without_fallback() {
    let app = project(vec![
        proc_module(
            "Main",
            "Public r As Variant\nPublic initCount As Long\nSub Main()\n  Dim beforeId As Variant\n  beforeId = Counter.Id\n  Set Counter = New Counter\n  r = Counter.Id\nEnd Sub\n",
        ),
        predeclared_class_module(
            "Counter",
            "Private n As Long\nPrivate Sub Class_Initialize()\n  Main.initCount = Main.initCount + 1\n  n = Main.initCount\nEnd Sub\nPublic Property Get Id() As Long\n  Id = n\nEnd Property\n",
        ),
    ]);

    let vm3 = run_project_closure(Executor::Vm3, &[app.clone()]);
    assert_completed_prefix_i32("VM3", vm3, &[2, 2]);

    let jit = run_project_closure(Executor::Jit, &[app]);
    assert_completed_prefix_i32("JIT", jit, &[2, 2]);
}

#[test]
fn jit_predeclared_held_reference_survives_reset_without_fallback() {
    let app = project(vec![
        proc_module(
            "Main",
            "Public r As Variant\nPublic afterReset As Variant\nPublic initCount As Long\nSub Main()\n  Dim oldDefault As Counter\n  Set oldDefault = Counter\n  Set Counter = Nothing\n  r = oldDefault.Id\n  afterReset = Counter.Id\nEnd Sub\n",
        ),
        predeclared_class_module(
            "Counter",
            "Private n As Long\nPrivate Sub Class_Initialize()\n  Main.initCount = Main.initCount + 1\n  n = Main.initCount\nEnd Sub\nPublic Property Get Id() As Long\n  Id = n\nEnd Property\n",
        ),
    ]);

    let vm3 = run_project_closure(Executor::Vm3, &[app.clone()]);
    assert_completed_prefix_i32("VM3", vm3, &[1, 2, 2]);

    let jit = run_project_closure(Executor::Jit, &[app]);
    assert_completed_prefix_i32("JIT", jit, &[1, 2, 2]);
}

#[test]
fn jit_predeclared_failed_initialize_clears_slot_for_retry_without_fallback() {
    let app = project(vec![
        proc_module(
            "Main",
            "Public r As Variant\nPublic initCount As Long\nPublic firstErr As Long\nPublic failOnce As Boolean\nSub Main()\n  failOnce = True\n  On Error Resume Next\n  Dim firstId As Variant\n  firstId = Counter.Id\n  firstErr = Err.Number\n  Err.Clear\n  r = Counter.Id\nEnd Sub\n",
        ),
        predeclared_class_module(
            "Counter",
            "Private n As Long\nPrivate Sub Class_Initialize()\n  Main.initCount = Main.initCount + 1\n  If Main.failOnce Then\n    Main.failOnce = False\n    Err.Raise 5\n  End If\n  n = Main.initCount\nEnd Sub\nPublic Property Get Id() As Long\n  Id = n\nEnd Property\n",
        ),
    ]);

    let vm3 = run_project_closure(Executor::Vm3, &[app.clone()]);
    assert_completed_prefix_i32("VM3", vm3, &[2, 2, 5]);

    let jit = run_project_closure(Executor::Jit, &[app]);
    assert_completed_prefix_i32("JIT", jit, &[2, 2, 5]);
}

#[test]
fn jit_project_set_nothing_runs_class_terminate_before_next_statement() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nPublic initCount As Long\nPublic termCount As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  Set w = Nothing\n  r = termCount\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Private Sub Class_Initialize()\n  Main.initCount = Main.initCount + 1\nEnd Sub\nPrivate Sub Class_Terminate()\n  Main.termCount = Main.termCount + 1\n  Err.Raise 77\nEnd Sub\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_prefix_i32("VM3", vm3, &[1, 1, 1]);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_prefix_i32("JIT", jit, &[1, 1, 1]);
}

#[test]
fn jit_project_local_object_terminates_after_callee_exit_before_caller_statement() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nPublic initCount As Long\nPublic termCount As Long\nSub Main()\n  MakeWidget\n  r = termCount\nEnd Sub\nSub MakeWidget()\n  Dim w As Widget\n  Set w = New Widget\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Private Sub Class_Initialize()\n  Main.initCount = Main.initCount + 1\nEnd Sub\nPrivate Sub Class_Terminate()\n  Main.termCount = Main.termCount + 1\n  Err.Raise 77\nEnd Sub\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_prefix_i32("VM3", vm3, &[1, 1, 1]);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_prefix_i32("JIT", jit, &[1, 1, 1]);
}

#[test]
fn jit_project_initialize_failure_terminates_partial_instance_and_retries() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nPublic initCount As Long\nPublic termCount As Long\nPublic firstErr As Long\nPublic failOnce As Boolean\nSub Main()\n  failOnce = True\n  On Error Resume Next\n  Dim w As Widget\n  Set w = New Widget\n  firstErr = Err.Number\n  Err.Clear\n  Set w = New Widget\n  Set w = Nothing\n  r = termCount\nEnd Sub\n",
        ),
        (
            "Widget",
            Class,
            "Private Sub Class_Initialize()\n  Main.initCount = Main.initCount + 1\n  If Main.failOnce Then\n    Main.failOnce = False\n    Err.Raise 5\n  End If\nEnd Sub\nPrivate Sub Class_Terminate()\n  Main.termCount = Main.termCount + 1\n  Err.Raise 77\nEnd Sub\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_prefix_i32("VM3", vm3, &[2, 2, 2, 5]);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_prefix_i32("JIT", jit, &[2, 2, 2, 5]);
}

#[test]
fn jit_project_class_terminate_field_release_cascades_to_child() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nPublic ownerInit As Long\nPublic ownerTerm As Long\nPublic childInit As Long\nPublic childTerm As Long\nSub Main()\n  Dim o As Owner\n  Set o = New Owner\n  Set o = Nothing\n  r = childTerm\nEnd Sub\n",
        ),
        (
            "Owner",
            Class,
            "Private child As Child\nPrivate Sub Class_Initialize()\n  Main.ownerInit = Main.ownerInit + 1\n  Set child = New Child\nEnd Sub\nPrivate Sub Class_Terminate()\n  Main.ownerTerm = Main.ownerTerm + 1\nEnd Sub\n",
        ),
        (
            "Child",
            Class,
            "Private Sub Class_Initialize()\n  Main.childInit = Main.childInit + 1\nEnd Sub\nPrivate Sub Class_Terminate()\n  Main.childTerm = Main.childTerm + 1\n  Err.Raise 88\nEnd Sub\n",
        ),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_prefix_i32("VM3", vm3, &[1, 1, 1, 1, 1]);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_completed_prefix_i32("JIT", jit, &[1, 1, 1, 1, 1]);
}

#[test]
fn jit_referenced_project_new_class_property_call_matches_vm3_without_fallback() {
    let lib = referenced(
        "Lib",
        vec![exposed_class_module(
            "Widget",
            "Private m As Long\nPrivate Sub Class_Initialize()\n  m = 42\nEnd Sub\nPublic Property Get Value() As Long\n  Value = m\nEnd Property\n",
        )],
    );
    let app = project_with_refs(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\nPublic nameLen As Long\nPublic isWidget As Long\nSub Main()\n  Dim w As Lib.Widget\n  Dim b As Lib.Widget\n  Set w = New Lib.Widget\n  Set b = w\n  r = b.Value\n  nameLen = Len(TypeName(b))\n  If TypeOf b Is Lib.Widget Then\n    isWidget = 1\n  Else\n    isWidget = 2\n  End If\nEnd Sub\n",
        )],
        vec![lib.clone()],
    );
    let lib_project = library_project(&lib);

    let vm3 = run_project_closure(Executor::Vm3, &[lib_project.clone(), app.clone()]);
    assert_completed_prefix_i32("VM3", vm3, &[42, 6, 1]);

    let jit = run_project_closure(Executor::Jit, &[lib_project, app]);
    assert_completed_prefix_i32("JIT", jit, &[42, 6, 1]);
}

#[test]
fn jit_referenced_project_predeclared_singleton_matches_vm3_without_fallback() {
    let lib = referenced(
        "Lib",
        vec![predeclared_class_module(
            "Host",
            "Private m As Long\nPrivate Sub Class_Initialize()\n  m = 17\nEnd Sub\nPublic Sub Bump()\n  m = 18\nEnd Sub\nPublic Property Get Value() As Long\n  Value = m\nEnd Property\n",
        )],
    );
    let app = project_with_refs(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\nSub Main()\n  Host.Bump\n  r = Host.Value\nEnd Sub\n",
        )],
        vec![lib.clone()],
    );
    let lib_project = library_project(&lib);

    let vm3 = run_project_closure(Executor::Vm3, &[lib_project.clone(), app.clone()]);
    assert_completed_with_i32("VM3", vm3, 18);

    let jit = run_project_closure(Executor::Jit, &[lib_project, app]);
    assert_completed_with_i32("JIT", jit, 18);
}

#[test]
fn jit_referenced_project_incompatible_assignment_diagnostic_matches_vm3_without_fallback() {
    let lib = referenced(
        "Lib",
        vec![
            exposed_class_module(
                "Alpha",
                "Public Property Get Value() As Long\n  Value = 1\nEnd Property\n",
            ),
            exposed_class_module(
                "Beta",
                "Public Property Get Value() As Long\n  Value = 2\nEnd Property\n",
            ),
        ],
    );
    let app = project_with_refs(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\nPublic okErr As Long\nSub Main()\n  Dim a As Lib.Alpha\n  Dim b As Lib.Beta\n  Set b = New Lib.Beta\n  On Error Resume Next\n  Set a = b\n  r = Err.Number\n  Err.Clear\n  Set a = New Lib.Alpha\n  okErr = Err.Number\n  On Error GoTo 0\nEnd Sub\n",
        )],
        vec![lib.clone()],
    );
    let lib_project = library_project(&lib);

    let vm3 = run_project_closure(Executor::Vm3, &[lib_project.clone(), app.clone()]);
    assert_completed_prefix_i32("VM3", vm3, &[13, 0]);

    let jit = run_project_closure(Executor::Jit, &[lib_project, app]);
    assert_completed_prefix_i32("JIT", jit, &[13, 0]);
}
