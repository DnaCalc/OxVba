//! vm3 `Format`/`FormatNumber` coerce a numeric/date **String** operand before
//! formatting, instead of treating every string as 0. Format's inputs are
//! pervasively strings (cell text, control `.Value`, DB fields), and
//! `coerce_to(String, Double)` had no arm, so `Format("123.5","0.00")` was
//! "0.00".

use oxvba_differential::{Canon, Executor, run};

fn result_of(expr: &str) -> Vec<Canon> {
    let source = format!("Public r As Variant\nSub Main()\n    r = {expr}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 run failed for {expr}: {err}"))
}

fn assert_formats(expr: &str, expected: &str) {
    let values = result_of(expr);
    assert!(
        values.contains(&Canon::Str(expected.to_string())),
        "{expr} = {values:?}, expected {expected:?}"
    );
}

#[test]
fn numeric_string_operand_is_parsed_not_zeroed() {
    assert_formats("Format(\"123.5\", \"0.00\")", "123.50");
    assert_formats("Format(\"42\", \"General Number\")", "42");
    assert_formats("FormatNumber(\"1234.5\", 2)", "1,234.50");
    assert_formats("Format(\"-7\", \"0.00\")", "-7.00");
}

#[test]
fn date_string_operand_under_date_mask_is_parsed() {
    assert_formats("Format(\"2020-05-01\", \"yyyy\")", "2020");
}

#[test]
fn non_numeric_string_still_formats_as_zero() {
    // A genuinely non-numeric string is 0 (VBA), not an error.
    assert_formats("Format(\"abc\", \"0.00\")", "0.00");
}

#[test]
fn numeric_operand_is_unchanged() {
    // Regression: a real numeric operand still formats as before.
    assert_formats("Format(123.5, \"0.00\")", "123.50");
    assert_formats("FormatNumber(1234.5, 2)", "1,234.50");
}
