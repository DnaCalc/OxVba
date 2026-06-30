//! Regression guard for **bd-us4v**: dynamic-array element access must be O(1), so a
//! loop over a module-level dynamic array is O(N), not O(N²).
//!
//! The defect (reported from OxForms,
//! `docs/handovers/HANDOVER_OxVba_vm3_dynamic_array_access_perf.md`): reading/writing
//! `arr(i)` deep-cloned the WHOLE SAFEARRAY backing store per access (the `Variant`
//! clone in the index path), so a single pass over N elements was O(N²). At N=400 a
//! pass cost ~2.2 s; native is ~µs. The fix reads/writes one element directly through
//! the descriptor (`array_get_fast`/`array_set_fast`).
//!
//! This test fills then reads a 2000-element module-level dynamic `Long` array. Under
//! the O(N²) defect that is tens of seconds; under the O(N) fix it is milliseconds.
//! The 5 s ceiling has a ~100× margin over the fixed cost, so it fails loudly on a
//! regression and never flakes on a healthy run. It also pins the computed sum, so it
//! is a correctness test for the fast paths as well as a perf guard.

use std::time::{Duration, Instant};

use oxvba_differential::{Executor, canon, run};
use oxvba_runtime::Variant;

#[test]
fn dynamic_array_loop_is_linear_not_quadratic() {
    let n = 2000;
    let source = format!(
        "Public a() As Long\nPublic total As Long\n\
         Sub Main()\n\
         \u{20}   Dim i As Long\n\
         \u{20}   ReDim a(0 To {n} - 1)\n\
         \u{20}   For i = 0 To {n} - 1\n\
         \u{20}       a(i) = i\n\
         \u{20}   Next i\n\
         \u{20}   total = 0\n\
         \u{20}   For i = 0 To {n} - 1\n\
         \u{20}       total = total + a(i)\n\
         \u{20}   Next i\n\
         End Sub\n",
    );

    let start = Instant::now();
    let outcome = run(Executor::Vm3, &source);
    let elapsed = start.elapsed();

    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome
        .result
        .unwrap_or_else(|e| panic!("vm3 run failed: {e}"));
    // sum(0 .. n-1) = (n-1)*n/2 = 1999 * 2000 / 2 = 1_999_000
    let expected = 1_999_000i32;
    assert!(
        snap.contains(&canon(&Variant::from_i32(expected))),
        "expected total={expected} in snapshot {snap:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "fill+read of a {n}-element dynamic array took {elapsed:?}; the O(1) element-access \
         fix should make this milliseconds — a multi-second time means element access regressed \
         to O(N) (the O(N²)-loop defect bd-us4v)"
    );
}
