//! vm3 coverage for `SendKeys` and `AppActivate`.
//!
//! Live Excel/VBA 7.1 oracle evidence:
//! `docs/evidence/conformance/vm3_sendkeys_appactivate_oracle_20260702T144333Z/`.

use oxvba_differential::{Canon, Executor, run};

fn snapshot(source: &str) -> Vec<Canon> {
    let outcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}\n{source}",
        outcome.unsupported
    );
    outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 run failed: {err}\n{source}"))
}

fn assert_contains_str(values: &[Canon], expected: &str) {
    assert!(
        values.contains(&Canon::Str(expected.to_string())),
        "missing {expected:?} in {values:?}"
    );
}

fn assert_bind_rejected_with(source: &str, fragments: &[&str]) {
    let outcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "expected bind rejection, got unsupported: {:?}",
        outcome.unsupported
    );
    let err = outcome.result.expect_err("expected bind rejection");
    for fragment in fragments {
        assert!(
            err.contains(fragment),
            "expected error to contain `{fragment}`, got {err:?}"
        );
    }
}

#[test]
fn sendkeys_empty_statement_succeeds_without_interaction_effect() {
    let values = snapshot(
        r#"
Public result As String
Sub Main()
    On Error Resume Next
    Err.Clear
    SendKeys "", False
    result = CStr(Err.Number) & ":" & Err.Description
End Sub
"#,
    );
    assert_contains_str(&values, "0:");
}

#[test]
fn appactivate_missing_window_raises_error_five() {
    let values = snapshot(
        r#"
Public result As String
Sub Main()
    On Error Resume Next
    Err.Clear
    AppActivate "__OXVBA_NO_SUCH_WINDOW_20260702__", False
    result = CStr(Err.Number) & ":" & Err.Description
End Sub
"#,
    );
    assert_contains_str(&values, "5:Invalid procedure call or argument");
}

#[test]
fn sendkeys_and_appactivate_are_not_value_expressions() {
    assert_bind_rejected_with(
        r#"
Public result As Variant
Sub Main()
    result = SendKeys("")
End Sub
"#,
        &["Expected Function or variable"],
    );
    assert_bind_rejected_with(
        r#"
Public result As Variant
Sub Main()
    result = AppActivate("__OXVBA_NO_SUCH_WINDOW_20260702__")
End Sub
"#,
        &["Expected Function or variable"],
    );
}
