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
use crate::windows_invoke::{ComInvokeExceptionInfo, ComInvokeFailure, bstr_to_string_and_free};
use crate::{ComValue, TypeLibMemberInvokeKind, TypeLibParamType};
use oxvba_runtime::{ObjectRef, Variant};
use std::ffi::c_void;
use windows_sys::Win32::Foundation::{SysAllocString, SysFreeString};
use windows_sys::Win32::System::Variant::{VARIANT, VT_DISPATCH, VT_UNKNOWN, VariantClear};

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
    /// An interface pointer we obtained by `QueryInterface`-ing an `[in]` object arg to
    /// its declared param IID (Bug-4b); we own that one reference and `Release` it after
    /// the call (the callee only borrowed it).
    Interface(*mut c_void),
}

/// The retval out-cell, sized to the member's return VARTYPE. The trailing
/// `[out,retval]` pointer passed to the callee points into this.
enum OutCell {
    None,
    I32(Box<i32>),
    I16(Box<i16>),
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
    Bstr(Box<windows_sys::core::BSTR>),
    Variant(Box<VARIANT>),
    Interface(Box<*mut c_void>),
}

/// Call a dual-interface member through its COM vtable slot.
///
/// Marshals each inbound [`Variant`] arg per its FUNCDESC `parameter_types`
/// VARTYPE, appends a caller-owned `[out,retval]` cell when `return_type` is
/// set, calls `(*(*this))[slot](this, …, retval_ptr)` via libffi (HRESULT in
/// EAX), then decodes the retval. On `hr < 0` returns a [`ComInvokeFailure`]
/// carrying the `GetErrorInfo`-retrieved rich error.
///
/// `slot` is the x64 vtable **slot index** (the FUNCDESC `oVft / 8` that S1
/// stored), NOT a byte offset.
///
/// NOTE (workset S2): not yet wired into live dispatch; S3 owns that. Kept
/// `pub(crate)` and exercised by the in-process unit test so it is not
/// dead-code-linted.
///
/// # Safety
/// `this` must be a live `IDispatch`/dual-interface pointer whose vtable has at
/// least `slot + 1` entries and whose slot-`slot` function has the C ABI implied
/// by `parameter_types` + `return_type` (the dual-member contract:
/// `HRESULT slot(this, params…, retval*)`). The closures must uphold COM
/// ownership and identity rules for object handles and returned interfaces.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn vtable_invoke<FResolveObject, FBindDispatch>(
    this: *mut c_void,
    slot: u16,
    parameter_types: &[TypeLibParamType],
    parameter_iids: &[Option<crate::ComInterfaceIid>],
    return_type: Option<TypeLibParamType>,
    _invoke_kind: TypeLibMemberInvokeKind,
    args: &[Variant],
    label: &'static str,
    dispid: i32,
    resolve_object: &mut FResolveObject,
    bind_dispatch_result: &mut FBindDispatch,
) -> Result<Variant, ComInvokeFailure>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut c_void, String>,
    FBindDispatch: FnMut(*mut c_void) -> Result<Variant, String>,
{
    if this.is_null() {
        return Err(validation_failure(
            label,
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
            label,
            dispid,
            format!("misaligned this pointer {this:p} for vtable invoke"),
        ));
    }
    if args.len() != parameter_types.len() {
        return Err(validation_failure(
            label,
            dispid,
            format!(
                "vtable arity mismatch: {} args for {} parameters",
                args.len(),
                parameter_types.len()
            ),
        ));
    }

    // Resolve the slot function pointer: this is `*const *const fnptr`, so the
    // vtable is `*(*this)` and the entry is `vtable[slot]`.
    // SAFETY: the fn contract guarantees `this` is a live interface pointer with
    // a vtable of at least `slot + 1` entries; we read the vtable base then the
    // slot-`slot` function pointer, both in bounds.
    let fnptr = unsafe {
        let vtbl = *this.cast::<*const *const c_void>();
        *vtbl.add(slot as usize)
    };
    if fnptr.is_null() {
        return Err(validation_failure(
            label,
            dispid,
            "null vtable slot function pointer",
        ));
    }

    // Build the libffi argument list left-to-right: arg0 = this, then the
    // marshalled params, then (if any) the trailing [out,retval] pointer.
    let mut ffi_args: Vec<FfiArg> = Vec::with_capacity(args.len() + 2);
    let mut inbound_owned: Vec<InboundOwned> = Vec::with_capacity(args.len());
    ffi_args.push(FfiArg::Pointer(this));

    for (i, (param_type, arg)) in parameter_types.iter().zip(args.iter()).enumerate() {
        // The per-parameter declared interface IID for an object arg (Bug-4b); `None`
        // for scalars or when no IID was recovered. `get(i)` tolerates synthesized
        // trailing optionals (which extend `args` beyond `parameter_iids`).
        let param_iid = parameter_iids.get(i).copied().flatten();
        match marshal_inbound_param(*param_type, arg, param_iid, resolve_object) {
            Ok((ffi_arg, owned)) => {
                ffi_args.push(ffi_arg);
                inbound_owned.push(owned);
            }
            Err(detail) => {
                // Free everything marshalled so far before bailing.
                free_inbound(&mut inbound_owned);
                return Err(validation_failure(label, dispid, detail));
            }
        }
    }

    // Allocate the [out,retval] cell and append its pointer.
    let mut out_cell = match return_type {
        Some(rt) => match alloc_out_cell(rt) {
            Ok(cell) => cell,
            Err(detail) => {
                free_inbound(&mut inbound_owned);
                return Err(validation_failure(label, dispid, detail));
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

    // We own inbound [in] BSTRs (callee only borrowed them) and inbound VARIANT
    // cells — free them now, on success and failure alike.
    free_inbound(&mut inbound_owned);

    if hr < 0 {
        // Drop any retval cell payload the callee may have written before the
        // failure (defensive: most servers leave it zeroed on failure).
        discard_out_cell(out_cell);
        return Err(ComInvokeFailure {
            label,
            dispid,
            hr: Some(hr),
            arg_err: None,
            excep: take_error_info(),
            detail: None,
        });
    }

    // hr >= 0, so the callee populated the retval cell per its declared return
    // type; decode it and take ownership of any transferred BSTR/interface/
    // VARIANT payload.
    decode_out_cell(out_cell, label, dispid, bind_dispatch_result)
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

/// Marshal one inbound parameter from a [`Variant`] per the workset §4 table.
/// Internally calls into FFI helpers under documented `unsafe` blocks; the
/// returned [`InboundOwned`] records what the caller must free post-call.
fn marshal_inbound_param<FResolveObject>(
    param_type: TypeLibParamType,
    arg: &Variant,
    param_iid: Option<crate::ComInterfaceIid>,
    resolve_object: &mut FResolveObject,
) -> Result<(FfiArg, InboundOwned), String>
where
    FResolveObject: FnMut(ObjectRef) -> Result<*mut c_void, String>,
{
    use TypeLibParamType as P;
    let value = ComValue::from_variant(arg)?;
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
            // SAFETY: an Interface entry is the single reference our QueryInterface
            // handed us; the callee only borrowed it, so we Release it exactly once.
            InboundOwned::Interface(interface) => unsafe { crate::release_unknown(interface) },
        }
    }
}

fn alloc_out_cell(return_type: TypeLibParamType) -> Result<OutCell, String> {
    use TypeLibParamType as P;
    Ok(match return_type {
        P::Long => OutCell::I32(Box::new(0)),
        P::Integer => OutCell::I16(Box::new(0)),
        P::Boolean => OutCell::Bool(Box::new(0)),
        P::Byte => OutCell::I32(Box::new(0)),
        P::LongLong => OutCell::I64(Box::new(0)),
        P::Double => OutCell::F64(Box::new(0.0)),
        P::Date => OutCell::Date(Box::new(0.0)),
        P::Single => OutCell::F32(Box::new(0.0)),
        P::Currency => OutCell::Currency(Box::new(0)),
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

fn out_cell_ptr(cell: &mut OutCell) -> Option<*mut c_void> {
    match cell {
        OutCell::None => None,
        OutCell::I32(b) => Some((b.as_mut() as *mut i32).cast::<c_void>()),
        OutCell::I16(b) | OutCell::Bool(b) => Some((b.as_mut() as *mut i16).cast::<c_void>()),
        OutCell::I64(b) | OutCell::Currency(b) => Some((b.as_mut() as *mut i64).cast::<c_void>()),
        OutCell::F64(b) | OutCell::Date(b) => Some((b.as_mut() as *mut f64).cast::<c_void>()),
        OutCell::F32(b) => Some((b.as_mut() as *mut f32).cast::<c_void>()),
        OutCell::Bstr(b) => Some((b.as_mut() as *mut windows_sys::core::BSTR).cast::<c_void>()),
        OutCell::Variant(b) => Some((b.as_mut() as *mut VARIANT).cast::<c_void>()),
        OutCell::Interface(b) => Some((b.as_mut() as *mut *mut c_void).cast::<c_void>()),
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
        // VARIANT_BOOL: any non-zero is true (VBA convention writes -1 for true).
        OutCell::Bool(b) => Variant::from_bool(*b != 0),
        OutCell::I64(b) => Variant::from_i64(*b),
        OutCell::Currency(b) => Variant::from_currency_scaled_i64(*b),
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
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows_test_dispatch::{
        DUAL_CREATED_OLE_DATE, DUAL_PRICE_SCALED_I64, DUAL_RAISE_ERROR_DESCRIPTION,
        DUAL_RAISE_ERROR_SOURCE, DUAL_SLOT_EXISTS, DUAL_SLOT_GET_COUNT, DUAL_SLOT_GET_CREATED,
        DUAL_SLOT_GET_OWNER, DUAL_SLOT_GET_PRICE, DUAL_SLOT_LOOKUP, DUAL_SLOT_PUT_VALUE,
        DUAL_SLOT_RAISE_ERROR, create_oxvba_dual_vtable_object,
    };
    use oxvba_runtime::VarType;

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

    #[test]
    fn get_count_first_light_returns_expected_i32() {
        // FIRST LIGHT: isolates this-ptr + out-cell + HRESULT + oVft/8 slot
        // index. If the slot index were wrong this would crash the host, so it
        // must pass before any BSTR/VARIANT/interface complexity is trusted.
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        // SAFETY: `this` is a live dual-vtable fixture object with the known
        // slot layout; get_Count is slot 7 with retval Long.
        let result = unsafe {
            vtable_invoke(
                this,
                DUAL_SLOT_GET_COUNT,
                &[],
                &[],
                Some(TypeLibParamType::Long),
                TypeLibMemberInvokeKind::PropertyGet,
                &[],
                "property-get",
                1,
                &mut resolve,
                &mut bind,
            )
        };
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
        // SAFETY: Exists is slot 8: (i32 key) -> VARIANT_BOOL retval.
        let yes = unsafe {
            vtable_invoke(
                this,
                DUAL_SLOT_EXISTS,
                &[TypeLibParamType::Long],
                &[],
                Some(TypeLibParamType::Boolean),
                TypeLibMemberInvokeKind::Method,
                &[Variant::from_i32(42)],
                "method",
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
                DUAL_SLOT_EXISTS,
                &[TypeLibParamType::Long],
                &[],
                Some(TypeLibParamType::Boolean),
                TypeLibMemberInvokeKind::Method,
                &[Variant::from_i32(7)],
                "method",
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
        // SAFETY: put_Value is slot 9: ([in] VARIANT*) -> HRESULT, no retval.
        let result = unsafe {
            vtable_invoke(
                this,
                DUAL_SLOT_PUT_VALUE,
                &[TypeLibParamType::Variant],
                &[],
                None,
                TypeLibMemberInvokeKind::PropertyPut,
                &[Variant::from_i32(1234)],
                "property-put",
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
    fn lookup_returns_a_bound_object_variant() {
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        // SAFETY: Lookup is slot 10: ([in] BSTR) -> IDispatch* retval.
        let result = unsafe {
            vtable_invoke(
                this,
                DUAL_SLOT_LOOKUP,
                &[TypeLibParamType::String],
                &[],
                Some(TypeLibParamType::Object),
                TypeLibMemberInvokeKind::PropertyGet,
                &[Variant::from_string("alpha")],
                "property-get",
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
    fn get_price_decodes_currency_out_cell() {
        // S5c: a VT_CY [out,retval] decodes through OutCell::Currency to a Currency
        // Variant (i64 scaled ×10000), not a plain integer.
        let this = create_oxvba_dual_vtable_object();
        let mut resolve = no_object_resolver();
        let mut bind = release_and_bind();
        // SAFETY: get_Price is slot 12: () -> CY retval.
        let value = unsafe {
            vtable_invoke(
                this,
                DUAL_SLOT_GET_PRICE,
                &[],
                &[],
                Some(TypeLibParamType::Currency),
                TypeLibMemberInvokeKind::PropertyGet,
                &[],
                "property-get",
                12,
                &mut resolve,
                &mut bind,
            )
        }
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
        // SAFETY: get_Created is slot 13: () -> DATE retval.
        let value = unsafe {
            vtable_invoke(
                this,
                DUAL_SLOT_GET_CREATED,
                &[],
                &[],
                Some(TypeLibParamType::Date),
                TypeLibMemberInvokeKind::PropertyGet,
                &[],
                "property-get",
                13,
                &mut resolve,
                &mut bind,
            )
        }
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
        // SAFETY: get_Owner is slot 14: () -> IUnknown* retval (a TestDispatch whose
        // IUnknown aliases its IDispatch, so the bound pointer is a live object).
        let value = unsafe {
            vtable_invoke(
                this,
                DUAL_SLOT_GET_OWNER,
                &[],
                &[],
                Some(TypeLibParamType::Object),
                TypeLibMemberInvokeKind::PropertyGet,
                &[],
                "property-get",
                14,
                &mut resolve,
                &mut bind,
            )
        }
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
        // SAFETY: raise_error is slot 11: SetErrorInfo + fail HRESULT.
        let result = unsafe {
            vtable_invoke(
                this,
                DUAL_SLOT_RAISE_ERROR,
                &[],
                &[],
                Some(TypeLibParamType::Long),
                TypeLibMemberInvokeKind::Method,
                &[],
                "method",
                5,
                &mut resolve,
                &mut bind,
            )
        };
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
