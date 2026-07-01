//! vm3 `Null` coercion should surface VBA error 94 (`Invalid use of Null`) rather
//! than a generic Type mismatch 13.

use oxvba_differential::{Executor, run};

fn error_number(body: &str) -> i32 {
    let source = format!("Sub Main()\n    Dim n As Variant\n    n = Null\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.raised,
        "expected a VBA error, got {:?}\n{source}",
        outcome.result
    );
    outcome.err.number
}

#[test]
fn explicit_numeric_conversions_of_null_raise_94() {
    for expr in [
        "CBool(n)",
        "CByte(n)",
        "CInt(n)",
        "CLng(n)",
        "CLngLng(n)",
        "CLngPtr(n)",
        "CSng(n)",
        "CDbl(n)",
        "CCur(n)",
        "CDec(n)",
        "CDate(n)",
    ] {
        assert_eq!(
            error_number(&format!("    Dim v As Variant\n    v = {expr}")),
            94,
            "{expr}"
        );
    }
}

#[test]
fn implicit_scalar_assignment_of_null_raises_94() {
    for (decl, assign) in [
        ("Boolean", "b"),
        ("Byte", "b"),
        ("Integer", "i"),
        ("Long", "l"),
        ("LongLong", "ll"),
        ("LongPtr", "lp"),
        ("Single", "s"),
        ("Double", "d"),
        ("Currency", "c"),
        ("Date", "dt"),
    ] {
        assert_eq!(
            error_number(&format!("    Dim {assign} As {decl}\n    {assign} = n")),
            94,
            "{decl}"
        );
    }
}
