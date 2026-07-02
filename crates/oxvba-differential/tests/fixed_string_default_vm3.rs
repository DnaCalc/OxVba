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

fn value(source: &str) -> Canon {
    first_value(source)
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

#[test]
fn fixed_length_udt_field_defaults_to_nuls() {
    assert_eq!(
        value(
            "Private Type Person\n    Name As String * 5\nEnd Type\nPublic r As Variant\nSub Main()\n    Dim p As Person\n    r = CStr(Len(p.Name)) & \":|\" & p.Name & \"|:\" & CStr(Asc(Mid(p.Name, 1, 1)))\nEnd Sub\n"
        ),
        s("5:|\0\0\0\0\0|:0")
    );
}

#[test]
fn fixed_length_udt_field_assignment_pads_and_truncates() {
    assert_eq!(
        value(
            "Private Type Person\n    Name As String * 5\nEnd Type\nPublic r As Variant\nSub Main()\n    Dim p As Person\n    p.Name = \"ab\"\n    r = CStr(Len(p.Name)) & \":|\" & p.Name & \"|:\" & CStr(Asc(Mid(p.Name, 3, 1)))\nEnd Sub\n"
        ),
        s("5:|ab   |:32")
    );
    assert_eq!(
        value(
            "Private Type Person\n    Name As String * 5\nEnd Type\nPublic r As Variant\nSub Main()\n    Dim p As Person\n    p.Name = \"abcdef\"\n    r = CStr(Len(p.Name)) & \":|\" & p.Name & \"|\"\nEnd Sub\n"
        ),
        s("5:|abcde|")
    );
}

#[test]
fn fixed_length_udt_field_arrays_and_whole_copy_keep_field_width() {
    assert_eq!(
        value(
            "Private Type Person\n    Name As String * 5\nEnd Type\nPublic r As Variant\nSub Main()\n    Dim people(0 To 1) As Person\n    people(1).Name = \"xy\"\n    r = CStr(Len(people(0).Name)) & \":|\" & people(0).Name & \"|;\" & CStr(Len(people(1).Name)) & \":|\" & people(1).Name & \"|\"\nEnd Sub\n"
        ),
        s("5:|\0\0\0\0\0|;5:|xy   |")
    );
    assert_eq!(
        value(
            "Private Type Person\n    Name As String * 5\nEnd Type\nPublic r As Variant\nSub Main()\n    Dim p As Person\n    Dim q As Person\n    p.Name = \"ab\"\n    q = p\n    p.Name = \"zzzzz\"\n    r = CStr(Len(q.Name)) & \":|\" & q.Name & \"|\"\nEnd Sub\n"
        ),
        s("5:|ab   |")
    );
}

#[test]
fn fixed_length_udt_field_null_assignment_raises_94_and_preserves_nuls() {
    assert_eq!(
        value(
            "Private Type Person\n    Name As String * 5\nEnd Type\nPublic r As Variant\nSub Main()\n    On Error GoTo EH\n    Dim p As Person\n    p.Name = Null\n    r = \"ok:\" & CStr(Len(p.Name)) & \":|\" & p.Name & \"|\"\n    Exit Sub\nEH:\n    r = \"err:\" & CStr(Err.Number) & \":\" & Err.Description & \":|\" & p.Name & \"|\"\nEnd Sub\n"
        ),
        s("err:94:Invalid use of Null:|\0\0\0\0\0|")
    );
}

#[test]
fn fixed_length_udt_field_len_and_lenb_match_vba_record_sizes() {
    assert_eq!(
        value(
            "Private Type LayoutProbe\n    B As Byte\n    Name As String * 5\n    Tail As Integer\nEnd Type\nPublic r As Variant\nSub Main()\n    Dim p As LayoutProbe\n    r = CStr(Len(p)) & \":\" & CStr(LenB(p))\nEnd Sub\n"
        ),
        s("8:14")
    );
}
