//! Regression guard for the remaining bd-us4v shape: a UDT fixed-array field
//! (`rec.arr(i)`) must not materialize the whole inline field array per element.
//!
//! The old lowering used `RecordGet` + `ArrayGet`; `RecordGet` constructed a
//! temporary SAFEARRAY from every inline element, making a loop over the field O(N^2).
//! `RecordArrayGet`/`RecordArraySet` read and write the selected inline element
//! directly.

use std::time::{Duration, Instant};

use oxvba_differential::{Executor, canon, run};
use oxvba_runtime::Variant;

#[test]
fn udt_fixed_array_field_loop_is_linear_not_quadratic() {
    let n = 2000usize;
    let upper = n - 1;
    let source = format!(
        "Type T\n\
         \u{20}   arr(0 To {upper}) As Long\n\
         End Type\n\
         Public total As Long\n\
         Public first As Long\n\
         Public last As Long\n\
         Sub Main()\n\
         \u{20}   Dim rec As T\n\
         \u{20}   Dim i As Long\n\
         \u{20}   For i = 0 To {upper}\n\
         \u{20}       rec.arr(i) = i\n\
         \u{20}   Next i\n\
         \u{20}   For i = 0 To {upper}\n\
         \u{20}       total = total + rec.arr(i)\n\
         \u{20}   Next i\n\
         \u{20}   first = rec.arr(0)\n\
         \u{20}   last = rec.arr({upper})\n\
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
    let expected = ((n - 1) * n / 2) as i32;
    assert!(
        snap.contains(&canon(&Variant::from_i32(expected))),
        "expected total={expected} in snapshot {snap:?}"
    );
    assert!(
        snap.contains(&canon(&Variant::from_i32(0))),
        "first element missing: {snap:?}"
    );
    assert!(
        snap.contains(&canon(&Variant::from_i32(upper as i32))),
        "last element missing: {snap:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "fill+read of a {n}-element UDT fixed-array field took {elapsed:?}; \
         record-array element access must be O(1), not materialize the full field per access"
    );
}
