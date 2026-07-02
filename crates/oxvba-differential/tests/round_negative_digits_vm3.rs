//! vm3 `Round` should reject negative decimal-place counts with VBA error 5.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn run_expr(expr: &str) -> oxvba_differential::RunOutcome {
    let source = format!("Public r As Variant\nSub Main()\n    r = {expr}\nEnd Sub\n");
    run(Executor::Vm3, &source)
}

fn value(expr: &str) -> Canon {
    let outcome = run_expr(expr);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"));
    snap.first().cloned().expect("snapshot slot")
}

#[test]
fn round_rejects_negative_decimal_places_with_error_5() {
    let outcome = run_expr("Round(19, -1)");
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.raised,
        "negative digits should raise a VBA error, got {:?}",
        outcome.result
    );
    assert_eq!(outcome.err.number, 5);
}

#[test]
fn round_default_and_positive_places_still_work() {
    assert_eq!(value("Round(2.5)"), canon(&Variant::from_f64(2.0)));
    assert_eq!(value("Round(1.25, 1)"), canon(&Variant::from_f64(1.2)));
}
