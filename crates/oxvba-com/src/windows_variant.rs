use crate::ComValue;
use oxvba_runtime::{
    CurrencyValue, Decimal96, F64Subtype, F64Value, ObjectRef,
    bstr::BStr,
    safe_array::{
        SafeArray, SafeArrayBound, VT_BOOL_VALUE, VT_BSTR_VALUE, VT_CY_VALUE, VT_DATE_VALUE,
        VT_DECIMAL_VALUE, VT_DISPATCH_VALUE, VT_I1_VALUE, VT_I2_VALUE, VT_I4_VALUE, VT_I8_VALUE,
        VT_INT_VALUE, VT_R4_VALUE, VT_R8_VALUE, VT_UI1_VALUE, VT_UI2_VALUE, VT_UI4_VALUE,
        VT_UI8_VALUE, VT_UINT_VALUE, VT_UNKNOWN_VALUE, VT_VARIANT_VALUE,
    },
};
use std::ffi::c_void;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{
    DECIMAL, SysAllocStringLen, SysFreeString, SysStringLen, VARIANT_BOOL,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Com::{CY, SAFEARRAY, SAFEARRAYBOUND};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Ole::{
    SafeArrayCreate, SafeArrayCreateVector, SafeArrayDestroy, SafeArrayGetDim, SafeArrayGetElement,
    SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayGetVartype, SafeArrayPutElement,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Variant::{
    VARIANT, VT_ARRAY, VT_BOOL, VT_BSTR, VT_BYREF, VT_DECIMAL, VT_DISPATCH, VT_EMPTY, VT_ERROR,
    VT_I1, VT_I2, VT_I4, VT_I8, VT_INT, VT_NULL, VT_UI1, VT_UI2, VT_UI4, VT_UI8, VT_UINT,
    VT_UNKNOWN, VT_VARIANT, VariantClear,
};

const VT_R4_VARENUM: u16 = 4;
const VT_R8_VARENUM: u16 = 5;
const VT_CY_VARENUM: u16 = 6;
const VT_DATE_VARENUM: u16 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantResultValue {
    Value(ComValue),
    Dispatch(*mut c_void),
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn, clippy::too_many_arguments)]
unsafe fn enumvariant_to_runtime_value<FQueryDispatch, FAddRefDispatch, FBindDispatch>(
    enum_variant: *mut crate::windows_client::RawIEnumVARIANT,
    query_dispatch_from_unknown: &mut FQueryDispatch,
    add_ref_dispatch: &mut FAddRefDispatch,
    bind_dispatch_result: &mut FBindDispatch,
    prog_id_hint: &str,
    op: &'static str,
) -> Result<oxvba_runtime::RuntimeValue, String>
where
    FQueryDispatch: FnMut(*mut c_void) -> Result<*mut c_void, String>,
    FAddRefDispatch: FnMut(*mut c_void),
    FBindDispatch:
        FnMut(*mut c_void, &str, &'static str) -> Result<oxvba_runtime::RuntimeValue, String>,
{
    let mut values = Vec::new();
    loop {
        let mut element: VARIANT = std::mem::zeroed();
        let mut fetched = 0u32;
        let hr = ((*(*enum_variant).vtbl).next)(enum_variant.cast(), 1, &mut element, &mut fetched);
        if hr < 0 {
            return Err(format!(
                "IEnumVARIANT::Next failed with HRESULT {:#010X}",
                hr as u32
            ));
        }
        if fetched == 0 {
            break;
        }
        let value = match variant_to_runtime_value(
            &element,
            query_dispatch_from_unknown,
            add_ref_dispatch,
            bind_dispatch_result,
            prog_id_hint,
            op,
        ) {
            Ok(value) => value,
            Err(detail) => {
                let _ = VariantClear(&mut element);
                return Err(detail);
            }
        };
        let _ = VariantClear(&mut element);
        values.push(value);
    }
    Ok(oxvba_runtime::RuntimeValue::ArrayIntent(
        SafeArray::from_values(values),
    ))
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn, clippy::too_many_arguments)]
unsafe fn unknown_to_runtime_value<FQueryDispatch, FAddRefDispatch, FBindDispatch>(
    unknown: *mut c_void,
    query_dispatch_from_unknown: &mut FQueryDispatch,
    add_ref_dispatch: &mut FAddRefDispatch,
    bind_dispatch_result: &mut FBindDispatch,
    prog_id_hint: &str,
    op: &'static str,
) -> Result<oxvba_runtime::RuntimeValue, String>
where
    FQueryDispatch: FnMut(*mut c_void) -> Result<*mut c_void, String>,
    FAddRefDispatch: FnMut(*mut c_void),
    FBindDispatch:
        FnMut(*mut c_void, &str, &'static str) -> Result<oxvba_runtime::RuntimeValue, String>,
{
    match query_dispatch_from_unknown(unknown) {
        Ok(dispatch) => bind_dispatch_result(dispatch, prog_id_hint, op),
        Err(dispatch_err) => {
            let enum_variant = match crate::windows_client::query_enumvariant_from_unknown(
                unknown.cast::<crate::windows_client::RawIUnknown>(),
            ) {
                Ok(enum_variant) => enum_variant,
                Err(_) => return Err(dispatch_err),
            };
            let result = enumvariant_to_runtime_value(
                enum_variant,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
                prog_id_hint,
                op,
            );
            crate::windows_client::release_enumvariant(enum_variant);
            result
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
// `oxvba-com` owns the COM wire seam. The current runtime still stores
// semantic-first strings, so BSTR allocation remains an explicit projection at
// this boundary.
unsafe fn alloc_bstr(text: &BStr) -> windows_sys::core::BSTR {
    let core = text.owned_core();
    let len =
        u32::try_from(core.len_code_units()).expect("BSTR UTF-16 code-unit length should fit u32");
    SysAllocStringLen(core.payload_ptr(), len)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn runtime_bstr_from_windows(bstr: windows_sys::core::BSTR) -> BStr {
    if bstr.is_null() {
        return BStr::empty();
    }
    let len = usize::try_from(SysStringLen(bstr)).unwrap_or(0);
    let slice = std::slice::from_raw_parts(bstr, len);
    BStr::from_utf16_lossy(slice)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn decimal96_from_windows(decimal: DECIMAL) -> Decimal96 {
    Decimal96::from_scale_sign(
        decimal.Anonymous2.Anonymous.Lo32,
        decimal.Anonymous2.Anonymous.Mid32,
        decimal.Hi32,
        (u16::from(decimal.Anonymous1.Anonymous.sign) << 8)
            | u16::from(decimal.Anonymous1.Anonymous.scale),
    )
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn decimal96_to_windows(value: Decimal96) -> DECIMAL {
    let mut decimal: DECIMAL = std::mem::zeroed();
    decimal.wReserved = 0;
    decimal.Anonymous1.Anonymous.scale = value.scale();
    decimal.Anonymous1.Anonymous.sign = if value.is_negative() { 0x80 } else { 0 };
    decimal.Hi32 = value.hi;
    decimal.Anonymous2.Anonymous.Lo32 = value.lo;
    decimal.Anonymous2.Anonymous.Mid32 = value.mid;
    decimal
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
// SAFEARRAY/VARIANT decoding is a boundary translation step. It reconstructs
// semantic `ComValue` payloads from COM wire values rather than exposing the
// canonical runtime carrier directly.
unsafe fn safe_array_to_com_value(psa: *mut SAFEARRAY) -> Result<ComValue, String> {
    if psa.is_null() {
        return Err("VT_ARRAY result carried null SAFEARRAY".to_string());
    }
    let dims = SafeArrayGetDim(psa.cast_const());
    if dims < 1 {
        return Err("VT_ARRAY result carried SAFEARRAY with zero dimensions".to_string());
    }
    let mut element_vt = 0u16;
    let hr = SafeArrayGetVartype(psa.cast_const(), &mut element_vt);
    if hr < 0 {
        return Err(format!(
            "SafeArrayGetVartype failed with HRESULT {:#010X}",
            hr as u32
        ));
    }

    // Collect per-dimension bounds (dimension indices are 1-based in SafeArray API).
    let mut bounds = Vec::with_capacity(dims as usize);
    let mut total_len: usize = 1;
    for dim in 1..=dims {
        let mut lower = 0i32;
        let hr = SafeArrayGetLBound(psa.cast_const(), dim, &mut lower);
        if hr < 0 {
            return Err(format!(
                "SafeArrayGetLBound(dim={dim}) failed with HRESULT {:#010X}",
                hr as u32
            ));
        }
        let mut upper = -1i32;
        let hr = SafeArrayGetUBound(psa.cast_const(), dim, &mut upper);
        if hr < 0 {
            return Err(format!(
                "SafeArrayGetUBound(dim={dim}) failed with HRESULT {:#010X}",
                hr as u32
            ));
        }
        let count = if upper < lower {
            0u32
        } else {
            u32::try_from(upper - lower + 1)
                .map_err(|_| "SAFEARRAY dimension bounds exceed supported u32 range".to_string())?
        };
        total_len = total_len.checked_mul(count as usize).ok_or_else(|| {
            "SAFEARRAY total element count exceeds supported usize range".to_string()
        })?;
        bounds.push(SafeArrayBound { lower, count });
    }

    let values = if dims == 1 {
        let lower = bounds[0].lower;
        let upper = lower + bounds[0].count as i32 - 1;
        let mut values = Vec::with_capacity(total_len);
        for index in lower..=upper {
            values.push(safe_array_element_to_runtime_value(
                psa.cast_const(),
                index,
                element_vt,
            )?);
        }
        values
    } else {
        let mut values = Vec::with_capacity(total_len);
        let mut indices: Vec<i32> = bounds.iter().map(|b| b.lower).collect();

        for _ in 0..total_len {
            let value = safe_array_element_nd(psa.cast_const(), &indices, element_vt)?;
            values.push(value);

            let mut carry = true;
            for (dim_idx, bound) in bounds.iter().enumerate() {
                if !carry {
                    break;
                }
                indices[dim_idx] += 1;
                if indices[dim_idx] >= bound.lower + bound.count as i32 {
                    indices[dim_idx] = bound.lower;
                } else {
                    carry = false;
                }
            }
        }
        values
    };

    let array = if element_vt != VT_VARIANT_VALUE
        && SafeArray::supports_intrinsic_element_vartype(element_vt)
    {
        if dims == 1 {
            SafeArray::from_typed_values(element_vt, values)?
        } else {
            SafeArray::from_typed_values_nd(bounds, element_vt, values)?
        }
    } else if dims == 1 {
        SafeArray::from_values(values)
    } else {
        SafeArray::from_values_nd(bounds, values)
    };
    Ok(ComValue::ArrayIntent(array))
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn safe_array_element_nd(
    psa: *const SAFEARRAY,
    indices: &[i32],
    element_vt: u16,
) -> Result<oxvba_runtime::RuntimeValue, String> {
    // SafeArrayGetElement accepts a pointer to an array of i32 indices for multi-dim access.
    if element_vt == VT_VARIANT {
        let mut element: VARIANT = std::mem::zeroed();
        let hr = SafeArrayGetElement(psa, indices.as_ptr(), (&mut element as *mut VARIANT).cast());
        if hr < 0 {
            return Err(format!(
                "SafeArrayGetElement failed with HRESULT {:#010X} at indices {indices:?}",
                hr as u32
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
        return Ok(value);
    }
    // For non-VARIANT typed arrays, delegate to single-element extraction with a flat index.
    // Multi-dimensional typed SAFEARRAYs still use sequential flat indexing internally.
    // SafeArrayGetElement with a single i32 pointer works for the innermost dimension
    // for typed arrays, but for true multi-dim typed access we use the indices array.
    // For typed elements that are scalar, SafeArrayGetElement with indices works directly.
    safe_array_element_typed_nd(psa, indices, element_vt)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn safe_array_element_typed_nd(
    psa: *const SAFEARRAY,
    indices: &[i32],
    element_vt: u16,
) -> Result<oxvba_runtime::RuntimeValue, String> {
    macro_rules! get_element {
        ($ty:ty, $indices:expr) => {{
            let mut element: $ty = std::mem::zeroed();
            let hr = SafeArrayGetElement(psa, $indices.as_ptr(), (&mut element as *mut $ty).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at indices {:?}",
                    hr as u32, $indices
                ));
            }
            element
        }};
    }
    match element_vt {
        VT_I1 => Ok(ComValue::I32(get_element!(i8, indices) as i32).to_runtime_value()),
        VT_I2 => Ok(ComValue::I32(get_element!(i16, indices) as i32).to_runtime_value()),
        VT_I4 => Ok(ComValue::I32(get_element!(i32, indices)).to_runtime_value()),
        VT_I8 => {
            let val = get_element!(i64, indices);
            match i32::try_from(val) {
                Ok(n) => Ok(ComValue::I32(n).to_runtime_value()),
                Err(_) => Ok(ComValue::I64(val).to_runtime_value()),
            }
        }
        VT_INT => Ok(ComValue::I32(get_element!(i32, indices)).to_runtime_value()),
        VT_UI1 => Ok(ComValue::I32(get_element!(u8, indices) as i32).to_runtime_value()),
        VT_UI2 => Ok(ComValue::I32(get_element!(u16, indices) as i32).to_runtime_value()),
        VT_UI4 => {
            let val = get_element!(u32, indices);
            match i32::try_from(val) {
                Ok(n) => Ok(ComValue::I32(n).to_runtime_value()),
                Err(_) => Ok(ComValue::I64(val as i64).to_runtime_value()),
            }
        }
        VT_UI8 => {
            let val = get_element!(u64, indices);
            match i32::try_from(val) {
                Ok(n) => Ok(ComValue::I32(n).to_runtime_value()),
                Err(_) => match i64::try_from(val) {
                    Ok(n) => Ok(ComValue::I64(n).to_runtime_value()),
                    Err(_) => Err(format!(
                        "VT_UI8 SAFEARRAY element {val} exceeds i64 carrier range"
                    )),
                },
            }
        }
        VT_R4_VARENUM => Ok(ComValue::F64(F64Value::from_single_f64(
            get_element!(f32, indices) as f64
        ))
        .to_runtime_value()),
        VT_R8_VARENUM => {
            Ok(ComValue::F64(F64Value::from_f64(get_element!(f64, indices))).to_runtime_value())
        }
        VT_CY_VARENUM => {
            let cy: CY = get_element!(CY, indices);
            Ok(ComValue::Currency(CurrencyValue::from_scaled_i64(cy.int64)).to_runtime_value())
        }
        VT_DATE_VARENUM => Ok(
            ComValue::F64(F64Value::from_date_f64(get_element!(f64, indices))).to_runtime_value(),
        ),
        VT_DECIMAL => {
            let dec: DECIMAL = get_element!(DECIMAL, indices);
            Ok(ComValue::Decimal(decimal96_from_windows(dec)).to_runtime_value())
        }
        VT_UINT => {
            let val = get_element!(u32, indices);
            match i32::try_from(val) {
                Ok(n) => Ok(ComValue::I32(n).to_runtime_value()),
                Err(_) => Ok(ComValue::I64(val as i64).to_runtime_value()),
            }
        }
        VT_BOOL => {
            let val = get_element!(VARIANT_BOOL, indices);
            Ok(ComValue::Bool(val != 0).to_runtime_value())
        }
        VT_BSTR => {
            let bstr: windows_sys::core::BSTR = get_element!(windows_sys::core::BSTR, indices);
            let text = if bstr.is_null() {
                String::new()
            } else {
                let len = usize::try_from(SysStringLen(bstr)).unwrap_or(0);
                let slice = std::slice::from_raw_parts(bstr, len);
                let text = String::from_utf16_lossy(slice);
                SysFreeString(bstr);
                text
            };
            Ok(ComValue::String(BStr::from(text)).to_runtime_value())
        }
        other => Err(format!(
            "unsupported SAFEARRAY element vartype {other} in multi-dimensional array"
        )),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn safe_array_element_to_runtime_value(
    psa: *const SAFEARRAY,
    index: i32,
    element_vt: u16,
) -> Result<oxvba_runtime::RuntimeValue, String> {
    match element_vt {
        VT_VARIANT => {
            let mut element: VARIANT = std::mem::zeroed();
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut VARIANT).cast());
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
            Ok(value)
        }
        VT_I1 => {
            let mut element = 0i8;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut i8).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            Ok(ComValue::I32(element as i32).to_runtime_value())
        }
        VT_I2 => {
            let mut element = 0i16;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut i16).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            Ok(ComValue::I32(element as i32).to_runtime_value())
        }
        VT_I4 => {
            let mut element = 0i32;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut i32).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            Ok(ComValue::I32(element).to_runtime_value())
        }
        VT_I8 => {
            let mut element = 0i64;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut i64).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            match i32::try_from(element) {
                Ok(narrowed) => Ok(ComValue::I32(narrowed).to_runtime_value()),
                Err(_) => Ok(ComValue::I64(element).to_runtime_value()),
            }
        }
        VT_INT => {
            let mut element = 0i32;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut i32).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            Ok(ComValue::I32(element).to_runtime_value())
        }
        VT_UI2 => {
            let mut element = 0u16;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut u16).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            Ok(ComValue::I32(element as i32).to_runtime_value())
        }
        VT_UI1 => {
            let mut element = 0u8;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut u8).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            Ok(ComValue::I32(element as i32).to_runtime_value())
        }
        VT_UI4 => {
            let mut element = 0u32;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut u32).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            match i32::try_from(element) {
                Ok(narrowed) => Ok(ComValue::I32(narrowed).to_runtime_value()),
                Err(_) => Ok(ComValue::I64(element as i64).to_runtime_value()),
            }
        }
        VT_UI8 => {
            let mut element = 0u64;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut u64).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            match i32::try_from(element) {
                Ok(narrowed) => Ok(ComValue::I32(narrowed).to_runtime_value()),
                Err(_) => match i64::try_from(element) {
                    Ok(narrowed) => Ok(ComValue::I64(narrowed).to_runtime_value()),
                    Err(_) => Err(format!(
                        "VT_UI8 SAFEARRAY element {element} exceeds i64 carrier range"
                    )),
                },
            }
        }
        VT_R4_VARENUM => {
            let mut element = 0f32;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut f32).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            Ok(ComValue::F64(F64Value::from_single_f64(element as f64)).to_runtime_value())
        }
        VT_R8_VARENUM => {
            let mut element = 0f64;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut f64).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            Ok(ComValue::F64(F64Value::from_f64(element)).to_runtime_value())
        }
        VT_CY_VARENUM => {
            let mut element: CY = std::mem::zeroed();
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut CY).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            Ok(
                ComValue::Currency(CurrencyValue::from_scaled_i64(element.int64))
                    .to_runtime_value(),
            )
        }
        VT_DATE_VARENUM => {
            let mut element = 0f64;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut f64).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            Ok(ComValue::F64(F64Value::from_date_f64(element)).to_runtime_value())
        }
        VT_DECIMAL => {
            let mut element: DECIMAL = std::mem::zeroed();
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut DECIMAL).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            Ok(ComValue::Decimal(decimal96_from_windows(element)).to_runtime_value())
        }
        VT_UINT => {
            let mut element = 0u32;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut u32).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            match i32::try_from(element) {
                Ok(narrowed) => Ok(ComValue::I32(narrowed).to_runtime_value()),
                Err(_) => Ok(ComValue::I64(element as i64).to_runtime_value()),
            }
        }
        VT_BOOL => {
            let mut element: VARIANT_BOOL = 0;
            let hr = SafeArrayGetElement(psa, &index, (&mut element as *mut VARIANT_BOOL).cast());
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            Ok(ComValue::Bool(element != 0).to_runtime_value())
        }
        VT_BSTR => {
            let mut element: windows_sys::core::BSTR = std::ptr::null_mut();
            let hr = SafeArrayGetElement(
                psa,
                &index,
                (&mut element as *mut windows_sys::core::BSTR).cast(),
            );
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at index {}",
                    hr as u32, index
                ));
            }
            let value = runtime_bstr_from_windows(element);
            if !element.is_null() {
                SysFreeString(element);
            }
            Ok(ComValue::String(value).to_runtime_value())
        }
        other => Err(format!(
            "unsupported SAFEARRAY element vartype {other}; supported element vartypes are VT_VARIANT, VT_I1, VT_I2, VT_I4, VT_I8, VT_INT, VT_UI1, VT_UI2, VT_UI4, VT_UI8, VT_R4, VT_R8, VT_CY, VT_DATE, VT_UINT, VT_BOOL, and VT_BSTR"
        )),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn safe_array_to_runtime_value<FQueryDispatch, FAddRefDispatch, FBindDispatch>(
    psa: *mut SAFEARRAY,
    query_dispatch_from_unknown: &mut FQueryDispatch,
    add_ref_dispatch: &mut FAddRefDispatch,
    bind_dispatch_result: &mut FBindDispatch,
    prog_id_hint: &str,
    op: &'static str,
) -> Result<oxvba_runtime::RuntimeValue, String>
where
    FQueryDispatch: FnMut(*mut c_void) -> Result<*mut c_void, String>,
    FAddRefDispatch: FnMut(*mut c_void),
    FBindDispatch:
        FnMut(*mut c_void, &str, &'static str) -> Result<oxvba_runtime::RuntimeValue, String>,
{
    if psa.is_null() {
        return Err("VT_ARRAY result carried null SAFEARRAY".to_string());
    }
    let dims = SafeArrayGetDim(psa.cast_const());
    if dims < 1 {
        return Err("VT_ARRAY result carried SAFEARRAY with zero dimensions".to_string());
    }
    let mut element_vt = 0u16;
    let hr = SafeArrayGetVartype(psa.cast_const(), &mut element_vt);
    if hr < 0 {
        return Err(format!(
            "SafeArrayGetVartype failed with HRESULT {:#010X}",
            hr as u32
        ));
    }

    // Collect per-dimension bounds.
    let mut bounds = Vec::with_capacity(dims as usize);
    let mut total_len: usize = 1;
    for dim in 1..=dims {
        let mut lower = 0i32;
        let hr = SafeArrayGetLBound(psa.cast_const(), dim, &mut lower);
        if hr < 0 {
            return Err(format!(
                "SafeArrayGetLBound(dim={dim}) failed with HRESULT {:#010X}",
                hr as u32
            ));
        }
        let mut upper = -1i32;
        let hr = SafeArrayGetUBound(psa.cast_const(), dim, &mut upper);
        if hr < 0 {
            return Err(format!(
                "SafeArrayGetUBound(dim={dim}) failed with HRESULT {:#010X}",
                hr as u32
            ));
        }
        let count = if upper < lower {
            0u32
        } else {
            u32::try_from(upper - lower + 1)
                .map_err(|_| "SAFEARRAY dimension bounds exceed supported u32 range".to_string())?
        };
        total_len = total_len.checked_mul(count as usize).ok_or_else(|| {
            "SAFEARRAY total element count exceeds supported usize range".to_string()
        })?;
        bounds.push(SafeArrayBound { lower, count });
    }

    // Helper closure: extract one element by indices for variant/dispatch/unknown/typed paths.
    let mut extract_element = |indices: &[i32]| -> Result<oxvba_runtime::RuntimeValue, String> {
        if element_vt == VT_VARIANT {
            let mut element: VARIANT = std::mem::zeroed();
            let hr = SafeArrayGetElement(
                psa.cast_const(),
                indices.as_ptr(),
                (&mut element as *mut VARIANT).cast(),
            );
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at indices {indices:?}",
                    hr as u32
                ));
            }
            let value = match variant_to_runtime_value(
                &element,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
                prog_id_hint,
                op,
            ) {
                Ok(value) => value,
                Err(detail) => {
                    let _ = VariantClear(&mut element);
                    return Err(detail);
                }
            };
            let _ = VariantClear(&mut element);
            return Ok(value);
        }
        if element_vt == VT_UNKNOWN {
            let mut unknown: *mut c_void = std::ptr::null_mut();
            let hr = SafeArrayGetElement(
                psa.cast_const(),
                indices.as_ptr(),
                (&mut unknown as *mut *mut c_void).cast(),
            );
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at indices {indices:?}",
                    hr as u32
                ));
            }
            let mut element: VARIANT = std::mem::zeroed();
            element.Anonymous.Anonymous.vt = VT_UNKNOWN;
            element.Anonymous.Anonymous.Anonymous.punkVal = unknown.cast();
            let value = match variant_to_runtime_value(
                &element,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
                prog_id_hint,
                op,
            ) {
                Ok(value) => value,
                Err(detail) => {
                    let _ = VariantClear(&mut element);
                    return Err(detail);
                }
            };
            let _ = VariantClear(&mut element);
            return Ok(value);
        }
        if element_vt == VT_DISPATCH {
            let mut dispatch: *mut c_void = std::ptr::null_mut();
            let hr = SafeArrayGetElement(
                psa.cast_const(),
                indices.as_ptr(),
                (&mut dispatch as *mut *mut c_void).cast(),
            );
            if hr < 0 {
                return Err(format!(
                    "SafeArrayGetElement failed with HRESULT {:#010X} at indices {indices:?}",
                    hr as u32
                ));
            }
            let mut element: VARIANT = std::mem::zeroed();
            element.Anonymous.Anonymous.vt = VT_DISPATCH;
            element.Anonymous.Anonymous.Anonymous.pdispVal = dispatch.cast();
            let value = match variant_to_runtime_value(
                &element,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
                prog_id_hint,
                op,
            ) {
                Ok(value) => value,
                Err(detail) => {
                    let _ = VariantClear(&mut element);
                    return Err(detail);
                }
            };
            let _ = VariantClear(&mut element);
            return Ok(value);
        }
        safe_array_element_typed_nd(psa.cast_const(), indices, element_vt)
    };

    let mut values = Vec::with_capacity(total_len);
    let mut indices: Vec<i32> = bounds.iter().map(|b| b.lower).collect();

    for _ in 0..total_len {
        values.push(extract_element(&indices)?);

        // Increment indices in column-major order (first dimension varies fastest).
        let mut carry = true;
        for (dim_idx, bound) in bounds.iter().enumerate() {
            if !carry {
                break;
            }
            indices[dim_idx] += 1;
            if indices[dim_idx] >= bound.lower + bound.count as i32 {
                indices[dim_idx] = bound.lower;
            } else {
                carry = false;
            }
        }
    }

    let array = if element_vt != VT_VARIANT_VALUE
        && SafeArray::supports_intrinsic_element_vartype(element_vt)
    {
        if dims == 1 {
            SafeArray::from_typed_values(element_vt, values)?
        } else {
            SafeArray::from_typed_values_nd(bounds, element_vt, values)?
        }
    } else if dims == 1 {
        SafeArray::from_values(values)
    } else {
        SafeArray::from_values_nd(bounds, values)
    };

    Ok(oxvba_runtime::RuntimeValue::ArrayIntent(array))
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
    FResolve: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
    FAddRef: FnMut(*mut core::ffi::c_void),
{
    let Some(values) = array.elements() else {
        (*variant).Anonymous.Anonymous.vt = VT_I4;
        (*variant).Anonymous.Anonymous.Anonymous.lVal =
            ComValue::ArrayIntent(array.clone()).to_legacy_dispatch_token()?;
        return Ok(());
    };

    let element_vt = array.element_vartype();
    if element_vt != VT_VARIANT_VALUE
        && SafeArray::supports_intrinsic_element_vartype(element_vt)
    {
        return set_typed_array_arg(variant, array, &values, resolve_object, add_ref_dispatch);
    }

    // Multi-dimensional path: use SafeArrayCreate with per-dimension bounds.
    if let Some(bounds) = array.bounds()
        && bounds.len() > 1
    {
        let dims = u32::try_from(bounds.len())
            .map_err(|_| "SAFEARRAY dimension count exceeds supported u32 range".to_string())?;
        // SAFEARRAYBOUND array in reverse order (SAFEARRAY expects rightmost dimension first).
        let sa_bounds: Vec<SAFEARRAYBOUND> = bounds
            .iter()
            .map(|b| SAFEARRAYBOUND {
                cElements: b.count,
                lLbound: b.lower,
            })
            .collect();
        let psa = SafeArrayCreate(VT_VARIANT, dims, sa_bounds.as_ptr());
        if psa.is_null() {
            return Err("SafeArrayCreate(VT_VARIANT) returned null".to_string());
        }
        // Iterate in column-major order matching the bounds.
        let mut indices: Vec<i32> = bounds.iter().map(|b| b.lower).collect();
        for runtime_value in values.iter() {
            let mut element: VARIANT = std::mem::zeroed();
            let value = ComValue::from_runtime_value(runtime_value);
            if let Err(detail) =
                set_variant_from_com_value(&mut element, &value, resolve_object, add_ref_dispatch)
            {
                let _ = VariantClear(&mut element);
                let _ = SafeArrayDestroy(psa.cast_const());
                return Err(detail);
            }
            let hr = SafeArrayPutElement(
                psa.cast_const(),
                indices.as_ptr(),
                (&element as *const VARIANT).cast(),
            );
            let _ = VariantClear(&mut element);
            if hr < 0 {
                let _ = SafeArrayDestroy(psa.cast_const());
                return Err(format!(
                    "SafeArrayPutElement failed with HRESULT {:#010X} at indices {indices:?}",
                    hr as u32
                ));
            }
            // Increment indices in column-major order.
            let mut carry = true;
            for (dim_idx, bound) in bounds.iter().enumerate() {
                if !carry {
                    break;
                }
                indices[dim_idx] += 1;
                if indices[dim_idx] >= bound.lower + bound.count as i32 {
                    indices[dim_idx] = bound.lower;
                } else {
                    carry = false;
                }
            }
        }
        (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_VARIANT;
        (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
        return Ok(());
    }

    // Single-dimensional path.
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
fn runtime_value_to_i64(value: &oxvba_runtime::RuntimeValue) -> Result<i64, String> {
    match value {
        oxvba_runtime::RuntimeValue::I32(value) => Ok(i64::from(*value)),
        oxvba_runtime::RuntimeValue::I64(value) => Ok(*value),
        other => Err(format!("expected integer-compatible SAFEARRAY element, got {other:?}")),
    }
}

#[cfg(target_os = "windows")]
fn runtime_value_to_f64(value: &oxvba_runtime::RuntimeValue) -> Result<f64, String> {
    match value {
        oxvba_runtime::RuntimeValue::F64(value) => Ok(value.as_f64()),
        other => Err(format!("expected floating-point SAFEARRAY element, got {other:?}")),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn put_typed_safe_array_element(
    psa: *mut SAFEARRAY,
    indices: &[i32],
    element_vt: u16,
    value: &oxvba_runtime::RuntimeValue,
    resolve_object: &mut impl FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
    add_ref_dispatch: &mut impl FnMut(*mut core::ffi::c_void),
) -> Result<(), String> {
    let hr = match element_vt {
        VT_I1_VALUE => {
            let scalar = i8::try_from(runtime_value_to_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_I1 SAFEARRAY element"))?;
            SafeArrayPutElement(psa.cast_const(), indices.as_ptr(), (&scalar as *const i8).cast())
        }
        VT_UI1_VALUE => {
            let scalar = u8::try_from(runtime_value_to_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_UI1 SAFEARRAY element"))?;
            SafeArrayPutElement(psa.cast_const(), indices.as_ptr(), (&scalar as *const u8).cast())
        }
        VT_I2_VALUE => {
            let scalar = i16::try_from(runtime_value_to_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_I2 SAFEARRAY element"))?;
            SafeArrayPutElement(psa.cast_const(), indices.as_ptr(), (&scalar as *const i16).cast())
        }
        VT_UI2_VALUE => {
            let scalar = u16::try_from(runtime_value_to_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_UI2 SAFEARRAY element"))?;
            SafeArrayPutElement(psa.cast_const(), indices.as_ptr(), (&scalar as *const u16).cast())
        }
        VT_I4_VALUE | VT_INT_VALUE => {
            let scalar = i32::try_from(runtime_value_to_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_I4 SAFEARRAY element"))?;
            SafeArrayPutElement(psa.cast_const(), indices.as_ptr(), (&scalar as *const i32).cast())
        }
        VT_UI4_VALUE | VT_UINT_VALUE => {
            let scalar = u32::try_from(runtime_value_to_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_UI4 SAFEARRAY element"))?;
            SafeArrayPutElement(psa.cast_const(), indices.as_ptr(), (&scalar as *const u32).cast())
        }
        VT_I8_VALUE => {
            let scalar = runtime_value_to_i64(value)?;
            SafeArrayPutElement(psa.cast_const(), indices.as_ptr(), (&scalar as *const i64).cast())
        }
        VT_UI8_VALUE => {
            let scalar = u64::try_from(runtime_value_to_i64(value)?)
                .map_err(|_| format!("value {value:?} does not fit VT_UI8 SAFEARRAY element"))?;
            SafeArrayPutElement(psa.cast_const(), indices.as_ptr(), (&scalar as *const u64).cast())
        }
        VT_R4_VALUE => {
            let scalar = runtime_value_to_f64(value)? as f32;
            SafeArrayPutElement(psa.cast_const(), indices.as_ptr(), (&scalar as *const f32).cast())
        }
        VT_R8_VALUE | VT_DATE_VALUE => {
            let scalar = runtime_value_to_f64(value)?;
            SafeArrayPutElement(psa.cast_const(), indices.as_ptr(), (&scalar as *const f64).cast())
        }
        VT_CY_VALUE => {
            let oxvba_runtime::RuntimeValue::Currency(currency) = value else {
                return Err(format!("expected Currency SAFEARRAY element, got {value:?}"));
            };
            let scalar = CY {
                int64: currency.scaled_i64(),
            };
            SafeArrayPutElement(psa.cast_const(), indices.as_ptr(), (&scalar as *const CY).cast())
        }
        VT_BOOL_VALUE => {
            let oxvba_runtime::RuntimeValue::Bool(boolean) = value else {
                return Err(format!("expected Bool SAFEARRAY element, got {value:?}"));
            };
            let scalar: VARIANT_BOOL = if *boolean { -1 } else { 0 };
            SafeArrayPutElement(
                psa.cast_const(),
                indices.as_ptr(),
                (&scalar as *const VARIANT_BOOL).cast(),
            )
        }
        VT_BSTR_VALUE => {
            let oxvba_runtime::RuntimeValue::String(text) = value else {
                return Err(format!("expected String SAFEARRAY element, got {value:?}"));
            };
            let bstr = alloc_bstr(text);
            let hr = SafeArrayPutElement(psa.cast_const(), indices.as_ptr(), bstr.cast());
            SysFreeString(bstr);
            hr
        }
        VT_DISPATCH_VALUE => {
            let oxvba_runtime::RuntimeValue::Object(object) = value else {
                return Err(format!("expected Object SAFEARRAY element, got {value:?}"));
            };
            let dispatch = resolve_object(object.clone())?;
            add_ref_dispatch(dispatch);
            SafeArrayPutElement(
                psa.cast_const(),
                indices.as_ptr(),
                (&dispatch as *const *mut core::ffi::c_void).cast(),
            )
        }
        VT_UNKNOWN_VALUE => {
            let oxvba_runtime::RuntimeValue::Object(object) = value else {
                return Err(format!("expected Object SAFEARRAY element, got {value:?}"));
            };
            let unknown = object.query_iunknown().raw_iunknown().cast::<core::ffi::c_void>();
            SafeArrayPutElement(
                psa.cast_const(),
                indices.as_ptr(),
                (&unknown as *const *mut core::ffi::c_void).cast(),
            )
        }
        VT_DECIMAL_VALUE => {
            let oxvba_runtime::RuntimeValue::Decimal(decimal) = value else {
                return Err(format!("expected Decimal SAFEARRAY element, got {value:?}"));
            };
            let scalar = decimal96_to_windows(*decimal);
            SafeArrayPutElement(
                psa.cast_const(),
                indices.as_ptr(),
                (&scalar as *const DECIMAL).cast(),
            )
        }
        other => {
            return Err(format!(
                "unsupported intrinsic SAFEARRAY element vartype 0x{other:04X}"
            ));
        }
    };
    if hr < 0 {
        return Err(format!(
            "SafeArrayPutElement failed with HRESULT {:#010X} at indices {indices:?}",
            hr as u32
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_typed_array_arg(
    variant: *mut VARIANT,
    array: &SafeArray,
    values: &[oxvba_runtime::RuntimeValue],
    resolve_object: &mut impl FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
    add_ref_dispatch: &mut impl FnMut(*mut core::ffi::c_void),
) -> Result<(), String> {
    let element_vt = array.element_vartype();
    let bounds = array.bounds().unwrap_or_else(|| {
        vec![SafeArrayBound {
            lower: 0,
            count: u32::try_from(array.len()).unwrap_or(u32::MAX),
        }]
    });
    let dims = u32::try_from(bounds.len())
        .map_err(|_| "SAFEARRAY dimension count exceeds supported u32 range".to_string())?;
    let sa_bounds: Vec<SAFEARRAYBOUND> = bounds
        .iter()
        .map(|b| SAFEARRAYBOUND {
            cElements: b.count,
            lLbound: b.lower,
        })
        .collect();
    let psa = if bounds.len() == 1 && bounds[0].lower == 0 {
        SafeArrayCreateVector(element_vt, 0, bounds[0].count)
    } else {
        SafeArrayCreate(element_vt, dims, sa_bounds.as_ptr())
    };
    if psa.is_null() {
        return Err(format!(
            "SafeArrayCreate(0x{element_vt:04X}) returned null"
        ));
    }

    let mut indices: Vec<i32> = bounds.iter().map(|b| b.lower).collect();
    for value in values {
        if let Err(err) = put_typed_safe_array_element(
            psa,
            &indices,
            element_vt,
            value,
            resolve_object,
            add_ref_dispatch,
        ) {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(err);
        }
        let mut carry = true;
        for (dim_idx, bound) in bounds.iter().enumerate() {
            if !carry {
                break;
            }
            indices[dim_idx] += 1;
            if indices[dim_idx] >= bound.lower + bound.count as i32 {
                indices[dim_idx] = bound.lower;
            } else {
                carry = false;
            }
        }
    }

    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | element_vt;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// Convert a Windows `VARIANT` carrying the supported scalar/string/one-dimensional
/// SAFEARRAY subset into the shared semantic `ComValue` carrier.
///
/// # Safety
/// The caller must provide a valid initialized `VARIANT` reference whose pointed-to storage
/// remains alive for the duration of this call. If the variant contains nested COM-owned payloads,
/// they must satisfy the ownership rules required by the Windows `Variant`/`SafeArray` APIs.
pub unsafe fn variant_to_com_value(variant: &VARIANT) -> Result<ComValue, String> {
    let vt = variant.Anonymous.Anonymous.vt;
    if vt & VT_BYREF != 0 {
        let base_vt = vt & !VT_BYREF;
        // VT_BYREF | VT_ARRAY: dereference the pointer-to-pointer to get the SAFEARRAY*.
        if base_vt & VT_ARRAY != 0 {
            let pparray = variant.Anonymous.Anonymous.Anonymous.pparray;
            if pparray.is_null() {
                return Err("VT_BYREF|VT_ARRAY carried null pparray pointer".to_string());
            }
            let parray = *pparray;
            return safe_array_to_com_value(parray);
        }
        // VT_BYREF | VT_VARIANT: dereference to the pointed-to VARIANT and recurse.
        if base_vt == VT_VARIANT {
            let pvar = variant.Anonymous.Anonymous.Anonymous.pvarVal;
            if pvar.is_null() {
                return Err("VT_BYREF|VT_VARIANT carried null pvarVal pointer".to_string());
            }
            return variant_to_com_value(&*pvar);
        }
        // Scalar BYREF types: dereference the pointer to recover the underlying value.
        let value = match base_vt {
            VT_I2 => ComValue::I32(*variant.Anonymous.Anonymous.Anonymous.piVal as i32),
            VT_I4 => ComValue::I32(*variant.Anonymous.Anonymous.Anonymous.plVal),
            VT_R4_VARENUM => ComValue::F64(F64Value::from_single_f64(
                *variant.Anonymous.Anonymous.Anonymous.pfltVal as f64,
            )),
            VT_R8_VARENUM => ComValue::F64(F64Value::from_f64(
                *variant.Anonymous.Anonymous.Anonymous.pdblVal,
            )),
            VT_CY_VARENUM => ComValue::Currency(CurrencyValue::from_scaled_i64(
                (*variant.Anonymous.Anonymous.Anonymous.pcyVal).int64,
            )),
            VT_BOOL => {
                let val: VARIANT_BOOL = *variant.Anonymous.Anonymous.Anonymous.pboolVal;
                ComValue::Bool(val != 0)
            }
            VT_BSTR => {
                let bstr = *variant.Anonymous.Anonymous.Anonymous.pbstrVal;
                ComValue::String(runtime_bstr_from_windows(bstr))
            }
            VT_DISPATCH => {
                let dispatch = *variant.Anonymous.Anonymous.Anonymous.ppdispVal;
                return Err(format!(
                    "VT_BYREF|VT_DISPATCH dereferences to raw dispatch {dispatch:?}; \
                     use variant_to_runtime_value for object binding"
                ));
            }
            _ => {
                return Err(format!(
                    "unsupported VARIANT BYREF scalar type vt={vt} (base_vt={base_vt})"
                ));
            }
        };
        return Ok(value);
    }
    if vt & VT_ARRAY != 0 {
        let parray = variant.Anonymous.Anonymous.Anonymous.parray;
        return safe_array_to_com_value(parray);
    }
    let value = match vt {
        VT_EMPTY => ComValue::Empty,
        VT_I1 => ComValue::I32(variant.Anonymous.Anonymous.Anonymous.cVal as i32),
        VT_I2 => ComValue::I32(variant.Anonymous.Anonymous.Anonymous.iVal as i32),
        VT_I4 => ComValue::I32(variant.Anonymous.Anonymous.Anonymous.lVal),
        VT_I8 => {
            let value = variant.Anonymous.Anonymous.Anonymous.llVal;
            match i32::try_from(value) {
                Ok(narrowed) => ComValue::I32(narrowed),
                Err(_) => ComValue::I64(value),
            }
        }
        VT_INT => ComValue::I32(variant.Anonymous.Anonymous.Anonymous.intVal),
        VT_UI1 => ComValue::I32(variant.Anonymous.Anonymous.Anonymous.bVal as i32),
        VT_UI2 => ComValue::I32(variant.Anonymous.Anonymous.Anonymous.uiVal as i32),
        VT_UI4 => {
            let value = variant.Anonymous.Anonymous.Anonymous.ulVal;
            match i32::try_from(value) {
                Ok(narrowed) => ComValue::I32(narrowed),
                Err(_) => ComValue::I64(value as i64),
            }
        }
        VT_UI8 => {
            let value = variant.Anonymous.Anonymous.Anonymous.ullVal;
            match i32::try_from(value) {
                Ok(narrowed) => ComValue::I32(narrowed),
                Err(_) => match i64::try_from(value) {
                    Ok(narrowed) => ComValue::I64(narrowed),
                    Err(_) => {
                        return Err(format!("VT_UI8 value {value} exceeds i64 carrier range"));
                    }
                },
            }
        }
        VT_R4_VARENUM => ComValue::F64(F64Value::from_single_f64(
            variant.Anonymous.Anonymous.Anonymous.fltVal as f64,
        )),
        VT_R8_VARENUM => ComValue::F64(F64Value::from_f64(
            variant.Anonymous.Anonymous.Anonymous.dblVal,
        )),
        VT_CY_VARENUM => ComValue::Currency(CurrencyValue::from_scaled_i64(
            variant.Anonymous.Anonymous.Anonymous.cyVal.int64,
        )),
        VT_DATE_VARENUM => ComValue::F64(F64Value::from_date_f64(
            variant.Anonymous.Anonymous.Anonymous.dblVal,
        )),
        VT_DECIMAL => ComValue::Decimal(decimal96_from_windows(variant.Anonymous.decVal)),
        VT_UINT => {
            let value = variant.Anonymous.Anonymous.Anonymous.uintVal;
            match i32::try_from(value) {
                Ok(narrowed) => ComValue::I32(narrowed),
                Err(_) => ComValue::I64(value as i64),
            }
        }
        VT_BOOL => {
            let value: VARIANT_BOOL = variant.Anonymous.Anonymous.Anonymous.boolVal;
            ComValue::Bool(value != 0)
        }
        VT_BSTR => {
            let bstr = variant.Anonymous.Anonymous.Anonymous.bstrVal;
            ComValue::String(runtime_bstr_from_windows(bstr))
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
#[allow(unsafe_op_in_unsafe_fn, clippy::missing_safety_doc)]
pub unsafe fn variant_to_runtime_value<FQueryDispatch, FAddRefDispatch, FBindDispatch>(
    variant: &VARIANT,
    query_dispatch_from_unknown: &mut FQueryDispatch,
    add_ref_dispatch: &mut FAddRefDispatch,
    bind_dispatch_result: &mut FBindDispatch,
    prog_id_hint: &str,
    op: &'static str,
) -> Result<oxvba_runtime::RuntimeValue, String>
where
    FQueryDispatch: FnMut(*mut c_void) -> Result<*mut c_void, String>,
    FAddRefDispatch: FnMut(*mut c_void),
    FBindDispatch:
        FnMut(*mut c_void, &str, &'static str) -> Result<oxvba_runtime::RuntimeValue, String>,
{
    let vt = variant.Anonymous.Anonymous.vt;
    if vt & VT_BYREF != 0 {
        let base_vt = vt & !VT_BYREF;
        // VT_BYREF | VT_ARRAY: dereference pparray then delegate to the full runtime path.
        if base_vt & VT_ARRAY != 0 {
            let pparray = variant.Anonymous.Anonymous.Anonymous.pparray;
            if pparray.is_null() {
                return Err("VT_BYREF|VT_ARRAY carried null pparray pointer".to_string());
            }
            let parray = *pparray;
            return safe_array_to_runtime_value(
                parray,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
                prog_id_hint,
                op,
            );
        }
        // VT_BYREF | VT_VARIANT: dereference to the inner VARIANT and recurse.
        if base_vt == VT_VARIANT {
            let pvar = variant.Anonymous.Anonymous.Anonymous.pvarVal;
            if pvar.is_null() {
                return Err("VT_BYREF|VT_VARIANT carried null pvarVal pointer".to_string());
            }
            return variant_to_runtime_value(
                &*pvar,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
                prog_id_hint,
                op,
            );
        }
        // VT_BYREF | VT_DISPATCH: dereference the double-pointer and bind.
        if base_vt == VT_DISPATCH {
            let ppdispatch = variant.Anonymous.Anonymous.Anonymous.ppdispVal;
            if ppdispatch.is_null() {
                return Err("VT_BYREF|VT_DISPATCH carried null ppdispVal pointer".to_string());
            }
            let dispatch = *ppdispatch;
            if !dispatch.is_null() {
                add_ref_dispatch(dispatch.cast());
            }
            return bind_dispatch_result(dispatch.cast(), prog_id_hint, op);
        }
        // Other scalar BYREF types: delegate to variant_to_com_value which handles them.
        return Ok(variant_to_com_value(variant)?.to_runtime_value());
    }
    if vt & VT_ARRAY != 0 {
        let parray = variant.Anonymous.Anonymous.Anonymous.parray;
        return safe_array_to_runtime_value(
            parray,
            query_dispatch_from_unknown,
            add_ref_dispatch,
            bind_dispatch_result,
            prog_id_hint,
            op,
        );
    }
    if vt == VT_DISPATCH {
        let dispatch = variant.Anonymous.Anonymous.Anonymous.pdispVal;
        if !dispatch.is_null() {
            add_ref_dispatch(dispatch.cast());
        }
        return bind_dispatch_result(dispatch.cast(), prog_id_hint, op);
    }
    if vt == VT_UNKNOWN {
        let unknown = variant.Anonymous.Anonymous.Anonymous.punkVal;
        return unknown_to_runtime_value(
            unknown.cast(),
            query_dispatch_from_unknown,
            add_ref_dispatch,
            bind_dispatch_result,
            prog_id_hint,
            op,
        );
    }
    Ok(variant_to_com_value(variant)?.to_runtime_value())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn, clippy::missing_safety_doc)]
pub unsafe fn take_variant_result_runtime_value<FQueryDispatch, FAddRefDispatch, FBindDispatch>(
    result: &mut VARIANT,
    query_dispatch_from_unknown: &mut FQueryDispatch,
    add_ref_dispatch: &mut FAddRefDispatch,
    bind_dispatch_result: &mut FBindDispatch,
    prog_id_hint: &str,
    op: &'static str,
) -> Result<oxvba_runtime::RuntimeValue, String>
where
    FQueryDispatch: FnMut(*mut c_void) -> Result<*mut c_void, String>,
    FAddRefDispatch: FnMut(*mut c_void),
    FBindDispatch:
        FnMut(*mut c_void, &str, &'static str) -> Result<oxvba_runtime::RuntimeValue, String>,
{
    let value = variant_to_runtime_value(
        result,
        query_dispatch_from_unknown,
        add_ref_dispatch,
        bind_dispatch_result,
        prog_id_hint,
        op,
    )?;
    let _ = VariantClear(result);
    Ok(value)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// Populate a Windows `VARIANT` from the shared semantic `ComValue` carrier for the currently
/// supported scalar/string/one-dimensional SAFEARRAY subset and `VT_DISPATCH`
/// object-handle lane.
///
/// # Safety
/// The caller must provide a valid writable `VARIANT` pointer. `resolve_object` must return a live
/// `IDispatch` pointer for any provided `ObjectRef`, and `add_ref_dispatch` must apply the
/// matching COM reference-count increment to that pointer before the variant assumes ownership.
pub unsafe fn set_variant_from_com_value<FResolve, FAddRef>(
    variant: *mut VARIANT,
    value: &ComValue,
    resolve_object: &mut FResolve,
    add_ref_dispatch: &mut FAddRef,
) -> Result<(), String>
where
    FResolve: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
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
        ComValue::I64(value) => {
            (*variant).Anonymous.Anonymous.vt = VT_I8;
            (*variant).Anonymous.Anonymous.Anonymous.llVal = *value;
        }
        ComValue::F64(value) => match value.subtype() {
            F64Subtype::Single => {
                (*variant).Anonymous.Anonymous.vt = VT_R4_VARENUM;
                (*variant).Anonymous.Anonymous.Anonymous.fltVal = value.as_f64() as f32;
            }
            F64Subtype::Double => {
                (*variant).Anonymous.Anonymous.vt = VT_R8_VARENUM;
                (*variant).Anonymous.Anonymous.Anonymous.dblVal = value.as_f64();
            }
            F64Subtype::Date => {
                (*variant).Anonymous.Anonymous.vt = VT_DATE_VARENUM;
                (*variant).Anonymous.Anonymous.Anonymous.dblVal = value.as_f64();
            }
        },
        ComValue::Decimal(value) => {
            (*variant).Anonymous.decVal = decimal96_to_windows(*value);
            (*variant).Anonymous.decVal.wReserved = VT_DECIMAL;
        }
        ComValue::Currency(value) => {
            (*variant).Anonymous.Anonymous.vt = VT_CY_VARENUM;
            (*variant).Anonymous.Anonymous.Anonymous.cyVal.int64 = value.scaled_i64();
        }
        ComValue::String(value) => {
            (*variant).Anonymous.Anonymous.vt = VT_BSTR;
            (*variant).Anonymous.Anonymous.Anonymous.bstrVal = alloc_bstr(value);
        }
        ComValue::ArrayIntent(array) => {
            set_variant_array_arg(variant, array, resolve_object, add_ref_dispatch)?;
        }
        ComValue::Object(handle) => {
            let dispatch = resolve_object(handle.clone())?;
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

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// Consume an Invoke-owned result `VARIANT`, clear it, and classify the supported result shape as
/// either a semantic `ComValue` or a dispatch-capable object pointer for adapter-owned binding.
///
/// # Safety
/// The caller must provide a valid writable `VARIANT` pointer containing an Invoke-owned result
/// value. `query_dispatch_from_unknown` must apply the correct COM query semantics for `VT_UNKNOWN`
/// payloads, and `add_ref_dispatch` must retain a stable dispatch reference before the result
/// variant is cleared when a `VT_DISPATCH` payload is returned.
pub unsafe fn take_variant_result_value<FQueryDispatch, FAddRefDispatch>(
    result: &mut VARIANT,
    query_dispatch_from_unknown: &mut FQueryDispatch,
    add_ref_dispatch: &mut FAddRefDispatch,
) -> Result<VariantResultValue, String>
where
    FQueryDispatch: FnMut(*mut c_void) -> Result<*mut c_void, String>,
    FAddRefDispatch: FnMut(*mut c_void),
{
    let vt = result.Anonymous.Anonymous.vt;
    if vt == VT_DISPATCH {
        let dispatch = result.Anonymous.Anonymous.Anonymous.pdispVal;
        if !dispatch.is_null() {
            add_ref_dispatch(dispatch);
        }
        let _ = VariantClear(result);
        return Ok(VariantResultValue::Dispatch(dispatch));
    }
    if vt == VT_UNKNOWN {
        let unknown = result.Anonymous.Anonymous.Anonymous.punkVal;
        let dispatch = query_dispatch_from_unknown(unknown)?;
        let _ = VariantClear(result);
        return Ok(VariantResultValue::Dispatch(dispatch));
    }
    let value = variant_to_com_value(result)?;
    let _ = VariantClear(result);
    Ok(VariantResultValue::Value(value))
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::{
        VT_CY_VARENUM, VT_DATE_VARENUM, VT_R4_VARENUM, VT_R8_VARENUM, VariantResultValue,
        decimal96_to_windows, set_variant_from_com_value, take_variant_result_value,
        variant_to_com_value, variant_to_runtime_value,
    };
    use crate::ComValue;
    use crate::windows_test_dispatch::create_oxvba_test_enum_unknown;
    use oxvba_runtime::{
        CurrencyValue, Decimal96, F64Value, RuntimeValue, bstr::BStr,
        safe_array::{
            SafeArray, VT_BSTR_VALUE, VT_CY_VALUE, VT_DATE_VALUE, VT_DECIMAL_VALUE,
            VT_I2_VALUE, VT_I8_VALUE, VT_R4_VALUE, VT_R8_VALUE, VT_UI8_VALUE,
        },
    };
    use windows_sys::Win32::System::Ole::{SafeArrayCreateVector, SafeArrayPutElement};
    use windows_sys::Win32::System::Variant::{
        VARIANT, VT_ARRAY, VT_BSTR, VT_BYREF, VT_DECIMAL, VT_DISPATCH, VT_I2, VT_I4, VT_I8, VT_UI8,
        VT_UNKNOWN, VT_VARIANT, VariantClear,
    };
    use windows_sys::Win32::{
        Foundation::{DECIMAL, SysAllocString, SysFreeString},
        System::Com::CY,
    };

    #[test]
    fn string_variant_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let value = ComValue::String(BStr::from("Hello"));
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
    fn string_variant_preserves_embedded_nuls_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let value = ComValue::String(BStr::from("A\0B"));
        let mut resolve_object =
            |_handle| Err("object dispatch resolution not expected".to_string());
        let mut add_ref = |_dispatch| {};
        unsafe {
            set_variant_from_com_value(&mut variant, &value, &mut resolve_object, &mut add_ref)
                .expect("set string variant with embedded nul");
            let bstr = variant.Anonymous.Anonymous.Anonymous.bstrVal;
            assert_eq!(windows_sys::Win32::Foundation::SysStringLen(bstr), 3);
            assert_eq!(
                variant_to_com_value(&variant).expect("read string variant with embedded nul"),
                value
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn scalar_single_variant_reads_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            variant.Anonymous.Anonymous.vt = VT_R4_VARENUM;
            variant.Anonymous.Anonymous.Anonymous.fltVal = 12.5;
            assert_eq!(
                variant_to_com_value(&variant).expect("read single variant"),
                ComValue::F64(F64Value::from_single_f64(12.5))
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn scalar_date_variant_reads_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            variant.Anonymous.Anonymous.vt = VT_DATE_VARENUM;
            variant.Anonymous.Anonymous.Anonymous.dblVal = 45200.25;
            assert_eq!(
                variant_to_com_value(&variant).expect("read date variant"),
                ComValue::F64(F64Value::from_date_f64(45200.25))
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn scalar_currency_variant_reads_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            variant.Anonymous.Anonymous.vt = VT_CY_VARENUM;
            variant.Anonymous.Anonymous.Anonymous.cyVal.int64 = 125_000;
            assert_eq!(
                variant_to_com_value(&variant).expect("read currency variant"),
                ComValue::Currency(CurrencyValue::from_scaled_i64(125_000))
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn scalar_decimal_variant_reads_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            variant.Anonymous.decVal =
                decimal96_to_windows(Decimal96::from_parts(123_450, 0, 0, 3, true));
            variant.Anonymous.decVal.wReserved = VT_DECIMAL;
            assert_eq!(
                variant_to_com_value(&variant).expect("read decimal variant"),
                ComValue::Decimal(Decimal96::from_parts(123_450, 0, 0, 3, true))
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn scalar_i64_variant_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let value = ComValue::I64(5_000_000_000);
        let mut resolve_object =
            |_handle| Err("object dispatch resolution not expected".to_string());
        let mut add_ref = |_dispatch| {};
        unsafe {
            set_variant_from_com_value(&mut variant, &value, &mut resolve_object, &mut add_ref)
                .expect("set i64 variant");
            assert_eq!(variant.Anonymous.Anonymous.vt, VT_I8);
            assert_eq!(
                variant_to_com_value(&variant).expect("read i64 variant"),
                value
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn scalar_ui8_variant_reads_wide_value_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            variant.Anonymous.Anonymous.vt = VT_UI8;
            variant.Anonymous.Anonymous.Anonymous.ullVal = 5_000_000_000;
            assert_eq!(
                variant_to_com_value(&variant).expect("read ui8 variant"),
                ComValue::I64(5_000_000_000)
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn scalar_byref_i32_variant_dereferences_to_underlying_value() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let mut backing = 321i32;
        unsafe {
            variant.Anonymous.Anonymous.vt = VT_BYREF | VT_I4;
            variant.Anonymous.Anonymous.Anonymous.plVal = &mut backing;
            let value = variant_to_com_value(&variant).expect("VT_BYREF|VT_I4 should dereference");
            assert_eq!(value, ComValue::I32(321));
        }
    }

    #[test]
    fn typed_byref_i32_array_variant_dereferences_pparray_and_rejects_null_safearray() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let mut psa = std::ptr::null_mut();
        unsafe {
            variant.Anonymous.Anonymous.vt = VT_BYREF | VT_ARRAY | VT_I4;
            variant.Anonymous.Anonymous.Anonymous.pparray = &mut psa;
            // Now dereferences pparray successfully but finds a null SAFEARRAY inside.
            let err = variant_to_com_value(&variant)
                .expect_err("VT_BYREF|VT_ARRAY with null SAFEARRAY should error");
            assert_eq!(err, "VT_ARRAY result carried null SAFEARRAY");
        }
    }

    #[test]
    fn scalar_double_variant_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let value = ComValue::F64(F64Value::from_f64(3.5));
        let mut resolve_object =
            |_handle| Err("object dispatch resolution not expected".to_string());
        let mut add_ref = |_dispatch| {};
        unsafe {
            set_variant_from_com_value(&mut variant, &value, &mut resolve_object, &mut add_ref)
                .expect("set double variant");
            assert_eq!(variant.Anonymous.Anonymous.vt, VT_R8_VARENUM);
            assert_eq!(
                variant_to_com_value(&variant).expect("read double variant"),
                value
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn scalar_single_variant_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let value = ComValue::F64(F64Value::from_single_f64(3.5));
        let mut resolve_object =
            |_handle| Err("object dispatch resolution not expected".to_string());
        let mut add_ref = |_dispatch| {};
        unsafe {
            set_variant_from_com_value(&mut variant, &value, &mut resolve_object, &mut add_ref)
                .expect("set single variant");
            assert_eq!(variant.Anonymous.Anonymous.vt, VT_R4_VARENUM);
            assert_eq!(
                variant_to_com_value(&variant).expect("read single variant"),
                value
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn scalar_date_variant_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let value = ComValue::F64(F64Value::from_date_f64(45200.25));
        let mut resolve_object =
            |_handle| Err("object dispatch resolution not expected".to_string());
        let mut add_ref = |_dispatch| {};
        unsafe {
            set_variant_from_com_value(&mut variant, &value, &mut resolve_object, &mut add_ref)
                .expect("set date variant");
            assert_eq!(variant.Anonymous.Anonymous.vt, VT_DATE_VARENUM);
            assert_eq!(
                variant_to_com_value(&variant).expect("read date variant"),
                value
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn scalar_currency_variant_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let value = ComValue::Currency(CurrencyValue::from_scaled_i64(-42_500));
        let mut resolve_object =
            |_handle| Err("object dispatch resolution not expected".to_string());
        let mut add_ref = |_dispatch| {};
        unsafe {
            set_variant_from_com_value(&mut variant, &value, &mut resolve_object, &mut add_ref)
                .expect("set currency variant");
            assert_eq!(
                variant_to_com_value(&variant).expect("read currency variant"),
                value
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn scalar_decimal_variant_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let value = ComValue::Decimal(Decimal96::from_parts(123_450, 0, 0, 3, false));
        let mut resolve_object =
            |_handle| Err("object dispatch resolution not expected".to_string());
        let mut add_ref = |_dispatch| {};
        unsafe {
            set_variant_from_com_value(&mut variant, &value, &mut resolve_object, &mut add_ref)
                .expect("set decimal variant");
            assert_eq!(
                variant_to_com_value(&variant).expect("read decimal variant"),
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
            RuntimeValue::String(BStr::from("Hello")),
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

    #[test]
    fn typed_i2_safe_array_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            let psa = SafeArrayCreateVector(VT_I2, 0, 3);
            assert!(
                !psa.is_null(),
                "SafeArrayCreateVector(VT_I2) should succeed"
            );
            for (index, value) in [12i16, -4i16, 321i16].into_iter().enumerate() {
                let index = i32::try_from(index).expect("index should fit in i32");
                let hr =
                    SafeArrayPutElement(psa.cast_const(), &index, (&value as *const i16).cast());
                assert!(
                    hr >= 0,
                    "SafeArrayPutElement(VT_I2) should succeed: {hr:#010X}"
                );
            }
            variant.Anonymous.Anonymous.vt = VT_ARRAY | VT_I2;
            variant.Anonymous.Anonymous.Anonymous.parray = psa;
            assert_eq!(
                variant_to_com_value(&variant).expect("read VT_I2 SAFEARRAY"),
                ComValue::ArrayIntent(
                    SafeArray::from_typed_values(
                        VT_I2_VALUE,
                        vec![
                            RuntimeValue::I32(12),
                            RuntimeValue::I32(-4),
                            RuntimeValue::I32(321),
                        ],
                    )
                    .expect("typed i2 array"),
                )
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn typed_i8_safe_array_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            let psa = SafeArrayCreateVector(VT_I8, 0, 3);
            assert!(
                !psa.is_null(),
                "SafeArrayCreateVector(VT_I8) should succeed"
            );
            for (index, value) in [12i64, 5_000_000_000i64, -4i64].into_iter().enumerate() {
                let index = i32::try_from(index).expect("index should fit in i32");
                let hr =
                    SafeArrayPutElement(psa.cast_const(), &index, (&value as *const i64).cast());
                assert!(
                    hr >= 0,
                    "SafeArrayPutElement(VT_I8) should succeed: {hr:#010X}"
                );
            }
            variant.Anonymous.Anonymous.vt = VT_ARRAY | VT_I8;
            variant.Anonymous.Anonymous.Anonymous.parray = psa;
            assert_eq!(
                variant_to_com_value(&variant).expect("read VT_I8 SAFEARRAY"),
                ComValue::ArrayIntent(
                    SafeArray::from_typed_values(
                        VT_I8_VALUE,
                        vec![
                            RuntimeValue::I32(12),
                            RuntimeValue::I64(5_000_000_000),
                            RuntimeValue::I32(-4),
                        ],
                    )
                    .expect("typed i8 array"),
                )
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn typed_ui8_safe_array_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            let psa = SafeArrayCreateVector(VT_UI8, 0, 3);
            assert!(
                !psa.is_null(),
                "SafeArrayCreateVector(VT_UI8) should succeed"
            );
            for (index, value) in [12u64, 5_000_000_000u64, 321u64].into_iter().enumerate() {
                let index = i32::try_from(index).expect("index should fit in i32");
                let hr =
                    SafeArrayPutElement(psa.cast_const(), &index, (&value as *const u64).cast());
                assert!(
                    hr >= 0,
                    "SafeArrayPutElement(VT_UI8) should succeed: {hr:#010X}"
                );
            }
            variant.Anonymous.Anonymous.vt = VT_ARRAY | VT_UI8;
            variant.Anonymous.Anonymous.Anonymous.parray = psa;
            assert_eq!(
                variant_to_com_value(&variant).expect("read VT_UI8 SAFEARRAY"),
                ComValue::ArrayIntent(
                    SafeArray::from_typed_values(
                        VT_UI8_VALUE,
                        vec![
                            RuntimeValue::I32(12),
                            RuntimeValue::I64(5_000_000_000),
                            RuntimeValue::I32(321),
                        ],
                    )
                    .expect("typed ui8 array"),
                )
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn typed_r4_safe_array_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            let psa = SafeArrayCreateVector(VT_R4_VARENUM, 0, 3);
            assert!(
                !psa.is_null(),
                "SafeArrayCreateVector(VT_R4_VARENUM) should succeed"
            );
            for (index, value) in [12.5f32, -4.25f32, 321.0f32].into_iter().enumerate() {
                let index = i32::try_from(index).expect("index should fit in i32");
                let hr =
                    SafeArrayPutElement(psa.cast_const(), &index, (&value as *const f32).cast());
                assert!(
                    hr >= 0,
                    "SafeArrayPutElement(VT_R4_VARENUM) should succeed: {hr:#010X}"
                );
            }
            variant.Anonymous.Anonymous.vt = VT_ARRAY | VT_R4_VARENUM;
            variant.Anonymous.Anonymous.Anonymous.parray = psa;
            assert_eq!(
                variant_to_com_value(&variant).expect("read VT_R4_VARENUM SAFEARRAY"),
                ComValue::ArrayIntent(
                    SafeArray::from_typed_values(
                        VT_R4_VALUE,
                        vec![
                            RuntimeValue::F64(F64Value::from_single_f64(12.5)),
                            RuntimeValue::F64(F64Value::from_single_f64(-4.25)),
                            RuntimeValue::F64(F64Value::from_single_f64(321.0)),
                        ],
                    )
                    .expect("typed r4 array"),
                )
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn typed_currency_safe_array_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            let psa = SafeArrayCreateVector(VT_CY_VARENUM, 0, 3);
            assert!(
                !psa.is_null(),
                "SafeArrayCreateVector(VT_CY_VARENUM) should succeed"
            );
            for (index, value) in [125_000i64, -42_500i64, 3_210_000i64]
                .into_iter()
                .enumerate()
            {
                let index = i32::try_from(index).expect("index should fit in i32");
                let element = CY { int64: value };
                let hr =
                    SafeArrayPutElement(psa.cast_const(), &index, (&element as *const CY).cast());
                assert!(
                    hr >= 0,
                    "SafeArrayPutElement(VT_CY_VARENUM) should succeed: {hr:#010X}"
                );
            }
            variant.Anonymous.Anonymous.vt = VT_ARRAY | VT_CY_VARENUM;
            variant.Anonymous.Anonymous.Anonymous.parray = psa;
            assert_eq!(
                variant_to_com_value(&variant).expect("read VT_CY_VARENUM SAFEARRAY"),
                ComValue::ArrayIntent(
                    SafeArray::from_typed_values(
                        VT_CY_VALUE,
                        vec![
                            RuntimeValue::Currency(CurrencyValue::from_scaled_i64(125_000)),
                            RuntimeValue::Currency(CurrencyValue::from_scaled_i64(-42_500)),
                            RuntimeValue::Currency(CurrencyValue::from_scaled_i64(3_210_000)),
                        ],
                    )
                    .expect("typed currency array"),
                )
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn typed_decimal_safe_array_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            let psa = SafeArrayCreateVector(VT_DECIMAL, 0, 3);
            assert!(
                !psa.is_null(),
                "SafeArrayCreateVector(VT_DECIMAL) should succeed"
            );
            for (index, value) in [
                Decimal96::from_parts(123_450, 0, 0, 3, false),
                Decimal96::from_parts(42_500, 0, 0, 4, true),
                Decimal96::from_parts(3_210_000, 0, 0, 4, false),
            ]
            .into_iter()
            .enumerate()
            {
                let index = i32::try_from(index).expect("index should fit in i32");
                let element = decimal96_to_windows(value);
                let hr = SafeArrayPutElement(
                    psa.cast_const(),
                    &index,
                    (&element as *const DECIMAL).cast(),
                );
                assert!(
                    hr >= 0,
                    "SafeArrayPutElement(VT_DECIMAL) should succeed: {hr:#010X}"
                );
            }
            variant.Anonymous.Anonymous.vt = VT_ARRAY | VT_DECIMAL;
            variant.Anonymous.Anonymous.Anonymous.parray = psa;
            assert_eq!(
                variant_to_com_value(&variant).expect("read VT_DECIMAL SAFEARRAY"),
                ComValue::ArrayIntent(
                    SafeArray::from_typed_values(
                        VT_DECIMAL_VALUE,
                        vec![
                            RuntimeValue::Decimal(Decimal96::from_parts(123_450, 0, 0, 3, false)),
                            RuntimeValue::Decimal(Decimal96::from_parts(42_500, 0, 0, 4, true)),
                            RuntimeValue::Decimal(Decimal96::from_parts(3_210_000, 0, 0, 4, false)),
                        ],
                    )
                    .expect("typed decimal array"),
                )
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn typed_date_safe_array_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            let psa = SafeArrayCreateVector(VT_DATE_VARENUM, 0, 3);
            assert!(
                !psa.is_null(),
                "SafeArrayCreateVector(VT_DATE_VARENUM) should succeed"
            );
            for (index, value) in [45200.25f64, 12.5f64, -4.25f64].into_iter().enumerate() {
                let index = i32::try_from(index).expect("index should fit in i32");
                let hr =
                    SafeArrayPutElement(psa.cast_const(), &index, (&value as *const f64).cast());
                assert!(
                    hr >= 0,
                    "SafeArrayPutElement(VT_DATE_VARENUM) should succeed: {hr:#010X}"
                );
            }
            variant.Anonymous.Anonymous.vt = VT_ARRAY | VT_DATE_VARENUM;
            variant.Anonymous.Anonymous.Anonymous.parray = psa;
            assert_eq!(
                variant_to_com_value(&variant).expect("read VT_DATE_VARENUM SAFEARRAY"),
                ComValue::ArrayIntent(
                    SafeArray::from_typed_values(
                        VT_DATE_VALUE,
                        vec![
                            RuntimeValue::F64(F64Value::from_date_f64(45200.25)),
                            RuntimeValue::F64(F64Value::from_date_f64(12.5)),
                            RuntimeValue::F64(F64Value::from_date_f64(-4.25)),
                        ],
                    )
                    .expect("typed date array"),
                )
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn typed_r8_safe_array_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            let psa = SafeArrayCreateVector(VT_R8_VARENUM, 0, 3);
            assert!(
                !psa.is_null(),
                "SafeArrayCreateVector(VT_R8_VARENUM) should succeed"
            );
            for (index, value) in [12.5f64, -4.25f64, 321.0f64].into_iter().enumerate() {
                let index = i32::try_from(index).expect("index should fit in i32");
                let hr =
                    SafeArrayPutElement(psa.cast_const(), &index, (&value as *const f64).cast());
                assert!(
                    hr >= 0,
                    "SafeArrayPutElement(VT_R8_VARENUM) should succeed: {hr:#010X}"
                );
            }
            variant.Anonymous.Anonymous.vt = VT_ARRAY | VT_R8_VARENUM;
            variant.Anonymous.Anonymous.Anonymous.parray = psa;
            assert_eq!(
                variant_to_com_value(&variant).expect("read VT_R8_VARENUM SAFEARRAY"),
                ComValue::ArrayIntent(
                    SafeArray::from_typed_values(
                        VT_R8_VALUE,
                        vec![
                            RuntimeValue::F64(F64Value::from_f64(12.5)),
                            RuntimeValue::F64(F64Value::from_f64(-4.25)),
                            RuntimeValue::F64(F64Value::from_f64(321.0)),
                        ],
                    )
                    .expect("typed r8 array"),
                )
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn typed_bstr_safe_array_roundtrips_through_windows_bridge() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            let psa = SafeArrayCreateVector(VT_BSTR, 0, 2);
            assert!(
                !psa.is_null(),
                "SafeArrayCreateVector(VT_BSTR) should succeed"
            );
            for (index, value) in ["Alpha", "Beta"].into_iter().enumerate() {
                let index = i32::try_from(index).expect("index should fit in i32");
                let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
                let bstr = SysAllocString(wide.as_ptr());
                assert!(!bstr.is_null(), "SysAllocString should succeed");
                let hr = SafeArrayPutElement(psa.cast_const(), &index, bstr.cast());
                SysFreeString(bstr);
                assert!(
                    hr >= 0,
                    "SafeArrayPutElement(VT_BSTR) should succeed: {hr:#010X}"
                );
            }
            variant.Anonymous.Anonymous.vt = VT_ARRAY | VT_BSTR;
            variant.Anonymous.Anonymous.Anonymous.parray = psa;
            assert_eq!(
                variant_to_com_value(&variant).expect("read VT_BSTR SAFEARRAY"),
                ComValue::ArrayIntent(
                    SafeArray::from_typed_values(
                        VT_BSTR_VALUE,
                        vec![
                            RuntimeValue::String(BStr::from("Alpha")),
                            RuntimeValue::String(BStr::from("Beta")),
                        ],
                    )
                    .expect("typed bstr array"),
                )
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[test]
    fn dispatch_result_classification_preserves_dispatch_pointer() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let mut add_ref_calls = 0usize;
        unsafe {
            variant.Anonymous.Anonymous.vt = VT_DISPATCH;
            variant.Anonymous.Anonymous.Anonymous.pdispVal = std::ptr::null_mut();
            let value = take_variant_result_value(
                &mut variant,
                &mut |_unknown| Err("unknown query should not be used".to_string()),
                &mut |_dispatch| add_ref_calls += 1,
            )
            .expect("classify dispatch result");
            assert_eq!(value, VariantResultValue::Dispatch(std::ptr::null_mut()));
        }
        assert_eq!(add_ref_calls, 0);
    }

    #[test]
    fn unknown_result_classification_propagates_query_error() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            variant.Anonymous.Anonymous.vt = VT_UNKNOWN;
            variant.Anonymous.Anonymous.Anonymous.punkVal = std::ptr::null_mut();
            let err = take_variant_result_value(
                &mut variant,
                &mut |raw_unknown| {
                    assert_eq!(raw_unknown, std::ptr::null_mut());
                    Err("null unknown".to_string())
                },
                &mut |_dispatch| panic!("dispatch AddRef is not expected on VT_UNKNOWN path"),
            )
            .expect_err("null unknown should fail deterministically");
            assert_eq!(err, "null unknown");
        }
    }

    #[test]
    fn unknown_enumvariant_result_materializes_to_runtime_array() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        unsafe {
            variant.Anonymous.Anonymous.vt = VT_UNKNOWN;
            variant.Anonymous.Anonymous.Anonymous.punkVal = create_oxvba_test_enum_unknown().cast();
            let value = variant_to_runtime_value(
                &variant,
                &mut |unknown| {
                    crate::query_dispatch_from_unknown(unknown.cast::<crate::RawIUnknown>())
                        .map(|dispatch| dispatch.cast())
                },
                &mut |_dispatch| panic!("dispatch AddRef is not expected on IEnumVARIANT path"),
                &mut |_dispatch, _prog_id_hint, _op| {
                    panic!("dispatch rebinding is not expected on IEnumVARIANT path")
                },
                "OxVba.TestDispatch",
                "dispatch_invoke",
            )
            .expect("IEnumVARIANT should materialize into an array-backed runtime value");
            assert_eq!(
                value,
                RuntimeValue::ArrayIntent(SafeArray::from_values(vec![
                    RuntimeValue::I32(41),
                    RuntimeValue::I32(42),
                ]))
            );
            let _ = VariantClear(&mut variant);
        }
    }
}
