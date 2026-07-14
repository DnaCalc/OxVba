//! vm3 `DateAdd` month/year arithmetic clamps the day to the target month
//! (VBA never rolls `1/31` past the end of February) and preserves the time of
//! day. Previously the day was reused verbatim (Feb-31 normalized into March)
//! and the time-of-day fraction was dropped.

use oxvba_differential::{Executor, canon, run};
use oxvba_runtime::Variant;

/// Run `result = <expr>` (Boolean) and return whether it evaluated to True.
fn is_true(expr: &str) -> bool {
    let source = format!("Public result As Boolean\nSub Main()\n    result = {expr}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let values = outcome.result.unwrap_or_else(|e| panic!("run failed for {expr}: {e}"));
    values.contains(&canon(&Variant::from_bool(true)))
}

/// `|a - b| < 1e-7` at the VBA level (robust to serial float representation).
fn close(a: &str, b: &str) -> String {
    format!("Abs(CDbl({a}) - CDbl({b})) < 0.0000001")
}

#[test]
fn dateadd_month_clamps_to_end_of_shorter_month() {
    // 1/31 + 1 month -> 2/28 (not 3/3).
    assert!(is_true(&close("DateAdd(\"m\", 1, #1/31/2021#)", "#2/28/2021#")));
    // 1/31 + 1 month in a leap year -> 2/29.
    assert!(is_true(&close("DateAdd(\"m\", 1, #1/31/2020#)", "#2/29/2020#")));
    // 3-month quarter that lands on a short month clamps too: 11/30 + 1q -> 2/28.
    assert!(is_true(&close("DateAdd(\"q\", 1, #11/30/2020#)", "#2/28/2021#")));
}

#[test]
fn dateadd_year_clamps_leap_day() {
    // 2/29 + 1 year -> 2/28 (target year is not a leap year).
    assert!(is_true(&close("DateAdd(\"yyyy\", 1, #2/29/2020#)", "#2/28/2021#")));
}

#[test]
fn dateadd_month_without_clamp_is_unchanged() {
    // A day that exists in the target month is untouched.
    assert!(is_true(&close("DateAdd(\"m\", 1, #1/15/2021#)", "#2/15/2021#")));
    assert!(is_true(&close("DateAdd(\"m\", 2, #1/15/2021#)", "#3/15/2021#")));
}

#[test]
fn dateadd_month_preserves_time_of_day() {
    // #1/31/2021# + 0.625 == 1/31/2021 3:00 PM; +1 month -> 2/28/2021 3:00 PM.
    assert!(is_true(&close(
        "DateAdd(\"m\", 1, #1/31/2021# + 0.625)",
        "#2/28/2021# + 0.625"
    )));
    // A non-clamped date keeps its time too.
    assert!(is_true(&close(
        "DateAdd(\"m\", 1, #1/15/2021# + 0.25)",
        "#2/15/2021# + 0.25"
    )));
}
