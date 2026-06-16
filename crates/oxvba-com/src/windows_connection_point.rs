#![allow(unsafe_op_in_unsafe_fn)]

use crate::ComValue;
use crate::windows_client::{
    COM_CONNECT_E_CANNOTCONNECT, COM_CONNECT_E_NOCONNECTION, COM_DISP_E_BADPARAMCOUNT,
    COM_DISP_E_MEMBERNOTFOUND, COM_DISP_E_TYPEMISMATCH, COM_DISP_E_UNKNOWNNAME, COM_E_INVALIDARG,
    COM_E_NOINTERFACE, COM_E_NOTIMPL, COM_S_OK, IID_ICONNECTIONPOINTCONTAINER, IID_IDISPATCH,
    IID_IUNKNOWN, RawIConnectionPoint, RawIConnectionPointContainer, RawIDispatch,
    RawIDispatchVtbl, parse_guid_canonical, release_connection_point,
};
use crate::windows_variant::variant_to_com_value;
use std::ffi::c_void;
use std::sync::Arc;
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicU32, Ordering};
use windows_sys::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, DISPPARAMS, EXCEPINFO,
};
use windows_sys::Win32::System::Variant::VARIANT;
use windows_sys::core::GUID;

/// Event-sink callback: `(args, object_marshals) -> consumed`. `args` carries
/// every event argument (object-typed slots hold a `Nothing` placeholder), and
/// `object_marshals` carries `(arg_index, git_cookie)` for each object arg that was
/// registered in the Global Interface Table on the delivery thread for the VM
/// thread to revive (see [`ComEventCallback::pending_marshals`]).
type DispatchEventCallback = Arc<dyn Fn(&[ComValue], &[(usize, u32)]) -> bool + Send + Sync>;

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

/// `IID_IMarshal` `{00000003-0000-0000-C000-000000000046}` — the interface COM queries an
/// object for when marshalling it across an apartment boundary. Delegating it to an
/// aggregated free-threaded marshaler is what makes a sink AGILE, so an out-of-process
/// server (Excel) can call it back directly instead of through an STA stub that deadlocks
/// our outbound `Advise`. See docs/COM_OOP_EVENT_SINK_MARSHALLING.md.
#[cfg(target_os = "windows")]
const IID_IMARSHAL: GUID = GUID {
    data1: 0x0000_0003,
    data2: 0,
    data3: 0,
    data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};

// `windows-sys` 0.59 does not surface this ole32 export, so declare it directly (as the
// vtable module does for Get/SetErrorInfo). It creates an aggregatable free-threaded
// marshaler: `punkouter` is the controlling IUnknown (our sink), `ppunkmarshal` receives
// the inner IUnknown we own.
#[cfg(target_os = "windows")]
unsafe extern "system" {
    fn CoCreateFreeThreadedMarshaler(punkouter: *mut c_void, ppunkmarshal: *mut *mut c_void) -> i32;
    fn GetCurrentThreadId() -> u32;
}

// ── Global Interface Table: cross-apartment handoff of object event arguments ──
//
// An out-of-process source (Excel) delivers events to our AGILE sink on an MTA RPC
// worker thread, but the VM consumes them on its STA thread. An interface pointer
// marshalled into the MTA apartment cannot be touched from the STA, so every
// object-typed event argument is registered in the process-global GIT on the
// delivery thread (`register_object_in_git`) and revived on the VM thread at poll
// time (`take_dispatch_from_git`). This is the canonical mechanism for handing a
// COM interface across apartments.
#[cfg(target_os = "windows")]
const CLSID_STD_GLOBAL_INTERFACE_TABLE: GUID = GUID {
    data1: 0x0000_0323,
    data2: 0,
    data3: 0,
    data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};
#[cfg(target_os = "windows")]
const IID_IGLOBAL_INTERFACE_TABLE: GUID = GUID {
    data1: 0x0000_0146,
    data2: 0,
    data3: 0,
    data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};
#[cfg(target_os = "windows")]
#[repr(C)]
struct IGlobalInterfaceTableVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    register_interface_in_global:
        unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, *mut u32) -> i32,
    revoke_interface_from_global: unsafe extern "system" fn(*mut c_void, u32) -> i32,
    get_interface_from_global:
        unsafe extern "system" fn(*mut c_void, u32, *const GUID, *mut *mut c_void) -> i32,
}

