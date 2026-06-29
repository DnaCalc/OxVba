//! vm3 mixed String-vs-numeric comparison raises Type mismatch (13).
//!
//! VBA compares only same-kind operands: a `String` compared with a numeric raises error 13,
//! even when the string is numeric-looking (`"5" = 5` → 13). Previously vm3 silently fell back
//! to a string or numeric comparison and returned a Boolean. `Empty` is exempt (it adapts to
//! the other operand). Closes `mixed-string-numeric-compare-no-13`. Operands are held in
//! variables so the comparison is evaluated at run time, not constant-folded.

use oxvba_differential::{Canon, Executor, RunOutcome, canon, run};
use oxvba_runtime::Variant;

fn run_main(body: &str) -> RunOutcome {
    let source = format!("Public r As Variant\nSub Main()\n{body}End Sub\n");
    run(Executor::Vm3, &source)
}

fn assert_raises_13(body: &str) {
    let outcome = run_main(body);
    assert!(outcome.unsupported.is_none(), "unsupported: {:?}", outcome.unsupported);
    assert!(outcome.result.is_err(), "expected Type mismatch, completed: {:?}", outcome.result);
    assert_eq!(outcome.err.number, 13, "err={:?}", outcome.err);
}

fn assert_first(body: &str, expected: &Canon) {
    let outcome = run_main(body);
    assert!(outcome.unsupported.is_none(), "unsupported: {:?}", outcome.unsupported);
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"));
    assert_eq!(snap.first(), Some(expected), "{snap:?}");
}

#[test]
fn string_vs_number_raises_13() {
    assert_raises_13("    Dim s As Variant\n    Dim n As Variant\n    s = \"abc\"\n    n = 5\n    r = (s = n)\n");
}

#[test]
fn numeric_string_vs_number_still_raises_13() {
    // `"5" = 5` is a Type mismatch in VBA — the string is NOT coerced to a number.
    assert_raises_13("    Dim s As Variant\n    Dim n As Variant\n    s = \"5\"\n    n = 5\n    r = (s < n)\n");
}

#[test]
fn empty_vs_number_is_exempt() {
    // Empty adapts to 0, so `Empty = 0` is True (no Type mismatch).
    assert_first(
        "    Dim e As Variant\n    Dim n As Variant\n    n = 0\n    r = (e = n)\n",
        &canon(&Variant::from_bool(true)),
    );
}

#[test]
fn same_kind_comparisons_still_work() {
    // String vs String compares lexically; number vs number compares numerically.
    assert_first(
        "    Dim a As Variant\n    Dim b As Variant\n    a = \"abc\"\n    b = \"abc\"\n    r = (a = b)\n",
        &canon(&Variant::from_bool(true)),
    );
    assert_first(
        "    Dim a As Variant\n    Dim b As Variant\n    a = 5\n    b = 5\n    r = (a = b)\n",
        &canon(&Variant::from_bool(true)),
    );
}
