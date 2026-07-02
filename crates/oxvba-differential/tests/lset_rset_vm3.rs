//! vm3 `LSet`/`RSet` string alignment, backed by
//! `docs/evidence/conformance/vm3_lset_rset_oracle_20260701T215755Z/`
//! and UDT record overlays backed by
//! `docs/evidence/conformance/vm3_lset_rset_oracle_20260702T_bd57_udt/`.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn first_value(source: &str) -> Canon {
    let outcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}\nsource:\n{source}",
        outcome.unsupported
    );
    let snap = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 run failed: {err}\nsource:\n{source}"));
    snap.first().cloned().expect("snapshot slot")
}

fn aligned(body: &str) -> Canon {
    first_value(&format!("Public r As Variant\nSub Main()\n{body}End Sub\n"))
}

fn s(text: &str) -> Canon {
    canon(&Variant::from_string(text.to_string()))
}

fn bind_rejection(source: &str, fragment: &str) {
    let outcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}\nsource:\n{source}",
        outcome.unsupported
    );
    let err = outcome.result.expect_err("expected bind/compile rejection");
    assert!(
        err.contains(fragment),
        "expected error to contain `{fragment}`, got {err:?}\nsource:\n{source}"
    );
}

#[test]
fn lset_rset_align_fixed_length_strings() {
    assert_eq!(
        aligned(
            "    Dim t As String * 5\n    LSet t = \"ab\"\n    r = CStr(Len(t)) & \":|\" & t & \"|\"\n"
        ),
        s("5:|ab   |")
    );
    assert_eq!(
        aligned(
            "    Dim t As String * 5\n    RSet t = \"ab\"\n    r = CStr(Len(t)) & \":|\" & t & \"|\"\n"
        ),
        s("5:|   ab|")
    );
}

#[test]
fn lset_rset_truncate_from_the_right_of_the_source() {
    assert_eq!(
        aligned(
            "    Dim t As String * 5\n    LSet t = \"abcdef\"\n    r = CStr(Len(t)) & \":|\" & t & \"|\"\n"
        ),
        s("5:|abcde|")
    );
    assert_eq!(
        aligned(
            "    Dim t As String * 5\n    RSet t = \"abcdef\"\n    r = CStr(Len(t)) & \":|\" & t & \"|\"\n"
        ),
        s("5:|abcde|")
    );
}

#[test]
fn lset_rset_use_current_width_for_variable_length_strings() {
    assert_eq!(
        aligned(
            "    Dim t As String\n    t = \".....\"\n    LSet t = \"ab\"\n    r = CStr(Len(t)) & \":|\" & t & \"|\"\n"
        ),
        s("5:|ab   |")
    );
    assert_eq!(
        aligned(
            "    Dim t As String\n    t = \".....\"\n    RSet t = \"ab\"\n    r = CStr(Len(t)) & \":|\" & t & \"|\"\n"
        ),
        s("5:|   ab|")
    );
    assert_eq!(
        aligned(
            "    Dim t As String\n    LSet t = \"ab\"\n    r = CStr(Len(t)) & \":|\" & t & \"|\"\n"
        ),
        s("0:||")
    );
    assert_eq!(
        aligned(
            "    Dim t As String\n    RSet t = \"ab\"\n    r = CStr(Len(t)) & \":|\" & t & \"|\"\n"
        ),
        s("0:||")
    );
    assert_eq!(
        aligned(
            "    Dim t As String\n    t = \"...\"\n    RSet t = \"abcdef\"\n    r = CStr(Len(t)) & \":|\" & t & \"|\"\n"
        ),
        s("3:|abc|")
    );
}

#[test]
fn rset_coerces_numeric_rhs_before_alignment() {
    assert_eq!(
        aligned(
            "    Dim t As String * 5\n    RSet t = 42\n    r = CStr(Len(t)) & \":|\" & t & \"|\"\n"
        ),
        s("5:|   42|")
    );
}

