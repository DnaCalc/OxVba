use oxvba_runtime::{
    ObjectRef, RuntimeByRefSlot, RuntimeByRefWriteback, RuntimeCallArgument, RuntimeCallContext,
    RuntimeCallError, RuntimeCallFrame, RuntimeCallKind, RuntimeCallResult, RuntimeCallSelector,
    RuntimeCallSource, RuntimeValueType, Variant,
};
use windows_sys::Win32::Foundation::SysAllocStringLen;
use windows_sys::Win32::System::Com::{
    DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPATCH_PROPERTYPUTREF, DISPPARAMS, EXCEPINFO,
};
use windows_sys::Win32::System::Variant::{VARIANT, VT_BYREF, VT_I4, VT_VARIANT};

use crate::{
    ComValue, windows_client::COM_DISP_E_EXCEPTION, windows_client::COM_DISPID_PROPERTYPUT,
    windows_variant::set_variant_from_com_value, windows_variant::variant_to_com_value,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComCallFrameMarshalError {
    pub message: String,
    pub logical_arg_index: Option<usize>,
}

impl ComCallFrameMarshalError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            logical_arg_index: None,
        }
    }

    fn with_logical_arg_index(mut self, logical_arg_index: usize) -> Self {
        self.logical_arg_index = Some(logical_arg_index);
        self
    }
}

/// Converts COM `DISPPARAMS` from `IDispatch::Invoke` into a runtime call frame.
///
/// # Safety
/// `params` must be non-null and point to a valid `DISPPARAMS` for the duration
/// of this call. When its argument counts are non-zero, `rgvarg` and
/// `rgdispidNamedArgs` must point to arrays valid for those counts.
pub unsafe fn disp_params_to_runtime_call_frame(
    dispatch_id: i32,
    flags: u16,
    params: *const DISPPARAMS,
    locale_id: u32,
) -> Result<RuntimeCallFrame, ComCallFrameMarshalError> {
    if params.is_null() {
        return Err(ComCallFrameMarshalError::new(
            "IDispatch::Invoke received null DISPPARAMS",
        ));
    }
    let params = unsafe { &*params };
    let arg_count = params.cArgs as usize;
    let named_count = params.cNamedArgs as usize;
    if arg_count > 0 && params.rgvarg.is_null() {
        return Err(ComCallFrameMarshalError::new(
            "IDispatch::Invoke DISPPARAMS had cArgs > 0 with null rgvarg",
        ));
    }
    if named_count > 0 && params.rgdispidNamedArgs.is_null() {
        return Err(ComCallFrameMarshalError::new(
            "IDispatch::Invoke DISPPARAMS had cNamedArgs > 0 with null rgdispidNamedArgs",
        ));
    }

    let selector = RuntimeCallSelector::DispatchId {
        interface: None,
        dispatch_id,
    };
    let mut frame = RuntimeCallFrame::new(selector, call_kind_from_dispatch_flags(flags))
        .with_context(
            RuntimeCallContext::new(RuntimeCallSource::ExternalComDispatch).with_locale(locale_id),
        );

    let named_dispids = if named_count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(params.rgdispidNamedArgs, named_count) }
    };
    let property_put_com_index = named_dispids
        .iter()
        .position(|dispid| *dispid == COM_DISPID_PROPERTYPUT)
        .and_then(|index| if index < arg_count { Some(index) } else { None });

    for com_index in (0..arg_count).rev() {
        let logical_index = arg_count - 1 - com_index;
        let variant = unsafe { &*params.rgvarg.add(com_index) };
        let argument = variant_to_runtime_call_argument(variant, logical_index)
            .map_err(|err| err.with_logical_arg_index(logical_index))?;
        if Some(com_index) == property_put_com_index {
            frame.set_property_put_arg(argument);
        } else if com_index < named_count && named_dispids[com_index] != COM_DISPID_PROPERTYPUT {
            frame.push_named_arg(format!("[dispid:{}]", named_dispids[com_index]), argument);
        } else {
            frame.push_positional_arg(argument);
        }
    }

    Ok(frame)
}

