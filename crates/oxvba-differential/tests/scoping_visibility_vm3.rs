//! Multi-module scoping/visibility fixtures for the `bd-4ktq.9` batch.
//!
//! Live Excel/VBA 7.1 oracle evidence is captured in:
//! `docs/evidence/conformance/vm3_scoping_visibility_oracle_20260701T0945Z/`.
//! The tests below pin legal baseline shapes and oracle-backed resolver
//! diagnostics as each follow-on scoping bead closes.

use std::collections::BTreeMap;

use oxvba_differential::{Canon, Executor, RunOutcome, canon, run_modules, run_project_closure};
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

fn private_class_module(name: &str, src: &str) -> ModuleUnit {
    module(name, Class, src)
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
        conditional_compilation_target: Default::default(),
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

fn assert_compile_rejected_with(outcome: RunOutcome, fragments: &[&str]) {
    assert!(
        outcome.unsupported.is_none(),
        "expected compile/bind diagnostic, got unsupported: {:?}",
        outcome.unsupported
    );
    let err = outcome.result.expect_err("expected compile/bind rejection");
    let err_lower = err.to_ascii_lowercase();
    for fragment in fragments {
        assert!(
            err_lower.contains(&fragment.to_ascii_lowercase()),
            "expected diagnostic to contain `{fragment}`, got {err:?}"
        );
    }
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
fn public_const_variable_collision_keeps_module_qualified_access() {
    assert_snapshot_contains(
        run_scoping_case(&[
            (
                "Main",
                Procedural,
                "Public result As Variant\n\
                 Sub Main()\n\
                 \x20   Beta.SharedName = 2\n\
                 \x20   result = Alpha.SharedName * 10 + Beta.SharedName\n\
                 End Sub\n",
            ),
            ("Alpha", Procedural, "Public Const SharedName As Long = 1\n"),
            ("Beta", Procedural, "Public SharedName As Long\n"),
        ]),
        canon(&Variant::from_i32(12)),
    );
}

#[test]
fn public_const_variable_collision_keeps_project_qualified_access() {
    assert_snapshot_contains(
        run_scoping_case(&[
            (
                "Main",
                Procedural,
                "Public result As Variant\n\
                 Sub Main()\n\
                 \x20   VBAProject.Beta.SharedName = 3\n\
                 \x20   result = VBAProject.Alpha.SharedName * 10 + VBAProject.Beta.SharedName\n\
                 End Sub\n",
            ),
            ("Alpha", Procedural, "Public Const SharedName As Long = 1\n"),
            ("Beta", Procedural, "Public SharedName As Long\n"),
        ]),
        canon(&Variant::from_i32(13)),
    );
}

#[test]
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
fn public_udt_enum_collision_keeps_module_qualified_udt_type() {
    assert_snapshot_contains(
        run_scoping_case(&[
            (
                "Main",
                Procedural,
                "Public result As Variant\n\
                 Sub Main()\n\
                 \x20   Dim value As Types.Payload\n\
                 \x20   value.Value = 7\n\
                 \x20   result = value.Value\n\
                 End Sub\n",
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
        canon(&Variant::from_i32(7)),
    );
}

#[test]
fn public_udt_enum_collision_keeps_project_qualified_udt_type() {
    assert_snapshot_contains(
        run_scoping_case(&[
            (
                "Main",
                Procedural,
                "Public result As Variant\n\
                 Sub Main()\n\
                 \x20   Dim value As VBAProject.Types.Payload\n\
                 \x20   value.Value = 8\n\
                 \x20   result = value.Value\n\
                 End Sub\n",
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
        canon(&Variant::from_i32(8)),
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
fn option_private_module_allows_same_project_access() {
    assert_snapshot_contains(
        run_scoping_case(&[
            (
                "Main",
                Procedural,
                "Public result As Variant\nSub Main()\n    result = HiddenValue()\nEnd Sub\n",
            ),
            (
                "HiddenTools",
                Procedural,
                "Option Private Module\n\
                 Public Function HiddenValue() As Long\n    HiddenValue = 77\nEnd Function\n",
            ),
        ]),
        canon(&Variant::from_i32(77)),
    );
}

#[test]
fn option_private_module_keeps_public_referenced_module_visible() {
    let lib_modules = || {
        vec![
            option_private_proc_module(
                "HiddenTools",
                "Option Private Module\n\
                 Public Function HiddenValue() As Long\n    HiddenValue = 77\nEnd Function\n",
            ),
            proc_module(
                "VisibleTools",
                "Public Function VisibleValue() As Long\n    VisibleValue = 22\nEnd Function\n",
            ),
        ]
    };
    let lib = project("LibProj", lib_modules(), vec![]);
    let app = project(
        "AppProj",
        vec![proc_module(
            "Main",
            "Public result As Variant\n\
             Sub Main()\n\
             \x20   result = VisibleValue() + LibProj.VisibleTools.VisibleValue()\n\
             End Sub\n",
        )],
        vec![referenced("LibProj", lib_modules())],
    );
    assert_project_closure_contains(&[lib, app], canon(&Variant::from_i32(44)));
}

#[test]
fn option_private_module_hides_referenced_project_qualified_export() {
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
        vec![proc_module(
            "Main",
            "Public result As Variant\n\
             Sub Main()\n\
             \x20   result = LibProj.HiddenTools.HiddenValue()\n\
             End Sub\n",
        )],
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
fn active_project_member_shadows_referenced_project_member() {
    let lib_modules = || {
        vec![proc_module(
            "PickTools",
            "Public Function Pick() As Long\n    Pick = 1\nEnd Function\n",
        )]
    };
    let lib = project("LibProj", lib_modules(), vec![]);
    let app = project(
        "AppProj",
        vec![
            proc_module(
                "Main",
                "Public result As Variant\n\
                 Sub Main()\n\
                 \x20   result = Pick() * 10 + LibProj.PickTools.Pick()\n\
                 End Sub\n",
            ),
            proc_module(
                "LocalTools",
                "Public Function Pick() As Long\n    Pick = 9\nEnd Function\n",
            ),
        ],
        vec![referenced("LibProj", lib_modules())],
    );
    assert_project_closure_contains(&[lib, app], canon(&Variant::from_i32(91)));
}

#[test]
fn wrong_referenced_project_qualifier_should_be_rejected() {
    let lib_modules = || {
        vec![proc_module(
            "PickTools",
            "Public Function Pick() As Long\n    Pick = 1\nEnd Function\n",
        )]
    };
    let lib = project("LibProj", lib_modules(), vec![]);
    let app = project(
        "AppProj",
        vec![proc_module(
            "Main",
            "Public result As Variant\n\
             Sub Main()\n\
             \x20   result = MissingProj.PickTools.Pick()\n\
             End Sub\n",
        )],
        vec![referenced("LibProj", lib_modules())],
    );
    assert_compile_rejected(run_scoping_closure(&[lib, app]));
}

#[test]
fn duplicate_referenced_project_global_name_should_be_ambiguous() {
    let lib_modules = || {
        vec![
            proc_module(
                "Alpha",
                "Public Function Clash() As Long\n    Clash = 1\nEnd Function\n",
            ),
            proc_module(
                "Beta",
                "Public Function Clash() As Long\n    Clash = 2\nEnd Function\n",
            ),
        ]
    };
    let lib = project("LibProj", lib_modules(), vec![]);
    let app = project(
        "AppProj",
        vec![proc_module(
            "Main",
            "Public result As Variant\nSub Main()\n    result = Clash()\nEnd Sub\n",
        )],
        vec![referenced("LibProj", lib_modules())],
    );
    assert_ambiguous_compile_rejected(run_scoping_closure(&[lib, app]), "Clash");
}

#[test]
fn duplicate_referenced_project_global_name_blocks_later_reference_fallback() {
    let lib_a_modules = || {
        vec![
            proc_module(
                "Alpha",
                "Public Function Clash() As Long\n    Clash = 1\nEnd Function\n",
            ),
            proc_module(
                "Beta",
                "Public Function Clash() As Long\n    Clash = 2\nEnd Function\n",
            ),
        ]
    };
    let lib_b_modules = || {
        vec![proc_module(
            "Only",
            "Public Function Clash() As Long\n    Clash = 9\nEnd Function\n",
        )]
    };
    let lib_a = project("LibA", lib_a_modules(), vec![]);
    let lib_b = project("LibB", lib_b_modules(), vec![]);
    let app = project(
        "AppProj",
        vec![proc_module(
            "Main",
            "Public result As Variant\nSub Main()\n    result = Clash()\nEnd Sub\n",
        )],
        vec![
            referenced("LibA", lib_a_modules()),
            referenced("LibB", lib_b_modules()),
        ],
    );
    assert_ambiguous_compile_rejected(run_scoping_closure(&[lib_a, lib_b, app]), "Clash");
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

#[test]
fn active_project_withevents_source_routes_to_handler() {
    assert_project_closure_contains(
        &[project(
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
                    "Clock",
                    "Public Event Tick(ByVal n As Long)\n\
                     Public Sub Fire()\n    RaiseEvent Tick(23)\nEnd Sub\n",
                ),
                class_module(
                    "Listener",
                    "Private WithEvents src As Clock\n\
                     Public Fired As Long\n\
                     Public Sub Hook()\n    Set src = New Clock\nEnd Sub\n\
                     Public Sub Fire()\n    src.Fire\nEnd Sub\n\
                     Private Sub src_Tick(ByVal n As Long)\n    Fired = n\nEnd Sub\n",
                ),
            ],
            vec![],
        )],
        canon(&Variant::from_i32(23)),
    );
}

#[test]
fn withevents_handler_prefix_mismatch_does_not_route() {
    assert_project_closure_contains(
        &[project(
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
                    "Clock",
                    "Public Event Tick(ByVal n As Long)\n\
                     Public Sub Fire()\n    RaiseEvent Tick(23)\nEnd Sub\n",
                ),
                class_module(
                    "Listener",
                    "Private WithEvents src As Clock\n\
                     Public Fired As Long\n\
                     Public Sub Hook()\n    Fired = 9\n    Set src = New Clock\nEnd Sub\n\
                     Public Sub Fire()\n    src.Fire\nEnd Sub\n\
                     Private Sub wrong_Tick(ByVal n As Long)\n    Fired = n\nEnd Sub\n",
                ),
            ],
            vec![],
        )],
        canon(&Variant::from_i32(9)),
    );
}

#[test]
fn withevents_in_procedural_module_should_be_rejected() {
    assert_compile_rejected_with(
        run_scoping_case(&[
            (
                "Main",
                Procedural,
                "Private WithEvents src As Clock\n\
                 Public result As Variant\n\
                 Sub Main()\n\
                 \x20   result = 0\n\
                 End Sub\n",
            ),
            (
                "Clock",
                Class,
                "Public Event Tick(ByVal n As Long)\n\
                 Public Sub Fire()\n    RaiseEvent Tick(23)\nEnd Sub\n",
            ),
        ]),
        &["withevents"],
    );
}

#[test]
fn private_referenced_project_withevents_source_should_be_rejected() {
    let lib_modules = || {
        vec![private_class_module(
            "Clock",
            "Public Event Tick(ByVal n As Long)\n\
             Public Sub Fire()\n    RaiseEvent Tick(23)\nEnd Sub\n",
        )]
    };
    let lib = project("LibProj", lib_modules(), vec![]);
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
        vec![referenced("LibProj", lib_modules())],
    );
    assert_compile_rejected_with(run_scoping_closure(&[lib, app]), &["libproj", "clock"]);
}

#[test]
fn private_referenced_project_withevents_declaration_should_be_rejected() {
    let lib_modules = || {
        vec![private_class_module(
            "Clock",
            "Public Event Tick(ByVal n As Long)\n",
        )]
    };
    let lib = project("LibProj", lib_modules(), vec![]);
    let app = project(
        "AppProj",
        vec![
            proc_module(
                "Main",
                "Public result As Variant\n\
                 Sub Main()\n\
                 \x20   Dim listener As Listener\n\
                 \x20   Set listener = New Listener\n\
                 \x20   result = 1\n\
                 End Sub\n",
            ),
            class_module("Listener", "Private WithEvents src As LibProj.Clock\n"),
        ],
        vec![referenced("LibProj", lib_modules())],
    );
    assert_compile_rejected_with(run_scoping_closure(&[lib, app]), &["libproj", "clock"]);
}
