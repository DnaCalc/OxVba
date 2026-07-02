//! vm3 `Option Explicit` ordinary undeclared-name parity.
//!
//! Live Excel/VBA 7.1 oracle evidence:
//! `docs/evidence/conformance/vm3_option_explicit_oracle_20260702T140228Z/`.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn first(source: &str) -> Canon {
    let outcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}\n{source}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("run failed: {err}\n{source}"));
    values.first().cloned().expect("snapshot slot")
}

fn s(text: &str) -> Canon {
    canon(&Variant::from_string(text))
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
fn undeclared_read_without_option_explicit_is_empty_variant_local() {
    let source = r#"
Public result As String
Sub Main()
    result = CStr(VarType(x)) & ":" & TypeName(x)
End Sub
"#;
    assert_eq!(first(source), s("0:Empty"));
}

#[test]
fn undeclared_assignment_without_option_explicit_creates_variant_local() {
    let source = r#"
Public result As String
Sub Main()
    x = 7
    result = CStr(x) & ":" & CStr(VarType(x))
End Sub
"#;
    assert_eq!(first(source), s("7:2"));
}

#[test]
fn undeclared_byref_argument_without_option_explicit_is_real_local() {
    let source = r#"
Public result As String
Sub Main()
    Inc x
    result = CStr(x) & ":" & CStr(VarType(x))
End Sub

Private Sub Inc(ByRef v As Variant)
    v = 12
End Sub
"#;
    assert_eq!(first(source), s("12:2"));
}

#[test]
fn missing_statement_call_without_option_explicit_is_not_implicit_variable() {
    let source = r#"
Public result As String
Sub Main()
    MissingProc
    result = "unreachable"
End Sub
"#;
    assert_bind_rejected_with(source, &["Sub or Function not defined"]);
}

#[test]
fn missing_expression_call_without_option_explicit_is_not_implicit_indexed_variable() {
    let source = r#"
Public result As String
Sub Main()
    x = MissingProc(1)
    result = "unreachable"
End Sub
"#;
    assert_bind_rejected_with(source, &["Sub or Function not defined"]);
}

#[test]
fn undeclared_read_with_option_explicit_is_variable_not_defined() {
    let source = r#"
Option Explicit
Public result As String
Sub Main()
    result = x
End Sub
"#;
    assert_bind_rejected_with(source, &["Variable not defined"]);
}

#[test]
fn undeclared_assignment_with_option_explicit_is_variable_not_defined() {
    let source = r#"
Option Explicit
Public result As String
Sub Main()
    x = 7
    result = x
End Sub
"#;
    assert_bind_rejected_with(source, &["Variable not defined"]);
}

#[test]
fn undeclared_byref_argument_with_option_explicit_is_variable_not_defined() {
    let source = r#"
Option Explicit
Public result As String
Sub Main()
    Inc x
    result = x
End Sub

Private Sub Inc(ByRef v As Variant)
    v = 12
End Sub
"#;
    assert_bind_rejected_with(source, &["Variable not defined"]);
}
