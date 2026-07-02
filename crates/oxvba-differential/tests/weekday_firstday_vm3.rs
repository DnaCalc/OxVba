//! vm3 `Weekday` should honor the optional `firstdayofweek` argument.

use oxvba_differential::{Canon, Executor, canon, run};
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
fn weekday_defaults_to_sunday_first() {
    assert_eq!(
        value("Weekday(DateSerial(2024, 1, 7))"),
        canon(&Variant::from_i32(1))
    );
    assert_eq!(
        value("Weekday(DateSerial(2024, 1, 8))"),
        canon(&Variant::from_i32(2))
    );
}

#[test]
fn weekday_honors_monday_first() {
    assert_eq!(
        value("Weekday(DateSerial(2024, 1, 7), 2)"),
        canon(&Variant::from_i32(7))
    );
    assert_eq!(
        value("Weekday(DateSerial(2024, 1, 8), 2)"),
        canon(&Variant::from_i32(1))
    );
}

#[test]
fn weekday_explicit_sunday_matches_default() {
    assert_eq!(
        value("Weekday(DateSerial(2024, 1, 8), 1)"),
        canon(&Variant::from_i32(2))
    );
    assert_eq!(
        value("Weekday(DateSerial(2024, 1, 8), 0)"),
        canon(&Variant::from_i32(2))
    );
}
