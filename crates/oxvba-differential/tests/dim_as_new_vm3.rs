//! vm3 `Dim As New` lifecycle/resurrection regressions (`bd-4ktq.56`).

use oxvba_differential::{Executor, RunOutcome, canon, run_modules};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};

const COUNTER_CLASS: &str = "Private n As Long\n\n\
    Private Sub Class_Initialize()\n    n = 10\n    Main.Log = Main.Log & \"I;\"\nEnd Sub\n\n\
    Private Sub Class_Terminate()\n    Main.Log = Main.Log & \"T\" & CStr(n) & \";\"\nEnd Sub\n\n\
    Public Sub Bump()\n    n = n + 1\nEnd Sub\n\n\
    Public Property Get Total() As Long\n    Total = n\nEnd Property\n";

const HOST_CLASS: &str = "Private child As New Counter\n\n\
    Public Function FieldDimOnly() As String\n    FieldDimOnly = CStr(Len(Main.Log)) & \"|\" & Main.Log\nEnd Function\n\n\
    Public Function FieldFirstMember() As String\n    child.Bump\n    FieldFirstMember = CStr(child.Total) & \"|\" & Main.Log\nEnd Function\n\n\
    Public Function FieldIsNothing() As String\n    FieldIsNothing = CStr(child Is Nothing) & \"|\" & Main.Log\nEnd Function\n\n\
    Public Function FieldSetNothingBeforeAccess() As String\n    Set child = Nothing\n    FieldSetNothingBeforeAccess = CStr(Len(Main.Log)) & \"|\" & Main.Log\nEnd Function\n\n\
    Public Function FieldSetNothingResurrect() As String\n    child.Bump\n    Set child = Nothing\n    FieldSetNothingResurrect = CStr(child.Total) & \"|\" & Main.Log\nEnd Function\n\n\
    Public Function FieldBumpTotal() As Long\n    child.Bump\n    FieldBumpTotal = child.Total\nEnd Function\n";

fn run_case(main_body: &str) -> RunOutcome {
    let main = format!("Public Log As String\nPublic Result As String\n{main_body}");
    run_modules(
        Executor::Vm3,
        &[
            ("Main", Procedural, &main),
            ("Counter", Class, COUNTER_CLASS),
        ],
        "VBAProject",
    )
}

fn run_case_with_host(main_body: &str) -> RunOutcome {
    let main = format!("Public Log As String\nPublic Result As String\n{main_body}");
    run_modules(
        Executor::Vm3,
        &[
            ("Main", Procedural, &main),
            ("Counter", Class, COUNTER_CLASS),
            ("Host", Class, HOST_CLASS),
        ],
        "VBAProject",
    )
}

fn assert_outcome(outcome: RunOutcome, expected: &str) {
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

fn assert_result(main_body: &str, expected: &str) {
    assert_outcome(run_case(main_body), expected);
}

fn assert_host_result(main_body: &str, expected: &str) {
    assert_outcome(run_case_with_host(main_body), expected);
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

#[test]
fn class_field_dim_as_new_declaration_is_lazy() {
    assert_host_result(
        "Sub Main()\n    Dim h As New Host\n    Result = h.FieldDimOnly()\nEnd Sub\n",
        "0|",
    );
}

#[test]
fn class_field_dim_as_new_first_member_access_instantiates() {
    assert_host_result(
        "Sub Main()\n    Dim h As New Host\n    Result = h.FieldFirstMember()\nEnd Sub\n",
        "11|I;",
    );
}

#[test]
fn class_field_dim_as_new_is_nothing_instantiates_and_returns_false() {
    assert_host_result(
        "Sub Main()\n    Dim h As New Host\n    Result = h.FieldIsNothing()\nEnd Sub\n",
        "False|I;",
    );
}

#[test]
fn class_field_dim_as_new_set_nothing_before_access_does_not_instantiate() {
    assert_host_result(
        "Sub Main()\n    Dim h As New Host\n    Result = h.FieldSetNothingBeforeAccess()\nEnd Sub\n",
        "0|",
    );
}

#[test]
fn class_field_dim_as_new_set_nothing_resurrects_on_next_access() {
    assert_host_result(
        "Sub Main()\n    Dim h As New Host\n    Result = h.FieldSetNothingResurrect()\nEnd Sub\n",
        "10|I;T11;I;",
    );
}

#[test]
fn class_field_dim_as_new_slots_are_per_host_instance() {
    assert_host_result(
        "Sub Main()\n    Dim a As New Host\n    Dim b As New Host\n    Result = CStr(a.FieldBumpTotal()) & \"/\" & CStr(b.FieldBumpTotal()) & \"/\" & CStr(a.FieldBumpTotal()) & \"|\" & Log\nEnd Sub\n",
        "11/11/12|I;I;",
    );
}
