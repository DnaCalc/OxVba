#[cfg(target_os = "windows")]
#[cfg(target_os = "windows")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Com::{CLSCTX_SERVER, CLSIDFromProgID, CoCreateInstance};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Variant::VARIANT;

#[cfg(target_os = "windows")]
pub const IID_IDISPATCH: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0x0002_0400,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

#[cfg(target_os = "windows")]
pub const IID_NULL: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0,
    data2: 0,
    data3: 0,
    data4: [0; 8],
};

#[cfg(target_os = "windows")]
pub const IID_IUNKNOWN: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0x0000_0000,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

#[cfg(target_os = "windows")]
pub const IID_IENUMVARIANT: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0x0002_0404,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

#[cfg(target_os = "windows")]
pub const IID_ICONNECTIONPOINTCONTAINER: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0xB196_B284,
    data2: 0xBAB4,
    data3: 0x101A,
    data4: [0xB6, 0x9C, 0x00, 0xAA, 0x00, 0x34, 0x1D, 0x07],
};

#[cfg(target_os = "windows")]
pub const IID_ICONNECTIONPOINT: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0xB196_B286,
    data2: 0xBAB4,
    data3: 0x101A,
    data4: [0xB6, 0x9C, 0x00, 0xAA, 0x00, 0x34, 0x1D, 0x07],
};

#[cfg(target_os = "windows")]
pub const COM_S_OK: i32 = 0;
#[cfg(target_os = "windows")]
pub const COM_S_FALSE: i32 = 1;
#[cfg(target_os = "windows")]
pub const COM_E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;
#[cfg(target_os = "windows")]
pub const COM_E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
#[cfg(target_os = "windows")]
pub const COM_E_INVALIDARG: i32 = 0x8007_0057u32 as i32;
#[cfg(target_os = "windows")]
pub const COM_DISP_E_MEMBERNOTFOUND: i32 = 0x8002_0003u32 as i32;
#[cfg(target_os = "windows")]
pub const COM_DISP_E_EXCEPTION: i32 = 0x8002_0009u32 as i32;
#[cfg(target_os = "windows")]
pub const COM_DISP_E_PARAMNOTFOUND: i32 = 0x8002_0004u32 as i32;
#[cfg(target_os = "windows")]
pub const COM_DISP_E_UNKNOWNNAME: i32 = 0x8002_0006u32 as i32;
#[cfg(target_os = "windows")]
pub const COM_DISP_E_BADPARAMCOUNT: i32 = 0x8002_000Eu32 as i32;
#[cfg(target_os = "windows")]
pub const COM_DISP_E_TYPEMISMATCH: i32 = 0x8002_0005u32 as i32;
#[cfg(target_os = "windows")]
pub const COM_DISPID_PROPERTYPUT: i32 = -3;
#[cfg(target_os = "windows")]
pub const COM_CONNECT_E_NOCONNECTION: i32 = 0x8004_0004u32 as i32;
#[cfg(target_os = "windows")]
pub const COM_CONNECT_E_CANNOTCONNECT: i32 = 0x8004_0002u32 as i32;

#[cfg(target_os = "windows")]
#[repr(C)]
pub struct RawIUnknownVtbl {
    pub query_interface: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        riid: *const windows_sys::core::GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32,
    pub release: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
pub struct RawIUnknown {
    pub vtbl: *const RawIUnknownVtbl,
}

#[cfg(target_os = "windows")]
#[repr(C)]
pub struct RawIDispatchVtbl {
    pub unknown: RawIUnknownVtbl,
    pub get_type_info_count:
        unsafe extern "system" fn(this: *mut core::ffi::c_void, pctinfo: *mut u32) -> i32,
    pub get_type_info: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        itinfo: u32,
        lcid: u32,
        pptinfo: *mut *mut core::ffi::c_void,
    ) -> i32,
    pub get_ids_of_names: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        riid: *const windows_sys::core::GUID,
        rgsznames: *mut *mut u16,
        cnames: u32,
        lcid: u32,
        rgdispid: *mut i32,
    ) -> i32,
    pub invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        dispidmember: i32,
        riid: *const windows_sys::core::GUID,
        lcid: u32,
        wflags: u16,
        pparams: *mut windows_sys::Win32::System::Com::DISPPARAMS,
        pvarresult: *mut windows_sys::Win32::System::Variant::VARIANT,
        pexcepinfo: *mut windows_sys::Win32::System::Com::EXCEPINFO,
        puargerr: *mut u32,
    ) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
pub struct RawIDispatch {
    pub vtbl: *const RawIDispatchVtbl,
}

