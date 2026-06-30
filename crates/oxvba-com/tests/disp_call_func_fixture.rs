#![cfg(all(target_os = "windows", target_arch = "x86_64"))]
#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;

use windows_sys::Win32::System::Variant::{VARIANT, VT_I4, VariantClear, VariantInit};

const CC_STDCALL: u32 = 4;

unsafe extern "system" {
    fn DispCallFunc(
        pvInstance: *mut c_void,
        oVft: usize,
        cc: u32,
        vtReturn: u16,
        cActuals: u32,
        prgvt: *mut u16,
        prgpvarg: *mut *mut c_void,
        pvargResult: *mut VARIANT,
    ) -> i32;
}

#[repr(C)]
struct SyntheticVTable {
    slot0: usize,
    slot1: usize,
}

#[repr(C)]
struct SyntheticObject {
    vtbl: *const SyntheticVTable,
    seen_this: usize,
    seen_arg: i32,
}

unsafe extern "system" fn slot_no_args(this: *mut SyntheticObject) -> i32 {
    (*this).seen_this = this as usize;
    0x2345
}

unsafe extern "system" fn slot_one_i4(this: *mut SyntheticObject, value: i32) -> i32 {
    (*this).seen_this = this as usize;
    (*this).seen_arg = value;
    0x1000 + value
}

fn variant_i4(value: &VARIANT) -> Option<i32> {
    // SAFETY: `value` is a live, initialized VARIANT; reading the `vt` discriminant is
    // always valid, and `lVal` is read only after confirming `vt == VT_I4`, so the
    // accessed union member matches the active variant tag.
    unsafe {
        if value.Anonymous.Anonymous.vt == VT_I4 {
            Some(value.Anonymous.Anonymous.Anonymous.lVal)
        } else {
            None
        }
    }
}

#[test]
fn dispcallfunc_invokes_synthetic_vtable_slots_and_returns_variant_i4() {
    let vtable = SyntheticVTable {
        slot0: slot_no_args as *const () as usize,
        slot1: slot_one_i4 as *const () as usize,
    };
    let mut object = SyntheticObject {
        vtbl: &vtable,
        seen_this: 0,
        seen_arg: 0,
    };
    let expected_this = (&mut object as *mut SyntheticObject) as usize;

    // SAFETY: an all-zero VARIANT is a valid VT_EMPTY value (VT_EMPTY == 0).
    let mut no_arg_result: VARIANT = unsafe { std::mem::zeroed() };
    // SAFETY: `no_arg_result` is a live, writable VARIANT; VariantInit sets it VT_EMPTY.
    unsafe { VariantInit(&mut no_arg_result) };
    // SAFETY: `object` is a live SyntheticObject whose slot0 (`slot_no_args`) matches
    // the `extern "system" fn(this) -> i32` signature DispCallFunc invokes: oVft=0
    // selects slot0, cActuals=0 with null arg arrays, vtReturn=VT_I4, and
    // `no_arg_result` is a live VARIANT out-cell.
    let hr = unsafe {
        DispCallFunc(
            (&mut object as *mut SyntheticObject).cast(),
            0,
            CC_STDCALL,
            VT_I4,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut no_arg_result,
        )
    };
    assert_eq!(hr, 0, "zero-arg DispCallFunc HRESULT");
    assert_eq!(variant_i4(&no_arg_result), Some(0x2345));
    assert_eq!(object.seen_this, expected_this);
    // SAFETY: `no_arg_result` is the live VARIANT populated by DispCallFunc; clearing
    // it releases any owned contents exactly once.
    unsafe {
        let _ = VariantClear(&mut no_arg_result);
    }

    // SAFETY: an all-zero VARIANT is a valid VT_EMPTY value (VT_EMPTY == 0).
    let mut value: VARIANT = unsafe { std::mem::zeroed() };
    // SAFETY: `value` is a live, writable VARIANT; init it to VT_EMPTY, then set the
    // VT_I4 tag and write its `lVal` union member consistently with that tag.
    unsafe {
        VariantInit(&mut value);
        value.Anonymous.Anonymous.vt = VT_I4;
        value.Anonymous.Anonymous.Anonymous.lVal = 37;
    }
    let mut arg_types = [VT_I4];
    let mut arg_ptrs = [(&mut value as *mut VARIANT).cast::<c_void>()];
    // SAFETY: an all-zero VARIANT is a valid VT_EMPTY value (VT_EMPTY == 0).
    let mut one_arg_result: VARIANT = unsafe { std::mem::zeroed() };
    // SAFETY: `one_arg_result` is a live, writable VARIANT; VariantInit sets it VT_EMPTY.
    unsafe { VariantInit(&mut one_arg_result) };
    // SAFETY: `object`'s slot1 (`slot_one_i4`) matches the `fn(this, i32) -> i32`
    // signature: oVft=size_of::<usize>() selects slot1, cActuals=1 with arg_types=[VT_I4]
    // and arg_ptrs referencing the live `value` VARIANT, vtReturn=VT_I4, and
    // `one_arg_result` is a live VARIANT out-cell.
    let hr = unsafe {
        DispCallFunc(
            (&mut object as *mut SyntheticObject).cast(),
            std::mem::size_of::<usize>(),
            CC_STDCALL,
            VT_I4,
            1,
            arg_types.as_mut_ptr(),
            arg_ptrs.as_mut_ptr(),
            &mut one_arg_result,
        )
    };
    assert_eq!(hr, 0, "one-arg DispCallFunc HRESULT");
    assert_eq!(object.seen_arg, 37);
    assert_eq!(variant_i4(&one_arg_result), Some(0x1025));
    // SAFETY: `one_arg_result` and `value` are both live, initialized VARIANTs,
    // each cleared exactly once at the end of the test.
    unsafe {
        let _ = VariantClear(&mut one_arg_result);
        let _ = VariantClear(&mut value);
    }
}
