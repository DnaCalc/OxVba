//! vm3 `Date` → String renders as a formatted date, not the raw serial number.
//!
//! `CStr`/`Print`/`&` of a `Date` previously emitted the underlying serial (e.g. "43845");
//! VBA renders it as the locale "General Date" form. Closes `date-to-string-emits-serial`.
//! (`Date + Date` arithmetic still loses the `Date` subtype — the separate
//! `date-arith-loses-date-type` gap — so the combined date+time case uses `CDate` to retype.)

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn eval_str(expr: &str) -> Canon {
    let source = format!("Public r As String\nSub Main()\n    r = {expr}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    outcome
        .result
        .unwrap_or_else(|e| panic!("vm3 run failed: {e}"))
        .into_iter()
        .next()
        .expect("global r")
}

fn s(text: &str) -> Canon {
    canon(&Variant::from_string(text))
}

#[test]
fn date_only_renders_short_date() {
    assert_eq!(eval_str("CStr(DateSerial(2020, 1, 15))"), s("1/15/2020"));
}

#[test]
fn time_only_renders_long_time() {
    assert_eq!(eval_str("CStr(TimeSerial(13, 30, 0))"), s("1:30:00 PM"));
    assert_eq!(eval_str("CStr(TimeSerial(0, 0, 0))"), s("12:00:00 AM"));
}

#[test]
fn date_and_time_render_together() {
    assert_eq!(
        eval_str("CStr(CDate(DateSerial(2020, 1, 15) + TimeSerial(13, 30, 5)))"),
        s("1/15/2020 1:30:05 PM")
    );
}

#[test]
fn date_in_concatenation_is_formatted() {
    // `&` uses the same display path, so a concatenated date is formatted, not a serial.
    assert_eq!(
        eval_str("\"D=\" & DateSerial(2020, 1, 15)"),
        s("D=1/15/2020")
    );
}
