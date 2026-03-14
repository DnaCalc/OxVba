use crate::{
    ComBinding, ComInvokeArg, ComInvokeRequest, VariantResultValue, set_variant_from_com_value,
    take_variant_result_value,
};
use oxvba_runtime::{ObjectHandle, RuntimeValue};
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

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(clippy::too_many_arguments)]
/// Execute a Windows `IDispatch::Invoke` call over the shared semantic COM request carrier and
/// return the final OxVba runtime-facing value shape, delegating any dispatch-backed result
/// rebinding to the caller.
///
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call.
/// The callback closures must uphold COM ownership and runtime identity guarantees for any object
/// handles or returned interface pointers they touch.
pub unsafe fn invoke_dispatch_runtime_value<
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
) -> Result<RuntimeValue, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectHandle) -> Result<*mut core::ffi::c_void, String>,
    FQueryUnknown: FnMut(*mut core::ffi::c_void) -> Result<*mut core::ffi::c_void, String>,
    FAddRefDispatch: FnMut(*mut core::ffi::c_void),
    FBindDispatch:
        FnMut(*mut core::ffi::c_void, &str, &'static str) -> Result<RuntimeValue, String>,
{
    match invoke_dispatch_variant_result(
        dispatch,
        dispid,
        flags,
        args,
        named_arg_dispids,
        label,
        resolve_object,
        query_dispatch_from_unknown,
        add_ref_dispatch,
    )? {
        VariantResultValue::Value(value) => Ok(value.to_runtime_value()),
        VariantResultValue::Dispatch(dispatch) => {
            bind_dispatch_result(dispatch, prog_id_hint, "dispatch_invoke").map_err(|detail| {
                ComInvokeFailure {
                    label,
                    dispid,
                    hr: None,
                    arg_err: None,
                    excep: None,
                    detail: Some(detail),
                }
            })
        }
    }
}

#[cfg(target_os = "windows")]
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
/// Execute a member-metadata-backed Windows `IDispatch::Invoke` call over the shared semantic carrier.
///
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call.
/// The callback closures must uphold COM ownership and runtime identity guarantees for any object handles or returned interface pointers they touch.
pub unsafe fn invoke_member_spec_runtime_value<
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
) -> Result<RuntimeValue, ComInvokeFailure>
where
    FResolveNamedArgDispids: FnMut(&str, &[ComInvokeArg]) -> Result<Vec<i32>, String>,
    FResolveObject: FnMut(ObjectHandle) -> Result<*mut core::ffi::c_void, String>,
    FQueryUnknown: FnMut(*mut core::ffi::c_void) -> Result<*mut core::ffi::c_void, String>,
    FAddRefDispatch: FnMut(*mut core::ffi::c_void),
    FBindDispatch:
        FnMut(*mut core::ffi::c_void, &str, &'static str) -> Result<RuntimeValue, String>,
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
            crate::TypeLibMemberInvokeKind::PropertyGet => invoke_dispatch_runtime_value(
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
            crate::TypeLibMemberInvokeKind::Method => invoke_dispatch_runtime_value(
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
            invoke_dispatch_runtime_value(
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
            invoke_dispatch_runtime_value(
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
            invoke_dispatch_runtime_value(
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
            invoke_dispatch_runtime_value(
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
/// Execute a direct-DISPID Windows `IDispatch::Invoke` call over the shared semantic carrier.
///
/// # Safety
/// `dispatch` must point to a live `IDispatch` implementation for the duration of the call.
/// The callback closures must uphold COM ownership and runtime identity guarantees for any object handles or returned interface pointers they touch.
pub unsafe fn invoke_direct_dispid_runtime_value<
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
) -> Result<RuntimeValue, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectHandle) -> Result<*mut core::ffi::c_void, String>,
    FQueryUnknown: FnMut(*mut core::ffi::c_void) -> Result<*mut core::ffi::c_void, String>,
    FAddRefDispatch: FnMut(*mut core::ffi::c_void),
    FBindDispatch:
        FnMut(*mut core::ffi::c_void, &str, &'static str) -> Result<RuntimeValue, String>,
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
            crate::TypeLibMemberInvokeKind::PropertyGet => invoke_dispatch_runtime_value(
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
            crate::TypeLibMemberInvokeKind::Method => invoke_dispatch_runtime_value(
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
        crate::TypeLibMemberInvokeKind::PropertyGet => invoke_dispatch_runtime_value(
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
        crate::TypeLibMemberInvokeKind::Method => invoke_dispatch_runtime_value(
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
        crate::TypeLibMemberInvokeKind::PropertyPut => invoke_dispatch_runtime_value(
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
        crate::TypeLibMemberInvokeKind::PropertyPutRef => invoke_dispatch_runtime_value(
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
#[allow(clippy::too_many_arguments)]
pub fn execute_bound_runtime_value<
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
) -> Result<RuntimeValue, String>
where
    FTryVtable: FnMut(i32, &[i32]) -> Result<Option<i32>, String>,
    FResolveMember: FnMut(i32, Option<i32>) -> Result<Option<(i32, crate::ComMemberSpec)>, String>,
    FInvokeMember:
        FnMut(i32, &crate::ComMemberSpec, &[ComInvokeArg], &str) -> Result<RuntimeValue, String>,
    FInvokeDirect: FnMut(
        i32,
        crate::TypeLibMemberInvokeKind,
        bool,
        &[ComInvokeArg],
        &str,
    ) -> Result<RuntimeValue, String>,
    FInvokeBound: FnMut(i32, &[ComInvokeArg], &str) -> Result<RuntimeValue, String>,
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
        return Ok(RuntimeValue::I32(value));
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

    invoke_bound_dispatch(effective_member.raw(), args, &binding.prog_id_name)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn, clippy::missing_safety_doc)]
pub unsafe fn invoke_member_spec_runtime_value_with_shared_state(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    spec: &crate::ComMemberSpec,
    args: &[ComInvokeArg],
    prog_id_hint: &str,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<RuntimeValue, ComInvokeFailure> {
    invoke_member_spec_runtime_value(
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
        &mut |dispatch: *mut core::ffi::c_void, prog_id_hint: &str, _op: &'static str| unsafe {
            crate::bind_native_dispatch_result_shared(
                com_state,
                dispatch.cast::<crate::RawIDispatch>(),
                prog_id_hint,
            )
            .map(RuntimeValue::ObjectHandle)
        },
    )
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn, clippy::missing_safety_doc)]
pub unsafe fn invoke_direct_dispid_runtime_value_with_shared_state(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    invoke_kind: crate::TypeLibMemberInvokeKind,
    requires_argument: bool,
    args: &[ComInvokeArg],
    prog_id_hint: &str,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<RuntimeValue, ComInvokeFailure> {
    invoke_direct_dispid_runtime_value(
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
        &mut |dispatch: *mut core::ffi::c_void, prog_id_hint: &str, _op: &'static str| unsafe {
            crate::bind_native_dispatch_result_shared(
                com_state,
                dispatch.cast::<crate::RawIDispatch>(),
                prog_id_hint,
            )
            .map(RuntimeValue::ObjectHandle)
        },
    )
}

#[cfg(target_os = "windows")]
#[allow(
    unsafe_op_in_unsafe_fn,
    clippy::missing_safety_doc,
    clippy::too_many_arguments
)]
pub unsafe fn invoke_dispatch_runtime_value_with_shared_state(
    dispatch: *mut core::ffi::c_void,
    dispid: i32,
    flags: u16,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    label: &'static str,
    prog_id_hint: &str,
    com_state: &std::sync::Arc<std::sync::Mutex<crate::WindowsComClientState>>,
) -> Result<RuntimeValue, ComInvokeFailure> {
    invoke_dispatch_runtime_value(
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
            crate::bind_native_dispatch_result_shared(
                com_state,
                dispatch.cast::<crate::RawIDispatch>(),
                prog_id_hint,
            )
            .map(RuntimeValue::ObjectHandle)
        },
    )
}
