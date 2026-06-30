//! Regression guard for **bd-us4v** (residual): element access of an array held as a
//! CLASS-INSTANCE FIELD (`Private m()` in a `.cls`) or a UDT MEMBER (`r.items()`) must be
//! O(1), so a loop over such an array is O(N), not O(N²).
//!
//! The residual (reported from OxForms,
//! `docs/handovers/HANDOVER_OxVba_vm3_dynamic_array_access_perf.md`): the first fix made
//! `arr(i)` O(1) only for arrays in ordinary SLOTS. A field-held array was reached via a
//! `FieldGet`/`RecordGet` that deep-cloned the WHOLE field array on EVERY loop iteration,
//! so a class-field loop stayed O(N²) — the OxForms hit-test workload. The fix fuses the
//! field-load and the index into `ArrayGetField`/`ArraySetField` (object fields) and
//! `ArrayGetRecordField`/`ArraySetRecordField` (UDT members), borrowing the field's array
//! in place and reading/writing one element through the SAFEARRAY descriptor — no clone.
//!
//! Each test fills then reads a 2000-element field array. Under the O(N²) residual that is
//! several seconds; under the O(1) fix it is tens of milliseconds (measured ~57 ms). The
//! sub-second ceiling sits ~16× over the healthy cost and ~9× under the broken cost, so it
//! fails loudly on a regression and does not flake on a healthy run. Each also pins the
//! computed sum, so it is a correctness test for the in-place field/member element access
//! (incl. write-through) as well as a perf guard.

use std::time::{Duration, Instant};

use oxvba_differential::{canon, run, run_modules, Executor};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind;

/// N elements, filled `m(i) = i` then summed: sum(0 .. N-1) = (N-1)*N/2.
const N: usize = 2000;
const EXPECTED_SUM: i32 = ((N - 1) * N / 2) as i32; // 1_999_000
/// ~16× over the measured healthy ~57 ms, ~9× under the ~8 s O(N²) residual.
const CEILING: Duration = Duration::from_millis(900);

#[test]
fn class_field_array_loop_is_linear_not_quadratic() {
    let main = "Public total As Long\n\
                Sub Main()\n\
                \u{20}   Dim o As Thing\n\
                \u{20}   Set o = New Thing\n\
                \u{20}   o.Fill\n\
                \u{20}   total = o.SumAll()\n\
                End Sub\n";
    let cls = format!(
        "Private m() As Long\n\
         Public Sub Fill()\n\
         \u{20}   Dim i As Long\n\
         \u{20}   ReDim m(0 To {N} - 1)\n\
         \u{20}   For i = 0 To {N} - 1\n\
         \u{20}       m(i) = i\n\
         \u{20}   Next i\n\
         End Sub\n\
         Public Function SumAll() As Long\n\
         \u{20}   Dim i As Long, s As Long\n\
         \u{20}   For i = 0 To {N} - 1\n\
         \u{20}       s = s + m(i)\n\
         \u{20}   Next i\n\
         \u{20}   SumAll = s\n\
         End Function\n"
    );
    let modules = [
        ("Main", ModuleKind::Procedural, main),
        ("Thing", ModuleKind::Class, cls.as_str()),
    ];

    let start = Instant::now();
    let outcome = run_modules(Executor::Vm3, &modules, "Bench");
    let elapsed = start.elapsed();

    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome
        .result
        .unwrap_or_else(|e| panic!("vm3 class-field run failed: {e}"));
    assert!(
        snap.contains(&canon(&Variant::from_i32(EXPECTED_SUM))),
        "expected total={EXPECTED_SUM} in snapshot {snap:?}"
    );
    assert!(
        elapsed < CEILING,
        "fill+read of a {N}-element class-instance-field array took {elapsed:?}; the O(1) \
         field-array fix should make this tens of milliseconds — a near-second time means \
         field-held element access regressed to O(N) (the O(N²)-loop residual of bd-us4v)"
    );
}

#[test]
fn udt_field_array_loop_is_linear_not_quadratic() {
    // A module-level UDT value with a dynamic-array member, filled and summed in place.
    let src = format!(
        "Type T\n\
         \u{20}   items() As Long\n\
         End Type\n\
         Public total As Long\n\
         Sub Main()\n\
         \u{20}   Dim r As T, i As Long, s As Long\n\
         \u{20}   ReDim r.items(0 To {N} - 1)\n\
         \u{20}   For i = 0 To {N} - 1\n\
         \u{20}       r.items(i) = i\n\
         \u{20}   Next i\n\
         \u{20}   For i = 0 To {N} - 1\n\
         \u{20}       s = s + r.items(i)\n\
         \u{20}   Next i\n\
         \u{20}   total = s\n\
         End Sub\n"
    );

    let start = Instant::now();
    let outcome = run(Executor::Vm3, &src);
    let elapsed = start.elapsed();

    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome
        .result
        .unwrap_or_else(|e| panic!("vm3 UDT-field run failed: {e}"));
    assert!(
        snap.contains(&canon(&Variant::from_i32(EXPECTED_SUM))),
        "expected total={EXPECTED_SUM} in snapshot {snap:?}"
    );
    assert!(
        elapsed < CEILING,
        "fill+read of a {N}-element UDT-member array took {elapsed:?}; the O(1) field-array \
         fix should make this tens of milliseconds — a near-second time means UDT-member \
         element access regressed to O(N) (the O(N²)-loop residual of bd-us4v)"
    );
}

/// Correctness guard for the fused field index ops' fallback: an OBJECT field whose
/// parentheses are a default-member call (a `Collection` field `c(i)`) must still route
/// through the object default member, not be mistaken for a field array. `ArrayGetField`
/// borrows the field in place, finds it is not an array, and falls back to the
/// materialise-then-default-member path — exactly as the un-fused `FieldGet` + `ArrayGet`
/// did.
#[test]
fn collection_field_indexing_routes_to_default_member() {
    let main = "Public total As Long\n\
                Sub Main()\n\
                \u{20}   Dim o As Thing\n\
                \u{20}   Set o = New Thing\n\
                \u{20}   o.Build\n\
                \u{20}   total = o.Second()\n\
                End Sub\n";
    let cls = "Private c As Collection\n\
               Public Sub Build()\n\
               \u{20}   Set c = New Collection\n\
               \u{20}   c.Add 10\n\
               \u{20}   c.Add 20\n\
               \u{20}   c.Add 30\n\
               End Sub\n\
               Public Function Second() As Long\n\
               \u{20}   Second = c(2)\n\
               End Function\n";
    let modules = [
        ("Main", ModuleKind::Procedural, main),
        ("Thing", ModuleKind::Class, cls),
    ];
    let outcome = run_modules(Executor::Vm3, &modules, "Bench");
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome
        .result
        .unwrap_or_else(|e| panic!("vm3 collection-field run failed: {e}"));
    // c(2) is the second added item: 20.
    assert!(
        snap.contains(&canon(&Variant::from_i32(20))),
        "expected c(2)=20 in snapshot {snap:?}"
    );
}
