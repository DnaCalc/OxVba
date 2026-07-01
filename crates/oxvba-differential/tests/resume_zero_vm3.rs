//! vm3 should treat `Resume 0` as bare `Resume`.

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

fn s(text: &str) -> Canon {
    canon(&Variant::from_string(text.to_string()))
}

#[test]
fn resume_zero_reenters_faulting_statement() {
    assert_eq!(
        value(
            "    On Error GoTo H\n    Dim k As Long\n    Dim x As Long\n    x = 1 / k\n    r = CStr(x) & \":\" & CStr(Err.Number)\n    Exit Sub\nH:\n    k = 1\n    Resume 0\n"
        ),
        s("1:0")
    );
}

#[test]
fn bare_resume_control_reenters_faulting_statement() {
    assert_eq!(
        value(
            "    On Error GoTo H\n    Dim k As Long\n    Dim x As Long\n    x = 1 / k\n    r = CStr(x) & \":\" & CStr(Err.Number)\n    Exit Sub\nH:\n    k = 1\n    Resume\n"
        ),
        s("1:0")
    );
}
