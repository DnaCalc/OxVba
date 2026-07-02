//! vm3 implicit String→Boolean coercion conformance.
//!
//! `Dim b As Boolean: b = "True"` used to raise error 13: the implicit Let-coercion
//! ran through `arith::coerce_numeric`'s Boolean arm, which parsed the string as an
//! f64 (`"True"` fails → Type mismatch), while explicit `CBool("True")` succeeded via
//! `pure::cbool`'s `"true"`/`"false"` recognizer. VBA's implicit assignment to a typed
//! variable uses the *same* conversion as the explicit `C…` function, so the two must
//! agree. The fix shares one `oxvba_runtime::coerce::parse_bool_text` recognizer between
//! both paths; these tests pin that implicit and explicit now match.

use oxvba_differential::{Executor, RunOutcome, canon, run};
use oxvba_runtime::Variant;

fn assert_bool(source: &str, expected: bool) {
    let outcome: RunOutcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined as unsupported: {:?}\nsource:\n{source}",
        outcome.unsupported
    );
    match &outcome.result {
        Ok(values) => assert!(
            values.contains(&canon(&Variant::from_bool(expected))),
            "vm3 result {values:?} does not contain {expected}\nsource:\n{source}"
        ),
        Err(msg) => panic!("vm3 run failed: {msg}\nsource:\n{source}"),
    }
}

/// `Dim b As Boolean: b = <expr>` (implicit Let-coercion).
fn assert_implicit(rhs: &str, expected: bool) {
    let source = format!("Public result As Boolean\nSub Main()\n    result = {rhs}\nEnd Sub\n");
    assert_bool(&source, expected);
}

/// `result = CBool(<expr>)` (explicit conversion) — the oracle the implicit path must match.
fn assert_explicit(arg: &str, expected: bool) {
    let source =
        format!("Public result As Boolean\nSub Main()\n    result = CBool({arg})\nEnd Sub\n");
    assert_bool(&source, expected);
}

/// Assert that running `source` raises the given VBA error number.
fn assert_raises(source: &str, number: i32) {
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

#[test]
fn implicit_string_true_false_literals() {
    assert_implicit(r#""True""#, true);
    assert_implicit(r#""False""#, false);
}

#[test]
fn implicit_string_true_false_are_case_insensitive() {
    assert_implicit(r#""true""#, true);
    assert_implicit(r#""FALSE""#, false);
    assert_implicit(r#""tRuE""#, true);
}

#[test]
fn implicit_numeric_strings_are_truthy_by_nonzero() {
    assert_implicit(r#""5""#, true);
    assert_implicit(r#""0""#, false);
    assert_implicit(r#""-1""#, true);
    assert_implicit(r#""3.7""#, true);
}

#[test]
fn whitespace_padded_bool_literal_is_type_mismatch() {
    // Live VBA does NOT trim before the True/False match: `CBool("  False  ")` and the
    // implicit `b = "  False  "` both raise Type mismatch (13). The padded string is not a
    // recognized literal and is not numeric, so it falls through to the numeric path → 13.
    assert_raises(
        "Public result As Boolean\nSub Main()\n    result = \"  False  \"\nEnd Sub\n",
        13,
    );
    assert_raises(
        "Public result As Boolean\nSub Main()\n    result = CBool(\"  False  \")\nEnd Sub\n",
        13,
    );
}

#[test]
fn implicit_matches_explicit_cbool() {
    for (arg, expected) in [
        (r#""True""#, true),
        (r#""False""#, false),
        (r#""5""#, true),
        (r#""0""#, false),
    ] {
        assert_implicit(arg, expected);
        assert_explicit(arg, expected);
    }
}
