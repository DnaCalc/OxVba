use std::collections::BTreeMap;

use oxvba_differential::{Executor, RunOutcome, canon, run_modules, run_project_closure};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, SymbolProjectManifest,
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

fn assert_jit_declines(outcome: RunOutcome, instruction: &str, expected_detail: &str) {
    let unsupported = outcome
        .unsupported
        .as_deref()
        .unwrap_or_else(|| panic!("JIT should decline {instruction} explicitly: {outcome:?}"));
    assert!(
        unsupported.contains(instruction) && unsupported.contains(expected_detail),
        "unexpected JIT unsupported diagnostic for {instruction}: {unsupported}"
    );
    assert!(
        matches!(outcome.result.as_ref(), Ok(values) if values.is_empty()),
        "JIT decline must not return a VM3 result: {outcome:?}"
    );
    assert!(
        outcome
            .handle_balance
            .is_some_and(oxvba_runtime::HandleBalance::is_zero),
        "jit decline handle imbalance: {:?}",
        outcome.handle_balance
    );
}

#[test]
fn jit_project_class_new_declines_without_vm_fallback() {
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
    assert_jit_declines(jit, "NewObject", "VM3-only");
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
fn jit_project_dim_as_new_is_nothing_declines_without_vm_fallback() {
    let modules = [
        (
            "Main",
            Procedural,
            "Public r As Long\nSub Main()\n  Dim w As New Widget\n  If w Is Nothing Then\n    r = 41\n  Else\n    r = 43\n  End If\nEnd Sub\n",
        ),
        ("Widget", Class, "' project class marker\n"),
    ];

    let vm3 = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert_completed_with_i32("VM3", vm3, 43);

    let jit = run_modules(Executor::Jit, &modules, "VBAProject");
    assert_jit_declines(jit, "AsNew", "lazy activation");
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
fn jit_project_member_dispatch_on_unset_object_declines_without_vm_fallback() {
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
    assert_jit_declines(jit, "ComCallLate", "late-bound COM invocation");
}

#[test]
fn jit_project_typeof_declines_without_vm_fallback() {
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
    assert_jit_declines(jit, "TypeOfIs", "runtime descriptors");
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
fn jit_project_typename_object_declines_without_vm_fallback() {
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
    assert_jit_declines(jit, "TypeName", "VarType/TypeName/Is*");
}

#[test]
fn jit_predeclared_default_instance_declines_without_vm_fallback() {
    let app = project(vec![
        proc_module(
            "Main",
            "Public r As Long\nSub Main()\n  r = Counter.Total\nEnd Sub\n",
        ),
        predeclared_class_module(
            "Counter",
            "Private n As Long\nPrivate Sub Class_Initialize()\n  n = 10\nEnd Sub\nPublic Property Get Total() As Long\n  Total = n\nEnd Property\n",
        ),
    ]);

    let vm3 = run_project_closure(Executor::Vm3, &[app.clone()]);
    assert_completed_with_i32("VM3", vm3, 10);

    let jit = run_project_closure(Executor::Jit, &[app]);
    assert_jit_declines(jit, "Predeclared", "VM3-only");
}