#[cfg(target_os = "windows")]
#[repr(C)]
pub struct RawIEnumVARIANTVtbl {
    pub unknown: RawIUnknownVtbl,
    pub next: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        celt: u32,
        rgvar: *mut VARIANT,
        pcelt_fetched: *mut u32,
    ) -> i32,
    pub skip: unsafe extern "system" fn(this: *mut core::ffi::c_void, celt: u32) -> i32,
    pub reset: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> i32,
    pub clone: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        ppenum: *mut *mut core::ffi::c_void,
    ) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
pub struct RawIEnumVARIANT {
    pub vtbl: *const RawIEnumVARIANTVtbl,
}

#[cfg(target_os = "windows")]
#[repr(C)]
pub struct RawIConnectionPointContainerVtbl {
    pub unknown: RawIUnknownVtbl,
    pub enum_connection_points: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        pp_enum: *mut *mut core::ffi::c_void,
    ) -> i32,
    pub find_connection_point: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        riid: *const windows_sys::core::GUID,
        pp_cp: *mut *mut core::ffi::c_void,
    ) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
pub struct RawIConnectionPointContainer {
    pub vtbl: *const RawIConnectionPointContainerVtbl,
}

#[cfg(target_os = "windows")]
#[repr(C)]
pub struct RawIConnectionPointVtbl {
    pub unknown: RawIUnknownVtbl,
    pub get_connection_interface: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        p_iid: *mut windows_sys::core::GUID,
    ) -> i32,
    pub get_connection_point_container: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        pp_cpc: *mut *mut core::ffi::c_void,
    ) -> i32,
    pub advise: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        p_unk_sink: *mut core::ffi::c_void,
        pdw_cookie: *mut u32,
    ) -> i32,
    pub unadvise: unsafe extern "system" fn(this: *mut core::ffi::c_void, dw_cookie: u32) -> i32,
    pub enum_connections: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        pp_enum: *mut *mut core::ffi::c_void,
    ) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
pub struct RawIConnectionPoint {
    pub vtbl: *const RawIConnectionPointVtbl,
}

