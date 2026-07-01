//! vm3 `LSet`/`RSet` string alignment, backed by
//! `docs/evidence/conformance/vm3_lset_rset_oracle_20260701T215755Z/`.

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
