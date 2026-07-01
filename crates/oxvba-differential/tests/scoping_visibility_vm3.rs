//! Multi-module scoping/visibility fixtures for the `bd-4ktq.9` batch.
//!
//! Live Excel/VBA 7.1 oracle evidence is captured in:
//! `docs/evidence/conformance/vm3_scoping_visibility_oracle_20260701T0945Z/`.
//! The passing tests below pin legal baseline shapes. The ignored tests encode the
//! oracle-backed gaps that the follow-on scoping beads are expected to unignore
//! and satisfy as each resolver diagnostic is implemented.

use oxvba_differential::{canon, run_modules, Canon, Executor, RunOutcome};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};

fn run_scoping_case(modules: &[(&str, oxvba_symbol::manifest::ModuleKind, &str)]) -> RunOutcome {
    run_modules(Executor::Vm3, modules, "VBAProject")
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

fn assert_compile_rejected(outcome: RunOutcome) {
    assert!(
        outcome.unsupported.is_some() || outcome.result.is_err() || outcome.raised,
        "expected compile/bind rejection or failure, got {outcome:?}"
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
#[ignore = "bd-4ktq.9.5: valid Project.Module.Member qualifier is not accepted by vm3 today"]
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
#[ignore = "bd-4ktq.9.3: duplicate Public unqualified lookup still picks a candidate today"]
fn duplicate_public_unqualified_should_be_ambiguous() {
    assert_compile_rejected(run_scoping_case(&[
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
    ]));
}

#[test]
#[ignore = "bd-4ktq.9.4: module/member name collision still picks a candidate today"]
fn module_name_public_member_collision_should_be_rejected() {
    assert_compile_rejected(run_scoping_case(&[
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
#[ignore = "bd-4ktq.9.5: wrong Project.Module.Member qualifier is still ignored today"]
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
#[ignore = "bd-4ktq.9.6: Friend on standard modules is still accepted today"]
fn friend_on_standard_module_should_be_rejected() {
    assert_compile_rejected(run_scoping_case(&[(
        "Main",
        Procedural,
        "Friend Sub Helper()\nEnd Sub\n\nPublic result As Variant\nSub Main()\n    Helper\n    result = 1\nEnd Sub\n",
    )]));
}
