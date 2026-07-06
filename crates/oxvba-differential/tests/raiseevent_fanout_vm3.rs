//! vm3 `RaiseEvent` / `WithEvents` fan-out ordering parity.
//!
//! Live Excel/VBA 7.1 oracle evidence:
//! `docs/evidence/conformance/vm3_raiseevent_fanout_oracle_20260702T043855Z/`.

use oxvba_differential::{Canon, Executor, canon, run_modules};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};

fn s(text: &str) -> Canon {
    canon(&Variant::from_string(text.to_string()))
}

fn assert_result(modules: &[(&str, oxvba_symbol::manifest::ModuleKind, &str)], expected: &str) {
    let outcome = run_modules(Executor::Vm3, modules, "VBAProject");
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined RaiseEvent fan-out case as unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome
            .handle_balance
            .is_some_and(|balance| balance.is_zero()),
        "vm3 RaiseEvent fan-out case leaked runtime handles: {:?}",
        outcome.handle_balance
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 RaiseEvent fan-out case failed: {err}"));
    let expected = s(expected);
    assert!(
        values.contains(&expected),
        "snapshot {values:?} did not contain {expected:?}"
    );
}

const SOURCE: &str = r#"
Public Event Poked(ByRef n As Long)

Public Function FireWith(ByVal start As Long) As Long
    Dim n As Long
    n = start
    RaiseEvent Poked(n)
    FireWith = n
End Function
"#;

const SINGLE_SINK: &str = r#"
Private WithEvents src As Source
Public Name As String
Public Digit As Long

Public Sub Wire(ByVal value As Source)
    Set src = value
End Sub

Public Sub Clear()
    Set src = Nothing
End Sub

Private Sub src_Poked(ByRef n As Long)
    Trace = Trace & Name & CStr(n) & ";"
    n = n * 10 + Digit
End Sub
"#;

const DOUBLE_SINK: &str = r#"
Private WithEvents first As Source
Private WithEvents second As Source
Public Trace As String

Public Sub WireFirst(ByVal value As Source)
    Set first = value
End Sub

Public Sub WireSecond(ByVal value As Source)
    Set second = value
End Sub

Private Sub first_Poked(ByRef n As Long)
    Trace = Trace & "first" & CStr(n) & ";"
    n = n * 10 + 1
End Sub

Private Sub second_Poked(ByRef n As Long)
    Trace = Trace & "second" & CStr(n) & ";"
    n = n * 10 + 2
End Sub
"#;

const SOURCE_SWITCH_SINK: &str = r#"
Private WithEvents src As Source
Public Trace As String

Public Sub Wire(ByVal value As Source)
    Set src = value
End Sub

Private Sub src_Poked(ByRef n As Long)
    Trace = Trace & CStr(n) & ";"
    n = n * 10 + 7
End Sub
"#;

const TERMINATING_SINK: &str = r#"
Private WithEvents src As Source

Public Sub Wire(ByVal value As Source)
    Set src = value
End Sub

Private Sub Class_Terminate()
    Main.Trace = Main.Trace & "T;"
End Sub

Private Sub src_Poked(ByRef n As Long)
    Main.Trace = Main.Trace & "H" & CStr(n) & ";"
    n = n * 10 + 3
End Sub
"#;

#[test]
fn same_sink_two_fields_dispatch_in_subscription_order() {
    assert_result(
        &[
            (
                "Main",
                Procedural,
                r#"
Public result As String
Sub Main()
    Dim s As Source
    Dim k As DoubleSink
    Dim finalValue As Long
    Set s = New Source
    Set k = New DoubleSink
    k.WireSecond s
    k.WireFirst s
    finalValue = s.FireWith(1)
    result = k.Trace & "|" & CStr(finalValue)
End Sub
"#,
            ),
            ("Source", Class, SOURCE),
            ("DoubleSink", Class, DOUBLE_SINK),
        ],
        "second1;first12;|121",
    );
}

#[test]
fn two_sink_instances_dispatch_in_subscription_order_not_owner_identity_order() {
    assert_result(
        &[
            (
                "Main",
                Procedural,
                r#"
Public Trace As String
Public result As String
Sub Main()
    Dim s As Source
    Dim a As SingleSink
    Dim b As SingleSink
    Dim finalValue As Long
    Set s = New Source
    Set a = New SingleSink
    Set b = New SingleSink
    a.Name = "A": a.Digit = 1
    b.Name = "B": b.Digit = 2
    b.Wire s
    a.Wire s
    finalValue = s.FireWith(1)
    result = Trace & "|" & CStr(finalValue)
End Sub
"#,
            ),
            ("Source", Class, SOURCE),
            ("SingleSink", Class, SINGLE_SINK),
        ],
        "B1;A12;|121",
    );
}

