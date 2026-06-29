//! vm3 `Null` propagation through value-returning string functions.
//!
//! VBA returns `Null` when a string function is given a `Null` argument; vm3 instead reached
//! `variant_to_vba_string`, which raises Type mismatch 13. Closes
//! `null-not-propagated-string-fns` (the `$`-suffixed forms should raise 94 instead, but the
//! binder does not yet thread the suffix, so they also return `Null` for now). The `Null` is
//! held in a variable so the call is evaluated at run time, not constant-folded.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

/// Run `Main` with a `Null` variable `n`, assigning `r = <expr>`, and return `r` (global 0).
fn eval_with_null(expr: &str) -> Canon {
    let source = format!(
        "Public r As Variant\nSub Main()\n    Dim n As Variant\n    n = Null\n    r = {expr}\nEnd Sub\n"
    );
    let outcome = run(Executor::Vm3, &source);
    assert!(outcome.unsupported.is_none(), "unsupported: {:?}", outcome.unsupported);
    let snap = outcome.result.unwrap_or_else(|e| panic!("vm3 run failed: {e}\n{source}"));
    snap.into_iter().next().expect("global r")
}

#[test]
fn string_functions_propagate_null() {
    let null = canon(&Variant::null());
    for expr in [
        "Left(n, 2)",
        "Right(n, 2)",
        "Mid(n, 1)",
        "UCase(n)",
        "LCase(n)",
        "Trim(n)",
        "Len(n)",
        "StrReverse(n)",
        "Replace(n, \"a\", \"b\")",
        "InStr(n, \"x\")",
        "InStr(1, n, \"x\")",
        "StrComp(n, \"a\")",
        "Chr(n)",
        "Asc(n)",
    ] {
        assert_eq!(eval_with_null(expr), null, "{expr} should be Null");
    }
}

#[test]
fn non_null_string_functions_still_work() {
    // The guard must not disturb the ordinary (non-Null) path.
    assert_eq!(eval_with_null("UCase(\"abc\")"), canon(&Variant::from_string("ABC")));
    assert_eq!(eval_with_null("Len(\"abcd\")"), canon(&Variant::from_i32(4)));
}
