//! vm3 `Err.Number`, `Err.Description`, and `Err.Source` writes should match
//! Excel/VBA 7.1. Live oracle evidence:
//! `docs/evidence/conformance/vm3_err_property_writes_oracle_20260701T1442Z/`.

use oxvba_differential::{Canon, Executor, run, run_modules};
use oxvba_symbol::manifest::ModuleKind;

fn string_result(body: &str) -> String {
    let source = format!("Public result As String\nSub Main()\n{body}\nEnd Sub\n");
    let modules = [("Main", ModuleKind::Procedural, source.as_str())];
    let outcome = run_modules(Executor::Vm3, &modules, "VBAProject");
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}\n{source}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("Err property write probe failed: {err}\n{source}"));
    match values.first() {
        Some(Canon::Str(value)) => value.clone(),
        other => panic!("expected string result, got {other:?} from {values:?}"),
    }
}

fn bind_error(source: &str) -> String {
    let outcome = run(Executor::Vm3, source);
    assert!(outcome.result.is_err(), "expected bind/runtime error for {source}");
    outcome.result.err().unwrap()
}

#[test]
fn err_number_description_source_writes_are_observable() {
    assert_eq!(
        string_result(
            r#"    On Error Resume Next
    Err.Clear
    Err.Number = 6
    Dim a As String
    a = CStr(Err.Number) & ":" & Err.Description & ":" & Err.Source
    Err.Description = "custom description"
    Err.Source = "custom source"
    Dim b As String
    b = CStr(Err.Number) & ":" & Err.Description & ":" & Err.Source
    Err.Raise 7
    Dim c As String
    c = CStr(Err.Number) & ":" & Err.Description & ":" & Err.Source
    Err.Clear
    Err.Description = "desc while clear"
    Err.Source = "source while clear"
    Err.Raise 8
    Dim d As String
    d = CStr(Err.Number) & ":" & Err.Description & ":" & Err.Source
    result = a & "|" & b & "|" & c & "|" & d"#
        ),
        "6::|6:custom description:custom source|7:custom description:custom source|8:desc while clear:source while clear"
    );
}

#[test]
fn err_number_zero_write_preserves_existing_inheritable_fields_but_does_not_create_them() {
    assert_eq!(
        string_result(
            r#"    On Error Resume Next
    Err.Raise 5, "src5", "desc5"
    Err.Number = 0
    Err.Raise 8
    Dim a As String
    a = CStr(Err.Number) & ":" & Err.Description & ":" & Err.Source
    Err.Clear
    Err.Number = 0
    Err.Raise 9
    Dim b As String
    b = CStr(Err.Number) & ":" & Err.Description & ":" & Err.Source
    Err.Clear
    Err.Number = 6
    Err.Raise 7
    Dim c As String
    c = CStr(Err.Number) & ":" & Err.Description & ":" & Err.Source
    result = a & "|" & b & "|" & c"#
        ),
        "8:desc5:src5|9:Subscript out of range:VBAProject|7:Out of memory:VBAProject"
    );
}

#[test]
fn err_lastdllerror_assignment_is_rejected() {
    let err = bind_error(
        "Sub Main()\n    Err.LastDllError = 123\nEnd Sub\n",
    );
    assert!(
        err.contains("Err.LastDllError is read-only"),
        "unexpected error: {err}"
    );
}
