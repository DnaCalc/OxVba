//! Checked Single/Double add/sub/mul must overflow to VBA error 6 in the JIT
//! exactly as in vm3. The JIT's f32/f64 fast paths used raw fadd/fsub/fmul and
//! stored ±Inf silently; vm3 rejects a non-finite result (error 6).

use oxvba_differential::{Executor, run};

fn assert_jit_matches_vm3(source: &str) {
    let vm3 = run(Executor::Vm3, source);
    assert!(
        vm3.unsupported.is_none(),
        "vm3 declined: {:?}\n{source}",
        vm3.unsupported
    );
    let jit = run(Executor::Jit, source);
    assert!(
        jit.unsupported.is_none(),
        "jit declined: {:?}\n{source}",
        jit.unsupported
    );
    assert_eq!(jit.raised, vm3.raised, "raised mismatch\n{source}");
    assert_eq!(jit.err, vm3.err, "Err mismatch\n{source}");
    assert_eq!(jit.result, vm3.result, "snapshot mismatch\n{source}");
}

/// Both executors raise VBA Overflow (error 6) for `source`.
fn assert_both_overflow(source: &str) {
    for exec in [Executor::Vm3, Executor::Jit] {
        let outcome = run(exec, source);
        assert!(
            outcome.unsupported.is_none(),
            "{exec:?} declined: {:?}\n{source}",
            outcome.unsupported
        );
        assert!(outcome.raised, "{exec:?} did not raise\n{source}");
        assert_eq!(outcome.err.number, 6, "{exec:?} error != 6\n{source}");
    }
}

fn single(body: &str) -> String {
    format!("Public r As Variant\nSub Main()\n    Dim a As Single\n    Dim b As Single\n{body}    r = b\nEnd Sub\n")
}

fn double(body: &str) -> String {
    format!("Public r As Variant\nSub Main()\n    Dim a As Double\n    Dim b As Double\n{body}    r = b\nEnd Sub\n")
}

#[test]
fn jit_single_overflow_matches_vm3() {
    for src in [
        single("    a = 2E38\n    b = a + a\n"),
        single("    a = -2E38\n    b = a - 2E38\n"),
        single("    a = 2E19\n    b = a * a\n"),
    ] {
        assert_jit_matches_vm3(&src);
        assert_both_overflow(&src);
    }
}

#[test]
fn jit_double_overflow_matches_vm3() {
    // vm3 does NOT raise on Double overflow (it yields ±Inf); the JIT must match
    // that, not raise. (Whether VBA should raise here is a separate vm3 gap.)
    for src in [
        double("    a = 1E308\n    b = a + a\n"),
        double("    a = 1E308\n    b = a * 10\n"),
    ] {
        assert_jit_matches_vm3(&src);
        let outcome = run(Executor::Vm3, &src);
        assert!(!outcome.raised, "vm3 unexpectedly raised on Double overflow");
    }
}

#[test]
fn jit_finite_float_arithmetic_still_matches_vm3() {
    // Regression: non-overflowing arithmetic is unchanged and produces no error.
    assert_jit_matches_vm3(&single("    a = 1.5\n    b = a + 2.5\n"));
    assert_jit_matches_vm3(&single("    a = 100\n    b = a * 3\n"));
    assert_jit_matches_vm3(&double("    a = 1E100\n    b = a + a\n"));
    assert_jit_matches_vm3(&double("    a = 12.5\n    b = a - 4.5\n"));
}
