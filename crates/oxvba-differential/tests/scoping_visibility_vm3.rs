//! Multi-module scoping/visibility fixtures for the `bd-4ktq.9` batch.
//!
//! Live Excel/VBA 7.1 oracle evidence is captured in:
//! `docs/evidence/conformance/vm3_scoping_visibility_oracle_20260701T0945Z/`.
//! The passing tests below pin legal baseline shapes. The ignored tests encode the
//! oracle-backed gaps that the follow-on scoping beads are expected to unignore
//! and satisfy as each resolver diagnostic is implemented.

use std::collections::BTreeMap;

use oxvba_differential::{
    Canon, Executor, RunOutcome, canon, run_modules, run_project_closure,
};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectReference,
    ReferencedProjectManifest, SymbolProjectManifest,
};

fn run_scoping_case(modules: &[(&str, oxvba_symbol::manifest::ModuleKind, &str)]) -> RunOutcome {
    run_modules(Executor::Vm3, modules, "VBAProject")
}

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

fn class_module(name: &str, src: &str) -> ModuleUnit {
    let mut module = module(name, Class, src);
    module.attributes.vb_exposed = true;
    module.attributes.vb_creatable = true;
    module
}

fn option_private_proc_module(name: &str, src: &str) -> ModuleUnit {
    let mut module = proc_module(name, src);
    module.attributes.option_private_module = true;
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
    }
}

fn run_scoping_closure(closure_leaf_first: &[SymbolProjectManifest]) -> RunOutcome {
    run_project_closure(Executor::Vm3, closure_leaf_first)
}

fn assert_snapshot_contains(outcome: RunOutcome, expected: Canon) {
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined scoping case as unsupported: {:?}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 scoping case failed: {err}"));
    assert!(
        values.contains(&expected),
        "snapshot {values:?} did not contain {expected:?}"
    );
}

fn assert_project_closure_contains(closure_leaf_first: &[SymbolProjectManifest], expected: Canon) {
    assert_snapshot_contains(run_scoping_closure(closure_leaf_first), expected);
}

fn assert_compile_rejected(outcome: RunOutcome) {
    assert!(
        outcome.unsupported.is_some() || outcome.result.is_err() || outcome.raised,
        "expected compile/bind rejection or failure, got {outcome:?}"
    );
}

fn assert_ambiguous_compile_rejected(outcome: RunOutcome, name: &str) {
    assert!(
        outcome.unsupported.is_none(),
        "expected ambiguity diagnostic, got unsupported: {:?}",
        outcome.unsupported
    );
    let err = outcome
        .result
        .expect_err("expected duplicate-public case to fail binding");
    let err_lower = err.to_ascii_lowercase();
    assert!(
        err_lower.contains("ambiguous") && err_lower.contains(&name.to_ascii_lowercase()),
        "expected ambiguity diagnostic for {name}, got {err:?}"
    );
}

fn assert_module_name_collision_rejected(outcome: RunOutcome) {
    assert!(
        outcome.unsupported.is_none(),
        "expected module-name collision diagnostic, got unsupported: {:?}",
        outcome.unsupported
    );
    let err = outcome
        .result
        .expect_err("expected module-name collision to fail binding");
    let err_lower = err.to_ascii_lowercase();
    assert!(
        (err_lower.contains("expected variable or procedure")
            || err.contains("ExpectedVariableOrProcedureNotModule"))
            && err_lower.contains("module"),
        "expected module-name collision diagnostic, got {err:?}"
    );
}

#[test]
fn same_module_private_member_matches_oracle() {
    assert_snapshot_contains(
        run_scoping_case(&[(
            "Main",
            Procedural,
            "Private Function Secret() As Long\n    Secret = 7\nEnd Function\n\nPublic result As Variant\nSub Main()\n    result = Secret()\nEnd Sub\n",
        )]),
        canon(&Variant::from_i32(7)),
    );
}

#[test]
fn cross_module_public_unqualified_matches_oracle() {
    assert_snapshot_contains(
        run_scoping_case(&[
            (
                "Main",
                Procedural,
                "Public result As Variant\nSub Main()\n    result = Pub()\nEnd Sub\n",
            ),
            (
                "Lib",
                Procedural,
                "Public Function Pub() As Long\n    Pub = 12\nEnd Function\n",
            ),
        ]),
        canon(&Variant::from_i32(12)),
    );
}

#[test]
fn cross_module_public_qualified_matches_oracle() {
    assert_snapshot_contains(
        run_scoping_case(&[
            (
                "Main",
                Procedural,
                "Public result As Variant\nSub Main()\n    result = Lib.Pub()\nEnd Sub\n",
            ),
            (
                "Lib",
                Procedural,
                "Public Function Pub() As Long\n    Pub = 13\nEnd Function\n",
            ),
        ]),
        canon(&Variant::from_i32(13)),
    );
}

