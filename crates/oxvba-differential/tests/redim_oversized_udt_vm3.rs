//! `ReDim`-ing an array of a UDT whose native record layout cannot be built
//! (here, a record above the 64 KiB size limit) must raise a recoverable VBA
//! error, not abort the host: `default_array_element` used to `expect()` the
//! layout/allocation and panic on ordinary guest-legal UDT arrays.

use oxvba_differential::{Executor, run};

const OVERSIZED_UDT: &str = "Type Big\n    data(1 To 10000) As Double\nEnd Type\n\
Public r As Variant\nSub Main()\n    Dim a() As Big\n    ReDim a(0 To 1)\nEnd Sub\n";

#[test]
fn redim_array_of_oversized_udt_raises_error_not_panic_vm3() {
    // 10000 Doubles = 80000 bytes > the 64 KiB record limit.
    let outcome = run(Executor::Vm3, OVERSIZED_UDT);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.raised,
        "expected a recoverable VBA error, got {:?}",
        outcome.result
    );
    // Recoverable type-mismatch (13), mirroring the scalar NewRecord path.
    assert_eq!(outcome.err.number, 13);
}

#[test]
fn redim_array_of_oversized_udt_does_not_abort_jit() {
    // The JIT filled elements via resize_with(default_array_element), which
    // panicked identically; it must now decline or fault, never abort.
    let outcome = run(Executor::Jit, OVERSIZED_UDT);
    // Either an honest unsupported decline or a seated VBA error is acceptable;
    // the point is the process survives (the test binary reaching here proves it).
    assert!(
        outcome.unsupported.is_some() || outcome.raised || outcome.result.is_ok(),
        "unexpected JIT outcome: {outcome:?}"
    );
}