#[cfg(target_os = "windows")]
pub fn activate_dispatch_by_prog_id(prog_id: &str) -> Result<*mut RawIDispatch, String> {
    let wide: Vec<u16> = prog_id.encode_utf16().chain(std::iter::once(0)).collect();
    let mut clsid = windows_sys::core::GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    // SAFETY: `wide` is a NUL-terminated UTF-16 buffer built just above and alive
    // across the call, and `&mut clsid` is a live out-slot for one GUID that
    // CLSIDFromProgID writes on success.
    let hr = unsafe { CLSIDFromProgID(wide.as_ptr(), &mut clsid) };
    if hr < 0 {
        return Err(format!(
            "CLSIDFromProgID failed for `{prog_id}` with HRESULT {:#010X}",
            hr as u32
        ));
    }

    let mut dispatch_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
    // SAFETY: `clsid` was populated by the successful CLSIDFromProgID call above,
    // `IID_IDISPATCH` is a 'static GUID, and `&mut dispatch_ptr` is a live out-slot.
    // An uninitialized COM apartment is not UB here: CoCreateInstance then fails
    // with an HRESULT (e.g. CO_E_NOTINITIALIZED), which the hr/null checks below
    // turn into an Err. On success the returned IDispatch reference is owned by the
    // caller (one Release is owed).
    let hr = unsafe {
        CoCreateInstance(
            &clsid,
            std::ptr::null_mut(),
            CLSCTX_SERVER,
            &IID_IDISPATCH,
            &mut dispatch_ptr,
        )
    };
    if hr < 0 {
        return Err(format!(
            "CoCreateInstance failed for `{prog_id}` with HRESULT {:#010X}",
            hr as u32
        ));
    }
    if dispatch_ptr.is_null() {
        return Err("CoCreateInstance returned a null IDispatch pointer".to_string());
    }
    Ok(dispatch_ptr.cast::<RawIDispatch>())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
///
/// `lhs` must be either null or point to a valid `GUID` for the duration of the call.
pub unsafe fn guid_equals(
    lhs: *const windows_sys::core::GUID,
    rhs: &windows_sys::core::GUID,
) -> bool {
    if lhs.is_null() {
        return false;
    }
    let lhs = &*lhs;
    lhs.data1 == rhs.data1
        && lhs.data2 == rhs.data2
        && lhs.data3 == rhs.data3
        && lhs.data4 == rhs.data4
}

#[cfg(target_os = "windows")]
pub fn parse_guid_canonical(input: &str) -> Option<windows_sys::core::GUID> {
    let normalized = normalize_guid_like(input);
    let parts: Vec<&str> = normalized.split('-').collect();
    if parts.len() != 5
        || parts[0].len() != 8
        || parts[1].len() != 4
        || parts[2].len() != 4
        || parts[3].len() != 4
        || parts[4].len() != 12
    {
        return None;
    }
    let data1 = u32::from_str_radix(parts[0], 16).ok()?;
    let data2 = u16::from_str_radix(parts[1], 16).ok()?;
    let data3 = u16::from_str_radix(parts[2], 16).ok()?;
    let mut data4 = [0u8; 8];
    data4[0] = u8::from_str_radix(&parts[3][0..2], 16).ok()?;
    data4[1] = u8::from_str_radix(&parts[3][2..4], 16).ok()?;
    for idx in 0..6 {
        let start = idx * 2;
        let end = start + 2;
        data4[idx + 2] = u8::from_str_radix(&parts[4][start..end], 16).ok()?;
    }
    Some(windows_sys::core::GUID {
        data1,
        data2,
        data3,
        data4,
    })
}

#[cfg(target_os = "windows")]
fn normalize_guid_like(input: &str) -> String {
    input
        .trim()
        .trim_matches('{')
        .trim_matches('}')
        .to_ascii_lowercase()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
///
/// `dispatch` must be null or a valid live `IDispatch` pointer whose vtable can be dereferenced.
pub unsafe fn add_ref_dispatch(dispatch: *mut RawIDispatch) -> u32 {
    if dispatch.is_null() {
        return 0;
    }
    let vtbl = (*dispatch).vtbl;
    ((*vtbl).unknown.add_ref)(dispatch.cast())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// Attempt to obtain an `IDispatch` pointer from a raw `IUnknown` via `QueryInterface`.
///
/// **Non-IDispatch interface-pointer policy:** VBA's late-bound object model requires
/// `IDispatch`. COM objects that only expose `IUnknown` (without `IDispatch`) are
/// deterministically rejected with a stable diagnostic containing the failing HRESULT.
/// This matches VBA 7.1 behavior where non-`IDispatch` COM objects cannot participate
/// in late-bound dispatch, property access, or event subscription.
///
/// # Safety
///
/// `unknown` must be a valid live COM interface pointer whose `IUnknown` vtable can be called.
pub unsafe fn query_dispatch_from_unknown(
    unknown: *mut RawIUnknown,
) -> Result<*mut RawIDispatch, String> {
    if unknown.is_null() {
        return Err("VT_UNKNOWN result carried null IUnknown pointer".to_string());
    }
    let mut dispatch: *mut core::ffi::c_void = std::ptr::null_mut();
    let vtbl = (*unknown).vtbl;
    let hr = ((*vtbl).query_interface)(unknown.cast(), &IID_IDISPATCH, &mut dispatch);
    if hr < 0 || dispatch.is_null() {
        return Err(format!(
            "IUnknown::QueryInterface(IDispatch) failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    Ok(dispatch.cast())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// Attempt to obtain the canonical `IUnknown` identity pointer from a live dispatch pointer.
///
/// # Safety
///
/// `dispatch` must be a valid live COM `IDispatch` pointer whose `IUnknown`
/// vtable can be called.
pub unsafe fn query_unknown_from_dispatch(
    dispatch: *mut RawIDispatch,
) -> Result<*mut RawIUnknown, String> {
    if dispatch.is_null() {
        return Err("IDispatch identity query received null dispatch pointer".to_string());
    }
    let mut unknown: *mut core::ffi::c_void = core::ptr::null_mut();
    let vtbl = (*dispatch).vtbl;
    let hr = ((*vtbl).unknown.query_interface)(dispatch.cast(), &IID_IUNKNOWN, &mut unknown);
    if hr < 0 || unknown.is_null() {
        return Err(format!(
            "IDispatch::QueryInterface(IUnknown) failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    Ok(unknown.cast())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// `QueryInterface` a live COM object for an arbitrary interface IID, returning a
/// fresh interface pointer that carries one reference the caller must `Release`.
///
/// Workset S5a uses this to obtain the typelib-declared **dual interface** pointer
/// for an early-bound member: the oleaut universal marshaler (`PSOAInterface`)
/// builds a real vtable proxy for any oleaut-compatible dual interface, so the
/// returned pointer is vtable-callable uniformly in-process and out-of-process —
/// exactly how the VBA IDE holds an early-bound reference. Returns `Err` on
/// `E_NOINTERFACE` / any failing HRESULT / a null result, so the dispatch site can
/// cleanly fall back to `IDispatch` (it must NEVER vtable-call without a verified
/// pointer for the exact IID).
///
/// # Safety
///
/// `object` must be a valid live COM interface pointer whose first field is an
/// `IUnknown` vtable, held alive for the duration of the call.
pub unsafe fn query_interface_pointer(
    object: *mut core::ffi::c_void,
    iid: &windows_sys::core::GUID,
) -> Result<*mut core::ffi::c_void, String> {
    if object.is_null() {
        return Err("QueryInterface received a null object pointer".to_string());
    }
    let unknown = object.cast::<RawIUnknown>();
    let mut interface: *mut core::ffi::c_void = core::ptr::null_mut();
    let vtbl = (*unknown).vtbl;
    let hr = ((*vtbl).query_interface)(unknown.cast(), iid, &mut interface);
    if hr < 0 || interface.is_null() {
        return Err(format!(
            "IUnknown::QueryInterface(custom interface) failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    Ok(interface)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
///
/// `unknown` must be a valid live COM interface pointer whose `IUnknown` vtable can be called.
pub unsafe fn query_enumvariant_from_unknown(
    unknown: *mut RawIUnknown,
) -> Result<*mut RawIEnumVARIANT, String> {
    if unknown.is_null() {
        return Err("VT_UNKNOWN result carried null IUnknown pointer".to_string());
    }
    let mut enum_variant: *mut core::ffi::c_void = std::ptr::null_mut();
    let vtbl = (*unknown).vtbl;
    let hr = ((*vtbl).query_interface)(unknown.cast(), &IID_IENUMVARIANT, &mut enum_variant);
    if hr < 0 || enum_variant.is_null() {
        return Err(format!(
            "IUnknown::QueryInterface(IEnumVARIANT) failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    Ok(enum_variant.cast())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
///
/// `dispatch` must be null or a valid `IDispatch` pointer owned by the caller for one final `Release` call.
pub unsafe fn release_dispatch(dispatch: *mut RawIDispatch) {
    if dispatch.is_null() {
        return;
    }
    let vtbl = (*dispatch).vtbl;
    ((*vtbl).unknown.release)(dispatch.cast());
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
///
/// `enum_variant` must be null or a valid `IEnumVARIANT` pointer owned by the caller for one
/// final `Release` call.
pub unsafe fn release_enumvariant(enum_variant: *mut RawIEnumVARIANT) {
    if enum_variant.is_null() {
        return;
    }
    let vtbl = (*enum_variant).vtbl;
    ((*vtbl).unknown.release)(enum_variant.cast());
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
///
/// `connection_point` must be null or a valid `IConnectionPoint` pointer owned by the caller for one final `Release` call.
pub unsafe fn release_connection_point(connection_point: *mut RawIConnectionPoint) {
    if connection_point.is_null() {
        return;
    }
    let vtbl = (*connection_point).vtbl;
    ((*vtbl).unknown.release)(connection_point.cast());
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
///
/// `unknown` must be null or point to a valid COM interface whose first field is an `IUnknown` vtable.
pub unsafe fn release_unknown(unknown: *mut core::ffi::c_void) {
    if unknown.is_null() {
        return;
    }
    let unknown = unknown.cast::<RawIUnknown>();
    let vtbl = (*unknown).vtbl;
    ((*vtbl).release)(unknown.cast());
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
///
/// `dispatch` must be a valid live `IDispatch` pointer for the duration of the lookup.
pub unsafe fn get_dispid_by_name(dispatch: *mut RawIDispatch, name: &str) -> Result<i32, String> {
    let dispids = get_dispids_by_names(dispatch, &[name])?;
    Ok(dispids[0])
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
///
/// `dispatch` must be a valid live `IDispatch` pointer for the duration of the lookup.
pub unsafe fn get_dispids_by_names(
    dispatch: *mut RawIDispatch,
    names: &[&str],
) -> Result<Vec<i32>, String> {
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let mut wide_names: Vec<Vec<u16>> = names
        .iter()
        .map(|name| name.encode_utf16().chain(std::iter::once(0)).collect())
        .collect();
    let mut name_ptrs: Vec<*mut u16> = wide_names
        .iter_mut()
        .map(|name| name.as_mut_ptr())
        .collect();
    let mut dispids = vec![0i32; names.len()];
    let hr = ((*(*dispatch).vtbl).get_ids_of_names)(
        dispatch.cast(),
        &IID_NULL,
        name_ptrs.as_mut_ptr(),
        names.len() as u32,
        0x0400,
        dispids.as_mut_ptr(),
    );
    if hr < 0 {
        return Err(format!(
            "IDispatch::GetIDsOfNames failed for `{}` with HRESULT {:#010X}",
            names.join(", "),
            hr as u32
        ));
    }
    Ok(dispids)
}

#[cfg(target_os = "windows")]
pub fn activate_runtime_dispatch(
    prog_id: &str,
    force_registered_test_dispatch: bool,
) -> Result<*mut RawIDispatch, String> {
    #[cfg(not(any(test, feature = "fixture-typelibs")))]
    let _ = force_registered_test_dispatch;

    #[cfg(any(test, feature = "fixture-typelibs"))]
    if prog_id.eq_ignore_ascii_case(crate::windows_test_dispatch::OXVBA_TEST_DISPATCH_PROGID)
        && !force_registered_test_dispatch
    {
        return Ok(crate::windows_test_dispatch::create_oxvba_test_dispatch());
    }
    activate_dispatch_by_prog_id(prog_id)
}

#[cfg(target_os = "windows")]
pub fn activate_runtime_binding(
    prog_id: &str,
    metadata: Option<&crate::TypeLibMetadataBlob>,
    force_registered_test_dispatch: bool,
) -> Result<crate::ComBinding, String> {
    let dispatch = activate_runtime_dispatch(prog_id, force_registered_test_dispatch)?;
    // When the ProgID could not be resolved to a typelib through the registry the
    // caller hands us `metadata = None` — the case for `Excel.Application`, whose
    // `HKCR\CLSID\{clsid}` key carries no `\TypeLib` subkey. Recover the typelib
    // from the live dispatch's own type information so the binding still gets
    // member/event specs; without this an out-of-process object's `event_specs`
    // stay empty and a `WithEvents` subscription on it cannot resolve.
    let recovered_metadata = if metadata.is_none() && !dispatch.is_null() {
        // SAFETY: `dispatch` is the live IDispatch just activated (non-null
        // checked) and is owned by us for the duration of this call.
        unsafe {
            crate::windows_typelib_loader::build_metadata_blob_from_dispatch(dispatch, prog_id)
        }
    } else {
        None
    };
    let effective_metadata = metadata.or(recovered_metadata.as_ref());
    let mut binding = crate::binding_from_typelib_metadata(
        prog_id.to_string(),
        dispatch as usize,
        effective_metadata,
    );
    // Record the canonical IUnknown identity so that a `this`/back-reference
    // returned by this object (e.g. ReturnSelfObject, ws.Application) dedups to
    // the original handle in `bind_native_dispatch_result` rather than minting a
    // fresh one — preserving `Is` identity. Mirrors `bind_host_dispatch_object`:
    // the retained IUnknown reference is owned by the binding for its lifetime
    // and released in `release_object_binding`. On QI failure we leave
    // `native_unknown` at 0 (identity dedup simply won't apply) rather than fail
    // activation — the dispatch reference stays owned by the binding either way.
    if !dispatch.is_null() {
        // SAFETY: `dispatch` is non-null (checked) and carries the single
        // retained reference handed back by `activate_runtime_dispatch`, now
        // owned by `binding` via `native_dispatch`, so it is a live `IDispatch`
        // whose `IUnknown` vtable can be queried.
        if let Ok(unknown) = unsafe { query_unknown_from_dispatch(dispatch) } {
            binding.native_unknown = unknown as usize;
        }
    }
    Ok(binding)
}

#[cfg(target_os = "windows")]
pub fn activate_runtime_object_binding_shared<F>(
    com_state: &Arc<Mutex<crate::WindowsComClientState>>,
    prog_id: &str,
    metadata: Option<&crate::TypeLibMetadataBlob>,
    force_registered_test_dispatch: bool,
    mut configure_binding: F,
) -> Result<oxvba_runtime::ObjectRef, String>
where
    F: FnMut(&mut crate::ComBinding) -> Result<(), String>,
{
    let mut binding = activate_runtime_binding(prog_id, metadata, force_registered_test_dispatch)?;
    configure_binding(&mut binding)?;
    crate::insert_bound_object_binding_shared(com_state, binding)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
///
/// `dispatch` must be a valid live `IDispatch` pointer.
pub unsafe fn resolve_named_argument_dispids(
    dispatch: *mut RawIDispatch,
    member_name: &str,
    args: &[crate::ComInvokeArg],
) -> Result<Vec<i32>, String> {
    let mut names = Vec::with_capacity(args.len().saturating_add(1));
    names.push(member_name.to_string());
    for arg in args {
        if let Some(name) = &arg.name {
            names.push(name.clone());
        }
    }
    if names.len() == 1 {
        return Ok(Vec::new());
    }
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    Ok(get_dispids_by_names(dispatch, &name_refs)?
        .into_iter()
        .skip(1)
        .collect())
}
