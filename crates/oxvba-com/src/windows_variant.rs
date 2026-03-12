use crate::ComValue;
use oxvba_runtime::{ObjectHandle, bstr::BStr, safe_array::SafeArray};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{SysAllocString, SysStringLen, VARIANT_BOOL};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Com::SAFEARRAY;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Ole::{
    SafeArrayCreateVector, SafeArrayDestroy, SafeArrayGetDim, SafeArrayGetElement,
    SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayGetVartype, SafeArrayPutElement,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Variant::{
    VARIANT, VT_ARRAY, VT_BOOL, VT_BSTR, VT_EMPTY, VT_ERROR, VT_I2, VT_I4, VT_NULL, VT_UI2, VT_UI4,
    VT_VARIANT, VariantClear,
};

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn alloc_bstr(text: &str) -> windows_sys::core::BSTR {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    SysAllocString(wide.as_ptr())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn safe_array_to_com_value(psa: *mut SAFEARRAY) -> Result<ComValue, String> {
    if psa.is_null() {
        return Err("VT_ARRAY result carried null SAFEARRAY".to_string());
    }
    let dims = SafeArrayGetDim(psa.cast_const());
    if dims != 1 {
        return Err(format!(
            "unsupported SAFEARRAY rank {dims}; only one-dimensional VT_VARIANT arrays are supported"
        ));
    }
    let mut element_vt = 0u16;
    let hr = SafeArrayGetVartype(psa.cast_const(), &mut element_vt);
    if hr < 0 {
        return Err(format!(
            "SafeArrayGetVartype failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    if element_vt != VT_VARIANT {
        return Err(format!(
            "unsupported SAFEARRAY element vartype {element_vt}; only VT_VARIANT arrays are supported"
        ));
    }
    let mut lower = 0i32;
    let hr = SafeArrayGetLBound(psa.cast_const(), 1, &mut lower);
    if hr < 0 {
        return Err(format!(
            "SafeArrayGetLBound failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    let mut upper = -1i32;
    let hr = SafeArrayGetUBound(psa.cast_const(), 1, &mut upper);
    if hr < 0 {
        return Err(format!(
            "SafeArrayGetUBound failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    let len = if upper < lower {
        0usize
    } else {
        usize::try_from(upper - lower + 1)
            .map_err(|_| "SAFEARRAY bounds exceed supported usize range".to_string())?
    };
    let mut values = Vec::with_capacity(len);
    for index in lower..=upper {
        let mut element: VARIANT = std::mem::zeroed();
        let hr = SafeArrayGetElement(
            psa.cast_const(),
            &index,
            (&mut element as *mut VARIANT).cast(),
        );
        if hr < 0 {
            return Err(format!(
                "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
        let value = match variant_to_com_value(&element) {
            Ok(value) => value.to_runtime_value(),
            Err(detail) => {
                let _ = VariantClear(&mut element);
                return Err(detail);
            }
        };
        let _ = VariantClear(&mut element);
        values.push(value);
    }
    Ok(ComValue::ArrayIntent(SafeArray::from_values(values)))
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_array_arg<FResolve, FAddRef>(
    variant: *mut VARIANT,
    array: &SafeArray,
    resolve_object: &mut FResolve,
    add_ref_dispatch: &mut FAddRef,
) -> Result<(), String>
where
    FResolve: FnMut(ObjectHandle) -> Result<*mut core::ffi::c_void, String>,
    FAddRef: FnMut(*mut core::ffi::c_void),
{
    let Some(values) = array.elements.as_ref() else {
        (*variant).Anonymous.Anonymous.vt = VT_I4;
        (*variant).Anonymous.Anonymous.Anonymous.lVal =
            ComValue::ArrayIntent(array.clone()).to_legacy_dispatch_token()?;
        return Ok(());
    };
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_VARIANT, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_VARIANT) returned null".to_string());
    }
    for (offset, runtime_value) in values.iter().enumerate() {
        let mut element: VARIANT = std::mem::zeroed();
        let value = ComValue::from_runtime_value(runtime_value);
        if let Err(detail) =
            set_variant_from_com_value(&mut element, &value, resolve_object, add_ref_dispatch)
        {
            let _ = VariantClear(&mut element);
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(detail);
        }
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(
            psa.cast_const(),
            &index,
            (&element as *const VARIANT).cast(),
        );
        let _ = VariantClear(&mut element);
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_VARIANT;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// Convert a Windows `VARIANT` carrying the supported scalar/string/one-dimensional-`VT_VARIANT`
/// SAFEARRAY subset into the shared semantic `ComValue` carrier.
///
/// # Safety
/// The caller must provide a valid initialized `VARIANT` reference whose pointed-to storage
/// remains alive for the duration of this call. If the variant contains nested COM-owned payloads,
/// they must satisfy the ownership rules required by the Windows `Variant`/`SafeArray` APIs.
pub unsafe fn variant_to_com_value(variant: &VARIANT) -> Result<ComValue, String> {
    let vt = variant.Anonymous.Anonymous.vt;
    if vt & VT_ARRAY != 0 {
        let element_vt = vt & !VT_ARRAY;
        if element_vt != VT_VARIANT {
            return Err(format!("unsupported VARIANT return type vt={vt}"));
        }
        let parray = variant.Anonymous.Anonymous.Anonymous.parray;
        return safe_array_to_com_value(parray);
    }
    let value = match vt {
        VT_EMPTY => ComValue::Empty,
        VT_I2 => ComValue::I32(variant.Anonymous.Anonymous.Anonymous.iVal as i32),
        VT_I4 => ComValue::I32(variant.Anonymous.Anonymous.Anonymous.lVal),
        VT_UI2 => ComValue::I32(variant.Anonymous.Anonymous.Anonymous.uiVal as i32),
        VT_UI4 => ComValue::I32(variant.Anonymous.Anonymous.Anonymous.ulVal as i32),
        VT_BOOL => {
            let value: VARIANT_BOOL = variant.Anonymous.Anonymous.Anonymous.boolVal;
            ComValue::Bool(value != 0)
        }
        VT_BSTR => {
            let bstr = variant.Anonymous.Anonymous.Anonymous.bstrVal;
            let text = if bstr.is_null() {
                String::new()
            } else {
                let len = usize::try_from(SysStringLen(bstr)).unwrap_or(0);
                let slice = std::slice::from_raw_parts(bstr, len);
                String::from_utf16_lossy(slice)
            };
            ComValue::String(BStr(text))
        }
        VT_NULL => ComValue::Null,
        VT_ERROR => ComValue::ErrorCode(variant.Anonymous.Anonymous.Anonymous.scode),
        vt => {
            return Err(format!("unsupported VARIANT return type vt={vt}"));
        }
    };
    Ok(value)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// Populate a Windows `VARIANT` from the shared semantic `ComValue` carrier for the currently
/// supported scalar/string/one-dimensional-`VT_VARIANT` SAFEARRAY subset and `VT_DISPATCH`
/// object-handle lane.
///
/// # Safety
/// The caller must provide a valid writable `VARIANT` pointer. `resolve_object` must return a live
/// `IDispatch` pointer for any provided `ObjectHandle`, and `add_ref_dispatch` must apply the
/// matching COM reference-count increment to that pointer before the variant assumes ownership.
pub unsafe fn set_variant_from_com_value<FResolve, FAddRef>(
    variant: *mut VARIANT,
    value: &ComValue,
    resolve_object: &mut FResolve,
    add_ref_dispatch: &mut FAddRef,
) -> Result<(), String>
where
    FResolve: FnMut(ObjectHandle) -> Result<*mut core::ffi::c_void, String>,
    FAddRef: FnMut(*mut core::ffi::c_void),
{
    if variant.is_null() {
        return Ok(());
    }
    match value {
        ComValue::Empty => {
            (*variant).Anonymous.Anonymous.vt = VT_EMPTY;
        }
        ComValue::Null => {
            (*variant).Anonymous.Anonymous.vt = VT_NULL;
        }
        ComValue::ErrorCode(code) => {
            (*variant).Anonymous.Anonymous.vt = VT_ERROR;
            (*variant).Anonymous.Anonymous.Anonymous.scode = *code;
        }
        ComValue::Bool(value) => {
            (*variant).Anonymous.Anonymous.vt = VT_BOOL;
            (*variant).Anonymous.Anonymous.Anonymous.boolVal = if *value { -1 } else { 0 };
        }
        ComValue::I32(value) => {
            (*variant).Anonymous.Anonymous.vt = VT_I4;
            (*variant).Anonymous.Anonymous.Anonymous.lVal = *value;
        }
        ComValue::String(BStr(value)) => {
            (*variant).Anonymous.Anonymous.vt = VT_BSTR;
            (*variant).Anonymous.Anonymous.Anonymous.bstrVal = alloc_bstr(value);
        }
        ComValue::ArrayIntent(array) => {
            set_variant_array_arg(variant, array, resolve_object, add_ref_dispatch)?;
        }
        ComValue::ObjectHandle(handle) => {
            let dispatch = resolve_object(*handle)?;
            if dispatch.is_null() {
                return Err("object handle resolved to null IDispatch pointer".to_string());
            }
            add_ref_dispatch(dispatch);
            (*variant).Anonymous.Anonymous.vt = windows_sys::Win32::System::Variant::VT_DISPATCH;
            (*variant).Anonymous.Anonymous.Anonymous.pdispVal = dispatch;
        }
    }
    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::{set_variant_from_com_value, variant_to_com_value};
    use crate::ComValue;
    use oxvba_runtime::{RuntimeValue, bstr::BStr, safe_array::SafeArray};
    use windows_sys::Win32::System::Variant::{VARIANT, VT_ARRAY, VT_VARIANT, VariantClear};

    #[test]
    fn string_variant_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let value = ComValue::String(BStr("Hello".to_string()));
        let mut resolve_object =
            |_handle| Err("object dispatch resolution not expected".to_string());
        let mut add_ref = |_dispatch| {};
        unsafe {
            set_variant_from_com_value(&mut variant, &value, &mut resolve_object, &mut add_ref)
                .expect("set string variant");
            assert_eq!(
                variant_to_com_value(&variant).expect("read string variant"),
                value
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn safe_array_variant_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let value = ComValue::ArrayIntent(SafeArray::from_values(vec![
            RuntimeValue::I32(4),
            RuntimeValue::Bool(true),
            RuntimeValue::String(BStr("Hello".to_string())),
            RuntimeValue::Null,
        ]));
        let mut resolve_object =
            |_handle| Err("object dispatch resolution not expected".to_string());
        let mut add_ref = |_dispatch| {};
        unsafe {
            set_variant_from_com_value(&mut variant, &value, &mut resolve_object, &mut add_ref)
                .expect("set SAFEARRAY variant");
            assert_eq!(variant.Anonymous.Anonymous.vt, VT_ARRAY | VT_VARIANT);
            assert_eq!(
                variant_to_com_value(&variant).expect("read SAFEARRAY variant"),
                value
            );
            let _ = VariantClear(&mut variant);
        }
    }
}
