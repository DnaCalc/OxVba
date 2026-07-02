//! vm3 `LeftB`/`RightB` byte-string behavior, including odd-byte BSTR results.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn value(body: &str) -> Canon {
    let source = format!("Public r As Variant\nSub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}\n{source}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 run failed: {err}\n{source}"));
    values.first().cloned().expect("global r")
}

#[test]
fn leftb_rightb_match_vba_byte_slice_observables() {
    assert_eq!(
        value(
            "    Dim s As String\n\
             s = \"ABC\"\n\
             r = \"L0=\" & CStr(Len(LeftB(s, 0))) & \":\" & CStr(LenB(LeftB(s, 0)))\n\
             r = r & \";L1=\" & CStr(Len(LeftB(s, 1))) & \":\" & CStr(LenB(LeftB(s, 1)))\n\
             r = r & \";L3=\" & CStr(Len(LeftB(s, 3))) & \":\" & CStr(LenB(LeftB(s, 3))) & \":\" & CStr(AscW(LeftB(s, 3)))\n\
             r = r & \";L99=\" & CStr(Len(LeftB(s, 99))) & \":\" & CStr(LenB(LeftB(s, 99)))\n\
             r = r & \";R1=\" & CStr(Len(RightB(s, 1))) & \":\" & CStr(LenB(RightB(s, 1)))\n\
             r = r & \";R3=\" & CStr(Len(RightB(s, 3))) & \":\" & CStr(LenB(RightB(s, 3))) & \":\" & CStr(AscW(RightB(s, 3)))\n\
             r = r & \";R99=\" & CStr(Len(RightB(s, 99))) & \":\" & CStr(LenB(RightB(s, 99)))"
        ),
        canon(&Variant::from_string(
            "L0=0:0;L1=0:1;L3=1:3:65;L99=3:6;R1=0:1;R3=1:3:17152;R99=3:6"
        ))
    );
}

#[test]
fn leftb_rightb_propagate_null_but_aliases_raise_94() {
    assert_eq!(
        value(
            "    Dim n As Variant\n\
             n = Null\n\
             r = CStr(IsNull(LeftB(n, 2))) & \":\" & CStr(IsNull(RightB(n, 2)))\n\
             On Error Resume Next\n\
             Dim s As String\n\
             s = LeftB$(n, 2)\n\
             r = r & \":\" & CStr(Err.Number)\n\
             Err.Clear\n\
             s = RightB$(n, 2)\n\
             r = r & \":\" & CStr(Err.Number)"
        ),
        canon(&Variant::from_string("True:True:94:94"))
    );
}

#[test]
fn leftb_rightb_negative_count_raises_error_5() {
    assert_eq!(
        value(
            "    Dim s As String\n\
             s = \"ABC\"\n\
             On Error Resume Next\n\
             Dim t As String\n\
             t = LeftB(s, -1)\n\
             r = CStr(Err.Number)\n\
             Err.Clear\n\
             t = RightB(s, -1)\n\
             r = r & \":\" & CStr(Err.Number)"
        ),
        canon(&Variant::from_string("5:5"))
    );
}
