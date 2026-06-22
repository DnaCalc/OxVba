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

    let mut no_arg_result: VARIANT = unsafe { std::mem::zeroed() };
    unsafe { VariantInit(&mut no_arg_result) };
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
    unsafe {
        let _ = VariantClear(&mut no_arg_result);
    }

    let mut value: VARIANT = unsafe { std::mem::zeroed() };
    unsafe {
        VariantInit(&mut value);
        value.Anonymous.Anonymous.vt = VT_I4;
        value.Anonymous.Anonymous.Anonymous.lVal = 37;
    }
    let mut arg_types = [VT_I4];
    let mut arg_ptrs = [(&mut value as *mut VARIANT).cast::<c_void>()];
    let mut one_arg_result: VARIANT = unsafe { std::mem::zeroed() };
    unsafe { VariantInit(&mut one_arg_result) };
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
    unsafe {
        let _ = VariantClear(&mut one_arg_result);
        let _ = VariantClear(&mut value);
    }
}
