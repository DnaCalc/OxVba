#![allow(clippy::result_large_err)]

use crate::{
    ComBinding, ComInvokeArg, ComInvokeRequest, VariantResultValue, set_variant_from_com_value,
    take_variant_result_value, take_variant_result_variant,
};
use oxvba_runtime::{ObjectRef, Variant};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{SysFreeString, SysStringLen};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Com::{
    DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPATCH_PROPERTYPUTREF,
    DISPPARAMS, EXCEPINFO,
};
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
    pub help_file: Option<String>,
    pub help_context: Option<u32>,
    pub scode: Option<i32>,
    pub wcode: Option<u16>,
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
fn render_invoke_fault_message(failure: &ComInvokeFailure) -> String {
    let label = failure.classification_label();
    let mut suffix = String::new();
    if let Some(hr) = failure.hr {
        suffix.push_str(&format!("hresult=0x{:08X};", hr as u32));
    }
    if let Some(value) = failure.arg_err {
        suffix.push_str(&format!("arg_err={value};"));
    }
    if let Some(excep) = &failure.excep {
        if let Some(scode) = excep.scode {
            suffix.push_str(&format!("excep_scode=0x{:08X};", scode as u32));
        }
        if let Some(wcode) = excep.wcode {
            suffix.push_str(&format!("excep_wcode={wcode};"));
        }
    }
    let prefix = if suffix.is_empty() {
        format!("com-dispatch-{label}")
    } else {
        format!("com-dispatch-{label};{suffix}")
    };
    format!("{prefix} {}", failure.render())
}

#[cfg(target_os = "windows")]
impl ComInvokeFailure {
    pub fn classification_label(&self) -> &'static str {
        if self.hr.is_none()
            && self.arg_err.is_none()
            && let Some(label) = classify_invoke_detail_label(self.detail.as_deref())
        {
            return label;
        }
        map_com_hresult_label(self.hr.map(|hr| hr as u32), self.arg_err)
    }

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
            if let Some(help_file) = &excep.help_file {
                message.push_str(&format!(
                    " excep_help_file=\"{}\"",
                    sanitize_error_text(help_file)
                ));
            }
            if let Some(help_context) = excep.help_context {
                message.push_str(&format!(" excep_help_context={help_context}"));
            }
            if let Some(scode) = excep.scode {
                message.push_str(&format!(" excep_scode={:#010X}", scode as u32));
            }
            if let Some(wcode) = excep.wcode {
                message.push_str(&format!(" excep_wcode={wcode}"));
            }
        }
        if let Some(detail) = &self.detail {
            message.push_str(&format!(" detail=\"{}\"", sanitize_error_text(detail)));
        }
        message
    }
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
fn classify_invoke_detail_label(detail: Option<&str>) -> Option<&'static str> {
    let detail = detail?;
    if detail.contains("exceeds i64 carrier range") {
        return Some("carrier-overflow");
    }
    if detail.starts_with("unsupported VARIANT BYREF return type vt=") {
        return Some("unsupported-byref-return");
    }
    None
}

