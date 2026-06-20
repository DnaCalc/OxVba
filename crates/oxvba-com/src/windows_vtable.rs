//! COM vtable this-call marshaller for early-bound dual-interface dispatch.
//!
//! For a **dual interface** member with a known FUNCDESC vtable slot, this calls
//! the custom-interface slot directly — `fnptr = (*(*this))[slot]; hr =
//! fnptr(this, arg1, …, retval_ptr)` — instead of routing through
//! `IDispatch::Invoke(dispid)`. It reuses the one libffi ABI engine
//! (`windows_ffi_bridge::call_via_libffi`) shared with the `Declare` path, and
//! reuses the existing Variant↔VARIANT / BSTR / interface-binding helpers so
//! ownership matches the IDispatch path exactly.
//!
//! Scope: introduced in workset S2 (proven by an in-process unit test against a
//! real custom dual vtable fixture). As of S3 it is wired into live dispatch via
//! [`crate::windows_invoke::try_vtable_member_spec_invoke_with_shared_state`],
//! gated behind the `PreferVtable` policy with an IDispatch fallback for any
//! ineligible member or unsupported marshalling shape.
//!
//! Error model: a vtable call has no `EXCEPINFO`. On `hr < 0` the rich error is
//! retrieved via `GetErrorInfo(0)` → `IErrorInfo` and mapped into the SAME
//! [`ComInvokeExceptionInfo`] the IDispatch path uses, so
//! `render_invoke_fault_message` / `map_com_hresult_label` are reused verbatim.

#![cfg(all(target_os = "windows", target_arch = "x86_64"))]
// `ComInvokeFailure` is a large Err variant shared with the IDispatch path
// (windows_invoke.rs allows the same lint); keeping the failure shape identical
// is what lets S3 route vtable faults through the existing render/classify lane.
#![allow(clippy::result_large_err)]

use crate::windows_ffi_bridge::{FfiArg, FfiReturnType, call_via_libffi};
use crate::windows_invoke::{
    ComInvokeExceptionInfo, ComInvokeFailure, VtableInvocationPlan, bstr_to_string_and_free,
};
use crate::{ComValue, TypeLibParamType, TypeLibRecordInfo, TypeLibWireType};
use oxvba_runtime::{
    ComRecord, Decimal96, ObjectRef, RuntimeByRefSlot, RuntimeByRefWriteback, Variant,
};
use std::ffi::c_void;
use windows_sys::Win32::Foundation::{DECIMAL, SysAllocString, SysFreeString};
use windows_sys::Win32::System::Com::SAFEARRAY;
use windows_sys::Win32::System::Ole::GetRecordInfoFromGuids;
use windows_sys::Win32::System::Variant::{
    VARIANT, VT_ARRAY, VT_DISPATCH, VT_UNKNOWN, VT_VARIANT, VariantClear,
};

// ── IErrorInfo + Get/SetErrorInfo ──
//
// windows-sys 0.59 exposes GetErrorInfo/SetErrorInfo as free functions over an
// untyped `*mut c_void` IErrorInfo; it does not expose the IErrorInfo vtable, so
// we declare the slots we read here (the four accessors after IUnknown).

