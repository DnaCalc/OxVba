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