/// Writes a runtime call result into a COM `VARIANT`.
///
/// # Safety
/// `variant` must be a valid writable `VARIANT` pointer. Object resolution
/// callbacks must return valid COM interface pointers, and `add_ref_dispatch`
/// must apply the ownership convention expected by the COM caller.
pub unsafe fn runtime_call_result_to_variant<FResolve, FAddRef>(
    result: &RuntimeCallResult,
    variant: *mut VARIANT,
    resolve_object: &mut FResolve,
    add_ref_dispatch: &mut FAddRef,
) -> Result<(), String>
where
    FResolve: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
    FAddRef: FnMut(*mut core::ffi::c_void),
{
    let value = result.value.clone().unwrap_or_else(Variant::empty);
    let com_value = ComValue::from_variant(&value)?;
    unsafe { set_variant_from_com_value(variant, &com_value, resolve_object, add_ref_dispatch) }
}

/// Applies runtime ByRef writebacks to the original COM argument array.
///
/// # Safety
/// When `params` is non-null, it must point to a valid mutable `DISPPARAMS`.
/// If writebacks are present, `rgvarg` must be a writable array valid for
/// `cArgs` elements and each targeted ByRef slot must be writable.
pub unsafe fn apply_runtime_call_writebacks_to_disp_params(
    result: &RuntimeCallResult,
    params: *mut DISPPARAMS,
) -> Result<(), ComCallFrameMarshalError> {
    if params.is_null() {
        return Ok(());
    }
    let params = unsafe { &mut *params };
    let arg_count = params.cArgs as usize;
    for writeback in &result.writebacks {
        let logical_index = writeback.slot.id as usize;
        let Some(com_index) = logical_arg_index_to_com_arg_index(logical_index, arg_count) else {
            return Err(ComCallFrameMarshalError::new(format!(
                "byref writeback slot {} is outside DISPPARAMS arg count {}",
                writeback.slot.id, arg_count
            ))
            .with_logical_arg_index(logical_index));
        };
        let target = unsafe { &mut *params.rgvarg.add(com_index as usize) };
        unsafe { writeback_variant_value(target, writeback) }.map_err(|message| {
            ComCallFrameMarshalError {
                message,
                logical_arg_index: Some(logical_index),
            }
        })?;
    }
    Ok(())
}

pub fn logical_arg_index_to_com_arg_index(logical_index: usize, arg_count: usize) -> Option<u32> {
    if logical_index >= arg_count {
        None
    } else {
        Some((arg_count - 1 - logical_index) as u32)
    }
}

/// Converts a runtime call error into COM `EXCEPINFO` state.
///
/// # Safety
/// Non-null `excep_info` and `arg_err` pointers must be valid for writes. The
/// caller owns any BSTRs written to `excep_info` according to COM `EXCEPINFO`
/// rules and must release them.
pub unsafe fn runtime_call_error_to_excepinfo(
    error: &RuntimeCallError,
    excep_info: *mut EXCEPINFO,
    arg_err: *mut u32,
    arg_count: usize,
) -> i32 {
    if !arg_err.is_null()
        && let Some(logical_index) = error.argument_index
        && let Some(com_index) = logical_arg_index_to_com_arg_index(logical_index, arg_count)
    {
        unsafe {
            *arg_err = com_index;
        }
    }
    if !excep_info.is_null() {
        unsafe {
            (*excep_info).bstrSource = alloc_bstr("OxVBA Runtime");
            (*excep_info).bstrDescription = alloc_bstr(&error.message);
            (*excep_info).scode = error.code;
        }
    }
    COM_DISP_E_EXCEPTION
}

fn call_kind_from_dispatch_flags(flags: u16) -> RuntimeCallKind {
    if flags & DISPATCH_PROPERTYPUTREF != 0 {
        RuntimeCallKind::PropertySet
    } else if flags & DISPATCH_PROPERTYPUT != 0 {
        RuntimeCallKind::PropertyLet
    } else if flags & DISPATCH_PROPERTYGET != 0 {
        RuntimeCallKind::PropertyGet
    } else {
        RuntimeCallKind::Method
    }
}

