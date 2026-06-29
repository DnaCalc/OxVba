//! vm3 fixed-size vs dynamic array `Erase` semantics.
//!
//! VBA `Erase` behaves differently by the array's storage class:
//!   * a **fixed-size** array (`Dim a(1 To 3)`, or a UDT fixed-array field) is
//!     *reinitialized* — every element returns to its type default and the array
//!     stays allocated and usable;
//!   * a **dynamic** array (`Dim a()` + `ReDim`) is *deallocated* — it becomes
//!     uninitialized and indexing it raises until it is re-`ReDim`'d.
//!
//! vm3 carries the distinction on the runtime SAFEARRAY's `FADF_FIXEDSIZE` bit
//! (set at allocation, travelling with copies), exactly as real VBA models it,
//! and `Erase` dispatches on the array value's own flag. These tests pin both
//! arms across every element type; the dynamic-deallocate companion lives in
//! `compound_place_vm3.rs` (`erase_compound_member_array_deallocates`).

use oxvba_differential::{Canon, Executor, RunOutcome, canon, run};
use oxvba_runtime::Variant;

/// Assert the vm3 run completed (not `unsupported`, not `Err`) and that the
/// snapshot CONTAINS `expected` — the read-back of a fixed array element after
/// `Erase` must be the type default, with the array still readable.
fn assert_contains(source: &str, expected: &Canon) {
    let outcome: RunOutcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined the program as unsupported: {:?}\nsource:\n{source}",
        outcome.unsupported
    );
    match &outcome.result {
        Ok(values) => assert!(
            values.contains(expected),
            "vm3 result {values:?} does not contain {expected:?}\nsource:\n{source}"
        ),
        Err(msg) => panic!("vm3 run failed: {msg}\nsource:\n{source}"),
    }
}

/// Assert the vm3 run reached execution but did NOT complete — an indexing
/// attempt against a deallocated dynamic array raises rather than returning a
/// value. (Not `unsupported`: the program is in scope, it just faults.)
fn assert_read_fails(source: &str) {
    let outcome: RunOutcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined the program as unsupported: {:?}\nsource:\n{source}",
        outcome.unsupported
    );
    assert!(
        outcome.result.is_err(),
        "vm3 unexpectedly completed reading a deallocated dynamic array: {:?}\nsource:\n{source}",
        outcome.result
    );
}

/// Build the expected canon for a value by routing a runtime [`Variant`] through
/// the harness's own projection — type-faithful (a `Long 0` differs from an
/// `Integer 0`) without hand-encoding tags.
fn expect(v: Variant) -> Canon {
    canon(&v)
}

// ── Top-level fixed arrays: `Erase` resets each element type to its default ──

/// The handover's headline reproduction: `Erase` of a fixed `Long` array resets
/// `a(2)` to 0 AND the array stays usable (the read after `Erase` succeeds — vm3
/// previously stored `Empty`, so the read faulted with error 13).
#[test]
fn fixed_long_array_erase_resets_and_stays_usable() {
    let source = r#"
Public result As Long
Sub Main()
    Dim arr(1 To 3) As Long
    arr(2) = 7
    Erase arr
    result = arr(2)
End Sub
"#;
    assert_contains(source, &expect(Variant::from_i32(0)));
}

#[test]
fn fixed_integer_array_erase_resets() {
    let source = r#"
Public result As Integer
Sub Main()
    Dim arr(1 To 3) As Integer
    arr(2) = 7
    Erase arr
    result = arr(2)
End Sub
"#;
    assert_contains(source, &expect(Variant::from_i16(0)));
}

#[test]
fn fixed_byte_array_erase_resets() {
    let source = r#"
Public result As Byte
Sub Main()
    Dim arr(1 To 3) As Byte
    arr(2) = 7
    Erase arr
    result = arr(2)
End Sub
"#;
    assert_contains(source, &expect(Variant::from_u8(0)));
}

#[test]
fn fixed_double_array_erase_resets() {
    let source = r#"
Public result As Double
Sub Main()
    Dim arr(1 To 3) As Double
    arr(2) = 7.5
    Erase arr
    result = arr(2)
End Sub
"#;
    assert_contains(source, &expect(Variant::from_f64(0.0)));
}

