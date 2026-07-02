//! vm3 `For` counter increment overflows for a fixed-integer counter — verified
//! against live Office VBA 7.1.
//!
//! `For i As Integer = 32766 To 32767` runs the body for 32766 and 32767, then the
//! increment to 32768 raises Overflow (6). Long and Byte counters behave the same
//! at their bounds. A `Variant` counter instead PROMOTES (Integer→Long) and does
//! not overflow. Previously the increment always widened, so a fixed counter
//! silently promoted too. Closes `for-counter-no-overflow`.

use oxvba_differential::{Canon, Executor, RunOutcome, canon, run};
use oxvba_runtime::Variant;

fn run_main(decls: &str, body: &str) -> RunOutcome {
    let source = format!("Public r As Variant\nSub Main()\n{decls}{body}End Sub\n");
    run(Executor::Vm3, &source)
}

fn assert_overflow(decls: &str, body: &str) {
    let outcome = run_main(decls, body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.result.is_err(),
        "expected Overflow 6, completed: {:?}",
        outcome.result
    );
    assert_eq!(outcome.err.number, 6, "err={:?}", outcome.err);
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
fn integer_counter_overflows_at_max() {
    // Body runs for 32766 and 32767; the increment to 32768 overflows.
    assert_overflow(
        "    Dim i As Integer\n    Dim n As Long\n",
        "    For i = 32766 To 32767\n        n = n + 1\n    Next i\n",
    );
}

#[test]
fn long_counter_overflows_at_max() {
    assert_overflow(
        "    Dim i As Long\n    Dim n As Long\n",
        "    For i = 2147483646 To 2147483647\n        n = n + 1\n    Next i\n",
    );
}

#[test]
fn byte_counter_overflows_at_max() {
    assert_overflow(
        "    Dim b As Byte\n    Dim n As Long\n",
        "    For b = 254 To 255\n        n = n + 1\n    Next b\n",
    );
}

#[test]
fn variant_counter_promotes_without_overflow() {
    // A Variant counter promotes Integer→Long at the increment; no error, and the
    // loop runs exactly twice.
    assert_first(
        "    Dim k As Variant\n    Dim n As Long\n",
        "    For k = 32766 To 32767\n        n = n + 1\n    Next k\n    r = n\n",
        &canon(&Variant::from_i32(2)),
    );
}

#[test]
fn integer_counter_in_range_runs_normally() {
    // A fixed counter that never reaches its max iterates normally (Checked mode
    // only errors on actual overflow).
    assert_first(
        "    Dim i As Integer\n    Dim n As Long\n",
        "    For i = 1 To 5\n        n = n + 1\n    Next i\n    r = n\n",
        &canon(&Variant::from_i32(5)),
    );
}
