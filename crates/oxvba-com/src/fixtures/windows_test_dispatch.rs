#![allow(unsafe_op_in_unsafe_fn)]

use crate::windows_client::{
    COM_CONNECT_E_CANNOTCONNECT, COM_CONNECT_E_NOCONNECTION, COM_DISP_E_BADPARAMCOUNT,
    COM_DISP_E_EXCEPTION, COM_DISP_E_MEMBERNOTFOUND, COM_DISP_E_PARAMNOTFOUND,
    COM_DISP_E_TYPEMISMATCH, COM_DISP_E_UNKNOWNNAME, COM_E_INVALIDARG, COM_E_NOINTERFACE,
    COM_E_NOTIMPL, COM_S_FALSE, COM_S_OK, IID_ICONNECTIONPOINT, IID_ICONNECTIONPOINTCONTAINER,
    IID_IDISPATCH, IID_IENUMVARIANT, IID_IUNKNOWN, IID_NULL, RawIConnectionPointContainerVtbl,
    RawIConnectionPointVtbl, RawIDispatch, RawIDispatchVtbl, RawIEnumVARIANT, RawIEnumVARIANTVtbl,
    RawIUnknown, RawIUnknownVtbl, add_ref_dispatch as raw_add_ref_dispatch, guid_equals,
    release_dispatch as raw_release_dispatch, release_unknown as raw_release_unknown,
};
use crate::windows_variant::{
    set_variant_from_com_value as com_set_variant_from_com_value,
    variant_to_com_value as com_variant_to_com_value,
};
use crate::{COM_DISPID_PROPERTYPUT, ComValue};
use std::{
    collections::BTreeMap,
    sync::Mutex,
    sync::atomic::{AtomicI32, AtomicU32, Ordering},
};
use windows_sys::Win32::{
    Foundation::{DECIMAL, SysAllocString, SysFreeString, VARIANT_BOOL},
    System::{
        Com::{
            CY, DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT,
            DISPATCH_PROPERTYPUTREF, DISPPARAMS, EXCEPINFO, SAFEARRAY, SAFEARRAYBOUND,
        },
        Ole::{
            SafeArrayCreate, SafeArrayCreateVector, SafeArrayDestroy, SafeArrayGetDim,
            SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayGetVartype,
            SafeArrayPutElement,
        },
        Variant::{
            VARIANT, VT_ARRAY, VT_BOOL, VT_BSTR, VT_BYREF, VT_DECIMAL, VT_DISPATCH, VT_EMPTY,
            VT_ERROR, VT_I1, VT_I2, VT_I4, VT_I8, VT_INT, VT_NULL, VT_UI1, VT_UI2, VT_UI4, VT_UI8,
            VT_UINT, VT_UNKNOWN, VT_VARIANT, VariantClear,
        },
    },
};

const VT_R4_VARENUM: u16 = 4;
const VT_R8_VARENUM: u16 = 5;
const VT_CY_VARENUM: u16 = 6;
const VT_DATE_VARENUM: u16 = 7;

type RawDispatchPtr = usize;
type RawUnknownPtr = usize;

pub const OXVBA_TEST_DISPATCH_PROGID: &str = "OxVba.TestDispatch";
pub const IID_OXVBA_TEST_DISPATCH_EVENTS: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0x1111_1112,
    data2: 0x2222,
    data3: 0x3333,
    data4: [0x44, 0x44, 0x55, 0x55, 0x55, 0x55, 0x55, 0x56],
};
pub const IID_OXVBA_TEST_DISPATCH_EVENTS_STR: &str = "11111112-2222-3333-4444-555555555556";
pub const IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS: windows_sys::core::GUID =
    windows_sys::core::GUID {
        data1: 0x1111_1113,
        data2: 0x2222,
        data3: 0x3333,
        data4: [0x44, 0x44, 0x55, 0x55, 0x55, 0x55, 0x55, 0x57],
    };
pub const IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS_STR: &str = "11111113-2222-3333-4444-555555555557";
pub const TEST_DISPID_NEWENUM: i32 = -4;
pub const TEST_DISPID_COUNT: i32 = 1;
pub const TEST_DISPID_EXISTS: i32 = 2;
pub const TEST_DISPID_FIRE_CHANGED: i32 = 3;
pub const TEST_DISPID_FIRE_CHANGED_PAIR: i32 = 4;
pub const TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE: i32 = 11;
pub const TEST_DISPID_PING: i32 = 5;
pub const TEST_DISPID_LOOKUP: i32 = 6;
pub const TEST_DISPID_SET_VALUE: i32 = 7;
pub const TEST_DISPID_SET_VALUE_REF: i32 = 8;
pub const TEST_DISPID_VALUE: i32 = 9;
pub const TEST_DISPID_EXCEL_QUIT: i32 = 10;
pub const TEST_DISPID_SUM_PAIR: i32 = 12;
pub const TEST_DISPID_LOOKUP_PAIR: i32 = 13;
pub const TEST_DISPID_SET_INDEXED_VALUE: i32 = 14;
pub const TEST_DISPID_SET_INDEXED_VALUE_REF: i32 = 15;
pub const TEST_DISPID_ECHO_VARIANT: i32 = 16;
pub const TEST_DISPID_RAISE_EXCEPTION: i32 = 17;
pub const TEST_DISPID_RAISE_PARAM_NOT_FOUND: i32 = 87;
pub const TEST_DISPID_RAISE_RICH_EXCEPTION: i32 = 88;
pub const TEST_DISPID_RETURN_SMALLINT: i32 = 18;
pub const TEST_DISPID_RETURN_UNSIGNED_WORD: i32 = 19;
pub const TEST_DISPID_RETURN_SMALLINT_ARRAY: i32 = 20;
pub const TEST_DISPID_RETURN_BOOL_ARRAY: i32 = 21;
pub const TEST_DISPID_RETURN_STRING_ARRAY: i32 = 22;
pub const TEST_DISPID_RETURN_SELF_DISPATCH: i32 = 23;
pub const TEST_DISPID_RETURN_SELF_UNKNOWN: i32 = 24;
pub const TEST_DISPID_CLASSIFY_VARIANT_ARG: i32 = 25;
pub const TEST_DISPID_CLASSIFY_VARIANT_ARRAY_FIRST_ELEMENT_ARG: i32 = 26;
pub const TEST_DISPID_RETURN_SELF_DISPATCH_ARRAY: i32 = 27;
pub const TEST_DISPID_RETURN_SELF_TYPED_DISPATCH_ARRAY: i32 = 28;
pub const TEST_DISPID_RETURN_SELF_TYPED_UNKNOWN_ARRAY: i32 = 29;
pub const TEST_DISPID_RETURN_SMALLINT_MATRIX: i32 = 30;
pub const TEST_DISPID_RETURN_PLAIN_UNKNOWN: i32 = 31;
pub const TEST_DISPID_RETURN_PLAIN_UNKNOWN_ARRAY: i32 = 32;
pub const TEST_DISPID_RETURN_LONG_ARRAY: i32 = 33;
pub const TEST_DISPID_RETURN_UNSIGNED_LONG_ARRAY: i32 = 34;
pub const TEST_DISPID_RETURN_LONG: i32 = 35;
pub const TEST_DISPID_RETURN_UNSIGNED_LONG: i32 = 36;
pub const TEST_DISPID_RETURN_BYTE: i32 = 37;
pub const TEST_DISPID_RETURN_BYTE_ARRAY: i32 = 38;
pub const TEST_DISPID_RETURN_SIGNED_BYTE: i32 = 39;
pub const TEST_DISPID_RETURN_SIGNED_BYTE_ARRAY: i32 = 40;
pub const TEST_DISPID_RETURN_PLATFORM_INT: i32 = 41;
pub const TEST_DISPID_RETURN_PLATFORM_UINT: i32 = 42;
pub const TEST_DISPID_RETURN_PLATFORM_INT_ARRAY: i32 = 43;
pub const TEST_DISPID_RETURN_PLATFORM_UINT_ARRAY: i32 = 44;
pub const TEST_DISPID_RETURN_HYPER: i32 = 45;
pub const TEST_DISPID_RETURN_UNSIGNED_HYPER: i32 = 46;
pub const TEST_DISPID_RETURN_HYPER_ARRAY: i32 = 47;
pub const TEST_DISPID_RETURN_UNSIGNED_HYPER_ARRAY: i32 = 48;
pub const TEST_DISPID_RETURN_DOUBLE: i32 = 49;
pub const TEST_DISPID_RETURN_DOUBLE_ARRAY: i32 = 50;
pub const TEST_DISPID_RETURN_SINGLE: i32 = 51;
pub const TEST_DISPID_RETURN_SINGLE_ARRAY: i32 = 52;
pub const TEST_DISPID_RETURN_DATE: i32 = 53;
pub const TEST_DISPID_RETURN_DATE_ARRAY: i32 = 54;
pub const TEST_DISPID_RETURN_CURRENCY: i32 = 55;
pub const TEST_DISPID_RETURN_CURRENCY_ARRAY: i32 = 56;
pub const TEST_DISPID_RETURN_DECIMAL: i32 = 57;
pub const TEST_DISPID_RETURN_DECIMAL_ARRAY: i32 = 58;
pub const TEST_DISPID_RETURN_WIDE_UNSIGNED_LONG: i32 = 59;
pub const TEST_DISPID_RETURN_WIDE_UNSIGNED_LONG_ARRAY: i32 = 60;
pub const TEST_DISPID_RETURN_WIDE_PLATFORM_UINT: i32 = 61;
pub const TEST_DISPID_RETURN_WIDE_PLATFORM_UINT_ARRAY: i32 = 62;
pub const TEST_DISPID_RETURN_BOOL: i32 = 63;
pub const TEST_DISPID_RETURN_STRING: i32 = 64;
pub const TEST_DISPID_RETURN_EMPTY: i32 = 65;
pub const TEST_DISPID_RETURN_NULL: i32 = 66;
pub const TEST_DISPID_RETURN_ERROR: i32 = 67;
pub const TEST_DISPID_RETURN_BYREF_LONG: i32 = 68;
pub const TEST_DISPID_RETURN_BYREF_LONG_ARRAY: i32 = 69;
pub const TEST_DISPID_RETURN_WIDE_HYPER: i32 = 70;
pub const TEST_DISPID_RETURN_WIDE_HYPER_ARRAY: i32 = 71;
pub const TEST_DISPID_RETURN_WIDE_UNSIGNED_HYPER: i32 = 72;
pub const TEST_DISPID_RETURN_WIDE_UNSIGNED_HYPER_ARRAY: i32 = 73;
pub const TEST_DISPID_RETURN_VARIANT_MATRIX: i32 = 74;
pub const TEST_DISPID_RETURN_PLAIN_UNKNOWN_VARIANT_ARRAY: i32 = 75;
pub const TEST_DISPID_RETURN_MISSING_MEMBER_NAME: i32 = 76;
pub const TEST_DISPID_RETURN_PING_MEMBER_NAME: i32 = 77;
pub const TEST_DISPID_RETURN_LOOKUP_MEMBER_NAME: i32 = 78;
pub const TEST_DISPID_RETURN_SUM_PAIR_MEMBER_NAME: i32 = 79;
pub const TEST_DISPID_RETURN_LOOKUP_PAIR_MEMBER_NAME: i32 = 80;
pub const TEST_DISPID_RETURN_SET_VALUE_MEMBER_NAME: i32 = 81;
pub const TEST_DISPID_RETURN_SET_VALUE_REF_MEMBER_NAME: i32 = 82;
pub const TEST_DISPID_RETURN_SET_INDEXED_VALUE_MEMBER_NAME: i32 = 83;
pub const TEST_DISPID_RETURN_SET_INDEXED_VALUE_REF_MEMBER_NAME: i32 = 84;
pub const TEST_DISPID_RETURN_VALUE_MEMBER_NAME: i32 = 85;
pub const TEST_DISPID_RETURN_DEFAULT_MEMBER_NAME: i32 = 86;
pub const TEST_NAMED_DISPID_LHS: i32 = 101;

