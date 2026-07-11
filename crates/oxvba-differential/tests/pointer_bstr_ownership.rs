#![cfg(target_os = "windows")]

use oxvba_runtime::{
    Variant,
    bstr::BStr,
    live_handle_counts,
    pointer_helpers::{
        free_pins, lookup_pointer, register_string_variant_pointer, register_utf16_string,
    },
};
use windows_sys::{
    Win32::Foundation::{SysAllocStringLen, SysFreeString},
    core::BSTR,
};

#[test]
fn bstr_pin_owners_balance_unchanged_null_and_native_replacement_paths() {
    let ordinary_before = live_handle_counts();
    let ordinary = register_utf16_string("normal-pin").expect("ordinary BSTR pin");
    assert_eq!(ordinary_before.balance_to(live_handle_counts()).bstrs, 1);
    free_pins(&[ordinary]);
    assert!(
        ordinary_before.balance_to(live_handle_counts()).is_zero(),
        "ordinary BSTR pin release must balance through the canonical owner"
    );

    let source = Variant::from_string(BStr::from("unchanged"));
    let unchanged_before = live_handle_counts();
    let unchanged = register_string_variant_pointer(&source).expect("unchanged BSTR cell");
    assert_eq!(unchanged_before.balance_to(live_handle_counts()).bstrs, 1);
    free_pins(&[unchanged]);
    assert!(
        unchanged_before.balance_to(live_handle_counts()).is_zero(),
        "unchanged BSTR cell must drop its original canonical owner"
    );

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
    free_pins(&[nulled]);
    assert!(
        null_before.balance_to(live_handle_counts()).is_zero(),
        "native consumption followed by a null cell must debit only the original"
    );

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
    free_pins(&[replaced]);
    assert!(
        replacement_before
            .balance_to(live_handle_counts())
            .is_zero(),
        "native replacement must reconcile the consumed original without falsely debiting the untracked replacement"
    );
}
