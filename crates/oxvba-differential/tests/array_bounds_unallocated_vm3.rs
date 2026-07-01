//! vm3 `LBound`/`UBound` should raise error 13 on unallocated dynamic arrays.

use oxvba_differential::{canon, run, Canon, Executor};
use oxvba_runtime::Variant;

fn run_body(body: &str) -> oxvba_differential::RunOutcome {
    let source = format!("Public r As Variant\nSub Main()\n{body}End Sub\n");
    run(Executor::Vm3, &source)
}

fn error_number(body: &str) -> i32 {
    let outcome = run_body(body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(outcome.raised, "expected error, got {:?}", outcome.result);
    outcome.err.number
}

fn value(body: &str) -> Canon {
    let outcome = run_body(body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"));
    snap.first().cloned().expect("snapshot slot")
}

#[test]
fn lbound_ubound_unallocated_dynamic_array_raise_13() {
    assert_eq!(error_number("    Dim a() As Long\n    r = LBound(a)\n"), 13);
    assert_eq!(error_number("    Dim a() As Long\n    r = UBound(a)\n"), 13);
}

#[test]
fn lbound_ubound_erased_dynamic_array_raise_13() {
    let body = "    Dim a() As Long\n    ReDim a(1 To 3)\n    Erase a\n";
    assert_eq!(error_number(&format!("{body}    r = LBound(a)\n")), 13);
    assert_eq!(error_number(&format!("{body}    r = UBound(a)\n")), 13);
}

#[test]
fn allocated_arrays_still_report_bounds() {
    assert_eq!(
        value("    Dim a() As Long\n    ReDim a(2 To 4)\n    r = CStr(LBound(a)) & \":\" & CStr(UBound(a))\n"),
        canon(&Variant::from_string("2:4".to_string()))
    );
    assert_eq!(
        value("    Dim a(5 To 6) As Long\n    r = CStr(LBound(a)) & \":\" & CStr(UBound(a))\n"),
        canon(&Variant::from_string("5:6".to_string()))
    );
}