static mut TEST_BYREF_I32_RESULT: i32 = 321;
static mut TEST_BYREF_I32_ARRAY_RESULT: *mut SAFEARRAY = std::ptr::null_mut();
pub const TEST_NAMED_DISPID_RHS: i32 = 102;
pub const TEST_NAMED_DISPID_INDEX: i32 = 103;
pub const TEST_NAMED_DISPID_VALUE: i32 = 104;
pub const TEST_EVENT_CHANGED: i32 = 1;
pub const TEST_EVENT_CHANGED_PAIR: i32 = 3;
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn alloc_bstr(text: &str) -> windows_sys::core::BSTR {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    SysAllocString(wide.as_ptr())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn populate_excepinfo(excep: *mut EXCEPINFO, source: &str, description: &str, scode: i32) {
    if excep.is_null() {
        return;
    }
    (*excep).bstrSource = alloc_bstr(source);
    (*excep).bstrDescription = alloc_bstr(description);
    (*excep).scode = scode;
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn populate_rich_excepinfo(
    excep: *mut EXCEPINFO,
    source: &str,
    description: &str,
    help_file: &str,
    help_context: u32,
    scode: i32,
    wcode: u16,
) {
    if excep.is_null() {
        return;
    }
    (*excep).wCode = wcode;
    (*excep).bstrSource = alloc_bstr(source);
    (*excep).bstrDescription = alloc_bstr(description);
    (*excep).bstrHelpFile = alloc_bstr(help_file);
    (*excep).dwHelpContext = help_context;
    (*excep).scode = scode;
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_i16_array(values: &[i16], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_I2, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_I2) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const i16).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_I2) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_I2;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_i32_array(values: &[i32], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_I4, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_I4) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const i32).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_I4) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_I4;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_u32_array(values: &[u32], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_UI4, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_UI4) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const u32).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_UI4) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_UI4;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_u8_array(values: &[u8], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_UI1, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_UI1) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const u8).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_UI1) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_UI1;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_i8_array(values: &[i8], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_I1, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_I1) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const i8).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_I1) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_I1;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_platform_i32_array(
    values: &[i32],
    variant: *mut VARIANT,
) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_INT, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_INT) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const i32).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_INT) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_INT;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_platform_u32_array(
    values: &[u32],
    variant: *mut VARIANT,
) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_UINT, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_UINT) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const u32).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_UINT) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_UINT;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_i64_array(values: &[i64], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_I8, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_I8) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const i64).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_I8) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_I8;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_f64_array(values: &[f64], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_R8_VARENUM, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_R8_VARENUM) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const f64).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_R8_VARENUM) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_R8_VARENUM;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_f32_array(values: &[f32], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_R4_VARENUM, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_R4_VARENUM) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const f32).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_R4_VARENUM) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_R4_VARENUM;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_date_array(values: &[f64], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_DATE_VARENUM, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_DATE_VARENUM) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const f64).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_DATE_VARENUM) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_DATE_VARENUM;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_currency_array(values: &[i64], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_CY_VARENUM, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_CY_VARENUM) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let element = CY { int64: *value };
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (&element as *const CY).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_CY_VARENUM) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_CY_VARENUM;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn decimal_from_parts(lo: u32, mid: u32, hi: u32, scale: u8, negative: bool) -> DECIMAL {
    let mut decimal: DECIMAL = std::mem::zeroed();
    decimal.wReserved = 0;
    decimal.Anonymous1.Anonymous.scale = scale;
    decimal.Anonymous1.Anonymous.sign = if negative { 0x80 } else { 0 };
    decimal.Hi32 = hi;
    decimal.Anonymous2.Anonymous.Lo32 = lo;
    decimal.Anonymous2.Anonymous.Mid32 = mid;
    decimal
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_decimal_array(
    values: &[DECIMAL],
    variant: *mut VARIANT,
) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_DECIMAL, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_DECIMAL) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const DECIMAL).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_DECIMAL) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_DECIMAL;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}
unsafe fn set_variant_u64_array(values: &[u64], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_UI8, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_UI8) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const u64).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_UI8) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_UI8;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_i16_matrix(variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let bounds = [
        SAFEARRAYBOUND {
            cElements: 2,
            lLbound: 0,
        },
        SAFEARRAYBOUND {
            cElements: 2,
            lLbound: 0,
        },
    ];
    let psa = SafeArrayCreate(VT_I2, 2, bounds.as_ptr());
    if psa.is_null() {
        return Err("SafeArrayCreate(VT_I2, rank=2) returned null".to_string());
    }
    let values = [[1i16, 2i16], [3i16, 4i16]];
    for row in 0..2 {
        for col in 0..2 {
            let indices = [row, col];
            let hr = SafeArrayPutElement(
                psa.cast_const(),
                indices.as_ptr(),
                (&values[row as usize][col as usize] as *const i16).cast(),
            );
            if hr < 0 {
                let _ = SafeArrayDestroy(psa.cast_const());
                return Err(format!(
                    "SafeArrayPutElement(VT_I2 rank=2) failed with HRESULT {:#010X} at [{}, {}]",
                    hr as u32, row, col
                ));
            }
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_I2;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_variant_matrix(variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let bounds = [
        SAFEARRAYBOUND {
            cElements: 2,
            lLbound: 0,
        },
        SAFEARRAYBOUND {
            cElements: 2,
            lLbound: 0,
        },
    ];
    let psa = SafeArrayCreate(VT_VARIANT, 2, bounds.as_ptr());
    if psa.is_null() {
        return Err("SafeArrayCreate(VT_VARIANT, rank=2) returned null".to_string());
    }
    let values = [[1i32, 2i32], [3i32, 4i32]];
    for row in 0..2 {
        for col in 0..2 {
            let indices = [row, col];
            let mut element: VARIANT = std::mem::zeroed();
            set_variant_i32(values[row as usize][col as usize], &mut element);
            let hr = SafeArrayPutElement(
                psa.cast_const(),
                indices.as_ptr(),
                (&element as *const VARIANT).cast(),
            );
            let _ = VariantClear(&mut element);
            if hr < 0 {
                let _ = SafeArrayDestroy(psa.cast_const());
                return Err(format!(
                    "SafeArrayPutElement(VT_VARIANT rank=2) failed with HRESULT {:#010X} at [{}, {}]",
                    hr as u32, row, col
                ));
            }
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_VARIANT;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

unsafe fn set_variant_bool_array(values: &[bool], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_BOOL, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_BOOL) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let raw: VARIANT_BOOL = if *value { -1 } else { 0 };
        let hr = SafeArrayPutElement(
            psa.cast_const(),
            &index,
            (&raw as *const VARIANT_BOOL).cast(),
        );
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_BOOL) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_BOOL;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_bstr_array(values: &[&str], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_BSTR, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_BSTR) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let bstr = alloc_bstr(value);
        if bstr.is_null() {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err("SysAllocString returned null for VT_BSTR SAFEARRAY element".to_string());
        }
        let hr = SafeArrayPutElement(psa.cast_const(), &index, bstr.cast());
        SysFreeString(bstr);
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_BSTR) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_BSTR;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn map_com_hresult_label(hresult: Option<u32>, arg_err: Option<u32>) -> &'static str {
    if arg_err.is_some() {
        return "arg-error";
    }
    match hresult {
        Some(0x8004_0154) => "class-not-registered",
        Some(0x8004_01F3) => "invalid-class-string",
        Some(0x8000_4002) => "no-interface",
        Some(0x8002_0003) => "member-not-found",
        Some(0x8002_0004) => "param-not-found",
        Some(0x8002_0006) => "unknown-name",
        Some(0x8002_0005) => "type-mismatch",
        Some(0x8002_000E) => "bad-param-count",
        Some(0x8002_0009) => "exception-raised",
        Some(0x8007_0057) => "invalid-argument",
        Some(_) => "native-failure",
        None => "fault-unspecified",
    }
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaTestDispatchInterface {
    vtbl: *const RawIDispatchVtbl,
    owner: *mut OxvbaTestDispatchObject,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaTestConnectionPointContainerInterface {
    vtbl: *const RawIConnectionPointContainerVtbl,
    owner: *mut OxvbaTestDispatchObject,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaTestConnectionPointInterface {
    vtbl: *const RawIConnectionPointVtbl,
    owner: *mut OxvbaTestDispatchObject,
    kind: OxvbaTestConnectionPointKind,
}

#[cfg(target_os = "windows")]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OxvbaTestConnectionPointKind {
    Dispatch = 1,
    SourceInterface = 2,
}

#[cfg(target_os = "windows")]
struct OxvbaTestDispatchObject {
    dispatch: OxvbaTestDispatchInterface,
    connection_point_container: OxvbaTestConnectionPointContainerInterface,
    dispatch_connection_point: OxvbaTestConnectionPointInterface,
    source_connection_point: OxvbaTestConnectionPointInterface,
    value_state: AtomicI32,
    ref_count: AtomicU32,
    next_cookie: AtomicU32,
    dispatch_sinks: Mutex<BTreeMap<u32, RawDispatchPtr>>,
    source_interface_sinks: Mutex<BTreeMap<u32, RawUnknownPtr>>,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaTestPlainUnknown {
    unknown: RawIUnknown,
    ref_count: AtomicU32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaTestEnumVariant {
    enum_variant: RawIEnumVARIANT,
    ref_count: AtomicU32,
    next_index: AtomicU32,
}
#[cfg(target_os = "windows")]
#[repr(C)]
struct RawOxvbaTestDispatchSourceEventsVtbl {
    unknown: RawIUnknownVtbl,
    changed: unsafe extern "system" fn(this: *mut core::ffi::c_void, value: i32) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawOxvbaTestDispatchSourceEvents {
    vtbl: *const RawOxvbaTestDispatchSourceEventsVtbl,
}

#[cfg(target_os = "windows")]
static OXVBA_TEST_PLAIN_UNKNOWN_VTBL: RawIUnknownVtbl = RawIUnknownVtbl {
    query_interface: oxvba_test_plain_unknown_query_interface,
    add_ref: oxvba_test_plain_unknown_add_ref,
    release: oxvba_test_plain_unknown_release,
};

#[cfg(target_os = "windows")]
static OXVBA_TEST_ENUMVARIANT_VTBL: RawIEnumVARIANTVtbl = RawIEnumVARIANTVtbl {
    unknown: RawIUnknownVtbl {
        query_interface: oxvba_test_enumvariant_query_interface,
        add_ref: oxvba_test_enumvariant_add_ref,
        release: oxvba_test_enumvariant_release,
    },
    next: oxvba_test_enumvariant_next,
    skip: oxvba_test_enumvariant_skip,
    reset: oxvba_test_enumvariant_reset,
    clone: oxvba_test_enumvariant_clone,
};
#[cfg(target_os = "windows")]
static OXVBA_TEST_DISPATCH_VTBL: RawIDispatchVtbl = RawIDispatchVtbl {
    unknown: RawIUnknownVtbl {
        query_interface: oxvba_test_query_interface,
        add_ref: oxvba_test_add_ref,
        release: oxvba_test_release,
    },
    get_type_info_count: oxvba_test_get_type_info_count,
    get_type_info: oxvba_test_get_type_info,
    get_ids_of_names: oxvba_test_get_ids_of_names,
    invoke: oxvba_test_invoke,
};

#[cfg(target_os = "windows")]
static OXVBA_TEST_CONNECTIONPOINTCONTAINER_VTBL: RawIConnectionPointContainerVtbl =
    RawIConnectionPointContainerVtbl {
        unknown: RawIUnknownVtbl {
            query_interface: oxvba_test_connection_point_container_query_interface,
            add_ref: oxvba_test_connection_point_container_add_ref,
            release: oxvba_test_connection_point_container_release,
        },
        enum_connection_points: oxvba_test_enum_connection_points,
        find_connection_point: oxvba_test_find_connection_point,
    };

#[cfg(target_os = "windows")]
static OXVBA_TEST_CONNECTIONPOINT_VTBL: RawIConnectionPointVtbl = RawIConnectionPointVtbl {
    unknown: RawIUnknownVtbl {
        query_interface: oxvba_test_connection_point_query_interface,
        add_ref: oxvba_test_connection_point_add_ref,
        release: oxvba_test_connection_point_release,
    },
    get_connection_interface: oxvba_test_get_connection_interface,
    get_connection_point_container: oxvba_test_get_connection_point_container,
    advise: oxvba_test_advise,
    unadvise: oxvba_test_unadvise,
    enum_connections: oxvba_test_enum_connections,
};

#[cfg(target_os = "windows")]
pub fn create_oxvba_test_dispatch() -> *mut RawIDispatch {
    let mut object = Box::new(OxvbaTestDispatchObject {
        dispatch: OxvbaTestDispatchInterface {
            vtbl: &OXVBA_TEST_DISPATCH_VTBL,
            owner: std::ptr::null_mut(),
        },
        connection_point_container: OxvbaTestConnectionPointContainerInterface {
            vtbl: &OXVBA_TEST_CONNECTIONPOINTCONTAINER_VTBL,
            owner: std::ptr::null_mut(),
        },
        dispatch_connection_point: OxvbaTestConnectionPointInterface {
            vtbl: &OXVBA_TEST_CONNECTIONPOINT_VTBL,
            owner: std::ptr::null_mut(),
            kind: OxvbaTestConnectionPointKind::Dispatch,
        },
        source_connection_point: OxvbaTestConnectionPointInterface {
            vtbl: &OXVBA_TEST_CONNECTIONPOINT_VTBL,
            owner: std::ptr::null_mut(),
            kind: OxvbaTestConnectionPointKind::SourceInterface,
        },
        value_state: AtomicI32::new(0),
        ref_count: AtomicU32::new(1),
        next_cookie: AtomicU32::new(0),
        dispatch_sinks: Mutex::new(BTreeMap::new()),
        source_interface_sinks: Mutex::new(BTreeMap::new()),
    });
    let object_ptr: *mut OxvbaTestDispatchObject = &mut *object;
    object.dispatch.owner = object_ptr;
    object.connection_point_container.owner = object_ptr;
    object.dispatch_connection_point.owner = object_ptr;
    object.source_connection_point.owner = object_ptr;
    let dispatch_ptr =
        (&mut object.dispatch as *mut OxvbaTestDispatchInterface).cast::<RawIDispatch>();
    let _ = Box::into_raw(object);
    dispatch_ptr
}

#[cfg(target_os = "windows")]
pub fn create_oxvba_test_plain_unknown() -> *mut RawIUnknown {
    let mut object = Box::new(OxvbaTestPlainUnknown {
        unknown: RawIUnknown {
            vtbl: &OXVBA_TEST_PLAIN_UNKNOWN_VTBL,
        },
        ref_count: AtomicU32::new(1),
    });
    let unknown_ptr = (&mut object.unknown as *mut RawIUnknown).cast::<RawIUnknown>();
    let _ = Box::into_raw(object);
    unknown_ptr
}

#[cfg(target_os = "windows")]
pub fn create_oxvba_test_enum_unknown() -> *mut RawIUnknown {
    let mut object = Box::new(OxvbaTestEnumVariant {
        enum_variant: RawIEnumVARIANT {
            vtbl: &OXVBA_TEST_ENUMVARIANT_VTBL,
        },
        ref_count: AtomicU32::new(1),
        next_index: AtomicU32::new(0),
    });
    let unknown_ptr = (&mut object.enum_variant as *mut RawIEnumVARIANT).cast::<RawIUnknown>();
    let _ = Box::into_raw(object);
    unknown_ptr
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_test_plain_unknown(this: *mut core::ffi::c_void) -> *mut OxvbaTestPlainUnknown {
    this.cast::<OxvbaTestPlainUnknown>()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_test_enumvariant(this: *mut core::ffi::c_void) -> *mut OxvbaTestEnumVariant {
    this.cast::<OxvbaTestEnumVariant>()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_plain_unknown_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    if riid.is_null() {
        return COM_E_NOINTERFACE;
    }
    if guid_equals(riid, &IID_IUNKNOWN) {
        *ppv = this;
        let _ = oxvba_test_plain_unknown_add_ref(this);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_plain_unknown_add_ref(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_plain_unknown(this);
    (*owner).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_plain_unknown_release(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_plain_unknown(this);
    let remaining = (*owner).ref_count.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        drop(Box::from_raw(owner));
    }
    remaining
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_enumvariant_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    if riid.is_null() {
        return COM_E_NOINTERFACE;
    }
    if guid_equals(riid, &IID_IUNKNOWN) || guid_equals(riid, &IID_IENUMVARIANT) {
        *ppv = this;
        let _ = oxvba_test_enumvariant_add_ref(this);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_enumvariant_add_ref(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_enumvariant(this);
    (*owner).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_enumvariant_release(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_enumvariant(this);
    let remaining = (*owner).ref_count.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        drop(Box::from_raw(owner));
    }
    remaining
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_enumvariant_next(
    this: *mut core::ffi::c_void,
    celt: u32,
    rgvar: *mut VARIANT,
    pcelt_fetched: *mut u32,
) -> i32 {
    const VALUES: [i32; 2] = [41, 42];

    if celt > 1 && pcelt_fetched.is_null() {
        return COM_E_INVALIDARG;
    }
    if celt != 0 && rgvar.is_null() {
        return COM_E_INVALIDARG;
    }
    if !pcelt_fetched.is_null() {
        *pcelt_fetched = 0;
    }

    let owner = as_oxvba_test_enumvariant(this);
    let mut index = (*owner).next_index.load(Ordering::Acquire) as usize;
    let requested = celt as usize;
    let mut fetched = 0usize;
    while fetched < requested && index < VALUES.len() {
        let slot = rgvar.add(fetched);
        *slot = std::mem::zeroed();
        set_variant_i32(VALUES[index], slot);
        fetched += 1;
        index += 1;
    }
    (*owner).next_index.store(index as u32, Ordering::Release);
    if !pcelt_fetched.is_null() {
        *pcelt_fetched = fetched as u32;
    }
    if fetched == requested {
        COM_S_OK
    } else {
        COM_S_FALSE
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_enumvariant_skip(
    this: *mut core::ffi::c_void,
    celt: u32,
) -> i32 {
    const VALUES_LEN: usize = 2;

    let owner = as_oxvba_test_enumvariant(this);
    let current = (*owner).next_index.load(Ordering::Acquire) as usize;
    let requested_next = current.saturating_add(celt as usize);
    let next = requested_next.min(VALUES_LEN);
    (*owner).next_index.store(next as u32, Ordering::Release);
    if next == requested_next {
        COM_S_OK
    } else {
        COM_S_FALSE
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_enumvariant_reset(this: *mut core::ffi::c_void) -> i32 {
    let owner = as_oxvba_test_enumvariant(this);
    (*owner).next_index.store(0, Ordering::Release);
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_enumvariant_clone(
    this: *mut core::ffi::c_void,
    ppenum: *mut *mut core::ffi::c_void,
) -> i32 {
    if ppenum.is_null() {
        return COM_E_INVALIDARG;
    }
    let owner = as_oxvba_test_enumvariant(this);
    let mut clone = Box::new(OxvbaTestEnumVariant {
        enum_variant: RawIEnumVARIANT {
            vtbl: &OXVBA_TEST_ENUMVARIANT_VTBL,
        },
        ref_count: AtomicU32::new(1),
        next_index: AtomicU32::new((*owner).next_index.load(Ordering::Acquire)),
    });
    *ppenum = (&mut clone.enum_variant as *mut RawIEnumVARIANT).cast();
    let _ = Box::into_raw(clone);
    COM_S_OK
}
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn read_utf16_z(ptr: *const u16) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len = len.saturating_add(1);
    }
    let slice = std::slice::from_raw_parts(ptr, len);
    String::from_utf16(slice).ok()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_test_dispatch_owner_from_dispatch(
    this: *mut core::ffi::c_void,
) -> *mut OxvbaTestDispatchObject {
    let iface = this.cast::<OxvbaTestDispatchInterface>();
    (*iface).owner
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_test_dispatch_owner_from_connection_point_container(
    this: *mut core::ffi::c_void,
) -> *mut OxvbaTestDispatchObject {
    let iface = this.cast::<OxvbaTestConnectionPointContainerInterface>();
    (*iface).owner
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_test_dispatch_owner_from_connection_point(
    this: *mut core::ffi::c_void,
) -> *mut OxvbaTestDispatchObject {
    let iface = this.cast::<OxvbaTestConnectionPointInterface>();
    (*iface).owner
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_connection_point_kind(
    this: *mut core::ffi::c_void,
) -> OxvbaTestConnectionPointKind {
    let iface = this.cast::<OxvbaTestConnectionPointInterface>();
    (*iface).kind
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_owner_add_ref(owner: *mut OxvbaTestDispatchObject) -> u32 {
    (*owner).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_owner_release(owner: *mut OxvbaTestDispatchObject) -> u32 {
    let prev = (*owner).ref_count.fetch_sub(1, Ordering::AcqRel);
    let next = prev.saturating_sub(1);
    if next == 0 {
        let mut dispatch_sinks = match (*owner).dispatch_sinks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let retained_dispatch: Vec<RawDispatchPtr> = dispatch_sinks.values().copied().collect();
        dispatch_sinks.clear();
        drop(dispatch_sinks);
        for sink in retained_dispatch {
            raw_release_dispatch(sink as *mut RawIDispatch);
        }
        let mut source_interface_sinks = match (*owner).source_interface_sinks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let retained_source: Vec<RawUnknownPtr> =
            source_interface_sinks.values().copied().collect();
        source_interface_sinks.clear();
        drop(source_interface_sinks);
        for sink in retained_source {
            raw_release_unknown(sink as *mut core::ffi::c_void);
        }
        drop(Box::from_raw(owner));
    }
    next
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_owner_query_interface(
    owner: *mut OxvbaTestDispatchObject,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    if guid_equals(riid, &IID_IUNKNOWN) || guid_equals(riid, &IID_IDISPATCH) {
        *ppv = (&mut (*owner).dispatch as *mut OxvbaTestDispatchInterface).cast();
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    if guid_equals(riid, &IID_ICONNECTIONPOINTCONTAINER) {
        *ppv = (&mut (*owner).connection_point_container
            as *mut OxvbaTestConnectionPointContainerInterface)
            .cast();
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    if guid_equals(riid, &IID_ICONNECTIONPOINT) {
        *ppv = (&mut (*owner).dispatch_connection_point as *mut OxvbaTestConnectionPointInterface)
            .cast();
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_fire_event(
    owner: *mut OxvbaTestDispatchObject,
    dispid: i32,
    args: &[i32],
) -> i32 {
    let sinks: Vec<RawDispatchPtr> = {
        let sinks = match (*owner).dispatch_sinks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        sinks.values().copied().collect()
    };
    for sink in sinks {
        let dispatch = sink as *mut RawIDispatch;
        let _ = raw_dispatch_invoke_event(dispatch, dispid, args);
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_fire_source_interface_event(
    owner: *mut OxvbaTestDispatchObject,
    value: i32,
) -> i32 {
    let sinks: Vec<RawUnknownPtr> = {
        let sinks = match (*owner).source_interface_sinks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        sinks.values().copied().collect()
    };
    for sink in sinks {
        let source = sink as *mut RawOxvbaTestDispatchSourceEvents;
        if source.is_null() {
            continue;
        }
        let _ = ((*(*source).vtbl).changed)(source.cast(), value);
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_invoke_event(
    dispatch: *mut RawIDispatch,
    dispid: i32,
    args: &[i32],
) -> Result<(), String> {
    if dispatch.is_null() {
        return Err("event sink dispatch pointer is null".to_string());
    }
    // Build the event-call DISPPARAMS the way a plain positional IDispatch caller does:
    // rgvarg in reverse order (`rgvarg[0]` is the last declared argument). Excel's
    // multi-arg events can instead use named DISPIDs, which the sink maps through
    // rgdispidNamedArgs before applying this positional rule.
    let mut variants: Vec<VARIANT> = vec![std::mem::zeroed(); args.len()];
    for (idx, value) in args.iter().enumerate() {
        let slot = args.len().saturating_sub(1).saturating_sub(idx);
        variants[slot].Anonymous.Anonymous.vt = VT_I4;
        variants[slot].Anonymous.Anonymous.Anonymous.lVal = *value;
    }
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = 0u32;
    let mut params = DISPPARAMS {
        rgvarg: if variants.is_empty() {
            std::ptr::null_mut()
        } else {
            variants.as_mut_ptr()
        },
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: u32::try_from(variants.len()).unwrap_or(u32::MAX),
        cNamedArgs: 0,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        DISPATCH_METHOD,
        &mut params,
        std::ptr::null_mut(),
        &mut excep,
        &mut arg_err,
    );
    if hr < 0 {
        return Err(format!(
            "IDispatch::Invoke(event dispid={dispid}) failed with HRESULT {:#010X} (arg_err={arg_err})",
            hr as u32
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_token_from_invoke_arg(
    variant: *const VARIANT,
    arg_index: usize,
) -> Result<i32, i32> {
    if variant.is_null() {
        return Err(COM_DISP_E_TYPEMISMATCH);
    }
    match (*variant).Anonymous.Anonymous.vt {
        VT_I2 => Ok((*variant).Anonymous.Anonymous.Anonymous.iVal as i32),
        VT_I4 => Ok((*variant).Anonymous.Anonymous.Anonymous.lVal),
        VT_UI2 => Ok((*variant).Anonymous.Anonymous.Anonymous.uiVal as i32),
        VT_UI4 => Ok((*variant).Anonymous.Anonymous.Anonymous.ulVal as i32),
        VT_BOOL => Ok(if (*variant).Anonymous.Anonymous.Anonymous.boolVal == 0 {
            0
        } else {
            1
        }),
        VT_NULL => Ok(0),
        VT_ERROR => Ok((*variant).Anonymous.Anonymous.Anonymous.scode),
        VT_EMPTY if arg_index == 0 => Ok(0),
        _ => Err(COM_DISP_E_TYPEMISMATCH),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_property_put_token_from_invoke_arg(
    variant: *const VARIANT,
    arg_index: usize,
) -> Result<i32, i32> {
    if variant.is_null() {
        return Err(COM_DISP_E_TYPEMISMATCH);
    }
    if (*variant).Anonymous.Anonymous.vt == VT_DISPATCH {
        let dispatch = (*variant).Anonymous.Anonymous.Anonymous.pdispVal;
        if dispatch.is_null() {
            return Err(COM_DISP_E_TYPEMISMATCH);
        }
        return raw_oxvba_test_dispatch_vtable_invoke(dispatch.cast(), TEST_DISPID_COUNT, &[])
            .map_err(|_| COM_DISP_E_TYPEMISMATCH)?
            .ok_or(COM_DISP_E_TYPEMISMATCH);
    }
    raw_variant_token_from_invoke_arg(variant, arg_index)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_value_from_invoke_arg(
    variant: *const VARIANT,
    arg_index: usize,
) -> Result<ComValue, i32> {
    if variant.is_null() {
        return Err(COM_DISP_E_TYPEMISMATCH);
    }
    match raw_variant_to_com_value(&*variant) {
        Ok(value) => Ok(value),
        Err(_) if (*variant).Anonymous.Anonymous.vt == VT_EMPTY && arg_index == 0 => {
            Ok(ComValue::I32(0))
        }
        Err(_) => Err(COM_DISP_E_TYPEMISMATCH),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_token_from_dispparams(
    pparams: *mut DISPPARAMS,
    logical_index: usize,
    puargerr: *mut u32,
) -> Result<i32, i32> {
    if pparams.is_null() {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let params = &*pparams;
    let cargs = params.cArgs as usize;
    if logical_index >= cargs || params.rgvarg.is_null() {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let raw_index = cargs - 1 - logical_index;
    let arg = params.rgvarg.add(raw_index);
    match raw_variant_token_from_invoke_arg(arg, logical_index) {
        Ok(value) => Ok(value),
        Err(hr) => {
            if !puargerr.is_null() {
                *puargerr = raw_index as u32;
            }
            Err(hr)
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_value_from_dispparams(
    pparams: *mut DISPPARAMS,
    logical_index: usize,
    puargerr: *mut u32,
) -> Result<ComValue, i32> {
    if pparams.is_null() {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let params = &*pparams;
    let cargs = params.cArgs as usize;
    if logical_index >= cargs || params.rgvarg.is_null() {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let raw_index = cargs - 1 - logical_index;
    let arg = params.rgvarg.add(raw_index);
    match raw_variant_value_from_invoke_arg(arg, logical_index) {
        Ok(value) => Ok(value),
        Err(hr) => {
            if !puargerr.is_null() {
                *puargerr = raw_index as u32;
            }
            Err(hr)
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_token_from_named_dispid(
    pparams: *mut DISPPARAMS,
    named_dispid: i32,
    puargerr: *mut u32,
) -> Option<Result<i32, i32>> {
    if pparams.is_null() {
        return Some(Err(COM_DISP_E_BADPARAMCOUNT));
    }
    let params = &*pparams;
    if params.cNamedArgs == 0 || params.rgdispidNamedArgs.is_null() || params.rgvarg.is_null() {
        return None;
    }
    for raw_index in 0..params.cNamedArgs as usize {
        if *params.rgdispidNamedArgs.add(raw_index) != named_dispid {
            continue;
        }
        let arg = params.rgvarg.add(raw_index);
        return Some(match raw_variant_token_from_invoke_arg(arg, raw_index) {
            Ok(value) => Ok(value),
            Err(hr) => {
                if !puargerr.is_null() {
                    *puargerr = raw_index as u32;
                }
                Err(hr)
            }
        });
    }
    None
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_property_put_args_from_params(
    pparams: *mut DISPPARAMS,
    expected_count: usize,
    puargerr: *mut u32,
) -> Result<Vec<i32>, i32> {
    if pparams.is_null() {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let params = &*pparams;
    let cargs = params.cArgs as usize;
    if params.cArgs != expected_count as u32
        || params.cNamedArgs == 0
        || params.rgvarg.is_null()
        || params.rgdispidNamedArgs.is_null()
    {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let mut found_property_put = false;
    for raw_index in 0..params.cNamedArgs as usize {
        if *params.rgdispidNamedArgs.add(raw_index) == COM_DISPID_PROPERTYPUT {
            found_property_put = true;
            break;
        }
    }
    if !found_property_put {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let mut values = Vec::with_capacity(expected_count);
    for logical_index in 0..expected_count {
        let raw_index = cargs - 1 - logical_index;
        let arg = params.rgvarg.add(raw_index);
        match raw_variant_property_put_token_from_invoke_arg(arg, logical_index) {
            Ok(value) => values.push(value),
            Err(hr) => {
                if !puargerr.is_null() {
                    *puargerr = raw_index as u32;
                }
                return Err(hr);
            }
        }
    }
    Ok(values)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_property_put_i4_from_params(
    pparams: *mut DISPPARAMS,
    puargerr: *mut u32,
) -> Result<i32, i32> {
    raw_property_put_args_from_params(pparams, 1, puargerr).map(|args| args[0])
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_i32(value: i32, result: *mut VARIANT) {
    if result.is_null() {
        return;
    }
    (*result).Anonymous.Anonymous.vt = VT_I4;
    (*result).Anonymous.Anonymous.Anonymous.lVal = value;
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_i32_byref(result: *mut VARIANT) {
    if result.is_null() {
        return;
    }
    (*result).Anonymous.Anonymous.vt = VT_BYREF | VT_I4;
    (*result).Anonymous.Anonymous.Anonymous.plVal = &raw mut TEST_BYREF_I32_RESULT;
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_i32_array_byref(result: *mut VARIANT) -> Result<(), String> {
    if TEST_BYREF_I32_ARRAY_RESULT.is_null() {
        let psa = SafeArrayCreateVector(VT_I4, 0, 3);
        if psa.is_null() {
            return Err(
                "SafeArrayCreateVector(VT_I4) returned null for VT_BYREF array".to_string(),
            );
        }
        for (offset, value) in [12i32, -4i32, 321i32].into_iter().enumerate() {
            let index = i32::try_from(offset)
                .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
            let hr = SafeArrayPutElement(psa.cast_const(), &index, (&value as *const i32).cast());
            if hr < 0 {
                let _ = SafeArrayDestroy(psa.cast_const());
                return Err(format!(
                    "SafeArrayPutElement(VT_I4) failed with HRESULT {:#010X} at index {}",
                    hr as u32, offset
                ));
            }
        }
        TEST_BYREF_I32_ARRAY_RESULT = psa;
    }
    if result.is_null() {
        return Ok(());
    }
    (*result).Anonymous.Anonymous.vt = VT_BYREF | VT_ARRAY | VT_I4;
    (*result).Anonymous.Anonymous.Anonymous.pparray = &raw mut TEST_BYREF_I32_ARRAY_RESULT;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_bool(value: bool, result: *mut VARIANT) {
    if result.is_null() {
        return;
    }
    (*result).Anonymous.Anonymous.vt = VT_BOOL;
    (*result).Anonymous.Anonymous.Anonymous.boolVal = if value { -1 } else { 0 };
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
    oxvba_test_owner_query_interface(owner, riid, ppv)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_add_ref(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
    oxvba_test_owner_add_ref(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_release(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
    oxvba_test_owner_release(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_container_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point_container(this);
    oxvba_test_owner_query_interface(owner, riid, ppv)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_container_add_ref(
    this: *mut core::ffi::c_void,
) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point_container(this);
    oxvba_test_owner_add_ref(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_container_release(
    this: *mut core::ffi::c_void,
) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point_container(this);
    oxvba_test_owner_release(owner)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn oxvba_test_enum_connection_points(
    _this: *mut core::ffi::c_void,
    _pp_enum: *mut *mut core::ffi::c_void,
) -> i32 {
    COM_E_NOTIMPL
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_find_connection_point(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    pp_cp: *mut *mut core::ffi::c_void,
) -> i32 {
    if pp_cp.is_null() {
        return COM_E_INVALIDARG;
    }
    *pp_cp = std::ptr::null_mut();
    let owner = as_oxvba_test_dispatch_owner_from_connection_point_container(this);
    if guid_equals(riid, &IID_OXVBA_TEST_DISPATCH_EVENTS) {
        *pp_cp = (&mut (*owner).dispatch_connection_point
            as *mut OxvbaTestConnectionPointInterface)
            .cast();
    } else if guid_equals(riid, &IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS) {
        *pp_cp = (&mut (*owner).source_connection_point as *mut OxvbaTestConnectionPointInterface)
            .cast();
    } else {
        return COM_CONNECT_E_NOCONNECTION;
    }
    let _ = oxvba_test_owner_add_ref(owner);
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    if guid_equals(riid, &IID_IUNKNOWN) || guid_equals(riid, &IID_ICONNECTIONPOINT) {
        *ppv = this;
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    if guid_equals(riid, &IID_ICONNECTIONPOINTCONTAINER) {
        *ppv = (&mut (*owner).connection_point_container
            as *mut OxvbaTestConnectionPointContainerInterface)
            .cast();
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_add_ref(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    oxvba_test_owner_add_ref(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_release(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    oxvba_test_owner_release(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_get_connection_interface(
    this: *mut core::ffi::c_void,
    p_iid: *mut windows_sys::core::GUID,
) -> i32 {
    if p_iid.is_null() {
        return COM_E_INVALIDARG;
    }
    *p_iid = match oxvba_test_connection_point_kind(this) {
        OxvbaTestConnectionPointKind::Dispatch => IID_OXVBA_TEST_DISPATCH_EVENTS,
        OxvbaTestConnectionPointKind::SourceInterface => IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS,
    };
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_get_connection_point_container(
    this: *mut core::ffi::c_void,
    pp_cpc: *mut *mut core::ffi::c_void,
) -> i32 {
    if pp_cpc.is_null() {
        return COM_E_INVALIDARG;
    }
    *pp_cpc = std::ptr::null_mut();
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    *pp_cpc = (&mut (*owner).connection_point_container
        as *mut OxvbaTestConnectionPointContainerInterface)
        .cast();
    let _ = oxvba_test_owner_add_ref(owner);
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_advise(
    this: *mut core::ffi::c_void,
    p_unk_sink: *mut core::ffi::c_void,
    pdw_cookie: *mut u32,
) -> i32 {
    if p_unk_sink.is_null() || pdw_cookie.is_null() {
        return COM_E_INVALIDARG;
    }
    *pdw_cookie = 0;
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    let kind = oxvba_test_connection_point_kind(this);
    let mut sink_interface: *mut core::ffi::c_void = std::ptr::null_mut();
    let unknown = p_unk_sink.cast::<RawIUnknown>();
    let expected_iid = match kind {
        OxvbaTestConnectionPointKind::Dispatch => &IID_IDISPATCH,
        OxvbaTestConnectionPointKind::SourceInterface => &IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS,
    };
    let hr = ((*(*unknown).vtbl).query_interface)(p_unk_sink, expected_iid, &mut sink_interface);
    if hr < 0 || sink_interface.is_null() {
        return COM_CONNECT_E_CANNOTCONNECT;
    }
    let mut cookie = (*owner)
        .next_cookie
        .fetch_add(1, Ordering::AcqRel)
        .saturating_add(1);
    if cookie == 0 {
        cookie = 1;
    }
    match kind {
        OxvbaTestConnectionPointKind::Dispatch => {
            let mut sinks = match (*owner).dispatch_sinks.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            while sinks.contains_key(&cookie) {
                cookie = cookie.saturating_add(1).max(1);
            }
            sinks.insert(cookie, sink_interface as RawDispatchPtr);
        }
        OxvbaTestConnectionPointKind::SourceInterface => {
            let mut sinks = match (*owner).source_interface_sinks.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            while sinks.contains_key(&cookie) {
                cookie = cookie.saturating_add(1).max(1);
            }
            sinks.insert(cookie, sink_interface as RawUnknownPtr);
        }
    }
    *pdw_cookie = cookie;
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_unadvise(this: *mut core::ffi::c_void, dw_cookie: u32) -> i32 {
    if dw_cookie == 0 {
        return COM_CONNECT_E_NOCONNECTION;
    }
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    let kind = oxvba_test_connection_point_kind(this);
    let sink = match kind {
        OxvbaTestConnectionPointKind::Dispatch => {
            let mut sinks = match (*owner).dispatch_sinks.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            sinks.remove(&dw_cookie).map(|sink| sink as RawUnknownPtr)
        }
        OxvbaTestConnectionPointKind::SourceInterface => {
            let mut sinks = match (*owner).source_interface_sinks.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            sinks.remove(&dw_cookie)
        }
    };
    let Some(sink) = sink else {
        return COM_CONNECT_E_NOCONNECTION;
    };
    raw_release_unknown(sink as *mut core::ffi::c_void);
    COM_S_OK
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn oxvba_test_enum_connections(
    _this: *mut core::ffi::c_void,
    _pp_enum: *mut *mut core::ffi::c_void,
) -> i32 {
    COM_E_NOTIMPL
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_get_type_info_count(
    _this: *mut core::ffi::c_void,
    pctinfo: *mut u32,
) -> i32 {
    if pctinfo.is_null() {
        return COM_E_INVALIDARG;
    }
    *pctinfo = 0;
    COM_S_OK
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn oxvba_test_get_type_info(
    _this: *mut core::ffi::c_void,
    _itinfo: u32,
    _lcid: u32,
    _pptinfo: *mut *mut core::ffi::c_void,
) -> i32 {
    COM_E_NOTIMPL
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_get_ids_of_names(
    _this: *mut core::ffi::c_void,
    _riid: *const windows_sys::core::GUID,
    rgsznames: *mut *mut u16,
    cnames: u32,
    _lcid: u32,
    rgdispid: *mut i32,
) -> i32 {
    if rgsznames.is_null() || rgdispid.is_null() || cnames == 0 {
        return COM_E_INVALIDARG;
    }
    for index in 0..cnames as usize {
        let name = read_utf16_z(*rgsznames.add(index))
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let dispid = match name.as_str() {
            "newenum" => TEST_DISPID_NEWENUM,
            "count" => TEST_DISPID_COUNT,
            "exists" => TEST_DISPID_EXISTS,
            "firechanged" => TEST_DISPID_FIRE_CHANGED,
            "firechangedpair" => TEST_DISPID_FIRE_CHANGED_PAIR,
            "firechangedsourceinterface" => TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE,
            "ping" => TEST_DISPID_PING,
            "lookup" => TEST_DISPID_LOOKUP,
            "setvalue" => TEST_DISPID_SET_VALUE,
            "setvalueref" => TEST_DISPID_SET_VALUE_REF,
            "value" if index > 0 => TEST_NAMED_DISPID_VALUE,
            "value" => TEST_DISPID_VALUE,
            "sumpair" => TEST_DISPID_SUM_PAIR,
            "lookuppair" => TEST_DISPID_LOOKUP_PAIR,
            "setindexedvalue" => TEST_DISPID_SET_INDEXED_VALUE,
            "setindexedvalueref" => TEST_DISPID_SET_INDEXED_VALUE_REF,
            "echovariant" => TEST_DISPID_ECHO_VARIANT,
            "raiseexception" => TEST_DISPID_RAISE_EXCEPTION,
            "raiserichexception" => TEST_DISPID_RAISE_RICH_EXCEPTION,
            "returnsmallint" => TEST_DISPID_RETURN_SMALLINT,
            "returnunsignedword" => TEST_DISPID_RETURN_UNSIGNED_WORD,
            "returnsmallintarray" => TEST_DISPID_RETURN_SMALLINT_ARRAY,
            "returnboolarray" => TEST_DISPID_RETURN_BOOL_ARRAY,
            "returnstringarray" => TEST_DISPID_RETURN_STRING_ARRAY,
            "returnselfdispatch" => TEST_DISPID_RETURN_SELF_DISPATCH,
            "selfdispatch" => TEST_DISPID_RETURN_SELF_DISPATCH,
            "returnselfunknown" => TEST_DISPID_RETURN_SELF_UNKNOWN,
            "selfunknown" => TEST_DISPID_RETURN_SELF_UNKNOWN,
            "classifyvariantarg" => TEST_DISPID_CLASSIFY_VARIANT_ARG,
            "classifyvariantarrayfirstelementarg" => {
                TEST_DISPID_CLASSIFY_VARIANT_ARRAY_FIRST_ELEMENT_ARG
            }
            "returnselfdispatcharray" => TEST_DISPID_RETURN_SELF_DISPATCH_ARRAY,
            "returnselftypeddispatcharray" => TEST_DISPID_RETURN_SELF_TYPED_DISPATCH_ARRAY,
            "returnselftypedunknownarray" => TEST_DISPID_RETURN_SELF_TYPED_UNKNOWN_ARRAY,
            "returnsmallintmatrix" => TEST_DISPID_RETURN_SMALLINT_MATRIX,
            "returnplainunknown" => TEST_DISPID_RETURN_PLAIN_UNKNOWN,
            "returnplainunknownarray" => TEST_DISPID_RETURN_PLAIN_UNKNOWN_ARRAY,
            "returnlongarray" => TEST_DISPID_RETURN_LONG_ARRAY,
            "returnunsignedlongarray" => TEST_DISPID_RETURN_UNSIGNED_LONG_ARRAY,
            "returnlong" => TEST_DISPID_RETURN_LONG,
            "returnunsignedlong" => TEST_DISPID_RETURN_UNSIGNED_LONG,
            "returnbyte" => TEST_DISPID_RETURN_BYTE,
            "returnbytearray" => TEST_DISPID_RETURN_BYTE_ARRAY,
            "returnsignedbyte" => TEST_DISPID_RETURN_SIGNED_BYTE,
            "returnsignedbytearray" => TEST_DISPID_RETURN_SIGNED_BYTE_ARRAY,
            "returnplatformint" => TEST_DISPID_RETURN_PLATFORM_INT,
            "returnplatformuint" => TEST_DISPID_RETURN_PLATFORM_UINT,
            "returnplatformintarray" => TEST_DISPID_RETURN_PLATFORM_INT_ARRAY,
            "returnplatformuintarray" => TEST_DISPID_RETURN_PLATFORM_UINT_ARRAY,
            "returnhyper" => TEST_DISPID_RETURN_HYPER,
            "returnunsignedhyper" => TEST_DISPID_RETURN_UNSIGNED_HYPER,
            "returnhyperarray" => TEST_DISPID_RETURN_HYPER_ARRAY,
            "returnunsignedhyperarray" => TEST_DISPID_RETURN_UNSIGNED_HYPER_ARRAY,
            "returndouble" => TEST_DISPID_RETURN_DOUBLE,
            "returndoublearray" => TEST_DISPID_RETURN_DOUBLE_ARRAY,
            "returnsingle" => TEST_DISPID_RETURN_SINGLE,
            "returnsinglearray" => TEST_DISPID_RETURN_SINGLE_ARRAY,
            "returndate" => TEST_DISPID_RETURN_DATE,
            "returndatearray" => TEST_DISPID_RETURN_DATE_ARRAY,
            "returncurrency" => TEST_DISPID_RETURN_CURRENCY,
            "returncurrencyarray" => TEST_DISPID_RETURN_CURRENCY_ARRAY,
            "returndecimal" => TEST_DISPID_RETURN_DECIMAL,
            "returndecimalarray" => TEST_DISPID_RETURN_DECIMAL_ARRAY,
            "returnwideunsignedlong" => TEST_DISPID_RETURN_WIDE_UNSIGNED_LONG,
            "returnwideunsignedlongarray" => TEST_DISPID_RETURN_WIDE_UNSIGNED_LONG_ARRAY,
            "returnwideplatformuint" => TEST_DISPID_RETURN_WIDE_PLATFORM_UINT,
            "returnwideplatformuintarray" => TEST_DISPID_RETURN_WIDE_PLATFORM_UINT_ARRAY,
            "returnbool" => TEST_DISPID_RETURN_BOOL,
            "returnstring" => TEST_DISPID_RETURN_STRING,
            "returnempty" => TEST_DISPID_RETURN_EMPTY,
            "returnnull" => TEST_DISPID_RETURN_NULL,
            "returnerror" => TEST_DISPID_RETURN_ERROR,
            "returnbyreflong" => TEST_DISPID_RETURN_BYREF_LONG,
            "returnbyreflongarray" => TEST_DISPID_RETURN_BYREF_LONG_ARRAY,
            "returnwidehyper" => TEST_DISPID_RETURN_WIDE_HYPER,
            "returnwidehyperarray" => TEST_DISPID_RETURN_WIDE_HYPER_ARRAY,
            "returnwideunsignedhyper" => TEST_DISPID_RETURN_WIDE_UNSIGNED_HYPER,
            "returnwideunsignedhyperarray" => TEST_DISPID_RETURN_WIDE_UNSIGNED_HYPER_ARRAY,
            "returnvariantmatrix" => TEST_DISPID_RETURN_VARIANT_MATRIX,
            "returnplainunknownvariantarray" => TEST_DISPID_RETURN_PLAIN_UNKNOWN_VARIANT_ARRAY,
            "returnmissingmembername" => TEST_DISPID_RETURN_MISSING_MEMBER_NAME,
            "returnpingmembername" => TEST_DISPID_RETURN_PING_MEMBER_NAME,
            "returnlookupmembername" => TEST_DISPID_RETURN_LOOKUP_MEMBER_NAME,
            "returnsumpairmembername" => TEST_DISPID_RETURN_SUM_PAIR_MEMBER_NAME,
            "returnlookuppairmembername" => TEST_DISPID_RETURN_LOOKUP_PAIR_MEMBER_NAME,
            "returnsetvaluemembername" => TEST_DISPID_RETURN_SET_VALUE_MEMBER_NAME,
            "returnsetvaluerefmembername" => TEST_DISPID_RETURN_SET_VALUE_REF_MEMBER_NAME,
            "returnsetindexedvaluemembername" => TEST_DISPID_RETURN_SET_INDEXED_VALUE_MEMBER_NAME,
            "returnsetindexedvaluerefmembername" => {
                TEST_DISPID_RETURN_SET_INDEXED_VALUE_REF_MEMBER_NAME
            }
            "returnvaluemembername" => TEST_DISPID_RETURN_VALUE_MEMBER_NAME,
            "returndefaultmembername" => TEST_DISPID_RETURN_DEFAULT_MEMBER_NAME,
            "raiseparamnotfound" => TEST_DISPID_RAISE_PARAM_NOT_FOUND,
            "lhs" => TEST_NAMED_DISPID_LHS,
            "rhs" => TEST_NAMED_DISPID_RHS,
            "index" => TEST_NAMED_DISPID_INDEX,
            "valuearg" | "value_param" | "valueargtoken" => TEST_NAMED_DISPID_VALUE,
            _ => return COM_DISP_E_UNKNOWNNAME,
        };
        *rgdispid.add(index) = dispid;
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_invoke(
    this: *mut core::ffi::c_void,
    dispidmember: i32,
    _riid: *const windows_sys::core::GUID,
    _lcid: u32,
    wflags: u16,
    pparams: *mut DISPPARAMS,
    pvarresult: *mut VARIANT,
    _pexcepinfo: *mut EXCEPINFO,
    puargerr: *mut u32,
) -> i32 {
    let (cargs, rgvarg) = if pparams.is_null() {
        (0, std::ptr::null_mut())
    } else {
        ((*pparams).cArgs, (*pparams).rgvarg)
    };
    match dispidmember {
        TEST_DISPID_NEWENUM => {
            if (wflags & DISPATCH_PROPERTYGET) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_UNKNOWN;
                (*pvarresult).Anonymous.Anonymous.Anonymous.punkVal =
                    create_oxvba_test_enum_unknown().cast();
            }
            COM_S_OK
        }
        TEST_DISPID_COUNT => {
            if (wflags & DISPATCH_PROPERTYGET) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            set_variant_i32(7, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_EXISTS => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let key = match raw_variant_token_from_dispparams(pparams, 0, puargerr) {
                Ok(value) => value,
                Err(hr) => return hr,
            };
            set_variant_bool(key == 42, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_FIRE_CHANGED => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let arg = &*rgvarg;
            let value = match raw_variant_token_from_invoke_arg(arg, 0) {
                Ok(value) => value,
                Err(hr) => {
                    if !puargerr.is_null() {
                        *puargerr = 0;
                    }
                    return hr;
                }
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let _ = oxvba_test_fire_event(owner, TEST_EVENT_CHANGED, &[value]);
            set_variant_i32(value, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_FIRE_CHANGED_PAIR => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let arg = &*rgvarg;
            let value = match raw_variant_token_from_invoke_arg(arg, 0) {
                Ok(value) => value,
                Err(hr) => {
                    if !puargerr.is_null() {
                        *puargerr = 0;
                    }
                    return hr;
                }
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let _ = oxvba_test_fire_event(
                owner,
                TEST_EVENT_CHANGED_PAIR,
                &[value, value.saturating_add(1)],
            );
            set_variant_i32(value.saturating_add(1), pvarresult);
            COM_S_OK
        }
        TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let arg = &*rgvarg;
            let value = match raw_variant_token_from_invoke_arg(arg, 0) {
                Ok(value) => value,
                Err(hr) => {
                    if !puargerr.is_null() {
                        *puargerr = 0;
                    }
                    return hr;
                }
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let _ = oxvba_test_fire_source_interface_event(owner, value);
            set_variant_i32(value, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_PING => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            set_variant_i32(123, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_LOOKUP => {
            if (wflags & DISPATCH_PROPERTYGET) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let value = match raw_variant_token_from_dispparams(pparams, 0, puargerr) {
                Ok(value) => value,
                Err(hr) => {
                    return hr;
                }
            };
            set_variant_i32(value.saturating_add(1_000), pvarresult);
            COM_S_OK
        }
        TEST_DISPID_SUM_PAIR => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 2 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let lhs =
                match raw_variant_token_from_named_dispid(pparams, TEST_NAMED_DISPID_LHS, puargerr)
                {
                    Some(result) => match result {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                    None => match raw_variant_token_from_dispparams(pparams, 0, puargerr) {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                };
            let rhs =
                match raw_variant_token_from_named_dispid(pparams, TEST_NAMED_DISPID_RHS, puargerr)
                {
                    Some(result) => match result {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                    None => match raw_variant_token_from_dispparams(pparams, 1, puargerr) {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                };
            set_variant_i32(lhs.saturating_mul(1_000).saturating_add(rhs), pvarresult);
            COM_S_OK
        }
        TEST_DISPID_LOOKUP_PAIR => {
            if (wflags & DISPATCH_PROPERTYGET) == 0 || cargs != 2 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let lhs =
                match raw_variant_token_from_named_dispid(pparams, TEST_NAMED_DISPID_LHS, puargerr)
                {
                    Some(result) => match result {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                    None => match raw_variant_token_from_dispparams(pparams, 0, puargerr) {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                };
            let rhs =
                match raw_variant_token_from_named_dispid(pparams, TEST_NAMED_DISPID_RHS, puargerr)
                {
                    Some(result) => match result {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                    None => match raw_variant_token_from_dispparams(pparams, 1, puargerr) {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                };
            set_variant_i32(
                lhs.saturating_mul(1_000)
                    .saturating_add(rhs)
                    .saturating_add(200_000),
                pvarresult,
            );
            COM_S_OK
        }
        TEST_DISPID_SET_VALUE => {
            if (wflags & DISPATCH_PROPERTYPUT) == 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let value = match raw_property_put_i4_from_params(pparams, puargerr) {
                Ok(value) => value,
                Err(hr) => return hr,
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            (*owner).value_state.store(value, Ordering::Release);
            set_variant_i32(value, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_SET_VALUE_REF => {
            if (wflags & DISPATCH_PROPERTYPUTREF) == 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let value = match raw_property_put_i4_from_params(pparams, puargerr) {
                Ok(value) => value,
                Err(hr) => return hr,
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let stored = value.saturating_add(100_000);
            (*owner).value_state.store(stored, Ordering::Release);
            set_variant_i32(stored, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_SET_INDEXED_VALUE => {
            if (wflags & DISPATCH_PROPERTYPUT) == 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let args = match raw_property_put_args_from_params(pparams, 2, puargerr) {
                Ok(args) => args,
                Err(hr) => return hr,
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let stored = args[0]
                .saturating_mul(1_000)
                .saturating_add(args[1])
                .saturating_add(300_000);
            (*owner).value_state.store(stored, Ordering::Release);
            set_variant_i32(stored, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_SET_INDEXED_VALUE_REF => {
            if (wflags & DISPATCH_PROPERTYPUTREF) == 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let args = match raw_property_put_args_from_params(pparams, 2, puargerr) {
                Ok(args) => args,
                Err(hr) => return hr,
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let stored = args[0]
                .saturating_mul(1_000)
                .saturating_add(args[1])
                .saturating_add(400_000);
            (*owner).value_state.store(stored, Ordering::Release);
            set_variant_i32(stored, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_ECHO_VARIANT => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let value = match raw_variant_value_from_dispparams(pparams, 0, puargerr) {
                Ok(value) => value,
                Err(hr) => return hr,
            };
            let mut resolve_object = |_handle: oxvba_runtime::ObjectRef| {
                Err("object dispatch echo unsupported in test helper".to_string())
            };
            if set_variant_dispatch_arg(pvarresult, &value, &mut resolve_object).is_err() {
                return COM_DISP_E_TYPEMISMATCH;
            }
            COM_S_OK
        }
        TEST_DISPID_RAISE_EXCEPTION => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            populate_excepinfo(
                _pexcepinfo,
                OXVBA_TEST_DISPATCH_PROGID,
                "controlled dispatch exception",
                COM_DISP_E_EXCEPTION,
            );
            COM_DISP_E_EXCEPTION
        }
        TEST_DISPID_RAISE_RICH_EXCEPTION => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            populate_rich_excepinfo(
                _pexcepinfo,
                OXVBA_TEST_DISPATCH_PROGID,
                "controlled rich exception with full ExcepInfo surface",
                "OxVba.TestDispatch.hlp",
                1001,
                COM_DISP_E_EXCEPTION,
                42,
            );
            COM_DISP_E_EXCEPTION
        }
        TEST_DISPID_RAISE_PARAM_NOT_FOUND => {
            if (wflags & (DISPATCH_METHOD | DISPATCH_PROPERTYGET)) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            COM_DISP_E_PARAMNOTFOUND
        }
        TEST_DISPID_RETURN_SMALLINT => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_I2;
                (*pvarresult).Anonymous.Anonymous.Anonymous.iVal = 321;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_UNSIGNED_WORD => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_UI2;
                (*pvarresult).Anonymous.Anonymous.Anonymous.uiVal = 65000;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_LONG => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_I4;
                (*pvarresult).Anonymous.Anonymous.Anonymous.lVal = 70000;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_UNSIGNED_LONG => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_UI4;
                (*pvarresult).Anonymous.Anonymous.Anonymous.ulVal = 70000;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_BYTE => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_UI1;
                (*pvarresult).Anonymous.Anonymous.Anonymous.bVal = 255;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_SIGNED_BYTE => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_I1;
                (*pvarresult).Anonymous.Anonymous.Anonymous.cVal = -5;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_PLATFORM_INT => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_INT;
                (*pvarresult).Anonymous.Anonymous.Anonymous.intVal = -70_000;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_PLATFORM_UINT => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_UINT;
                (*pvarresult).Anonymous.Anonymous.Anonymous.uintVal = 70_000;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_HYPER => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_I8;
                (*pvarresult).Anonymous.Anonymous.Anonymous.llVal = -70_000;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_UNSIGNED_HYPER => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_UI8;
                (*pvarresult).Anonymous.Anonymous.Anonymous.ullVal = 70_000;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_WIDE_HYPER => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_I8;
                (*pvarresult).Anonymous.Anonymous.Anonymous.llVal = 5_000_000_000;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_WIDE_UNSIGNED_HYPER => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_UI8;
                (*pvarresult).Anonymous.Anonymous.Anonymous.ullVal = 5_000_000_000;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_DOUBLE => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_R8_VARENUM;
                (*pvarresult).Anonymous.Anonymous.Anonymous.dblVal = 12.5;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_SINGLE => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_R4_VARENUM;
                (*pvarresult).Anonymous.Anonymous.Anonymous.fltVal = 12.5;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_DATE => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_DATE_VARENUM;
                (*pvarresult).Anonymous.Anonymous.Anonymous.dblVal = 45200.25;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_CURRENCY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_CY_VARENUM;
                (*pvarresult).Anonymous.Anonymous.Anonymous.cyVal.int64 = 125_000;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_DECIMAL => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.decVal = decimal_from_parts(123_450, 0, 0, 3, true);
                (*pvarresult).Anonymous.decVal.wReserved = VT_DECIMAL;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_WIDE_UNSIGNED_LONG => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_UI4;
                (*pvarresult).Anonymous.Anonymous.Anonymous.ulVal = 4_000_000_000;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_WIDE_PLATFORM_UINT => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_UINT;
                (*pvarresult).Anonymous.Anonymous.Anonymous.uintVal = 4_000_000_000;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_BOOL => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_BOOL;
                (*pvarresult).Anonymous.Anonymous.Anonymous.boolVal = -1;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_STRING => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                let bstr = alloc_bstr("Scalar BSTR");
                if bstr.is_null() {
                    return COM_E_INVALIDARG;
                }
                (*pvarresult).Anonymous.Anonymous.vt = VT_BSTR;
                (*pvarresult).Anonymous.Anonymous.Anonymous.bstrVal = bstr;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_MISSING_MEMBER_NAME => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                let bstr = alloc_bstr("DefinitelyMissingMember");
                if bstr.is_null() {
                    return COM_E_INVALIDARG;
                }
                (*pvarresult).Anonymous.Anonymous.vt = VT_BSTR;
                (*pvarresult).Anonymous.Anonymous.Anonymous.bstrVal = bstr;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_PING_MEMBER_NAME => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                let bstr = alloc_bstr("Ping");
                if bstr.is_null() {
                    return COM_E_INVALIDARG;
                }
                (*pvarresult).Anonymous.Anonymous.vt = VT_BSTR;
                (*pvarresult).Anonymous.Anonymous.Anonymous.bstrVal = bstr;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_LOOKUP_MEMBER_NAME => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                let bstr = alloc_bstr("Lookup");
                if bstr.is_null() {
                    return COM_E_INVALIDARG;
                }
                (*pvarresult).Anonymous.Anonymous.vt = VT_BSTR;
                (*pvarresult).Anonymous.Anonymous.Anonymous.bstrVal = bstr;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_SUM_PAIR_MEMBER_NAME => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                let bstr = alloc_bstr("SumPair");
                if bstr.is_null() {
                    return COM_E_INVALIDARG;
                }
                (*pvarresult).Anonymous.Anonymous.vt = VT_BSTR;
                (*pvarresult).Anonymous.Anonymous.Anonymous.bstrVal = bstr;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_LOOKUP_PAIR_MEMBER_NAME => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                let bstr = alloc_bstr("LookupPair");
                if bstr.is_null() {
                    return COM_E_INVALIDARG;
                }
                (*pvarresult).Anonymous.Anonymous.vt = VT_BSTR;
                (*pvarresult).Anonymous.Anonymous.Anonymous.bstrVal = bstr;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_SET_VALUE_MEMBER_NAME => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                let bstr = alloc_bstr("SetValue");
                if bstr.is_null() {
                    return COM_E_INVALIDARG;
                }
                (*pvarresult).Anonymous.Anonymous.vt = VT_BSTR;
                (*pvarresult).Anonymous.Anonymous.Anonymous.bstrVal = bstr;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_SET_VALUE_REF_MEMBER_NAME => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                let bstr = alloc_bstr("SetValueRef");
                if bstr.is_null() {
                    return COM_E_INVALIDARG;
                }
                (*pvarresult).Anonymous.Anonymous.vt = VT_BSTR;
                (*pvarresult).Anonymous.Anonymous.Anonymous.bstrVal = bstr;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_SET_INDEXED_VALUE_MEMBER_NAME => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                let bstr = alloc_bstr("SetIndexedValue");
                if bstr.is_null() {
                    return COM_E_INVALIDARG;
                }
                (*pvarresult).Anonymous.Anonymous.vt = VT_BSTR;
                (*pvarresult).Anonymous.Anonymous.Anonymous.bstrVal = bstr;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_SET_INDEXED_VALUE_REF_MEMBER_NAME => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                let bstr = alloc_bstr("SetIndexedValueRef");
                if bstr.is_null() {
                    return COM_E_INVALIDARG;
                }
                (*pvarresult).Anonymous.Anonymous.vt = VT_BSTR;
                (*pvarresult).Anonymous.Anonymous.Anonymous.bstrVal = bstr;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_VALUE_MEMBER_NAME => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                let bstr = alloc_bstr("Value");
                if bstr.is_null() {
                    return COM_E_INVALIDARG;
                }
                (*pvarresult).Anonymous.Anonymous.vt = VT_BSTR;
                (*pvarresult).Anonymous.Anonymous.Anonymous.bstrVal = bstr;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_DEFAULT_MEMBER_NAME => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                let bstr = alloc_bstr("EchoVariant");
                if bstr.is_null() {
                    return COM_E_INVALIDARG;
                }
                (*pvarresult).Anonymous.Anonymous.vt = VT_BSTR;
                (*pvarresult).Anonymous.Anonymous.Anonymous.bstrVal = bstr;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_EMPTY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_EMPTY;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_NULL => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_NULL;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_ERROR => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_ERROR;
                (*pvarresult).Anonymous.Anonymous.Anonymous.scode = 17;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_BYREF_LONG => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            set_variant_i32_byref(pvarresult);
            COM_S_OK
        }
        TEST_DISPID_RETURN_BYREF_LONG_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_i32_array_byref(pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_WIDE_HYPER_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_i64_array(&[12, 5_000_000_000, -4], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_WIDE_UNSIGNED_HYPER_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_u64_array(&[12, 5_000_000_000, 70_000], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_VARIANT_MATRIX => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_variant_matrix(pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_SMALLINT_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_i16_array(&[12, -4, 321], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_SMALLINT_MATRIX => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_i16_matrix(pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_PLAIN_UNKNOWN => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_UNKNOWN;
                (*pvarresult).Anonymous.Anonymous.Anonymous.punkVal =
                    create_oxvba_test_plain_unknown().cast();
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_PLAIN_UNKNOWN_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            set_variant_typed_plain_unknown_array(pvarresult)
        }
        TEST_DISPID_RETURN_PLAIN_UNKNOWN_VARIANT_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            set_variant_plain_unknown_in_variant_array(pvarresult)
        }
        TEST_DISPID_RETURN_LONG_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_i32_array(&[12, -4, 70_000], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_UNSIGNED_LONG_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_u32_array(&[12, 4_096, 70_000], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_BYTE_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_u8_array(&[0, 12, 255], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_SIGNED_BYTE_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_i8_array(&[-5, 0, 120], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_PLATFORM_INT_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_platform_i32_array(&[-70_000, 0, 12], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_PLATFORM_UINT_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_platform_u32_array(&[12, 4_096, 70_000], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_HYPER_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_i64_array(&[-70_000, 0, 12], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_UNSIGNED_HYPER_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_u64_array(&[12, 4_096, 70_000], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_DOUBLE_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_f64_array(&[12.5, -4.25, 321.0], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_SINGLE_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_f32_array(&[12.5, -4.25, 321.0], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_DATE_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_date_array(&[45200.25, 12.5, -4.25], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_CURRENCY_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_currency_array(&[125_000, -42_500, 3_210_000], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_DECIMAL_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_decimal_array(
                &[
                    decimal_from_parts(123_450, 0, 0, 3, false),
                    decimal_from_parts(42_500, 0, 0, 4, true),
                    decimal_from_parts(3_210_000, 0, 0, 4, false),
                ],
                pvarresult,
            ) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_WIDE_UNSIGNED_LONG_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_u32_array(&[12, 4_000_000_000, 70_000], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_WIDE_PLATFORM_UINT_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_platform_u32_array(&[12, 4_000_000_000, 70_000], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_BOOL_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_bool_array(&[true, false, true], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_STRING_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_bstr_array(&["Alpha", "Beta"], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_SELF_DISPATCH => {
            if (wflags & (DISPATCH_METHOD | DISPATCH_PROPERTYGET)) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                oxvba_test_add_ref(this);
                (*pvarresult).Anonymous.Anonymous.vt = VT_DISPATCH;
                (*pvarresult).Anonymous.Anonymous.Anonymous.pdispVal = this.cast();
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_SELF_UNKNOWN => {
            if (wflags & (DISPATCH_METHOD | DISPATCH_PROPERTYGET)) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                oxvba_test_add_ref(this);
                (*pvarresult).Anonymous.Anonymous.vt = VT_UNKNOWN;
                (*pvarresult).Anonymous.Anonymous.Anonymous.punkVal = this.cast();
            }
            COM_S_OK
        }
        TEST_DISPID_CLASSIFY_VARIANT_ARG => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let vt = (*rgvarg).Anonymous.Anonymous.vt as i32;
            set_variant_i32(vt, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_CLASSIFY_VARIANT_ARRAY_FIRST_ELEMENT_ARG => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            classify_variant_array_first_element_vt(rgvarg, pvarresult)
        }
        TEST_DISPID_RETURN_SELF_DISPATCH_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            set_variant_dispatch_array(this, pvarresult)
        }
        TEST_DISPID_RETURN_SELF_TYPED_DISPATCH_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            set_variant_typed_dispatch_array(this, pvarresult)
        }
        TEST_DISPID_RETURN_SELF_TYPED_UNKNOWN_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            set_variant_typed_unknown_array(this, pvarresult)
        }
        TEST_DISPID_VALUE => {
            if (wflags & DISPATCH_PROPERTYGET) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let value = (*owner).value_state.load(Ordering::Acquire);
            set_variant_i32(value, pvarresult);
            COM_S_OK
        }
        _ => COM_DISP_E_MEMBERNOTFOUND,
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_dispatch_array(
    this: *mut core::ffi::c_void,
    pvarresult: *mut VARIANT,
) -> i32 {
    let psa = SafeArrayCreateVector(VT_VARIANT, 0, 1);
    if psa.is_null() {
        return COM_E_INVALIDARG;
    }
    let mut element: VARIANT = std::mem::zeroed();
    oxvba_test_add_ref(this);
    element.Anonymous.Anonymous.vt = VT_DISPATCH;
    element.Anonymous.Anonymous.Anonymous.pdispVal = this.cast();
    let index = 0i32;
    let hr = SafeArrayPutElement(
        psa.cast_const(),
        &index,
        (&element as *const VARIANT).cast(),
    );
    let _ = VariantClear(&mut element);
    if hr < 0 {
        let _ = SafeArrayDestroy(psa.cast_const());
        return COM_E_INVALIDARG;
    }
    if !pvarresult.is_null() {
        (*pvarresult).Anonymous.Anonymous.vt = VT_ARRAY | VT_VARIANT;
        (*pvarresult).Anonymous.Anonymous.Anonymous.parray = psa;
        return COM_S_OK;
    }
    let _ = SafeArrayDestroy(psa.cast_const());
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_plain_unknown_in_variant_array(pvarresult: *mut VARIANT) -> i32 {
    let psa = SafeArrayCreateVector(VT_VARIANT, 0, 1);
    if psa.is_null() {
        return COM_E_INVALIDARG;
    }
    let mut element: VARIANT = std::mem::zeroed();
    element.Anonymous.Anonymous.vt = VT_UNKNOWN;
    element.Anonymous.Anonymous.Anonymous.punkVal = create_oxvba_test_plain_unknown().cast();
    let index = 0i32;
    let hr = SafeArrayPutElement(
        psa.cast_const(),
        &index,
        (&element as *const VARIANT).cast(),
    );
    let _ = VariantClear(&mut element);
    if hr < 0 {
        let _ = SafeArrayDestroy(psa.cast_const());
        return COM_E_INVALIDARG;
    }
    if !pvarresult.is_null() {
        (*pvarresult).Anonymous.Anonymous.vt = VT_ARRAY | VT_VARIANT;
        (*pvarresult).Anonymous.Anonymous.Anonymous.parray = psa;
        return COM_S_OK;
    }
    let _ = SafeArrayDestroy(psa.cast_const());
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_typed_dispatch_array(
    this: *mut core::ffi::c_void,
    pvarresult: *mut VARIANT,
) -> i32 {
    let psa = SafeArrayCreateVector(VT_DISPATCH, 0, 1);
    if psa.is_null() {
        return COM_E_INVALIDARG;
    }
    let dispatch = this.cast::<RawIDispatch>();
    let index = 0i32;
    let hr = SafeArrayPutElement(psa.cast_const(), &index, dispatch.cast());
    if hr < 0 {
        let _ = SafeArrayDestroy(psa.cast_const());
        return COM_E_INVALIDARG;
    }
    if !pvarresult.is_null() {
        (*pvarresult).Anonymous.Anonymous.vt = VT_ARRAY | VT_DISPATCH;
        (*pvarresult).Anonymous.Anonymous.Anonymous.parray = psa;
        return COM_S_OK;
    }
    let _ = SafeArrayDestroy(psa.cast_const());
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_typed_plain_unknown_array(pvarresult: *mut VARIANT) -> i32 {
    let psa = SafeArrayCreateVector(VT_UNKNOWN, 0, 1);
    if psa.is_null() {
        return COM_E_INVALIDARG;
    }
    let unknown = create_oxvba_test_plain_unknown();
    let index = 0i32;
    let hr = SafeArrayPutElement(psa.cast_const(), &index, unknown.cast());
    raw_release_unknown(unknown.cast());
    if hr < 0 {
        let _ = SafeArrayDestroy(psa.cast_const());
        return COM_E_INVALIDARG;
    }
    if !pvarresult.is_null() {
        (*pvarresult).Anonymous.Anonymous.vt = VT_ARRAY | VT_UNKNOWN;
        (*pvarresult).Anonymous.Anonymous.Anonymous.parray = psa;
        return COM_S_OK;
    }
    let _ = SafeArrayDestroy(psa.cast_const());
    COM_S_OK
}
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_typed_unknown_array(
    this: *mut core::ffi::c_void,
    pvarresult: *mut VARIANT,
) -> i32 {
    let psa = SafeArrayCreateVector(VT_UNKNOWN, 0, 1);
    if psa.is_null() {
        return COM_E_INVALIDARG;
    }
    let unknown = this.cast::<RawIUnknown>();
    let index = 0i32;
    let hr = SafeArrayPutElement(psa.cast_const(), &index, unknown.cast());
    if hr < 0 {
        let _ = SafeArrayDestroy(psa.cast_const());
        return COM_E_INVALIDARG;
    }
    if !pvarresult.is_null() {
        (*pvarresult).Anonymous.Anonymous.vt = VT_ARRAY | VT_UNKNOWN;
        (*pvarresult).Anonymous.Anonymous.Anonymous.parray = psa;
        return COM_S_OK;
    }
    let _ = SafeArrayDestroy(psa.cast_const());
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn classify_variant_array_first_element_vt(
    rgvarg: *const VARIANT,
    pvarresult: *mut VARIANT,
) -> i32 {
    let variant = &*rgvarg;
    let vt = variant.Anonymous.Anonymous.vt;
    if (vt & VT_ARRAY) == 0 {
        return COM_DISP_E_TYPEMISMATCH;
    }
    let psa = variant.Anonymous.Anonymous.Anonymous.parray;
    if psa.is_null() {
        return COM_E_INVALIDARG;
    }
    let dims = SafeArrayGetDim(psa.cast_const());
    if dims != 1 {
        return COM_E_INVALIDARG;
    }
    let mut lower = 0i32;
    if SafeArrayGetLBound(psa.cast_const(), 1, &mut lower) < 0 {
        return COM_E_INVALIDARG;
    }
    let mut upper = -1i32;
    if SafeArrayGetUBound(psa.cast_const(), 1, &mut upper) < 0 {
        return COM_E_INVALIDARG;
    }
    if upper < lower {
        set_variant_i32(VT_EMPTY as i32, pvarresult);
        return COM_S_OK;
    }
    let mut element_vt = 0u16;
    if SafeArrayGetVartype(psa.cast_const(), &mut element_vt) < 0 {
        return COM_E_INVALIDARG;
    }
    if element_vt != VT_VARIANT {
        set_variant_i32(element_vt as i32, pvarresult);
        return COM_S_OK;
    }
    let mut element: VARIANT = std::mem::zeroed();
    let hr = SafeArrayGetElement(
        psa.cast_const(),
        &lower,
        (&mut element as *mut VARIANT).cast(),
    );
    if hr < 0 {
        return COM_E_INVALIDARG;
    }
    let inner_vt = element.Anonymous.Anonymous.vt as i32;
    let _ = VariantClear(&mut element);
    set_variant_i32(inner_vt, pvarresult);
    COM_S_OK
}
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_to_com_value(variant: &VARIANT) -> Result<ComValue, String> {
    com_variant_to_com_value(variant)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_dispatch_arg<F>(
    variant: *mut VARIANT,
    value: &ComValue,
    resolve_object: &mut F,
) -> Result<(), String>
where
    F: FnMut(oxvba_runtime::ObjectRef) -> Result<*mut RawIDispatch, String>,
{
    if variant.is_null() {
        return Ok(());
    }
    let mut resolve_dispatch = |handle: oxvba_runtime::ObjectRef| {
        resolve_object(handle).map(|dispatch| dispatch.cast::<core::ffi::c_void>())
    };
    let mut add_ref_dispatch = |dispatch: *mut core::ffi::c_void| {
        raw_add_ref_dispatch(dispatch.cast::<RawIDispatch>());
    };
    match value {
        ComValue::Object(_) => com_set_variant_from_com_value(
            variant,
            value,
            &mut resolve_dispatch,
            &mut add_ref_dispatch,
        )?,
        _ => {
            let mut unexpected_object_resolution = |_handle: oxvba_runtime::ObjectRef| {
                Err("object dispatch resolution not expected for non-object COM value".to_string())
            };
            let mut unexpected_add_ref = |_dispatch: *mut core::ffi::c_void| {};
            com_set_variant_from_com_value(
                variant,
                value,
                &mut unexpected_object_resolution,
                &mut unexpected_add_ref,
            )?;
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
///
/// `dispatch` must be a live pointer returned by the controlled `OxVba.TestDispatch`
/// fixture or another object with the exact same vtable layout. Callers must ensure the
/// pointed-to COM object remains alive for the duration of the call.
pub unsafe fn raw_oxvba_test_dispatch_vtable_invoke(
    dispatch: *mut RawIDispatch,
    member: i32,
    args: &[i32],
) -> Result<Option<i32>, String> {
    if dispatch.is_null() {
        return Err("null dispatch pointer for vtable invoke".to_string());
    }
    if !std::ptr::eq((*dispatch).vtbl, &OXVBA_TEST_DISPATCH_VTBL) {
        return Ok(None);
    }
    match member {
        TEST_DISPID_COUNT => Ok(Some(7)),
        TEST_DISPID_EXISTS => {
            if args.len() != 1 {
                return Err(format!(
                    "IDispatch::Invoke(method) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(if args[0] == 42 { 1 } else { 0 }))
        }
        TEST_DISPID_FIRE_CHANGED => {
            if args.len() != 1 {
                return Err(format!(
                    "IDispatch::Invoke(method) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(args[0]))
        }
        TEST_DISPID_FIRE_CHANGED_PAIR => {
            if args.len() != 1 {
                return Err(format!(
                    "IDispatch::Invoke(method) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(args[0].saturating_add(1)))
        }
        TEST_DISPID_SUM_PAIR => {
            if args.len() != 2 {
                return Err(format!(
                    "IDispatch::Invoke(method) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(args[0].saturating_mul(1_000).saturating_add(args[1])))
        }
        TEST_DISPID_LOOKUP_PAIR => {
            if args.len() != 2 {
                return Err(format!(
                    "IDispatch::Invoke(property-get) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(
                args[0]
                    .saturating_mul(1_000)
                    .saturating_add(args[1])
                    .saturating_add(200_000),
            ))
        }
        TEST_DISPID_SET_INDEXED_VALUE => {
            if args.len() != 2 {
                return Err(format!(
                    "IDispatch::Invoke(property-put) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(
                args[0]
                    .saturating_mul(1_000)
                    .saturating_add(args[1])
                    .saturating_add(300_000),
            ))
        }
        TEST_DISPID_SET_INDEXED_VALUE_REF => {
            if args.len() != 2 {
                return Err(format!(
                    "IDispatch::Invoke(property-putref) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(
                args[0]
                    .saturating_mul(1_000)
                    .saturating_add(args[1])
                    .saturating_add(400_000),
            ))
        }
        TEST_DISPID_ECHO_VARIANT => {
            if args.len() != 1 {
                return Err(format!(
                    "IDispatch::Invoke(method) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(args[0]))
        }
        TEST_DISPID_RAISE_EXCEPTION => Err(format!(
            "IDispatch::Invoke(method) failed with HRESULT {:#010X} excep_description=\"controlled dispatch exception\" excep_scode={:#010X}",
            COM_DISP_E_EXCEPTION as u32, COM_DISP_E_EXCEPTION as u32
        )),
        TEST_DISPID_RAISE_RICH_EXCEPTION => Err(format!(
            "IDispatch::Invoke(method) failed with HRESULT {:#010X} excep_description=\"controlled rich exception with full ExcepInfo surface\" excep_help_file=\"OxVba.TestDispatch.hlp\" excep_help_context=1001 excep_scode={:#010X} excep_wcode=42",
            COM_DISP_E_EXCEPTION as u32, COM_DISP_E_EXCEPTION as u32
        )),
        TEST_DISPID_RETURN_SMALLINT => Ok(Some(321)),
        TEST_DISPID_RETURN_UNSIGNED_WORD => Ok(Some(65_000)),
        _ => Ok(None),
    }
}

// ════════════════════════════════════════════════════════════════════════
// Real custom dual vtable fixture (workset S2)
//
// Unlike `raw_oxvba_test_dispatch_vtable_invoke` (a behavioral ORACLE that
// re-implements members in Rust and never touches a vtable), this is a REAL
// ABI vtable: a `#[repr(C)]` struct whose first 7 slots are the standard
// IUnknown+IDispatch slots and whose slots 7.. are `extern "system"` custom
// dual-interface members. The S2 unit test drives `vtable_invoke` through
// libffi against these slots to prove the marshaller end-to-end.
//
// Slot layout (index → member):
//   0  QueryInterface          (IUnknown)
//   1  AddRef                  (IUnknown)
//   2  Release                 (IUnknown)
//   3  GetTypeInfoCount        (IDispatch)
//   4  GetTypeInfo             (IDispatch)
//   5  GetIDsOfNames           (IDispatch)
//   6  Invoke                  (IDispatch)
//   7  get_Count(this, i32*)                       -> HRESULT
//   8  Exists(this, i32, VARIANT_BOOL*)            -> HRESULT
//   9  put_Value(this, VARIANT*)                   -> HRESULT
//   10 Lookup(this, BSTR, IDispatch**)             -> HRESULT
//   11 raise_error(this, i32*) [SetErrorInfo+fail] -> HRESULT
//   12..23 additional typed ABI coverage slots
// ════════════════════════════════════════════════════════════════════════

/// The custom-interface IErrorInfo IID, `{1CF2B120-547D-101B-8E65-08002B2BD119}`.
#[cfg(target_os = "windows")]
const IID_IERRORINFO: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0x1CF2_B120,
    data2: 0x547D,
    data3: 0x101B,
    data4: [0x8E, 0x65, 0x08, 0x00, 0x2B, 0x2B, 0xD1, 0x19],
};

/// Slot indices of the custom dual members (for the unit test to call by name).
pub const DUAL_SLOT_GET_COUNT: u16 = 7;
pub const DUAL_SLOT_EXISTS: u16 = 8;
pub const DUAL_SLOT_PUT_VALUE: u16 = 9;
pub const DUAL_SLOT_LOOKUP: u16 = 10;
pub const DUAL_SLOT_RAISE_ERROR: u16 = 11;
/// slot 12: `get_Price(this, [out,retval] CY*)` — a VT_CY (currency, i64 scaled
/// ×10000) return, exercising the marshaller's `OutCell::Currency` decoder.
pub const DUAL_SLOT_GET_PRICE: u16 = 12;
/// slot 13: `get_Created(this, [out,retval] DATE*)` — a VT_DATE (f64 OLE date)
/// return, exercising the marshaller's date out-cell decoder.
pub const DUAL_SLOT_GET_CREATED: u16 = 13;
/// slot 14: `get_Owner(this, [out,retval] IUnknown**)` — a VT_UNKNOWN return that
/// the marshaller must `QueryInterface(IDispatch)` (`query_dispatch_from_unknown`).
pub const DUAL_SLOT_GET_OWNER: u16 = 14;
pub const DUAL_SLOT_VALIDATE_ALL_INPUTS: u16 = 15;
pub const DUAL_SLOT_GET_BYTE_VALUE: u16 = 16;
pub const DUAL_SLOT_GET_INTEGER_VALUE: u16 = 17;
pub const DUAL_SLOT_GET_LONGLONG_VALUE: u16 = 18;
pub const DUAL_SLOT_GET_SINGLE_VALUE: u16 = 19;
pub const DUAL_SLOT_GET_DOUBLE_VALUE: u16 = 20;
pub const DUAL_SLOT_GET_TEXT_VALUE: u16 = 21;
pub const DUAL_SLOT_GET_VARIANT_VALUE: u16 = 22;
pub const DUAL_SLOT_PUTREF_OBJECT_VALUE: u16 = 23;

/// The currency value `get_Price` returns: 12.3456 → scaled i64 123456.
pub const DUAL_PRICE_SCALED_I64: i64 = 123_456;
/// The OLE-date value `get_Created` returns (an arbitrary fixed f64).
pub const DUAL_CREATED_OLE_DATE: f64 = 45_000.5;
pub const DUAL_BYTE_VALUE: u8 = 201;
pub const DUAL_INTEGER_VALUE: i16 = -1234;
pub const DUAL_LONGLONG_VALUE: i64 = 5_000_000_000;
pub const DUAL_SINGLE_VALUE: f32 = 12.5;
pub const DUAL_DOUBLE_VALUE: f64 = -9876.25;
pub const DUAL_TEXT_VALUE: &str = "vtable-text";
pub const DUAL_VARIANT_VALUE: i32 = 4242;

/// The custom **dual interface IID** the fixture answers from `QueryInterface`
/// (besides `IUnknown`/`IDispatch`). Workset S5a: the vtable dispatch path QIs the
/// object for the member's dual IID and calls the slot on the returned pointer.
/// The fixture's QI returns `this` for this IID (its dual vtable aliases the
/// IDispatch vtable in-process), so the QI'd pointer is the same vtable-callable
/// object the slot tests already drive.
#[cfg(target_os = "windows")]
pub const IID_OXVBA_DUAL_FIXTURE: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0xE2A3_0D01,
    data2: 0x0D01,
    data3: 0x0D01,
    data4: [0x0D, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
};

/// The same dual IID as the platform-neutral [`crate::ComInterfaceIid`] carrier
/// the member spec / metadata blob hold, so a test can stamp it onto a spec.
#[cfg(target_os = "windows")]
pub const DUAL_FIXTURE_INTERFACE_IID: crate::ComInterfaceIid = crate::ComInterfaceIid {
    data1: 0xE2A3_0D01,
    data2: 0x0D01,
    data3: 0x0D01,
    data4: [0x0D, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
};

/// Source/Description the `raise_error` slot installs via `SetErrorInfo`, for
/// the unit test to assert it surfaces through `ComInvokeExceptionInfo`.
pub const DUAL_RAISE_ERROR_SOURCE: &str = "OxVba.DualFixture";
pub const DUAL_RAISE_ERROR_DESCRIPTION: &str = "controlled vtable error via SetErrorInfo";
/// The failure HRESULT `raise_error` returns (DISP_E_EXCEPTION).
#[cfg(target_os = "windows")]
pub const DUAL_RAISE_ERROR_HRESULT: i32 = COM_DISP_E_EXCEPTION;

#[cfg(target_os = "windows")]
// SAFETY: oleaut32 export transcribed with the stdcall `system` ABI;
// SetErrorInfo installs the thread's current error object (or clears it on a
// null pointer). The call site passes a live IErrorInfo it owns one ref of.
unsafe extern "system" {
    fn SetErrorInfo(dwreserved: u32, perrinfo: *mut core::ffi::c_void) -> i32;
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawDualVtbl {
    /// The 7 standard IUnknown+IDispatch slots (indices 0..=6).
    dispatch: RawIDispatchVtbl,
    /// slot 7
    get_count: unsafe extern "system" fn(this: *mut core::ffi::c_void, out: *mut i32) -> i32,
    /// slot 8
    exists: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        key: i32,
        out: *mut VARIANT_BOOL,
    ) -> i32,
    /// slot 9
    put_value: unsafe extern "system" fn(this: *mut core::ffi::c_void, value: *mut VARIANT) -> i32,
    /// slot 10
    lookup: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        key: windows_sys::core::BSTR,
        out: *mut *mut RawIDispatch,
    ) -> i32,
    /// slot 11
    raise_error: unsafe extern "system" fn(this: *mut core::ffi::c_void, out: *mut i32) -> i32,
    /// slot 12: `get_Price(this, [out,retval] CY*)`
    get_price: unsafe extern "system" fn(this: *mut core::ffi::c_void, out: *mut CY) -> i32,
    /// slot 13: `get_Created(this, [out,retval] DATE* as f64)`
    get_created: unsafe extern "system" fn(this: *mut core::ffi::c_void, out: *mut f64) -> i32,
    /// slot 14: `get_Owner(this, [out,retval] IUnknown**)`
    get_owner:
        unsafe extern "system" fn(this: *mut core::ffi::c_void, out: *mut *mut RawIUnknown) -> i32,
    /// slot 15
    validate_all_inputs: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        byte_value: u8,
        integer_value: i16,
        long_value: i32,
        longlong_value: i64,
        single_value: f32,
        double_value: f64,
        currency_value: i64,
        date_value: f64,
        bool_value: VARIANT_BOOL,
        text_value: windows_sys::core::BSTR,
        variant_value: *mut VARIANT,
        object_value: *mut RawIDispatch,
        out: *mut VARIANT_BOOL,
    ) -> i32,
    /// slot 16
    get_byte_value: unsafe extern "system" fn(this: *mut core::ffi::c_void, out: *mut u8) -> i32,
    /// slot 17
    get_integer_value:
        unsafe extern "system" fn(this: *mut core::ffi::c_void, out: *mut i16) -> i32,
    /// slot 18
    get_longlong_value:
        unsafe extern "system" fn(this: *mut core::ffi::c_void, out: *mut i64) -> i32,
    /// slot 19
    get_single_value: unsafe extern "system" fn(this: *mut core::ffi::c_void, out: *mut f32) -> i32,
    /// slot 20
    get_double_value: unsafe extern "system" fn(this: *mut core::ffi::c_void, out: *mut f64) -> i32,
    /// slot 21
    get_text_value: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        out: *mut windows_sys::core::BSTR,
    ) -> i32,
    /// slot 22
    get_variant_value:
        unsafe extern "system" fn(this: *mut core::ffi::c_void, out: *mut VARIANT) -> i32,
    /// slot 23
    putref_object_value:
        unsafe extern "system" fn(this: *mut core::ffi::c_void, value: *mut RawIDispatch) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaDualObject {
    vtbl: *const RawDualVtbl,
    ref_count: AtomicU32,
    /// Last value stashed by `put_Value`, so the unit test can round-trip it.
    last_put_value: AtomicI32,
}

#[cfg(target_os = "windows")]
static OXVBA_DUAL_VTBL: RawDualVtbl = RawDualVtbl {
    dispatch: RawIDispatchVtbl {
        unknown: RawIUnknownVtbl {
            query_interface: oxvba_dual_query_interface,
            add_ref: oxvba_dual_add_ref,
            release: oxvba_dual_release,
        },
        get_type_info_count: oxvba_dual_get_type_info_count,
        get_type_info: oxvba_dual_get_type_info,
        get_ids_of_names: oxvba_dual_get_ids_of_names,
        invoke: oxvba_dual_invoke,
    },
    get_count: oxvba_dual_get_count,
    exists: oxvba_dual_exists,
    put_value: oxvba_dual_put_value,
    lookup: oxvba_dual_lookup,
    raise_error: oxvba_dual_raise_error,
    get_price: oxvba_dual_get_price,
    get_created: oxvba_dual_get_created,
    get_owner: oxvba_dual_get_owner,
    validate_all_inputs: oxvba_dual_validate_all_inputs,
    get_byte_value: oxvba_dual_get_byte_value,
    get_integer_value: oxvba_dual_get_integer_value,
    get_longlong_value: oxvba_dual_get_longlong_value,
    get_single_value: oxvba_dual_get_single_value,
    get_double_value: oxvba_dual_get_double_value,
    get_text_value: oxvba_dual_get_text_value,
    get_variant_value: oxvba_dual_get_variant_value,
    putref_object_value: oxvba_dual_putref_object_value,
};

/// Construct the real custom dual-vtable fixture object. Returns the `this`
/// pointer (a `*const *const fnptr` whose vtable is [`OXVBA_DUAL_VTBL`]) with one
/// reference; release it with `oxvba_dual_release` (slot 2) or by driving the
/// vtable. The unit test owns the single reference.
#[cfg(target_os = "windows")]
pub fn create_oxvba_dual_vtable_object() -> *mut core::ffi::c_void {
    let object = Box::new(OxvbaDualObject {
        vtbl: &OXVBA_DUAL_VTBL,
        ref_count: AtomicU32::new(1),
        last_put_value: AtomicI32::new(0),
    });
    Box::into_raw(object).cast::<core::ffi::c_void>()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_dual(this: *mut core::ffi::c_void) -> *mut OxvbaDualObject {
    this.cast::<OxvbaDualObject>()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    if riid.is_null() {
        return COM_E_NOINTERFACE;
    }
    // S5a: also answer the custom dual interface IID (returning `this`, which in
    // an in-process dual aliases the IDispatch vtable). This is what the vtable
    // dispatch path QueryInterfaces for before calling a custom slot.
    if guid_equals(riid, &IID_IUNKNOWN)
        || guid_equals(riid, &IID_IDISPATCH)
        || guid_equals(riid, &IID_OXVBA_DUAL_FIXTURE)
    {
        *ppv = this;
        let _ = oxvba_dual_add_ref(this);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_add_ref(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_dual(this);
    (*owner).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_release(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_dual(this);
    let remaining = (*owner).ref_count.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        drop(Box::from_raw(owner));
    }
    remaining
}

// The IDispatch slots are present so the layout matches a real dual interface,
// but the S2 marshaller calls the custom slots directly; these are minimal.
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_type_info_count(
    _this: *mut core::ffi::c_void,
    pctinfo: *mut u32,
) -> i32 {
    if !pctinfo.is_null() {
        *pctinfo = 0;
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_type_info(
    _this: *mut core::ffi::c_void,
    _itinfo: u32,
    _lcid: u32,
    pptinfo: *mut *mut core::ffi::c_void,
) -> i32 {
    if !pptinfo.is_null() {
        *pptinfo = std::ptr::null_mut();
    }
    COM_E_NOTIMPL
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_ids_of_names(
    _this: *mut core::ffi::c_void,
    _riid: *const windows_sys::core::GUID,
    _names: *mut *mut u16,
    _count: u32,
    _lcid: u32,
    _dispids: *mut i32,
) -> i32 {
    COM_DISP_E_UNKNOWNNAME
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_invoke(
    _this: *mut core::ffi::c_void,
    _dispid: i32,
    _riid: *const windows_sys::core::GUID,
    _lcid: u32,
    _flags: u16,
    _params: *mut DISPPARAMS,
    _result: *mut VARIANT,
    _excep: *mut EXCEPINFO,
    _arg_err: *mut u32,
) -> i32 {
    // The dual fixture is exercised via its vtable slots, not Invoke.
    COM_DISP_E_MEMBERNOTFOUND
}

// ── Custom dual-interface slots (indices 7..) ──

/// slot 7: `get_Count(this, [out,retval] i32*)` — the simplest member; proves
/// this-ptr + out-cell + HRESULT + slot-index addressing in isolation.
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_count(
    _this: *mut core::ffi::c_void,
    out: *mut i32,
) -> i32 {
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    *out = 7;
    COM_S_OK
}

/// slot 8: `Exists(this, i32 key, [out,retval] VARIANT_BOOL*)`.
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_exists(
    _this: *mut core::ffi::c_void,
    key: i32,
    out: *mut VARIANT_BOOL,
) -> i32 {
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    *out = if key == 42 { -1 } else { 0 };
    COM_S_OK
}

/// slot 9: `put_Value(this, [in] VARIANT*)` — stashes the i32 payload so the
/// test can round-trip it via `get_Count`-style read or the returned status.
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_put_value(
    this: *mut core::ffi::c_void,
    value: *mut VARIANT,
) -> i32 {
    if value.is_null() {
        return COM_E_INVALIDARG;
    }
    let owner = as_oxvba_dual(this);
    match com_variant_to_com_value(&*value) {
        Ok(ComValue::I32(v)) => {
            (*owner).last_put_value.store(v, Ordering::Release);
            COM_S_OK
        }
        Ok(_) => COM_DISP_E_TYPEMISMATCH,
        Err(_) => COM_DISP_E_TYPEMISMATCH,
    }
}

/// slot 10: `Lookup(this, [in] BSTR key, [out,retval] IDispatch**)` — returns a
/// fresh `OxVba.TestDispatch` object (AddRef'd) when the key is non-empty.
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_lookup(
    _this: *mut core::ffi::c_void,
    key: windows_sys::core::BSTR,
    out: *mut *mut RawIDispatch,
) -> i32 {
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    *out = std::ptr::null_mut();
    if key.is_null() {
        return COM_E_INVALIDARG;
    }
    // create_oxvba_test_dispatch returns a fresh object with one reference; the
    // [out,retval] convention transfers that reference to the caller.
    *out = create_oxvba_test_dispatch();
    COM_S_OK
}

/// slot 11: `raise_error(this, [out,retval] i32*)` — installs a rich
/// IErrorInfo via SetErrorInfo, then returns a failure HRESULT (the out-cell is
/// left zeroed; the caller must not read it on failure).
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_raise_error(
    _this: *mut core::ffi::c_void,
    out: *mut i32,
) -> i32 {
    if !out.is_null() {
        *out = 0;
    }
    let errinfo =
        create_oxvba_dual_error_info(DUAL_RAISE_ERROR_SOURCE, DUAL_RAISE_ERROR_DESCRIPTION);
    let _ = SetErrorInfo(0, errinfo.cast::<core::ffi::c_void>());
    DUAL_RAISE_ERROR_HRESULT
}

/// slot 12: `get_Price(this, [out,retval] CY*)` — writes a currency value (i64
/// scaled ×10000) so the marshaller's `OutCell::Currency` decoder is exercised.
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_price(this: *mut core::ffi::c_void, out: *mut CY) -> i32 {
    let _ = this;
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    // A CY is an 8-byte i64 union; write the scaled value directly.
    (*out).int64 = DUAL_PRICE_SCALED_I64;
    COM_S_OK
}

/// slot 13: `get_Created(this, [out,retval] DATE*)` — writes an OLE date (f64) so
/// the marshaller's date out-cell decoder is exercised.
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_created(
    this: *mut core::ffi::c_void,
    out: *mut f64,
) -> i32 {
    let _ = this;
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    *out = DUAL_CREATED_OLE_DATE;
    COM_S_OK
}

/// slot 14: `get_Owner(this, [out,retval] IUnknown**)` — returns a fresh
/// `OxVba.TestDispatch` as a bare `IUnknown` (AddRef'd, ownership transferred), so
/// the marshaller must `QueryInterface(IDispatch)` the returned IUnknown
/// (`query_dispatch_from_unknown`) to bind it as an object result.
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_owner(
    this: *mut core::ffi::c_void,
    out: *mut *mut RawIUnknown,
) -> i32 {
    let _ = this;
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    // create_oxvba_test_dispatch returns a fresh IDispatch with one reference; its
    // first field is its IUnknown vtable, so it casts to IUnknown* directly and the
    // [out,retval] convention transfers that single reference to the caller.
    *out = create_oxvba_test_dispatch().cast::<RawIUnknown>();
    COM_S_OK
}

#[cfg(target_os = "windows")]
unsafe fn variant_i32_value(variant: *mut VARIANT) -> Option<i32> {
    if variant.is_null() || (*variant).Anonymous.Anonymous.vt != VT_I4 {
        return None;
    }
    Some((*variant).Anonymous.Anonymous.Anonymous.lVal)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn, clippy::too_many_arguments)]
unsafe extern "system" fn oxvba_dual_validate_all_inputs(
    _this: *mut core::ffi::c_void,
    byte_value: u8,
    integer_value: i16,
    long_value: i32,
    longlong_value: i64,
    single_value: f32,
    double_value: f64,
    currency_value: i64,
    date_value: f64,
    bool_value: VARIANT_BOOL,
    text_value: windows_sys::core::BSTR,
    variant_value: *mut VARIANT,
    object_value: *mut RawIDispatch,
    out: *mut VARIANT_BOOL,
) -> i32 {
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    let ok = byte_value == 9
        && integer_value == -12
        && long_value == 34_567
        && longlong_value == DUAL_LONGLONG_VALUE
        && (single_value - 1.5).abs() < f32::EPSILON
        && (double_value + 2.25).abs() < f64::EPSILON
        && currency_value == DUAL_PRICE_SCALED_I64
        && (date_value - DUAL_CREATED_OLE_DATE).abs() < f64::EPSILON
        && bool_value != 0
        && !text_value.is_null()
        && variant_i32_value(variant_value) == Some(1234)
        && !object_value.is_null();
    *out = if ok { -1 } else { 0 };
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_byte_value(
    _this: *mut core::ffi::c_void,
    out: *mut u8,
) -> i32 {
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    *out = DUAL_BYTE_VALUE;
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_integer_value(
    _this: *mut core::ffi::c_void,
    out: *mut i16,
) -> i32 {
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    *out = DUAL_INTEGER_VALUE;
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_longlong_value(
    _this: *mut core::ffi::c_void,
    out: *mut i64,
) -> i32 {
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    *out = DUAL_LONGLONG_VALUE;
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_single_value(
    _this: *mut core::ffi::c_void,
    out: *mut f32,
) -> i32 {
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    *out = DUAL_SINGLE_VALUE;
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_double_value(
    _this: *mut core::ffi::c_void,
    out: *mut f64,
) -> i32 {
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    *out = DUAL_DOUBLE_VALUE;
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_text_value(
    _this: *mut core::ffi::c_void,
    out: *mut windows_sys::core::BSTR,
) -> i32 {
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    let wide: Vec<u16> = DUAL_TEXT_VALUE
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    *out = SysAllocString(wide.as_ptr());
    if (*out).is_null() {
        return COM_E_INVALIDARG;
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_get_variant_value(
    _this: *mut core::ffi::c_void,
    out: *mut VARIANT,
) -> i32 {
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    (*out).Anonymous.Anonymous.vt = VT_I4;
    (*out).Anonymous.Anonymous.Anonymous.lVal = DUAL_VARIANT_VALUE;
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_putref_object_value(
    this: *mut core::ffi::c_void,
    value: *mut RawIDispatch,
) -> i32 {
    if value.is_null() {
        return COM_E_INVALIDARG;
    }
    let owner = as_oxvba_dual(this);
    (*owner).last_put_value.store(777, Ordering::Release);
    COM_S_OK
}

// ── Minimal IErrorInfo implementation for the raise_error slot ──

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawErrorInfoVtbl {
    query_interface: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        riid: *const windows_sys::core::GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32,
    get_guid: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        pguid: *mut windows_sys::core::GUID,
    ) -> i32,
    get_source: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        pbstr: *mut windows_sys::core::BSTR,
    ) -> i32,
    get_description: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        pbstr: *mut windows_sys::core::BSTR,
    ) -> i32,
    get_help_file: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        pbstr: *mut windows_sys::core::BSTR,
    ) -> i32,
    get_help_context: unsafe extern "system" fn(this: *mut core::ffi::c_void, pdw: *mut u32) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaDualErrorInfo {
    vtbl: *const RawErrorInfoVtbl,
    ref_count: AtomicU32,
    source: Vec<u16>,
    description: Vec<u16>,
}

#[cfg(target_os = "windows")]
static OXVBA_DUAL_ERRORINFO_VTBL: RawErrorInfoVtbl = RawErrorInfoVtbl {
    query_interface: oxvba_dual_errorinfo_query_interface,
    add_ref: oxvba_dual_errorinfo_add_ref,
    release: oxvba_dual_errorinfo_release,
    get_guid: oxvba_dual_errorinfo_get_guid,
    get_source: oxvba_dual_errorinfo_get_source,
    get_description: oxvba_dual_errorinfo_get_description,
    get_help_file: oxvba_dual_errorinfo_get_help_file,
    get_help_context: oxvba_dual_errorinfo_get_help_context,
};

#[cfg(target_os = "windows")]
fn create_oxvba_dual_error_info(source: &str, description: &str) -> *mut core::ffi::c_void {
    let object = Box::new(OxvbaDualErrorInfo {
        vtbl: &OXVBA_DUAL_ERRORINFO_VTBL,
        ref_count: AtomicU32::new(1),
        source: source.encode_utf16().chain(std::iter::once(0)).collect(),
        description: description
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect(),
    });
    Box::into_raw(object).cast::<core::ffi::c_void>()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_dual_error_info(this: *mut core::ffi::c_void) -> *mut OxvbaDualErrorInfo {
    this.cast::<OxvbaDualErrorInfo>()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_errorinfo_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    if riid.is_null() {
        return COM_E_NOINTERFACE;
    }
    if guid_equals(riid, &IID_IUNKNOWN) || guid_equals(riid, &IID_IERRORINFO) {
        *ppv = this;
        let _ = oxvba_dual_errorinfo_add_ref(this);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_errorinfo_add_ref(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_dual_error_info(this);
    (*owner).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_errorinfo_release(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_dual_error_info(this);
    let remaining = (*owner).ref_count.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        drop(Box::from_raw(owner));
    }
    remaining
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_errorinfo_get_guid(
    _this: *mut core::ffi::c_void,
    pguid: *mut windows_sys::core::GUID,
) -> i32 {
    if !pguid.is_null() {
        *pguid = IID_NULL;
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_errorinfo_get_source(
    this: *mut core::ffi::c_void,
    pbstr: *mut windows_sys::core::BSTR,
) -> i32 {
    if pbstr.is_null() {
        return COM_E_INVALIDARG;
    }
    let owner = as_oxvba_dual_error_info(this);
    *pbstr = SysAllocString((*owner).source.as_ptr());
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_errorinfo_get_description(
    this: *mut core::ffi::c_void,
    pbstr: *mut windows_sys::core::BSTR,
) -> i32 {
    if pbstr.is_null() {
        return COM_E_INVALIDARG;
    }
    let owner = as_oxvba_dual_error_info(this);
    *pbstr = SysAllocString((*owner).description.as_ptr());
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_errorinfo_get_help_file(
    _this: *mut core::ffi::c_void,
    pbstr: *mut windows_sys::core::BSTR,
) -> i32 {
    if !pbstr.is_null() {
        *pbstr = std::ptr::null_mut();
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_dual_errorinfo_get_help_context(
    _this: *mut core::ffi::c_void,
    pdw: *mut u32,
) -> i32 {
    if !pdw.is_null() {
        *pdw = 0;
    }
    COM_S_OK
}
