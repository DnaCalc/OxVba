#![allow(unsafe_op_in_unsafe_fn)]

use crate::ComValue;
use crate::windows_client::{
    COM_CONNECT_E_CANNOTCONNECT, COM_CONNECT_E_NOCONNECTION, COM_DISP_E_BADPARAMCOUNT,
    COM_DISP_E_MEMBERNOTFOUND, COM_DISP_E_PARAMNOTFOUND, COM_DISP_E_TYPEMISMATCH,
    COM_DISP_E_UNKNOWNNAME, COM_E_INVALIDARG, COM_E_NOINTERFACE, COM_E_NOTIMPL, COM_S_OK,
    IID_ICONNECTIONPOINTCONTAINER, IID_IDISPATCH, IID_IUNKNOWN, RawIConnectionPoint,
    RawIConnectionPointContainer, RawIDispatch, RawIDispatchVtbl, parse_guid_canonical,
    release_connection_point,
};
use crate::windows_variant::variant_to_com_value;
use std::ffi::c_void;
use std::sync::Arc;
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicU32, Ordering};
use windows_sys::Win32::System::Com::{DISPPARAMS, EXCEPINFO};
use windows_sys::Win32::System::Variant::VARIANT;
use windows_sys::core::GUID;

/// Native event argument as received by the sink. Scalar values are already
/// projected to `ComValue`; object values carry one retained `IDispatch`
/// reference for the runtime state to bind or release.
pub enum WindowsEventArg {
    Value(ComValue),
    Dispatch(*mut RawIDispatch),
}

type DispatchEventCallback = Arc<dyn Fn(&[WindowsEventArg]) -> bool + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsConnectionPointTransport {
    pub connection_point: usize,
    pub cookie: u32,
}

#[derive(Clone)]
pub struct DispatchEventSinkConfig {
    pub event_dispatch_member: i32,
    pub expected_arity: usize,
    pub connection_point_iid: Option<GUID>,
    pub on_event: DispatchEventCallback,
}

/// The live `IDispatch`/`IUnknown` pointer carried by an object-typed event-argument
/// VARIANT (`VT_DISPATCH`/`VT_UNKNOWN`, optionally `VT_BYREF`), retained as an
/// `IDispatch`, or `None` for any non-object VARIANT.
///
/// # Safety
/// `variant` must be a live VARIANT for the duration of the read.
#[cfg(target_os = "windows")]
unsafe fn retain_dispatch_arg(variant: &VARIANT) -> Option<*mut RawIDispatch> {
    const VT_DISPATCH_T: u16 = 9;
    const VT_UNKNOWN_T: u16 = 13;
    const VT_BYREF_T: u16 = 0x4000;
    // SAFETY: reading the discriminant of a live VARIANT.
    let vt = unsafe { variant.Anonymous.Anonymous.vt };
    let base = vt & !VT_BYREF_T;
    if base != VT_DISPATCH_T && base != VT_UNKNOWN_T {
        return None;
    }
    // SAFETY: the vt tag selects the active union member; for BYREF the field is a
    // pointer-to-pointer that we dereference once.
    let ptr = unsafe {
        if vt & VT_BYREF_T != 0 {
            let pp = variant.Anonymous.Anonymous.Anonymous.ppdispVal;
            if pp.is_null() {
                return None;
            }
            (*pp).cast::<c_void>()
        } else {
            variant
                .Anonymous
                .Anonymous
                .Anonymous
                .pdispVal
                .cast::<c_void>()
        }
    };
    if ptr.is_null() {
        return None;
    }
    if base == VT_DISPATCH_T {
        let dispatch = ptr.cast::<RawIDispatch>();
        // SAFETY: the event VARIANT carries a live borrowed IDispatch pointer; retain
        // one reference so the queued runtime binding owns its lifetime.
        unsafe {
            ((*(*dispatch).vtbl).unknown.add_ref)(dispatch.cast::<c_void>());
        }
        return Some(dispatch);
    }
    let mut out: *mut c_void = std::ptr::null_mut();
    // SAFETY: the event VARIANT carries a live borrowed IUnknown pointer; its first
    // field is the standard IUnknown vtable used for QueryInterface below.
    let vtbl = unsafe { *(ptr as *const *const crate::RawIUnknownVtbl) };
    // SAFETY: `out` is a valid out-slot and IID_IDISPATCH is a static interface ID;
    // on success QueryInterface returns one retained IDispatch reference.
    let hr = unsafe { ((*vtbl).query_interface)(ptr, &IID_IDISPATCH, &mut out) };
    if hr < 0 || out.is_null() {
        None
    } else {
        Some(out.cast::<RawIDispatch>())
    }
}

