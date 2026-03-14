#![allow(unsafe_op_in_unsafe_fn)]

use crate::windows_client::{
    COM_CONNECT_E_CANNOTCONNECT, COM_CONNECT_E_NOCONNECTION, COM_DISP_E_BADPARAMCOUNT,
    COM_DISP_E_EXCEPTION, COM_DISP_E_MEMBERNOTFOUND, COM_DISP_E_TYPEMISMATCH,
    COM_DISP_E_UNKNOWNNAME, COM_E_INVALIDARG, COM_E_NOINTERFACE, COM_E_NOTIMPL, COM_S_OK,
    IID_ICONNECTIONPOINT, IID_ICONNECTIONPOINTCONTAINER, IID_IDISPATCH, IID_IUNKNOWN, IID_NULL,
    RawIConnectionPointContainerVtbl, RawIConnectionPointVtbl, RawIDispatch, RawIDispatchVtbl,
    RawIUnknown, RawIUnknownVtbl, add_ref_dispatch as raw_add_ref_dispatch, guid_equals,
    release_dispatch as raw_release_dispatch, release_unknown as raw_release_unknown,
};
use crate::windows_variant::{
    set_variant_from_com_value as com_set_variant_from_com_value,
    variant_to_com_value as com_variant_to_com_value,
};
use crate::{COM_DISPID_PROPERTYPUT, ComValue};
use oxvba_runtime::{
    ObjectHandle,
    value_tags::{NULL_TAG, error_tag_from_code},
};
use std::{
    collections::BTreeMap,
    sync::Mutex,
    sync::atomic::{AtomicI32, AtomicU32, Ordering},
};
use windows_sys::Win32::{
    Foundation::{SysAllocString, SysFreeString, VARIANT_BOOL},
    System::{
        Com::{
            DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPATCH_PROPERTYPUTREF,
            DISPPARAMS, EXCEPINFO,
        },
        Ole::{
            SafeArrayCreateVector, SafeArrayDestroy, SafeArrayGetDim, SafeArrayGetElement,
            SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayGetVartype, SafeArrayPutElement,
        },
        Variant::{
            VARIANT, VT_ARRAY, VT_BOOL, VT_BSTR, VT_DISPATCH, VT_EMPTY, VT_ERROR, VT_I2, VT_I4,
            VT_NULL, VT_UI2, VT_UI4, VT_UNKNOWN, VT_VARIANT, VariantClear,
        },
    },
};

type RawDispatchPtr = usize;
type RawUnknownPtr = usize;

pub const OXVBA_TEST_DISPATCH_PROGID: &str = "OxVba.TestDispatch";
pub const IID_OXVBA_TEST_DISPATCH_EVENTS: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0x1111_1112,
    data2: 0x2222,
    data3: 0x3333,
    data4: [0x44, 0x44, 0x55, 0x55, 0x55, 0x55, 0x55, 0x56],
};
pub const IID_OXVBA_TEST_DISPATCH_EVENTS_STR: &str = "11111112-2222-3333-4444-555555555556";
pub const IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS: windows_sys::core::GUID =
    windows_sys::core::GUID {
        data1: 0x1111_1113,
        data2: 0x2222,
        data3: 0x3333,
        data4: [0x44, 0x44, 0x55, 0x55, 0x55, 0x55, 0x55, 0x57],
    };