#[test]
fn lset_rset_null_rhs_raises_invalid_use_of_null() {
    for stmt in ["LSet", "RSet"] {
        let source = format!("Sub Main()\n    Dim t As String * 5\n    {stmt} t = Null\nEnd Sub\n");
        let outcome = run(Executor::Vm3, &source);
        assert!(
            outcome.unsupported.is_none(),
            "unsupported: {:?}\nsource:\n{source}",
            outcome.unsupported
        );
        assert!(outcome.raised, "expected raised error 94, got {outcome:?}");
        assert_eq!(outcome.err.number, 94);
    }
}

#[test]
fn lset_udt_record_copy_overlays_byte_storage() {
    assert_eq!(
        first_value(
            "Private Type A\n    X As String * 2\n    N As Integer\nEnd Type\n\
             Private Type B\n    X As String * 2\n    N As Integer\nEnd Type\n\
             Public r As Variant\n\
             Sub Main()\n    Dim a As A\n    Dim b As B\n    b.X = \"xy\"\n    b.N = 513\n    LSet a = b\n    r = \"|\" & a.X & \"|:\" & CStr(a.N)\nEnd Sub\n"
        ),
        s("|xy|:513")
    );
    assert_eq!(
        first_value(
            "Private Type A\n    I As Integer\n    B1 As Byte\n    B2 As Byte\nEnd Type\n\
             Private Type B\n    L As Long\nEnd Type\n\
             Public r As Variant\n\
             Sub Main()\n    Dim a As A\n    Dim b As B\n    b.L = &H4030201\n    LSet a = b\n    r = CStr(a.I) & \":\" & CStr(a.B1) & \":\" & CStr(a.B2)\nEnd Sub\n"
        ),
        s("513:3:4")
    );
}

#[test]
fn lset_udt_record_copy_truncates_or_preserves_target_tail_by_size() {
    assert_eq!(
        first_value(
            "Private Type A\n    X As String * 4\nEnd Type\n\
             Private Type B\n    X As String * 2\nEnd Type\n\
             Public r As Variant\n\
             Sub Main()\n    Dim a As A\n    Dim b As B\n    a.X = \"zzzz\"\n    b.X = \"xy\"\n    LSet a = b\n    r = a.X\nEnd Sub\n"
        ),
        s("xyzz")
    );
    assert_eq!(
        first_value(
            "Private Type A\n    X As String * 2\nEnd Type\n\
             Private Type B\n    X As String * 4\nEnd Type\n\
             Public r As Variant\n\
             Sub Main()\n    Dim a As A\n    Dim b As B\n    a.X = \"zz\"\n    b.X = \"wxyz\"\n    LSet a = b\n    r = a.X\nEnd Sub\n"
        ),
        s("wx")
    );
}

#[test]
fn lset_udt_record_copy_overlays_fixed_arrays() {
    assert_eq!(
        first_value(
            "Private Type A\n    B(0 To 3) As Byte\nEnd Type\n\
             Private Type B\n    L As Long\nEnd Type\n\
             Public r As Variant\n\
             Sub Main()\n    Dim a As A\n    Dim b As B\n    b.L = &H4030201\n    LSet a = b\n    r = CStr(a.B(0)) & \":\" & CStr(a.B(1)) & \":\" & CStr(a.B(2)) & \":\" & CStr(a.B(3))\nEnd Sub\n"
        ),
        s("1:2:3:4")
    );
}

#[test]
fn lset_udt_rejects_nonrecord_rset_and_owning_fields_like_vba() {
    bind_rejection(
        "Private Type A\n    X As String * 2\nEnd Type\n\
         Public r As Variant\n\
         Sub Main()\n    Dim a As A\n    LSet a = \"xy\"\n    r = a.X\nEnd Sub\n",
        "Type mismatch",
    );
    bind_rejection(
        "Private Type A\n    X As String * 2\nEnd Type\n\
         Private Type B\n    X As String * 2\nEnd Type\n\
         Public r As Variant\n\
         Sub Main()\n    Dim a As A\n    Dim b As B\n    RSet a = b\n    r = a.X\nEnd Sub\n",
        "RSet allowed only on strings",
    );
    bind_rejection(
        "Private Type A\n    S As String\nEnd Type\n\
         Private Type B\n    S As String\nEnd Type\n\
         Public r As Variant\n\
         Sub Main()\n    Dim a As A\n    Dim b As B\n    LSet a = b\n    r = a.S\nEnd Sub\n",
        "Type mismatch",
    );
}
