//! vm3 `Round` should accept negative decimal places and keep banker's
//! rounding at the shifted place.

use oxvba_differential::{canon, run, Canon, Executor};
use oxvba_runtime::Variant;

fn value(expr: &str) -> Canon {
    let source = format!("Public r As Variant\nSub Main()\n    r = {expr}\nEnd Sub\n");
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
fn round_accepts_negative_decimal_places() {
    assert_eq!(value("Round(19, -1)"), canon(&Variant::from_f64(20.0)));
    assert_eq!(
        value("Round(1234.5, -2)"),
        canon(&Variant::from_f64(1200.0))
    );
}

#[test]
fn round_negative_places_still_uses_half_even() {
    assert_eq!(value("Round(1250, -2)"), canon(&Variant::from_f64(1200.0)));
    assert_eq!(value("Round(1350, -2)"), canon(&Variant::from_f64(1400.0)));
}

#[test]
fn round_default_and_positive_places_still_work() {
    assert_eq!(value("Round(2.5)"), canon(&Variant::from_f64(2.0)));
    assert_eq!(value("Round(1.25, 1)"), canon(&Variant::from_f64(1.2)));
}
