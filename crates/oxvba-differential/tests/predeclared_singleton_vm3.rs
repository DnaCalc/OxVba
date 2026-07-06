//! `VB_PredeclaredId` default-instance lifecycle fixtures.
//!
//! Live Excel/VBA 7.1 oracle evidence is captured in:
//! `docs/evidence/conformance/vm3_predeclared_singleton_oracle_20260702T080743Z/`.

use std::collections::BTreeMap;

use oxvba_differential::{Canon, Executor, RunOutcome, run_project_closure};
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

fn referenced(project_name: &str, modules: Vec<ModuleUnit>) -> ReferencedProjectManifest {
    ReferencedProjectManifest {
        project_name: project_name.to_string(),
        project_kind: ProjectKind::Library,
        modules,
    }
}

fn project(
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

fn counter_class() -> ModuleUnit {
    predeclared_class_module(
        "Counter",
        "Private n As Long\n\
         Private Sub Class_Initialize()\n\
         \x20   n = 10\n\
         \x20   Main.Log = Main.Log & \"I;\"\n\
         End Sub\n\
         Private Sub Class_Terminate()\n\
         \x20   Main.Log = Main.Log & \"T\" & CStr(n) & \";\"\n\
         End Sub\n\
         Public Sub Bump()\n\
         \x20   n = n + 1\n\
         End Sub\n\
         Public Property Get Total() As Long\n\
         \x20   Total = n\n\
         End Property\n",
    )
}

fn app_with_main(main_src: &str) -> SymbolProjectManifest {
    project(
        "App",
        vec![proc_module("Main", main_src), counter_class()],
        vec![],
    )
}

fn run_case(main_src: &str) -> RunOutcome {
    let app = app_with_main(main_src);
    run_project_closure(Executor::Vm3, &[app])
}

fn assert_contains_string(outcome: RunOutcome, expected: &str) {
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined predeclared singleton case as unsupported: {:?}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 predeclared singleton case failed: {err}"));
    let expected = Canon::Str(expected.to_string());
    assert!(
        values.contains(&expected),
        "snapshot {values:?} did not contain {expected:?}"
    );
}

#[test]
fn predeclared_access_persists_default_instance() {
    assert_contains_string(
        run_case(
            "Public result As Variant\n\
             Public Log As String\n\
             Sub Main()\n\
             \x20   Counter.Bump\n\
             \x20   Counter.Bump\n\
             \x20   result = CStr(Counter.Total) & \"|\" & Log\n\
             End Sub\n",
        ),
        "12|I;",
    );
}

#[test]
fn dropping_local_reference_does_not_reset_predeclared_instance() {
    assert_contains_string(
        run_case(
            "Public result As Variant\n\
             Public Log As String\n\
             Sub Main()\n\
             \x20   Dim c As Counter\n\
             \x20   Set c = Counter\n\
             \x20   c.Bump\n\
             \x20   Set c = Nothing\n\
             \x20   result = CStr(Counter.Total) & \"|\" & Log\n\
             End Sub\n",
        ),
        "11|I;",
    );
}

#[test]
fn set_predeclared_nothing_resets_default_instance() {
    assert_contains_string(
        run_case(
            "Public result As Variant\n\
             Public Log As String\n\
             Sub Main()\n\
             \x20   Counter.Bump\n\
             \x20   Dim beforeTotal As Long\n\
             \x20   beforeTotal = Counter.Total\n\
             \x20   Set Counter = Nothing\n\
             \x20   result = CStr(beforeTotal) & \":\" & CStr(Counter.Total) & \"|\" & Log\n\
             End Sub\n",
        ),
        "11:10|I;T11;I;",
    );
}

#[test]
fn set_predeclared_new_replaces_default_after_rhs_initialization() {
    assert_contains_string(
        run_case(
            "Public result As Variant\n\
             Public Log As String\n\
             Sub Main()\n\
             \x20   Counter.Bump\n\
             \x20   Set Counter = New Counter\n\
             \x20   result = CStr(Counter.Total) & \"|\" & Log\n\
             End Sub\n",
        ),
        "10|I;I;T11;",
    );
}

#[test]
fn held_reference_survives_default_slot_reset() {
    assert_contains_string(
        run_case(
            "Public result As Variant\n\
             Public Log As String\n\
             Sub Main()\n\
             \x20   Dim oldDefault As Counter\n\
             \x20   Set oldDefault = Counter\n\
             \x20   Counter.Bump\n\
             \x20   Set Counter = Nothing\n\
             \x20   result = CStr(oldDefault.Total) & \":\" & CStr(Counter.Total) & \"|\" & Log\n\
             End Sub\n",
        ),
        "11:10|I;I;",
    );
}

#[test]
fn failed_predeclared_initialize_clears_default_slot_for_retry() {
    let counter = predeclared_class_module(
        "Counter",
        "Private n As Long\n\
         Private Sub Class_Initialize()\n\
         \x20   Main.Log = Main.Log & \"I;\"\n\
         \x20   If Main.FailOnce Then\n\
         \x20       Main.FailOnce = False\n\
         \x20       Err.Raise 5\n\
         \x20   End If\n\
         \x20   n = 10\n\
         End Sub\n\
         Private Sub Class_Terminate()\n\
         \x20   Main.Log = Main.Log & \"T;\"\n\
         End Sub\n\
         Public Sub Bump()\n\
         \x20   n = n + 1\n\
         End Sub\n\
         Public Property Get Total() As Long\n\
         \x20   Total = n\n\
         End Property\n",
    );
    let app = project(
        "App",
        vec![
            proc_module(
                "Main",
                "Public result As Variant\n\
                 Public Log As String\n\
                 Public FailOnce As Boolean\n\
                 Sub Main()\n\
                 \x20   FailOnce = True\n\
                 \x20   On Error Resume Next\n\
                 \x20   Counter.Bump\n\
                 \x20   Dim firstErr As Long\n\
                 \x20   firstErr = Err.Number\n\
                 \x20   Err.Clear\n\
                 \x20   Dim afterFail As String\n\
                 \x20   afterFail = Log\n\
                 \x20   result = CStr(Counter.Total) & \"|\" & afterFail & \"|\" & Log & \"|\" & CStr(firstErr)\n\
                 End Sub\n",
            ),
            counter,
        ],
        vec![],
    );

    assert_contains_string(
        run_project_closure(Executor::Vm3, &[app]),
        "10|I;T;|I;T;I;|5",
    );
}

#[test]
fn failed_predeclared_initialize_preserves_replaced_default_slot() {
    let counter = predeclared_class_module(
        "Counter",
        "Private n As Long\n\
         Private Sub Class_Initialize()\n\
         \x20   Main.Log = Main.Log & \"I;\"\n\
         \x20   If Main.FailOnce Then\n\
         \x20       Main.FailOnce = False\n\
         \x20       Main.ReplaceDefault\n\
         \x20       Err.Raise 5\n\
         \x20   End If\n\
         \x20   n = 10\n\
         End Sub\n\
         Private Sub Class_Terminate()\n\
         \x20   Main.Log = Main.Log & \"T;\"\n\
         End Sub\n\
         Public Property Get Total() As Long\n\
         \x20   Total = n\n\
         End Property\n",
    );
    let app = project(
        "App",
        vec![
            proc_module(
                "Main",
                "Public result As Variant\n\
                 Public Log As String\n\
                 Public FailOnce As Boolean\n\
                 Public Sub ReplaceDefault()\n\
                 \x20   Set Counter = New Counter\n\
                 End Sub\n\
                 Sub Main()\n\
                 \x20   FailOnce = True\n\
                 \x20   On Error Resume Next\n\
                 \x20   Counter.Total\n\
                 \x20   Dim firstErr As Long\n\
                 \x20   firstErr = Err.Number\n\
                 \x20   Err.Clear\n\
                 \x20   Dim afterFail As String\n\
                 \x20   afterFail = Log\n\
                 \x20   result = CStr(Counter.Total) & \"|\" & afterFail & \"|\" & Log & \"|\" & CStr(firstErr)\n\
                 End Sub\n",
            ),
            counter,
        ],
        vec![],
    );

    assert_contains_string(
        run_project_closure(Executor::Vm3, &[app]),
        "10|I;I;T;|I;I;T;|5",
    );
}

#[test]
fn cross_project_set_predeclared_nothing_resets_owning_default_slot() {
    let host = || {
        predeclared_class_module(
            "HostEnv",
            "Private n As Long\n\
             Private Sub Class_Initialize()\n\
             \x20   n = 10\n\
             End Sub\n\
             Public Sub Bump()\n\
             \x20   n = n + 1\n\
             End Sub\n\
             Public Property Get Total() As Long\n\
             \x20   Total = n\n\
             End Property\n",
        )
    };
    let lib = project("Lib", vec![host()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public result As Variant\n\
             Sub Main()\n\
             \x20   HostEnv.Bump\n\
             \x20   Set HostEnv = Nothing\n\
             \x20   result = CStr(HostEnv.Total)\n\
             End Sub\n",
        )],
        vec![referenced("Lib", vec![host()])],
    );

    assert_contains_string(run_project_closure(Executor::Vm3, &[lib, app]), "10");
}