#[repr(C)]
struct RawIErrorInfoVtbl {
    query_interface: unsafe extern "system" fn(
        this: *mut c_void,
        iid: *const windows_sys::core::GUID,
        ppv: *mut *mut c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(this: *mut c_void) -> u32,
    release: unsafe extern "system" fn(this: *mut c_void) -> u32,
    get_guid:
        unsafe extern "system" fn(this: *mut c_void, pguid: *mut windows_sys::core::GUID) -> i32,
    get_source:
        unsafe extern "system" fn(this: *mut c_void, pbstr: *mut windows_sys::core::BSTR) -> i32,
    get_description:
        unsafe extern "system" fn(this: *mut c_void, pbstr: *mut windows_sys::core::BSTR) -> i32,
    get_help_file:
        unsafe extern "system" fn(this: *mut c_void, pbstr: *mut windows_sys::core::BSTR) -> i32,
    get_help_context: unsafe extern "system" fn(this: *mut c_void, pdw: *mut u32) -> i32,
}

#[repr(C)]
struct RawIErrorInfo {
    vtbl: *const RawIErrorInfoVtbl,
}

// SAFETY: oleaut32 exports transcribed with the stdcall `system` ABI.
// GetErrorInfo retrieves (and clears) the thread's pending IErrorInfo into an
// out-pointer; SetErrorInfo installs (or clears, with a null pointer) it. Each
// call site documents the pointer invariants it upholds.
unsafe extern "system" {
    fn GetErrorInfo(dwreserved: u32, pperrinfo: *mut *mut c_void) -> i32;
    fn SetErrorInfo(dwreserved: u32, perrinfo: *mut c_void) -> i32;
}

/// Clear any pending thread error info so a stale `IErrorInfo` from an earlier
/// call cannot be mis-attributed to this one (mirrors what the COM runtime does
/// before a server entry point).
fn clear_thread_error_info() {
    // SAFETY: SetErrorInfo(0, NULL) is the documented form that clears the
    // thread's current error object; it dereferences no pointer.
    unsafe {
        let _ = SetErrorInfo(0, std::ptr::null_mut());
    }
}

/// Retrieve the rich error the failing call installed via `SetErrorInfo`, if
/// any, and project it into the shared [`ComInvokeExceptionInfo`]. The
/// `IErrorInfo` is Released here; its Source/Description/HelpFile BSTRs are
/// transferred to us and freed via [`bstr_to_string_and_free`].
fn take_error_info() -> Option<ComInvokeExceptionInfo> {
    let mut perrinfo: *mut c_void = std::ptr::null_mut();
    // SAFETY: `perrinfo` is a valid out-pointer; GetErrorInfo writes either null
    // (no pending error, returns S_FALSE) or a single owned IErrorInfo reference.
    let hr = unsafe { GetErrorInfo(0, &mut perrinfo) };
    if hr < 0 || perrinfo.is_null() {
        return None;
    }
    // SAFETY: `perrinfo` is a non-null IErrorInfo carrying one reference we now
    // own; its first field is its vtable (RawIErrorInfo prefix). We call the
    // accessors, then Release exactly once below, and do not use the pointer
    // afterward.
    let info = unsafe {
        let err = perrinfo.cast::<RawIErrorInfo>();
        let vtbl = &*(*err).vtbl;

        let mut source_bstr: windows_sys::core::BSTR = std::ptr::null_mut();
        let source = if (vtbl.get_source)(perrinfo, &mut source_bstr) >= 0 {
            bstr_to_string_and_free(source_bstr)
        } else {
            None
        };

        let mut description_bstr: windows_sys::core::BSTR = std::ptr::null_mut();
        let description = if (vtbl.get_description)(perrinfo, &mut description_bstr) >= 0 {
            bstr_to_string_and_free(description_bstr)
        } else {
            None
        };

        let mut help_file_bstr: windows_sys::core::BSTR = std::ptr::null_mut();
        let help_file = if (vtbl.get_help_file)(perrinfo, &mut help_file_bstr) >= 0 {
            bstr_to_string_and_free(help_file_bstr)
        } else {
            None
        };

        let mut help_context: u32 = 0;
        let help_context =
            if (vtbl.get_help_context)(perrinfo, &mut help_context) >= 0 && help_context != 0 {
                Some(help_context)
            } else {
                None
            };

        (vtbl.release)(perrinfo);

        ComInvokeExceptionInfo {
            source,
            description,
            help_file,
            help_context,
            scode: None,
            wcode: None,
        }
    };

    // An IErrorInfo with nothing readable is no richer than the bare HRESULT.
    if info.source.is_none()
        && info.description.is_none()
        && info.help_file.is_none()
        && info.help_context.is_none()
    {
        None
    } else {
        Some(info)
    }
}

/// How a single inbound parameter was marshalled, so post-call cleanup frees
/// exactly what we own (inbound `[in]` BSTRs and inbound VARIANTs) without
/// double-freeing callee-transferred retvals.
enum InboundOwned {
    /// Nothing to free (scalars, interface [in] borrowed without extra AddRef).
    None,
    /// An `[in]` BSTR we allocated with `SysAllocString` and the callee borrows;
    /// we free it after the call.
    Bstr(windows_sys::core::BSTR),
    /// A VARIANT cell we populated (VT_VARIANT [in] param); `VariantClear` after.
    Variant(Box<VARIANT>),
    /// A VARIANT cell whose SAFEARRAY payload owns the `[in] SAFEARRAY*` we pass.
    SafeArray(Box<VARIANT>),
    /// A DECIMAL cell passed by pointer for Automation `VT_DECIMAL` parameters.
    Decimal(Box<DECIMAL>),
    /// An interface pointer we obtained by `QueryInterface`-ing an `[in]` object arg to
    /// its declared param IID (Bug-4b); we own that one reference and `Release` it after
    /// the call (the callee only borrowed it).
    Interface(*mut c_void),
    /// A COM record handle held alive while its record data pointer is borrowed
    /// by a typed record `[in]` vtable parameter.
    Record(ComRecord),
    /// A mutable ByRef cell whose post-call contents must be decoded before cleanup.
    ByRef(ByRefCell),
}

enum ByRefCell {
    I32(RuntimeByRefSlot, Box<i32>),
    I16(RuntimeByRefSlot, Box<i16>),
    U8(RuntimeByRefSlot, Box<u8>),
    Bool(RuntimeByRefSlot, Box<i16>),
    I64(RuntimeByRefSlot, Box<i64>),
    F64(RuntimeByRefSlot, Box<f64>),
    F32(RuntimeByRefSlot, Box<f32>),
    Currency(RuntimeByRefSlot, Box<i64>),
    Date(RuntimeByRefSlot, Box<f64>),
    Decimal(RuntimeByRefSlot, Box<DECIMAL>),
    Variant(RuntimeByRefSlot, Box<VARIANT>),
    Bstr(RuntimeByRefSlot, Box<windows_sys::core::BSTR>),
    Interface(RuntimeByRefSlot, Box<*mut c_void>),
    LongPtr(RuntimeByRefSlot, Box<isize>),
    SafeArray(RuntimeByRefSlot, Box<VARIANT>),
    Record(RuntimeByRefSlot, ComRecord),
}

pub(crate) struct VtableInvokeResult {
    pub(crate) value: Variant,
    pub(crate) writebacks: Vec<RuntimeByRefWriteback>,
}

/// The retval out-cell, sized to the member's return VARTYPE. The trailing
/// `[out,retval]` pointer passed to the callee points into this.
enum OutCell {
    None,
    I32(Box<i32>),
    I16(Box<i16>),
    U8(Box<u8>),
    /// A `VARIANT_BOOL` (i16) retval cell that decodes to a Boolean Variant
    /// (`!= 0` → true), distinct from a plain `VT_I2` Integer retval.
    Bool(Box<i16>),
    I64(Box<i64>),
    F64(Box<f64>),
    F32(Box<f32>),
    /// A `VT_DATE` (f64 OLE date) retval cell that decodes to a Date Variant,
    /// distinct from a plain `VT_R8` Double retval (`OutCell::F64`).
    Date(Box<f64>),
    Currency(Box<i64>),
    Decimal(Box<DECIMAL>),
    Bstr(Box<windows_sys::core::BSTR>),
    Variant(Box<VARIANT>),
    Interface(Box<*mut c_void>),
    SafeArray(Box<*mut SAFEARRAY>),
    Record(ComRecord),
}

/// Call a dual-interface member through its COM vtable slot.
///
/// Executes an admitted [`VtableInvocationPlan`]: marshals each inbound
/// [`Variant`] arg per the plan's FUNCDESC parameter VARTYPEs, appends a
/// caller-owned `[out,retval]` cell when the plan has a return type, calls
/// `(*(*this))[slot](this, ..., retval_ptr)` via libffi (HRESULT in EAX), then
/// decodes the retval. On `hr < 0` returns a [`ComInvokeFailure`] carrying the
/// `GetErrorInfo`-retrieved rich error.
///
/// `slot` is the x64 vtable **slot index** (the FUNCDESC `oVft / 8` that S1
/// stored), NOT a byte offset.
///
/// # Safety
/// `this` must be a live `IDispatch`/dual-interface pointer whose vtable has at
/// least `slot + 1` entries and whose slot-`slot` function has the C ABI implied
/// by the plan's parameter and return metadata (the dual-member contract:
/// `HRESULT slot(this, params…, retval*)`). The closures must uphold COM
/// ownership and identity rules for object handles and returned interfaces.
pub(crate) unsafe fn vtable_invoke<FResolveObject, FBindDispatch>(
    this: *mut c_void,
    plan: &VtableInvocationPlan,
    args: &[Variant],
    dispid: i32,
    resolve_object: &mut FResolveObject,
    bind_dispatch_result: &mut FBindDispatch,
) -> Result<Variant, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut c_void, String>,
    FBindDispatch: FnMut(*mut c_void) -> Result<Variant, String>,
{
    if plan.parameter_byref_slots.iter().any(Option::is_some) {
        return Err(validation_failure(
            plan.label,
            dispid,
            "vtable ByRef writebacks require the writeback-capable invoke path",
        ));
    }
    // SAFETY: this value-only wrapper has rejected writeback-capable plans, so
    // forwarding the caller's safety contract cannot silently discard ByRef
    // mutations.
    let result = unsafe {
        vtable_invoke_with_writebacks(
            this,
            plan,
            args,
            dispid,
            resolve_object,
            bind_dispatch_result,
        )
    }?;
    if !result.writebacks.is_empty() {
        return Err(validation_failure(
            plan.label,
            dispid,
            "vtable ByRef writebacks require the writeback-capable invoke path",
        ));
    }
    Ok(result.value)
}

pub(crate) unsafe fn vtable_invoke_with_writebacks<FResolveObject, FBindDispatch>(
    this: *mut c_void,
    plan: &VtableInvocationPlan,
    args: &[Variant],
    dispid: i32,
    resolve_object: &mut FResolveObject,
    bind_dispatch_result: &mut FBindDispatch,
) -> Result<VtableInvokeResult, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut c_void, String>,
    FBindDispatch: FnMut(*mut c_void) -> Result<Variant, String>,
{
    if this.is_null() {
        return Err(validation_failure(
            plan.label,
            dispid,
            "null this pointer for vtable invoke",
        ));
    }
    // AV-SAFETY: a live COM interface pointer is always pointer-aligned (8 on x64).
    // A misaligned `this` cannot be a real interface — dereferencing it to read the
    // vtable would access-violate (or, in debug, trip the misaligned-deref check).
    // Decline (validation failure → IDispatch fallback) rather than deref it.
    if !(this as usize).is_multiple_of(std::mem::align_of::<*const c_void>()) {
        return Err(validation_failure(
            plan.label,
            dispid,
            format!("misaligned this pointer {this:p} for vtable invoke"),
        ));
    }
    if args.len() != plan.parameter_types.len() {
        return Err(validation_failure(
            plan.label,
            dispid,
            format!(
                "vtable arity mismatch: {} args for {} parameters",
                args.len(),
                plan.parameter_types.len()
            ),
        ));
    }
    if let Err(issue) = crate::typelib::validate_vtable_wire_signature(
        &plan.parameter_types,
        &plan.parameter_wire_types,
        &plan.parameter_iids,
        plan.return_type,
        plan.return_wire_type.as_ref(),
    ) {
        return Err(validation_failure(
            plan.label,
            dispid,
            vtable_signature_issue_detail(
                issue,
                plan.parameter_wire_types.len(),
                plan.parameter_types.len(),
            ),
        ));
    }
    for (index, param_type) in plan.parameter_types.iter().enumerate() {
        let wire_type = plan.parameter_wire_types.get(index);
        let needs_writeback = param_type.is_by_ref()
            || matches!(wire_type, Some(TypeLibWireType::ByRefSafeArrayVariant));
        if needs_writeback
            && !plan
                .parameter_byref_slots
                .get(index)
                .is_some_and(Option::is_some)
        {
            return Err(validation_failure(
                plan.label,
                dispid,
                format!("vtable ByRef parameter {param_type:?} is missing its writeback slot"),
            ));
        }
    }

    // Resolve the slot function pointer: this is `*const *const fnptr`, so the
    // vtable is `*(*this)` and the entry is `vtable[slot]`.
    // SAFETY: the fn contract guarantees `this` is a live interface pointer with
    // a vtable of at least `slot + 1` entries; we read the vtable base then the
    // slot-`slot` function pointer, both in bounds.
    let fnptr = unsafe {
        let vtbl = *this.cast::<*const *const c_void>();
        *vtbl.add(plan.slot as usize)
    };
    if fnptr.is_null() {
        return Err(validation_failure(
            plan.label,
            dispid,
            "null vtable slot function pointer",
        ));
    }

    // Build the libffi argument list left-to-right: arg0 = this, then the
    // marshalled params, then (if any) the trailing [out,retval] pointer.
    let mut ffi_args: Vec<FfiArg> = Vec::with_capacity(args.len() + 2);
    let mut inbound_owned: Vec<InboundOwned> = Vec::with_capacity(args.len());
    ffi_args.push(FfiArg::Pointer(this));

    for (i, (param_type, arg)) in plan.parameter_types.iter().zip(args.iter()).enumerate() {
        // The per-parameter declared interface IID for an object arg (Bug-4b); `None`
        // for scalars or when no IID was recovered. `get(i)` tolerates synthesized
        // trailing optionals (which extend `args` beyond `parameter_iids`).
        let param_iid = plan.parameter_iids.get(i).copied().flatten();
        let wire_type = plan.parameter_wire_types.get(i);
        let byref_slot = plan.parameter_byref_slots.get(i).copied().flatten();
        match marshal_inbound_param(
            *param_type,
            wire_type,
            arg,
            param_iid,
            byref_slot,
            resolve_object,
        ) {
            Ok((ffi_arg, owned)) => {
                ffi_args.push(ffi_arg);
                inbound_owned.push(owned);
            }
            Err(detail) => {
                // Free everything marshalled so far before bailing.
                free_inbound(&mut inbound_owned);
                return Err(validation_failure(plan.label, dispid, detail));
            }
        }
    }

    // Allocate the [out,retval] cell and append its pointer.
    let mut out_cell = match plan.return_type {
        Some(rt) => match alloc_out_cell(rt, plan.return_wire_type.as_ref()) {
            Ok(cell) => cell,
            Err(detail) => {
                free_inbound(&mut inbound_owned);
                return Err(validation_failure(plan.label, dispid, detail));
            }
        },
        None => OutCell::None,
    };
    if let Some(ptr) = out_cell_ptr(&mut out_cell) {
        ffi_args.push(FfiArg::Pointer(ptr));
    }

    // Clear any stale thread error info, then call. HRESULT is the EAX return.
    clear_thread_error_info();
    // SAFETY: `fnptr` is the slot-`slot` function whose ABI the fn contract
    // pins to `HRESULT slot(this, params…, retval*)`; every pointer-bearing
    // arg (this, inbound BSTR/VARIANT cells, the out-cell) is owned by the
    // `inbound_owned`/`out_cell` locals and stays alive across the call.
    let hr = unsafe { call_via_libffi(fnptr as usize, &ffi_args, FfiReturnType::Long) } as i32;

    if hr < 0 {
        // We own inbound cells even on failure; release them before surfacing the
        // COM error. Writebacks are not valid on a failing HRESULT.
        free_inbound(&mut inbound_owned);
        // Drop any retval cell payload the callee may have written before the
        // failure (defensive: most servers leave it zeroed on failure).
        discard_out_cell(out_cell);
        return Err(ComInvokeFailure {
            label: plan.label,
            dispid,
            hr: Some(hr),
            arg_err: None,
            excep: take_error_info(),
            detail: None,
        });
    }

    let writebacks =
        match collect_writebacks(&mut inbound_owned, plan.label, dispid, bind_dispatch_result) {
            Ok(writebacks) => writebacks,
            Err(failure) => {
                free_inbound(&mut inbound_owned);
                discard_out_cell(out_cell);
                return Err(failure);
            }
        };
    // We own inbound [in] and ByRef cells; free them after writeback decoding.
    free_inbound(&mut inbound_owned);

    // hr >= 0, so the callee populated the retval cell per its declared return
    // type; decode it and take ownership of any transferred BSTR/interface/
    // VARIANT payload.
    let value = decode_out_cell(out_cell, plan.label, dispid, bind_dispatch_result)?;
    Ok(VtableInvokeResult { value, writebacks })
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

fn vtable_signature_issue_detail(
    issue: crate::typelib::TypeLibVtableSignatureIssue,
    parameter_wire_count: usize,
    parameter_count: usize,
) -> String {
    match issue {
        crate::typelib::TypeLibVtableSignatureIssue::UnsupportedParameterType(param_type) => {
            format!(
                "vtable inbound parameter VARTYPE {param_type:?} is not supported in v1 (use the IDispatch fallback)"
            )
        }
        crate::typelib::TypeLibVtableSignatureIssue::UnsupportedReturnType(return_type) => {
            format!(
                "vtable retval VARTYPE {return_type:?} is not supported in v1 (use the IDispatch fallback)"
            )
        }
        crate::typelib::TypeLibVtableSignatureIssue::ParameterWireTypeArityMismatch => format!(
            "vtable wire-shape arity mismatch: {parameter_wire_count} wire types for {parameter_count} parameters"
        ),
        crate::typelib::TypeLibVtableSignatureIssue::UnsupportedParameterWireType => {
            "vtable parameter wire shape is not supported in v1".to_string()
        }
        crate::typelib::TypeLibVtableSignatureIssue::UnsupportedReturnWireType => {
            "vtable return wire shape is not supported in v1".to_string()
        }
        crate::typelib::TypeLibVtableSignatureIssue::MissingObjectParameterIid => {
            "vtable object parameter is missing its declared interface IID".to_string()
        }
        crate::typelib::TypeLibVtableSignatureIssue::MissingRecordParameterWireType => {
            "vtable record parameter is missing explicit record wire metadata".to_string()
        }
        crate::typelib::TypeLibVtableSignatureIssue::MissingRecordReturnInfo => {
            "vtable record retval is missing IRecordInfo allocation metadata".to_string()
        }
    }
}

/// Marshal one inbound parameter from a [`Variant`] per the workset §4 table.
/// Internally calls into FFI helpers under documented `unsafe` blocks; the
/// returned [`InboundOwned`] records what the caller must free post-call.
fn marshal_inbound_param<FResolveObject>(
    param_type: TypeLibParamType,
    wire_type: Option<&TypeLibWireType>,
    arg: &Variant,
    param_iid: Option<crate::ComInterfaceIid>,
    byref_slot: Option<RuntimeByRefSlot>,
    resolve_object: &mut FResolveObject,
) -> Result<(FfiArg, InboundOwned), String>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut c_void, String>,
{
    use TypeLibParamType as P;
    let value = ComValue::from_variant(arg)?;
    if matches!(wire_type, Some(TypeLibWireType::SafeArrayVariant)) {
        return marshal_inbound_safearray_param(&value, resolve_object);
    }
    if matches!(wire_type, Some(TypeLibWireType::ByRefSafeArrayVariant)) {
        return marshal_byref_safearray_param(&value, byref_slot, resolve_object);
    }
    if param_type.is_by_ref() {
        return marshal_byref_param(
            param_type,
            wire_type,
            &value,
            arg,
            param_iid,
            byref_slot,
            resolve_object,
        );
    }
    Ok(match param_type {
        // Scalars by value (reusing the dynlink scalar conventions). BOOL is an
        // i16 VARIANT_BOOL (-1/0); CY is an i64 scaled ×10000; DATE is an f64.
        P::Long => (
            FfiArg::Long(com_value_to_i32(&value, "VT_I4")?),
            InboundOwned::None,
        ),
        P::Integer => (
            FfiArg::Integer(com_value_to_i16(&value, "VT_I2")?),
            InboundOwned::None,
        ),
        P::Byte => (FfiArg::Byte(com_value_to_u8(&value)?), InboundOwned::None),
        P::LongLong => (
            FfiArg::LongLong(com_value_to_i64(&value)?),
            InboundOwned::None,
        ),
        P::Boolean => {
            let b = arg
                .as_bool()
                .ok_or_else(|| "vtable VT_BOOL parameter expects a Boolean argument".to_string())?;
            (FfiArg::Boolean(if b { -1 } else { 0 }), InboundOwned::None)
        }
        P::Double => (
            FfiArg::Double(com_value_to_f64(&value)?),
            InboundOwned::None,
        ),
        P::Single => (
            FfiArg::Single(com_value_to_f64(&value)? as f32),
            InboundOwned::None,
        ),
        P::Date => (
            FfiArg::Double(com_value_to_f64(&value)?),
            InboundOwned::None,
        ),
        P::Currency => (
            FfiArg::LongLong(com_value_to_currency_i64(&value)?),
            InboundOwned::None,
        ),
        P::Decimal => {
            let decimal = com_value_to_decimal(&value)?;
            let mut cell = Box::new(decimal96_to_windows(decimal));
            let ptr = cell.as_mut() as *mut DECIMAL;
            (
                FfiArg::Pointer(ptr.cast::<c_void>()),
                InboundOwned::Decimal(cell),
            )
        }
        // BSTR: we allocate the wide string; the callee borrows it [in]; we free
        // it after the call (InboundOwned::Bstr).
        P::String => {
            let text = match &value {
                ComValue::String(s) => s.to_string(),
                other => {
                    return Err(format!(
                        "vtable VT_BSTR parameter expects a String, got {other:?}"
                    ));
                }
            };
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            // SAFETY: `wide` is a NUL-terminated UTF-16 buffer alive across this
            // call; SysAllocString copies it into a fresh BSTR we own.
            let bstr = unsafe { SysAllocString(wide.as_ptr()) };
            if bstr.is_null() {
                return Err("SysAllocString returned null for vtable VT_BSTR parameter".to_string());
            }
            (
                FfiArg::Pointer(bstr as *mut c_void),
                InboundOwned::Bstr(bstr),
            )
        }
        // Interface [in]: QueryInterface the resolved object for the parameter's
        // DECLARED interface IID (Bug-4b) so the callee receives the exact vtable it
        // expects — passing a raw IDispatch where `IFoo*` is declared would call the
        // wrong vtable and AV the host. We own the QI'd reference and Release it after
        // the call (InboundOwned::Interface). When no IID was recovered (None — e.g.
        // fixture/catalog metadata), the vtable gate has already declined this member,
        // so this falls back to the borrowed raw IDispatch only on a path the gate
        // never admits for a real slot call.
        P::Object => {
            let object = arg.as_object_ref().ok_or_else(|| {
                "vtable VT_DISPATCH parameter expects an object argument".to_string()
            })?;
            let dispatch = resolve_object(object)?;
            match param_iid {
                Some(iid) => {
                    // SAFETY: `dispatch` is the live bindings-map-retained object pointer
                    // the resolver returned; QueryInterface reads its IUnknown vtable and
                    // hands back one fresh reference we own (Released in `free_inbound`).
                    let interface =
                        unsafe { crate::query_interface_pointer(dispatch, &iid.to_guid()) }?;
                    (
                        FfiArg::Pointer(interface),
                        InboundOwned::Interface(interface),
                    )
                }
                None => (FfiArg::Pointer(dispatch), InboundOwned::None),
            }
        }
        P::Record => match value {
            ComValue::Record(record) => (
                FfiArg::Pointer(record.record_data_ptr()),
                InboundOwned::Record(record),
            ),
            other => {
                return Err(format!(
                    "vtable record parameter expects a Record argument, got {other:?}"
                ));
            }
        },
        // VT_VARIANT [in] by reference: marshal the value into a heap VARIANT and
        // pass its pointer; we VariantClear it after the call.
        P::Variant => {
            // REFCOUNT SAFETY: when the `[in]` VARIANT carries an OBJECT, the cell must
            // own its OWN reference on that object — `set_variant_from_com_value` places
            // the IDispatch and the `add_ref` closure below `AddRef`s it (+1), which the
            // post-call `free_inbound` → `VariantClear` (Release, −1) exactly balances.
            // (The earlier v1 used an add-ref-noop here, a net under-ref that could free a
            // still-referenced server object → a dangling pointer + host AV; that decline
            // is now lifted.) Scalar / string / numeric VARIANTs carry no reference, so
            // the `AddRef` closure is simply never invoked for them.
            // SAFETY: an all-zero VARIANT is a valid VT_EMPTY VARIANT.
            let mut cell: Box<VARIANT> = Box::new(unsafe { std::mem::zeroed() });
            let mut add_ref = |dispatch: *mut c_void| {
                // SAFETY: `dispatch` is the live bindings-map-retained IDispatch the
                // resolver returned; the cell now holds it, so AddRef gives the cell its
                // own reference (balanced by VariantClear in `free_inbound`).
                unsafe {
                    let _ = crate::add_ref_dispatch(dispatch.cast::<crate::RawIDispatch>());
                }
            };
            // SAFETY: `cell` is a fresh zeroed writable VARIANT; the resolver and
            // add-ref closures uphold the helper's object-handle contract.
            unsafe {
                crate::set_variant_from_com_value(
                    cell.as_mut(),
                    &value,
                    resolve_object,
                    &mut add_ref,
                )?;
            }
            let ptr = cell.as_mut() as *mut VARIANT;
            (
                FfiArg::Pointer(ptr.cast::<c_void>()),
                InboundOwned::Variant(cell),
            )
        }
        other => {
            return Err(format!(
                "vtable inbound parameter VARTYPE {other:?} is not supported in v1 (use the IDispatch fallback)"
            ));
        }
    })
}

fn marshal_byref_param<FResolveObject>(
    param_type: TypeLibParamType,
    wire_type: Option<&TypeLibWireType>,
    value: &ComValue,
    arg: &Variant,
    param_iid: Option<crate::ComInterfaceIid>,
    byref_slot: Option<RuntimeByRefSlot>,
    resolve_object: &mut FResolveObject,
) -> Result<(FfiArg, InboundOwned), String>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut c_void, String>,
{
    use TypeLibParamType as P;
    let slot = byref_slot.ok_or_else(|| {
        format!("vtable ByRef parameter {param_type:?} requires a runtime ByRef slot")
    })?;
    let (ptr, cell) = match param_type {
        P::ByRefLong => {
            let mut cell = Box::new(com_value_to_i32(value, "VT_BYREF|VT_I4")?);
            let ptr = cell.as_mut() as *mut i32;
            (ptr.cast::<c_void>(), ByRefCell::I32(slot, cell))
        }
        P::ByRefInteger => {
            let mut cell = Box::new(com_value_to_i16(value, "VT_BYREF|VT_I2")?);
            let ptr = cell.as_mut() as *mut i16;
            (ptr.cast::<c_void>(), ByRefCell::I16(slot, cell))
        }
        P::ByRefByte => {
            let mut cell = Box::new(com_value_to_u8(value)?);
            let ptr = cell.as_mut() as *mut u8;
            (ptr.cast::<c_void>(), ByRefCell::U8(slot, cell))
        }
        P::ByRefBoolean => {
            let b = arg
                .as_bool()
                .ok_or_else(|| "vtable VT_BYREF|VT_BOOL parameter expects Boolean".to_string())?;
            let mut cell = Box::new(if b { -1 } else { 0 });
            let ptr = cell.as_mut() as *mut i16;
            (ptr.cast::<c_void>(), ByRefCell::Bool(slot, cell))
        }
        P::ByRefLongLong => {
            let mut cell = Box::new(com_value_to_i64(value)?);
            let ptr = cell.as_mut() as *mut i64;
            (ptr.cast::<c_void>(), ByRefCell::I64(slot, cell))
        }
        P::ByRefDouble => {
            let mut cell = Box::new(com_value_to_f64(value)?);
            let ptr = cell.as_mut() as *mut f64;
            (ptr.cast::<c_void>(), ByRefCell::F64(slot, cell))
        }
        P::ByRefSingle => {
            let mut cell = Box::new(com_value_to_f64(value)? as f32);
            let ptr = cell.as_mut() as *mut f32;
            (ptr.cast::<c_void>(), ByRefCell::F32(slot, cell))
        }
        P::ByRefCurrency => {
            let mut cell = Box::new(com_value_to_currency_i64(value)?);
            let ptr = cell.as_mut() as *mut i64;
            (ptr.cast::<c_void>(), ByRefCell::Currency(slot, cell))
        }
        P::ByRefDate => {
            let mut cell = Box::new(com_value_to_f64(value)?);
            let ptr = cell.as_mut() as *mut f64;
            (ptr.cast::<c_void>(), ByRefCell::Date(slot, cell))
        }
        P::ByRefDecimal => {
            let mut cell = Box::new(decimal96_to_windows(com_value_to_decimal(value)?));
            let ptr = cell.as_mut() as *mut DECIMAL;
            (ptr.cast::<c_void>(), ByRefCell::Decimal(slot, cell))
        }
        P::ByRefVariant => {
            // SAFETY: an all-zero VARIANT is a valid VT_EMPTY VARIANT.
            let mut cell: Box<VARIANT> = Box::new(unsafe { std::mem::zeroed() });
            // SAFETY: the VARIANT cell owns any object reference written into it,
            // so this AddRef is later balanced by VariantClear in free_byref_cell.
            let mut add_ref = |dispatch: *mut c_void| unsafe {
                let _ = crate::add_ref_dispatch(dispatch.cast::<crate::RawIDispatch>());
            };
            // SAFETY: `cell` is writable, and the resolver/add-ref closures uphold
            // set_variant_from_com_value's object ownership contract.
            unsafe {
                crate::set_variant_from_com_value(
                    cell.as_mut(),
                    value,
                    resolve_object,
                    &mut add_ref,
                )?;
            }
            let ptr = cell.as_mut() as *mut VARIANT;
            (ptr.cast::<c_void>(), ByRefCell::Variant(slot, cell))
        }
        P::ByRefString => {
            let text = match value {
                ComValue::String(s) => s.to_string(),
                other => {
                    return Err(format!(
                        "vtable VT_BYREF|VT_BSTR parameter expects a String, got {other:?}"
                    ));
                }
            };
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            // SAFETY: `wide` is NUL-terminated and alive for the call;
            // SysAllocString copies it into an owned BSTR variable cell.
            let bstr = unsafe { SysAllocString(wide.as_ptr()) };
            if bstr.is_null() {
                return Err(
                    "SysAllocString returned null for vtable VT_BYREF|VT_BSTR parameter"
                        .to_string(),
                );
            }
            let mut cell = Box::new(bstr);
            let ptr = cell.as_mut() as *mut windows_sys::core::BSTR;
            (ptr.cast::<c_void>(), ByRefCell::Bstr(slot, cell))
        }
        P::ByRefObject => {
            if !matches!(wire_type, Some(TypeLibWireType::InterfacePointer { .. })) {
                return Err(
                    "vtable ByRef object requires explicit InterfacePointer wire metadata"
                        .to_string(),
                );
            }
            let iid = param_iid.ok_or_else(|| {
                "vtable ByRef object parameter is missing its declared interface IID".to_string()
            })?;
            let object = arg.as_object_ref().ok_or_else(|| {
                "vtable VT_BYREF|VT_DISPATCH parameter expects an object argument".to_string()
            })?;
            let dispatch = resolve_object(object)?;
            // SAFETY: `dispatch` is a live COM pointer from the bindings map; QI
            // returns one owned reference for the ByRef interface variable.
            let interface = unsafe { crate::query_interface_pointer(dispatch, &iid.to_guid()) }?;
            let mut cell = Box::new(interface);
            let ptr = cell.as_mut() as *mut *mut c_void;
            (ptr.cast::<c_void>(), ByRefCell::Interface(slot, cell))
        }
        P::ByRefLongPtr => {
            let mut cell = Box::new(com_value_to_i64(value)? as isize);
            let ptr = cell.as_mut() as *mut isize;
            (ptr.cast::<c_void>(), ByRefCell::LongPtr(slot, cell))
        }
        P::ByRefRecord => {
            if !matches!(wire_type, Some(TypeLibWireType::ByRefRecord { .. })) {
                return Err(
                    "vtable ByRef record requires explicit ByRefRecord wire metadata".to_string(),
                );
            }
            let ComValue::Record(record) = value else {
                return Err(format!(
                    "vtable ByRef record parameter expects a Record argument, got {value:?}"
                ));
            };
            let record = record.deep_clone()?;
            let ptr = record.record_data_ptr();
            (ptr.cast::<c_void>(), ByRefCell::Record(slot, record))
        }
        other => {
            return Err(format!(
                "vtable ByRef parameter VARTYPE {other:?} is not supported yet"
            ));
        }
    };
    Ok((FfiArg::Pointer(ptr), InboundOwned::ByRef(cell)))
}

fn marshal_byref_safearray_param<FResolveObject>(
    value: &ComValue,
    byref_slot: Option<RuntimeByRefSlot>,
    resolve_object: &mut FResolveObject,
) -> Result<(FfiArg, InboundOwned), String>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut c_void, String>,
{
    let slot = byref_slot
        .ok_or_else(|| "vtable ByRef SAFEARRAY requires a runtime ByRef slot".to_string())?;
    let (_, owned) = marshal_inbound_safearray_param(value, resolve_object)?;
    let InboundOwned::SafeArray(cell) = owned else {
        return Err("vtable ByRef SAFEARRAY lowered to an unexpected inbound cell".to_string());
    };
    let mut cell = cell;
    // SAFETY: `cell` is a VARIANT initialized by marshal_inbound_safearray_param
    // with a SAFEARRAY payload, so taking the address of its parray field gives
    // the SAFEARRAY** required by VT_BYREF|VT_ARRAY parameters.
    let ptr = unsafe {
        (&mut cell.Anonymous.Anonymous.Anonymous.parray as *mut *mut SAFEARRAY).cast::<c_void>()
    };
    Ok((
        FfiArg::Pointer(ptr),
        InboundOwned::ByRef(ByRefCell::SafeArray(slot, cell)),
    ))
}

fn marshal_inbound_safearray_param<FResolveObject>(
    value: &ComValue,
    resolve_object: &mut FResolveObject,
) -> Result<(FfiArg, InboundOwned), String>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut c_void, String>,
{
    if !matches!(value, ComValue::ArrayIntent(_)) {
        return Err(format!(
            "vtable SAFEARRAY(VARIANT) parameter expects an array argument, got {value:?}"
        ));
    }
    // SAFETY: an all-zero VARIANT is a valid VT_EMPTY VARIANT.
    let mut cell: Box<VARIANT> = Box::new(unsafe { std::mem::zeroed() });
    let mut add_ref = |dispatch: *mut c_void| {
        // SAFETY: `dispatch` is a live COM object pointer resolved from the
        // bindings map; the SAFEARRAY/VARIANT cell owns its reference until
        // VariantClear in `free_inbound`.
        unsafe {
            let _ = crate::add_ref_dispatch(dispatch.cast::<crate::RawIDispatch>());
        }
    };
    // SAFETY: `cell` is a fresh writable VARIANT. `set_variant_from_com_value`
    // creates a Windows SAFEARRAY payload owned by the VARIANT.
    unsafe {
        crate::set_variant_from_com_value(cell.as_mut(), value, resolve_object, &mut add_ref)?;
    }
    // SAFETY: `cell` is initialized by `set_variant_from_com_value`, so reading
    // the VARIANT discriminant is valid.
    let vt = unsafe { cell.Anonymous.Anonymous.vt };
    if vt & VT_ARRAY == 0 {
        // SAFETY: `cell` owns any payload written by `set_variant_from_com_value`;
        // clearing it releases that payload before the validation error returns.
        unsafe {
            let _ = VariantClear(cell.as_mut());
        }
        return Err(format!(
            "vtable SAFEARRAY(VARIANT) parameter lowered to non-array VARIANT vt={vt:#06X}"
        ));
    }
    // SAFETY: the discriminant above proves this VARIANT carries a SAFEARRAY
    // payload, so reading the union's parray field is valid.
    let psa = unsafe { cell.Anonymous.Anonymous.Anonymous.parray };
    if psa.is_null() {
        // SAFETY: as above, `cell` owns any payload that must be released before
        // returning the validation error.
        unsafe {
            let _ = VariantClear(cell.as_mut());
        }
        return Err("vtable SAFEARRAY(VARIANT) parameter lowered to null SAFEARRAY".to_string());
    }
    Ok((
        FfiArg::Pointer(psa.cast::<c_void>()),
        InboundOwned::SafeArray(cell),
    ))
}

fn free_inbound(owned: &mut Vec<InboundOwned>) {
    for entry in owned.drain(..) {
        match entry {
            InboundOwned::None => {}
            // SAFETY: a Bstr entry is a BSTR we allocated via SysAllocString and
            // the callee only borrowed; freeing it exactly once here is correct.
            InboundOwned::Bstr(bstr) => unsafe { SysFreeString(bstr) },
            // SAFETY: a Variant entry is a VARIANT cell we populated; clearing it
            // releases any BSTR/SAFEARRAY/interface payload it owns exactly once.
            InboundOwned::Variant(mut cell) => unsafe {
                let _ = VariantClear(cell.as_mut());
            },
            InboundOwned::SafeArray(mut cell) => {
                // SAFETY: the cell owns the SAFEARRAY payload that backed the
                // borrowed inbound SAFEARRAY* for the duration of the call.
                unsafe {
                    let _ = VariantClear(cell.as_mut());
                }
            }
            InboundOwned::Decimal(cell) => drop(cell),
            // SAFETY: an Interface entry is the single reference our QueryInterface
            // handed us; the callee only borrowed it, so we Release it exactly once.
            InboundOwned::Interface(interface) => unsafe { crate::release_unknown(interface) },
            InboundOwned::Record(record) => drop(record),
            InboundOwned::ByRef(cell) => free_byref_cell(cell),
        }
    }
}

fn collect_writebacks<FBindDispatch>(
    owned: &mut [InboundOwned],
    label: &'static str,
    dispid: i32,
    bind_dispatch_result: &mut FBindDispatch,
) -> Result<Vec<RuntimeByRefWriteback>, ComInvokeFailure>
where
    FBindDispatch: FnMut(*mut c_void) -> Result<Variant, String>,
{
    let mut writebacks = Vec::new();
    for entry in owned.iter_mut() {
        if let InboundOwned::ByRef(cell) = entry {
            writebacks.push(decode_byref_cell(
                cell,
                label,
                dispid,
                bind_dispatch_result,
            )?);
        }
    }
    Ok(writebacks)
}

fn decode_byref_cell<FBindDispatch>(
    cell: &mut ByRefCell,
    label: &'static str,
    dispid: i32,
    bind_dispatch_result: &mut FBindDispatch,
) -> Result<RuntimeByRefWriteback, ComInvokeFailure>
where
    FBindDispatch: FnMut(*mut c_void) -> Result<Variant, String>,
{
    let (slot, value) = match cell {
        ByRefCell::I32(slot, cell) => (*slot, Variant::from_i32(**cell)),
        ByRefCell::I16(slot, cell) => (*slot, Variant::from_i16(**cell)),
        ByRefCell::U8(slot, cell) => (*slot, Variant::from_u8(**cell)),
        ByRefCell::Bool(slot, cell) => (*slot, Variant::from_bool(**cell != 0)),
        ByRefCell::I64(slot, cell) => (*slot, Variant::from_i64(**cell)),
        ByRefCell::F64(slot, cell) => (*slot, Variant::from_f64(**cell)),
        ByRefCell::F32(slot, cell) => (*slot, Variant::from_f64(f64::from(**cell))),
        ByRefCell::Currency(slot, cell) => (*slot, Variant::from_currency_scaled_i64(**cell)),
        ByRefCell::Date(slot, cell) => (*slot, Variant::from_date_f64(**cell)),
        ByRefCell::Decimal(slot, cell) => (
            *slot,
            Variant::from_decimal96(decimal96_from_windows(cell.as_ref())),
        ),
        ByRefCell::Variant(slot, cell) => {
            // SAFETY: on a successful HRESULT the ByRef VARIANT cell is initialized
            // and variant_to_com_value only reads its discriminant/payload.
            let value = unsafe { crate::variant_to_com_value(cell.as_ref()) }
                .and_then(|value| value.to_variant())
                .map_err(|detail| validation_failure(label, dispid, detail))?;
            (*slot, value)
        }
        ByRefCell::Bstr(slot, cell) => {
            let raw = **cell;
            **cell = std::ptr::null_mut();
            // SAFETY: the ByRef BSTR cell owns the final BSTR value after a
            // successful call; this converts and frees it exactly once.
            let text = unsafe { bstr_to_string_and_free(raw) }.unwrap_or_default();
            (*slot, Variant::from_string(text))
        }
        ByRefCell::Interface(slot, cell) => {
            let raw = **cell;
            **cell = std::ptr::null_mut();
            if raw.is_null() {
                (
                    *slot,
                    Variant::from_object_ref(ObjectRef::from_compat_identity(0)),
                )
            } else {
                let value = bind_dispatch_result(raw).map_err(|detail| {
                    validation_failure(label, dispid, format!("ByRef object writeback: {detail}"))
                })?;
                (*slot, value)
            }
        }
        ByRefCell::LongPtr(slot, cell) => (*slot, Variant::from_i64(**cell as i64)),
        ByRefCell::SafeArray(slot, cell) => {
            // SAFETY: on success the VARIANT cell still describes the SAFEARRAY
            // pointer variable. variant_to_com_value clones the array into the
            // runtime carrier; free_byref_cell later clears the cell payload.
            let value = unsafe { crate::variant_to_com_value(cell.as_ref()) }
                .and_then(|value| value.to_variant())
                .map_err(|detail| validation_failure(label, dispid, detail))?;
            (*slot, value)
        }
        ByRefCell::Record(slot, record) => (*slot, Variant::from_com_record(record.clone())),
    };
    Ok(RuntimeByRefWriteback::new(slot, value))
}

fn free_byref_cell(cell: ByRefCell) {
    match cell {
        ByRefCell::Variant(_, mut cell) | ByRefCell::SafeArray(_, mut cell) => {
            // SAFETY: the ByRef VARIANT cell was initialized by this marshaller and
            // is owned by the cell; clearing it releases any payload exactly once.
            unsafe {
                let _ = VariantClear(cell.as_mut());
            }
        }
        ByRefCell::Bstr(_, cell) => {
            if !(*cell).is_null() {
                // SAFETY: the BSTR cell still owns this BSTR because decode did
                // not take it, typically due to an earlier decode failure.
                unsafe { SysFreeString(*cell) };
            }
        }
        ByRefCell::Interface(_, cell) => {
            if !(*cell).is_null() {
                // SAFETY: the interface cell still owns this QI/transferred
                // reference because decode did not bind it.
                unsafe { crate::release_unknown(*cell) };
            }
        }
        _ => {}
    }
}

fn alloc_out_cell(
    return_type: TypeLibParamType,
    return_wire_type: Option<&TypeLibWireType>,
) -> Result<OutCell, String> {
    use TypeLibParamType as P;
    if matches!(return_wire_type, Some(TypeLibWireType::SafeArrayVariant)) {
        if return_type == P::Variant {
            return Ok(OutCell::SafeArray(Box::new(std::ptr::null_mut())));
        }
        return Err(format!(
            "vtable SAFEARRAY(VARIANT) retval requires semantic Variant, got {return_type:?}"
        ));
    }
    if matches!(
        return_wire_type,
        Some(TypeLibWireType::Record {
            record_info: Some(_),
            ..
        })
    ) {
        if return_type == P::Record {
            return Ok(OutCell::Record(alloc_record_retval(return_wire_type)?));
        }
        return Err(format!(
            "vtable record retval requires semantic Record, got {return_type:?}"
        ));
    }
    Ok(match return_type {
        P::Long => OutCell::I32(Box::new(0)),
        P::Integer => OutCell::I16(Box::new(0)),
        P::Boolean => OutCell::Bool(Box::new(0)),
        P::Byte => OutCell::U8(Box::new(0)),
        P::LongLong => OutCell::I64(Box::new(0)),
        P::Double => OutCell::F64(Box::new(0.0)),
        P::Date => OutCell::Date(Box::new(0.0)),
        P::Single => OutCell::F32(Box::new(0.0)),
        P::Currency => OutCell::Currency(Box::new(0)),
        // SAFETY: zeroed DECIMAL is 0 with scale/sign/reserved fields clear.
        P::Decimal => OutCell::Decimal(Box::new(unsafe { std::mem::zeroed() })),
        P::String => OutCell::Bstr(Box::new(std::ptr::null_mut())),
        P::Object => OutCell::Interface(Box::new(std::ptr::null_mut())),
        // SAFETY: zeroing a VARIANT yields a valid VT_EMPTY VARIANT.
        P::Variant => OutCell::Variant(Box::new(unsafe { std::mem::zeroed() })),
        other => {
            return Err(format!(
                "vtable retval VARTYPE {other:?} is not supported in v1 (use the IDispatch fallback)"
            ));
        }
    })
}

fn alloc_record_retval(return_wire_type: Option<&TypeLibWireType>) -> Result<ComRecord, String> {
    let Some(TypeLibWireType::Record {
        record_info: Some(record_info),
        ..
    }) = return_wire_type
    else {
        return Err("vtable record retval is missing IRecordInfo allocation metadata".to_string());
    };
    let record_info = get_record_info_from_descriptor(record_info)?;
    // SAFETY: `get_record_info_from_descriptor` returns one owned IRecordInfo
    // reference. The ComRecord adopts and releases that reference.
    unsafe { crate::windows_variant::create_com_record_from_record_info(record_info) }
}

fn get_record_info_from_descriptor(record_info: &TypeLibRecordInfo) -> Result<*mut c_void, String> {
    let libid = record_info.libid.to_guid();
    let type_guid = record_info.type_guid.to_guid();
    let mut raw: *mut c_void = std::ptr::null_mut();
    // SAFETY: all GUID/version/LCID fields come from live typelib metadata. On
    // success OleAut writes one owned IRecordInfo reference to `raw`.
    let hr = unsafe {
        GetRecordInfoFromGuids(
            &libid,
            u32::from(record_info.major),
            u32::from(record_info.minor),
            record_info.lcid,
            &type_guid,
            &mut raw,
        )
    };
    if hr < 0 || raw.is_null() {
        return Err(format!(
            "GetRecordInfoFromGuids failed for record retval with HRESULT {:#010X}",
            hr as u32
        ));
    }
    Ok(raw)
}

fn out_cell_ptr(cell: &mut OutCell) -> Option<*mut c_void> {
    match cell {
        OutCell::None => None,
        OutCell::I32(b) => Some((b.as_mut() as *mut i32).cast::<c_void>()),
        OutCell::I16(b) | OutCell::Bool(b) => Some((b.as_mut() as *mut i16).cast::<c_void>()),
        OutCell::U8(b) => Some((b.as_mut() as *mut u8).cast::<c_void>()),
        OutCell::I64(b) | OutCell::Currency(b) => Some((b.as_mut() as *mut i64).cast::<c_void>()),
        OutCell::Decimal(b) => Some((b.as_mut() as *mut DECIMAL).cast::<c_void>()),
        OutCell::F64(b) | OutCell::Date(b) => Some((b.as_mut() as *mut f64).cast::<c_void>()),
        OutCell::F32(b) => Some((b.as_mut() as *mut f32).cast::<c_void>()),
        OutCell::Bstr(b) => Some((b.as_mut() as *mut windows_sys::core::BSTR).cast::<c_void>()),
        OutCell::Variant(b) => Some((b.as_mut() as *mut VARIANT).cast::<c_void>()),
        OutCell::Interface(b) => Some((b.as_mut() as *mut *mut c_void).cast::<c_void>()),
        OutCell::SafeArray(b) => Some((b.as_mut() as *mut *mut SAFEARRAY).cast::<c_void>()),
        OutCell::Record(record) => Some(record.record_data_ptr().cast::<c_void>()),
    }
}

/// Free a retval cell's payload without decoding it (failure path).
fn discard_out_cell(cell: OutCell) {
    match cell {
        OutCell::Bstr(b) => {
            if !(*b).is_null() {
                // SAFETY: on the (rare) path where a failing callee still wrote a
                // retval BSTR, ownership transferred to us; free it once.
                unsafe { SysFreeString(*b) };
            }
        }
        OutCell::Variant(mut b) => {
            // SAFETY: clears any payload a failing callee wrote into the cell.
            unsafe {
                let _ = VariantClear(b.as_mut());
            }
        }
        OutCell::Interface(b) => {
            if !(*b).is_null() {
                // SAFETY: a transferred interface reference is released via its
                // own IUnknown::Release (vtable slot 2).
                unsafe {
                    let unknown = (*b).cast::<*const *const c_void>();
                    let release = *(*unknown).add(2);
                    let release: unsafe extern "system" fn(*mut c_void) -> u32 =
                        std::mem::transmute(release);
                    let _ = release(*b);
                }
            }
        }
        OutCell::SafeArray(b) => {
            if !(*b).is_null() {
                // SAFETY: a failing callee unexpectedly transferred a SAFEARRAY
                // retval. Wrap it in a VARIANT and clear once to destroy it.
                unsafe {
                    let mut variant: VARIANT = std::mem::zeroed();
                    variant.Anonymous.Anonymous.vt = VT_ARRAY | VT_VARIANT;
                    variant.Anonymous.Anonymous.Anonymous.parray = *b;
                    let _ = VariantClear(&mut variant);
                }
            }
        }
        OutCell::Record(record) => drop(record),
        _ => {}
    }
}

/// Decode the populated retval cell into a [`Variant`] (success path). Owns any
/// transferred BSTR (freed via [`bstr_to_string_and_free`]) / interface (bound
/// via `bind_dispatch_result`, which takes the transferred reference) / VARIANT.
/// Must only be called after a success HRESULT, so the cell is populated per the
/// member's declared return type; internal FFI reads run under documented
/// `unsafe` blocks.
fn decode_out_cell<FBindDispatch>(
    cell: OutCell,
    label: &'static str,
    dispid: i32,
    bind_dispatch_result: &mut FBindDispatch,
) -> Result<Variant, ComInvokeFailure>
where
    FBindDispatch: FnMut(*mut c_void) -> Result<Variant, String>,
{
    Ok(match cell {
        OutCell::None => Variant::empty(),
        OutCell::I32(b) => Variant::from_i32(*b),
        OutCell::I16(b) => Variant::from_i16(*b),
        OutCell::U8(b) => Variant::from_u8(*b),
        // VARIANT_BOOL: any non-zero is true (VBA convention writes -1 for true).
        OutCell::Bool(b) => Variant::from_bool(*b != 0),
        OutCell::I64(b) => Variant::from_i64(*b),
        OutCell::Currency(b) => Variant::from_currency_scaled_i64(*b),
        OutCell::Decimal(b) => Variant::from_decimal96(decimal96_from_windows(b.as_ref())),
        OutCell::F64(b) => Variant::from_f64(*b),
        // A VT_DATE retval decodes to a Date Variant (not a plain Double).
        OutCell::Date(b) => Variant::from_date_f64(*b),
        OutCell::F32(b) => Variant::from_f64(f64::from(*b)),
        OutCell::Bstr(b) => {
            // Callee transferred ownership of the retval BSTR; take + free it.
            // SAFETY: `*b` is the BSTR the callee wrote (or null); ownership
            // transferred to us, so freeing it exactly once is correct.
            let text = unsafe { bstr_to_string_and_free(*b) }.unwrap_or_default();
            Variant::from_string(text)
        }
        OutCell::Variant(mut b) => {
            // SAFETY: on the success path the callee populated this VARIANT per its
            // declared retval type; reading its discriminant `vt` is always valid.
            let vt = unsafe { b.Anonymous.Anonymous.vt };
            if vt == VT_DISPATCH || vt == VT_UNKNOWN {
                // An object returned INSIDE a VARIANT (`As Object` → VT_DISPATCH) must be
                // bound through the bindings map, NOT read as a scalar — `variant_to_com_value`
                // does not register the object. The callee's [out] VARIANT owns one
                // reference; AddRef a SECOND (which we transfer to `bind_dispatch_result`,
                // matching the OutCell::Interface ownership contract) and let VariantClear
                // below release the callee's original — net-balanced.
                //
                // KNOWN LATENT AV (VT_UNKNOWN only, narrow, deferred — see code review): the
                // `add_ref_dispatch` cast and `bind_dispatch_result` store the raw pointer as
                // the binding's `native_dispatch` WITHOUT QI'ing it for IDispatch (unlike the
                // proven IDispatch result path's `query_dispatch_from_unknown`). Object
                // IDENTITY is fine (`native_unknown` is derived via `QI(IUnknown)`, valid on a
                // bare IUnknown), but a LATER late-bound member call on a VT_UNKNOWN payload
                // that does NOT implement IDispatch would invoke IDispatch vtable slots on a
                // 3-slot IUnknown vtable → host AV. Automation duals alias IUnknown==IDispatch
                // so the common path is safe. The correct fix is to carry an "is-dispatchable"
                // flag on the binding and raise a clean runtime error (not an AV) at the
                // member-dispatch boundary — binding-model work pending live verification, NOT
                // a blind QI-or-Nothing here (that would regress pure-IUnknown identity).
                // SAFETY: the VARIANT's payload pointer is the object the callee wrote.
                let object_ptr = unsafe {
                    if vt == VT_DISPATCH {
                        b.Anonymous.Anonymous.Anonymous.pdispVal.cast::<c_void>()
                    } else {
                        b.Anonymous.Anonymous.Anonymous.punkVal.cast::<c_void>()
                    }
                };
                if !object_ptr.is_null() {
                    // SAFETY: `object_ptr` is the live object the callee placed in the
                    // VARIANT; AddRef gives us the reference we hand to the binding map.
                    unsafe {
                        let _ = crate::add_ref_dispatch(object_ptr.cast::<crate::RawIDispatch>());
                    }
                }
                // A null object pointer binds to Nothing, exactly as the OutCell::Interface
                // path handles a null `[out,retval]` interface.
                let bound = bind_dispatch_result(object_ptr)
                    .map_err(|detail| validation_failure(label, dispid, detail));
                // SAFETY: clear the cell once, releasing the callee's original reference.
                unsafe {
                    let _ = VariantClear(b.as_mut());
                }
                bound?
            } else {
                // SAFETY: on the success path the callee populated this VARIANT per its
                // declared retval type; variant_to_com_value only reads it.
                let value = unsafe { crate::variant_to_com_value(b.as_ref()) }
                    .and_then(|value| value.to_variant())
                    .map_err(|detail| validation_failure(label, dispid, detail));
                // SAFETY: take ownership of any payload the callee wrote, then clear
                // the cell exactly once.
                unsafe {
                    let _ = VariantClear(b.as_mut());
                }
                value?
            }
        }
        OutCell::Interface(b) => {
            // Callee AddRef'd the returned interface; we own that reference and
            // hand it to the binding map via bind_dispatch_result.
            bind_dispatch_result(*b).map_err(|detail| validation_failure(label, dispid, detail))?
        }
        OutCell::SafeArray(b) => {
            if (*b).is_null() {
                return Err(validation_failure(
                    label,
                    dispid,
                    "vtable SAFEARRAY(VARIANT) retval returned null SAFEARRAY",
                ));
            }
            // SAFETY: the successful callee transferred ownership of the
            // SAFEARRAY* through the `[out,retval]` cell. A temporary VARIANT
            // lets the existing Windows SAFEARRAY reader clone it into the
            // runtime carrier, then VariantClear destroys the COM-owned array.
            unsafe {
                let mut variant: VARIANT = std::mem::zeroed();
                variant.Anonymous.Anonymous.vt = VT_ARRAY | VT_VARIANT;
                variant.Anonymous.Anonymous.Anonymous.parray = *b;
                let value = crate::variant_to_com_value(&variant)
                    .and_then(|value| value.to_variant())
                    .map_err(|detail| validation_failure(label, dispid, detail));
                let _ = VariantClear(&mut variant);
                value?
            }
        }
        OutCell::Record(record) => Variant::from_com_record(record.clone()),
    })
}

fn decimal96_from_windows(decimal: &DECIMAL) -> Decimal96 {
    // SAFETY: `decimal` is the initialized DECIMAL cell written by the COM
    // callee on a successful HRESULT. Reading the documented DECIMAL union
    // views mirrors the existing VARIANT conversion path.
    unsafe {
        Decimal96::from_scale_sign(
            decimal.Anonymous2.Anonymous.Lo32,
            decimal.Anonymous2.Anonymous.Mid32,
            decimal.Hi32,
            (u16::from(decimal.Anonymous1.Anonymous.sign) << 8)
                | u16::from(decimal.Anonymous1.Anonymous.scale),
        )
    }
}

fn decimal96_to_windows(value: Decimal96) -> DECIMAL {
    // SAFETY: zeroed DECIMAL is a valid zero value; the fields below are then
    // filled with the runtime Decimal96 parts using the documented layout.
    unsafe {
        let mut decimal: DECIMAL = std::mem::zeroed();
        decimal.wReserved = 0;
        decimal.Anonymous1.Anonymous.scale = value.scale();
        decimal.Anonymous1.Anonymous.sign = if value.is_negative() { 0x80 } else { 0 };
        decimal.Hi32 = value.hi;
        decimal.Anonymous2.Anonymous.Lo32 = value.lo;
        decimal.Anonymous2.Anonymous.Mid32 = value.mid;
        decimal
    }
}

// ── ComValue → scalar conversions (shared shapes with the dynlink path) ──

fn com_value_to_i32(value: &ComValue, what: &str) -> Result<i32, String> {
    match value {
        ComValue::I32(v) => Ok(*v),
        ComValue::I64(v) => i32::try_from(*v)
            .map_err(|_| format!("vtable {what} parameter value {v} exceeds i32 range")),
        ComValue::Bool(v) => Ok(if *v { -1 } else { 0 }),
        other => Err(format!(
            "vtable {what} parameter expects an integer, got {other:?}"
        )),
    }
}

fn com_value_to_i16(value: &ComValue, what: &str) -> Result<i16, String> {
    let as_i32 = com_value_to_i32(value, what)?;
    i16::try_from(as_i32)
        .map_err(|_| format!("vtable {what} parameter value {as_i32} exceeds i16 range"))
}

fn com_value_to_u8(value: &ComValue) -> Result<u8, String> {
    let as_i32 = com_value_to_i32(value, "VT_UI1")?;
    u8::try_from(as_i32)
        .map_err(|_| format!("vtable VT_UI1 parameter value {as_i32} exceeds u8 range"))
}

fn com_value_to_i64(value: &ComValue) -> Result<i64, String> {
    match value {
        ComValue::I32(v) => Ok(i64::from(*v)),
        ComValue::I64(v) => Ok(*v),
        ComValue::U64(v) => i64::try_from(*v)
            .map_err(|_| format!("vtable VT_I8 parameter value {v} exceeds i64 range")),
        other => Err(format!(
            "vtable VT_I8 parameter expects an integer, got {other:?}"
        )),
    }
}

fn com_value_to_f64(value: &ComValue) -> Result<f64, String> {
    match value {
        ComValue::F64(v) => Ok(v.as_f64()),
        ComValue::I32(v) => Ok(f64::from(*v)),
        ComValue::I64(v) => Ok(*v as f64),
        other => Err(format!(
            "vtable floating-point parameter expects a number, got {other:?}"
        )),
    }
}

fn com_value_to_currency_i64(value: &ComValue) -> Result<i64, String> {
    match value {
        ComValue::Currency(v) => Ok(v.scaled_i64()),
        ComValue::I32(v) => Ok(i64::from(*v) * 10_000),
        ComValue::I64(v) => Ok(v.saturating_mul(10_000)),
        other => Err(format!(
            "vtable VT_CY parameter expects a currency/integer, got {other:?}"
        )),
    }
}

fn com_value_to_decimal(value: &ComValue) -> Result<Decimal96, String> {
    match value {
        ComValue::Decimal(v) => Ok(*v),
        other => Err(format!(
            "vtable VT_DECIMAL parameter expects a Decimal, got {other:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows_test_dispatch::{
        DUAL_BYTE_VALUE, DUAL_CREATED_OLE_DATE, DUAL_DECIMAL_HI, DUAL_DECIMAL_LO, DUAL_DECIMAL_MID,
        DUAL_DECIMAL_NEGATIVE, DUAL_DECIMAL_SCALE, DUAL_DOUBLE_VALUE, DUAL_INTEGER_VALUE,
        DUAL_LONGLONG_VALUE, DUAL_PRICE_SCALED_I64, DUAL_RAISE_ERROR_DESCRIPTION,
        DUAL_RAISE_ERROR_SOURCE, DUAL_RECORD_MUTATED_VALUE, DUAL_RECORD_RETURN_VALUE,
        DUAL_RECORD_VALUE, DUAL_SINGLE_VALUE, DUAL_SLOT_EXISTS, DUAL_SLOT_GET_BYTE_VALUE,
        DUAL_SLOT_GET_COUNT, DUAL_SLOT_GET_CREATED, DUAL_SLOT_GET_DECIMAL_VALUE,
        DUAL_SLOT_GET_DOUBLE_VALUE, DUAL_SLOT_GET_INTEGER_VALUE, DUAL_SLOT_GET_LONGLONG_VALUE,
        DUAL_SLOT_GET_OWNER, DUAL_SLOT_GET_PRICE, DUAL_SLOT_GET_RECORD_VALUE,
        DUAL_SLOT_GET_SAFEARRAY_VALUE, DUAL_SLOT_GET_SINGLE_VALUE, DUAL_SLOT_GET_TEXT_VALUE,
        DUAL_SLOT_GET_VARIANT_VALUE, DUAL_SLOT_LOOKUP, DUAL_SLOT_MUTATE_BYREF_BREADTH,
        DUAL_SLOT_MUTATE_BYREF_LONG, DUAL_SLOT_MUTATE_BYREF_OBJECT_STRING_ARRAY,
        DUAL_SLOT_MUTATE_BYREF_RECORD, DUAL_SLOT_PUT_VALUE, DUAL_SLOT_PUTREF_OBJECT_VALUE,
        DUAL_SLOT_RAISE_ERROR, DUAL_SLOT_VALIDATE_ALL_INPUTS, DUAL_SLOT_VALIDATE_RECORD_VALUE,
        DUAL_SLOT_VALIDATE_SAFEARRAY_VALUE, DUAL_TEXT_VALUE, DUAL_VARIANT_VALUE,
        create_oxvba_dual_vtable_object, create_oxvba_test_dispatch,
    };
    use crate::{TypeLibMemberInvokeKind, TypeLibWireType};
    use oxvba_runtime::ComRecord;
    use oxvba_runtime::safe_array::SafeArray;
    use oxvba_runtime::{RuntimeByRefSlot, RuntimeValueType, VarType};

    /// Resolver that never sees an object arg in these tests.
    fn no_object_resolver() -> impl FnMut(ObjectRef) -> Result<*mut c_void, String> {
        |_object| Err("no object arguments in this test".to_string())
    }

    /// Binds a Lookup retval interface: we own the AddRef'd reference, so release
    /// it (no real bindings map in the unit test) and surface a sentinel object
    /// Variant so the test can assert a non-Nothing object was returned.
    fn release_and_bind() -> impl FnMut(*mut c_void) -> Result<Variant, String> {
        |dispatch| {
            if dispatch.is_null() {
                return Ok(Variant::from_object_ref(ObjectRef::from_compat_identity(0)));
            }
            // SAFETY: the [out,retval] convention transferred one reference to us;
            // release it exactly once.
            unsafe {
                crate::release_dispatch(dispatch.cast::<crate::RawIDispatch>());
            }
            Ok(Variant::from_object_ref(ObjectRef::from_compat_identity(
                99,
            )))
        }
    }

    /// Release the fixture object's single reference (slot 2 = IUnknown::Release).
    ///
    /// # Safety
    /// `this` must be a live dual-vtable fixture object holding the reference
    /// being released.
    unsafe fn release_dual(this: *mut c_void) {
        // SAFETY: `this` is the fixture object; its first field is the vtable,
        // slot 2 is IUnknown::Release, which we call once to drop our reference.
        unsafe {
            let vtbl = *this.cast::<*const *const c_void>();
            let release = *vtbl.add(2);
            let release: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(release);
            let _ = release(this);
        }
    }

    fn idispatch_iid() -> crate::ComInterfaceIid {
        crate::ComInterfaceIid {
            data1: 0x0002_0400,
            data2: 0,
            data3: 0,
            data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
        }
    }

    fn object_resolver_for(
        expected: ObjectRef,
        dispatch: *mut crate::RawIDispatch,
    ) -> impl FnMut(ObjectRef) -> Result<*mut c_void, String> {
        move |object| {
            if object == expected {
                Ok(dispatch.cast::<c_void>())
            } else {
                Err(format!("unexpected object handle {:?}", object.raw()))
            }
        }
    }

    fn invocation_plan(
        slot: u16,
        parameter_types: Vec<TypeLibParamType>,
        parameter_wire_types: Vec<TypeLibWireType>,
        parameter_iids: Vec<Option<crate::ComInterfaceIid>>,
        return_type: Option<TypeLibParamType>,
        return_wire_type: Option<TypeLibWireType>,
        invoke_kind: TypeLibMemberInvokeKind,
    ) -> VtableInvocationPlan {
        let label = match invoke_kind {
            TypeLibMemberInvokeKind::PropertyGet => "property-get",
            TypeLibMemberInvokeKind::Method => "method",
            TypeLibMemberInvokeKind::PropertyPut => "property-put",
            TypeLibMemberInvokeKind::PropertyPutRef => "property-putref",
        };
        VtableInvocationPlan {
            slot,
            slot_bound: slot.saturating_add(1),
            interface_iid: crate::ComInterfaceIid {
                data1: 0x1234_5678,
                data2: 0x1234,
                data3: 0x5678,
                data4: [1, 2, 3, 4, 5, 6, 7, 8],
            },
            parameter_types,
            parameter_wire_types,
            parameter_iids,
            parameter_byref_slots: vec![],
            return_type,
            return_wire_type,
            invoke_kind,
            label,
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct TestRecord {
        value: i32,
    }

    unsafe fn clone_test_record(
        record_info: *mut c_void,
        record_data: *const c_void,
    ) -> Result<(*mut c_void, *mut c_void), String> {
        if record_info.is_null() || record_data.is_null() {
            return Err("test record clone received a null record pointer".to_string());
        }
        let value = unsafe { *record_data.cast::<TestRecord>() };
        Ok((record_info, Box::into_raw(Box::new(value)).cast::<c_void>()))
    }

    unsafe fn destroy_test_record(_record_info: *mut c_void, record_data: *mut c_void) {
        if !record_data.is_null() {
            unsafe {
                drop(Box::from_raw(record_data.cast::<TestRecord>()));
            }
        }
    }

    fn test_record_variant(value: i32) -> Variant {
        static RECORD_INFO_SENTINEL: u8 = 0;
        let data = Box::into_raw(Box::new(TestRecord { value })).cast::<c_void>();
        let info = (&RECORD_INFO_SENTINEL as *const u8)
            .cast_mut()
            .cast::<c_void>();
        let record = unsafe {
            ComRecord::from_raw_parts(info, data, clone_test_record, destroy_test_record)
        }
        .expect("test record pointers are non-null");
        Variant::from_com_record(record)
    }

    fn record_variant_value(value: &Variant) -> i32 {
        let record = value
            .as_com_record()
            .expect("writeback should contain a COM record");
        let ptr = record.record_data_ptr();
        assert!(!ptr.is_null(), "record data pointer should be non-null");
        // SAFETY: this helper only reads records created by `test_record_variant`
        // and mutated by the fixture's `DualRecordFixture` slot.
        unsafe { (*ptr.cast::<TestRecord>()).value }
    }

    #[repr(C)]
    struct TestIUnknownVtbl {
        query_interface: unsafe extern "system" fn(
            *mut c_void,
            *const windows_sys::core::GUID,
            *mut *mut c_void,
        ) -> i32,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
    }

    #[repr(C)]
    struct TestCreateTypeLib2Vtbl {
        query_interface: unsafe extern "system" fn(
            *mut c_void,
            *const windows_sys::core::GUID,
            *mut *mut c_void,
        ) -> i32,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        create_type_info:
            unsafe extern "system" fn(*mut c_void, *const u16, i32, *mut *mut c_void) -> i32,
        set_name: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
        set_version: unsafe extern "system" fn(*mut c_void, u16, u16) -> i32,
        set_guid: unsafe extern "system" fn(*mut c_void, *const windows_sys::core::GUID) -> i32,
        set_doc_string: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
        set_help_file_name: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
        set_help_context: unsafe extern "system" fn(*mut c_void, u32) -> i32,
        set_lcid: unsafe extern "system" fn(*mut c_void, u32) -> i32,
        set_lib_flags: unsafe extern "system" fn(*mut c_void, u32) -> i32,
        save_all_changes: unsafe extern "system" fn(*mut c_void) -> i32,
    }

    #[repr(C)]
    struct TestCreateTypeInfoVtbl {
        query_interface: unsafe extern "system" fn(
            *mut c_void,
            *const windows_sys::core::GUID,
            *mut *mut c_void,
        ) -> i32,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        set_guid: unsafe extern "system" fn(*mut c_void, *const windows_sys::core::GUID) -> i32,
        set_type_flags: unsafe extern "system" fn(*mut c_void, u32) -> i32,
        set_doc_string: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
        set_help_context: unsafe extern "system" fn(*mut c_void, u32) -> i32,
        set_version: unsafe extern "system" fn(*mut c_void, u16, u16) -> i32,
        add_ref_type_info: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut u32) -> i32,
        add_func_desc: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *mut windows_sys::Win32::System::Com::FUNCDESC,
        ) -> i32,
        add_impl_type: unsafe extern "system" fn(*mut c_void, u32, u32) -> i32,
        set_impl_type_flags: unsafe extern "system" fn(*mut c_void, u32, i32) -> i32,
        set_alignment: unsafe extern "system" fn(*mut c_void, u16) -> i32,
        set_schema: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
        add_var_desc: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *mut windows_sys::Win32::System::Com::VARDESC,
        ) -> i32,
        set_func_and_param_names:
            unsafe extern "system" fn(*mut c_void, u32, *mut *mut u16, u32) -> i32,
        set_var_name: unsafe extern "system" fn(*mut c_void, u32, *const u16) -> i32,
        set_type_desc_alias: unsafe extern "system" fn(
            *mut c_void,
            *mut windows_sys::Win32::System::Com::TYPEDESC,
        ) -> i32,
        define_func_as_dll_entry:
            unsafe extern "system" fn(*mut c_void, u32, *const u16, *const u16) -> i32,
        set_func_doc_string: unsafe extern "system" fn(*mut c_void, u32, *const u16) -> i32,
        set_var_doc_string: unsafe extern "system" fn(*mut c_void, u32, *const u16) -> i32,
        set_func_help_context: unsafe extern "system" fn(*mut c_void, u32, u32) -> i32,
        set_var_help_context: unsafe extern "system" fn(*mut c_void, u32, u32) -> i32,
        set_mops: unsafe extern "system" fn(*mut c_void, u32, *const u16) -> i32,
        set_type_idldesc: unsafe extern "system" fn(
            *mut c_void,
            *mut windows_sys::Win32::System::Com::IDLDESC,
        ) -> i32,
        lay_out: unsafe extern "system" fn(*mut c_void) -> i32,
    }

    struct RegisteredRecordTypelib {
        descriptor: TypeLibRecordInfo,
        path: std::path::PathBuf,
    }

    impl Drop for RegisteredRecordTypelib {
        fn drop(&mut self) {
            let libid = self.descriptor.libid.to_guid();
            unsafe {
                let _ = windows_sys::Win32::System::Ole::UnRegisterTypeLibForUser(
                    &libid,
                    self.descriptor.major,
                    self.descriptor.minor,
                    self.descriptor.lcid,
                    windows_sys::Win32::System::Com::SYS_WIN64,
                );
            }
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn wide_path(path: &std::path::Path) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn check_test_hr(op: &'static str, hr: i32) -> Result<(), String> {
        if hr < 0 {
            Err(format!("{op} failed with HRESULT {:#010X}", hr as u32))
        } else {
            Ok(())
        }
    }

    unsafe fn test_vtbl<T>(ptr: *mut c_void) -> &'static T {
        unsafe { &**(ptr as *const *const T) }
    }

    unsafe fn release_test_com_ptr(ptr: *mut c_void) {
        if !ptr.is_null() {
            let vtbl = unsafe { test_vtbl::<TestIUnknownVtbl>(ptr) };
            unsafe {
                let _ = (vtbl.release)(ptr);
            }
        }
    }

    fn create_registered_record_typelib() -> Result<RegisteredRecordTypelib, String> {
        use windows_sys::Win32::System::Com::{
            ELEMDESC, ELEMDESC_0, IDLFLAG_NONE, SYS_WIN64, TKIND_RECORD, TYPEDESC, VAR_PERINSTANCE,
            VARDESC, VARDESC_0,
        };
        use windows_sys::Win32::System::Ole::{
            CreateTypeLib2, LoadTypeLib, RegisterTypeLibForUser,
        };
        use windows_sys::Win32::System::Variant::VT_I4;

        let libid = windows_sys::core::GUID {
            data1: 0x67E5_2026,
            data2: 0x0619,
            data3: 0x1001,
            data4: [0x90, 0x01, 0x10, 0x32, 0x54, 0x76, 0x98, 0x10],
        };
        let record_guid = windows_sys::core::GUID {
            data1: 0x67E5_2026,
            data2: 0x0619,
            data3: 0x1002,
            data4: [0x90, 0x01, 0x10, 0x32, 0x54, 0x76, 0x98, 0x11],
        };
        let path = std::env::temp_dir().join(format!(
            "oxvba-record-retval-{}-{}.tlb",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let path_w = wide_path(&path);
        let mut typelib: *mut c_void = std::ptr::null_mut();
        check_test_hr("CreateTypeLib2", unsafe {
            CreateTypeLib2(SYS_WIN64, path_w.as_ptr(), &mut typelib)
        })?;
        if typelib.is_null() {
            return Err("CreateTypeLib2 returned null".to_string());
        }

        let result = (|| {
            let lib_vtbl = unsafe { test_vtbl::<TestCreateTypeLib2Vtbl>(typelib) };
            let lib_name = wide("OxVbaRecordRetvalFixture");
            check_test_hr("ICreateTypeLib2::SetGuid", unsafe {
                (lib_vtbl.set_guid)(typelib, &libid)
            })?;
            check_test_hr("ICreateTypeLib2::SetName", unsafe {
                (lib_vtbl.set_name)(typelib, lib_name.as_ptr())
            })?;
            check_test_hr("ICreateTypeLib2::SetVersion", unsafe {
                (lib_vtbl.set_version)(typelib, 1, 0)
            })?;
            check_test_hr("ICreateTypeLib2::SetLcid", unsafe {
                (lib_vtbl.set_lcid)(typelib, 0)
            })?;

            let record_name = wide("RecordRetvalFixture");
            let mut create_info: *mut c_void = std::ptr::null_mut();
            check_test_hr("ICreateTypeLib2::CreateTypeInfo", unsafe {
                (lib_vtbl.create_type_info)(
                    typelib,
                    record_name.as_ptr(),
                    TKIND_RECORD,
                    &mut create_info,
                )
            })?;
            if create_info.is_null() {
                return Err("CreateTypeInfo returned null".to_string());
            }

            let create_result = (|| {
                let ti_vtbl = unsafe { test_vtbl::<TestCreateTypeInfoVtbl>(create_info) };
                check_test_hr("ICreateTypeInfo::SetGuid", unsafe {
                    (ti_vtbl.set_guid)(create_info, &record_guid)
                })?;
                check_test_hr("ICreateTypeInfo::SetAlignment", unsafe {
                    (ti_vtbl.set_alignment)(create_info, 4)
                })?;
                let mut vardesc = VARDESC {
                    memid: 1,
                    lpstrSchema: std::ptr::null_mut(),
                    Anonymous: VARDESC_0 { oInst: 0 },
                    elemdescVar: ELEMDESC {
                        tdesc: TYPEDESC {
                            Anonymous: unsafe { std::mem::zeroed() },
                            vt: VT_I4,
                        },
                        Anonymous: ELEMDESC_0 {
                            idldesc: windows_sys::Win32::System::Com::IDLDESC {
                                dwReserved: 0,
                                wIDLFlags: IDLFLAG_NONE,
                            },
                        },
                    },
                    wVarFlags: 0,
                    varkind: VAR_PERINSTANCE,
                };
                check_test_hr("ICreateTypeInfo::AddVarDesc", unsafe {
                    (ti_vtbl.add_var_desc)(create_info, 0, &mut vardesc)
                })?;
                let value_name = wide("Value");
                check_test_hr("ICreateTypeInfo::SetVarName", unsafe {
                    (ti_vtbl.set_var_name)(create_info, 0, value_name.as_ptr())
                })?;
                check_test_hr("ICreateTypeInfo::LayOut", unsafe {
                    (ti_vtbl.lay_out)(create_info)
                })
            })();
            unsafe { release_test_com_ptr(create_info) };
            create_result?;

            check_test_hr("ICreateTypeLib2::SaveAllChanges", unsafe {
                (lib_vtbl.save_all_changes)(typelib)
            })?;
            Ok::<(), String>(())
        })();
        unsafe { release_test_com_ptr(typelib) };
        if let Err(err) = result {
            let _ = std::fs::remove_file(&path);
            return Err(err);
        }

        let mut loaded: *mut c_void = std::ptr::null_mut();
        if let Err(err) = check_test_hr("LoadTypeLib", unsafe {
            LoadTypeLib(path_w.as_ptr(), &mut loaded)
        }) {
            let _ = std::fs::remove_file(&path);
            return Err(err);
        }
        if loaded.is_null() {
            let _ = std::fs::remove_file(&path);
            return Err("LoadTypeLib returned null".to_string());
        }
        let register_result = check_test_hr("RegisterTypeLibForUser", unsafe {
            RegisterTypeLibForUser(loaded, path_w.as_ptr(), std::ptr::null())
        });
        unsafe { release_test_com_ptr(loaded) };
        if let Err(err) = register_result {
            let _ = std::fs::remove_file(&path);
            return Err(err);
        }

        Ok(RegisteredRecordTypelib {
            descriptor: TypeLibRecordInfo {
                libid: crate::ComInterfaceIid::from_guid(&libid),
                major: 1,
                minor: 0,
                lcid: 0,
                type_guid: crate::ComInterfaceIid::from_guid(&record_guid),
            },
            path,
        })
    }

    #[test]
    fn get_count_first_light_returns_expected_i32() {
        // FIRST LIGHT: isolates this-ptr + out-cell + HRESULT + oVft/8 slot
        // index. If the slot index were wrong this would crash the host, so it
        // must pass before any BSTR/VARIANT/interface complexity is trusted.
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_GET_COUNT,
            vec![],
            vec![],
            vec![],
            Some(TypeLibParamType::Long),
            None,
            TypeLibMemberInvokeKind::PropertyGet,
        );
        // SAFETY: `this` is a live dual-vtable fixture object with the known
        // slot layout; get_Count is slot 7 with retval Long.
        let result = unsafe { vtable_invoke(this, &plan, &[], 1, &mut resolve, &mut bind) };
        let value = result.expect("get_Count should succeed");
        assert_eq!(
            value.as_i32(),
            Some(7),
            "get_Count out-cell must decode to 7"
        );
        // SAFETY: balances the single reference create_* handed us.
        unsafe { release_dual(this) };
    }

    #[test]
    fn exists_returns_boolean_out_cell() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_EXISTS,
            vec![TypeLibParamType::Long],
            vec![],
            vec![],
            Some(TypeLibParamType::Boolean),
            None,
            TypeLibMemberInvokeKind::Method,
        );
        // SAFETY: Exists is slot 8: (i32 key) -> VARIANT_BOOL retval.
        let yes = unsafe {
            vtable_invoke(
                this,
                &plan,
                &[Variant::from_i32(42)],
                2,
                &mut resolve,
                &mut bind,
            )
        }
        .expect("Exists(42) should succeed");
        assert_eq!(yes.as_bool(), Some(true), "Exists(42) is true");

        // SAFETY: same fixture object, same slot/signature.
        let no = unsafe {
            vtable_invoke(
                this,
                &plan,
                &[Variant::from_i32(7)],
                2,
                &mut resolve,
                &mut bind,
            )
        }
        .expect("Exists(7) should succeed");
        assert_eq!(no.as_bool(), Some(false), "Exists(7) is false");
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn put_value_round_trips_a_variant() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_PUT_VALUE,
            vec![TypeLibParamType::Variant],
            vec![],
            vec![],
            None,
            None,
            TypeLibMemberInvokeKind::PropertyPut,
        );
        // SAFETY: put_Value is slot 9: ([in] VARIANT*) -> HRESULT, no retval.
        let result = unsafe {
            vtable_invoke(
                this,
                &plan,
                &[Variant::from_i32(1234)],
                3,
                &mut resolve,
                &mut bind,
            )
        };
        // No retval → Empty Variant on success; the success HRESULT proves the
        // VARIANT marshalled in and the server accepted it (VT_I4 1234).
        let value = result.expect("put_Value should succeed");
        assert_eq!(
            value.vtype(),
            VarType::Empty,
            "no-retval member returns Empty"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn byref_safearray_parameter_requires_writeback_capable_path() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let mut plan = invocation_plan(
            DUAL_SLOT_PUT_VALUE,
            vec![TypeLibParamType::Variant],
            vec![TypeLibWireType::ByRefSafeArrayVariant],
            vec![],
            None,
            None,
            TypeLibMemberInvokeKind::PropertyPut,
        );
        plan.parameter_byref_slots = vec![Some(RuntimeByRefSlot::new(
            0,
            Some(RuntimeValueType::Variant),
        ))];
        // SAFETY: `this` is a live fixture object. The value-only wrapper must
        // reject writeback-capable plans before slot execution.
        let failure = unsafe {
            vtable_invoke(
                this,
                &plan,
                &[Variant::from_safearray(SafeArray::from_variants(vec![
                    Variant::from_i32(1234),
                ]))],
                300,
                &mut resolve,
                &mut bind,
            )
        }
        .expect_err("ByRef SAFEARRAY requires writeback-capable vtable invoke");
        assert_eq!(
            failure.hr, None,
            "value-only writeback guard is a validation fallback"
        );
        assert!(
            failure
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("writeback-capable invoke path")),
            "failure should identify the writeback-capable invoke requirement"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn unsupported_byref_safearray_return_wire_shape_declines_before_slot_call() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_GET_COUNT,
            vec![],
            vec![],
            vec![],
            Some(TypeLibParamType::Variant),
            Some(TypeLibWireType::ByRefSafeArrayVariant),
            TypeLibMemberInvokeKind::PropertyGet,
        );
        // SAFETY: `this` is a live fixture object. The unsupported SAFEARRAY return
        // wire shape must be rejected as a validation failure before the slot is read/called.
        let failure = unsafe { vtable_invoke(this, &plan, &[], 301, &mut resolve, &mut bind) }
            .expect_err("unsupported return wire shape must decline");
        assert_eq!(
            failure.hr, None,
            "wire-shape decline is a validation fallback"
        );
        assert!(
            failure
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("return wire shape")),
            "failure should identify the unsupported return wire shape"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn safearray_parameter_lowers_to_explicit_safearray_wire_shape() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_VALIDATE_SAFEARRAY_VALUE,
            vec![TypeLibParamType::Variant],
            vec![TypeLibWireType::SafeArrayVariant],
            vec![],
            Some(TypeLibParamType::Boolean),
            Some(TypeLibWireType::Automation(TypeLibParamType::Boolean)),
            TypeLibMemberInvokeKind::Method,
        );
        let arg = Variant::from_safearray(SafeArray::from_variants(vec![
            Variant::from_i32(3),
            Variant::from_i32(5),
            Variant::from_i32(8),
        ]));
        // SAFETY: slot 24 validates the inbound SAFEARRAY(VARIANT)* payload.
        let value = unsafe { vtable_invoke(this, &plan, &[arg], 24, &mut resolve, &mut bind) }
            .expect("SAFEARRAY inbound vtable call should succeed");
        assert_eq!(
            value.as_bool(),
            Some(true),
            "fixture must see the array values through the SAFEARRAY wire pointer"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn record_parameter_lowers_to_record_data_pointer() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_VALIDATE_RECORD_VALUE,
            vec![TypeLibParamType::Record],
            vec![TypeLibWireType::Record {
                name: "TestLib.Point".to_string(),
                record_info: None,
            }],
            vec![None],
            Some(TypeLibParamType::Boolean),
            Some(TypeLibWireType::Automation(TypeLibParamType::Boolean)),
            TypeLibMemberInvokeKind::Method,
        );
        // SAFETY: slot 31 reads the inbound typed record pointer and writes a
        // VARIANT_BOOL retval; the ComRecord carrier owns the borrowed data for
        // the full duration of the libffi call.
        let value = unsafe {
            vtable_invoke(
                this,
                &plan,
                &[test_record_variant(DUAL_RECORD_VALUE)],
                31,
                &mut resolve,
                &mut bind,
            )
        }
        .expect("record inbound vtable call should succeed");
        assert_eq!(
            value.as_bool(),
            Some(true),
            "fixture must see the typed record data pointer"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn byref_record_parameter_returns_mutated_record_writeback() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let slot = RuntimeByRefSlot::new(0, Some(RuntimeValueType::Record));
        let mut plan = invocation_plan(
            DUAL_SLOT_MUTATE_BYREF_RECORD,
            vec![TypeLibParamType::ByRefRecord],
            vec![TypeLibWireType::ByRefRecord {
                name: "TestLib.Point".to_string(),
                record_info: None,
            }],
            vec![None],
            None,
            None,
            TypeLibMemberInvokeKind::Method,
        );
        plan.parameter_byref_slots = vec![Some(slot)];
        let result = unsafe {
            vtable_invoke_with_writebacks(
                this,
                &plan,
                &[test_record_variant(DUAL_RECORD_VALUE)],
                32,
                &mut resolve,
                &mut bind,
            )
        }
        .expect("ByRef record vtable call should succeed");
        assert_eq!(result.value.vtype(), VarType::Empty);
        assert_eq!(result.writebacks.len(), 1);
        assert_eq!(result.writebacks[0].slot, slot);
        assert_eq!(
            record_variant_value(&result.writebacks[0].value),
            DUAL_RECORD_MUTATED_VALUE,
            "fixture mutation should be returned through the ByRef record writeback"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn record_return_allocates_descriptor_backed_record_cell() {
        let registered_record =
            create_registered_record_typelib().expect("temp record typelib should register");
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_GET_RECORD_VALUE,
            vec![],
            vec![],
            vec![],
            Some(TypeLibParamType::Record),
            Some(TypeLibWireType::Record {
                name: "OxVbaRecordRetvalFixture.RecordRetvalFixture".to_string(),
                record_info: Some(registered_record.descriptor.clone()),
            }),
            TypeLibMemberInvokeKind::PropertyGet,
        );
        // SAFETY: slot 33 writes into the caller-owned record payload allocated
        // from the descriptor-backed IRecordInfo source.
        let value = unsafe { vtable_invoke(this, &plan, &[], 33, &mut resolve, &mut bind) }
            .expect("record retval vtable call should succeed");
        assert_eq!(
            record_variant_value(&value),
            DUAL_RECORD_RETURN_VALUE,
            "record retval should decode the record cell populated by the slot"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn safearray_return_decodes_transferred_safearray_pointer() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_GET_SAFEARRAY_VALUE,
            vec![],
            vec![],
            vec![],
            Some(TypeLibParamType::Variant),
            Some(TypeLibWireType::SafeArrayVariant),
            TypeLibMemberInvokeKind::PropertyGet,
        );
        // SAFETY: slot 25 returns an owned SAFEARRAY* through an out pointer.
        let value = unsafe { vtable_invoke(this, &plan, &[], 25, &mut resolve, &mut bind) }
            .expect("SAFEARRAY retval vtable call should succeed");
        let array = value
            .as_safearray()
            .expect("SAFEARRAY retval should decode to an array Variant");
        let elements = array
            .variant_elements()
            .expect("fixture array should expose element values");
        assert_eq!(
            elements,
            vec![
                Variant::from_i32(13),
                Variant::from_i32(21),
                Variant::from_i32(34),
            ],
            "SAFEARRAY retval values must survive COM ownership transfer"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn decimal_return_decodes_decimal_out_cell() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_GET_DECIMAL_VALUE,
            vec![],
            vec![],
            vec![],
            Some(TypeLibParamType::Decimal),
            Some(TypeLibWireType::Automation(TypeLibParamType::Decimal)),
            TypeLibMemberInvokeKind::PropertyGet,
        );
        // SAFETY: slot 26 is a no-arg DECIMAL retval getter.
        let value = unsafe { vtable_invoke(this, &plan, &[], 26, &mut resolve, &mut bind) }
            .expect("Decimal retval vtable call should succeed");
        assert_eq!(
            value.as_decimal96(),
            Some(Decimal96::from_parts(
                DUAL_DECIMAL_LO,
                DUAL_DECIMAL_MID,
                DUAL_DECIMAL_HI,
                DUAL_DECIMAL_SCALE,
                DUAL_DECIMAL_NEGATIVE
            )),
            "DECIMAL retval must decode to the runtime Decimal96 carrier"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn object_parameter_without_declared_iid_declines_before_resolution() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve =
            |_object| panic!("object resolver must not run before missing-IID validation");
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_PUT_VALUE,
            vec![TypeLibParamType::Object],
            vec![TypeLibWireType::InterfacePointer {
                name: "ITestDispatch".to_string(),
            }],
            vec![],
            None,
            None,
            TypeLibMemberInvokeKind::PropertyPut,
        );
        // SAFETY: `this` is a live fixture object. The missing object-parameter IID
        // must be rejected as a validation failure before the slot is read/called.
        let failure = unsafe {
            vtable_invoke(
                this,
                &plan,
                &[Variant::from_object_ref(ObjectRef::from_compat_identity(
                    123,
                ))],
                302,
                &mut resolve,
                &mut bind,
            )
        }
        .expect_err("object parameter without IID must decline");
        assert_eq!(
            failure.hr, None,
            "missing-IID decline is a validation fallback"
        );
        assert!(
            failure
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("declared interface IID")),
            "failure should identify the missing object parameter IID"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn unsupported_semantic_type_declines_before_slot_read() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            u16::MAX,
            vec![TypeLibParamType::ByRefLong],
            vec![TypeLibWireType::Automation(TypeLibParamType::ByRefLong)],
            vec![],
            None,
            None,
            TypeLibMemberInvokeKind::Method,
        );
        // SAFETY: `this` is live, but the slot is intentionally invalid. The
        // unsupported semantic ByRef shape must be rejected before any vtable
        // slot read, so this must return a validation failure rather than AV.
        let failure = unsafe {
            vtable_invoke(
                this,
                &plan,
                &[Variant::from_i32(7)],
                303,
                &mut resolve,
                &mut bind,
            )
        }
        .expect_err("unsupported semantic type must decline before slot read");
        assert_eq!(
            failure.hr, None,
            "unsupported semantic type decline is a validation fallback"
        );
        assert!(
            failure
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("ByRefLong")),
            "failure should identify the unsupported semantic type"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn byref_long_writeback_is_returned_from_writeback_capable_path() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let slot = RuntimeByRefSlot::new(0, Some(RuntimeValueType::Long));
        let mut plan = invocation_plan(
            DUAL_SLOT_MUTATE_BYREF_LONG,
            vec![TypeLibParamType::ByRefLong],
            vec![TypeLibWireType::Automation(TypeLibParamType::ByRefLong)],
            vec![None],
            None,
            None,
            TypeLibMemberInvokeKind::Method,
        );
        plan.parameter_byref_slots = vec![Some(slot)];
        let result = unsafe {
            vtable_invoke_with_writebacks(
                this,
                &plan,
                &[Variant::from_i32(7)],
                28,
                &mut resolve,
                &mut bind,
            )
        }
        .expect("ByRef Long vtable call should succeed");
        assert_eq!(result.value.vtype(), VarType::Empty);
        assert_eq!(result.writebacks.len(), 1);
        assert_eq!(result.writebacks[0].slot, slot);
        assert_eq!(result.writebacks[0].value.as_i32(), Some(1_007));
        unsafe { release_dual(this) };
    }

    #[test]
    fn byref_scalar_decimal_and_variant_breadth_returns_writebacks() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let parameter_types = vec![
            TypeLibParamType::ByRefInteger,
            TypeLibParamType::ByRefByte,
            TypeLibParamType::ByRefBoolean,
            TypeLibParamType::ByRefLongLong,
            TypeLibParamType::ByRefSingle,
            TypeLibParamType::ByRefDouble,
            TypeLibParamType::ByRefCurrency,
            TypeLibParamType::ByRefDate,
            TypeLibParamType::ByRefDecimal,
            TypeLibParamType::ByRefVariant,
        ];
        let mut plan = invocation_plan(
            DUAL_SLOT_MUTATE_BYREF_BREADTH,
            parameter_types.clone(),
            parameter_types
                .iter()
                .copied()
                .map(TypeLibWireType::Automation)
                .collect(),
            vec![None; parameter_types.len()],
            None,
            None,
            TypeLibMemberInvokeKind::Method,
        );
        let slots: Vec<RuntimeByRefSlot> = [
            RuntimeValueType::Integer,
            RuntimeValueType::Byte,
            RuntimeValueType::Boolean,
            RuntimeValueType::LongLong,
            RuntimeValueType::Single,
            RuntimeValueType::Double,
            RuntimeValueType::Currency,
            RuntimeValueType::Date,
            RuntimeValueType::Decimal,
            RuntimeValueType::Variant,
        ]
        .iter()
        .enumerate()
        .map(|(index, ty)| RuntimeByRefSlot::new(index as u32, Some(*ty)))
        .collect();
        plan.parameter_byref_slots = slots.iter().copied().map(Some).collect();
        let result = unsafe {
            vtable_invoke_with_writebacks(
                this,
                &plan,
                &[
                    Variant::from_i16(1),
                    Variant::from_u8(2),
                    Variant::from_bool(false),
                    Variant::from_i64(3),
                    Variant::from_f64(4.0),
                    Variant::from_f64(5.0),
                    Variant::from_currency_scaled_i64(6),
                    Variant::from_date_f64(7.0),
                    Variant::from_decimal96(Decimal96::from_parts(8, 0, 0, 0, false)),
                    Variant::from_i32(9),
                ],
                29,
                &mut resolve,
                &mut bind,
            )
        }
        .expect("ByRef breadth vtable call should succeed");
        assert_eq!(result.writebacks.len(), slots.len());
        assert_eq!(result.writebacks[0].value.as_i16(), Some(-321));
        assert_eq!(result.writebacks[1].value.as_u8(), Some(222));
        assert_eq!(result.writebacks[2].value.as_bool(), Some(true));
        assert_eq!(
            result.writebacks[3].value.as_i64(),
            Some(DUAL_LONGLONG_VALUE + 10)
        );
        assert_eq!(
            result.writebacks[4].value.as_f64(),
            Some(f64::from(DUAL_SINGLE_VALUE + 1.0))
        );
        assert_eq!(
            result.writebacks[5].value.as_f64(),
            Some(DUAL_DOUBLE_VALUE - 1.0)
        );
        assert_eq!(
            result.writebacks[6].value.as_currency_scaled_i64(),
            Some(DUAL_PRICE_SCALED_I64 + 10_000)
        );
        assert_eq!(
            result.writebacks[7].value.as_date_f64(),
            Some(DUAL_CREATED_OLE_DATE + 2.0)
        );
        assert_eq!(
            result.writebacks[8].value.as_decimal96(),
            Some(Decimal96::from_parts(
                DUAL_DECIMAL_LO + 1,
                DUAL_DECIMAL_MID,
                DUAL_DECIMAL_HI,
                DUAL_DECIMAL_SCALE,
                DUAL_DECIMAL_NEGATIVE,
            ))
        );
        assert_eq!(result.writebacks[9].value.as_i32(), Some(77));
        unsafe { release_dual(this) };
    }

    #[test]
    fn lookup_returns_a_bound_object_variant() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_LOOKUP,
            vec![TypeLibParamType::String],
            vec![],
            vec![],
            Some(TypeLibParamType::Object),
            Some(TypeLibWireType::InterfacePointer {
                name: "ITestDispatch".to_string(),
            }),
            TypeLibMemberInvokeKind::PropertyGet,
        );
        // SAFETY: Lookup is slot 10: ([in] BSTR) -> IDispatch* retval.
        let result = unsafe {
            vtable_invoke(
                this,
                &plan,
                &[Variant::from_string("alpha")],
                4,
                &mut resolve,
                &mut bind,
            )
        };
        let value = result.expect("Lookup should succeed");
        assert_eq!(
            value.vtype(),
            VarType::Object,
            "Lookup returns a bound object Variant"
        );
        assert_eq!(
            value.as_object_ref().map(|o| o.raw()),
            Some(99),
            "the bind closure's sentinel ObjectRef should surface"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn byref_string_object_longptr_and_safearray_return_writebacks() {
        let this = create_oxvba_dual_vtable_object();
        let object = ObjectRef::from_compat_identity(44);
        let plan = invocation_plan(
            DUAL_SLOT_MUTATE_BYREF_OBJECT_STRING_ARRAY,
            vec![
                TypeLibParamType::ByRefString,
                TypeLibParamType::ByRefObject,
                TypeLibParamType::ByRefLongPtr,
                TypeLibParamType::Variant,
            ],
            vec![
                TypeLibWireType::Automation(TypeLibParamType::ByRefString),
                TypeLibWireType::InterfacePointer {
                    name: "IDispatch".to_string(),
                },
                TypeLibWireType::Automation(TypeLibParamType::ByRefLongPtr),
                TypeLibWireType::ByRefSafeArrayVariant,
            ],
            vec![None, Some(idispatch_iid()), None, None],
            None,
            None,
            TypeLibMemberInvokeKind::Method,
        );
        let args = vec![
            Variant::from_string("initial"),
            Variant::from_object_ref(object.clone()),
            Variant::from_i64(0x20),
            Variant::from_safearray(SafeArray::from_variants(vec![Variant::from_i32(1)])),
        ];
        let slots: Vec<RuntimeByRefSlot> = [
            RuntimeValueType::String,
            RuntimeValueType::Object,
            RuntimeValueType::LongPtr,
            RuntimeValueType::Variant,
        ]
        .iter()
        .enumerate()
        .map(|(index, ty)| RuntimeByRefSlot::new(index as u32, Some(*ty)))
        .collect();
        let mut plan = plan;
        plan.parameter_byref_slots = slots.iter().copied().map(Some).collect();
        let mut resolver = object_resolver_for(object, this.cast::<crate::RawIDispatch>());
        let mut bind = release_and_bind();
        let result = unsafe {
            vtable_invoke_with_writebacks(this, &plan, &args, 7030, &mut resolver, &mut bind)
        }
        .expect("ByRef object/string/LongPtr/SAFEARRAY vtable call should succeed");

        assert_eq!(result.writebacks.len(), 4);
        assert_eq!(
            result.writebacks[0]
                .value
                .as_bstr()
                .map(|value| value.to_string()),
            Some("byref-string-mutated".to_string())
        );
        assert_eq!(
            result.writebacks[1].value.as_object_ref().map(|o| o.raw()),
            Some(99)
        );
        assert_eq!(result.writebacks[2].value.as_i64(), Some(0x1020));
        let array_values = result.writebacks[3]
            .value
            .as_safearray()
            .expect("SAFEARRAY writeback")
            .variant_elements()
            .expect("variant elements");
        assert_eq!(
            array_values.iter().map(Variant::as_i32).collect::<Vec<_>>(),
            vec![Some(55), Some(89)]
        );
        unsafe { release_dual(this) };
    }

    #[test]
    fn validates_supported_inbound_automation_breadth() {
        let this = create_oxvba_dual_vtable_object();
        let object = ObjectRef::from_compat_identity(123);
        let object_dispatch = create_oxvba_test_dispatch();
        let mut resolve = object_resolver_for(object.clone(), object_dispatch);
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_VALIDATE_ALL_INPUTS,
            vec![
                TypeLibParamType::Byte,
                TypeLibParamType::Integer,
                TypeLibParamType::Long,
                TypeLibParamType::LongLong,
                TypeLibParamType::Single,
                TypeLibParamType::Double,
                TypeLibParamType::Currency,
                TypeLibParamType::Date,
                TypeLibParamType::Boolean,
                TypeLibParamType::String,
                TypeLibParamType::Variant,
                TypeLibParamType::Object,
                TypeLibParamType::Decimal,
            ],
            vec![
                TypeLibWireType::Automation(TypeLibParamType::Byte),
                TypeLibWireType::Automation(TypeLibParamType::Integer),
                TypeLibWireType::Automation(TypeLibParamType::Long),
                TypeLibWireType::Automation(TypeLibParamType::LongLong),
                TypeLibWireType::Automation(TypeLibParamType::Single),
                TypeLibWireType::Automation(TypeLibParamType::Double),
                TypeLibWireType::Automation(TypeLibParamType::Currency),
                TypeLibWireType::Automation(TypeLibParamType::Date),
                TypeLibWireType::Automation(TypeLibParamType::Boolean),
                TypeLibWireType::Automation(TypeLibParamType::String),
                TypeLibWireType::Automation(TypeLibParamType::Variant),
                TypeLibWireType::InterfacePointer {
                    name: "IDispatch".to_string(),
                },
                TypeLibWireType::Automation(TypeLibParamType::Decimal),
            ],
            vec![
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(idispatch_iid()),
                None,
            ],
            Some(TypeLibParamType::Boolean),
            Some(TypeLibWireType::Automation(TypeLibParamType::Boolean)),
            TypeLibMemberInvokeKind::Method,
        );
        // SAFETY: slot 15 is the fixture's typed-input validator with the exact
        // ABI described by the plan.
        let value = unsafe {
            vtable_invoke(
                this,
                &plan,
                &[
                    Variant::from_u8(9),
                    Variant::from_i16(-12),
                    Variant::from_i32(34_567),
                    Variant::from_i64(DUAL_LONGLONG_VALUE),
                    Variant::from_f64(1.5),
                    Variant::from_f64(-2.25),
                    Variant::from_currency_scaled_i64(DUAL_PRICE_SCALED_I64),
                    Variant::from_date_f64(DUAL_CREATED_OLE_DATE),
                    Variant::from_bool(true),
                    Variant::from_string("typed-input"),
                    Variant::from_i32(1234),
                    Variant::from_object_ref(object),
                    Variant::from_decimal96(Decimal96::from_parts(
                        DUAL_DECIMAL_LO,
                        DUAL_DECIMAL_MID,
                        DUAL_DECIMAL_HI,
                        DUAL_DECIMAL_SCALE,
                        DUAL_DECIMAL_NEGATIVE,
                    )),
                ],
                15,
                &mut resolve,
                &mut bind,
            )
        }
        .expect("typed inbound vtable call should succeed");
        assert_eq!(
            value.as_bool(),
            Some(true),
            "fixture must confirm every inbound value survived ABI lowering"
        );
        // SAFETY: balance both fixture references handed to the test.
        unsafe {
            crate::release_dispatch(object_dispatch);
            release_dual(this);
        }
    }

