//! vm3 math intrinsics should raise VBA errors for invalid domains/overflow
//! instead of returning `NaN` or `Infinity`.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn run_expr(expr: &str) -> oxvba_differential::RunOutcome {
    let source = format!("Public r As Variant\nSub Main()\n    r = {expr}\nEnd Sub\n");
    run(Executor::Vm3, &source)
}

fn error_number(expr: &str) -> i32 {
    let outcome = run_expr(expr);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.raised,
        "expected a VBA error, got {:?}",
        outcome.result
    );
    outcome.err.number
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
fn sqr_and_log_invalid_domains_raise_error_5() {
    assert_eq!(error_number("Sqr(-1)"), 5);
    assert_eq!(error_number("Log(0)"), 5);
    assert_eq!(error_number("Log(-1)"), 5);
}

#[test]
fn exp_overflow_raises_error_6() {
    assert_eq!(error_number("Exp(1000)"), 6);
}

#[test]
fn valid_math_values_still_work() {
    assert_eq!(value("Sqr(9)"), canon(&Variant::from_f64(3.0)));
    assert_eq!(value("Log(1)"), canon(&Variant::from_f64(0.0)));
    assert_eq!(value("Exp(0)"), canon(&Variant::from_f64(1.0)));
}
