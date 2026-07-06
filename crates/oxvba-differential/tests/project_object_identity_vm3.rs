//! VM3 project-object identity regressions.

use oxvba_differential::{Canon, Executor, RunOutcome, run_modules};
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};

fn assert_contains_string(outcome: RunOutcome, expected: &str) {
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined project-object identity case as unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome
            .handle_balance
            .is_some_and(|balance| balance.is_zero()),
        "vm3 project-object identity case leaked runtime handles: {:?}",
        outcome.handle_balance
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 project-object identity case failed: {err}"));
    let expected = Canon::Str(expected.to_string());
    assert!(
        values.contains(&expected),
        "snapshot {values:?} did not contain {expected:?}"
    );
}

#[test]
fn project_class_is_compares_object_identity_for_non_null_instances() {
    let main = "Public result As Variant\n\
                Sub Main()\n\
                \x20   Dim a As Widget\n\
                \x20   Dim b As Widget\n\
                \x20   Dim c As Widget\n\
                \x20   Set a = New Widget\n\
                \x20   Set b = a\n\
                \x20   Set c = New Widget\n\
                \x20   result = CStr(a Is b) & \"|\" & CStr(a Is c) & \"|\" & CStr(a Is Nothing) & \"|\" & CStr(c Is Nothing)\n\
                \x20   Set b = Nothing\n\
                \x20   Set a = Nothing\n\
                \x20   Set c = Nothing\n\
                End Sub\n";
    let widget = "Private n As Long\n\
                  Private Sub Class_Initialize()\n\
                  \x20   n = 1\n\
                  End Sub\n\
                  Public Property Get Value() As Long\n\
                  \x20   Value = n\n\
                  End Property\n";

    assert_contains_string(
        run_modules(
            Executor::Vm3,
            &[("Main", Procedural, main), ("Widget", Class, widget)],
            "VBAProject",
        ),
        "True|False|False|False",
    );
}
