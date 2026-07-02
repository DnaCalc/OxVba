//! vm3 date-conversion conformance: `IsDate`, `DateValue`, `CDate` vs the VBA spec.
//!
//! `IsDate` was always False (it routed through a `coerce_to(.., Date)` with no successful
//! arm); now it is True for a Date or a string recognizable as a valid date/time, and False
//! for a raw number / invalid date string. `DateValue` only accepted strings (raising error
//! 13 on a Date or numeric serial); now it accepts a Date/numeric and returns the date-only
//! part. Invalid calendar dates (Feb 30) are rejected by all three.

use oxvba_differential::{Canon, Executor, RunOutcome, canon, run};
use oxvba_runtime::Variant;

fn assert_contains(source: &str, expected: &Canon) {
    let outcome: RunOutcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined as unsupported: {:?}\nsource:\n{source}",
        outcome.unsupported
    );
    match &outcome.result {
        Ok(values) => assert!(
            values.contains(expected),
            "vm3 result {values:?} does not contain {expected:?}\nsource:\n{source}"
        ),
        Err(msg) => panic!("vm3 run failed: {msg}\nsource:\n{source}"),
    }
}

fn assert_raises_number(source: &str, number: i32) {
    let outcome: RunOutcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.result.is_err(),
        "unexpectedly completed: {:?}",
        outcome.result
    );
    assert_eq!(
        outcome.err.number, number,
        "err={:?}\nsource:\n{source}",
        outcome.err
    );
}

/// Run `Sub Main` with `result = <expr>` and assert the Boolean result.
fn assert_bool_expr(expr: &str, expected: bool) {
    let source = format!("Public result As Boolean\nSub Main()\n    result = {expr}\nEnd Sub\n");
    assert_contains(&source, &canon(&Variant::from_bool(expected)));
}

#[test]
fn isdate_true_for_valid_date_strings() {
    assert_bool_expr(r#"IsDate("1 Jan 2000")"#, true);
    assert_bool_expr(r#"IsDate("January 1, 2000")"#, true);
    assert_bool_expr(r#"IsDate("2000-01-01")"#, true);
    assert_bool_expr(r#"IsDate("1/1/2000")"#, true);
}

#[test]
fn isdate_true_for_time_strings() {
    assert_bool_expr(r#"IsDate("13:45")"#, true);
    assert_bool_expr(r#"IsDate("1:30:00")"#, true);
}

#[test]
fn isdate_false_for_invalid_or_nondate() {
    // Invalid calendar date.
    assert_bool_expr(r#"IsDate("February 30, 2000")"#, false);
    // Not a date at all.
    assert_bool_expr(r#"IsDate("not a date")"#, false);
    assert_bool_expr(r#"IsDate("")"#, false);
    // A raw number is NOT a date to IsDate (even though CDate would convert it).
    assert_bool_expr("IsDate(42)", false);
}

#[test]
fn isdate_true_for_a_date_value() {
    assert_bool_expr("IsDate(DateSerial(2026, 2, 28))", true);
}

#[test]
fn datevalue_of_a_date_returns_date_only() {
    // DateSerial gives a midnight Date; DateValue of it is the same date.
    let source = r#"
Public result As Boolean
Sub Main()
    Dim d As Date
    d = DateSerial(2026, 2, 28)
    result = (DateValue(d) = d)
End Sub
"#;
    assert_contains(source, &canon(&Variant::from_bool(true)));
}

#[test]
fn datevalue_of_a_numeric_serial_returns_that_date() {
    // DateValue(serial) returns the date for that serial (was error 13).
    let source = r#"
Public result As Boolean
Sub Main()
    result = (DateValue(DateSerial(2026, 2, 28)) = DateValue("2026-02-28"))
End Sub
"#;
    assert_contains(source, &canon(&Variant::from_bool(true)));
}

#[test]
fn cdate_of_invalid_date_raises_13() {
    assert_raises_number(
        "Sub Main()\n    Dim d As Date\n    d = CDate(\"February 30, 2000\")\nEnd Sub\n",
        13,
    );
}