/// Obtains the process Global Interface Table singleton. The returned pointer carries
/// one reference the caller must Release.
///
/// # Safety
/// The current thread must be COM-initialized.
#[cfg(target_os = "windows")]
unsafe fn obtain_git() -> Option<*mut c_void> {
    let mut git: *mut c_void = std::ptr::null_mut();
    // SAFETY: all four pointer args are valid ('static GUIDs and a live out-slot);
    // CoCreateInstance writes a retained interface pointer into `git` on success.
    let hr = unsafe {
        CoCreateInstance(
            &CLSID_STD_GLOBAL_INTERFACE_TABLE,
            std::ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IGLOBAL_INTERFACE_TABLE,
            &mut git,
        )
    };
    if hr < 0 || git.is_null() {
        return None;
    }
    Some(git)
}

/// Registers a live interface pointer in the process GIT and returns its cookie.
/// Called on the event-delivery (MTA) thread, where `punk` is valid. The GIT
/// holds its own reference until the cookie is revoked, so the registration
/// outlives the inbound `Invoke` whose borrowed argument reference goes away when
/// the source's call returns.
///
/// # Safety
/// `punk` must be a live COM interface pointer and the current thread COM-initialized.
#[cfg(target_os = "windows")]
pub unsafe fn register_object_in_git(punk: *mut c_void) -> Option<u32> {
    if punk.is_null() {
        return None;
    }
    // SAFETY: the delivery thread is COM-initialized (it is servicing an inbound RPC).
    let git = unsafe { obtain_git() }?;
    // SAFETY: `git` is the live GIT singleton; its first field is the vtable.
    let vtbl = unsafe { *(git as *const *const IGlobalInterfaceTableVtbl) };
    let mut cookie: u32 = 0;
    // SAFETY: `punk` is live per the contract, `IID_IDISPATCH` is a 'static GUID, and
    // `&mut cookie` is a live out-slot; RegisterInterfaceInGlobal QIs `punk` for the IID
    // and AddRefs its own reference on success.
    let reg_hr =
        unsafe { ((*vtbl).register_interface_in_global)(git, punk, &IID_IDISPATCH, &mut cookie) };
    // SAFETY: `git` carries one reference from `obtain_git`; this balances it.
    unsafe { ((*vtbl).release)(git) };
    if reg_hr < 0 || cookie == 0 {
        None
    } else {
        Some(cookie)
    }
}

/// Revives a GIT-registered interface as an `IDispatch` valid in the CALLING
/// apartment (the VM/STA thread at poll time) and revokes the registration. The
/// returned pointer carries one reference the caller takes ownership of.
///
/// # Safety
/// The current thread must be COM-initialized.
#[cfg(target_os = "windows")]
pub unsafe fn take_dispatch_from_git(cookie: u32) -> Option<*mut RawIDispatch> {
    // SAFETY: the VM thread is COM-initialized before any event poll runs.
    let git = unsafe { obtain_git() }?;
    // SAFETY: `git` is the live GIT singleton; its first field is the vtable.
    let vtbl = unsafe { *(git as *const *const IGlobalInterfaceTableVtbl) };
    let mut ppv: *mut c_void = std::ptr::null_mut();
    // SAFETY: `IID_IDISPATCH` is a 'static GUID and `&mut ppv` a live out-slot;
    // GetInterfaceFromGlobal stores a retained, apartment-correct proxy on success.
    let get_hr =
        unsafe { ((*vtbl).get_interface_from_global)(git, cookie, &IID_IDISPATCH, &mut ppv) };
    // SAFETY: the cookie is consumed exactly once here, releasing the GIT's reference.
    unsafe {
        let _ = ((*vtbl).revoke_interface_from_global)(git, cookie);
        ((*vtbl).release)(git);
    }
    if get_hr < 0 || ppv.is_null() {
        None
    } else {
        Some(ppv.cast::<RawIDispatch>())
    }
}

/// Revokes a GIT registration without unmarshalling it — used to release object
/// args whose callback is purged (object released) before it is ever pumped, so
/// the GIT does not pin the source object forever.
///
/// # Safety
/// The current thread must be COM-initialized.
#[cfg(target_os = "windows")]
pub unsafe fn revoke_git_cookie(cookie: u32) {
    // SAFETY: the VM thread is COM-initialized when bindings are released.
    let Some(git) = (unsafe { obtain_git() }) else {
        return;
    };
    // SAFETY: `git` is the live GIT singleton; its first field is the vtable.
    let vtbl = unsafe { *(git as *const *const IGlobalInterfaceTableVtbl) };
    // SAFETY: revoke is idempotent-safe for a once-registered cookie; release balances obtain.
    unsafe {
        let _ = ((*vtbl).revoke_interface_from_global)(git, cookie);
        ((*vtbl).release)(git);
    }
}

