//! vm3 `ReDim` implicit-declaration parity.
//!
//! Live Excel/VBA 7.1 oracle evidence:
//! `docs/evidence/conformance/vm3_redim_implicit_oracle_20260701T2238Z/`.

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
fn redim_undeclared_declares_variant_array_without_option_explicit() {
    let source = r#"
Public result As String
Sub Main()
    ReDim a(1)
    a(0) = 7
    result = CStr(LBound(a)) & ":" & CStr(UBound(a)) & ":" & CStr(VarType(a)) & ":" & CStr(VarType(a(0))) & ":" & CStr(a(0))
End Sub
"#;
    assert_eq!(first(source), s("0:1:8204:2:7"));
}

#[test]
fn redim_undeclared_declares_variant_array_with_option_explicit() {
    let source = r#"
Option Explicit
Public result As String
Sub Main()
    ReDim a(1)
    a(0) = 7
    result = CStr(LBound(a)) & ":" & CStr(UBound(a)) & ":" & CStr(VarType(a)) & ":" & CStr(VarType(a(0))) & ":" & CStr(a(0))
End Sub
"#;
    assert_eq!(first(source), s("0:1:8204:2:7"));
}

#[test]
fn redim_declared_variant_target_matches_implicit_variant_array() {
    let source = r#"
Option Explicit
Public result As String
Sub Main()
    Dim a As Variant
    ReDim a(1)
    a(0) = 7
    result = CStr(LBound(a)) & ":" & CStr(UBound(a)) & ":" & CStr(VarType(a)) & ":" & CStr(VarType(a(0))) & ":" & CStr(a(0))
End Sub
"#;
    assert_eq!(first(source), s("0:1:8204:2:7"));
}

#[test]
fn redim_declared_dynamic_long_array_keeps_long_array_type() {
    let source = r#"
Option Explicit
Public result As String
Sub Main()
    Dim a() As Long
    ReDim a(1)
    a(0) = 7
    result = CStr(LBound(a)) & ":" & CStr(UBound(a)) & ":" & CStr(VarType(a)) & ":" & CStr(VarType(a(0))) & ":" & CStr(a(0))
End Sub
"#;
    assert_eq!(first(source), s("0:1:8195:3:7"));
}

#[test]
fn redim_preserve_does_not_implicitly_declare_target() {
    let source = r#"
Option Explicit
Public result As String
Sub Main()
    ReDim Preserve a(1)
    result = "unreachable"
End Sub
"#;
    assert_bind_rejected_with(source, &["VariableNotDefined"]);
}

#[test]
fn redim_preserve_does_not_implicitly_declare_target_without_option_explicit() {
    let source = r#"
Public result As String
Sub Main()
    ReDim Preserve a(1)
    result = "unreachable"
End Sub
"#;
    assert_bind_rejected_with(source, &["VariableNotDefined"]);
}

#[test]
fn redim_scalar_declared_target_is_expected_array() {
    let source = r#"
Option Explicit
Public result As String
Sub Main()
    Dim a As Long
    ReDim a(1)
    result = CStr(a)
End Sub
"#;
    assert_bind_rejected_with(source, &["ExpectedArray"]);
}
