//! vm3 renders a `Single` to string at `Single` precision (~7 significant
//! digits), not the widened `f64`. `CStr`/`Str`/`&` used to format
//! `f64::from(single)`, leaking the f32's imprecision as spurious digits
//! (`CStr(CSng(0.1))` -> "0.10000000149011612").

use oxvba_differential::{Executor, canon, run};
use oxvba_runtime::Variant;

fn assert_render(val: &str, render: &str, expected: &str) {
    let body = format!(
        "Public result As String\nSub Main()\n    Dim x As Single\n    x = {val}\n    result = {render}\nEnd Sub\n"
    );
    let outcome = run(Executor::Vm3, &body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|e| panic!("run failed for {val}/{render}: {e}"));
    assert!(
        values.contains(&canon(&Variant::from_string(expected.to_string()))),
        "{render} with x={val} = {values:?}, expected to contain {expected:?}"
    );
}

#[test]
fn cstr_of_single_has_no_spurious_precision() {
    assert_render("0.1", "CStr(x)", "0.1");
    assert_render("5", "CStr(x)", "5");
    assert_render("5.5", "CStr(x)", "5.5");
    assert_render("-0.1", "CStr(x)", "-0.1");
    assert_render("123.456", "CStr(x)", "123.456");
}

#[test]
fn cstr_of_single_uses_seven_significant_digits() {
    // 1/3 as a Single is 0.33333334 (f32 shortest round-trip); VBA shows the
    // 7 significant digits of Single precision.
    assert_render("1.0 / 3.0", "CStr(x)", "0.3333333");
}

#[test]
fn concatenation_of_single_matches_cstr() {
    assert_render("0.1", "\"\" & x", "0.1");
}