/// The live `IDispatch`/`IUnknown` pointer carried by an object-typed event-argument
/// VARIANT (`VT_DISPATCH`/`VT_UNKNOWN`, optionally `VT_BYREF`), or `None` for any
/// non-object VARIANT.
///
/// # Safety
/// `variant` must be a live VARIANT for the duration of the read.
#[cfg(target_os = "windows")]
unsafe fn object_arg_ptr(variant: &VARIANT) -> Option<*mut c_void> {
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
            variant.Anonymous.Anonymous.Anonymous.pdispVal.cast::<c_void>()
        }
    };
    if ptr.is_null() { None } else { Some(ptr) }
}

#[repr(C)]
struct WindowsDispatchEventSink {
    dispatch: RawIDispatch,
    ref_count: AtomicU32,
    event_dispatch_member: i32,
    expected_arity: usize,
    connection_point_iid: Option<GUID>,
    on_event: DispatchEventCallback,
    /// The thread that CREATED (advised) this sink — the subscriber's own thread.
    /// `windows_dispatch_event_sink_invoke` compares the calling thread to this to choose
    /// the rgvarg layout. The two layouts and the rule are PROVEN by a live per-slot rgvarg
    /// dump (in-proc `OnPairChanged(10,11)` and OOP Excel `Workbook.SheetChange(Sh,Target)`):
    ///   - a DIRECT in-process IDispatch call fires on this same thread and builds rgvarg in
    ///     the standard REVERSED convention (`rgvarg[0]` = last declared arg) — confirmed:
    ///     `rgvarg[0]=11(=b)`, `rgvarg[1]=10(=a)`;
    ///   - an OUT-OF-PROCESS call, marshaled to our agile sink through the oleaut IDispatch
    ///     proxy/stub, lands on a different (RPC worker) thread and is reconstructed by the
    ///     stub in DECLARED (forward) order (`rgvarg[0]` = first declared arg) — confirmed:
    ///     `rgvarg[0]`=Sh(Worksheet, no `.Column`), `rgvarg[1]`=Target(Range, has `.Column`).
    /// So the layout tracks the TRANSPORT (direct vs oleaut-remoted), and the calling-thread
    /// vs advise-thread comparison is a reliable proxy for it (direct calls stay on the
    /// advise/STA thread; remoted calls arrive on an RPC worker). Apartment type cannot be
    /// substituted — an agile call reports neutral, not MTA.
    ///
    /// KNOWN (non-VBA) GAP: the thread proxy breaks only for an in-process FREE-THREADED
    /// source that fires a multi-arg event DIRECTLY (reversed) from a non-advise thread —
    /// it would be misread as forward. No VBA-reachable source does this: project class
    /// instances and ordinary in-proc COM servers raise events on the calling (VM/STA)
    /// thread. See docs/COM_OOP_EVENT_SINK_MARSHALLING.md.
    created_thread_id: u32,
    /// The aggregated free-threaded marshaler's inner `IUnknown` (one owned reference),
    /// or null if aggregation failed (the sink then falls back to standard marshalling —
    /// fine in-process). `QueryInterface(IID_IMarshal)` delegates here so COM marshals the
    /// sink agilely to an out-of-process source. Released when the sink is destroyed.
    ftm: *mut c_void,
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
        // SAFETY: GetCurrentThreadId has no preconditions and cannot fail.
        created_thread_id: unsafe { GetCurrentThreadId() },
        ftm: std::ptr::null_mut(),
    });
    let raw = Box::into_raw(sink);
    // Aggregate the free-threaded marshaler so the sink is AGILE: an out-of-process source
    // (Excel) then calls it back directly rather than through an STA stub, which is what
    // deadlocked the cross-apartment `Advise`. The sink IS the controlling IUnknown
    // (`raw`); the FTM's inner IUnknown is stored for IID_IMarshal delegation + released on
    // destroy. On failure we leave `ftm` null and fall back to standard marshalling (which
    // is correct in-process, where no marshalling occurs).
    #[cfg(target_os = "windows")]
    // SAFETY: `raw` is the freshly-boxed sink whose first field is its IUnknown vtable, so
    // it is a valid controlling IUnknown for aggregation; `ftm` is a valid out-pointer. We
    // store the returned inner reference only on success and Release it exactly once on
    // destroy.
    unsafe {
        let mut ftm: *mut c_void = std::ptr::null_mut();
        let hr = CoCreateFreeThreadedMarshaler(raw.cast::<c_void>(), &mut ftm);
        if hr >= 0 {
            (*raw).ftm = ftm;
        }
    }
    raw.cast::<c_void>()
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
    // Delegate IID_IMarshal to the aggregated free-threaded marshaler so COM marshals the
    // sink AGILELY to an out-of-process source (the FTM's QI delegates AddRef/Release back
    // to this controlling IUnknown per aggregation rules, so the refcount stays balanced).
    if crate::guid_equals(riid, &IID_IMARSHAL) && !(*sink).ftm.is_null() {
        let ftm = (*sink).ftm;
        // SAFETY: `ftm` is the FTM's inner IUnknown (one ref we own); its first field is its
        // vtable, whose first slot is `QueryInterface(this, riid, ppv)`.
        let vtbl = *(ftm as *const *const crate::RawIUnknownVtbl);
        return ((*vtbl).query_interface)(ftm, riid, ppv);
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
        // Release the aggregated free-threaded marshaler's inner reference before freeing
        // the box (the FTM does not hold a ref on this controlling object, so no cycle).
        let ftm = (*sink).ftm;
        if !ftm.is_null() {
            let vtbl = *(ftm as *const *const crate::RawIUnknownVtbl);
            ((*vtbl).release)(ftm);
        }
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
    if cargs != (*sink).expected_arity || (cargs > 0 && rgvarg.is_null()) {
        return COM_DISP_E_BADPARAMCOUNT;
    }
    let mut args = Vec::with_capacity(cargs);
    let mut marshals: Vec<(usize, u32)> = Vec::new();
    // Recover the args into DECLARED order, `args[i]` = the i-th declared event parameter.
    // The DISPPARAMS layout depends on the TRANSPORT (proven by a live per-slot dump — see
    // `created_thread_id`): a DIRECT in-process call presents the standard IDispatch REVERSED
    // order (`rgvarg[0]` = last declared arg) on the advise thread; an OOP call marshaled to
    // our agile sink through the oleaut IDispatch stub presents DECLARED (forward) order on an
    // RPC worker thread. The calling-thread vs advise-thread comparison reliably distinguishes
    // them. (Single-arg events cannot reveal the order; it surfaced with the first 2-arg event.)
    // SAFETY: GetCurrentThreadId has no preconditions.
    let forward = unsafe { GetCurrentThreadId() } != (*sink).created_thread_id;
    for arg_index in 0..cargs {
        let raw_index = if forward {
            arg_index
        } else {
            cargs.saturating_sub(1).saturating_sub(arg_index)
        };
        let variant = rgvarg.add(raw_index);
        // Object-typed arguments belong to the source's apartment; register them in
        // the GIT here (on the delivery thread, where they are valid) and queue a
        // `Nothing` placeholder for the VM thread to revive at poll time. If
        // registration fails (a pure IUnknown with no IDispatch, or a GIT failure),
        // degrade THIS argument to `Nothing` rather than failing the entire event:
        // falling through to `variant_to_com_value` (which cannot carry an object)
        // would return DISP_E_TYPEMISMATCH and abort the whole inbound Invoke over a
        // single un-revivable object arg. This mirrors
        // `resolve_pending_event_marshals_for_next`, which also leaves `Nothing` on a
        // revive miss. `variant_to_com_value` then handles every scalar/string arg.
        if let Some(punk) = object_arg_ptr(&*variant) {
            if let Some(cookie) = register_object_in_git(punk) {
                marshals.push((arg_index, cookie));
            }
            args.push(ComValue::Object(
                oxvba_runtime::ObjectRef::from_compat_identity(0),
            ));
            continue;
        }
        let value = match variant_to_com_value(&*variant) {
            Ok(value) => value,
            Err(_) => {
                if !puargerr.is_null() {
                    *puargerr = u32::try_from(arg_index).unwrap_or(u32::MAX);
                }
                return COM_DISP_E_TYPEMISMATCH;
            }
        };
        args.push(value);
    }
    let _ = ((*sink).on_event)(args.as_slice(), marshals.as_slice());
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
    let args = [ComValue::I32(value)];
    // Source-interface i32 events carry no object arguments, so no GIT marshals.
    let _ = ((*sink).on_event)(&args, &[]);
    COM_S_OK
}
