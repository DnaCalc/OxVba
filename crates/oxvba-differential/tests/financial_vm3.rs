//! Financial depreciation/amortization intrinsics on vm3
//! (gap: financial-ipmt-ppmt-sln-syd-ddb-absent).
//!
//! Expected values are live-verified against VBA 7.1. The exact integer-valued
//! results (SLN/SYD/DDB) are asserted by value; the irrational IPmt/PPmt results
//! are checked inside the program against the live values within a tight
//! tolerance (so the assertion does not depend on last-bit f64 identity), and the
//! single annuity-due edge case (`IPmt(per:=1, type:=1) = 0`) is pinned too.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

#[test]
fn depreciation_and_amortization_match_vba() {
    let source = "\
Public sln5 As Double
Public syd1 As Double
Public syd5 As Double
Public ddb1 As Double
Public ddb5 As Double
Public ddbf3 As Double
Public amortOk As Boolean
Sub Main()
    sln5 = SLN(10000, 1000, 5)
    syd1 = SYD(10000, 1000, 5, 1)
    syd5 = SYD(10000, 1000, 5, 5)
    ddb1 = DDB(10000, 1000, 5, 1)
    ddb5 = DDB(10000, 1000, 5, 5)
    ddbf3 = DDB(10000, 1000, 5, 1, 3)
    amortOk = True
    amortOk = amortOk And (Abs(IPmt(0.1 / 12, 1, 36, 10000) - (-83.3333333333333)) < 0.0000001)
    amortOk = amortOk And (Abs(IPmt(0.1 / 12, 2, 36, 10000) - (-81.3388455116247)) < 0.0000001)
    amortOk = amortOk And (Abs(PPmt(0.1 / 12, 1, 36, 10000) - (-239.338538605042)) < 0.0000001)
    amortOk = amortOk And (IPmt(0.1 / 12, 1, 36, 10000, 0, 1) = 0)
    amortOk = amortOk And (Abs(IPmt(0.1 / 12, 2, 36, 10000, 0, 1) - (-80.6666236478922)) < 0.0000001)
End Sub
";
    let outcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome
        .result
        .unwrap_or_else(|e| panic!("vm3 run failed: {e}"));

    let has_f64 = |snap: &[Canon], v: f64| snap.contains(&canon(&Variant::from_f64(v)));
    assert!(has_f64(&snap, 1800.0), "SLN(10000,1000,5)=1800: {snap:?}");
    assert!(has_f64(&snap, 3000.0), "SYD(...,1)=3000: {snap:?}");
    assert!(has_f64(&snap, 600.0), "SYD(...,5)=600: {snap:?}");
    assert!(has_f64(&snap, 4000.0), "DDB(...,1)=4000: {snap:?}");
    assert!(
        has_f64(&snap, 296.0),
        "DDB(...,5)=296 (salvage-floored): {snap:?}"
    );
    assert!(
        has_f64(&snap, 6000.0),
        "DDB(...,1,factor:=3)=6000: {snap:?}"
    );
    assert!(
        snap.contains(&canon(&Variant::from_bool(true)))
            && !snap.contains(&canon(&Variant::from_bool(false))),
        "IPmt/PPmt amortization checks should all hold: {snap:?}"
    );
}