#[cfg(target_os = "windows")]
fn sanitize_error_text(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// Read a callee-transferred BSTR into an owned `String` and free it with
/// `SysFreeString` (we own the transferred string). Shared with the vtable
/// marshaller, which has the same retval-BSTR ownership shape.
///
/// # Safety
/// `bstr`, when non-null, must be a BSTR whose ownership has transferred to us
/// (so freeing it exactly once here is correct) and which is otherwise valid.
pub(crate) unsafe fn bstr_to_string_and_free(bstr: windows_sys::core::BSTR) -> Option<String> {
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
    let help_file = bstr_to_string_and_free(excep.bstrHelpFile);
    excep.bstrSource = std::ptr::null_mut();
    excep.bstrDescription = std::ptr::null_mut();
    excep.bstrHelpFile = std::ptr::null_mut();
    let help_context = if excep.dwHelpContext != 0 {
        Some(excep.dwHelpContext)
    } else {
        None
    };
    let scode = if excep.scode != 0 {
        Some(excep.scode)
    } else {
        None
    };
    let wcode = if excep.wCode != 0 {
        Some(excep.wCode)
    } else {
        None
    };
    if source.is_none()
        && description.is_none()
        && help_file.is_none()
        && help_context.is_none()
        && scode.is_none()
        && wcode.is_none()
    {
        None
    } else {
        Some(ComInvokeExceptionInfo {
            source,
            description,
            help_file,
            help_context,
            scode,
            wcode,
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
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
    FQueryUnknown: FnMut(*mut core::ffi::c_void) -> Result<*mut core::ffi::c_void, String>,
    FAddRefDispatch: FnMut(*mut core::ffi::c_void),
{
    let dispatch = dispatch.cast::<RawIDispatch>();
    let mut invoke_args: Vec<VARIANT> = Vec::with_capacity(args.len());
    for arg in args.iter().rev() {
        let mut variant: VARIANT = std::mem::zeroed();
        match arg.value {
            Some(ref value) => {
                let value = value.to_com_value();
                if let Err(detail) = set_variant_from_com_value(
                    &mut variant,
                    &value,
                    resolve_object,
                    add_ref_dispatch,
                ) {
                    // VARIANT has no Drop: free the args marshalled so far
                    // (owned BSTRs/SAFEARRAYs/AddRef'd interfaces) before
                    // propagating (W1-com-001).
                    let _ = windows_sys::Win32::System::Variant::VariantClear(&mut variant);
                    clear_variant_args(&mut invoke_args);
                    return Err(ComInvokeFailure {
                        label,
                        dispid,
                        hr: None,
                        arg_err: None,
                        excep: None,
                        detail: Some(detail),
                    });
                }
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

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// Windows `IDispatch::Invoke` call that projects the result
/// into the retained Variant value shape, delegating any dispatch-backed
/// result rebinding to the caller.
///
/// New value-model call sites that do not need dispatch-backed rebinding
/// should prefer [`invoke_dispatch_variant_result`].
///
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call.
/// The callback closures must uphold COM ownership and runtime identity guarantees for any object
/// handles or returned interface pointers they touch.
pub unsafe fn invoke_dispatch_variant<
    FResolveObject,
    FQueryUnknown,
    FAddRefDispatch,
    FBindDispatch,
>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    flags: u16,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    label: &'static str,
    prog_id_hint: &str,
    resolve_object: &mut FResolveObject,
    query_dispatch_from_unknown: &mut FQueryUnknown,
    add_ref_dispatch: &mut FAddRefDispatch,
    bind_dispatch_result: &mut FBindDispatch,
) -> Result<Variant, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
    FQueryUnknown: FnMut(*mut core::ffi::c_void) -> Result<*mut core::ffi::c_void, String>,
    FAddRefDispatch: FnMut(*mut core::ffi::c_void),
    FBindDispatch: FnMut(*mut core::ffi::c_void, &str, &'static str) -> Result<Variant, String>,
{
    let mut result = std::mem::zeroed();
    let mut excep = std::mem::zeroed();
    let mut arg_err = u32::MAX;
    let dispatch = dispatch.cast::<RawIDispatch>();
    let mut invoke_args: Vec<VARIANT> = Vec::with_capacity(args.len());
    for arg in args.iter().rev() {
        let mut variant: VARIANT = std::mem::zeroed();
        match arg.value {
            Some(ref value) => {
                let value = value.to_com_value();
                if let Err(detail) = set_variant_from_com_value(
                    &mut variant,
                    &value,
                    resolve_object,
                    add_ref_dispatch,
                ) {
                    // Free already-marshalled args before propagating (W1-com-001).
                    let _ = windows_sys::Win32::System::Variant::VariantClear(&mut variant);
                    clear_variant_args(&mut invoke_args);
                    return Err(validation_failure(label, dispid, detail));
                }
            }
            None => set_variant_missing_arg(&mut variant),
        }
        invoke_args.push(variant);
    }
    let mut named_arg_dispids_reversed =
        named_arg_dispids.iter().copied().rev().collect::<Vec<_>>();
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
    take_variant_result_variant(
        &mut result,
        query_dispatch_from_unknown,
        add_ref_dispatch,
        &mut |dispatch: *mut core::ffi::c_void, prog_id_hint: &str, op: &'static str| {
            bind_dispatch_result(dispatch, prog_id_hint, op)
        },
        prog_id_hint,
        "dispatch_invoke",
    )
    .map_err(|detail| ComInvokeFailure {
        label,
        dispid,
        hr: None,
        arg_err: None,
        excep: None,
        detail: Some(detail),
    })
}
fn validation_failure(
    label: &'static str,
    dispid: i32,
    detail: impl Into<String>,
) -> ComInvokeFailure {
    ComInvokeFailure {
        label,
        dispid,
        hr: None,
        arg_err: None,
        excep: None,
        detail: Some(detail.into()),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call, and
/// `resolve_object` must return stable live `IDispatch` pointers for any object-handle arguments.
pub unsafe fn invoke_dispatch_legacy_i32_result<FResolveObject>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    flags: u16,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    label: &'static str,
    resolve_object: &mut FResolveObject,
) -> Result<i32, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
{
    let dispatch = dispatch.cast::<RawIDispatch>();
    let mut invoke_args: Vec<VARIANT> = Vec::with_capacity(args.len());
    for arg in args.iter().rev() {
        let mut variant: VARIANT = std::mem::zeroed();
        match arg.value {
            Some(ref value) => {
                let value = value.to_com_value();
                let mut add_ref_dispatch = |_dispatch: *mut core::ffi::c_void| {};
                if let Err(detail) = set_variant_from_com_value(
                    &mut variant,
                    &value,
                    resolve_object,
                    &mut add_ref_dispatch,
                ) {
                    // Free already-marshalled args before propagating (W1-com-001).
                    let _ = windows_sys::Win32::System::Variant::VariantClear(&mut variant);
                    clear_variant_args(&mut invoke_args);
                    return Err(ComInvokeFailure {
                        label,
                        dispid,
                        hr: None,
                        arg_err: None,
                        excep: None,
                        detail: Some(detail),
                    });
                }
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

    let token = match crate::variant_to_com_value(&result) {
        Ok(value) => match value.to_runtime_token() {
            Ok(token) => token,
            Err(detail) => {
                let _ = windows_sys::Win32::System::Variant::VariantClear(&mut result);
                return Err(validation_failure(label, dispid, detail));
            }
        },
        Err(detail) => {
            let _ = windows_sys::Win32::System::Variant::VariantClear(&mut result);
            return Err(validation_failure(label, dispid, detail));
        }
    };
    let _ = windows_sys::Win32::System::Variant::VariantClear(&mut result);
    Ok(token)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call, and
/// `resolve_object` must return stable live `IDispatch` pointers for any object-handle arguments.
pub unsafe fn invoke_dispatch_legacy_i32_result_positional<FResolveObject>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    flags: u16,
    args: &[crate::ComValue],
    property_put_named_arg: bool,
    label: &'static str,
    resolve_object: &mut FResolveObject,
) -> Result<i32, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
{
    let dispatch = dispatch.cast::<RawIDispatch>();
    let mut invoke_args: Vec<VARIANT> = Vec::with_capacity(args.len());
    for arg in args.iter().rev() {
        let mut variant: VARIANT = std::mem::zeroed();
        let mut add_ref_dispatch = |_dispatch: *mut core::ffi::c_void| {};
        if let Err(detail) =
            set_variant_from_com_value(&mut variant, arg, resolve_object, &mut add_ref_dispatch)
        {
            // Free already-marshalled args before propagating (W1-com-001).
            let _ = windows_sys::Win32::System::Variant::VariantClear(&mut variant);
            clear_variant_args(&mut invoke_args);
            return Err(validation_failure(label, dispid, detail));
        }
        invoke_args.push(variant);
    }

    let mut named_arg = crate::COM_DISPID_PROPERTYPUT;
    let mut result: VARIANT = std::mem::zeroed();
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = u32::MAX;
    let mut params = DISPPARAMS {
        rgvarg: if invoke_args.is_empty() {
            std::ptr::null_mut()
        } else {
            invoke_args.as_mut_ptr()
        },
        rgdispidNamedArgs: if property_put_named_arg {
            &mut named_arg
        } else {
            std::ptr::null_mut()
        },
        cArgs: args.len() as u32,
        cNamedArgs: if property_put_named_arg { 1 } else { 0 },
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

    let token = match crate::variant_to_com_value(&result) {
        Ok(value) => match value.to_runtime_token() {
            Ok(token) => token,
            Err(detail) => {
                let _ = windows_sys::Win32::System::Variant::VariantClear(&mut result);
                return Err(validation_failure(label, dispid, detail));
            }
        },
        Err(detail) => {
            let _ = windows_sys::Win32::System::Variant::VariantClear(&mut result);
            return Err(validation_failure(label, dispid, detail));
        }
    };
    let _ = windows_sys::Win32::System::Variant::VariantClear(&mut result);
    Ok(token)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call, and
/// the callback closures must uphold COM ownership rules for resolved object arguments.
pub unsafe fn invoke_member_spec_legacy_i32_result<FResolveNamedArgDispids, FResolveObject>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    spec: &crate::ComMemberSpec,
    args: &[ComInvokeArg],
    resolve_named_arg_dispids: &mut FResolveNamedArgDispids,
    resolve_object: &mut FResolveObject,
) -> Result<i32, ComInvokeFailure>
where
    FResolveNamedArgDispids: FnMut(&str, &[ComInvokeArg]) -> Result<Vec<i32>, String>,
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
{
    let canonical_args;
    let args = match spec.invoke_kind {
        crate::TypeLibMemberInvokeKind::PropertyPut
        | crate::TypeLibMemberInvokeKind::PropertyPutRef => {
            canonical_args = crate::invoke_policy::canonicalize_member_known_args(spec, args)
                .map_err(|detail| validation_failure("dispatch_invoke", dispid, detail))?;
            canonical_args.as_slice()
        }
        _ => args,
    };
    if spec.requires_argument && args.iter().all(|arg| arg.value.is_none()) {
        return Err(validation_failure(
            "dispatch_invoke",
            dispid,
            "member requires argument but DispatchInvoke omitted the third argument",
        ));
    }
    if !spec.requires_argument {
        return match spec.invoke_kind {
            crate::TypeLibMemberInvokeKind::PropertyGet => invoke_dispatch_legacy_i32_result(
                dispatch,
                dispid,
                DISPATCH_PROPERTYGET,
                &[],
                &[],
                "property-get",
                resolve_object,
            ),
            crate::TypeLibMemberInvokeKind::Method => invoke_dispatch_legacy_i32_result(
                dispatch,
                dispid,
                DISPATCH_METHOD,
                &[],
                &[],
                "method",
                resolve_object,
            ),
            crate::TypeLibMemberInvokeKind::PropertyPut
            | crate::TypeLibMemberInvokeKind::PropertyPutRef => Err(validation_failure(
                "dispatch_invoke",
                dispid,
                "member requires argument for property put/putref dispatch",
            )),
        };
    }
    match spec.invoke_kind {
        crate::TypeLibMemberInvokeKind::PropertyGet => {
            let named_arg_dispids = resolve_named_arg_dispids(&spec.name, args)
                .map_err(|detail| validation_failure("property-get", dispid, detail))?;
            invoke_dispatch_legacy_i32_result(
                dispatch,
                dispid,
                DISPATCH_PROPERTYGET,
                args,
                &named_arg_dispids,
                "property-get",
                resolve_object,
            )
        }
        crate::TypeLibMemberInvokeKind::Method => {
            let named_arg_dispids = resolve_named_arg_dispids(&spec.name, args)
                .map_err(|detail| validation_failure("method", dispid, detail))?;
            invoke_dispatch_legacy_i32_result(
                dispatch,
                dispid,
                DISPATCH_METHOD,
                args,
                &named_arg_dispids,
                "method",
                resolve_object,
            )
        }
        crate::TypeLibMemberInvokeKind::PropertyPut => {
            let named_arg_dispids =
                resolve_named_arg_dispids(&spec.name, &args[..args.len().saturating_sub(1)])
                    .map_err(|detail| validation_failure("property-put", dispid, detail))?;
            if named_arg_dispids.is_empty()
                && args
                    .iter()
                    .all(|arg| arg.name.is_none() && arg.value.is_some())
            {
                let positional_args: Result<Vec<crate::ComValue>, String> = args
                    .iter()
                    .filter_map(|arg| arg.value.clone())
                    .map(|value| {
                        value.to_legacy_dispatch_token()?;
                        Ok(value.to_com_value())
                    })
                    .collect();
                if let Ok(positional_args) = positional_args {
                    return invoke_dispatch_legacy_i32_result_positional(
                        dispatch,
                        dispid,
                        DISPATCH_PROPERTYPUT,
                        &positional_args,
                        true,
                        "property-put",
                        resolve_object,
                    );
                }
            }
            let mut all_named = named_arg_dispids;
            all_named.push(crate::COM_DISPID_PROPERTYPUT);
            invoke_dispatch_legacy_i32_result(
                dispatch,
                dispid,
                DISPATCH_PROPERTYPUT,
                args,
                &all_named,
                "property-put",
                resolve_object,
            )
        }
        crate::TypeLibMemberInvokeKind::PropertyPutRef => {
            let named_arg_dispids =
                resolve_named_arg_dispids(&spec.name, &args[..args.len().saturating_sub(1)])
                    .map_err(|detail| validation_failure("property-putref", dispid, detail))?;
            if named_arg_dispids.is_empty()
                && args
                    .iter()
                    .all(|arg| arg.name.is_none() && arg.value.is_some())
            {
                let positional_args: Result<Vec<crate::ComValue>, String> = args
                    .iter()
                    .filter_map(|arg| arg.value.clone())
                    .map(|value| {
                        value.to_legacy_dispatch_token()?;
                        Ok(value.to_com_value())
                    })
                    .collect();
                if let Ok(positional_args) = positional_args {
                    return invoke_dispatch_legacy_i32_result_positional(
                        dispatch,
                        dispid,
                        DISPATCH_PROPERTYPUTREF,
                        &positional_args,
                        true,
                        "property-putref",
                        resolve_object,
                    );
                }
            }
            let mut all_named = named_arg_dispids;
            all_named.push(crate::COM_DISPID_PROPERTYPUT);
            invoke_dispatch_legacy_i32_result(
                dispatch,
                dispid,
                DISPATCH_PROPERTYPUTREF,
                args,
                &all_named,
                "property-putref",
                resolve_object,
            )
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// Member-metadata-backed Windows `IDispatch::Invoke` call.
///
/// Arguments are marshalled from retained `ComInvokeValue`/`Variant` payloads,
/// then the result is returned as retained [`Variant`] values.
///
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call.
/// The callback closures must uphold COM ownership and runtime identity guarantees for any object handles or returned interface pointers they touch.
pub unsafe fn invoke_member_spec_variant<
    FResolveNamedArgDispids,
    FResolveObject,
    FQueryUnknown,
    FAddRefDispatch,
    FBindDispatch,
>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    spec: &crate::ComMemberSpec,
    args: &[ComInvokeArg],
    prog_id_hint: &str,
    resolve_named_arg_dispids: &mut FResolveNamedArgDispids,
    resolve_object: &mut FResolveObject,
    query_dispatch_from_unknown: &mut FQueryUnknown,
    add_ref_dispatch: &mut FAddRefDispatch,
    bind_dispatch_result: &mut FBindDispatch,
) -> Result<Variant, ComInvokeFailure>
where
    FResolveNamedArgDispids: FnMut(&str, &[ComInvokeArg]) -> Result<Vec<i32>, String>,
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
    FQueryUnknown: FnMut(*mut core::ffi::c_void) -> Result<*mut core::ffi::c_void, String>,
    FAddRefDispatch: FnMut(*mut core::ffi::c_void),
    FBindDispatch: FnMut(*mut core::ffi::c_void, &str, &'static str) -> Result<Variant, String>,
{
    let canonical_args;
    let args = match spec.invoke_kind {
        crate::TypeLibMemberInvokeKind::PropertyPut
        | crate::TypeLibMemberInvokeKind::PropertyPutRef => {
            canonical_args = crate::invoke_policy::canonicalize_member_known_args(spec, args)
                .map_err(|detail| validation_failure("dispatch_invoke", dispid, detail))?;
            canonical_args.as_slice()
        }
        _ => args,
    };
    if spec.requires_argument && args.iter().all(|arg| arg.value.is_none()) {
        return Err(validation_failure(
            "dispatch_invoke",
            dispid,
            "member requires argument but DispatchInvoke omitted the third argument",
        ));
    }
    if !spec.requires_argument {
        return match spec.invoke_kind {
            crate::TypeLibMemberInvokeKind::PropertyGet => invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_PROPERTYGET,
                &[],
                &[],
                "property-get",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            ),
            crate::TypeLibMemberInvokeKind::Method => invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_METHOD,
                &[],
                &[],
                "method",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            ),
            crate::TypeLibMemberInvokeKind::PropertyPut
            | crate::TypeLibMemberInvokeKind::PropertyPutRef => Err(validation_failure(
                "dispatch_invoke",
                dispid,
                "member requires argument for property put/putref dispatch",
            )),
        };
    }
    match spec.invoke_kind {
        crate::TypeLibMemberInvokeKind::PropertyGet => {
            let named_arg_dispids = resolve_named_arg_dispids(&spec.name, args)
                .map_err(|detail| validation_failure("property-get", dispid, detail))?;
            invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_PROPERTYGET,
                args,
                &named_arg_dispids,
                "property-get",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            )
        }
        crate::TypeLibMemberInvokeKind::Method => {
            let named_arg_dispids = resolve_named_arg_dispids(&spec.name, args)
                .map_err(|detail| validation_failure("method", dispid, detail))?;
            invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_METHOD,
                args,
                &named_arg_dispids,
                "method",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            )
        }
        crate::TypeLibMemberInvokeKind::PropertyPut => {
            let named_arg_dispids =
                resolve_named_arg_dispids(&spec.name, &args[..args.len().saturating_sub(1)])
                    .map_err(|detail| validation_failure("property-put", dispid, detail))?;
            let mut all_named = named_arg_dispids;
            all_named.push(crate::COM_DISPID_PROPERTYPUT);
            invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_PROPERTYPUT,
                args,
                &all_named,
                "property-put",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            )
        }
        crate::TypeLibMemberInvokeKind::PropertyPutRef => {
            let named_arg_dispids =
                resolve_named_arg_dispids(&spec.name, &args[..args.len().saturating_sub(1)])
                    .map_err(|detail| validation_failure("property-putref", dispid, detail))?;
            let mut all_named = named_arg_dispids;
            all_named.push(crate::COM_DISPID_PROPERTYPUT);
            invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_PROPERTYPUTREF,
                args,
                &all_named,
                "property-putref",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            )
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// Direct-DISPID Windows `IDispatch::Invoke` call.
///
/// Arguments are marshalled from retained `ComInvokeValue`/`Variant` payloads,
/// then the result is returned as retained [`Variant`] values.
///
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call.
/// The callback closures must uphold COM ownership and runtime identity guarantees for any object handles or returned interface pointers they touch.
pub unsafe fn invoke_direct_dispid_variant<
    FResolveObject,
    FQueryUnknown,
    FAddRefDispatch,
    FBindDispatch,
>(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    invoke_kind: crate::TypeLibMemberInvokeKind,
    requires_argument: bool,
    args: &[ComInvokeArg],
    prog_id_hint: &str,
    resolve_object: &mut FResolveObject,
    query_dispatch_from_unknown: &mut FQueryUnknown,
    add_ref_dispatch: &mut FAddRefDispatch,
    bind_dispatch_result: &mut FBindDispatch,
) -> Result<Variant, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
    FQueryUnknown: FnMut(*mut core::ffi::c_void) -> Result<*mut core::ffi::c_void, String>,
    FAddRefDispatch: FnMut(*mut core::ffi::c_void),
    FBindDispatch: FnMut(*mut core::ffi::c_void, &str, &'static str) -> Result<Variant, String>,
{
    if requires_argument && args.iter().all(|arg| arg.value.is_none()) {
        return Err(validation_failure(
            "dispatch_invoke",
            dispid,
            "member requires argument but DispatchInvoke omitted the third argument",
        ));
    }
    if !requires_argument {
        return match invoke_kind {
            crate::TypeLibMemberInvokeKind::PropertyGet => invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_PROPERTYGET,
                &[],
                &[],
                "property-get",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            ),
            crate::TypeLibMemberInvokeKind::Method => invoke_dispatch_variant(
                dispatch,
                dispid,
                DISPATCH_METHOD,
                &[],
                &[],
                "method",
                prog_id_hint,
                resolve_object,
                query_dispatch_from_unknown,
                add_ref_dispatch,
                bind_dispatch_result,
            ),
            crate::TypeLibMemberInvokeKind::PropertyPut
            | crate::TypeLibMemberInvokeKind::PropertyPutRef => Err(validation_failure(
                "dispatch_invoke",
                dispid,
                "member requires argument for property put/putref dispatch",
            )),
        };
    }
    if args.iter().any(|arg| arg.name.is_some()) {
        return Err(validation_failure(
            "dispatch_invoke",
            dispid,
            "named arguments require a resolved COM member name and are unsupported for direct-DISPID dispatch",
        ));
    }
    match invoke_kind {
        crate::TypeLibMemberInvokeKind::PropertyGet => invoke_dispatch_variant(
            dispatch,
            dispid,
            DISPATCH_PROPERTYGET,
            args,
            &[],
            "property-get",
            prog_id_hint,
            resolve_object,
            query_dispatch_from_unknown,
            add_ref_dispatch,
            bind_dispatch_result,
        ),
        crate::TypeLibMemberInvokeKind::Method => invoke_dispatch_variant(
            dispatch,
            dispid,
            DISPATCH_METHOD,
            args,
            &[],
            "method",
            prog_id_hint,
            resolve_object,
            query_dispatch_from_unknown,
            add_ref_dispatch,
            bind_dispatch_result,
        ),
        crate::TypeLibMemberInvokeKind::PropertyPut => invoke_dispatch_variant(
            dispatch,
            dispid,
            DISPATCH_PROPERTYPUT,
            args,
            &[crate::COM_DISPID_PROPERTYPUT],
            "property-put",
            prog_id_hint,
            resolve_object,
            query_dispatch_from_unknown,
            add_ref_dispatch,
            bind_dispatch_result,
        ),
        crate::TypeLibMemberInvokeKind::PropertyPutRef => invoke_dispatch_variant(
            dispatch,
            dispid,
            DISPATCH_PROPERTYPUTREF,
            args,
            &[crate::COM_DISPID_PROPERTYPUT],
            "property-putref",
            prog_id_hint,
            resolve_object,
            query_dispatch_from_unknown,
            add_ref_dispatch,
            bind_dispatch_result,
        ),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call, and
/// the callback closures must uphold COM ownership rules for resolved object arguments.
pub unsafe fn invoke_bound_dispatch_legacy_i32_result<
    FKnownSpec,
    FResolveNamedArgDispids,
    FResolveObject,
>(
    dispatch: *mut crate::RawIDispatch,
    member: crate::ComMemberToken,
    args: &[ComInvokeArg],
    known_member_spec: &mut FKnownSpec,
    resolve_named_arg_dispids: &mut FResolveNamedArgDispids,
    resolve_object: &mut FResolveObject,
) -> Result<i32, ComInvokeFailure>
where
    FKnownSpec: FnMut(crate::ComMemberToken) -> Result<Option<crate::ComMemberSpec>, String>,
    FResolveNamedArgDispids: FnMut(&str, &[ComInvokeArg]) -> Result<Vec<i32>, String>,
    FResolveObject: FnMut(ObjectRef) -> Result<*mut core::ffi::c_void, String>,
{
    if let Some(spec) = known_member_spec(member)
        .map_err(|detail| validation_failure("dispatch_invoke", member.raw(), detail))?
    {
        let dispid = crate::get_dispid_by_name(dispatch, &spec.name)
            .map_err(|detail| validation_failure("dispatch_invoke", member.raw(), detail))?;
        return invoke_member_spec_legacy_i32_result(
            dispatch.cast(),
            dispid,
            &spec,
            args,
            resolve_named_arg_dispids,
            resolve_object,
        );
    }
    if args.iter().any(|arg| arg.name.is_some()) {
        return Err(validation_failure(
            "dispatch_invoke",
            member.raw(),
            "named arguments require a resolved COM member name and remain unsupported for default-member/direct-DISPID dispatch",
        ));
    }
    invoke_dispatch_legacy_i32_result(
        dispatch.cast(),
        member.raw(),
        DISPATCH_PROPERTYGET,
        args,
        &[],
        "property-get",
        resolve_object,
    )
}

#[cfg(target_os = "windows")]
#[allow(clippy::too_many_arguments)]
pub fn execute_bound_variant<
    FTryVtable,
    FResolveMember,
    FInvokeMember,
    FInvokeDirect,
    FInvokeBound,
>(
    binding: &ComBinding,
    request: &ComInvokeRequest,
    cached_dispid: Option<i32>,
    try_vtable_invoke: &mut FTryVtable,
    resolve_member_dispid: &mut FResolveMember,
    invoke_member_spec: &mut FInvokeMember,
    invoke_direct_dispid: &mut FInvokeDirect,
    invoke_bound_dispatch: &mut FInvokeBound,
) -> Result<Variant, String>
where
    FTryVtable: FnMut(i32, &[i32]) -> Result<Option<i32>, String>,
    FResolveMember: FnMut(i32, Option<i32>) -> Result<Option<(i32, crate::ComMemberSpec)>, String>,
    FInvokeMember:
        FnMut(i32, &crate::ComMemberSpec, &[ComInvokeArg], &str) -> Result<Variant, String>,
    FInvokeDirect: FnMut(
        i32,
        crate::TypeLibMemberInvokeKind,
        bool,
        &[ComInvokeArg],
        &str,
    ) -> Result<Variant, String>,
    FInvokeBound: FnMut(i32, &[ComInvokeArg], &str) -> Result<Variant, String>,
{
    let plan = crate::plan_bound_runtime_invoke(binding, request, cached_dispid)?;
    let effective_member = plan.effective_member;
    let effective_cached_dispid = plan.effective_cached_dispid;
    let named_default_member_spec = plan.named_default_member_spec;
    let direct_dispatch_spec = plan.direct_dispatch_spec;
    let legacy_vtable_candidate_args = plan.legacy_vtable_candidate_args;
    let args = request.args.as_slice();

    if let Some(positional_values) = legacy_vtable_candidate_args.as_ref()
        && let Some(value) = try_vtable_invoke(effective_member.raw(), positional_values)?
    {
        return Ok(Variant::from_i32(value));
    }

    if let Some((token, spec)) = named_default_member_spec {
        let (dispid, spec) = resolve_member_dispid(token.raw(), effective_cached_dispid)?
            .map(|(dispid, _)| (dispid, spec))
            .ok_or_else(|| {
                "default member identity unavailable for named late-bound dispatch".to_string()
            })?;
        return invoke_member_spec(dispid, &spec, args, &binding.prog_id_name);
    }

    if let Some((dispid, spec)) =
        resolve_member_dispid(effective_member.raw(), effective_cached_dispid)?
    {
        return invoke_member_spec(dispid, &spec, args, &binding.prog_id_name);
    }

    if let Some(spec) = direct_dispatch_spec {
        return invoke_direct_dispid(
            effective_member.raw(),
            spec.invoke_kind,
            spec.requires_argument,
            args,
            &binding.prog_id_name,
        );
    }

    match request.invoke_kind_hint {
        Some(crate::ComInvokeKind::PropertyPut) => {
            return invoke_direct_dispid(
                effective_member.raw(),
                crate::TypeLibMemberInvokeKind::PropertyPut,
                true,
                args,
                &binding.prog_id_name,
            );
        }
        Some(crate::ComInvokeKind::PropertyPutRef) => {
            return invoke_direct_dispid(
                effective_member.raw(),
                crate::TypeLibMemberInvokeKind::PropertyPutRef,
                true,
                args,
                &binding.prog_id_name,
            );
        }
        _ => {}
    }

    invoke_bound_dispatch(effective_member.raw(), args, &binding.prog_id_name)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn, clippy::missing_safety_doc)]
pub unsafe fn invoke_member_spec_variant_with_shared_state(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    spec: &crate::ComMemberSpec,
    args: &[ComInvokeArg],
    prog_id_hint: &str,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<Variant, ComInvokeFailure> {
    invoke_member_spec_variant(
        dispatch,
        dispid,
        spec,
        args,
        prog_id_hint,
        &mut |member_name, args| {
            crate::resolve_named_argument_dispids(
                dispatch.cast::<crate::RawIDispatch>(),
                member_name,
                args,
            )
        },
        &mut |handle| {
            crate::resolve_bound_native_dispatch_shared(com_state, handle)
                .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
        },
        &mut |unknown: *mut core::ffi::c_void| {
            crate::query_dispatch_from_unknown(unknown.cast::<crate::RawIUnknown>())
                .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
        },
        &mut |dispatch: *mut core::ffi::c_void| {
            crate::add_ref_dispatch(dispatch.cast::<crate::RawIDispatch>());
        },
        // SAFETY: take_variant_result_variant hands this callback a pointer
        // that is null or carries the one reference retained via
        // add_ref_dispatch before the result VARIANT was cleared; ownership of
        // that reference transfers to the bindings map, satisfying
        // bind_native_runtime_object_result_shared's contract.
        &mut |dispatch: *mut core::ffi::c_void, prog_id_hint: &str, _op: &'static str| unsafe {
            crate::windows_runtime_state::bind_native_runtime_object_result_shared(
                com_state,
                dispatch.cast::<crate::RawIDispatch>(),
                prog_id_hint,
            )
            .map(Variant::from_object_ref)
        },
    )
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn, clippy::missing_safety_doc)]
pub unsafe fn invoke_direct_dispid_variant_with_shared_state(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    invoke_kind: crate::TypeLibMemberInvokeKind,
    requires_argument: bool,
    args: &[ComInvokeArg],
    prog_id_hint: &str,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<Variant, ComInvokeFailure> {
    invoke_direct_dispid_variant(
        dispatch,
        dispid,
        invoke_kind,
        requires_argument,
        args,
        prog_id_hint,
        &mut |handle| {
            crate::resolve_bound_native_dispatch_shared(com_state, handle)
                .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
        },
        &mut |unknown: *mut core::ffi::c_void| {
            crate::query_dispatch_from_unknown(unknown.cast::<crate::RawIUnknown>())
                .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
        },
        &mut |dispatch: *mut core::ffi::c_void| {
            crate::add_ref_dispatch(dispatch.cast::<crate::RawIDispatch>());
        },
        // SAFETY: take_variant_result_variant hands this callback a pointer
        // that is null or carries the one reference retained via
        // add_ref_dispatch before the result VARIANT was cleared; ownership of
        // that reference transfers to the bindings map, satisfying
        // bind_native_runtime_object_result_shared's contract.
        &mut |dispatch: *mut core::ffi::c_void, prog_id_hint: &str, _op: &'static str| unsafe {
            crate::windows_runtime_state::bind_native_runtime_object_result_shared(
                com_state,
                dispatch.cast::<crate::RawIDispatch>(),
                prog_id_hint,
            )
            .map(Variant::from_object_ref)
        },
    )
}

#[cfg(target_os = "windows")]
#[allow(
    unsafe_op_in_unsafe_fn,
    clippy::missing_safety_doc,
    clippy::too_many_arguments
)]
pub unsafe fn invoke_dispatch_variant_with_shared_state(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    flags: u16,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    label: &'static str,
    prog_id_hint: &str,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<Variant, ComInvokeFailure> {
    invoke_dispatch_variant(
        dispatch,
        dispid,
        flags,
        args,
        named_arg_dispids,
        label,
        prog_id_hint,
        &mut |handle| {
            crate::resolve_bound_native_dispatch_shared(com_state, handle)
                .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
        },
        &mut |unknown: *mut core::ffi::c_void| {
            crate::query_dispatch_from_unknown(unknown.cast::<crate::RawIUnknown>())
                .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
        },
        &mut |dispatch: *mut core::ffi::c_void| {
            crate::add_ref_dispatch(dispatch.cast::<crate::RawIDispatch>());
        },
        &mut |dispatch: *mut core::ffi::c_void, prog_id_hint: &str, _op: &'static str| {
            crate::windows_runtime_state::bind_native_runtime_object_result_shared(
                com_state,
                dispatch.cast::<crate::RawIDispatch>(),
                prog_id_hint,
            )
            .map(Variant::from_object_ref)
        },
    )
}

#[cfg(target_os = "windows")]
#[allow(clippy::missing_safety_doc)]
pub unsafe fn invoke_bound_dispatch_variant_with_shared_state<FKnownSpec>(
    dispatch: *mut crate::RawIDispatch,
    prog_id: &str,
    member: crate::ComMemberToken,
    args: &[ComInvokeArg],
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
    known_member_spec: &mut FKnownSpec,
) -> Result<Variant, String>
where
    FKnownSpec:
        FnMut(&ComBinding, crate::ComMemberToken) -> Result<Option<crate::ComMemberSpec>, String>,
{
    let plan = crate::plan_unbound_runtime_invoke(
        member,
        args,
        known_member_spec(
            &ComBinding::new(prog_id.to_string(), dispatch as usize),
            member,
        )?,
    )?;
    match plan {
        crate::UnboundRuntimeInvokePlan::MemberSpec(spec) => {
            // SAFETY: this unsafe fn's contract requires `dispatch` to be a
            // live IDispatch for the duration of the call; the in-module
            // caller passes the pointer recovered from a live binding entry,
            // which the bindings map keeps retained.
            let dispid = unsafe { crate::get_dispid_by_name(dispatch, &spec.name) }?;
            // SAFETY: same live, bindings-map-retained `dispatch` as the
            // dispid lookup above; the shared-state helper installs callbacks
            // that uphold COM retention rules for arguments and results.
            unsafe {
                invoke_member_spec_variant_with_shared_state(
                    dispatch.cast(),
                    dispid,
                    &spec,
                    args,
                    prog_id,
                    com_state,
                )
            }
            .map_err(|failure| render_invoke_fault_message(&failure))
        }
        // SAFETY: this unsafe fn's contract requires `dispatch` to be a live
        // IDispatch (the in-module caller passes the bindings-map-retained
        // pointer); the shared-state helper installs callbacks that uphold COM
        // retention rules for arguments and results.
        crate::UnboundRuntimeInvokePlan::DirectGetOrCall { dispid } => unsafe {
            // Invoke kind is unknown for this trusted dispid, so let the server choose
            // between a method call and a property read. DAO/Jet strictly reject a method
            // dispatched as a bare property-get; the combined flag is what real Automation
            // clients (incl. VBA) issue when the call could be either.
            invoke_dispatch_variant_with_shared_state(
                dispatch.cast(),
                dispid.raw(),
                DISPATCH_METHOD | DISPATCH_PROPERTYGET,
                args,
                &[],
                "get-or-call",
                prog_id,
                com_state,
            )
        }
        .map_err(|failure| render_invoke_fault_message(&failure)),
    }
}

/// Whether a FUNCDESC parameter/return VARTYPE is in the v1 vtable marshalling
/// set (the exact shapes [`crate::windows_vtable::vtable_invoke`] marshals). Any
/// VARTYPE outside this set — `Decimal`, `LongPtr`, every `ByRef*`, SAFEARRAY
/// (which surfaces as a `Variant` array intent the marshaller rejects) — gates
/// the call to the IDispatch fallback rather than risking a wrong-ABI vtable
/// call. Mirrors the marshaller's own supported-shape match so the gate and the
/// marshaller never disagree.
#[cfg(target_os = "windows")]
pub fn is_v1_vtable_vartype(param_type: crate::TypeLibParamType) -> bool {
    use crate::TypeLibParamType as P;
    matches!(
        param_type,
        P::Variant
            | P::Long
            | P::Integer
            | P::String
            | P::Boolean
            | P::Double
            | P::Single
            | P::Currency
            | P::Date
            | P::Object
            | P::Byte
            | P::LongLong
    )
}

/// Per-bridge transport counters incremented at the exact return sites of a
/// successful early-bound member call: a vtable success bumps `vtable`, an
/// IDispatch `Invoke` success bumps `idispatch`. A host test reads these to
/// prove which transport carried a given member (the LAST call in a live script
/// — e.g. `app.Quit` — is not the asserted member, so a counter, not a
/// last-transport snapshot, is what lets the assertion target a specific call).
#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
pub struct ComTransportCounters<'a> {
    pub vtable: &'a std::sync::atomic::AtomicU64,
    pub idispatch: &'a std::sync::atomic::AtomicU64,
}

#[cfg(target_os = "windows")]
impl ComTransportCounters<'_> {
    fn record_vtable(&self) {
        self.vtable
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn record_idispatch(&self) {
        self.idispatch
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The GATE: take the vtable path iff `prefer_vtable` AND the resolved member
/// carries a full, callable, v1-marshallable dual signature. Returns `true` only
/// when ALL hold:
/// - a vtable slot is present, and the slot index is `>= 7` (oVft `>= 56`), so we
///   never call an `IUnknown`/`IDispatch` slot;
/// - the member is a real custom **interface** dual: `is_dual` AND
///   `source_typekind == Interface`. A pure dispinterface member (`Dispatch`) has
///   NO callable vtable slot — its `oVft` is authored for the FDUAL partner, so
///   the live-recovery path must cross to that partner (slice B) before a slot
///   can be admitted here;
/// - `slot < vtable_slot_bound` — THE AV-SAFETY NET. The bound is the source
///   INTERFACE's `cbSizeVft / 8` (from the partner typeinfo, not the
///   dispinterface). A slot `>= bound` would over-read the live vtable, which is
///   the access violation the value-oracle probe root-caused (Recordset.Close at
///   the wrong slot 98 over-ran the 92-slot vtable). Without a known bound we
///   decline;
/// - the member carries its defining **dual interface IID** (S5a), which the
///   dispatch site `QueryInterface`s the object for before any slot call — without
///   it we cannot obtain a verified vtable pointer, so we must not vtable-call;
/// - the FUNCDESC declares `CC_STDCALL`;
/// - EXACT arity: exactly one declared parameter type per supplied positional
///   arg. The vtable ABI cannot drop a trailing optional param (no DISPPARAMS to
///   shorten), so a member called with fewer args than its FUNCDESC declares
///   (e.g. an omitted optional) falls back to IDispatch (workset v1 deferral);
/// - every parameter VARTYPE and the return VARTYPE (if any) is in the v1 set. A
///   `None` return (a void method / HRESULT-only put) is fine — the marshaller
///   simply appends no `[out,retval]` cell.
///
/// When this returns `false` the caller runs the unchanged IDispatch path.
#[cfg(target_os = "windows")]
fn vtable_gate_admits(
    spec: &crate::ComMemberSpec,
    positional_arg_count: usize,
    return_type: Option<crate::TypeLibParamType>,
) -> bool {
    let Some(slot) = spec.vtable_slot else {
        return false;
    };
    // Never vtable-call an IUnknown (0..=2) or IDispatch (3..=6) slot.
    if slot < 7 {
        return false;
    }
    // A vtable slot is only callable when sourced from a real custom INTERFACE
    // (FDUAL + TKIND_INTERFACE). A pure dispinterface member must NOT be slot-called.
    if !spec.is_dual || spec.source_typekind != Some(crate::SourceTypeKind::Interface) {
        return false;
    }
    // AV-SAFETY NET: the slot must be in bounds of the source INTERFACE's live
    // vtable (cbSizeVft/8). A missing bound or an out-of-range slot declines —
    // this is the guard that prevents the host access violation.
    match spec.vtable_slot_bound {
        Some(bound) if slot < bound => {}
        _ => return false,
    }
    // S5a HOST-AV SAFETY: a usable dual interface IID is mandatory. We only ever
    // vtable-call after a SUCCESSFUL QueryInterface for this exact IID, so without
    // one (or with a null IID) we cannot prove the pointer's vtable layout and
    // must fall back to IDispatch.
    match spec.interface_iid {
        Some(iid) if !iid.is_null() => {}
        _ => return false,
    }
    if !spec.callconv_is_stdcall {
        return false;
    }
    // Exact arity: one declared parameter type per supplied positional arg. (A
    // property-put's trailing value arg is one of these.)
    if spec.parameter_types.len() != positional_arg_count {
        return false;
    }
    if spec
        .parameter_types
        .iter()
        .any(|p| !is_v1_vtable_vartype(*p))
    {
        return false;
    }
    if let Some(rt) = return_type
        && !is_v1_vtable_vartype(rt)
    {
        return false;
    }
    true
}

/// `IProxyManager` — `{00000008-0000-0000-C000-000000000046}`. EVERY COM
/// marshaling proxy (out-of-process / cross-apartment) implements it; a direct
/// in-process object does not. A successful `QueryInterface` for it ⇒ the pointer
/// we hold is a PROXY, not a direct interface, so its vtable is the marshaling
/// stub's — a typelib `oVft` slot does NOT index it and a slot call would
/// access-violate. The probe proved DAO (in-process) FAILS this QI while
/// out-of-process Excel SUCCEEDS it; that is the in/out-of-process discriminator.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const IID_IPROXYMANAGER: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0x0000_0008,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

/// True when `object` is a COM marshaling proxy (it answers `QueryInterface` for
/// `IID_IProxyManager`). Releases the probe reference. AV-free: a pure QI.
///
/// # Safety
/// `object` must be a live COM interface pointer for the duration of the call.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
unsafe fn dispatch_is_marshaling_proxy(object: *mut core::ffi::c_void) -> bool {
    // SAFETY: `object` is a live interface pointer per this fn's contract;
    // query_interface_pointer reads its IUnknown vtable and, on success, hands
    // back one owned reference we Release immediately.
    match unsafe { crate::query_interface_pointer(object, &IID_IPROXYMANAGER) } {
        Ok(proxy) => {
            // SAFETY: `proxy` is the single reference the QI handed us; Release it
            // exactly once (we only needed its existence as the proxy signal).
            unsafe { crate::release_unknown(proxy) };
            true
        }
        Err(_) => false,
    }
}

/// Attempt an early-bound member call through the COM vtable, with a clean
/// fall-back vs propagate distinction.
///
/// Returns:
/// - `Ok(Some(value))` — the vtable call ran and succeeded; the caller must NOT
///   also run the IDispatch path (and should record the vtable transport).
/// - `Ok(None)` — the member is ineligible (gate failed) OR the marshaller
///   reported an unsupported shape (a validation failure, `hr == None`); the
///   caller falls back to the unchanged IDispatch path.
/// - `Err(failure)` — the vtable call genuinely failed with a real COM HRESULT
///   (`hr == Some(hr < 0)`); the caller PROPAGATES it (no silent fallback that
///   would mask the failure).
///
/// # Safety
/// `dispatch` must be a live dual-interface pointer for the bound object (the
/// bindings map holds one retained reference, keeping it live for this call).
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
#[allow(clippy::result_large_err)]
pub unsafe fn try_vtable_member_spec_invoke_with_shared_state(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    spec: &crate::ComMemberSpec,
    args: &[ComInvokeArg],
    prefer_vtable: bool,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<Option<Variant>, ComInvokeFailure> {
    if !prefer_vtable {
        return Ok(None);
    }
    // The vtable carries left-to-right positional params only; any named or
    // omitted argument is a shape the IDispatch path owns.
    if args
        .iter()
        .any(|arg| arg.name.is_some() || arg.value.is_none())
    {
        return Ok(None);
    }
    let (label, return_type) = match spec.invoke_kind {
        // A property-get / method returns whatever its FUNCDESC declares — a value
        // (`Some`) or nothing (`None`, a void method like `Quit`).
        crate::TypeLibMemberInvokeKind::PropertyGet => ("property-get", spec.return_type),
        crate::TypeLibMemberInvokeKind::Method => ("method", spec.return_type),
        // A property-put's HRESULT-only member returns no value; the trailing
        // value argument is an ordinary positional [in] param to the vtable slot.
        crate::TypeLibMemberInvokeKind::PropertyPut => ("property-put", None),
        // PropertyPutRef (Set p = obj) is deferred to the IDispatch path in v1.
        crate::TypeLibMemberInvokeKind::PropertyPutRef => return Ok(None),
    };
    if !vtable_gate_admits(spec, args.len(), return_type) {
        return Ok(None);
    }
    // Marshal the positional args to `Variant` (the vtable marshaller's input).
    let variant_args: Vec<Variant> = args
        .iter()
        .filter_map(|arg| arg.value.as_ref().map(|v| v.variant().clone()))
        .collect();
    if variant_args.len() != args.len() {
        // A value went missing between the omitted-check and here; be safe.
        return Ok(None);
    }

    let mut resolve_object = |handle: ObjectRef| {
        crate::resolve_bound_native_dispatch_shared(com_state, handle)
            .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
    };
    // The vtable [out,retval] interface convention transfers one reference to
    // us; hand it to the bindings map, exactly as the IDispatch result path does.
    let mut bind_dispatch_result = |dispatch: *mut core::ffi::c_void| {
        // SAFETY: a non-null pointer here carries the one reference the callee
        // AddRef'd for the [out,retval]; ownership transfers to the bindings map,
        // satisfying bind_native_runtime_object_result_shared's contract.
        unsafe {
            crate::windows_runtime_state::bind_native_runtime_object_result_shared(
                com_state,
                dispatch.cast::<crate::RawIDispatch>(),
                &spec.name,
            )
        }
        .map(Variant::from_object_ref)
    };

    let slot = spec.vtable_slot.expect("gate guaranteed a slot");

    // IN/OUT-OF-PROCESS DISCRIMINATOR (workset slice E). The proven recipe runs
    // full in-process vtable dispatch but must EXCLUDE marshaling proxies, whose
    // vtable is the universal-marshaler stub (a typelib `oVft` slot does not index
    // it). The probe proved DAO is in-process (IID_IProxyManager QI FAILS) while
    // out-of-process Excel IS a proxy (QI SUCCEEDS) → IDispatch fallback. This is
    // why Excel stays IDispatch and DAO goes through the vtable.
    // SAFETY: `dispatch` is the live, bindings-map-retained interface pointer.
    if unsafe { dispatch_is_marshaling_proxy(dispatch) } {
        return Ok(None);
    }

    // QueryInterface the object for the member's typelib-declared DUAL interface
    // IID (the gate proved it is present and non-null) — this is how the VBA IDE
    // holds an early-bound reference. A non-aliasing in-process tear-off is now
    // ACCEPTED (the old `ptr::eq(dispatch, interface)` aliasing-only restriction is
    // GONE): the bound check below makes a bound-validated slot on a direct
    // in-process interface safe and correct. If the QI fails (E_NOINTERFACE /
    // null) we fall back to IDispatch; we NEVER call a slot on an unverified
    // pointer (no host AV).
    let iid = spec
        .interface_iid
        .expect("gate guaranteed an interface IID")
        .to_guid();
    // SAFETY: `dispatch` is the live, bindings-map-retained interface pointer for
    // this bound object; QueryInterface reads its IUnknown vtable and, on success,
    // hands back one fresh reference we own (Released below on every path).
    let interface = match unsafe { crate::query_interface_pointer(dispatch, &iid) } {
        Ok(interface) => interface,
        // E_NOINTERFACE or any failing QI: this object does not expose the dual
        // interface in a vtable-callable form here — fall back to IDispatch.
        Err(_) => return Ok(None),
    };

    // AV-SAFETY NET (re-asserted at the dispatch site, not just the gate): the slot
    // MUST be inside the source INTERFACE's live vtable (cbSizeVft/8). The gate
    // already checked this, but we re-verify here so a slot call can never over-run
    // the live vtable — the access violation the probe root-caused. Without a known
    // bound, or with an out-of-range slot, fall back to IDispatch.
    match spec.vtable_slot_bound {
        Some(bound) if slot < bound => {}
        _ => {
            // SAFETY: Release the QI'd reference we own, then fall back.
            unsafe { crate::release_unknown(interface) };
            return Ok(None);
        }
    }

    // SAFETY: `interface` is the QI'd dual-interface pointer carrying one reference
    // we own, on a DIRECT in-process object (not a marshaling proxy — excluded
    // above), so its vtable is the real custom-slot vtable we can index directly.
    // The gate + the bound re-check above proved `7 <= slot < cbSizeVft/8` with a
    // CC_STDCALL, fully-typed, v1-marshallable signature, so the slot's ABI is
    // `HRESULT slot(this, params…, retval*)` — exactly vtable_invoke's contract.
    let result = unsafe {
        crate::windows_vtable::vtable_invoke(
            interface,
            slot,
            &spec.parameter_types,
            return_type,
            spec.invoke_kind,
            &variant_args,
            label,
            dispid,
            &mut resolve_object,
            &mut bind_dispatch_result,
        )
    };

    // Release the QI'd interface reference on every path (success, COM error, and
    // unsupported-shape) — QI added one ref; we own and drop exactly that one.
    // SAFETY: `interface` is the single reference `query_interface_pointer` handed
    // us; we Release it exactly once here and never use it afterward.
    unsafe {
        crate::release_unknown(interface);
    }

    match result {
        Ok(value) => Ok(Some(value)),
        // A real COM error (the call ran and the server returned hr < 0):
        // PROPAGATE — never silently fall back and mask a genuine failure.
        Err(failure) if failure.hr.is_some() => Err(failure),
        // An unsupported-shape / validation signal (hr == None, detail set): the
        // marshaller declined this call. Fall back to the IDispatch path.
        Err(_unsupported) => Ok(None),
    }
}

/// Non-x64 Windows stub: the libffi this-call vtable marshaller is x64-only
/// (`windows_vtable` is gated to `target_arch = "x86_64"`), so every member
/// falls back to the IDispatch path here.
#[cfg(all(target_os = "windows", not(target_arch = "x86_64")))]
#[allow(clippy::result_large_err)]
pub unsafe fn try_vtable_member_spec_invoke_with_shared_state(
    _dispatch: *mut core::ffi::c_void,
    _dispid: i32,
    _spec: &crate::ComMemberSpec,
    _args: &[ComInvokeArg],
    _prefer_vtable: bool,
    _com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<Option<Variant>, ComInvokeFailure> {
    Ok(None)
}

#[cfg(target_os = "windows")]
#[allow(clippy::too_many_arguments, clippy::missing_safety_doc)]
pub unsafe fn execute_bound_variant_with_shared_state<FTryVtable, FKnownSpec>(
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
    request: &ComInvokeRequest,
    prefer_vtable: bool,
    transport: ComTransportCounters<'_>,
    try_vtable_invoke: &mut FTryVtable,
    known_member_spec: &mut FKnownSpec,
) -> Result<Option<Variant>, String>
where
    FTryVtable:
        FnMut(*mut crate::RawIDispatch, &ComBinding, i32, &[i32]) -> Result<Option<i32>, String>,
    FKnownSpec:
        FnMut(&ComBinding, crate::ComMemberToken) -> Result<Option<crate::ComMemberSpec>, String>,
{
    let (binding, cached_dispid) = {
        let state = com_state.lock().map_err(|_| {
            "COM-E-STATE-LOCK-POISONED: dispatch_invoke state lock poisoned".to_string()
        })?;
        let binding = state
            .bindings
            .get(&crate::ComObjectToken::new(request.object.raw()))
            .cloned();
        let cached_dispid = binding
            .as_ref()
            .and_then(|entry| entry.member_dispids.get(&request.member).copied());
        (binding, cached_dispid)
    };
    let Some(binding) = binding else {
        return Ok(None);
    };
    if binding.native_dispatch == 0 {
        return Ok(None);
    }
    let dispatch = binding.native_dispatch as *mut crate::RawIDispatch;
    let mut resolve_member_dispid = |member: i32, _cached_dispid: Option<i32>| {
        let mut state = com_state.lock().map_err(|_| {
            "COM-E-STATE-LOCK-POISONED: dispatch_invoke state lock poisoned".to_string()
        })?;
        // SAFETY: `dispatch` was recovered from a live bindings-map entry
        // (checked non-zero above); the bindings map owns one retained
        // IDispatch reference, keeping the pointer live for this lookup.
        unsafe {
            crate::resolve_member_dispid_cached(
                &mut state,
                dispatch,
                request.object.clone(),
                &binding,
                crate::ComMemberToken::new(member),
                None,
            )
        }
    };
    let mut invoke_member_spec =
        |dispid: i32, spec: &crate::ComMemberSpec, invoke_args: &[ComInvokeArg], prog_id: &str| {
            // Early-bound vtable fast path: when the policy prefers it AND the
            // resolved member carries a full, callable, v1-marshallable dual
            // signature, dispatch through the COM vtable slot directly. An
            // ineligible member or an unsupported shape returns Ok(None) → the
            // unchanged IDispatch path below; a real COM error (hr < 0) is
            // propagated, never silently swallowed.
            // SAFETY: `dispatch` is the live bindings-map-retained dual interface
            // for this bound object (checked non-zero above), exactly what the
            // vtable attempt's `# Safety` requires.
            match unsafe {
                try_vtable_member_spec_invoke_with_shared_state(
                    dispatch.cast(),
                    dispid,
                    spec,
                    invoke_args,
                    prefer_vtable,
                    com_state,
                )
            } {
                Ok(Some(value)) => {
                    transport.record_vtable();
                    return Ok(value);
                }
                Ok(None) => {}
                Err(failure) => {
                    return Err(render_invoke_fault_message(&failure));
                }
            }
            // SAFETY: `dispatch` was recovered from a live bindings-map entry
            // whose retained IDispatch reference outlives this call (the
            // invoking runtime holds the object); the shared-state helper
            // installs callbacks that uphold COM retention rules.
            let value = unsafe {
                invoke_member_spec_variant_with_shared_state(
                    dispatch.cast(),
                    dispid,
                    spec,
                    invoke_args,
                    prog_id,
                    com_state,
                )
            }
            .map_err(|failure| render_invoke_fault_message(&failure))?;
            transport.record_idispatch();
            Ok(value)
        };
    let mut invoke_direct_dispid = |member: i32,
                                    invoke_kind: crate::TypeLibMemberInvokeKind,
                                    requires_argument: bool,
                                    invoke_args: &[ComInvokeArg],
                                    prog_id: &str| {
        // SAFETY: `dispatch` was recovered from a live bindings-map entry
        // whose retained IDispatch reference outlives this call (the invoking
        // runtime holds the object); the shared-state helper installs
        // callbacks that uphold COM retention rules.
        unsafe {
            invoke_direct_dispid_variant_with_shared_state(
                dispatch.cast(),
                member,
                invoke_kind,
                requires_argument,
                invoke_args,
                prog_id,
                com_state,
            )
        }
        .map_err(|failure| render_invoke_fault_message(&failure))
    };
    // SAFETY: `dispatch` was recovered from a live bindings-map entry whose
    // retained IDispatch reference outlives this call (the invoking runtime
    // holds the object), as required by the callee's contract.
    let mut invoke_bound_dispatch = |member: i32, invoke_args: &[ComInvokeArg], prog_id: &str| unsafe {
        invoke_bound_dispatch_variant_with_shared_state(
            dispatch,
            prog_id,
            crate::ComMemberToken::new(member),
            invoke_args,
            com_state,
            known_member_spec,
        )
    };
    let mut try_vtable =
        |member: i32, positional: &[i32]| try_vtable_invoke(dispatch, &binding, member, positional);
    let value = execute_bound_variant(
        &binding,
        request,
        cached_dispid,
        &mut try_vtable,
        &mut resolve_member_dispid,
        &mut invoke_member_spec,
        &mut invoke_direct_dispid,
        &mut invoke_bound_dispatch,
    )?;
    let _ = crate::windows_runtime_state::queue_projection_event_callbacks_shared(
        com_state,
        request.object.clone(),
        &binding,
        request.member,
        request.args.as_slice(),
    )?;
    Ok(Some(value))
}

#[cfg(all(target_os = "windows", test))]
mod gate_tests {
    use super::vtable_gate_admits;
    use crate::{ComInterfaceIid, ComMemberSpec, SourceTypeKind, TypeLibMemberInvokeKind};

    /// A vtable-eligible spec: a real custom INTERFACE dual, CC_STDCALL, a slot
    /// at `slot` inside `bound`, a non-null IID, and a no-arg `Long` getter. The
    /// individual rejection tests then flip ONE field to prove the gate declines.
    fn eligible_spec(slot: u16, bound: u16) -> ComMemberSpec {
        ComMemberSpec {
            name: "Value".to_string(),
            requires_argument: false,
            invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
            parameter_names: Vec::new(),
            is_default_member: false,
            vtable_slot: Some(slot),
            parameter_types: Vec::new(),
            return_type: Some(crate::TypeLibParamType::Long),
            callconv_is_stdcall: true,
            interface_iid: Some(ComInterfaceIid {
                data1: 0x1234_5678,
                data2: 0x9abc,
                data3: 0xdef0,
                data4: [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80],
            }),
            is_dual: true,
            source_typekind: Some(SourceTypeKind::Interface),
            vtable_slot_bound: Some(bound),
        }
    }

    #[test]
    fn gate_admits_interface_dual_slot_in_bound() {
        // DAO Field.Value: slot 17, partner cbSizeVft=464 → bound 58. ACCEPT.
        let spec = eligible_spec(17, 58);
        assert!(vtable_gate_admits(
            &spec,
            0,
            Some(crate::TypeLibParamType::Long)
        ));
    }

    #[test]
    fn gate_rejects_dispinterface_member() {
        // A pure dispinterface member (source_typekind == Dispatch) has no
        // callable vtable slot, even with a slot+IID present. REJECT.
        let mut spec = eligible_spec(17, 58);
        spec.source_typekind = Some(SourceTypeKind::Dispatch);
        assert!(!vtable_gate_admits(
            &spec,
            0,
            Some(crate::TypeLibParamType::Long)
        ));
        // Likewise a non-FDUAL member.
        let mut not_dual = eligible_spec(17, 58);
        not_dual.is_dual = false;
        assert!(!vtable_gate_admits(
            &not_dual,
            0,
            Some(crate::TypeLibParamType::Long)
        ));
    }

    #[test]
    fn gate_rejects_slot_at_or_past_bound() {
        // THE AV-SAFETY NET: slot 98 against a 92-slot bound (the Recordset.Close
        // over-run the probe root-caused) must REJECT.
        let over = eligible_spec(98, 92);
        assert!(!vtable_gate_admits(
            &over,
            0,
            Some(crate::TypeLibParamType::Long)
        ));
        // Slot exactly == bound is also out of range (valid indices are 0..bound).
        let at_bound = eligible_spec(58, 58);
        assert!(!vtable_gate_admits(
            &at_bound,
            0,
            Some(crate::TypeLibParamType::Long)
        ));
        // A missing bound declines too — we never slot-call without a known bound.
        let mut no_bound = eligible_spec(17, 58);
        no_bound.vtable_slot_bound = None;
        assert!(!vtable_gate_admits(
            &no_bound,
            0,
            Some(crate::TypeLibParamType::Long)
        ));
    }
}
