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
fn redim_accepts_runtime_lower_bound_expression() {
    assert_eq!(
        value(
            "    Dim a() As Long\n    Dim n As Long\n    n = -2\n    ReDim a(n To n + 4)\n    a(-2) = 11\n    a(2) = 6\n    r = CStr(LBound(a)) & \":\" & CStr(UBound(a)) & \":\" & CStr(a(-2) + a(2))\n"
        ),
        s("-2:2:17")
    );
}

#[test]
fn redim_single_bound_still_uses_option_base() {
    let source = "Option Base 1\nPublic r As Variant\nSub Main()\n    Dim a() As Long\n    ReDim a(3)\n    a(1) = 8\n    a(3) = 9\n    r = CStr(LBound(a)) & \":\" & CStr(UBound(a)) & \":\" & CStr(a(1) + a(3))\nEnd Sub\n";
    let outcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"));
    assert_eq!(snap.first().cloned().expect("snapshot slot"), s("1:3:17"));
}