#[repr(C)]
struct WindowsDispatchEventSink {
    dispatch: RawIDispatch,
    ref_count: AtomicU32,
    event_dispatch_member: i32,
    expected_arity: usize,
    connection_point_iid: Option<GUID>,
    on_event: DispatchEventCallback,
}

static WINDOWS_DISPATCH_EVENT_SINK_VTBL: RawIDispatchVtbl = RawIDispatchVtbl {
    unknown: crate::RawIUnknownVtbl {
        query_interface: windows_dispatch_event_sink_query_interface,
        add_ref: windows_dispatch_event_sink_add_ref,
        release: windows_dispatch_event_sink_release,
    },
    get_type_info_count: windows_dispatch_event_sink_get_type_info_count,
    get_type_info: windows_dispatch_event_sink_get_type_info,
    get_ids_of_names: windows_dispatch_event_sink_get_ids_of_names,
    invoke: windows_dispatch_event_sink_invoke,
};

fn create_dispatch_event_sink(config: DispatchEventSinkConfig) -> *mut c_void {
    let sink = Box::new(WindowsDispatchEventSink {
        dispatch: RawIDispatch {
            vtbl: &WINDOWS_DISPATCH_EVENT_SINK_VTBL,
        },
        ref_count: AtomicU32::new(1),
        event_dispatch_member: config.event_dispatch_member,
        expected_arity: config.expected_arity,
        connection_point_iid: config.connection_point_iid,
        on_event: config.on_event,
    });
    Box::into_raw(sink).cast::<c_void>()
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_windows_dispatch_event_sink(this: *mut c_void) -> *mut WindowsDispatchEventSink {
    this.cast::<WindowsDispatchEventSink>()
}

/// # Safety
///
/// `dispatch` must be a valid live `IDispatch` pointer. `connection_point_iid` must identify the
/// event interface to advise, and `config` must remain valid to move into the sink object created
/// for the duration of the COM subscription.
pub unsafe fn try_advise_dispatch_event_sink(
    dispatch: *mut RawIDispatch,
    connection_point_iid: &str,
    config: DispatchEventSinkConfig,
) -> Result<Option<WindowsConnectionPointTransport>, String> {
    if dispatch.is_null() {
        return Err("dispatch pointer was null during connection-point advise".to_string());
    }
    let iid = parse_guid_canonical(connection_point_iid)
        .ok_or_else(|| format!("invalid connection-point IID `{connection_point_iid}`"))?;

    let mut container_ptr: *mut c_void = std::ptr::null_mut();
    let hr = ((*(*dispatch).vtbl).unknown.query_interface)(
        dispatch.cast::<c_void>(),
        &IID_ICONNECTIONPOINTCONTAINER,
        &mut container_ptr,
    );
    if hr == COM_E_NOINTERFACE {
        return Ok(None);
    }
    if hr < 0 {
        return Err(format!(
            "QueryInterface(IConnectionPointContainer) failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    if container_ptr.is_null() {
        return Err("QueryInterface(IConnectionPointContainer) returned null".to_string());
    }

    let container = container_ptr.cast::<RawIConnectionPointContainer>();
    let mut connection_point_ptr: *mut c_void = std::ptr::null_mut();
    let find_hr = ((*(*container).vtbl).find_connection_point)(
        container.cast::<c_void>(),
        &iid,
        &mut connection_point_ptr,
    );
    ((*(*container).vtbl).unknown.release)(container.cast::<c_void>());
    if find_hr == COM_E_NOINTERFACE || find_hr == COM_DISP_E_MEMBERNOTFOUND {
        return Ok(None);
    }
    if find_hr < 0 {
        return Err(format!(
            "FindConnectionPoint({connection_point_iid}) failed with HRESULT {:#010X}",
            find_hr as u32
        ));
    }
    if connection_point_ptr.is_null() {
        return Err("FindConnectionPoint returned null".to_string());
    }

    let connection_point = connection_point_ptr.cast::<RawIConnectionPoint>();
    let sink_ptr = create_dispatch_event_sink(DispatchEventSinkConfig {
        connection_point_iid: Some(iid),
        ..config
    });
    let mut cookie = 0u32;
    let advise_hr = ((*(*connection_point).vtbl).advise)(
        connection_point.cast::<c_void>(),
        sink_ptr,
        &mut cookie,
    );
    ((*(*(sink_ptr.cast::<RawIDispatch>())).vtbl).unknown.release)(sink_ptr);
    if advise_hr == COM_CONNECT_E_CANNOTCONNECT || advise_hr == COM_CONNECT_E_NOCONNECTION {
        release_connection_point(connection_point);
        return Ok(None);
    }
    if advise_hr < 0 || cookie == 0 {
        release_connection_point(connection_point);
        return Err(format!(
            "IConnectionPoint::Advise failed with HRESULT {:#010X}",
            advise_hr as u32
        ));
    }
    Ok(Some(WindowsConnectionPointTransport {
        connection_point: connection_point as usize,
        cookie,
    }))
}

/// # Safety
///
/// `transport` must describe a live connection point previously returned by
/// `try_advise_dispatch_event_sink` and still owned by the caller for one final `Unadvise`/`Release`.
pub unsafe fn unadvise_connection_point(
    transport: WindowsConnectionPointTransport,
) -> Result<(), String> {
    if transport.connection_point == 0 {
        return Ok(());
    }
    let connection_point = transport.connection_point as *mut RawIConnectionPoint;
    let hr =
        ((*(*connection_point).vtbl).unadvise)(connection_point.cast::<c_void>(), transport.cookie);
    release_connection_point(connection_point);
    if hr < 0 && hr != COM_CONNECT_E_NOCONNECTION {
        return Err(format!(
            "IConnectionPoint::Unadvise failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    Ok(())
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn windows_dispatch_event_sink_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    if crate::guid_equals(riid, &IID_IUNKNOWN) || crate::guid_equals(riid, &IID_IDISPATCH) {
        *ppv = this;
        let sink = as_windows_dispatch_event_sink(this);
        (*sink).ref_count.fetch_add(1, Ordering::AcqRel);
        return COM_S_OK;
    }
    let sink = as_windows_dispatch_event_sink(this);
    if let Some(ref cp_iid) = (*sink).connection_point_iid
        && crate::guid_equals(riid, cp_iid)
    {
        *ppv = this;
        (*sink).ref_count.fetch_add(1, Ordering::AcqRel);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn windows_dispatch_event_sink_add_ref(this: *mut c_void) -> u32 {
    let sink = as_windows_dispatch_event_sink(this);
    (*sink).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn windows_dispatch_event_sink_release(this: *mut c_void) -> u32 {
    let sink = as_windows_dispatch_event_sink(this);
    let prev = (*sink).ref_count.fetch_sub(1, Ordering::AcqRel);
    let next = prev.saturating_sub(1);
    if next == 0 {
        drop(Box::from_raw(sink));
    }
    next
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn windows_dispatch_event_sink_get_type_info_count(
    _this: *mut c_void,
    pctinfo: *mut u32,
) -> i32 {
    if pctinfo.is_null() {
        return COM_E_INVALIDARG;
    }
    *pctinfo = 0;
    COM_S_OK
}

unsafe extern "system" fn windows_dispatch_event_sink_get_type_info(
    _this: *mut c_void,
    _itinfo: u32,
    _lcid: u32,
    _pptinfo: *mut *mut c_void,
) -> i32 {
    COM_E_NOTIMPL
}

unsafe extern "system" fn windows_dispatch_event_sink_get_ids_of_names(
    _this: *mut c_void,
    _riid: *const GUID,
    _rgsznames: *mut *mut u16,
    _cnames: u32,
    _lcid: u32,
    _rgdispid: *mut i32,
) -> i32 {
    COM_DISP_E_UNKNOWNNAME
}

fn map_event_arg_raw_indices(
    params: &DISPPARAMS,
    expected_arity: usize,
) -> Result<Vec<usize>, (i32, Option<u32>)> {
    let cargs = params.cArgs as usize;
    let named_count = params.cNamedArgs as usize;
    if cargs != expected_arity
        || named_count > cargs
        || (cargs > 0 && params.rgvarg.is_null())
        || (named_count > 0 && params.rgdispidNamedArgs.is_null())
    {
        return Err((COM_DISP_E_BADPARAMCOUNT, None));
    }

    let mut raw_by_declared = vec![usize::MAX; expected_arity];
    let mut used = vec![false; expected_arity];

    // Automation stores named-argument values in the matching leading rgvarg
    // slots and identifies their declared parameter positions through
    // rgdispidNamedArgs. Excel SheetChange(Sh, Target) uses this form:
    // cNamedArgs=2, rgdispidNamedArgs=[0,1], rgvarg=[Sh,Target]. Positional
    // args remain the usual last-to-first rgvarg layout. References:
    // https://learn.microsoft.com/windows/win32/api/oaidl/nf-oaidl-idispatch-invoke
    // https://learn.microsoft.com/previous-versions/windows/desktop/automat/passing-parameters
    // .NET's ComEventsSink follows the same split in
    // dotnet/runtime:src/libraries/Common/src/System/Runtime/InteropServices/ComEventsSink.cs.
    for raw_index in 0..named_count {
        // SAFETY: `named_count > 0` was null-checked above, and callers pass a
        // DISPPARAMS whose named-argument array is valid for cNamedArgs elements.
        let dispid = unsafe { *params.rgdispidNamedArgs.add(raw_index) };
        if dispid < 0 {
            return Err((COM_DISP_E_PARAMNOTFOUND, Some(raw_index as u32)));
        }
        let declared_index = dispid as usize;
        if declared_index >= expected_arity || used[declared_index] {
            return Err((COM_DISP_E_PARAMNOTFOUND, Some(raw_index as u32)));
        }
        raw_by_declared[declared_index] = raw_index;
        used[declared_index] = true;
    }

    let mut declared_index = 0usize;
    for raw_index in (named_count..cargs).rev() {
        while declared_index < expected_arity && used[declared_index] {
            declared_index += 1;
        }
        if declared_index >= expected_arity {
            return Err((COM_DISP_E_BADPARAMCOUNT, None));
        }
        raw_by_declared[declared_index] = raw_index;
        used[declared_index] = true;
        declared_index += 1;
    }

    if used.iter().any(|slot_used| !slot_used) {
        return Err((COM_DISP_E_BADPARAMCOUNT, None));
    }
    Ok(raw_by_declared)
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn windows_dispatch_event_sink_invoke(
    this: *mut c_void,
    dispidmember: i32,
    _riid: *const GUID,
    _lcid: u32,
    _wflags: u16,
    pparams: *mut DISPPARAMS,
    _pvarresult: *mut VARIANT,
    _pexcepinfo: *mut EXCEPINFO,
    puargerr: *mut u32,
) -> i32 {
    let sink = as_windows_dispatch_event_sink(this);
    if (*sink).event_dispatch_member != dispidmember
        && (*sink).event_dispatch_member != i32::MIN + 3_333
    {
        return COM_DISP_E_MEMBERNOTFOUND;
    }
    let (cargs, rgvarg) = if pparams.is_null() {
        (0usize, std::ptr::null_mut())
    } else {
        ((*pparams).cArgs as usize, (*pparams).rgvarg)
    };
    if pparams.is_null() {
        if (*sink).expected_arity != 0 {
            return COM_DISP_E_BADPARAMCOUNT;
        }
    } else if cargs != (*sink).expected_arity || (cargs > 0 && rgvarg.is_null()) {
        return COM_DISP_E_BADPARAMCOUNT;
    }
    let raw_indices = if pparams.is_null() {
        Vec::new()
    } else {
        match map_event_arg_raw_indices(&*pparams, (*sink).expected_arity) {
            Ok(indices) => indices,
            Err((hr, arg_err)) => {
                if let Some(raw_index) = arg_err
                    && !puargerr.is_null()
                {
                    *puargerr = raw_index;
                }
                return hr;
            }
        }
    };
    let mut args = Vec::with_capacity(cargs);
    for raw_index in raw_indices.iter().copied().take(cargs) {
        let variant = rgvarg.add(raw_index);
        // Object arguments are retained and handed to the runtime state, which binds
        // them through the same native-dispatch path used by COM method returns.
        if let Some(dispatch) = retain_dispatch_arg(&*variant) {
            args.push(WindowsEventArg::Dispatch(dispatch));
            continue;
        }
        let value = match variant_to_com_value(&*variant) {
            Ok(value) => value,
            Err(_) => {
                if !puargerr.is_null() {
                    *puargerr = u32::try_from(raw_index).unwrap_or(u32::MAX);
                }
                return COM_DISP_E_TYPEMISMATCH;
            }
        };
        args.push(WindowsEventArg::Value(value));
    }
    let _ = ((*sink).on_event)(args.as_slice());
    COM_S_OK
}

#[repr(C)]
pub struct RawSingleI32SourceEventsVtbl {
    unknown: crate::RawIUnknownVtbl,
    changed: unsafe extern "system" fn(this: *mut c_void, value: i32) -> i32,
}

#[repr(C)]
pub struct RawSingleI32SourceEvents {
    pub vtbl: *const RawSingleI32SourceEventsVtbl,
}

#[repr(C)]
struct WindowsSingleI32SourceEventSink {
    source: RawSingleI32SourceEvents,
    ref_count: AtomicU32,
    expected_arity: usize,
    connection_point_iid: GUID,
    on_event: DispatchEventCallback,
}

static WINDOWS_SINGLE_I32_SOURCE_EVENT_SINK_VTBL: RawSingleI32SourceEventsVtbl =
    RawSingleI32SourceEventsVtbl {
        unknown: crate::RawIUnknownVtbl {
            query_interface: windows_single_i32_source_event_sink_query_interface,
            add_ref: windows_single_i32_source_event_sink_add_ref,
            release: windows_single_i32_source_event_sink_release,
        },
        changed: windows_single_i32_source_event_sink_changed,
    };

fn create_single_i32_source_event_sink(
    expected_arity: usize,
    connection_point_iid: GUID,
    on_event: DispatchEventCallback,
) -> *mut c_void {
    let sink = Box::new(WindowsSingleI32SourceEventSink {
        source: RawSingleI32SourceEvents {
            vtbl: &WINDOWS_SINGLE_I32_SOURCE_EVENT_SINK_VTBL,
        },
        ref_count: AtomicU32::new(1),
        expected_arity,
        connection_point_iid,
        on_event,
    });
    Box::into_raw(sink).cast::<c_void>()
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_windows_single_i32_source_event_sink(
    this: *mut c_void,
) -> *mut WindowsSingleI32SourceEventSink {
    this.cast::<WindowsSingleI32SourceEventSink>()
}

/// # Safety
///
/// `dispatch` must be a valid live `IDispatch` pointer. `connection_point_iid` must identify a
/// source-interface connection point whose callback shape is a single `i32` argument, and the
/// returned transport must be unadvised exactly once by the caller.
pub unsafe fn try_advise_single_i32_source_interface_event_sink(
    dispatch: *mut RawIDispatch,
    connection_point_iid: &str,
    expected_arity: usize,
    on_event: DispatchEventCallback,
) -> Result<Option<WindowsConnectionPointTransport>, String> {
    if dispatch.is_null() {
        return Err(
            "dispatch pointer was null during source-interface connection-point advise".to_string(),
        );
    }
    let iid = parse_guid_canonical(connection_point_iid)
        .ok_or_else(|| format!("invalid connection-point IID `{connection_point_iid}`"))?;

    let mut container_ptr: *mut c_void = std::ptr::null_mut();
    let hr = ((*(*dispatch).vtbl).unknown.query_interface)(
        dispatch.cast::<c_void>(),
        &IID_ICONNECTIONPOINTCONTAINER,
        &mut container_ptr,
    );
    if hr == COM_E_NOINTERFACE {
        return Ok(None);
    }
    if hr < 0 {
        return Err(format!(
            "QueryInterface(IConnectionPointContainer) failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    if container_ptr.is_null() {
        return Err("QueryInterface(IConnectionPointContainer) returned null".to_string());
    }

    let container = container_ptr.cast::<RawIConnectionPointContainer>();
    let mut connection_point_ptr: *mut c_void = std::ptr::null_mut();
    let find_hr = ((*(*container).vtbl).find_connection_point)(
        container.cast::<c_void>(),
        &iid,
        &mut connection_point_ptr,
    );
    ((*(*container).vtbl).unknown.release)(container.cast::<c_void>());
    if find_hr == COM_E_NOINTERFACE || find_hr == COM_DISP_E_MEMBERNOTFOUND {
        return Ok(None);
    }
    if find_hr < 0 {
        return Err(format!(
            "FindConnectionPoint({connection_point_iid}) failed with HRESULT {:#010X}",
            find_hr as u32
        ));
    }
    if connection_point_ptr.is_null() {
        return Err("FindConnectionPoint returned null".to_string());
    }

    let connection_point = connection_point_ptr.cast::<RawIConnectionPoint>();
    let sink_ptr = create_single_i32_source_event_sink(expected_arity, iid, on_event);
    let mut cookie = 0u32;
    let advise_hr = ((*(*connection_point).vtbl).advise)(
        connection_point.cast::<c_void>(),
        sink_ptr,
        &mut cookie,
    );
    ((*(*(sink_ptr.cast::<crate::RawIUnknown>())).vtbl).release)(sink_ptr);
    if advise_hr == COM_CONNECT_E_CANNOTCONNECT || advise_hr == COM_CONNECT_E_NOCONNECTION {
        release_connection_point(connection_point);
        return Ok(None);
    }
    if advise_hr < 0 || cookie == 0 {
        release_connection_point(connection_point);
        return Err(format!(
            "IConnectionPoint::Advise failed with HRESULT {:#010X}",
            advise_hr as u32
        ));
    }
    Ok(Some(WindowsConnectionPointTransport {
        connection_point: connection_point as usize,
        cookie,
    }))
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn windows_single_i32_source_event_sink_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    let sink = as_windows_single_i32_source_event_sink(this);
    if crate::guid_equals(riid, &IID_IUNKNOWN)
        || crate::guid_equals(riid, &(*sink).connection_point_iid)
    {
        *ppv = this;
        (*sink).ref_count.fetch_add(1, Ordering::AcqRel);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn windows_single_i32_source_event_sink_add_ref(this: *mut c_void) -> u32 {
    let sink = as_windows_single_i32_source_event_sink(this);
    (*sink).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn windows_single_i32_source_event_sink_release(this: *mut c_void) -> u32 {
    let sink = as_windows_single_i32_source_event_sink(this);
    let prev = (*sink).ref_count.fetch_sub(1, Ordering::AcqRel);
    let next = prev.saturating_sub(1);
    if next == 0 {
        drop(Box::from_raw(sink));
    }
    next
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn windows_single_i32_source_event_sink_changed(
    this: *mut c_void,
    value: i32,
) -> i32 {
    let sink = as_windows_single_i32_source_event_sink(this);
    if (*sink).expected_arity != 1 {
        return COM_DISP_E_BADPARAMCOUNT;
    }
    let args = [WindowsEventArg::Value(ComValue::I32(value))];
    let _ = ((*sink).on_event)(&args);
    COM_S_OK
}

#[cfg(test)]
#[allow(clippy::undocumented_unsafe_blocks)]
mod tests {
    use super::*;

    #[test]
    fn event_arg_map_unreverses_plain_positional_args() {
        unsafe {
            let mut variants: [VARIANT; 2] = std::mem::zeroed();
            let params = DISPPARAMS {
                rgvarg: variants.as_mut_ptr(),
                rgdispidNamedArgs: std::ptr::null_mut(),
                cArgs: 2,
                cNamedArgs: 0,
            };
            assert_eq!(map_event_arg_raw_indices(&params, 2), Ok(vec![1, 0]));
        }
    }

    #[test]
    fn event_arg_map_uses_named_dispids_as_declared_positions() {
        unsafe {
            let mut variants: [VARIANT; 2] = std::mem::zeroed();
            let mut named = [0, 1];
            let params = DISPPARAMS {
                rgvarg: variants.as_mut_ptr(),
                rgdispidNamedArgs: named.as_mut_ptr(),
                cArgs: 2,
                cNamedArgs: 2,
            };
            assert_eq!(map_event_arg_raw_indices(&params, 2), Ok(vec![0, 1]));
        }
    }

    #[test]
    fn event_arg_map_combines_named_and_positional_rules() {
        unsafe {
            let mut variants: [VARIANT; 4] = std::mem::zeroed();
            let mut named = [2, 3];
            let params = DISPPARAMS {
                rgvarg: variants.as_mut_ptr(),
                rgdispidNamedArgs: named.as_mut_ptr(),
                cArgs: 4,
                cNamedArgs: 2,
            };
            assert_eq!(map_event_arg_raw_indices(&params, 4), Ok(vec![3, 2, 0, 1]));
        }
    }

    #[test]
    fn event_arg_map_rejects_unknown_named_dispid_with_raw_argerr() {
        unsafe {
            let mut variants: [VARIANT; 1] = std::mem::zeroed();
            let mut named = [7];
            let params = DISPPARAMS {
                rgvarg: variants.as_mut_ptr(),
                rgdispidNamedArgs: named.as_mut_ptr(),
                cArgs: 1,
                cNamedArgs: 1,
            };
            assert_eq!(
                map_event_arg_raw_indices(&params, 1),
                Err((COM_DISP_E_PARAMNOTFOUND, Some(0)))
            );
        }
    }
}
