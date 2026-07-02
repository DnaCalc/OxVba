//! vm3 compound-place elaboration acceptance tests.
//!
//! A "compound place" is a place whose mutable base is itself nested — e.g. an array
//! element whose array lives in a UDT field (`b.arr(i)`), a UDT field nested in another
//! UDT (`o.x.y`), or a member array resized with `ReDim b.arr(n)`. The one-level paths
//! (`arr(i)` where `arr` is a local; `b.field`) always worked; these tests pin the
//! nested cases that the OxIR elaboration pass previously declined with
//! `unsupported(elaborate: compound place)`.
//!
//! Each program writes a value through a compound lvalue and reads it back into the
//! `Main` result snapshot, so a passing run is `Ok([.. expected ..])` — never
//! `unsupported(...)` or `Err(...)`.

use oxvba_differential::{Canon, Executor, RunOutcome, run};

/// A VBA `Long` literal `n` canonicalizes to a `Raw { tag: 3, .. }` whose first 4 little-
/// endian bytes are `n`. Build the expected `Canon` so assertions read cleanly.
fn long(n: i32) -> Canon {
    let b = n.to_le_bytes();
    Canon::Raw {
        tag: 3,
        bytes: [b[0], b[1], b[2], b[3], 0, 0, 0, 0],
        reserved: [0, 0, 0],
    }
}

/// Assert the vm3 run completed (not `unsupported`, not `Err`) and that `result`
/// CONTAINS the expected canon value (the snapshot is entry globals + `Main` locals, so
/// we don't pin the exact position — only that the read-back value is present).
fn assert_vm3_contains(source: &str, expected: &Canon) {
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

/// 1. Member-array element lvalue + read-back: an `Index` place whose `array` base is a
///    `RecordField` (`b.arr(2)`). Writing the element then reading it must round-trip.
#[test]
fn member_array_element_lvalue_and_readback() {
    let source = r#"
Type T
    arr(1 To 3) As Long
End Type

Public result As Long

Sub Main()
    Dim b As T
    b.arr(2) = 7
    result = b.arr(2)
End Sub
"#;
    assert_vm3_contains(source, &long(7));
}

/// 2. Nested UDT field: a `RecordField` place whose `base` is another `RecordField`
///    (`o.x.y`). The recursive write-back must mutate the inner record then store the
///    outer record.
#[test]
fn nested_udt_field_lvalue_and_readback() {
    let source = r#"
Type Inner
    y As Long
End Type

Type Outer
    x As Inner
End Type

Public result As Long

Sub Main()
    Dim o As Outer
    o.x.y = 9
    result = o.x.y
End Sub
"#;
    assert_vm3_contains(source, &long(9));
}

/// 3. Member-array `ReDim` + element assignment: `ReDim b.arr(...)` builds a fresh array
///    into a temp, then writes it back into the UDT field; the subsequent element
///    assignment + read-back must see the resized array.
#[test]
fn member_array_redim_and_element() {
    let source = r#"
Type T2
    arr() As Long
End Type

Public result As Long

Sub Main()
    Dim b As T2
    ReDim b.arr(1 To 4)
    b.arr(3) = 5
    result = b.arr(3)
End Sub
"#;
    assert_vm3_contains(source, &long(5));
}

/// 4. Reading a member-array element inside an expression (`b.arr(2) + 1`). The read
///    context must materialize the compound base and index it.
#[test]
fn member_array_element_read_in_expression() {
    let source = r#"
Type T
    arr(1 To 3) As Long
End Type

Public result As Long

Sub Main()
    Dim b As T
    b.arr(2) = 6
    result = b.arr(2) + 1
End Sub
"#;
    assert_vm3_contains(source, &long(7));
}

/// 5. Two levels of nesting that exercise BOTH a record-field base and an array-element
///    base together: an array of UDTs as a member array, writing through `b.items(2).y`.
///    (`Index` whose array is a `RecordField`, then a `RecordField` on the element.)
#[test]
fn member_array_of_udt_field_lvalue() {
    let source = r#"
Type Item
    y As Long
End Type

Type Bag
    items(1 To 3) As Item
End Type

Public result As Long

Sub Main()
    Dim b As Bag
    b.items(2).y = 11
    result = b.items(2).y
End Sub
"#;
    assert_vm3_contains(source, &long(11));
}

/// 6. **Compound `ByRef` copy-out change-detection (the `VariantChanged` guard).** A
///    compound place (`g.Count`, a `RecordField`) is passed `ByRef` to a proc that LEAVES
///    the parameter byte-identical but mutates the SAME place out-of-band (`g.Count = 100`).
///    The copied-in `ByRef` temp is unchanged, so its copy-out MUST be suppressed — otherwise
///    the stale pre-call snapshot (5) clobbers the out-of-band write (100). vm2 leaves 100
///    (its `VariantChanged` + `JumpIfZero` guard); vm3 must match. Without the guard, an
///    unconditional copy-out resets `g.Count` to 5.
#[test]
fn compound_byref_unchanged_param_does_not_clobber_out_of_band_mutation() {
    let source = r#"
Type T
    Count As Long
End Type

Public g As T
Public result As Long

Sub Main()
    g.Count = 5
    Foo g.Count
    result = g.Count
End Sub

Sub Foo(ByRef x As Long)
    ' x is NOT touched; the same place is mutated out-of-band.
    g.Count = 100
End Sub
"#;
    assert_vm3_contains(source, &long(100));
}

/// 7. Companion to (6): when the callee DOES change the `ByRef` compound parameter, the
///    copy-out must fire (the guard suppresses only unchanged copies). `Foo` sets `x = 42`,
///    so the read-back of `g.Count` is 42 (the copied-out value), not the original 5.
#[test]
fn compound_byref_changed_param_writes_back() {
    let source = r#"
Type T
    Count As Long
End Type

Public g As T
Public result As Long

Sub Main()
    g.Count = 5
    Foo g.Count
    result = g.Count
End Sub

Sub Foo(ByRef x As Long)
    x = 42
End Sub
"#;
    assert_vm3_contains(source, &long(42));
}

/// 8. **`Erase` of a compound (UDT member) array** — the verbatim ChibiPDF case
///    (`Erase m_Results.Lines`, a `Lines() As ocrLine` dynamic member array). The OxIR
///    `Erase` arm previously declined this with `unsupported(compound place)`, so the
///    ChibiEx class could not elaborate. The fix routes a compound `Erase` through the
///    same materialize-and-write-back as a compound `ReDim`: read the member array into a
///    temp, erase it (vm2-faithful: the array becomes `Empty`/deallocated — vm3's
///    `ArrayErase` matches vm2's "store Empty", element-reset of fixed arrays being a
///    deferred refinement in BOTH VMs), and write it back.
///
///    Strong observable: after `Erase`, a `ReDim Preserve` has nothing to preserve, so the
///    previously-set `b.arr(2) = 7` reads back as 0 — proving the member array was actually
///    deallocated, not left intact (a no-op `Erase` would preserve the 7).
#[test]
fn erase_compound_member_array_deallocates() {
    let source = r#"
Type T2
    arr() As Long
End Type

Public result As Long

Sub Main()
    Dim b As T2
    ReDim b.arr(1 To 3)
    b.arr(2) = 7
    Erase b.arr
    ReDim Preserve b.arr(1 To 3)
    result = b.arr(2)
End Sub
"#;
    assert_vm3_contains(source, &long(0));
}
