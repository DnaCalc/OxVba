#![cfg(target_os = "windows")]

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
    let drift = before.balance_to(live_handle_counts());
    assert_eq!(drift.bstrs, 1, "{label} must retain one BSTR token");
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

#[test]
fn windows_variant_pointer_bstr_balance() {
    let test_before = live_handle_counts();
    let pins_before = live_pin_count();

    let source = Variant::from_string(BStr::from("original\0value"));

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
    drop(array_source);
    drop(null_bstr);
    drop(source);
    assert_eq!(live_pin_count(), pins_before);
    assert_zero_carrier_drift(test_before, "complete Windows VARIANT BSTR test");
}
