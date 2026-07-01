//! vm3 coverage for `Command`/`Command$` and the `Error`/`Error$` function.

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
fn command_returns_deterministic_empty_string() {
    let snap = snapshot(
        "    Dim a As String, b As String\n\
             a = \"[\" & Command & \"]\"\n\
             b = \"[\" & Command$() & \"]\"",
    );
    assert_contains_str(&snap, "[]");
}

#[test]
fn error_function_returns_messages_and_current_err_description() {
    let snap = snapshot(
        "    Dim known As String, fallback As String, beforeErr As String\n\
             Dim afterErr As String, afterClear As String\n\
             known = Error(11)\n\
             fallback = Error(12345)\n\
             beforeErr = \"[\" & Error() & \"]\"\n\
             On Error Resume Next\n\
             Err.Raise 11\n\
             afterErr = Error()\n\
             Err.Clear\n\
             afterClear = \"[\" & Error$() & \"]\"",
    );
    assert_contains_str(&snap, "Division by zero");
    assert_contains_str(&snap, "Application-defined or object-defined error");
    assert_contains_str(&snap, "[]");
}

#[test]
fn error_function_rejects_invalid_numbers() {
    let outcome = run(
        Executor::Vm3,
        "Sub Main()\n    Dim s As String\n    s = Error(-1)\nEnd Sub\n",
    );
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.result.is_err(),
        "invalid Error argument should raise: {outcome:?}"
    );
    assert_eq!(outcome.err.number, 5);
}
