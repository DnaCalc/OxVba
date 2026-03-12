use crate::{
    ComInvokeArg, VariantResultValue, set_variant_from_com_value, take_variant_result_value,
};
use oxvba_runtime::ObjectHandle;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{SysFreeString, SysStringLen};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Com::{DISPPARAMS, EXCEPINFO};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Variant::{VARIANT, VT_ERROR};

#[cfg(target_os = "windows")]
const IID_NULL: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0,
    data2: 0,
    data3: 0,
    data4: [0; 8],
};

#[cfg(target_os = "windows")]
const COM_DISP_E_PARAMNOTFOUND: i32 = 0x8002_0004u32 as i32;

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIUnknownVtbl {
    query_interface: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        iid: *const windows_sys::core::GUID,
        interface: *mut *mut core::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIDispatchVtbl {
    unknown: RawIUnknownVtbl,
    get_type_info_count:
        unsafe extern "system" fn(this: *mut core::ffi::c_void, pctinfo: *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        itinfo: u32,
        lcid: u32,
        pptinfo: *mut *mut core::ffi::c_void,
    ) -> i32,
    get_ids_of_names: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        riid: *const windows_sys::core::GUID,
        names: *mut *mut u16,
        count: u32,
        lcid: u32,
        dispids: *mut i32,
    ) -> i32,
    invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        dispid_member: i32,
        riid: *const windows_sys::core::GUID,
        lcid: u32,
        w_flags: u16,
        params: *mut DISPPARAMS,
        result: *mut VARIANT,
        excep_info: *mut EXCEPINFO,
        pu_arg_err: *mut u32,
    ) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIDispatch {
    vtbl: *const RawIDispatchVtbl,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComInvokeExceptionInfo {
    pub source: Option<String>,
    pub description: Option<String>,
    pub scode: Option<i32>,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComInvokeFailure {
    pub label: &'static str,
    pub dispid: i32,
    pub hr: Option<i32>,
    pub arg_err: Option<u32>,
    pub excep: Option<ComInvokeExceptionInfo>,
    pub detail: Option<String>,
}

#[cfg(target_os = "windows")]
impl ComInvokeFailure {
    pub fn render(&self) -> String {
        let mut message = format!(
            "IDispatch::Invoke({} dispid={}) failed",
            self.label, self.dispid
        );
        if let Some(hr) = self.hr {
            message.push_str(&format!(" with HRESULT {:#010X}", hr as u32));
        }
        if let Some(arg_err) = self.arg_err {
            message.push_str(&format!(" (arg_err={arg_err})"));
        }
        if let Some(excep) = &self.excep {
            if let Some(source) = &excep.source {
                message.push_str(&format!(
                    " excep_source=\"{}\"",
                    sanitize_error_text(source)
                ));
            }
            if let Some(description) = &excep.description {
                message.push_str(&format!(
                    " excep_description=\"{}\"",
                    sanitize_error_text(description)
                ));
            }
            if let Some(scode) = excep.scode {
                message.push_str(&format!(" excep_scode={:#010X}", scode as u32));
            }
        }
        if let Some(detail) = &self.detail {
            message.push_str(&format!(" detail=\"{}\"", sanitize_error_text(detail)));
        }
        message
    }
}

#[cfg(target_os = "windows")]
fn sanitize_error_text(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn bstr_to_string_and_free(bstr: windows_sys::core::BSTR) -> Option<String> {
    if bstr.is_null() {
        return None;
    }
    let len = usize::try_from(SysStringLen(bstr)).unwrap_or(0);
    let slice = std::slice::from_raw_parts(bstr, len);
    let text = String::from_utf16_lossy(slice);
    SysFreeString(bstr);
    Some(text)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// Consume any owned BSTR fields from an `EXCEPINFO` and convert the non-empty details into the
/// shared invoke-failure payload used by the Windows COM bridge.
///
/// # Safety
/// The caller must provide a valid writable `EXCEPINFO` pointer whose BSTR fields, when non-null,
/// are owned by the caller and may be released exactly once by this function.
pub unsafe fn take_excepinfo(excep: &mut EXCEPINFO) -> Option<ComInvokeExceptionInfo> {
    let source = bstr_to_string_and_free(excep.bstrSource);
    let description = bstr_to_string_and_free(excep.bstrDescription);
    let _ = bstr_to_string_and_free(excep.bstrHelpFile);
    excep.bstrSource = std::ptr::null_mut();
    excep.bstrDescription = std::ptr::null_mut();
    excep.bstrHelpFile = std::ptr::null_mut();
    let scode = if excep.scode != 0 {
        Some(excep.scode)
    } else {
        None
    };
    if source.is_none() && description.is_none() && scode.is_none() {
        None
    } else {
        Some(ComInvokeExceptionInfo {
            source,
            description,
            scode,
        })
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_missing_arg(variant: &mut VARIANT) {
    variant.Anonymous.Anonymous.vt = VT_ERROR;
    variant.Anonymous.Anonymous.Anonymous.scode = COM_DISP_E_PARAMNOTFOUND;
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn clear_variant_args(args: &mut [VARIANT]) {
    for variant in args {
        let _ = windows_sys::Win32::System::Variant::VariantClear(variant);
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// Execute a Windows `IDispatch::Invoke` call over the shared semantic COM request carrier and
/// return the classified semantic result shape.
///
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call.
/// `resolve_object`, `query_dispatch_from_unknown`, and `add_ref_dispatch` must uphold the COM
/// ownership and identity rules for any object handles or returned interface pointers they touch.
pub unsafe fn invoke_dispatch_variant_result<FResolveObject, FQueryUnknown, FAddRefDispatch>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    flags: u16,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    label: &'static str,
    resolve_object: &mut FResolveObject,
    query_dispatch_from_unknown: &mut FQueryUnknown,
    add_ref_dispatch: &mut FAddRefDispatch,
) -> Result<VariantResultValue, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectHandle) -> Result<*mut core::ffi::c_void, String>,
    FQueryUnknown: FnMut(*mut core::ffi::c_void) -> Result<*mut core::ffi::c_void, String>,
    FAddRefDispatch: FnMut(*mut core::ffi::c_void),
{
    let dispatch = dispatch.cast::<RawIDispatch>();
    let mut invoke_args: Vec<VARIANT> = Vec::with_capacity(args.len());
    for arg in args.iter().rev() {
        let mut variant: VARIANT = std::mem::zeroed();
        match arg.value {
            Some(ref value) => {
                set_variant_from_com_value(&mut variant, value, resolve_object, add_ref_dispatch)
                    .map_err(|detail| ComInvokeFailure {
                        label,
                        dispid,
                        hr: None,
                        arg_err: None,
                        excep: None,
                        detail: Some(detail),
                    })?
            }
            None => set_variant_missing_arg(&mut variant),
        }
        invoke_args.push(variant);
    }

    let mut named_arg_dispids_reversed: Vec<i32> =
        named_arg_dispids.iter().rev().copied().collect();
    let mut result: VARIANT = std::mem::zeroed();
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = u32::MAX;
    let mut params = DISPPARAMS {
        rgvarg: if invoke_args.is_empty() {
            std::ptr::null_mut()
        } else {
            invoke_args.as_mut_ptr()
        },
        rgdispidNamedArgs: if named_arg_dispids_reversed.is_empty() {
            std::ptr::null_mut()
        } else {
            named_arg_dispids_reversed.as_mut_ptr()
        },
        cArgs: args.len() as u32,
        cNamedArgs: named_arg_dispids.len() as u32,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        flags,
        &mut params,
        &mut result,
        &mut excep,
        &mut arg_err,
    );
    clear_variant_args(&mut invoke_args);
    if hr < 0 {
        return Err(ComInvokeFailure {
            label,
            dispid,
            hr: Some(hr),
            arg_err: (arg_err != u32::MAX).then_some(arg_err),
            excep: take_excepinfo(&mut excep),
            detail: None,
        });
    }
    take_variant_result_value(&mut result, query_dispatch_from_unknown, add_ref_dispatch).map_err(
        |detail| ComInvokeFailure {
            label,
            dispid,
            hr: None,
            arg_err: None,
            excep: None,
            detail: Some(detail),
        },
    )
}
