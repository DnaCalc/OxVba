//! vm3 `Mid` function and statement forms should reject start positions less
//! than 1 with runtime error 5, matching live Excel/VBA 7.1.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn value(body: &str) -> Canon {
    let source = format!("Public r As Variant\nSub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}\n{source}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("Mid probe failed: {err}\n{source}"));
    values
        .first()
        .cloned()
        .unwrap_or_else(|| panic!("empty result: {values:?}\n{source}"))
}

fn error_number(body: &str) -> i32 {
    let source = format!("Sub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}\n{source}",
        outcome.unsupported
    );
    assert!(
        outcome.raised,
        "expected runtime error, got {:?}\n{source}",
        outcome.result
    );
    outcome.err.number
}

#[test]
fn mid_function_rejects_start_less_than_one() {
    assert_eq!(
        error_number("    Dim s As Variant\n    s = Mid(\"abcdef\", 0, 2)"),
        5
    );
    assert_eq!(
        error_number("    Dim s As Variant\n    s = Mid(\"abcdef\", -1, 2)"),
        5
    );
}

#[test]
fn mid_statement_rejects_start_less_than_one_without_mutating_target() {
    assert_eq!(
        value(
            "    On Error Resume Next\n    Dim s As String\n    s = \"abcdef\"\n    Mid(s, 0, 2) = \"ZZ\"\n    r = CStr(Err.Number) & \":\" & s"
        ),
        canon(&Variant::from_string("5:abcdef"))
    );
    assert_eq!(
        value(
            "    On Error Resume Next\n    Dim s As String\n    s = \"abcdef\"\n    Mid(s, -1, 2) = \"ZZ\"\n    r = CStr(Err.Number) & \":\" & s"
        ),
        canon(&Variant::from_string("5:abcdef"))
    );
}

#[test]
fn valid_mid_function_and_statement_controls_still_work() {
    assert_eq!(
        value("    r = Mid(\"abcdef\", 1, 2)"),
        canon(&Variant::from_string("ab"))
    );
    assert_eq!(
        value("    r = Mid(\"abcdef\", 99, 2)"),
        canon(&Variant::from_string(""))
    );
    assert_eq!(
        value("    Dim s As String\n    s = \"abcdef\"\n    Mid(s, 2, 2) = \"ZZ\"\n    r = s"),
        canon(&Variant::from_string("aZZdef"))
    );
}