fn variant_to_runtime_call_argument(
    variant: &VARIANT,
    logical_index: usize,
) -> Result<RuntimeCallArgument, ComCallFrameMarshalError> {
    let by_ref = unsafe { variant.Anonymous.Anonymous.vt & VT_BYREF != 0 };
    let value = unsafe { variant_to_com_value(variant) }
        .and_then(|value| value.to_variant())
        .map_err(|message| {
            ComCallFrameMarshalError::new(message).with_logical_arg_index(logical_index)
        })?;
    let expected_type = if by_ref {
        Some(runtime_value_type_from_variant(variant))
    } else {
        None
    };
    let slot = expected_type.map(|ty| RuntimeByRefSlot::new(logical_index as u32, Some(ty)));
    Ok(match slot {
        Some(slot) => RuntimeCallArgument::by_ref(value, slot),
        None => RuntimeCallArgument::by_value(value),
    })
}

fn runtime_value_type_from_variant(variant: &VARIANT) -> RuntimeValueType {
    let vt = unsafe { variant.Anonymous.Anonymous.vt & !VT_BYREF };
    match vt {
        VT_I4 => RuntimeValueType::Long,
        VT_VARIANT => RuntimeValueType::Variant,
        _ => RuntimeValueType::Variant,
    }
}

unsafe fn writeback_variant_value(
    target: &mut VARIANT,
    writeback: &RuntimeByRefWriteback,
) -> Result<(), String> {
    let vt = unsafe { target.Anonymous.Anonymous.vt };
    if vt != (VT_BYREF | VT_I4) {
        return Err(format!(
            "unsupported byref writeback target vt=0x{vt:04X}; currently supports VT_BYREF|VT_I4"
        ));
    }
    let Some(value) = writeback.value.as_i32() else {
        return Err("VT_BYREF|VT_I4 writeback requires Long Variant value".to_string());
    };
    let ptr = unsafe { target.Anonymous.Anonymous.Anonymous.plVal };
    if ptr.is_null() {
        return Err("VT_BYREF|VT_I4 writeback target was null".to_string());
    }
    unsafe {
        *ptr = value;
    }
    Ok(())
}

