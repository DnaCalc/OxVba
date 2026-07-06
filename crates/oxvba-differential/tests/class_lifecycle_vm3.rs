//! VM3 project-class lifecycle cleanup regressions.

use oxvba_differential::{Canon, Executor, RunOutcome, run_modules};
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};

fn assert_contains_string(outcome: RunOutcome, expected: &str) {
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined class lifecycle case as unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome
            .handle_balance
            .is_some_and(|balance| balance.is_zero()),
        "vm3 class lifecycle case leaked runtime handles: {:?}",
        outcome.handle_balance
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 class lifecycle case failed: {err}"));
    let expected = Canon::Str(expected.to_string());
    assert!(
        values.contains(&expected),
        "snapshot {values:?} did not contain {expected:?}"
    );
}

#[test]
fn ordinary_new_initialize_failure_terminates_partial_instance_and_allows_retry() {
    let main = "Public result As Variant\n\
                Public Log As String\n\
                Public FailOnce As Boolean\n\
                Sub Main()\n\
                \x20   FailOnce = True\n\
                \x20   On Error Resume Next\n\
                \x20   Dim w As Widget\n\
                \x20   Set w = New Widget\n\
                \x20   Dim firstErr As Long\n\
                \x20   firstErr = Err.Number\n\
                \x20   Err.Clear\n\
                \x20   Dim afterFail As String\n\
                \x20   afterFail = Log\n\
                \x20   Set w = New Widget\n\
                \x20   Dim retryTotal As Long\n\
                \x20   retryTotal = w.Total\n\
                \x20   Set w = Nothing\n\
                \x20   result = CStr(retryTotal) & \"|\" & afterFail & \"|\" & Log & \"|\" & CStr(firstErr)\n\
                End Sub\n";
    let widget = "Private n As Long\n\
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
                  \x20   Err.Raise 77\n\
                  End Sub\n\
                  Public Property Get Total() As Long\n\
                  \x20   Total = n\n\
                  End Property\n";

    assert_contains_string(
        run_modules(
            Executor::Vm3,
            &[("Main", Procedural, main), ("Widget", Class, widget)],
            "VBAProject",
        ),
        "10|I;T;|I;T;I;T;|5",
    );
}

#[test]
fn local_object_terminates_at_procedure_exit_without_explicit_nothing() {
    let main = "Public result As Variant\n\
                Public Log As String\n\
                Sub Main()\n\
                \x20   MakeWidget\n\
                \x20   result = Log\n\
                End Sub\n\
                Sub MakeWidget()\n\
                \x20   Dim w As Widget\n\
                \x20   Set w = New Widget\n\
                End Sub\n";
    let widget = "Private Sub Class_Initialize()\n\
                  \x20   Main.Log = Main.Log & \"I;\"\n\
                  End Sub\n\
                  Private Sub Class_Terminate()\n\
                  \x20   Main.Log = Main.Log & \"T;\"\n\
                  \x20   Err.Raise 77\n\
                  End Sub\n";

    assert_contains_string(
        run_modules(
            Executor::Vm3,
            &[("Main", Procedural, main), ("Widget", Class, widget)],
            "VBAProject",
        ),
        "I;T;",
    );
}

#[test]
fn local_object_terminates_during_fault_unwind_before_caller_resume_next() {
    let main = "Public result As Variant\n\
                Public Log As String\n\
                Sub Main()\n\
                \x20   On Error Resume Next\n\
                \x20   FailWithWidget\n\
                \x20   Dim seenErr As Long\n\
                \x20   seenErr = Err.Number\n\
                \x20   result = Log & \"|\" & CStr(seenErr)\n\
                End Sub\n\
                Sub FailWithWidget()\n\
                \x20   Dim w As Widget\n\
                \x20   Set w = New Widget\n\
                \x20   Err.Raise 5\n\
                End Sub\n";
    let widget = "Private Sub Class_Initialize()\n\
                  \x20   Main.Log = Main.Log & \"I;\"\n\
                  End Sub\n\
                  Private Sub Class_Terminate()\n\
                  \x20   Main.Log = Main.Log & \"T;\"\n\
                  \x20   Err.Raise 77\n\
                  End Sub\n";

    assert_contains_string(
        run_modules(
            Executor::Vm3,
            &[("Main", Procedural, main), ("Widget", Class, widget)],
            "VBAProject",
        ),
        "I;T;|5",
    );
}

#[test]
fn class_terminate_field_release_cascades_to_child_termination() {
    let main = "Public result As Variant\n\
                Public Log As String\n\
                Sub Main()\n\
                \x20   Dim o As Owner\n\
                \x20   Set o = New Owner\n\
                \x20   Set o = Nothing\n\
                \x20   Dim afterDrop As String\n\
                \x20   afterDrop = Log\n\
                \x20   result = afterDrop\n\
                End Sub\n";
    let owner = "Private child As Child\n\
                 Private Sub Class_Initialize()\n\
                 \x20   Main.Log = Main.Log & \"OI;\"\n\
                 \x20   Set child = New Child\n\
                 End Sub\n\
                 Private Sub Class_Terminate()\n\
                 \x20   Main.Log = Main.Log & \"OT;\"\n\
                 End Sub\n";
    let child = "Private Sub Class_Initialize()\n\
                 \x20   Main.Log = Main.Log & \"CI;\"\n\
                 End Sub\n\
                 Private Sub Class_Terminate()\n\
                 \x20   Main.Log = Main.Log & \"CT;\"\n\
                 \x20   Err.Raise 88\n\
                 End Sub\n";

    assert_contains_string(
        run_modules(
            Executor::Vm3,
            &[
                ("Main", Procedural, main),
                ("Owner", Class, owner),
                ("Child", Class, child),
            ],
            "VBAProject",
        ),
        "OI;CI;OT;CT;",
    );
}

#[test]
fn class_terminate_can_resurrect_me_without_double_terminating() {
    let main = "Public result As Variant\n\
                Public Log As String\n\
                Public Held As Widget\n\
                Sub Main()\n\
                \x20   Dim w As Widget\n\
                \x20   Set w = New Widget\n\
                \x20   w.Bump\n\
                \x20   Set w = Nothing\n\
                \x20   Dim afterDrop As String\n\
                \x20   afterDrop = Log\n\
                \x20   Dim heldTotal As Long\n\
                \x20   heldTotal = Held.Total\n\
                \x20   Set Held = Nothing\n\
                \x20   result = CStr(heldTotal) & \"|\" & afterDrop & \"|\" & Log\n\
                End Sub\n";
    let widget = "Private n As Long\n\
                  Private Sub Class_Initialize()\n\
                  \x20   n = 10\n\
                  \x20   Main.Log = Main.Log & \"I;\"\n\
                  End Sub\n\
                  Private Sub Class_Terminate()\n\
                  \x20   Main.Log = Main.Log & \"T\" & CStr(n) & \";\"\n\
                  \x20   Set Main.Held = Me\n\
                  End Sub\n\
                  Public Sub Bump()\n\
                  \x20   n = n + 1\n\
                  End Sub\n\
                  Public Property Get Total() As Long\n\
                  \x20   Total = n\n\
                  End Property\n";

    assert_contains_string(
        run_modules(
            Executor::Vm3,
            &[("Main", Procedural, main), ("Widget", Class, widget)],
            "VBAProject",
        ),
        "11|I;T11;|I;T11;",
    );
}
