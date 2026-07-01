//! vm3 coverage for the fixed-format helper family:
//! `FormatNumber`, `FormatCurrency`, `FormatPercent`, and `FormatDateTime`.
//!
//! Locale-sensitive defaults are pinned to oxvba-lib's deterministic formatting
//! boundary: `.` decimal, `,` grouping, `$` currency, and the existing date masks.

use oxvba_differential::{Canon, Executor, run};

fn snapshot(body: &str) -> Vec<Canon> {
    let source = format!("Sub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 run failed: {err}\n{body}"))
}

fn assert_contains_str(values: &[Canon], expected: &str) {
    assert!(
        values.contains(&Canon::Str(expected.to_string())),
        "missing {expected:?} in {values:?}"
    );
}

#[test]
fn fixed_format_family_resolves_and_formats_values() {
    let snap = snapshot(
        "    Dim nDefault As String, nLead As String, nParens As String\n\
             Dim cParens As String, pOne As String, pLead As String\n\
             nDefault = FormatNumber(1234.567)\n\
             nLead = FormatNumber(0.5, 2, vbFalse, vbFalse, vbFalse)\n\
             nParens = FormatNumber(-1234.5, 2, vbTrue, vbTrue, vbTrue)\n\
             cParens = FormatCurrency(-1234.5, 2, vbTrue, vbTrue, vbTrue)\n\
             pOne = FormatPercent(0.1234, 1, vbTrue, vbFalse, vbFalse)\n\
             pLead = FormatPercent(0.005, 2, vbFalse, vbFalse, vbFalse)",
    );
    assert_contains_str(&snap, "1,234.57");
    assert_contains_str(&snap, ".50");
    assert_contains_str(&snap, "(1,234.50)");
    assert_contains_str(&snap, "($1,234.50)");
    assert_contains_str(&snap, "12.3%");
    assert_contains_str(&snap, ".50%");
}

#[test]
fn format_date_time_resolves_named_formats() {
    let snap = snapshot(
        "    Dim d As Date\n\
             Dim general As String, longDate As String, shortDate As String\n\
             Dim longTime As String, shortTime As String\n\
             d = DateSerial(2020, 1, 15) + TimeSerial(13, 30, 0)\n\
             general = FormatDateTime(d, vbGeneralDate)\n\
             longDate = FormatDateTime(d, vbLongDate)\n\
             shortDate = FormatDateTime(d, vbShortDate)\n\
             longTime = FormatDateTime(d, vbLongTime)\n\
             shortTime = FormatDateTime(d, vbShortTime)",
    );
    assert_contains_str(&snap, "1/15/2020 1:30:00 PM");
    assert_contains_str(&snap, "Wednesday, January 15, 2020");
    assert_contains_str(&snap, "1/15/2020");
    assert_contains_str(&snap, "1:30:00 PM");
    assert_contains_str(&snap, "13:30");
}

#[test]
fn format_family_rejects_invalid_options() {
    let bad_decimals = run(
        Executor::Vm3,
        "Sub Main()\n    Dim s As String\n    s = FormatNumber(1.2, -2)\nEnd Sub\n",
    );
    assert!(
        bad_decimals.unsupported.is_none(),
        "unsupported: {:?}",
        bad_decimals.unsupported
    );
    assert!(
        bad_decimals.result.is_err(),
        "invalid decimals should raise: {bad_decimals:?}"
    );
    assert_eq!(bad_decimals.err.number, 5);

    let bad_named_format = run(
        Executor::Vm3,
        "Sub Main()\n    Dim s As String\n    s = FormatDateTime(1.2, 5)\nEnd Sub\n",
    );
    assert!(
        bad_named_format.unsupported.is_none(),
        "unsupported: {:?}",
        bad_named_format.unsupported
    );
    assert!(
        bad_named_format.result.is_err(),
        "invalid named format should raise: {bad_named_format:?}"
    );
    assert_eq!(bad_named_format.err.number, 5);
}