unsafe fn alloc_bstr(text: &str) -> windows_sys::core::BSTR {
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe { SysAllocStringLen(wide.as_ptr(), wide.len() as u32) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows_client::COM_DISP_E_TYPEMISMATCH;
    use oxvba_runtime::{RuntimeCallSource, bstr::BStr, safe_array::SafeArray};
    use windows_sys::Win32::System::Com::DISPATCH_METHOD;
    use windows_sys::Win32::System::Variant::{
        VT_ARRAY, VT_BSTR, VT_CLSID, VT_DISPATCH, VT_EMPTY, VT_VARIANT, VariantClear,
    };

    #[test]
    fn disp_params_to_frame_preserves_positional_order() {
        unsafe {
            let mut args: [VARIANT; 2] = std::mem::zeroed();
            args[0].Anonymous.Anonymous.vt = VT_I4;
            args[0].Anonymous.Anonymous.Anonymous.lVal = 20;
            args[1].Anonymous.Anonymous.vt = VT_I4;
            args[1].Anonymous.Anonymous.Anonymous.lVal = 10;
            let params = DISPPARAMS {
                rgvarg: args.as_mut_ptr(),
                rgdispidNamedArgs: std::ptr::null_mut(),
                cArgs: 2,
                cNamedArgs: 0,
            };
            let frame = disp_params_to_runtime_call_frame(7, DISPATCH_METHOD, &params, 1033)
                .expect("frame");
            assert_eq!(frame.kind, RuntimeCallKind::Method);
            assert_eq!(frame.context.source, RuntimeCallSource::ExternalComDispatch);
            assert_eq!(frame.context.locale_id, Some(1033));
            assert_eq!(frame.positional_args[0].value.as_i32(), Some(10));
            assert_eq!(frame.positional_args[1].value.as_i32(), Some(20));
        }
    }

    #[test]
    fn disp_params_to_frame_separates_property_put_arg() {
        unsafe {
            let mut arg: VARIANT = std::mem::zeroed();
            arg.Anonymous.Anonymous.vt = VT_I4;
            arg.Anonymous.Anonymous.Anonymous.lVal = 42;
            let mut named = [COM_DISPID_PROPERTYPUT];
            let params = DISPPARAMS {
                rgvarg: &mut arg,
                rgdispidNamedArgs: named.as_mut_ptr(),
                cArgs: 1,
                cNamedArgs: 1,
            };
            let frame = disp_params_to_runtime_call_frame(0, DISPATCH_PROPERTYPUT, &params, 0)
                .expect("frame");
            assert_eq!(frame.kind, RuntimeCallKind::PropertyLet);
            assert!(frame.positional_args.is_empty());
            assert_eq!(
                frame
                    .property_put_arg
                    .as_ref()
                    .and_then(|arg| arg.value.as_i32()),
                Some(42)
            );
        }
    }

    #[test]
    fn disp_params_to_frame_preserves_named_dispid_arguments() {
        unsafe {
            let mut args: [VARIANT; 2] = std::mem::zeroed();
            args[0].Anonymous.Anonymous.vt = VT_I4;
            args[0].Anonymous.Anonymous.Anonymous.lVal = 22;
            args[1].Anonymous.Anonymous.vt = VT_I4;
            args[1].Anonymous.Anonymous.Anonymous.lVal = 11;
            let mut named = [202, 101];
            let params = DISPPARAMS {
                rgvarg: args.as_mut_ptr(),
                rgdispidNamedArgs: named.as_mut_ptr(),
                cArgs: 2,
                cNamedArgs: 2,
            };
            let frame =
                disp_params_to_runtime_call_frame(7, DISPATCH_METHOD, &params, 0).expect("frame");
            assert!(frame.positional_args.is_empty());
            assert_eq!(frame.named_args[0].name, "[dispid:101]");
            assert_eq!(frame.named_args[0].argument.value.as_i32(), Some(11));
            assert_eq!(frame.named_args[1].name, "[dispid:202]");
            assert_eq!(frame.named_args[1].argument.value.as_i32(), Some(22));
        }
    }

    #[test]
    fn byref_i4_argument_writeback_uses_logical_argument_index() {
        unsafe {
            let mut first = 5i32;
            let mut second = 9i32;
            let mut args: [VARIANT; 2] = std::mem::zeroed();
            args[0].Anonymous.Anonymous.vt = VT_BYREF | VT_I4;
            args[0].Anonymous.Anonymous.Anonymous.plVal = &mut second;
            args[1].Anonymous.Anonymous.vt = VT_BYREF | VT_I4;
            args[1].Anonymous.Anonymous.Anonymous.plVal = &mut first;
            let mut params = DISPPARAMS {
                rgvarg: args.as_mut_ptr(),
                rgdispidNamedArgs: std::ptr::null_mut(),
                cArgs: 2,
                cNamedArgs: 0,
            };
            let frame =
                disp_params_to_runtime_call_frame(7, DISPATCH_METHOD, &params, 0).expect("frame");
            assert_eq!(frame.positional_args[0].by_ref.map(|slot| slot.id), Some(0));
            assert_eq!(frame.positional_args[1].by_ref.map(|slot| slot.id), Some(1));

            let mut result = RuntimeCallResult::empty();
            result.add_writeback(RuntimeByRefWriteback::new(
                RuntimeByRefSlot::new(1, Some(RuntimeValueType::Long)),
                Variant::from_i32(99),
            ));
            apply_runtime_call_writebacks_to_disp_params(&result, &mut params).expect("writeback");
            assert_eq!(first, 5);
            assert_eq!(second, 99);
        }
    }

    #[test]
    fn unsupported_variant_argument_reports_logical_argument_index() {
        unsafe {
            let mut args: [VARIANT; 1] = std::mem::zeroed();
            args[0].Anonymous.Anonymous.vt = VT_CLSID;
            let params = DISPPARAMS {
                rgvarg: args.as_mut_ptr(),
                rgdispidNamedArgs: std::ptr::null_mut(),
                cArgs: 1,
                cNamedArgs: 0,
            };
            let err = disp_params_to_runtime_call_frame(7, DISPATCH_METHOD, &params, 0)
                .expect_err("unsupported variant should fail");
            assert_eq!(err.logical_arg_index, Some(0));
            assert!(err.message.contains("unsupported VARIANT"));
        }
    }

    #[test]
    fn result_to_variant_and_runtime_error_mapping_are_explicit() {
        unsafe {
            let result = RuntimeCallResult::value(Variant::from_string(BStr::from("ok")));
            let mut variant: VARIANT = std::mem::zeroed();
            runtime_call_result_to_variant(
                &result,
                &mut variant,
                &mut |_object| Err("object resolution not expected".to_string()),
                &mut |_dispatch| {},
            )
            .expect("result variant");
            assert_eq!(variant.Anonymous.Anonymous.vt, VT_BSTR);
            VariantClear(&mut variant);

            let error = RuntimeCallError::new(
                COM_DISP_E_TYPEMISMATCH,
                "bad argument",
                RuntimeCallSource::ExternalComDispatch,
            )
            .with_argument_index(0);
            let mut excep: EXCEPINFO = std::mem::zeroed();
            let mut arg_err = u32::MAX;
            let hr = runtime_call_error_to_excepinfo(&error, &mut excep, &mut arg_err, 2);
            assert_eq!(hr, COM_DISP_E_EXCEPTION);
            assert_eq!(arg_err, 1);
            assert!(!excep.bstrDescription.is_null());
            windows_sys::Win32::Foundation::SysFreeString(excep.bstrSource);
            windows_sys::Win32::Foundation::SysFreeString(excep.bstrDescription);
        }
    }

    #[test]
    fn result_to_variant_maps_object_and_safearray_carriers() {
        unsafe {
            let object = ObjectRef::from_compat_identity(123);
            let mut object_variant: VARIANT = std::mem::zeroed();
            let mut add_refs = 0;
            runtime_call_result_to_variant(
                &RuntimeCallResult::value(Variant::from_object_ref(object.clone())),
                &mut object_variant,
                &mut |received| {
                    assert_eq!(received, object);
                    Ok(0x1234usize as *mut core::ffi::c_void)
                },
                &mut |dispatch| {
                    assert_eq!(dispatch, 0x1234usize as *mut core::ffi::c_void);
                    add_refs += 1;
                },
            )
            .expect("object result variant");
            assert_eq!(object_variant.Anonymous.Anonymous.vt, VT_DISPATCH);
            assert_eq!(add_refs, 1);

            let array = SafeArray::from_variants(vec![Variant::from_i32(4), Variant::from_i32(9)]);
            let mut array_variant: VARIANT = std::mem::zeroed();
            runtime_call_result_to_variant(
                &RuntimeCallResult::value(Variant::from_safearray(array)),
                &mut array_variant,
                &mut |_object| Err("object resolution not expected".to_string()),
                &mut |_dispatch| {},
            )
            .expect("SAFEARRAY result variant");
            assert_eq!(array_variant.Anonymous.Anonymous.vt, VT_ARRAY | VT_VARIANT);
            let com_value = variant_to_com_value(&array_variant).expect("read SAFEARRAY result");
            let ComValue::ArrayIntent(roundtripped) = com_value else {
                panic!("expected SAFEARRAY ComValue");
            };
            assert_eq!(roundtripped.len(), 2);
            VariantClear(&mut array_variant);
        }
    }

    #[test]
    fn result_to_variant_defaults_empty_result_to_vt_empty() {
        unsafe {
            let result = RuntimeCallResult::empty();
            let mut variant: VARIANT = std::mem::zeroed();
            runtime_call_result_to_variant(
                &result,
                &mut variant,
                &mut |_object| Err("object resolution not expected".to_string()),
                &mut |_dispatch| {},
            )
            .expect("empty result");
            assert_eq!(variant.Anonymous.Anonymous.vt, VT_EMPTY);
        }
    }
}
