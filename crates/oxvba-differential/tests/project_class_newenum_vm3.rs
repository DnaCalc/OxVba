//! vm3 project-class `For Each` via `VB_UserMemId = -4` (`bd-4ktq.49`).

use oxvba_differential::{Executor, RunOutcome, canon, run_modules};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};

fn run_case(main: &str, classes: &[(&str, &str)]) -> RunOutcome {
    let mut modules = vec![("Main", Procedural, main)];
    modules.extend(classes.iter().map(|(name, source)| (*name, Class, *source)));
    run_modules(Executor::Vm3, &modules, "VBAProject")
}

fn assert_global_long(outcome: RunOutcome, expected: i32) {
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined project-class NewEnum case as unsupported: {:?}",
        outcome.unsupported
    );
    let snapshot = outcome.result.unwrap_or_else(|err| {
        panic!(
            "vm3 project-class NewEnum case completed: {err}; err={:?}",
            outcome.err
        )
    });
    assert!(
        snapshot.contains(&canon(&Variant::from_i32(expected))),
        "expected snapshot to contain {expected}, got {snapshot:?}"
    );
}

fn assert_error_number(outcome: RunOutcome, expected: i32) {
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined project-class NewEnum error case as unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.raised,
        "expected VBA error {expected}, got {:?}",
        outcome.result
    );
    assert_eq!(outcome.err.number, expected);
}

#[test]
fn project_class_newenum_attribute_foreach_sums_collection_values() {
    let main = "Public r As Long\n\
        Sub Main()\n\
            Dim widget As New Widget\n\
            Dim item As Variant\n\
            For Each item In widget\n\
                r = r + item\n\
            Next item\n\
        End Sub\n";
    let widget = "Private items As VBA.Collection\n\n\
        Private Sub Class_Initialize()\n\
            Set items = New VBA.Collection\n\
            items.Add 41\n\
            items.Add 42\n\
        End Sub\n\n\
        Public Property Get NewEnum() As IUnknown\n\
            Set NewEnum = items.[_NewEnum]\n\
        End Property\n\
        Attribute NewEnum.VB_UserMemId = -4\n\
        Attribute NewEnum.VB_MemberFlags = \"40\"\n";
    assert_global_long(run_case(main, &[("Widget", widget)]), 83);
}

#[test]
fn project_class_without_newenum_foreach_raises_438() {
    let main = "Public r As Long\n\
        Sub Main()\n\
            Dim widget As New Widget\n\
            Dim item As Variant\n\
            For Each item In widget\n\
                r = r + 1\n\
            Next item\n\
        End Sub\n";
    let widget = "Public Value As Long\n";
    assert_error_number(run_case(main, &[("Widget", widget)]), 438);
}