#[test]
fn owner_creation_order_does_not_override_subscription_order() {
    assert_result(
        &[
            (
                "Main",
                Procedural,
                r#"
Public Trace As String
Public result As String
Sub Main()
    Dim s As Source
    Dim a As SingleSink
    Dim b As SingleSink
    Dim finalValue As Long
    Set s = New Source
    Set b = New SingleSink
    Set a = New SingleSink
    a.Name = "A": a.Digit = 1
    b.Name = "B": b.Digit = 2
    a.Wire s
    b.Wire s
    finalValue = s.FireWith(1)
    result = Trace & "|" & CStr(finalValue)
End Sub
"#,
            ),
            ("Source", Class, SOURCE),
            ("SingleSink", Class, SINGLE_SINK),
        ],
        "A1;B11;|112",
    );
}

#[test]
fn reassigning_existing_withevents_field_moves_subscription_to_end() {
    assert_result(
        &[
            (
                "Main",
                Procedural,
                r#"
Public Trace As String
Public result As String
Sub Main()
    Dim s As Source
    Dim a As SingleSink
    Dim b As SingleSink
    Dim finalValue As Long
    Set s = New Source
    Set a = New SingleSink
    Set b = New SingleSink
    a.Name = "A": a.Digit = 1
    b.Name = "B": b.Digit = 2
    a.Wire s
    b.Wire s
    a.Wire s
    finalValue = s.FireWith(1)
    result = Trace & "|" & CStr(finalValue)
End Sub
"#,
            ),
            ("Source", Class, SOURCE),
            ("SingleSink", Class, SINGLE_SINK),
        ],
        "B1;A12;|121",
    );
}

#[test]
fn clearing_and_rewiring_withevents_field_moves_subscription_to_end() {
    assert_result(
        &[
            (
                "Main",
                Procedural,
                r#"
Public Trace As String
Public result As String
Sub Main()
    Dim s As Source
    Dim a As SingleSink
    Dim b As SingleSink
    Dim finalValue As Long
    Set s = New Source
    Set a = New SingleSink
    Set b = New SingleSink
    a.Name = "A": a.Digit = 1
    b.Name = "B": b.Digit = 2
    a.Wire s
    b.Wire s
    a.Clear
    a.Wire s
    finalValue = s.FireWith(1)
    result = Trace & "|" & CStr(finalValue)
End Sub
"#,
            ),
            ("Source", Class, SOURCE),
            ("SingleSink", Class, SINGLE_SINK),
        ],
        "B1;A12;|121",
    );
}

#[test]
fn reassigned_withevents_field_detaches_old_source() {
    assert_result(
        &[
            (
                "Main",
                Procedural,
                r#"
Public result As String
Sub Main()
    Dim s1 As Source
    Dim s2 As Source
    Dim k As SourceSwitchSink
    Dim firstFinal As Long
    Dim secondFinal As Long
    Set s1 = New Source
    Set s2 = New Source
    Set k = New SourceSwitchSink
    k.Wire s1
    k.Wire s2
    firstFinal = s1.FireWith(1)
    secondFinal = s2.FireWith(1)
    result = k.Trace & "|" & CStr(firstFinal) & "|" & CStr(secondFinal)
End Sub
"#,
            ),
            ("Source", Class, SOURCE),
            ("SourceSwitchSink", Class, SOURCE_SWITCH_SINK),
        ],
        "1;|1|17",
    );
}

#[test]
fn terminated_withevents_owner_no_longer_receives_project_events() {
    assert_result(
        &[
            (
                "Main",
                Procedural,
                r#"
Public Trace As String
Public result As String
Sub Main()
    Dim s As Source
    Dim k As TerminatingSink
    Dim firstFinal As Long
    Dim secondFinal As Long
    Set s = New Source
    Set k = New TerminatingSink
    k.Wire s
    firstFinal = s.FireWith(1)
    Set k = Nothing
    secondFinal = s.FireWith(2)
    result = Trace & "|" & CStr(firstFinal) & "|" & CStr(secondFinal)
End Sub
"#,
            ),
            ("Source", Class, SOURCE),
            ("TerminatingSink", Class, TERMINATING_SINK),
        ],
        "H1;T;|13|2",
    );
}

#[test]
fn scoped_withevents_owner_terminates_and_unsubscribes_before_caller_continues() {
    assert_result(
        &[
            (
                "Main",
                Procedural,
                r#"
Public Trace As String
Public result As String

Sub WireAndDrop(ByVal s As Source)
    Dim k As TerminatingSink
    Dim firstFinal As Long
    Set k = New TerminatingSink
    k.Wire s
    firstFinal = s.FireWith(1)
    Trace = Trace & "F" & CStr(firstFinal) & ";"
End Sub

Sub Main()
    Dim s As Source
    Dim secondFinal As Long
    Set s = New Source
    WireAndDrop s
    secondFinal = s.FireWith(2)
    result = Trace & "|" & CStr(secondFinal)
End Sub
"#,
            ),
            ("Source", Class, SOURCE),
            ("TerminatingSink", Class, TERMINATING_SINK),
        ],
        "H1;F13;T;|2",
    );
}
