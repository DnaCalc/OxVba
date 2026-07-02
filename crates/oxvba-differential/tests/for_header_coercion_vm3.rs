//! vm3 `For` headers coerce start/end/step expressions to the declared counter type.
//!
//! Ordinary assignments already coerce values stored in declared scalar variables.
//! `For i = start To limit Step step` must do the same one-time header coercion,
//! otherwise string-valued numeric bounds or fractional Integer headers reach the
//! VM as raw Variants and diverge from VBA loop semantics.

use oxvba_differential::{Canon, Executor, RunOutcome, canon, run};
use oxvba_runtime::Variant;

fn run_main(decls: &str, body: &str) -> RunOutcome {
    let source = format!("Public r As Variant\nSub Main()\n{decls}{body}End Sub\n");
    run(Executor::Vm3, &source)
}

fn assert_first(decls: &str, body: &str, expected: &Canon) {
    let outcome = run_main(decls, body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"));
    assert_eq!(snap.first(), Some(expected), "{snap:?}");
}

#[test]
fn long_counter_coerces_string_bound_and_default_step() {
    assert_first(
        "    Dim i As Long\n    Dim total As Long\n",
        "    For i = 1 To \"3\"\n        total = total + i\n    Next i\n    r = total\n",
        &canon(&Variant::from_i32(6)),
    );
}

#[test]
fn long_counter_coerces_string_start_limit_and_step() {
    assert_first(
        "    Dim i As Long\n    Dim total As Long\n",
        "    For i = \"1\" To \"5\" Step \"2\"\n        total = total + i\n    Next i\n    r = total\n",
        &canon(&Variant::from_i32(9)),
    );
}

#[test]
fn integer_counter_coerces_fractional_header_once() {
    assert_first(
        "    Dim i As Integer\n    Dim seen As String\n",
        "    For i = 1.6 To 5.4 Step 2.1\n        seen = seen & CStr(i) & \",\"\n    Next i\n    r = seen\n",
        &canon(&Variant::from_string("2,4,")),
    );
}