pub const IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS_STR: &str = "11111113-2222-3333-4444-555555555557";
pub const TEST_DISPID_COUNT: i32 = 1;
pub const TEST_DISPID_EXISTS: i32 = 2;
pub const TEST_DISPID_FIRE_CHANGED: i32 = 3;
pub const TEST_DISPID_FIRE_CHANGED_PAIR: i32 = 4;
pub const TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE: i32 = 11;
pub const TEST_DISPID_PING: i32 = 5;
pub const TEST_DISPID_LOOKUP: i32 = 6;
pub const TEST_DISPID_SET_VALUE: i32 = 7;
pub const TEST_DISPID_SET_VALUE_REF: i32 = 8;
pub const TEST_DISPID_VALUE: i32 = 9;
pub const TEST_DISPID_EXCEL_QUIT: i32 = 10;
pub const TEST_DISPID_SUM_PAIR: i32 = 12;
pub const TEST_DISPID_LOOKUP_PAIR: i32 = 13;
pub const TEST_DISPID_SET_INDEXED_VALUE: i32 = 14;
pub const TEST_DISPID_SET_INDEXED_VALUE_REF: i32 = 15;
pub const TEST_DISPID_ECHO_VARIANT: i32 = 16;
pub const TEST_DISPID_RAISE_EXCEPTION: i32 = 17;
pub const TEST_DISPID_RETURN_SMALLINT: i32 = 18;
pub const TEST_DISPID_RETURN_UNSIGNED_WORD: i32 = 19;
pub const TEST_DISPID_RETURN_SMALLINT_ARRAY: i32 = 20;
pub const TEST_DISPID_RETURN_BOOL_ARRAY: i32 = 21;
pub const TEST_DISPID_RETURN_STRING_ARRAY: i32 = 22;
pub const TEST_DISPID_RETURN_SELF_DISPATCH: i32 = 23;
pub const TEST_DISPID_RETURN_SELF_UNKNOWN: i32 = 24;
pub const TEST_DISPID_CLASSIFY_VARIANT_ARG: i32 = 25;
pub const TEST_DISPID_CLASSIFY_VARIANT_ARRAY_FIRST_ELEMENT_ARG: i32 = 26;
pub const TEST_DISPID_RETURN_SELF_DISPATCH_ARRAY: i32 = 27;
pub const TEST_NAMED_DISPID_LHS: i32 = 101;
pub const TEST_NAMED_DISPID_RHS: i32 = 102;
pub const TEST_NAMED_DISPID_INDEX: i32 = 103;
pub const TEST_NAMED_DISPID_VALUE: i32 = 104;
pub const TEST_EVENT_CHANGED: i32 = 1;
pub const TEST_EVENT_CHANGED_PAIR: i32 = 3;
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn alloc_bstr(text: &str) -> windows_sys::core::BSTR {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    SysAllocString(wide.as_ptr())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn populate_excepinfo(excep: *mut EXCEPINFO, source: &str, description: &str, scode: i32) {
    if excep.is_null() {
        return;
    }
    (*excep).bstrSource = alloc_bstr(source);
    (*excep).bstrDescription = alloc_bstr(description);
    (*excep).scode = scode;
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_i16_array(values: &[i16], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_I2, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_I2) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let hr = SafeArrayPutElement(psa.cast_const(), &index, (value as *const i16).cast());
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_I2) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_I2;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_bool_array(values: &[bool], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_BOOL, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_BOOL) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let raw: VARIANT_BOOL = if *value { -1 } else { 0 };
        let hr = SafeArrayPutElement(
            psa.cast_const(),
            &index,
            (&raw as *const VARIANT_BOOL).cast(),
        );
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_BOOL) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_BOOL;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_bstr_array(values: &[&str], variant: *mut VARIANT) -> Result<(), String> {
    if variant.is_null() {
        return Ok(());
    }
    let len = u32::try_from(values.len())
        .map_err(|_| "SAFEARRAY payload length exceeds supported u32 range".to_string())?;
    let psa = SafeArrayCreateVector(VT_BSTR, 0, len);
    if psa.is_null() {
        return Err("SafeArrayCreateVector(VT_BSTR) returned null".to_string());
    }
    for (offset, value) in values.iter().enumerate() {
        let index = i32::try_from(offset)
            .map_err(|_| "SAFEARRAY index exceeds supported i32 range".to_string())?;
        let bstr = alloc_bstr(value);
        if bstr.is_null() {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err("SysAllocString returned null for VT_BSTR SAFEARRAY element".to_string());
        }
        let hr = SafeArrayPutElement(psa.cast_const(), &index, bstr.cast());
        SysFreeString(bstr);
        if hr < 0 {
            let _ = SafeArrayDestroy(psa.cast_const());
            return Err(format!(
                "SafeArrayPutElement(VT_BSTR) failed with HRESULT {:#010X} at index {}",
                hr as u32, index
            ));
        }
    }
    (*variant).Anonymous.Anonymous.vt = VT_ARRAY | VT_BSTR;
    (*variant).Anonymous.Anonymous.Anonymous.parray = psa;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn map_com_hresult_label(hresult: Option<u32>, arg_err: Option<u32>) -> &'static str {
    if arg_err.is_some() {
        return "arg-error";
    }
    match hresult {
        Some(0x8004_0154) => "class-not-registered",
        Some(0x8004_01F3) => "invalid-class-string",
        Some(0x8002_0003) => "member-not-found",
        Some(0x8002_0005) => "type-mismatch",
        Some(0x8002_0009) => "exception-raised",
        Some(0x8007_0057) => "invalid-argument",
        Some(_) => "native-failure",
        None => "fault-unspecified",
    }
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaTestDispatchInterface {
    vtbl: *const RawIDispatchVtbl,
    owner: *mut OxvbaTestDispatchObject,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaTestConnectionPointContainerInterface {
    vtbl: *const RawIConnectionPointContainerVtbl,
    owner: *mut OxvbaTestDispatchObject,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaTestConnectionPointInterface {
    vtbl: *const RawIConnectionPointVtbl,
    owner: *mut OxvbaTestDispatchObject,
    kind: OxvbaTestConnectionPointKind,
}

#[cfg(target_os = "windows")]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OxvbaTestConnectionPointKind {
    Dispatch = 1,
    SourceInterface = 2,
}

#[cfg(target_os = "windows")]
struct OxvbaTestDispatchObject {
    dispatch: OxvbaTestDispatchInterface,
    connection_point_container: OxvbaTestConnectionPointContainerInterface,
    dispatch_connection_point: OxvbaTestConnectionPointInterface,
    source_connection_point: OxvbaTestConnectionPointInterface,
    value_state: AtomicI32,
    ref_count: AtomicU32,
    next_cookie: AtomicU32,
    dispatch_sinks: Mutex<BTreeMap<u32, RawDispatchPtr>>,
    source_interface_sinks: Mutex<BTreeMap<u32, RawUnknownPtr>>,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawOxvbaTestDispatchSourceEventsVtbl {
    unknown: RawIUnknownVtbl,
    changed: unsafe extern "system" fn(this: *mut core::ffi::c_void, value: i32) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawOxvbaTestDispatchSourceEvents {
    vtbl: *const RawOxvbaTestDispatchSourceEventsVtbl,
}

#[cfg(target_os = "windows")]
static OXVBA_TEST_DISPATCH_VTBL: RawIDispatchVtbl = RawIDispatchVtbl {
    unknown: RawIUnknownVtbl {
        query_interface: oxvba_test_query_interface,
        add_ref: oxvba_test_add_ref,
        release: oxvba_test_release,
    },
    get_type_info_count: oxvba_test_get_type_info_count,
    get_type_info: oxvba_test_get_type_info,
    get_ids_of_names: oxvba_test_get_ids_of_names,
    invoke: oxvba_test_invoke,
};

#[cfg(target_os = "windows")]
static OXVBA_TEST_CONNECTIONPOINTCONTAINER_VTBL: RawIConnectionPointContainerVtbl =
    RawIConnectionPointContainerVtbl {
        unknown: RawIUnknownVtbl {
            query_interface: oxvba_test_connection_point_container_query_interface,
            add_ref: oxvba_test_connection_point_container_add_ref,
            release: oxvba_test_connection_point_container_release,
        },
        enum_connection_points: oxvba_test_enum_connection_points,
        find_connection_point: oxvba_test_find_connection_point,
    };

#[cfg(target_os = "windows")]
static OXVBA_TEST_CONNECTIONPOINT_VTBL: RawIConnectionPointVtbl = RawIConnectionPointVtbl {
    unknown: RawIUnknownVtbl {
        query_interface: oxvba_test_connection_point_query_interface,
        add_ref: oxvba_test_connection_point_add_ref,
        release: oxvba_test_connection_point_release,
    },
    get_connection_interface: oxvba_test_get_connection_interface,
    get_connection_point_container: oxvba_test_get_connection_point_container,
    advise: oxvba_test_advise,
    unadvise: oxvba_test_unadvise,
    enum_connections: oxvba_test_enum_connections,
};

#[cfg(target_os = "windows")]
pub fn create_oxvba_test_dispatch() -> *mut RawIDispatch {
    let mut object = Box::new(OxvbaTestDispatchObject {
        dispatch: OxvbaTestDispatchInterface {
            vtbl: &OXVBA_TEST_DISPATCH_VTBL,
            owner: std::ptr::null_mut(),
        },
        connection_point_container: OxvbaTestConnectionPointContainerInterface {
            vtbl: &OXVBA_TEST_CONNECTIONPOINTCONTAINER_VTBL,
            owner: std::ptr::null_mut(),
        },
        dispatch_connection_point: OxvbaTestConnectionPointInterface {
            vtbl: &OXVBA_TEST_CONNECTIONPOINT_VTBL,
            owner: std::ptr::null_mut(),
            kind: OxvbaTestConnectionPointKind::Dispatch,
        },
        source_connection_point: OxvbaTestConnectionPointInterface {
            vtbl: &OXVBA_TEST_CONNECTIONPOINT_VTBL,
            owner: std::ptr::null_mut(),
            kind: OxvbaTestConnectionPointKind::SourceInterface,
        },
        value_state: AtomicI32::new(0),
        ref_count: AtomicU32::new(1),
        next_cookie: AtomicU32::new(0),
        dispatch_sinks: Mutex::new(BTreeMap::new()),
        source_interface_sinks: Mutex::new(BTreeMap::new()),
    });
    let object_ptr: *mut OxvbaTestDispatchObject = &mut *object;
    object.dispatch.owner = object_ptr;
    object.connection_point_container.owner = object_ptr;
    object.dispatch_connection_point.owner = object_ptr;
    object.source_connection_point.owner = object_ptr;
    let dispatch_ptr =
        (&mut object.dispatch as *mut OxvbaTestDispatchInterface).cast::<RawIDispatch>();
    let _ = Box::into_raw(object);
    dispatch_ptr
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn read_utf16_z(ptr: *const u16) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len = len.saturating_add(1);
    }
    let slice = std::slice::from_raw_parts(ptr, len);
    String::from_utf16(slice).ok()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_test_dispatch_owner_from_dispatch(
    this: *mut core::ffi::c_void,
) -> *mut OxvbaTestDispatchObject {
    let iface = this.cast::<OxvbaTestDispatchInterface>();
    (*iface).owner
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_test_dispatch_owner_from_connection_point_container(
    this: *mut core::ffi::c_void,
) -> *mut OxvbaTestDispatchObject {
    let iface = this.cast::<OxvbaTestConnectionPointContainerInterface>();
    (*iface).owner
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_test_dispatch_owner_from_connection_point(
    this: *mut core::ffi::c_void,
) -> *mut OxvbaTestDispatchObject {
    let iface = this.cast::<OxvbaTestConnectionPointInterface>();
    (*iface).owner
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_connection_point_kind(
    this: *mut core::ffi::c_void,
) -> OxvbaTestConnectionPointKind {
    let iface = this.cast::<OxvbaTestConnectionPointInterface>();
    (*iface).kind
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_owner_add_ref(owner: *mut OxvbaTestDispatchObject) -> u32 {
    (*owner).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_owner_release(owner: *mut OxvbaTestDispatchObject) -> u32 {
    let prev = (*owner).ref_count.fetch_sub(1, Ordering::AcqRel);
    let next = prev.saturating_sub(1);
    if next == 0 {
        let mut dispatch_sinks = match (*owner).dispatch_sinks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let retained_dispatch: Vec<RawDispatchPtr> = dispatch_sinks.values().copied().collect();
        dispatch_sinks.clear();
        drop(dispatch_sinks);
        for sink in retained_dispatch {
            raw_release_dispatch(sink as *mut RawIDispatch);
        }
        let mut source_interface_sinks = match (*owner).source_interface_sinks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let retained_source: Vec<RawUnknownPtr> =
            source_interface_sinks.values().copied().collect();
        source_interface_sinks.clear();
        drop(source_interface_sinks);
        for sink in retained_source {
            raw_release_unknown(sink as *mut core::ffi::c_void);
        }
        drop(Box::from_raw(owner));
    }
    next
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_owner_query_interface(
    owner: *mut OxvbaTestDispatchObject,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    if guid_equals(riid, &IID_IUNKNOWN) || guid_equals(riid, &IID_IDISPATCH) {
        *ppv = (&mut (*owner).dispatch as *mut OxvbaTestDispatchInterface).cast();
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    if guid_equals(riid, &IID_ICONNECTIONPOINTCONTAINER) {
        *ppv = (&mut (*owner).connection_point_container
            as *mut OxvbaTestConnectionPointContainerInterface)
            .cast();
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    if guid_equals(riid, &IID_ICONNECTIONPOINT) {
        *ppv = (&mut (*owner).dispatch_connection_point as *mut OxvbaTestConnectionPointInterface)
            .cast();
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_fire_event(
    owner: *mut OxvbaTestDispatchObject,
    dispid: i32,
    args: &[i32],
) -> i32 {
    let sinks: Vec<RawDispatchPtr> = {
        let sinks = match (*owner).dispatch_sinks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        sinks.values().copied().collect()
    };
    for sink in sinks {
        let dispatch = sink as *mut RawIDispatch;
        let _ = raw_dispatch_invoke_event(dispatch, dispid, args);
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_fire_source_interface_event(
    owner: *mut OxvbaTestDispatchObject,
    value: i32,
) -> i32 {
    let sinks: Vec<RawUnknownPtr> = {
        let sinks = match (*owner).source_interface_sinks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        sinks.values().copied().collect()
    };
    for sink in sinks {
        let source = sink as *mut RawOxvbaTestDispatchSourceEvents;
        if source.is_null() {
            continue;
        }
        let _ = ((*(*source).vtbl).changed)(source.cast(), value);
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_invoke_event(
    dispatch: *mut RawIDispatch,
    dispid: i32,
    args: &[i32],
) -> Result<(), String> {
    if dispatch.is_null() {
        return Err("event sink dispatch pointer is null".to_string());
    }
    let mut variants: Vec<VARIANT> = vec![std::mem::zeroed(); args.len()];
    for (idx, value) in args.iter().enumerate() {
        let slot = args.len().saturating_sub(1).saturating_sub(idx);
        variants[slot].Anonymous.Anonymous.vt = VT_I4;
        variants[slot].Anonymous.Anonymous.Anonymous.lVal = *value;
    }
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = 0u32;
    let mut params = DISPPARAMS {
        rgvarg: if variants.is_empty() {
            std::ptr::null_mut()
        } else {
            variants.as_mut_ptr()
        },
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: u32::try_from(variants.len()).unwrap_or(u32::MAX),
        cNamedArgs: 0,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        DISPATCH_METHOD,
        &mut params,
        std::ptr::null_mut(),
        &mut excep,
        &mut arg_err,
    );
    if hr < 0 {
        return Err(format!(
            "IDispatch::Invoke(event dispid={dispid}) failed with HRESULT {:#010X} (arg_err={arg_err})",
            hr as u32
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_token_from_invoke_arg(
    variant: *const VARIANT,
    arg_index: usize,
) -> Result<i32, i32> {
    if variant.is_null() {
        return Err(COM_DISP_E_TYPEMISMATCH);
    }
    match (*variant).Anonymous.Anonymous.vt {
        VT_I2 => Ok((*variant).Anonymous.Anonymous.Anonymous.iVal as i32),
        VT_I4 => Ok((*variant).Anonymous.Anonymous.Anonymous.lVal),
        VT_UI2 => Ok((*variant).Anonymous.Anonymous.Anonymous.uiVal as i32),
        VT_UI4 => Ok((*variant).Anonymous.Anonymous.Anonymous.ulVal as i32),
        VT_BOOL => Ok(if (*variant).Anonymous.Anonymous.Anonymous.boolVal == 0 {
            0
        } else {
            1
        }),
        VT_NULL => Ok(NULL_TAG),
        VT_ERROR => Ok(error_tag_from_code(
            (*variant).Anonymous.Anonymous.Anonymous.scode,
        )),
        VT_EMPTY if arg_index == 0 => Ok(0),
        _ => Err(COM_DISP_E_TYPEMISMATCH),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_value_from_invoke_arg(
    variant: *const VARIANT,
    arg_index: usize,
) -> Result<ComValue, i32> {
    if variant.is_null() {
        return Err(COM_DISP_E_TYPEMISMATCH);
    }
    match raw_variant_to_com_value(&*variant) {
        Ok(value) => Ok(value),
        Err(_) if (*variant).Anonymous.Anonymous.vt == VT_EMPTY && arg_index == 0 => {
            Ok(ComValue::I32(0))
        }
        Err(_) => Err(COM_DISP_E_TYPEMISMATCH),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_token_from_dispparams(
    pparams: *mut DISPPARAMS,
    logical_index: usize,
    puargerr: *mut u32,
) -> Result<i32, i32> {
    if pparams.is_null() {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let params = &*pparams;
    let cargs = params.cArgs as usize;
    if logical_index >= cargs || params.rgvarg.is_null() {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let raw_index = cargs - 1 - logical_index;
    let arg = params.rgvarg.add(raw_index);
    match raw_variant_token_from_invoke_arg(arg, logical_index) {
        Ok(value) => Ok(value),
        Err(hr) => {
            if !puargerr.is_null() {
                *puargerr = raw_index as u32;
            }
            Err(hr)
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_value_from_dispparams(
    pparams: *mut DISPPARAMS,
    logical_index: usize,
    puargerr: *mut u32,
) -> Result<ComValue, i32> {
    if pparams.is_null() {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let params = &*pparams;
    let cargs = params.cArgs as usize;
    if logical_index >= cargs || params.rgvarg.is_null() {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let raw_index = cargs - 1 - logical_index;
    let arg = params.rgvarg.add(raw_index);
    match raw_variant_value_from_invoke_arg(arg, logical_index) {
        Ok(value) => Ok(value),
        Err(hr) => {
            if !puargerr.is_null() {
                *puargerr = raw_index as u32;
            }
            Err(hr)
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_token_from_named_dispid(
    pparams: *mut DISPPARAMS,
    named_dispid: i32,
    puargerr: *mut u32,
) -> Option<Result<i32, i32>> {
    if pparams.is_null() {
        return Some(Err(COM_DISP_E_BADPARAMCOUNT));
    }
    let params = &*pparams;
    if params.cNamedArgs == 0 || params.rgdispidNamedArgs.is_null() || params.rgvarg.is_null() {
        return None;
    }
    for raw_index in 0..params.cNamedArgs as usize {
        if *params.rgdispidNamedArgs.add(raw_index) != named_dispid {
            continue;
        }
        let arg = params.rgvarg.add(raw_index);
        return Some(match raw_variant_token_from_invoke_arg(arg, raw_index) {
            Ok(value) => Ok(value),
            Err(hr) => {
                if !puargerr.is_null() {
                    *puargerr = raw_index as u32;
                }
                Err(hr)
            }
        });
    }
    None
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_property_put_args_from_params(
    pparams: *mut DISPPARAMS,
    expected_count: usize,
    puargerr: *mut u32,
) -> Result<Vec<i32>, i32> {
    if pparams.is_null() {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let params = &*pparams;
    if params.cArgs != expected_count as u32
        || params.cNamedArgs == 0
        || params.rgvarg.is_null()
        || params.rgdispidNamedArgs.is_null()
    {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let mut found_property_put = false;
    for raw_index in 0..params.cNamedArgs as usize {
        if *params.rgdispidNamedArgs.add(raw_index) == COM_DISPID_PROPERTYPUT {
            found_property_put = true;
            break;
        }
    }
    if !found_property_put {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let mut values = Vec::with_capacity(expected_count);
    for logical_index in 0..expected_count {
        values.push(raw_variant_token_from_dispparams(
            pparams,
            logical_index,
            puargerr,
        )?);
    }
    Ok(values)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_property_put_i4_from_params(
    pparams: *mut DISPPARAMS,
    puargerr: *mut u32,
) -> Result<i32, i32> {
    raw_property_put_args_from_params(pparams, 1, puargerr).map(|args| args[0])
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_i32(value: i32, result: *mut VARIANT) {
    if result.is_null() {
        return;
    }
    (*result).Anonymous.Anonymous.vt = VT_I4;
    (*result).Anonymous.Anonymous.Anonymous.lVal = value;
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_bool(value: bool, result: *mut VARIANT) {
    if result.is_null() {
        return;
    }
    (*result).Anonymous.Anonymous.vt = VT_BOOL;
    (*result).Anonymous.Anonymous.Anonymous.boolVal = if value { -1 } else { 0 };
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
    oxvba_test_owner_query_interface(owner, riid, ppv)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_add_ref(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
    oxvba_test_owner_add_ref(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_release(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
    oxvba_test_owner_release(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_container_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point_container(this);
    oxvba_test_owner_query_interface(owner, riid, ppv)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_container_add_ref(
    this: *mut core::ffi::c_void,
) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point_container(this);
    oxvba_test_owner_add_ref(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_container_release(
    this: *mut core::ffi::c_void,
) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point_container(this);
    oxvba_test_owner_release(owner)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn oxvba_test_enum_connection_points(
    _this: *mut core::ffi::c_void,
    _pp_enum: *mut *mut core::ffi::c_void,
) -> i32 {
    COM_E_NOTIMPL
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_find_connection_point(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    pp_cp: *mut *mut core::ffi::c_void,
) -> i32 {
    if pp_cp.is_null() {
        return COM_E_INVALIDARG;
    }
    *pp_cp = std::ptr::null_mut();
    let owner = as_oxvba_test_dispatch_owner_from_connection_point_container(this);
    if guid_equals(riid, &IID_OXVBA_TEST_DISPATCH_EVENTS) {
        *pp_cp = (&mut (*owner).dispatch_connection_point
            as *mut OxvbaTestConnectionPointInterface)
            .cast();
    } else if guid_equals(riid, &IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS) {
        *pp_cp = (&mut (*owner).source_connection_point as *mut OxvbaTestConnectionPointInterface)
            .cast();
    } else {
        return COM_CONNECT_E_NOCONNECTION;
    }
    let _ = oxvba_test_owner_add_ref(owner);
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    if guid_equals(riid, &IID_IUNKNOWN) || guid_equals(riid, &IID_ICONNECTIONPOINT) {
        *ppv = this;
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    if guid_equals(riid, &IID_ICONNECTIONPOINTCONTAINER) {
        *ppv = (&mut (*owner).connection_point_container
            as *mut OxvbaTestConnectionPointContainerInterface)
            .cast();
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_add_ref(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    oxvba_test_owner_add_ref(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_release(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    oxvba_test_owner_release(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_get_connection_interface(
    this: *mut core::ffi::c_void,
    p_iid: *mut windows_sys::core::GUID,
) -> i32 {
    if p_iid.is_null() {
        return COM_E_INVALIDARG;
    }
    *p_iid = match oxvba_test_connection_point_kind(this) {
        OxvbaTestConnectionPointKind::Dispatch => IID_OXVBA_TEST_DISPATCH_EVENTS,
        OxvbaTestConnectionPointKind::SourceInterface => IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS,
    };
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_get_connection_point_container(
    this: *mut core::ffi::c_void,
    pp_cpc: *mut *mut core::ffi::c_void,
) -> i32 {
    if pp_cpc.is_null() {
        return COM_E_INVALIDARG;
    }
    *pp_cpc = std::ptr::null_mut();
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    *pp_cpc = (&mut (*owner).connection_point_container
        as *mut OxvbaTestConnectionPointContainerInterface)
        .cast();
    let _ = oxvba_test_owner_add_ref(owner);
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_advise(
    this: *mut core::ffi::c_void,
    p_unk_sink: *mut core::ffi::c_void,
    pdw_cookie: *mut u32,
) -> i32 {
    if p_unk_sink.is_null() || pdw_cookie.is_null() {
        return COM_E_INVALIDARG;
    }
    *pdw_cookie = 0;
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    let kind = oxvba_test_connection_point_kind(this);
    let mut sink_interface: *mut core::ffi::c_void = std::ptr::null_mut();
    let unknown = p_unk_sink.cast::<RawIUnknown>();
    let expected_iid = match kind {
        OxvbaTestConnectionPointKind::Dispatch => &IID_IDISPATCH,
        OxvbaTestConnectionPointKind::SourceInterface => &IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS,
    };
    let hr = ((*(*unknown).vtbl).query_interface)(p_unk_sink, expected_iid, &mut sink_interface);
    if hr < 0 || sink_interface.is_null() {
        return COM_CONNECT_E_CANNOTCONNECT;
    }
    let mut cookie = (*owner)
        .next_cookie
        .fetch_add(1, Ordering::AcqRel)
        .saturating_add(1);
    if cookie == 0 {
        cookie = 1;
    }
    match kind {
        OxvbaTestConnectionPointKind::Dispatch => {
            let mut sinks = match (*owner).dispatch_sinks.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            while sinks.contains_key(&cookie) {
                cookie = cookie.saturating_add(1).max(1);
            }
            sinks.insert(cookie, sink_interface as RawDispatchPtr);
        }
        OxvbaTestConnectionPointKind::SourceInterface => {
            let mut sinks = match (*owner).source_interface_sinks.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            while sinks.contains_key(&cookie) {
                cookie = cookie.saturating_add(1).max(1);
            }
            sinks.insert(cookie, sink_interface as RawUnknownPtr);
        }
    }
    *pdw_cookie = cookie;
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_unadvise(this: *mut core::ffi::c_void, dw_cookie: u32) -> i32 {
    if dw_cookie == 0 {
        return COM_CONNECT_E_NOCONNECTION;
    }
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    let kind = oxvba_test_connection_point_kind(this);
    let sink = match kind {
        OxvbaTestConnectionPointKind::Dispatch => {
            let mut sinks = match (*owner).dispatch_sinks.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            sinks.remove(&dw_cookie).map(|sink| sink as RawUnknownPtr)
        }
        OxvbaTestConnectionPointKind::SourceInterface => {
            let mut sinks = match (*owner).source_interface_sinks.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            sinks.remove(&dw_cookie)
        }
    };
    let Some(sink) = sink else {
        return COM_CONNECT_E_NOCONNECTION;
    };
    raw_release_unknown(sink as *mut core::ffi::c_void);
    COM_S_OK
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn oxvba_test_enum_connections(
    _this: *mut core::ffi::c_void,
    _pp_enum: *mut *mut core::ffi::c_void,
) -> i32 {
    COM_E_NOTIMPL
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_get_type_info_count(
    _this: *mut core::ffi::c_void,
    pctinfo: *mut u32,
) -> i32 {
    if pctinfo.is_null() {
        return COM_E_INVALIDARG;
    }
    *pctinfo = 0;
    COM_S_OK
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn oxvba_test_get_type_info(
    _this: *mut core::ffi::c_void,
    _itinfo: u32,
    _lcid: u32,
    _pptinfo: *mut *mut core::ffi::c_void,
) -> i32 {
    COM_E_NOTIMPL
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_get_ids_of_names(
    _this: *mut core::ffi::c_void,
    _riid: *const windows_sys::core::GUID,
    rgsznames: *mut *mut u16,
    cnames: u32,
    _lcid: u32,
    rgdispid: *mut i32,
) -> i32 {
    if rgsznames.is_null() || rgdispid.is_null() || cnames == 0 {
        return COM_E_INVALIDARG;
    }
    for index in 0..cnames as usize {
        let name = read_utf16_z(*rgsznames.add(index))
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let dispid = match name.as_str() {
            "count" => TEST_DISPID_COUNT,
            "exists" => TEST_DISPID_EXISTS,
            "firechanged" => TEST_DISPID_FIRE_CHANGED,
            "firechangedpair" => TEST_DISPID_FIRE_CHANGED_PAIR,
            "firechangedsourceinterface" => TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE,
            "ping" => TEST_DISPID_PING,
            "lookup" => TEST_DISPID_LOOKUP,
            "setvalue" => TEST_DISPID_SET_VALUE,
            "setvalueref" => TEST_DISPID_SET_VALUE_REF,
            "value" if index > 0 => TEST_NAMED_DISPID_VALUE,
            "value" => TEST_DISPID_VALUE,
            "sumpair" => TEST_DISPID_SUM_PAIR,
            "lookuppair" => TEST_DISPID_LOOKUP_PAIR,
            "setindexedvalue" => TEST_DISPID_SET_INDEXED_VALUE,
            "setindexedvalueref" => TEST_DISPID_SET_INDEXED_VALUE_REF,
            "echovariant" => TEST_DISPID_ECHO_VARIANT,
            "raiseexception" => TEST_DISPID_RAISE_EXCEPTION,
            "returnsmallint" => TEST_DISPID_RETURN_SMALLINT,
            "returnunsignedword" => TEST_DISPID_RETURN_UNSIGNED_WORD,
            "returnsmallintarray" => TEST_DISPID_RETURN_SMALLINT_ARRAY,
            "returnboolarray" => TEST_DISPID_RETURN_BOOL_ARRAY,
            "returnstringarray" => TEST_DISPID_RETURN_STRING_ARRAY,
            "returnselfdispatch" => TEST_DISPID_RETURN_SELF_DISPATCH,
            "returnselfunknown" => TEST_DISPID_RETURN_SELF_UNKNOWN,
            "classifyvariantarg" => TEST_DISPID_CLASSIFY_VARIANT_ARG,
            "classifyvariantarrayfirstelementarg" => {
                TEST_DISPID_CLASSIFY_VARIANT_ARRAY_FIRST_ELEMENT_ARG
            }
            "returnselfdispatcharray" => TEST_DISPID_RETURN_SELF_DISPATCH_ARRAY,
            "lhs" => TEST_NAMED_DISPID_LHS,
            "rhs" => TEST_NAMED_DISPID_RHS,
            "index" => TEST_NAMED_DISPID_INDEX,
            "valuearg" | "value_param" | "valueargtoken" => TEST_NAMED_DISPID_VALUE,
            _ => return COM_DISP_E_UNKNOWNNAME,
        };
        *rgdispid.add(index) = dispid;
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_invoke(
    this: *mut core::ffi::c_void,
    dispidmember: i32,
    _riid: *const windows_sys::core::GUID,
    _lcid: u32,
    wflags: u16,
    pparams: *mut DISPPARAMS,
    pvarresult: *mut VARIANT,
    _pexcepinfo: *mut EXCEPINFO,
    puargerr: *mut u32,
) -> i32 {
    let (cargs, rgvarg) = if pparams.is_null() {
        (0, std::ptr::null_mut())
    } else {
        ((*pparams).cArgs, (*pparams).rgvarg)
    };
    match dispidmember {
        TEST_DISPID_COUNT => {
            if (wflags & DISPATCH_PROPERTYGET) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            set_variant_i32(7, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_EXISTS => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let key = match raw_variant_token_from_dispparams(pparams, 0, puargerr) {
                Ok(value) => value,
                Err(hr) => return hr,
            };
            set_variant_bool(key == 42, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_FIRE_CHANGED => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let arg = &*rgvarg;
            let value = match raw_variant_token_from_invoke_arg(arg, 0) {
                Ok(value) => value,
                Err(hr) => {
                    if !puargerr.is_null() {
                        *puargerr = 0;
                    }
                    return hr;
                }
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let _ = oxvba_test_fire_event(owner, TEST_EVENT_CHANGED, &[value]);
            set_variant_i32(value, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_FIRE_CHANGED_PAIR => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let arg = &*rgvarg;
            let value = match raw_variant_token_from_invoke_arg(arg, 0) {
                Ok(value) => value,
                Err(hr) => {
                    if !puargerr.is_null() {
                        *puargerr = 0;
                    }
                    return hr;
                }
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let _ = oxvba_test_fire_event(
                owner,
                TEST_EVENT_CHANGED_PAIR,
                &[value, value.saturating_add(1)],
            );
            set_variant_i32(value.saturating_add(1), pvarresult);
            COM_S_OK
        }
        TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let arg = &*rgvarg;
            let value = match raw_variant_token_from_invoke_arg(arg, 0) {
                Ok(value) => value,
                Err(hr) => {
                    if !puargerr.is_null() {
                        *puargerr = 0;
                    }
                    return hr;
                }
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let _ = oxvba_test_fire_source_interface_event(owner, value);
            set_variant_i32(value, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_PING => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            set_variant_i32(123, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_LOOKUP => {
            if (wflags & DISPATCH_PROPERTYGET) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let value = match raw_variant_token_from_dispparams(pparams, 0, puargerr) {
                Ok(value) => value,
                Err(hr) => {
                    return hr;
                }
            };
            set_variant_i32(value.saturating_add(1_000), pvarresult);
            COM_S_OK
        }
        TEST_DISPID_SUM_PAIR => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 2 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let lhs =
                match raw_variant_token_from_named_dispid(pparams, TEST_NAMED_DISPID_LHS, puargerr)
                {
                    Some(result) => match result {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                    None => match raw_variant_token_from_dispparams(pparams, 0, puargerr) {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                };
            let rhs =
                match raw_variant_token_from_named_dispid(pparams, TEST_NAMED_DISPID_RHS, puargerr)
                {
                    Some(result) => match result {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                    None => match raw_variant_token_from_dispparams(pparams, 1, puargerr) {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                };
            set_variant_i32(lhs.saturating_mul(1_000).saturating_add(rhs), pvarresult);
            COM_S_OK
        }
        TEST_DISPID_LOOKUP_PAIR => {
            if (wflags & DISPATCH_PROPERTYGET) == 0 || cargs != 2 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let lhs =
                match raw_variant_token_from_named_dispid(pparams, TEST_NAMED_DISPID_LHS, puargerr)
                {
                    Some(result) => match result {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                    None => match raw_variant_token_from_dispparams(pparams, 0, puargerr) {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                };
            let rhs =
                match raw_variant_token_from_named_dispid(pparams, TEST_NAMED_DISPID_RHS, puargerr)
                {
                    Some(result) => match result {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                    None => match raw_variant_token_from_dispparams(pparams, 1, puargerr) {
                        Ok(value) => value,
                        Err(hr) => return hr,
                    },
                };
            set_variant_i32(
                lhs.saturating_mul(1_000)
                    .saturating_add(rhs)
                    .saturating_add(200_000),
                pvarresult,
            );
            COM_S_OK
        }
        TEST_DISPID_SET_VALUE => {
            if (wflags & DISPATCH_PROPERTYPUT) == 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let value = match raw_property_put_i4_from_params(pparams, puargerr) {
                Ok(value) => value,
                Err(hr) => return hr,
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            (*owner).value_state.store(value, Ordering::Release);
            set_variant_i32(value, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_SET_VALUE_REF => {
            if (wflags & DISPATCH_PROPERTYPUTREF) == 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let value = match raw_property_put_i4_from_params(pparams, puargerr) {
                Ok(value) => value,
                Err(hr) => return hr,
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let stored = value.saturating_add(100_000);
            (*owner).value_state.store(stored, Ordering::Release);
            set_variant_i32(stored, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_SET_INDEXED_VALUE => {
            if (wflags & DISPATCH_PROPERTYPUT) == 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let args = match raw_property_put_args_from_params(pparams, 2, puargerr) {
                Ok(args) => args,
                Err(hr) => return hr,
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let stored = args[0]
                .saturating_mul(1_000)
                .saturating_add(args[1])
                .saturating_add(300_000);
            (*owner).value_state.store(stored, Ordering::Release);
            set_variant_i32(stored, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_SET_INDEXED_VALUE_REF => {
            if (wflags & DISPATCH_PROPERTYPUTREF) == 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let args = match raw_property_put_args_from_params(pparams, 2, puargerr) {
                Ok(args) => args,
                Err(hr) => return hr,
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let stored = args[0]
                .saturating_mul(1_000)
                .saturating_add(args[1])
                .saturating_add(400_000);
            (*owner).value_state.store(stored, Ordering::Release);
            set_variant_i32(stored, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_ECHO_VARIANT => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let value = match raw_variant_value_from_dispparams(pparams, 0, puargerr) {
                Ok(value) => value,
                Err(hr) => return hr,
            };
            let mut resolve_object = |_handle: ObjectHandle| {
                Err("object dispatch echo unsupported in test helper".to_string())
            };
            if set_variant_dispatch_arg(pvarresult, &value, &mut resolve_object).is_err() {
                return COM_DISP_E_TYPEMISMATCH;
            }
            COM_S_OK
        }
        TEST_DISPID_RAISE_EXCEPTION => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            populate_excepinfo(
                _pexcepinfo,
                OXVBA_TEST_DISPATCH_PROGID,
                "controlled dispatch exception",
                COM_DISP_E_EXCEPTION,
            );
            COM_DISP_E_EXCEPTION
        }
        TEST_DISPID_RETURN_SMALLINT => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_I2;
                (*pvarresult).Anonymous.Anonymous.Anonymous.iVal = 321;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_UNSIGNED_WORD => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                (*pvarresult).Anonymous.Anonymous.vt = VT_UI2;
                (*pvarresult).Anonymous.Anonymous.Anonymous.uiVal = 65000;
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_SMALLINT_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_i16_array(&[12, -4, 321], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_BOOL_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_bool_array(&[true, false, true], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_STRING_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            match set_variant_bstr_array(&["Alpha", "Beta"], pvarresult) {
                Ok(()) => COM_S_OK,
                Err(_) => COM_E_INVALIDARG,
            }
        }
        TEST_DISPID_RETURN_SELF_DISPATCH => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                oxvba_test_add_ref(this);
                (*pvarresult).Anonymous.Anonymous.vt = VT_DISPATCH;
                (*pvarresult).Anonymous.Anonymous.Anonymous.pdispVal = this.cast();
            }
            COM_S_OK
        }
        TEST_DISPID_RETURN_SELF_UNKNOWN => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            if !pvarresult.is_null() {
                oxvba_test_add_ref(this);
                (*pvarresult).Anonymous.Anonymous.vt = VT_UNKNOWN;
                (*pvarresult).Anonymous.Anonymous.Anonymous.punkVal = this.cast();
            }
            COM_S_OK
        }
        TEST_DISPID_CLASSIFY_VARIANT_ARG => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let vt = (*rgvarg).Anonymous.Anonymous.vt as i32;
            set_variant_i32(vt, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_CLASSIFY_VARIANT_ARRAY_FIRST_ELEMENT_ARG => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            classify_variant_array_first_element_vt(rgvarg, pvarresult)
        }
        TEST_DISPID_RETURN_SELF_DISPATCH_ARRAY => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            set_variant_dispatch_array(this, pvarresult)
        }
        TEST_DISPID_VALUE => {
            if (wflags & DISPATCH_PROPERTYGET) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let value = (*owner).value_state.load(Ordering::Acquire);
            set_variant_i32(value, pvarresult);
            COM_S_OK
        }
        _ => COM_DISP_E_MEMBERNOTFOUND,
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_dispatch_array(
    this: *mut core::ffi::c_void,
    pvarresult: *mut VARIANT,
) -> i32 {
    let psa = SafeArrayCreateVector(VT_VARIANT, 0, 1);
    if psa.is_null() {
        return COM_E_INVALIDARG;
    }
    let mut element: VARIANT = std::mem::zeroed();
    oxvba_test_add_ref(this);
    element.Anonymous.Anonymous.vt = VT_DISPATCH;
    element.Anonymous.Anonymous.Anonymous.pdispVal = this.cast();
    let index = 0i32;
    let hr = SafeArrayPutElement(
        psa.cast_const(),
        &index,
        (&element as *const VARIANT).cast(),
    );
    let _ = VariantClear(&mut element);
    if hr < 0 {
        let _ = SafeArrayDestroy(psa.cast_const());
        return COM_E_INVALIDARG;
    }
    if !pvarresult.is_null() {
        (*pvarresult).Anonymous.Anonymous.vt = VT_ARRAY | VT_VARIANT;
        (*pvarresult).Anonymous.Anonymous.Anonymous.parray = psa;
        return COM_S_OK;
    }
    let _ = SafeArrayDestroy(psa.cast_const());
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn classify_variant_array_first_element_vt(
    rgvarg: *const VARIANT,
    pvarresult: *mut VARIANT,
) -> i32 {
    let variant = &*rgvarg;
    let vt = variant.Anonymous.Anonymous.vt;
    if (vt & VT_ARRAY) == 0 {
        return COM_DISP_E_TYPEMISMATCH;
    }
    let psa = variant.Anonymous.Anonymous.Anonymous.parray;
    if psa.is_null() {
        return COM_E_INVALIDARG;
    }
    let dims = SafeArrayGetDim(psa.cast_const());
    if dims != 1 {
        return COM_E_INVALIDARG;
    }
    let mut lower = 0i32;
    if SafeArrayGetLBound(psa.cast_const(), 1, &mut lower) < 0 {
        return COM_E_INVALIDARG;
    }
    let mut upper = -1i32;
    if SafeArrayGetUBound(psa.cast_const(), 1, &mut upper) < 0 {
        return COM_E_INVALIDARG;
    }
    if upper < lower {
        set_variant_i32(VT_EMPTY as i32, pvarresult);
        return COM_S_OK;
    }
    let mut element_vt = 0u16;
    if SafeArrayGetVartype(psa.cast_const(), &mut element_vt) < 0 {
        return COM_E_INVALIDARG;
    }
    if element_vt != VT_VARIANT {
        set_variant_i32(element_vt as i32, pvarresult);
        return COM_S_OK;
    }
    let mut element: VARIANT = std::mem::zeroed();
    let hr = SafeArrayGetElement(
        psa.cast_const(),
        &lower,
        (&mut element as *mut VARIANT).cast(),
    );
    if hr < 0 {
        return COM_E_INVALIDARG;
    }
    let inner_vt = element.Anonymous.Anonymous.vt as i32;
    let _ = VariantClear(&mut element);
    set_variant_i32(inner_vt, pvarresult);
    COM_S_OK
}
#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_to_com_value(variant: &VARIANT) -> Result<ComValue, String> {
    com_variant_to_com_value(variant)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_dispatch_arg<F>(
    variant: *mut VARIANT,
    value: &ComValue,
    resolve_object: &mut F,
) -> Result<(), String>
where
    F: FnMut(ObjectHandle) -> Result<*mut RawIDispatch, String>,
{
    if variant.is_null() {
        return Ok(());
    }
    let mut resolve_dispatch = |handle: ObjectHandle| {
        resolve_object(handle).map(|dispatch| dispatch.cast::<core::ffi::c_void>())
    };
    let mut add_ref_dispatch = |dispatch: *mut core::ffi::c_void| {
        raw_add_ref_dispatch(dispatch.cast::<RawIDispatch>());
    };
    match value {
        ComValue::ObjectHandle(_) => com_set_variant_from_com_value(
            variant,
            value,
            &mut resolve_dispatch,
            &mut add_ref_dispatch,
        )?,
        _ => {
            let mut unexpected_object_resolution = |_handle: ObjectHandle| {
                Err("object dispatch resolution not expected for non-object COM value".to_string())
            };
            let mut unexpected_add_ref = |_dispatch: *mut core::ffi::c_void| {};
            com_set_variant_from_com_value(
                variant,
                value,
                &mut unexpected_object_resolution,
                &mut unexpected_add_ref,
            )?;
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
/// # Safety
///
/// `dispatch` must be a live pointer returned by the controlled `OxVba.TestDispatch`
/// fixture or another object with the exact same vtable layout. Callers must ensure the
/// pointed-to COM object remains alive for the duration of the call.
pub unsafe fn raw_oxvba_test_dispatch_vtable_invoke(
    dispatch: *mut RawIDispatch,
    member: i32,
    args: &[i32],
) -> Result<Option<i32>, String> {
    if dispatch.is_null() {
        return Err("null dispatch pointer for vtable invoke".to_string());
    }
    if !std::ptr::eq((*dispatch).vtbl, &OXVBA_TEST_DISPATCH_VTBL) {
        return Ok(None);
    }
    match member {
        TEST_DISPID_COUNT => Ok(Some(7)),
        TEST_DISPID_EXISTS => {
            if args.len() != 1 {
                return Err(format!(
                    "IDispatch::Invoke(method) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(if args[0] == 42 { 1 } else { 0 }))
        }
        TEST_DISPID_FIRE_CHANGED => {
            if args.len() != 1 {
                return Err(format!(
                    "IDispatch::Invoke(method) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(args[0]))
        }
        TEST_DISPID_FIRE_CHANGED_PAIR => {
            if args.len() != 1 {
                return Err(format!(
                    "IDispatch::Invoke(method) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(args[0].saturating_add(1)))
        }
        TEST_DISPID_SUM_PAIR => {
            if args.len() != 2 {
                return Err(format!(
                    "IDispatch::Invoke(method) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(args[0].saturating_mul(1_000).saturating_add(args[1])))
        }
        TEST_DISPID_LOOKUP_PAIR => {
            if args.len() != 2 {
                return Err(format!(
                    "IDispatch::Invoke(property-get) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(
                args[0]
                    .saturating_mul(1_000)
                    .saturating_add(args[1])
                    .saturating_add(200_000),
            ))
        }
        TEST_DISPID_SET_INDEXED_VALUE => {
            if args.len() != 2 {
                return Err(format!(
                    "IDispatch::Invoke(property-put) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(
                args[0]
                    .saturating_mul(1_000)
                    .saturating_add(args[1])
                    .saturating_add(300_000),
            ))
        }
        TEST_DISPID_SET_INDEXED_VALUE_REF => {
            if args.len() != 2 {
                return Err(format!(
                    "IDispatch::Invoke(property-putref) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(
                args[0]
                    .saturating_mul(1_000)
                    .saturating_add(args[1])
                    .saturating_add(400_000),
            ))
        }
        TEST_DISPID_ECHO_VARIANT => {
            if args.len() != 1 {
                return Err(format!(
                    "IDispatch::Invoke(method) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(args[0]))
        }
        TEST_DISPID_RAISE_EXCEPTION => Err(format!(
            "IDispatch::Invoke(method) failed with HRESULT {:#010X} excep_description=\"controlled dispatch exception\" excep_scode={:#010X}",
            COM_DISP_E_EXCEPTION as u32, COM_DISP_E_EXCEPTION as u32
        )),
        TEST_DISPID_RETURN_SMALLINT => Ok(Some(321)),
        TEST_DISPID_RETURN_UNSIGNED_WORD => Ok(Some(65_000)),
        _ => Ok(None),
    }
}
