//! vm3 `VarType`/`TypeName` should report a SAFEARRAY's element type.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn value(body: &str) -> Canon {
    let source = format!("Public r As Variant\nSub Main()\n{body}End Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"));
    snap.first().cloned().expect("snapshot slot")
}

fn s(text: &str) -> Canon {
    canon(&Variant::from_string(text.to_string()))
}

#[test]
fn vartype_reports_typed_array_element_vartype() {
    assert_eq!(
        value("    Dim a(1 To 2) As Integer\n    r = VarType(a)\n"),
        canon(&Variant::from_i32(8194))
    );
    assert_eq!(
        value("    Dim a(1 To 2) As Long\n    r = VarType(a)\n"),
        canon(&Variant::from_i32(8195))
    );
    assert_eq!(
        value("    Dim a(1 To 2) As String\n    r = VarType(a)\n"),
        canon(&Variant::from_i32(8200))
    );
}

#[test]
fn typename_reports_typed_array_element_name() {
    assert_eq!(
        value("    Dim a(1 To 2) As Integer\n    r = TypeName(a)\n"),
        s("Integer()")
    );
    assert_eq!(
        value("    Dim a(1 To 2) As Long\n    r = TypeName(a)\n"),
        s("Long()")
    );
    assert_eq!(
        value("    Dim a(1 To 2) As String\n    r = TypeName(a)\n"),
        s("String()")
    );
}

#[test]
fn variant_array_control_stays_variant_array() {
    assert_eq!(
        value("    Dim a As Variant\n    a = Array(1, 2)\n    r = VarType(a)\n"),
        canon(&Variant::from_i32(8204))
    );
    assert_eq!(
        value("    Dim a As Variant\n    a = Array(1, 2)\n    r = TypeName(a)\n"),
        s("Variant()")
    );
}
