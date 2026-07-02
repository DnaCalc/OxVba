//! vm3 array argument copy and whole-array assignment parity.
//!
//! Live Excel/VBA 7.1 oracle evidence:
//! `docs/evidence/conformance/vm3_array_copy_assignment_oracle_20260702T025158Z/`.

use oxvba_differential::{canon, run, Canon, Executor, RunOutcome};
use oxvba_runtime::Variant;

fn run_case(source: &str) -> RunOutcome {
    run(Executor::Vm3, source)
}

fn s(text: &str) -> Canon {
    canon(&Variant::from_string(text))
}

fn assert_contains_string(source: &str, expected: &str) {
    let outcome = run_case(source);
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined array copy/assignment case as unsupported: {:?}\n{source}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 array copy/assignment case failed: {err}\n{source}"));
    let expected = s(expected);
    assert!(
        values.contains(&expected),
        "snapshot {values:?} did not contain {expected:?}\n{source}"
    );
}

fn assert_bind_rejected_with(source: &str, fragments: &[&str]) {
    let outcome = run_case(source);
    assert!(
        outcome.unsupported.is_none(),
        "expected bind rejection, got unsupported: {:?}\n{source}",
        outcome.unsupported
    );
    let err = outcome.result.expect_err("expected compile/bind rejection");
    for fragment in fragments {
        assert!(
            err.contains(fragment),
            "expected error to contain `{fragment}`, got {err:?}\n{source}"
        );
    }
}

#[test]
fn dynamic_array_byref_parameter_aliases_caller() {
    assert_contains_string(
        r#"
Public result As String
Sub Main()
    Dim x() As Long
    ReDim x(0 To 1)
    x(0) = 7
    Touch x
    result = CStr(x(0)) & ":" & CStr(x(1)) & ":" & CStr(LBound(x)) & ":" & CStr(UBound(x))
End Sub

Private Sub Touch(ByRef a() As Long)
    a(0) = 99
End Sub
"#,
        "99:0:0:1",
    );
}

#[test]
fn typed_array_byval_parameter_is_compile_error() {
    assert_bind_rejected_with(
        r#"
Public result As String
Sub Main()
    result = "unreachable"
End Sub

Private Sub Touch(ByVal a() As Long)
    a(0) = 99
End Sub
"#,
        &["Array argument must be ByRef"],
    );
}

#[test]
fn typed_array_passed_to_byval_variant_is_copied() {
    assert_contains_string(
        r#"
Public result As String
Sub Main()
    Dim x() As Long
    ReDim x(0 To 1)
    x(0) = 7
    x(1) = 8
    Touch x
    result = CStr(x(0)) & ":" & CStr(x(1)) & ":" & CStr(UBound(x))
End Sub

Private Sub Touch(ByVal v As Variant)
    v(0) = 99
End Sub
"#,
        "7:8:1",
    );
}

#[test]
fn typed_array_passed_to_byref_variant_mutates_caller() {
    assert_contains_string(
        r#"
Public result As String
Sub Main()
    Dim x() As Long
    ReDim x(0 To 1)
    x(0) = 7
    x(1) = 8
    Touch x
    result = CStr(x(0)) & ":" & CStr(x(1)) & ":" & CStr(UBound(x))
End Sub

Private Sub Touch(ByRef v As Variant)
    v(0) = 99
End Sub
"#,
        "99:8:1",
    );
}

#[test]
fn dynamic_whole_array_assignment_copies_values_and_bounds() {
    assert_contains_string(
        r#"
Public result As String
Sub Main()
    Dim src() As Long
    Dim dst() As Long
    ReDim src(2 To 3)
    src(2) = 7
    src(3) = 8
    dst = src
    src(2) = 99
    result = CStr(dst(2)) & ":" & CStr(dst(3)) & ":" & CStr(LBound(dst)) & ":" & CStr(UBound(dst))
End Sub
"#,
        "7:8:2:3",
    );
}

#[test]
fn dynamic_whole_array_assignment_is_independent_across_redim_preserve() {
    assert_contains_string(
        r#"
Public result As String
Sub Main()
    Dim src() As Long
    Dim dst() As Long
    ReDim src(0 To 1)
    src(0) = 7
    src(1) = 8
    dst = src
    ReDim Preserve src(0 To 2)
    src(0) = 99
    src(2) = 123
    result = CStr(dst(0)) & ":" & CStr(dst(1)) & ":" & CStr(LBound(dst)) & ":" & CStr(UBound(dst)) & ":" & CStr(UBound(src))
End Sub
"#,
        "7:8:0:1:2",
    );
}

#[test]
fn whole_array_assignment_to_fixed_lhs_from_dynamic_is_compile_error() {
    assert_bind_rejected_with(
        r#"
Public result As String
Sub Main()
    Dim src() As Long
    Dim dst(0 To 1) As Long
    ReDim src(0 To 1)
    src(0) = 7
    dst = src
    result = CStr(dst(0))
End Sub
"#,
        &["Can't assign to array"],
    );
}

#[test]
fn whole_array_assignment_to_fixed_lhs_from_fixed_is_compile_error() {
    assert_bind_rejected_with(
        r#"
Public result As String
Sub Main()
    Dim src(0 To 1) As Long
    Dim dst(0 To 1) As Long
    src(0) = 7
    dst = src
    result = CStr(dst(0))
End Sub
"#,
        &["Can't assign to array"],
    );
}

#[test]
fn dynamic_whole_array_assignment_from_fixed_rhs_is_allowed() {
    assert_contains_string(
        r#"
Public result As String
Sub Main()
    Dim src(0 To 1) As Long
    Dim dst() As Long
    src(0) = 7
    dst = src
    result = CStr(dst(0)) & ":" & CStr(LBound(dst)) & ":" & CStr(UBound(dst))
End Sub
"#,
        "7:0:1",
    );
}
