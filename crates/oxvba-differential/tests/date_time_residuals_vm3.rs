//! vm3 date/time residual coverage for the Tier 4/5 first-wave batch.
//!
//! These cases pin small VBA date/time semantics that used to diverge in the
//! shared library/runtime helpers: `DateDiff("w"/"ww")`, valid `Date` range
//! enforcement, negative serial decomposition, strict `TimeValue` parsing, and
//! second rounding that carries to the next displayed date.

use oxvba_differential::{Canon, Executor, RunOutcome, canon, run};
use oxvba_runtime::Variant;

fn run_main(decls: &str, body: &str) -> RunOutcome {
    let source = format!("{decls}Sub Main()\n{body}End Sub\n");
    run(Executor::Vm3, &source)
}

fn snapshot(decls: &str, body: &str) -> Vec<Canon> {
    let outcome = run_main(decls, body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"))
}

fn assert_raises(body: &str, number: i32) {
    let outcome = run_main("", body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(outcome.result.is_err(), "unexpected success: {outcome:?}");
    assert_eq!(outcome.err.number, number, "err={:?}", outcome.err);
}

#[test]
fn datediff_w_counts_matching_weekdays_not_days() {
    let snap = snapshot(
        "Public beforeNext As Long\nPublic atNext As Long\nPublic reverse As Long\n",
        "    beforeNext = DateDiff(\"w\", DateSerial(2024, 1, 1), DateSerial(2024, 1, 7))\n\
         \u{20}   atNext = DateDiff(\"w\", DateSerial(2024, 1, 1), DateSerial(2024, 1, 8))\n\
         \u{20}   reverse = DateDiff(\"w\", DateSerial(2024, 1, 8), DateSerial(2024, 1, 1))\n",
    );
    assert_eq!(
        snap.first(),
        Some(&canon(&Variant::from_i32(0))),
        "{snap:?}"
    );
    assert_eq!(snap.get(1), Some(&canon(&Variant::from_i32(1))), "{snap:?}");
    assert_eq!(
        snap.get(2),
        Some(&canon(&Variant::from_i32(-1))),
        "{snap:?}"
    );
}

#[test]
fn datediff_ww_honors_firstdayofweek() {
    let snap = snapshot(
        "Public sundayFirst As Long\nPublic mondayFirst As Long\n",
        "    sundayFirst = DateDiff(\"ww\", DateSerial(2024, 1, 7), DateSerial(2024, 1, 8))\n\
         \u{20}   mondayFirst = DateDiff(\"ww\", DateSerial(2024, 1, 7), DateSerial(2024, 1, 8), vbMonday)\n",
    );
    assert_eq!(
        snap.first(),
        Some(&canon(&Variant::from_i32(0))),
        "{snap:?}"
    );
    assert_eq!(snap.get(1), Some(&canon(&Variant::from_i32(1))), "{snap:?}");
}

#[test]
fn negative_date_serial_uses_whole_number_date_part() {
    let snap = snapshot("Public r As String\n", "    r = CStr(CDate(-1.25))\n");
    assert_eq!(
        snap.first(),
        Some(&canon(&Variant::from_string("12/29/1899 6:00:00 AM"))),
        "{snap:?}"
    );
}

#[test]
fn date_range_is_validated_for_constructors_and_isdate() {
    assert_raises("    Dim d As Date\n    d = DateSerial(10000, 1, 1)\n", 5);
    assert_raises("    Dim d As Date\n    d = CDate(2958466#)\n", 5);

    let snap = snapshot("Public r As Boolean\n", "    r = IsDate(\"1/1/10000\")\n");
    assert_eq!(
        snap.first(),
        Some(&canon(&Variant::from_bool(false))),
        "{snap:?}"
    );
}

#[test]
fn timevalue_rejects_invalid_time_strings_and_accepts_ampm() {
    assert_raises("    Dim d As Date\n    d = TimeValue(\"not a time\")\n", 13);
    assert_raises("    Dim d As Date\n    d = TimeValue(\"25:00\")\n", 13);

    let snap = snapshot(
        "Public r As String\n",
        "    r = CStr(TimeValue(\"2:24PM\"))\n",
    );
    assert_eq!(
        snap.first(),
        Some(&canon(&Variant::from_string("2:24:00 PM"))),
        "{snap:?}"
    );
}

#[test]
fn combined_date_time_strings_accept_trailing_ampm() {
    let snap = snapshot(
        "Public cdateText As String\nPublic timeText As String\n",
        "    cdateText = CStr(CDate(\"2020-01-15 2:24 PM\"))\n\
         \u{20}   timeText = CStr(TimeValue(\"2020-01-15 2:24 PM\"))\n",
    );
    assert_eq!(
        snap.first(),
        Some(&canon(&Variant::from_string("1/15/2020 2:24:00 PM"))),
        "{snap:?}"
    );
    assert_eq!(
        snap.get(1),
        Some(&canon(&Variant::from_string("2:24:00 PM"))),
        "{snap:?}"
    );
}

#[test]
fn rounded_hms_can_carry_to_next_display_date() {
    let snap = snapshot(
        "Public r As String\n",
        "    r = CStr(CDate(DateSerial(2020, 1, 15) + 86399.6 / 86400#))\n",
    );
    assert_eq!(
        snap.first(),
        Some(&canon(&Variant::from_string("1/16/2020"))),
        "{snap:?}"
    );
}
