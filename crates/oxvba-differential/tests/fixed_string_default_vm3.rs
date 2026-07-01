//! vm3 fixed-length scalar strings should default to their space-filled length.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn first_value(source: &str) -> Canon {
    let outcome = run(Executor::Vm3, source);
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
fn local_fixed_length_string_defaults_to_spaces() {
    assert_eq!(
        first_value(
            "Public r As Variant\nSub Main()\n    Dim fixed As String * 3\n    r = CStr(Len(fixed)) & \":\" & fixed\nEnd Sub\n"
        ),
        s("3:   ")
    );
}

#[test]
fn module_fixed_length_string_defaults_to_spaces() {
    assert_eq!(
        first_value(
            "Public r As Variant\nPublic fixed As String * 4\nSub Main()\n    r = CStr(Len(fixed)) & \":\" & fixed\nEnd Sub\n"
        ),
        s("4:    ")
    );
}

#[test]
fn fixed_length_string_assignment_controls_still_pad_and_truncate() {
    assert_eq!(
        first_value(
            "Public r As Variant\nSub Main()\n    Dim fixed As String * 3\n    fixed = \"ab\"\n    r = CStr(Len(fixed)) & \":\" & fixed\nEnd Sub\n"
        ),
        s("3:ab ")
    );
    assert_eq!(
        first_value(
            "Public r As Variant\nSub Main()\n    Dim fixed As String * 3\n    fixed = \"abcd\"\n    r = CStr(Len(fixed)) & \":\" & fixed\nEnd Sub\n"
        ),
        s("3:abc")
    );
}