#[test]
fn valid_project_qualifier_should_match_oracle() {
    assert_snapshot_contains(
        run_scoping_case(&[
            (
                "Main",
                Procedural,
                "Option Explicit\n\nPublic result As Variant\nSub Main()\n    result = VBAProject.Lib.Pub()\nEnd Sub\n",
            ),
            (
                "Lib",
                Procedural,
                "Public Function Pub() As Long\n    Pub = 13\nEnd Function\n",
            ),
        ]),
        canon(&Variant::from_i32(13)),
    );
}

#[test]
fn class_friend_member_matches_oracle() {
    assert_snapshot_contains(
        run_scoping_case(&[
            (
                "Main",
                Procedural,
                "Public result As Variant\nSub Main()\n    Dim w As Widget\n    Set w = New Widget\n    result = w.FriendValue()\nEnd Sub\n",
            ),
            (
                "Widget",
                Class,
                "Friend Function FriendValue() As Long\n    FriendValue = 19\nEnd Function\n",
            ),
        ]),
        canon(&Variant::from_i32(19)),
    );
}

#[test]
fn private_cross_module_unqualified_should_be_rejected() {
    assert_compile_rejected(run_scoping_case(&[
        (
            "Main",
            Procedural,
            "Public result As Variant\nSub Main()\n    result = Secret()\nEnd Sub\n",
        ),
        (
            "Lib",
            Procedural,
            "Private Function Secret() As Long\n    Secret = 9\nEnd Function\n",
        ),
    ]));
}

#[test]
fn private_cross_module_qualified_should_be_rejected() {
    assert_compile_rejected(run_scoping_case(&[
        (
            "Main",
            Procedural,
            "Public result As Variant\nSub Main()\n    result = Lib.Secret()\nEnd Sub\n",
        ),
        (
            "Lib",
            Procedural,
            "Private Function Secret() As Long\n    Secret = 11\nEnd Function\n",
        ),
    ]));
}

#[test]
fn duplicate_public_unqualified_should_be_ambiguous() {
    assert_ambiguous_compile_rejected(
        run_scoping_case(&[
            (
                "Main",
                Procedural,
                "Public result As Variant\nSub Main()\n    result = Dup()\nEnd Sub\n",
            ),
            (
                "Alpha",
                Procedural,
                "Public Function Dup() As Long\n    Dup = 1\nEnd Function\n",
            ),
            (
                "Beta",
                Procedural,
                "Public Function Dup() As Long\n    Dup = 2\nEnd Function\n",
            ),
        ]),
        "Dup",
    );
}

#[test]
fn module_name_public_member_collision_should_be_rejected() {
    assert_module_name_collision_rejected(run_scoping_case(&[
        (
            "Main",
            Procedural,
            "Public result As Variant\nSub Main()\n    result = Clash()\nEnd Sub\n",
        ),
        (
            "Clash",
            Procedural,
            "Public Function Value() As Long\n    Value = 3\nEnd Function\n",
        ),
        (
            "Other",
            Procedural,
            "Public Function Clash() As Long\n    Clash = 4\nEnd Function\n",
        ),
    ]));
}

#[test]
fn wrong_project_qualifier_should_be_rejected() {
    assert_compile_rejected(run_scoping_case(&[
        (
            "Main",
            Procedural,
            "Option Explicit\n\nPublic result As Variant\nSub Main()\n    result = WrongProject.Lib.Pub()\nEnd Sub\n",
        ),
        (
            "Lib",
            Procedural,
            "Public Function Pub() As Long\n    Pub = 17\nEnd Function\n",
        ),
    ]));
}

#[test]
fn friend_on_standard_module_should_be_rejected() {
    assert_compile_rejected(run_scoping_case(&[(
        "Main",
        Procedural,
        "Friend Sub Helper()\nEnd Sub\n\nPublic result As Variant\nSub Main()\n    Helper\n    result = 1\nEnd Sub\n",
    )]));
}

// Follow-up scoping batch: project-reference fixture surface.

fn reference_tools() -> Vec<ModuleUnit> {
    vec![proc_module(
        "RefTools",
        "Public Function RefValue() As Long\n    RefValue = 30\nEnd Function\n",
    )]
}

fn source_events() -> Vec<ModuleUnit> {
    vec![class_module(
        "Clock",
        "Public Event Tick(ByVal n As Long)\n\
         Public Sub Fire()\n    RaiseEvent Tick(23)\nEnd Sub\n",
    )]
}

#[test]
fn cross_project_fixture_baseline_uses_two_active_modules_and_reference() {
    let lib = project("LibProj", reference_tools(), vec![]);
    let app = project(
        "AppProj",
        vec![
            proc_module(
                "Main",
                "Public result As Variant\n\
                 Sub Main()\n\
                 \x20   result = LocalValue() + RefValue()\n\
                 End Sub\n",
            ),
            proc_module(
                "LocalTools",
                "Public Function LocalValue() As Long\n    LocalValue = 12\nEnd Function\n",
            ),
        ],
        vec![referenced("LibProj", reference_tools())],
    );
    assert_project_closure_contains(&[lib, app], canon(&Variant::from_i32(42)));
}

