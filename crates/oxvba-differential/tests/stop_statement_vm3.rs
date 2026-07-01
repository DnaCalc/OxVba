//! vm3 should bind headless `Stop` as a no-op debugger suspend point.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn value(body: &str) -> Canon {
    let source = format!("Public r As Variant\nSub Main()\n{body}End Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"));
    snap.first().cloned().expect("snapshot slot")
}

#[test]
fn stop_statement_does_not_prevent_subsequent_execution() {
    assert_eq!(
        value("    r = 1\n    Stop\n    r = r + 1\n"),
        canon(&Variant::from_i32(2))
    );
}

#[test]
fn stop_statement_does_not_set_err_number() {
    assert_eq!(
        value("    On Error Resume Next\n    Stop\n    r = Err.Number\n"),
        canon(&Variant::from_i32(0))
    );
}
