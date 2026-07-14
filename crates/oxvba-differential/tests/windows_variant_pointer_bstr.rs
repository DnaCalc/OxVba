#![cfg(target_os = "windows")]

use std::{
    collections::HashSet,
    panic::{AssertUnwindSafe, catch_unwind},
};

use oxvba_runtime::{
    VarType, Variant,
    bstr::BStr,
    live_handle_counts,
    pointer_helpers::{
        free_pins, live_pin_count, lookup_pointer, register_variant_var_variant_pointer,
    },
    safe_array::{SafeArray, SafeArrayBound},
};
use windows_sys::{
    Win32::{
        Foundation::{SysAllocStringLen, SysStringLen},
        System::{
            Ole::SafeArrayGetElement,
            Variant::{
                VARIANT, VT_ARRAY, VT_BSTR, VT_EMPTY, VT_I4, VT_NULL, VT_VARIANT, VariantClear,
            },
        },
    },
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

fn assert_one_bstr_token(before: oxvba_runtime::LiveHandleCounts, label: &str) {
    assert_bstr_token_count(before, 1, label);
}

fn assert_bstr_token_count(before: oxvba_runtime::LiveHandleCounts, expected: isize, label: &str) {
    let drift = before.balance_to(live_handle_counts());
    assert_eq!(
        drift.bstrs, expected,
        "{label} must retain {expected} BSTR token(s)"
    );
    assert_eq!(drift.object_boxes, 0, "{label} object drift");
    assert_eq!(drift.safearrays, 0, "{label} SAFEARRAY drift");
    assert_eq!(drift.record_buffers, 0, "{label} record drift");
}

fn raw_variant(pin: i64) -> *mut VARIANT {
    lookup_pointer(pin)
        .expect("registered VARIANT cell pointer")
        .cast()
}

unsafe fn variant_vt(cell: *const VARIANT) -> u16 {
    // SAFETY: the caller supplies the live VARIANT cell returned by the pointer registry.
    unsafe { (*cell).Anonymous.Anonymous.vt }
}

unsafe fn variant_bstr(cell: *const VARIANT) -> BSTR {
    // SAFETY: the caller verifies VT_BSTR before using this helper.
    unsafe { (*cell).Anonymous.Anonymous.Anonymous.bstrVal }
}

unsafe fn bstr_units(raw: BSTR) -> Vec<u16> {
    if raw.is_null() {
        return Vec::new();
    }
    // SAFETY: the caller supplies a live BSTR; SysStringLen returns its initialized
    // UTF-16 payload length, which bounds the slice below.
    let len = unsafe { SysStringLen(raw) } as usize;
    // SAFETY: `raw` remains live for this call and `len` is its prefix-derived extent.
    unsafe { std::slice::from_raw_parts(raw, len) }.to_vec()
}

fn expected_units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn free_pin_idempotently(pin: i64) {
    free_pins(&[pin, pin]);
    free_pins(&[pin]);
}

unsafe fn replace_with_native_bstr(cell: *mut VARIANT, text: &str) {
    // SAFETY: the caller supplies a live owned VARIANT cell. Clear its current
    // value first, then transfer one independently allocated native BSTR to it.
    unsafe {
        assert!(VariantClear(cell) >= 0);
        let units = expected_units(text);
        let len = u32::try_from(units.len()).expect("native replacement length");
        let replacement = SysAllocStringLen(units.as_ptr(), len);
        assert!(!replacement.is_null());
        (*cell).Anonymous.Anonymous.Anonymous.bstrVal = replacement;
        (*cell).Anonymous.Anonymous.vt = VT_BSTR;
    }
}

#[test]
fn windows_variant_pointer_bstr_balance() {
    let test_before = live_handle_counts();
    let pins_before = live_pin_count();

    let source = Variant::from_string(BStr::from("original\0value"));
    let second_source = Variant::from_string(BStr::from("second\0value"));

    // Two simultaneously live Variant cells must have independent stable
    // identities, payloads and accounting tokens. Release in both orders and
    // prove that removing one pin neither aliases nor invalidates the other.
    let pair_before = live_handle_counts();
    let first = register_variant_var_variant_pointer(&source).expect("first simultaneous cell");
    let second =
        register_variant_var_variant_pointer(&second_source).expect("second simultaneous cell");
    assert_ne!(first, second, "simultaneous Variant pin IDs must differ");
    let first_cell = raw_variant(first);
    let second_cell = raw_variant(second);
    assert_ne!(
        first_cell, second_cell,
        "simultaneous Variant cells must not alias"
    );
    assert_bstr_token_count(pair_before, 2, "two simultaneous Variant cells");
    // SAFETY: both pointers are distinct live registry-owned VARIANT cells.
    unsafe {
        assert!(VariantClear(first_cell) >= 0);
        (*first_cell).Anonymous.Anonymous.vt = VT_NULL;
        replace_with_native_bstr(second_cell, "native\0second");
    }
    free_pin_idempotently(first);
    assert!(lookup_pointer(first).is_none());
    assert_eq!(
        lookup_pointer(second).map(|pointer| pointer.cast::<VARIANT>()),
        Some(second_cell),
        "freeing the first pin must leave the second cell stable"
    );
    // SAFETY: `second_cell` remains owned by the second registry entry.
    unsafe {
        assert_eq!(variant_vt(second_cell), VT_BSTR);
        assert_eq!(
            bstr_units(variant_bstr(second_cell)),
            expected_units("native\0second")
        );
    }
    assert_bstr_token_count(pair_before, 1, "second simultaneous Variant cell");
    free_pin_idempotently(second);
    assert_zero_carrier_drift(pair_before, "first-then-second Variant release");

    let reverse_before = live_handle_counts();
    let reverse_first = register_variant_var_variant_pointer(&source).expect("reverse first cell");
    let reverse_second =
        register_variant_var_variant_pointer(&second_source).expect("reverse second cell");
    let reverse_first_cell = raw_variant(reverse_first);
    let reverse_second_cell = raw_variant(reverse_second);
    assert_ne!(reverse_first, reverse_second);
    assert_ne!(reverse_first_cell, reverse_second_cell);
    // SAFETY: both pointers are distinct live registry-owned VARIANT cells.
    unsafe {
        replace_with_native_bstr(reverse_first_cell, "native reverse first");
        assert!(VariantClear(reverse_second_cell) >= 0);
        (*reverse_second_cell).Anonymous.Anonymous.vt = VT_NULL;
    }
    assert_bstr_token_count(reverse_before, 2, "reverse-order Variant cells");
    free_pin_idempotently(reverse_second);
    assert_eq!(
        lookup_pointer(reverse_first).map(|pointer| pointer.cast::<VARIANT>()),
        Some(reverse_first_cell),
        "freeing the second pin must leave the first cell stable"
    );
    assert_bstr_token_count(reverse_before, 1, "reverse first Variant cell");
    free_pin_idempotently(reverse_first);
    assert_zero_carrier_drift(reverse_before, "second-then-first Variant release");

    // Force repeated HashMap growth after recording each cell address. The Box
    // allocation must keep every ID/pointer stable while map buckets move.
    let growth_source = Variant::from_string(BStr::from("rehash\0payload"));
    let growth_before = live_handle_counts();
    let mut growth_pins = Vec::new();
    let mut unique_ids = HashSet::new();
    for _ in 0..128 {
        let pin = register_variant_var_variant_pointer(&growth_source)
            .expect("register rehash Variant cell");
        assert!(unique_ids.insert(pin), "every live pin ID must be unique");
        growth_pins.push((pin, raw_variant(pin)));
    }
    assert_bstr_token_count(growth_before, 128, "rehash Variant cells");
    for (pin, original_pointer) in &growth_pins {
        assert_eq!(
            lookup_pointer(*pin).map(|pointer| pointer.cast::<VARIANT>()),
            Some(*original_pointer),
            "HashMap growth must not move Variant cell {pin:#x}"
        );
        // SAFETY: the corresponding pin is still live and was not mutated.
        unsafe {
            assert_eq!(variant_vt(*original_pointer), VT_BSTR);
            assert_eq!(
                bstr_units(variant_bstr(*original_pointer)),
                expected_units("rehash\0payload")
            );
        }
    }
    let even_pins: Vec<i64> = growth_pins.iter().step_by(2).map(|(pin, _)| *pin).collect();
    free_pins(&even_pins);
    assert_bstr_token_count(growth_before, 64, "odd rehash Variant cells");
    for (index, (pin, original_pointer)) in growth_pins.iter().enumerate() {
        if index % 2 == 0 {
            assert!(lookup_pointer(*pin).is_none());
        } else {
            assert_eq!(
                lookup_pointer(*pin).map(|pointer| pointer.cast::<VARIANT>()),
                Some(*original_pointer)
            );
        }
    }
    let odd_pins: Vec<i64> = growth_pins
        .iter()
        .skip(1)
        .step_by(2)
        .map(|(pin, _)| *pin)
        .collect();
    free_pins(&odd_pins);
    free_pins(&even_pins);
    free_pins(&odd_pins);
    assert_zero_carrier_drift(growth_before, "rehash Variant cell release");

    let unchanged_before = live_handle_counts();
    let unchanged = register_variant_var_variant_pointer(&source).expect("unchanged VARIANT cell");
    assert_one_bstr_token(unchanged_before, "unchanged VARIANT cell");
    let unchanged_cell = raw_variant(unchanged);
    // SAFETY: `unchanged_cell` remains registry-owned and has not been mutated.
    unsafe {
        assert_eq!(variant_vt(unchanged_cell), VT_BSTR);
        assert_eq!(
            bstr_units(variant_bstr(unchanged_cell)),
            expected_units("original\0value")
        );
    }
    free_pin_idempotently(unchanged);
    assert_zero_carrier_drift(unchanged_before, "unchanged VARIANT cell release");

    let null_before = live_handle_counts();
    let nulled = register_variant_var_variant_pointer(&source).expect("nulled VARIANT cell");
    assert_one_bstr_token(null_before, "nulled VARIANT cell before native clear");
    let nulled_cell = raw_variant(nulled);
    // SAFETY: this emulates a valid native in/out replacement: clear the owned
    // original first, then leave a canonical VT_NULL value in the live cell.
    unsafe {
        assert!(VariantClear(nulled_cell) >= 0);
        (*nulled_cell).Anonymous.Anonymous.vt = VT_NULL;
        assert_eq!(variant_vt(nulled_cell), VT_NULL);
    }
    assert_one_bstr_token(null_before, "nulled VARIANT cell after native clear");
    free_pin_idempotently(nulled);
    assert_zero_carrier_drift(null_before, "nulled VARIANT cell release");

    let replacement_before = live_handle_counts();
    let replaced = register_variant_var_variant_pointer(&source).expect("replaced VARIANT cell");
    assert_one_bstr_token(
        replacement_before,
        "replacement VARIANT cell before native clear",
    );
    let replaced_cell = raw_variant(replaced);
    let replacement_units = expected_units("native\0replacement");
    let replacement_len = u32::try_from(replacement_units.len()).expect("replacement length");
    // SAFETY: clear the cell's original owned BSTR, allocate an independent
    // native replacement from the live units, and install a valid VT_BSTR value.
    unsafe {
        assert!(VariantClear(replaced_cell) >= 0);
        let replacement = SysAllocStringLen(replacement_units.as_ptr(), replacement_len);
        assert!(!replacement.is_null());
        (*replaced_cell).Anonymous.Anonymous.Anonymous.bstrVal = replacement;
        (*replaced_cell).Anonymous.Anonymous.vt = VT_BSTR;
        assert_eq!(variant_vt(replaced_cell), VT_BSTR);
        assert_eq!(bstr_units(variant_bstr(replaced_cell)), replacement_units);
    }
    assert_one_bstr_token(
        replacement_before,
        "replacement VARIANT cell after native replacement",
    );
    free_pin_idempotently(replaced);
    assert_zero_carrier_drift(replacement_before, "native replacement VARIANT release");

    let null_bstr = Variant::zeroed(VarType::String);
    let null_bstr_before = live_handle_counts();
    let null_bstr_pin =
        register_variant_var_variant_pointer(&null_bstr).expect("null-BSTR VARIANT cell");
    let null_bstr_cell = raw_variant(null_bstr_pin);
    // SAFETY: this is the live cell just projected from the null String carrier.
    unsafe {
        assert_eq!(variant_vt(null_bstr_cell), VT_BSTR);
        assert!(variant_bstr(null_bstr_cell).is_null());
    }
    assert_zero_carrier_drift(null_bstr_before, "null-BSTR projection");
    free_pin_idempotently(null_bstr_pin);
    assert_zero_carrier_drift(null_bstr_before, "null-BSTR release");

    for (value, expected_vt, label) in [
        (Variant::empty(), VT_EMPTY, "Empty"),
        (Variant::null(), VT_NULL, "Null"),
        (Variant::from_i32(42), VT_I4, "Long"),
    ] {
        let before = live_handle_counts();
        let pin = register_variant_var_variant_pointer(&value)
            .unwrap_or_else(|error| panic!("{label} VARIANT projection failed: {error}"));
        let cell = raw_variant(pin);
        // SAFETY: `cell` is the live pointer-registry VARIANT just projected above.
        unsafe {
            assert_eq!(variant_vt(cell), expected_vt, "{label} native vt");
            if expected_vt == VT_I4 {
                assert_eq!((*cell).Anonymous.Anonymous.Anonymous.lVal, 42);
            }
        }
        assert_zero_carrier_drift(before, &format!("{label} projection"));
        free_pin_idempotently(pin);
        assert_zero_carrier_drift(before, &format!("{label} release"));
    }

    let array_source =
        Variant::from_safearray(SafeArray::from_variants(vec![Variant::from_string(
            "array\0text",
        )]));
    let array_before = live_handle_counts();
    let array_pin =
        register_variant_var_variant_pointer(&array_source).expect("BSTR array VARIANT cell");
    assert_zero_carrier_drift(array_before, "BSTR array projection temporaries");
    let array_cell = raw_variant(array_pin);
    // SAFETY: `array_cell` is the live VT_ARRAY|VT_VARIANT registry cell. The
    // element read deep-copies one VARIANT into zero-initialized output, which
    // this test clears exactly once after inspecting the BSTR value.
    unsafe {
        assert_eq!(variant_vt(array_cell), VT_ARRAY | VT_VARIANT);
        let psa = (*array_cell).Anonymous.Anonymous.Anonymous.parray;
        assert!(!psa.is_null());
        let mut element: VARIANT = std::mem::zeroed();
        let index = 0i32;
        assert!(
            SafeArrayGetElement(
                psa.cast_const(),
                &index,
                (&mut element as *mut VARIANT).cast()
            ) >= 0
        );
        assert_eq!(variant_vt(&element), VT_BSTR);
        assert_eq!(
            bstr_units(variant_bstr(&element)),
            expected_units("array\0text")
        );
        assert!(VariantClear(&mut element) >= 0);
    }
    free_pin_idempotently(array_pin);
    assert_zero_carrier_drift(array_before, "BSTR array VARIANT release");

    // A valid one-element dimension may start at LONG::MAX. Index progression
    // must not increment after the final element, and the native array must
    // remain fully owned across the catch_unwind boundary.
    let edge_array = Variant::from_safearray(SafeArray::from_variants_nd(
        vec![
            SafeArrayBound {
                count: 1,
                lower: i32::MAX,
            },
            SafeArrayBound { count: 1, lower: 0 },
        ],
        vec![Variant::from_string("edge\0bound")],
    ));
    let edge_before = live_handle_counts();
    let edge_outcome = catch_unwind(AssertUnwindSafe(|| {
        register_variant_var_variant_pointer(&edge_array)
    }));
    let edge_pin = edge_outcome
        .expect("LONG::MAX one-element bound must not unwind")
        .expect("LONG::MAX one-element bound must project");
    let edge_cell = raw_variant(edge_pin);
    // SAFETY: the projected cell owns a two-dimensional VT_VARIANT SAFEARRAY.
    unsafe {
        assert_eq!(variant_vt(edge_cell), VT_ARRAY | VT_VARIANT);
        let psa = (*edge_cell).Anonymous.Anonymous.Anonymous.parray;
        let mut element: VARIANT = std::mem::zeroed();
        let indices = [i32::MAX, 0];
        assert!(
            SafeArrayGetElement(
                psa.cast_const(),
                indices.as_ptr(),
                (&mut element as *mut VARIANT).cast()
            ) >= 0
        );
        assert_eq!(variant_vt(&element), VT_BSTR);
        assert_eq!(
            bstr_units(variant_bstr(&element)),
            expected_units("edge\0bound")
        );
        assert!(VariantClear(&mut element) >= 0);
    }
    free_pin_idempotently(edge_pin);
    assert_zero_carrier_drift(edge_before, "LONG::MAX edge-bound release");

    // This shape cannot advance to its second element within a Windows LONG
    // index. The first BSTR has already been copied into the native SAFEARRAY
    // when checked progression fails, so the guard must destroy that partial
    // array without a panic, published pin, or tracked residue.
    let overflow_array = Variant::from_safearray(SafeArray::from_variants_nd(
        vec![
            SafeArrayBound {
                count: 2,
                lower: i32::MAX,
            },
            SafeArrayBound { count: 1, lower: 0 },
        ],
        vec![
            Variant::from_string("partial first"),
            Variant::from_string("unreachable second"),
        ],
    ));
    let overflow_before = live_handle_counts();
    let overflow_pins = live_pin_count();
    let overflow_outcome = catch_unwind(AssertUnwindSafe(|| {
        register_variant_var_variant_pointer(&overflow_array)
    }));
    let overflow_error = overflow_outcome
        .expect("unrepresentable upper bound must return an error, not unwind")
        .expect_err("unrepresentable upper bound must fail projection");
    assert!(
        overflow_error.contains("exceeds the Windows LONG index range"),
        "unexpected upper-bound failure: {overflow_error}"
    );
    assert_eq!(live_pin_count(), overflow_pins);
    assert_zero_carrier_drift(overflow_before, "partial edge-bound cleanup");

    let failing_array = Variant::from_safearray(SafeArray::from_variants_nd(
        vec![
            SafeArrayBound { count: 2, lower: 3 },
            SafeArrayBound {
                count: 1,
                lower: -1,
            },
        ],
        vec![
            Variant::from_string("copied before failure"),
            Variant::from_proc_ref(1),
        ],
    ));
    let failure_before = live_handle_counts();
    let failure_pins = live_pin_count();
    let error = register_variant_var_variant_pointer(&failing_array)
        .expect_err("unsupported nested ProcRef must fail projection");
    assert!(
        error.contains("procedure references cannot be marshaled"),
        "unexpected nested projection error: {error}"
    );
    assert_eq!(live_pin_count(), failure_pins);
    assert_zero_carrier_drift(failure_before, "multi-dimensional failure cleanup");
    assert_eq!(
        failing_array
            .safearray_element(0)
            .expect("source array")
            .expect("source String")
            .string_units(),
        Some(expected_units("copied before failure"))
    );

    drop(failing_array);
    drop(overflow_array);
    drop(edge_array);
    drop(array_source);
    drop(growth_source);
    drop(second_source);
    drop(null_bstr);
    drop(source);
    assert_eq!(live_pin_count(), pins_before);
    assert_zero_carrier_drift(test_before, "complete Windows VARIANT BSTR test");
}
