//! vm3 should accept negative constant lower bounds on array declarations.

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

fn assert_rejected(body: &str) {
    let source = format!("Public r As Variant\nSub Main()\n{body}End Sub\n");
    let outcome = run(Executor::Vm3, &source);
    let err = outcome
        .result
        .err()
        .expect("expected nonconstant lower bound to remain rejected");
    assert!(
        err.contains("ReDim bound must be a constant"),
        "expected nonconstant lower-bound diagnostic, got {err:?}"
    );
}

#[test]
fn redim_accepts_negative_constant_lower_bound() {
    assert_eq!(
        value(
            "    Dim a() As Long\n    ReDim a(-2 To 2)\n    a(-2) = 10\n    a(2) = 5\n    r = CStr(LBound(a)) & \":\" & CStr(UBound(a)) & \":\" & CStr(a(-2) + a(2))\n"
        ),
        s("-2:2:15")
    );
}

#[test]
fn fixed_array_dim_accepts_negative_constant_lower_bound() {
    assert_eq!(
        value(
            "    Dim a(-1 To 1) As Long\n    a(-1) = 3\n    a(1) = 4\n    r = CStr(LBound(a)) & \":\" & CStr(UBound(a)) & \":\" & CStr(a(-1) + a(1))\n"
        ),
        s("-1:1:7")
    );
}

#[test]
fn redim_nonconstant_lower_bound_remains_separate_gap() {
    assert_rejected(
        "    Dim a() As Long\n    Dim n As Long\n    n = -2\n    ReDim a(n To 2)\n    r = LBound(a)\n",
    );
}
