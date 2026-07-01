//! vm3 must keep VBA `Nothing` distinct from `Empty`.
//!
//! Live Excel/VBA 7.1 evidence:
//! `docs/evidence/conformance/vm3_nothing_oracle_20260701T151239Z/`.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn run_result(body: &str) -> Canon {
    let source = format!("Public r As String\nSub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    outcome
        .result
        .unwrap_or_else(|e| panic!("vm3 run failed: {e}"))
        .into_iter()
        .next()
        .expect("global r")
}

fn s(text: &str) -> Canon {
    canon(&Variant::from_string(text))
}

fn assert_raises(source_body: &str, number: i32) {
    let source = format!("Sub Main()\n{source_body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.result.is_err(),
        "vm3 unexpectedly completed: {:?}",
        outcome.result
    );
    assert!(outcome.raised, "expected raised VBA error, got {outcome:?}");
    assert_eq!(outcome.err.number, number, "err={:?}", outcome.err);
}

#[test]
fn nothing_literal_introspection_is_object_null() {
    assert_eq!(
        run_result(
            "    r = CStr(VarType(Nothing)) & \":\" & TypeName(Nothing) & \":\" & CStr(IsObject(Nothing)) & \":\" & CStr(IsEmpty(Nothing))"
        ),
        s("9:Nothing:True:False")
    );
}

#[test]
fn object_variable_set_nothing_introspection() {
    assert_eq!(
        run_result(
            "    Dim o As Object\n    Set o = Nothing\n    r = CStr(VarType(o)) & \":\" & TypeName(o) & \":\" & CStr(IsObject(o)) & \":\" & CStr(IsEmpty(o)) & \":\" & CStr(o Is Nothing)"
        ),
        s("9:Nothing:True:False:True")
    );
}

#[test]
fn unset_object_variable_introspection() {
    assert_eq!(
        run_result(
            "    Dim o As Object\n    r = CStr(VarType(o)) & \":\" & TypeName(o) & \":\" & CStr(IsObject(o)) & \":\" & CStr(IsEmpty(o)) & \":\" & CStr(o Is Nothing)"
        ),
        s("9:Nothing:True:False:True")
    );
}

#[test]
fn empty_baseline_stays_empty() {
    assert_eq!(
        run_result(
            "    Dim v As Variant\n    r = CStr(VarType(v)) & \":\" & TypeName(v) & \":\" & CStr(IsObject(v)) & \":\" & CStr(IsEmpty(v))"
        ),
        s("0:Empty:False:True")
    );
}

#[test]
fn set_variant_to_nothing_stores_object_null() {
    assert_eq!(
        run_result(
            "    Dim v As Variant\n    Set v = Nothing\n    r = CStr(VarType(v)) & \":\" & TypeName(v) & \":\" & CStr(IsObject(v)) & \":\" & CStr(IsEmpty(v))"
        ),
        s("9:Nothing:True:False")
    );
}

#[test]
fn let_variant_to_nothing_raises_91_and_leaves_empty() {
    assert_eq!(
        run_result(
            "    Dim v As Variant\n    On Error Resume Next\n    v = Nothing\n    r = CStr(Err.Number) & \":\" & CStr(VarType(v)) & \":\" & TypeName(v) & \":\" & CStr(IsObject(v)) & \":\" & CStr(IsEmpty(v))"
        ),
        s("91:0:Empty:False:True")
    );
}

#[test]
fn unset_object_numeric_assignment_raises_91() {
    assert_eq!(
        run_result(
            "    Dim o As Object\n    Dim n As Long\n    On Error Resume Next\n    n = o\n    r = CStr(Err.Number) & \":\" & CStr(n)"
        ),
        s("91:0")
    );
}

#[test]
fn unset_object_arithmetic_raises_91() {
    assert_raises("    Dim o As Object\n    o = o + 1", 91);
}

#[test]
fn unset_object_numeric_comparison_raises_91() {
    assert_eq!(
        run_result(
            "    Dim o As Object\n    On Error Resume Next\n    If o = 1 Then r = \"bad\" Else r = \"ok\"\n    r = CStr(Err.Number) & \":\" & r"
        ),
        s("91:")
    );
}