#[test]
fn fixed_single_array_erase_resets() {
    let source = r#"
Public result As Single
Sub Main()
    Dim arr(1 To 3) As Single
    arr(2) = 7.5
    Erase arr
    result = arr(2)
End Sub
"#;
    assert_contains(source, &expect(Variant::from_f32(0.0)));
}

#[test]
fn fixed_currency_array_erase_resets() {
    let source = r#"
Public result As Currency
Sub Main()
    Dim arr(1 To 3) As Currency
    arr(2) = 7.25
    Erase arr
    result = arr(2)
End Sub
"#;
    assert_contains(source, &expect(Variant::from_currency_scaled_i64(0)));
}

#[test]
fn fixed_date_array_erase_resets() {
    let source = r#"
Public result As Date
Sub Main()
    Dim arr(1 To 3) As Date
    arr(2) = #2026-01-02#
    Erase arr
    result = arr(2)
End Sub
"#;
    assert_contains(source, &expect(Variant::from_date_f64(0.0)));
}

#[test]
fn fixed_string_array_erase_resets_to_empty_string() {
    let source = r#"
Public result As String
Sub Main()
    Dim arr(1 To 3) As String
    arr(2) = "seven"
    Erase arr
    result = arr(2)
End Sub
"#;
    assert_contains(source, &expect(Variant::from_string("")));
}

#[test]
fn fixed_boolean_array_erase_resets_to_false() {
    let source = r#"
Public result As Boolean
Sub Main()
    Dim arr(1 To 3) As Boolean
    arr(2) = True
    Erase arr
    result = arr(2)
End Sub
"#;
    assert_contains(source, &expect(Variant::from_bool(false)));
}

#[test]
fn fixed_variant_array_erase_resets_to_empty() {
    let source = r#"
Public result As Variant
Sub Main()
    Dim arr(1 To 3) As Variant
    arr(2) = 7
    Erase arr
    result = arr(2)
End Sub
"#;
    assert_contains(source, &Canon::Empty);
}

// ── Dynamic array: `Erase` deallocates (read afterwards raises) ──────────────

/// A dynamic array's `Erase` frees the storage — reading an element afterwards
/// raises (the array is uninitialized) instead of returning a default. This is
/// the discriminator from the fixed case above, which reads back 0 cleanly.
#[test]
fn dynamic_array_erase_deallocates_read_raises() {
    let source = r#"
Public result As Long
Sub Main()
    Dim arr() As Long
    ReDim arr(1 To 3)
    arr(2) = 7
    Erase arr
    result = arr(2)
End Sub
"#;
    assert_read_fails(source);
}

/// And after deallocation a fresh `ReDim` re-allocates from scratch — the
/// previously-set element is gone (0), proving the storage was freed not kept.
#[test]
fn dynamic_array_erase_then_redim_starts_fresh() {
    let source = r#"
Public result As Long
Sub Main()
    Dim arr() As Long
    ReDim arr(1 To 3)
    arr(2) = 7
    Erase arr
    ReDim arr(1 To 3)
    result = arr(2)
End Sub
"#;
    assert_contains(source, &expect(Variant::from_i32(0)));
}

// ── UDT fixed-array field: `Erase` resets the inline storage in place ────────

/// `Erase b.arr` where `arr(1 To 3) As Long` is a UDT *fixed-array* field resets
/// each element and the field stays usable (the materialized member array
/// carries `FADF_FIXEDSIZE`, so the reset array is written back into the inline
/// record storage rather than `Empty`, which the inline field cannot hold).
#[test]
fn udt_fixed_array_field_erase_resets_and_stays_usable() {
    let source = r#"
Type T
    arr(1 To 3) As Long
End Type
Public result As Long
Sub Main()
    Dim b As T
    b.arr(2) = 7
    Erase b.arr
    result = b.arr(2)
End Sub
"#;
    assert_contains(source, &expect(Variant::from_i32(0)));
}

#[test]
fn udt_fixed_array_field_erase_resets_string_field() {
    let source = r#"
Type T
    arr(1 To 3) As String
End Type
Public result As String
Sub Main()
    Dim b As T
    b.arr(2) = "seven"
    Erase b.arr
    result = b.arr(2)
End Sub
"#;
    assert_contains(source, &expect(Variant::from_string("")));
}