    #[test]
    fn returns_supported_scalar_string_and_variant_breadth() {
        let cases = [
            (
                DUAL_SLOT_GET_BYTE_VALUE,
                TypeLibParamType::Byte,
                TypeLibWireType::Automation(TypeLibParamType::Byte),
                Variant::from_u8(DUAL_BYTE_VALUE),
            ),
            (
                DUAL_SLOT_GET_INTEGER_VALUE,
                TypeLibParamType::Integer,
                TypeLibWireType::Automation(TypeLibParamType::Integer),
                Variant::from_i16(DUAL_INTEGER_VALUE),
            ),
            (
                DUAL_SLOT_GET_LONGLONG_VALUE,
                TypeLibParamType::LongLong,
                TypeLibWireType::Automation(TypeLibParamType::LongLong),
                Variant::from_i64(DUAL_LONGLONG_VALUE),
            ),
            (
                DUAL_SLOT_GET_SINGLE_VALUE,
                TypeLibParamType::Single,
                TypeLibWireType::Automation(TypeLibParamType::Single),
                Variant::from_f64(f64::from(DUAL_SINGLE_VALUE)),
            ),
            (
                DUAL_SLOT_GET_DOUBLE_VALUE,
                TypeLibParamType::Double,
                TypeLibWireType::Automation(TypeLibParamType::Double),
                Variant::from_f64(DUAL_DOUBLE_VALUE),
            ),
            (
                DUAL_SLOT_GET_TEXT_VALUE,
                TypeLibParamType::String,
                TypeLibWireType::Automation(TypeLibParamType::String),
                Variant::from_string(DUAL_TEXT_VALUE),
            ),
            (
                DUAL_SLOT_GET_VARIANT_VALUE,
                TypeLibParamType::Variant,
                TypeLibWireType::Automation(TypeLibParamType::Variant),
                Variant::from_i32(DUAL_VARIANT_VALUE),
            ),
        ];

        for (slot, return_type, return_wire_type, expected) in cases {
            let this = create_oxvba_dual_vtable_object();
            let mut resolve = no_object_resolver();
            let mut bind = release_and_bind();
            let plan = invocation_plan(
                slot,
                vec![],
                vec![],
                vec![],
                Some(return_type),
                Some(return_wire_type),
                TypeLibMemberInvokeKind::PropertyGet,
            );
            // SAFETY: each slot is a no-arg property getter returning the declared
            // automation shape.
            let value = unsafe {
                vtable_invoke(this, &plan, &[], i32::from(slot), &mut resolve, &mut bind)
            }
            .expect("typed return vtable call should succeed");
            assert_eq!(
                value, expected,
                "slot {slot} should decode through the declared out-cell"
            );
            // SAFETY: balances the create_* reference for this case.
            unsafe { release_dual(this) };
        }
    }

