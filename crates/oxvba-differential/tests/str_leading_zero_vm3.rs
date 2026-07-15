//! vm3 `Str` reproduces the VB `Str$` quirk of dropping the leading zero of a
//! magnitude below 1 (`Str(0.5)` = " .5", `Str(-0.5)` = "-.5"), while keeping
//! the leading space for non-negative values and leaving `CStr` untouched.

use oxvba_differential::{Executor, canon, run};
use oxvba_runtime::Variant;

fn assert_expr(expr: &str, expected: &str) {
    let source = format!("Public result As String\nSub Main()\n    result = {expr}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|e| panic!("run failed for {expr}: {e}"));
    assert!(
        values.contains(&canon(&Variant::from_string(expected.to_string()))),
        "{expr} = {values:?}, expected to contain {expected:?}"
    );
}

#[test]
fn str_drops_leading_zero_below_one() {
    assert_expr("Str(0.5)", " .5");
    assert_expr("Str(0.25)", " .25");
    assert_expr("Str(-0.5)", "-.5");
    assert_expr("Str(-0.25)", "-.25");
}

#[test]
fn str_keeps_leading_space_and_whole_numbers() {
    assert_expr("Str(5)", " 5");
    assert_expr("Str(-5)", "-5");
    assert_expr("Str(0)", " 0");
    assert_expr("Str(1.5)", " 1.5");
    assert_expr("Str(-1.5)", "-1.5");
}

#[test]
fn cstr_keeps_the_leading_zero() {
    // The Str$ quirk must not leak into CStr.
    assert_expr("CStr(0.5)", "0.5");
    assert_expr("CStr(-0.5)", "-0.5");
}