#[test]
fn cross_project_module_qualified_reference_call_matches_current_baseline() {
    let lib = project("LibProj", reference_tools(), vec![]);
    let app = project(
        "AppProj",
        vec![
            proc_module(
                "Main",
                "Public result As Variant\n\
                 Sub Main()\n\
                 \x20   result = LocalTools.LocalValue() + RefTools.RefValue()\n\
                 End Sub\n",
            ),
            proc_module(
                "LocalTools",
                "Public Function LocalValue() As Long\n    LocalValue = 12\nEnd Function\n",
            ),
        ],
        vec![referenced("LibProj", reference_tools())],
    );
    assert_project_closure_contains(&[lib, app], canon(&Variant::from_i32(42)));
}

#[test]
fn public_const_variable_collision_should_be_ambiguous() {
    assert_ambiguous_compile_rejected(
        run_scoping_case(&[
            (
                "Main",
                Procedural,
                "Public result As Variant\nSub Main()\n    result = SharedName\nEnd Sub\n",
            ),
            ("Alpha", Procedural, "Public Const SharedName As Long = 1\n"),
            ("Beta", Procedural, "Public SharedName As Long\n"),
        ]),
        "SharedName",
    );
}

#[test]
#[ignore = "bd-4ktq.36.3 follow-on: Public UDT/Public Enum collision diagnostic"]
fn public_udt_enum_collision_should_be_ambiguous() {
    assert_ambiguous_compile_rejected(
        run_scoping_case(&[
            (
                "Main",
                Procedural,
                "Public result As Variant\nSub Main()\n    Dim value As Payload\n    result = 1\nEnd Sub\n",
            ),
            (
                "Types",
                Procedural,
                "Public Type Payload\n    Value As Long\nEnd Type\n",
            ),
            (
                "Enums",
                Procedural,
                "Public Enum Payload\n    PayloadA = 1\nEnd Enum\n",
            ),
        ]),
        "Payload",
    );
}

#[test]
fn option_private_module_hides_referenced_project_export() {
    let hidden = || {
        option_private_proc_module(
            "HiddenTools",
            "Option Private Module\n\
             Public Function HiddenValue() As Long\n    HiddenValue = 77\nEnd Function\n",
        )
    };
    let lib = project("LibProj", vec![hidden()], vec![]);
    let app = project(
        "AppProj",
        vec![
            proc_module(
                "Main",
                "Public result As Variant\nSub Main()\n    result = HiddenValue()\nEnd Sub\n",
            ),
            proc_module(
                "LocalTools",
                "Public Function LocalValue() As Long\n    LocalValue = 1\nEnd Function\n",
            ),
        ],
        vec![referenced("LibProj", vec![hidden()])],
    );
    assert_compile_rejected(run_scoping_closure(&[lib, app]));
}

#[test]
fn referenced_project_precedence_and_project_qualifier_are_explicit() {
    let lib_a_modules = || {
        vec![proc_module(
            "PickTools",
            "Public Function Pick() As Long\n    Pick = 1\nEnd Function\n",
        )]
    };
    let lib_b_modules = || {
        vec![proc_module(
            "PickTools",
            "Public Function Pick() As Long\n    Pick = 2\nEnd Function\n",
        )]
    };
    let lib_a = project("LibA", lib_a_modules(), vec![]);
    let lib_b = project("LibB", lib_b_modules(), vec![]);
    let app = project(
        "AppProj",
        vec![
            proc_module(
                "Main",
                "Public result As Variant\n\
                 Sub Main()\n\
                 \x20   result = Pick() * 100 + LibB.PickTools.Pick()\n\
                 End Sub\n",
            ),
            proc_module(
                "LocalTools",
                "Public Function LocalValue() As Long\n    LocalValue = 0\nEnd Function\n",
            ),
        ],
        vec![
            referenced("LibA", lib_a_modules()),
            referenced("LibB", lib_b_modules()),
        ],
    );
    assert_project_closure_contains(&[lib_a, lib_b, app], canon(&Variant::from_i32(102)));
}

#[test]
fn referenced_project_withevents_source_routes_to_active_project_handler() {
    let lib = project("LibProj", source_events(), vec![]);
    let app = project(
        "AppProj",
        vec![
            proc_module(
                "Main",
                "Public result As Variant\n\
                 Sub Main()\n\
                 \x20   Dim listener As Listener\n\
                 \x20   Set listener = New Listener\n\
                 \x20   listener.Hook\n\
                 \x20   listener.Fire\n\
                 \x20   result = listener.Fired\n\
                 End Sub\n",
            ),
            class_module(
                "Listener",
                "Private WithEvents src As LibProj.Clock\n\
                 Public Fired As Long\n\
                 Public Sub Hook()\n    Set src = New LibProj.Clock\nEnd Sub\n\
                 Public Sub Fire()\n    src.Fire\nEnd Sub\n\
                 Private Sub src_Tick(ByVal n As Long)\n    Fired = n\nEnd Sub\n",
            ),
        ],
        vec![referenced("LibProj", source_events())],
    );
    assert_project_closure_contains(&[lib, app], canon(&Variant::from_i32(23)));
}
