//! vm3 `Dim As New` lifecycle/resurrection regressions (`bd-4ktq.56`).

use oxvba_differential::{Executor, RunOutcome, canon, run_modules};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};

fn run_case(main_body: &str) -> RunOutcome {
    let main = format!("Public Log As String\nPublic Result As String\n{main_body}");
    let counter = "Private n As Long\n\n\
        Private Sub Class_Initialize()\n    n = 10\n    Main.Log = Main.Log & \"I;\"\nEnd Sub\n\n\
        Private Sub Class_Terminate()\n    Main.Log = Main.Log & \"T\" & CStr(n) & \";\"\nEnd Sub\n\n\
        Public Sub Bump()\n    n = n + 1\nEnd Sub\n\n\
        Public Property Get Total() As Long\n    Total = n\nEnd Property\n";
    run_modules(
        Executor::Vm3,
        &[("Main", Procedural, &main), ("Counter", Class, counter)],
        "VBAProject",
    )
}

fn assert_result(main_body: &str, expected: &str) {
    let outcome = run_case(main_body);
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined Dim As New case as unsupported: {:?}",
        outcome.unsupported
    );
    let snapshot = outcome.result.unwrap_or_else(|err| {
        panic!(
            "vm3 Dim As New case completed: {err}; err={:?}",
            outcome.err
        )
    });
    let want = canon(&Variant::from_string(expected.to_string()));
    assert!(
        snapshot.contains(&want),
        "expected Result={expected:?}, got {snapshot:?}"
    );
}

#[test]
fn local_dim_as_new_declaration_is_lazy() {
    assert_result(
        "Sub Main()\n    Dim c As New Counter\n    Result = CStr(Len(Log)) & \"|\" & Log\nEnd Sub\n",
        "0|",
    );
}

#[test]
fn local_dim_as_new_first_member_access_instantiates() {
    assert_result(
        "Sub Main()\n    Dim c As New Counter\n    c.Bump\n    Result = CStr(c.Total) & \"|\" & Log\nEnd Sub\n",
        "11|I;",
    );
}

#[test]
fn local_dim_as_new_is_nothing_instantiates_and_returns_false() {
    assert_result(
        "Sub Main()\n    Dim c As New Counter\n    Result = CStr(c Is Nothing) & \"|\" & Log\nEnd Sub\n",
        "False|I;",
    );
}

#[test]
fn local_dim_as_new_set_nothing_before_access_does_not_instantiate() {
    assert_result(
        "Sub Main()\n    Dim c As New Counter\n    Set c = Nothing\n    Result = CStr(Len(Log)) & \"|\" & Log\nEnd Sub\n",
        "0|",
    );
}

#[test]
fn local_dim_as_new_set_nothing_resurrects_on_next_access() {
    assert_result(
        "Sub Main()\n    Dim c As New Counter\n    c.Bump\n    Set c = Nothing\n    Result = CStr(c.Total) & \"|\" & Log\nEnd Sub\n",
        "10|I;T11;I;",
    );
}

#[test]
fn module_dim_as_new_declaration_is_lazy() {
    assert_result(
        "Private g As New Counter\n\nSub Main()\n    Result = CStr(Len(Log)) & \"|\" & Log\nEnd Sub\n",
        "0|",
    );
}

#[test]
fn module_dim_as_new_set_nothing_resurrects_on_next_access() {
    assert_result(
        "Private g As New Counter\n\nSub Main()\n    g.Bump\n    Set g = Nothing\n    Result = CStr(g.Total) & \"|\" & Log\nEnd Sub\n",
        "10|I;T11;I;",
    );
}
