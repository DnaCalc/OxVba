#![cfg(target_os = "windows")]

use oxvba_runtime::{
    Variant,
    bstr::BStr,
    live_handle_counts,
    pointer_helpers::{
        free_pins, lookup_pointer, read_back_string_payload_variant,
        register_string_variant_pointer, register_utf16_string,
    },
};
use windows_sys::{
    Win32::Foundation::{SysAllocStringLen, SysFreeString},
    core::BSTR,
};

fn assert_zero_carrier_drift(before: oxvba_runtime::LiveHandleCounts, label: &str) {
    let drift = before.balance_to(live_handle_counts());
    assert_eq!(drift.bstrs, 0, "{label} must have zero BSTR drift");
    assert!(
        drift.is_zero(),
        "{label} must have zero total carrier drift, got {drift:?}"
    );
}

fn assert_string_value(value: &Variant, expected: &str, label: &str) {
    assert_eq!(
        value.string_units(),
        Some(expected.encode_utf16().collect()),
        "{label} must preserve the exact UTF-16 value"
    );
}

#[test]
fn bstr_pin_owners_balance_unchanged_null_and_native_replacement_paths() {
    let test_before = live_handle_counts();

    let ordinary_before = live_handle_counts();
    let ordinary = register_utf16_string("normal-pin").expect("ordinary BSTR pin");
    assert_eq!(ordinary_before.balance_to(live_handle_counts()).bstrs, 1);
    let ordinary_value =
        read_back_string_payload_variant(ordinary).expect("ordinary BSTR pin readback");
    assert_string_value(&ordinary_value, "normal-pin", "ordinary BSTR pin");
    drop(ordinary_value);
    free_pins(&[ordinary]);
    free_pins(&[ordinary]);
    assert_zero_carrier_drift(
        ordinary_before,
        "ordinary BSTR pin release through the canonical owner",
    );

    let source = Variant::from_string(BStr::from("unchanged"));
    let unchanged_before = live_handle_counts();
    let unchanged = register_string_variant_pointer(&source).expect("unchanged BSTR cell");
    assert_eq!(unchanged_before.balance_to(live_handle_counts()).bstrs, 1);
    let unchanged_value =
        read_back_string_payload_variant(unchanged).expect("unchanged BSTR cell readback");
    assert_string_value(&unchanged_value, "unchanged", "unchanged BSTR cell");
    drop(unchanged_value);
    free_pins(&[unchanged]);
    free_pins(&[unchanged]);
    assert_zero_carrier_drift(unchanged_before, "unchanged BSTR cell canonical-owner drop");

    let null_before = live_handle_counts();
    let nulled = register_string_variant_pointer(&source).expect("nulled BSTR cell");
    let nulled_cell = lookup_pointer(nulled)
        .expect("nulled BSTR cell pointer")
        .cast::<BSTR>();
    // SAFETY: `nulled` still names a live registry entry created specifically as
    // a `Box<BSTR>`, so its looked-up address is valid and aligned for one read.
    let consumed = unsafe { *nulled_cell };
    // SAFETY: `consumed` is the live original BSTR solely owned by the cell. This
    // emulates the documented LPBSTR convention: native code consumes the old
    // value before writing null back through the still-live cell.
    unsafe {
        SysFreeString(consumed);
        *nulled_cell = std::ptr::null_mut();
    }
    let null_value =
        read_back_string_payload_variant(nulled).expect("native-consumed null-cell readback");
    assert_string_value(&null_value, "", "native-consumed null BSTR cell");
    drop(null_value);
    free_pins(&[nulled]);
    free_pins(&[nulled]);
    assert_zero_carrier_drift(null_before, "native consumption followed by a null cell");

    let replacement_before = live_handle_counts();
    let replaced = register_string_variant_pointer(&source).expect("replaced BSTR cell");
    let replaced_cell = lookup_pointer(replaced)
        .expect("replaced BSTR cell pointer")
        .cast::<BSTR>();
    // SAFETY: `replaced` still names a live registry entry created specifically
    // as a `Box<BSTR>`, so its looked-up address is valid and aligned for one read.
    let consumed = unsafe { *replaced_cell };
    let replacement = BStr::from("native").owned_core();
    let replacement_len =
        u32::try_from(replacement.len_code_units()).expect("u32 replacement length");
    // SAFETY: `consumed` is the cell's live original BSTR, while `replacement`
    // provides `replacement_len` initialized UTF-16 units for the allocation.
    // This emulates an LPBSTR callee consuming the original before installing
    // its own non-OxVba-tracked BSTR in the still-live, writable cell.
    unsafe {
        SysFreeString(consumed);
        *replaced_cell = SysAllocStringLen(replacement.payload_ptr(), replacement_len);
    }
    // SAFETY: The cell entry remains live and writable until `free_pins`; the
    // preceding allocation stored either a valid BSTR or null in that cell.
    assert!(!unsafe { *replaced_cell }.is_null());
    let replacement_value =
        read_back_string_payload_variant(replaced).expect("native replacement readback");
    assert_string_value(&replacement_value, "native", "native replacement BSTR cell");
    drop(replacement_value);
    free_pins(&[replaced]);
    free_pins(&[replaced]);
    assert_zero_carrier_drift(
        replacement_before,
        "native replacement reconciliation without an untracked replacement debit",
    );

    assert_string_value(&source, "unchanged", "source after all pin releases");
    drop(source);
    assert_zero_carrier_drift(test_before, "complete isolated pointer-helper test");
}
