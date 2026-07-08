//! Regression guard for the remaining bd-us4v shape: a UDT fixed-array field
//! (`rec.arr(i)`) must not materialize the whole inline field array per element.
//!
//! The old lowering used `RecordGet` + `ArrayGet`; `RecordGet` constructed a
//! temporary SAFEARRAY from every inline element, making a loop over the field O(N^2).
//! `RecordArrayGet`/`RecordArraySet` read and write the selected inline element
//! directly.

use std::time::{Duration, Instant};

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn s(text: &str) -> Canon {
    canon(&Variant::from_string(text.to_string()))
}

fn assert_snapshot_contains(source: &str, expected: Canon) {
    let outcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}\nsource:\n{source}",
        outcome.unsupported
    );
    let snap = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 run failed: {err}\nsource:\n{source}"));
    assert!(
        snap.contains(&expected),
        "expected {expected:?} in snapshot {snap:?}\nsource:\n{source}"
    );
}

#[test]
fn udt_fixed_array_field_preserves_explicit_lower_bound() {
    assert_snapshot_contains(
        "Private Type State\n    Buses(1 To 2) As Long\nEnd Type\n\
         Public r As Variant\n\
         Sub Main()\n    Dim s As State\n    s.Buses(1) = 11\n    s.Buses(2) = 22\n    r = CStr(LBound(s.Buses)) & \":\" & CStr(UBound(s.Buses)) & \":\" & CStr(s.Buses(1)) & \":\" & CStr(s.Buses(2))\nEnd Sub\n",
        s("1:2:11:22"),
    );
}

#[test]
fn udt_fixed_array_field_single_bound_follows_option_base_one() {
    assert_snapshot_contains(
        "Option Base 1\n\
         Private Type State\n    Buses(2) As Long\nEnd Type\n\
         Public r As Variant\n\
         Sub Main()\n    Dim s As State\n    s.Buses(1) = 11\n    s.Buses(2) = 22\n    r = CStr(LBound(s.Buses)) & \":\" & CStr(UBound(s.Buses)) & \":\" & CStr(s.Buses(1)) & \":\" & CStr(s.Buses(2))\nEnd Sub\n",
        s("1:2:11:22"),
    );
}

#[test]
fn udt_fixed_array_field_preserves_negative_and_multidimensional_bounds() {
    assert_snapshot_contains(
        "Private Type State\n    Buses(-2 To 0) As Long\nEnd Type\n\
         Public r As Variant\n\
         Sub Main()\n    Dim s As State\n    s.Buses(-2) = 7\n    s.Buses(0) = 9\n    r = CStr(LBound(s.Buses)) & \":\" & CStr(UBound(s.Buses)) & \":\" & CStr(s.Buses(-2)) & \":\" & CStr(s.Buses(0))\nEnd Sub\n",
        s("-2:0:7:9"),
    );
    assert_snapshot_contains(
        "Private Type State\n    Grid(1 To 2, 3 To 4) As Long\nEnd Type\n\
         Public r As Variant\n\
         Sub Main()\n    Dim s As State\n    s.Grid(1, 3) = 13\n    s.Grid(2, 4) = 24\n    r = CStr(LBound(s.Grid, 1)) & \":\" & CStr(UBound(s.Grid, 1)) & \":\" & CStr(LBound(s.Grid, 2)) & \":\" & CStr(UBound(s.Grid, 2)) & \":\" & CStr(s.Grid(1, 3)) & \":\" & CStr(s.Grid(2, 4))\nEnd Sub\n",
        s("1:2:3:4:13:24"),
    );
}

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
