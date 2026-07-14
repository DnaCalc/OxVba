//! vm3 array subscripts and `ReDim` bounds are VBA `Long`s: a value outside
//! `Long` range must raise Overflow (6), not silently wrap through `as i32`
//! (which used to read/allocate the wrong element), and an over-large `ReDim`
//! must raise catchable Out of memory (7) rather than aborting the host.

use oxvba_differential::{Executor, run};

fn error_number(body: &str) -> i32 {
    let source = format!("Public r As Variant\nSub Main()\n{body}End Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.raised,
        "expected a VBA error, got {:?}",
        outcome.result
    );
    outcome.err.number
}

fn ok_slot0(body: &str) -> oxvba_differential::Canon {
    let source = format!("Public r As Variant\nSub Main()\n{body}End Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    outcome
        .result
        .unwrap_or_else(|e| panic!("run failed: {e}"))
        .first()
        .cloned()
        .expect("snapshot slot")
}

#[test]
fn redim_bound_beyond_long_range_is_overflow_6() {
    // 2^31 (just past Long max 2147483647): used to truncate to a tiny/negative
    // bound and silently allocate the wrong size.
    assert_eq!(error_number("    Dim a() As Long\n    ReDim a(2147483648#)\n"), 6);
}

#[test]
fn array_subscript_beyond_long_range_is_overflow_6() {
    // 2^32 used to wrap to offset 0 and read a(0).
    assert_eq!(
        error_number("    Dim a(0 To 10) As Long\n    r = a(4294967296#)\n"),
        6
    );
    // 2^32 + 3 used to wrap to offset 3 and read a(3) — the classic silent-wrong.
    assert_eq!(
        error_number("    Dim a(0 To 10) As Long\n    r = a(4294967299#)\n"),
        6
    );
}

#[test]
fn in_range_out_of_bounds_subscript_still_raises_9() {
    // A Long-range index outside the declared bounds is still error 9, not 6 —
    // the overflow check must not swallow the ordinary subscript check.
    assert_eq!(error_number("    Dim a(0 To 10) As Long\n    r = a(20)\n"), 9);
}

#[test]
fn ordinary_arrays_still_work() {
    // Regression guard: valid bounds/subscripts are unaffected.
    assert_eq!(
        ok_slot0("    Dim a(0 To 5) As Long\n    a(3) = 42\n    r = a(3)\n"),
        oxvba_differential::canon(&oxvba_runtime::Variant::from_i32(42))
    );
}

#[test]
fn oversized_redim_raises_out_of_memory_not_host_abort() {
    // `ReDim v(0 To 2000000000)` of a Variant element is an in-Long-range but
    // ~48 GB allocation. It must raise catchable error 7 (VBA "Out of memory"),
    // never abort the host via an infallible allocation. (Deterministic on
    // no-overcommit platforms such as Windows; the mechanism itself is unit
    // tested in oxvba-vm3 `try_build_default_elements_reports_oom_not_abort`.)
    assert_eq!(
        error_number("    Dim v() As Variant\n    ReDim v(0 To 2000000000)\n"),
        7
    );
}