    #[test]
    fn putref_object_value_uses_interface_pointer_vtable_shape() {
        let this = create_oxvba_dual_vtable_object();
        let object = ObjectRef::from_compat_identity(456);
        let object_dispatch = create_oxvba_test_dispatch();
        let mut resolve = object_resolver_for(object.clone(), object_dispatch);
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_PUTREF_OBJECT_VALUE,
            vec![TypeLibParamType::Object],
            vec![TypeLibWireType::InterfacePointer {
                name: "IDispatch".to_string(),
            }],
            vec![Some(idispatch_iid())],
            None,
            None,
            TypeLibMemberInvokeKind::PropertyPutRef,
        );
        // SAFETY: slot 23 is `putref_ObjectValue(IDispatch*)` with no retval.
        let value = unsafe {
            vtable_invoke(
                this,
                &plan,
                &[Variant::from_object_ref(object)],
                23,
                &mut resolve,
                &mut bind,
            )
        }
        .expect("putref object vtable call should succeed");
        assert_eq!(
            value.vtype(),
            VarType::Empty,
            "no-retval putref returns Empty on success"
        );
        // SAFETY: balance both fixture references handed to the test.
        unsafe {
            crate::release_dispatch(object_dispatch);
            release_dual(this);
        }
    }

    #[test]
    fn get_price_decodes_currency_out_cell() {
        // S5c: a VT_CY [out,retval] decodes through OutCell::Currency to a Currency
        // Variant (i64 scaled ×10000), not a plain integer.
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_GET_PRICE,
            vec![],
            vec![],
            vec![],
            Some(TypeLibParamType::Currency),
            None,
            TypeLibMemberInvokeKind::PropertyGet,
        );
        // SAFETY: get_Price is slot 12: () -> CY retval.
        let value = unsafe { vtable_invoke(this, &plan, &[], 12, &mut resolve, &mut bind) }
            .expect("get_Price should succeed");
        assert_eq!(
            value.vtype(),
            VarType::Currency,
            "VT_CY retval must decode to a Currency Variant"
        );
        assert_eq!(
            value.as_currency_scaled_i64(),
            Some(DUAL_PRICE_SCALED_I64),
            "the scaled currency value must round-trip"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn get_created_decodes_date_out_cell() {
        // S5c: a VT_DATE [out,retval] decodes through OutCell::Date to a Date
        // Variant (distinct from a plain Double), exercising the new date out-cell.
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_GET_CREATED,
            vec![],
            vec![],
            vec![],
            Some(TypeLibParamType::Date),
            None,
            TypeLibMemberInvokeKind::PropertyGet,
        );
        // SAFETY: get_Created is slot 13: () -> DATE retval.
        let value = unsafe { vtable_invoke(this, &plan, &[], 13, &mut resolve, &mut bind) }
            .expect("get_Created should succeed");
        assert_eq!(
            value.vtype(),
            VarType::Date,
            "VT_DATE retval must decode to a Date Variant, not a Double"
        );
        assert_eq!(
            value.as_date_f64(),
            Some(DUAL_CREATED_OLE_DATE),
            "the OLE date value must round-trip"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn get_owner_binds_iunknown_retval_object() {
        // S5c: a VT_UNKNOWN/VT_DISPATCH [out,retval] is decoded through
        // OutCell::Interface and handed to the bind closure, which takes ownership
        // of the transferred reference (here releasing it and surfacing a sentinel).
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_GET_OWNER,
            vec![],
            vec![],
            vec![],
            Some(TypeLibParamType::Object),
            Some(TypeLibWireType::InterfacePointer {
                name: "ITestDispatch".to_string(),
            }),
            TypeLibMemberInvokeKind::PropertyGet,
        );
        // SAFETY: get_Owner is slot 14: () -> IUnknown* retval (a TestDispatch whose
        // IUnknown aliases its IDispatch, so the bound pointer is a live object).
        let value = unsafe { vtable_invoke(this, &plan, &[], 14, &mut resolve, &mut bind) }
            .expect("get_Owner should succeed");
        assert_eq!(
            value.vtype(),
            VarType::Object,
            "an interface retval must decode to a bound object Variant"
        );
        assert_eq!(
            value.as_object_ref().map(|o| o.raw()),
            Some(99),
            "the bind closure's sentinel ObjectRef should surface for the retval"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }

    #[test]
    fn raise_error_surfaces_ierrorinfo_into_the_failure() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        let plan = invocation_plan(
            DUAL_SLOT_RAISE_ERROR,
            vec![],
            vec![],
            vec![],
            Some(TypeLibParamType::Long),
            None,
            TypeLibMemberInvokeKind::Method,
        );
        // SAFETY: raise_error is slot 11: SetErrorInfo + fail HRESULT.
        let result = unsafe { vtable_invoke(this, &plan, &[], 5, &mut resolve, &mut bind) };
        let failure = result.expect_err("raise_error must surface a ComInvokeFailure");
        assert_eq!(
            failure.hr,
            Some(crate::windows_test_dispatch::DUAL_RAISE_ERROR_HRESULT),
            "the failure carries the server's HRESULT"
        );
        let excep = failure
            .excep
            .expect("GetErrorInfo should have produced rich exception info");
        assert_eq!(
            excep.source.as_deref(),
            Some(DUAL_RAISE_ERROR_SOURCE),
            "IErrorInfo Source must surface"
        );
        assert_eq!(
            excep.description.as_deref(),
            Some(DUAL_RAISE_ERROR_DESCRIPTION),
            "IErrorInfo Description must surface"
        );
        // SAFETY: balances the create_* reference.
        unsafe { release_dual(this) };
    }
}
