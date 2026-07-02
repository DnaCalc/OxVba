//! vm3 `IsArray` should match VBA for allocated, unallocated, erased, and Variant-held arrays.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn run_body(body: &str) -> oxvba_differential::RunOutcome {
    let source = format!("Public r As Variant\nSub Main()\n{body}End Sub\n");
    run(Executor::Vm3, &source)
}

fn value(body: &str) -> Canon {
    let outcome = run_body(body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(!outcome.raised, "unexpected error: {:?}", outcome.err);
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"));
    snap.first().cloned().expect("snapshot slot")
}

fn bool_value(value: bool) -> Canon {
    canon(&Variant::from_bool(value))
}

fn string_value(value: &str) -> Canon {
    canon(&Variant::from_string(value.to_string()))
}

#[test]
fn dynamic_arrays_are_arrays_before_and_after_allocation() {
    assert_eq!(
        value("    Dim a() As Long\n    r = IsArray(a)\n"),
        bool_value(true)
    );
    assert_eq!(
        value("    Dim a() As Long\n    ReDim a(1 To 3)\n    r = IsArray(a)\n"),
        bool_value(true)
    );
    assert_eq!(
        value("    Dim a() As Long\n    ReDim a(1 To 3)\n    Erase a\n    r = IsArray(a)\n"),
        bool_value(true)
    );
}

#[test]
fn fixed_arrays_remain_arrays_after_erase() {
    assert_eq!(
        value("    Dim a(1 To 3) As Long\n    r = IsArray(a)\n"),
        bool_value(true)
    );
    assert_eq!(
        value("    Dim a(1 To 3) As Long\n    Erase a\n    r = IsArray(a)\n"),
        bool_value(true)
    );
}

#[test]
fn variant_controls_match_vba_array_identity() {
    assert_eq!(
        value("    Dim v As Variant\n    r = IsArray(v)\n"),
        bool_value(false)
    );
    assert_eq!(
        value("    Dim v As Variant\n    v = Array(1, 2)\n    r = IsArray(v)\n"),
        bool_value(true)
    );
    assert_eq!(
        value("    Dim v As Variant\n    v = Array(1, 2)\n    Erase v\n    r = IsArray(v)\n"),
        bool_value(true)
    );
}

#[test]
fn variant_copy_preserves_unallocated_dynamic_array_identity() {
    assert_eq!(
        value(
            "    Dim a() As Long\n    Dim v As Variant\n    ReDim a(1 To 3)\n    v = a\n    r = IsArray(v)\n"
        ),
        bool_value(true)
    );
    assert_eq!(
        value("    Dim a() As Long\n    Dim v As Variant\n    v = a\n    r = IsArray(v)\n"),
        bool_value(true)
    );
}

#[test]
fn unallocated_array_introspection_preserves_element_type() {
    assert_eq!(
        value(
            "    Dim a() As Long\n    r = CStr(IsArray(a)) & \":\" & CStr(VarType(a)) & \":\" & TypeName(a)\n"
        ),
        string_value("True:8195:Long()")
    );
    assert_eq!(
        value(
            "    Dim a() As Long\n    ReDim a(1 To 3)\n    Erase a\n    r = CStr(IsArray(a)) & \":\" & CStr(VarType(a)) & \":\" & TypeName(a)\n"
        ),
        string_value("True:8195:Long()")
    );
    assert_eq!(
        value(
            "    Dim a() As Long\n    Dim v As Variant\n    v = a\n    r = CStr(IsArray(v)) & \":\" & CStr(VarType(v)) & \":\" & TypeName(v)\n"
        ),
        string_value("True:8195:Long()")
    );
    assert_eq!(
        value(
            "    Dim v As Variant\n    v = Array(1, 2)\n    Erase v\n    r = CStr(IsArray(v)) & \":\" & CStr(VarType(v)) & \":\" & TypeName(v)\n"
        ),
        string_value("True:8204:Variant()")
    );
}
