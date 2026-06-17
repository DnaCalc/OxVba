use std::path::Path;

use crate::ComServerDescriptor;

pub fn generate_shim_source(
    descriptor: &ComServerDescriptor,
    oxb_path: &Path,
    descriptor_path: &Path,
    tlb_path: &Path,
) -> String {
    SHIM_TEMPLATE
        .replace("__PROJECT_NAME__", &descriptor.project_name)
        .replace("__LIBID__", &descriptor.libid)
        .replace("__OXB_PATH__", &oxb_path.display().to_string())
        .replace(
            "__DESCRIPTOR_PATH__",
            &descriptor_path.display().to_string(),
        )
        .replace("__TLB_PATH__", &tlb_path.display().to_string())
}

const SHIM_TEMPLATE: &str = r##"//! Auto-generated OxVBA WrappedComServer shim source for `__PROJECT_NAME__`.
//!
//! This is a real in-process COM DLL shim over the clean OxVBA package runtime.
//! It currently implements class factory activation, IUnknown/IDispatch,
//! per-user registration, generated type-library registration, and late-bound
//! Invoke. Source dispinterfaces are exposed through connection points. A
//! bounded dual-interface tier is emitted only for classes whose generated
//! TypeLib surface fits the implemented no-argument Long-returning vtable slot.

#![cfg(target_os = "windows")]
#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering, fence};

use oxvba_build::{
    ComClassDescriptor, ComInvokeKind, ComMemberDescriptor, ComParamType, ComServerDescriptor,
};
use oxvba_bundle::ProjectMemberKind;
use oxvba_com::{
    disp_params_to_runtime_call_frame, runtime_call_error_to_excepinfo,
    runtime_call_result_to_variant,
};
use oxvba_host::{Engine, HostConfig, ProjectRuntimeSession};
use oxvba_runtime::{
    ObjectRef, RuntimeCallError, RuntimeCallResult, RuntimeCallSource, Variant,
};
use windows_sys::Win32::Foundation::{ERROR_SUCCESS, HMODULE};
use windows_sys::Win32::System::Com::{
    DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPATCH_PROPERTYPUTREF,
    DISPPARAMS, EXCEPINFO, SYS_WIN64,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows_sys::Win32::System::Ole::{
    LoadTypeLibEx, REGKIND_NONE, RegisterTypeLibForUser, UnRegisterTypeLibForUser,
};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteKeyW,
    RegDeleteTreeW, RegSetValueExW,
};
use windows_sys::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
use windows_sys::Win32::System::Variant::{VARIANT, VT_EMPTY, VariantClear};
use windows_sys::core::GUID;

const PROJECT_NAME: &str = "__PROJECT_NAME__";
const LIBID: &str = "__LIBID__";
const BUNDLE_BYTES: &[u8] = include_bytes!(r#"__OXB_PATH__"#);
const DESCRIPTOR_JSON: &str = include_str!(r#"__DESCRIPTOR_PATH__"#);
const TLB_PATH: &str = r#"__TLB_PATH__"#;

const S_OK: i32 = 0;
const S_FALSE: i32 = 1;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const E_FAIL: i32 = 0x8000_4005u32 as i32;
const E_INVALIDARG: i32 = 0x8007_0057u32 as i32;
const E_OUTOFMEMORY: i32 = 0x8007_000Eu32 as i32;
const CLASS_E_NOAGGREGATION: i32 = 0x8004_0110u32 as i32;
const CLASS_E_CLASSNOTAVAILABLE: i32 = 0x8004_0111u32 as i32;
const CONNECT_E_NOCONNECTION: i32 = 0x8004_0004u32 as i32;
const CONNECT_E_CANNOTCONNECT: i32 = 0x8004_0002u32 as i32;
const DISP_E_MEMBERNOTFOUND: i32 = 0x8002_0003u32 as i32;
const DISP_E_TYPEMISMATCH: i32 = 0x8002_0005u32 as i32;
const DISP_E_UNKNOWNNAME: i32 = 0x8002_0006u32 as i32;
const SELFREG_E_CLASS: i32 = 0x8004_0201u32 as i32;

const IID_IUNKNOWN: GUID = GUID {
    data1: 0x0000_0000,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};
const IID_ICLASSFACTORY: GUID = GUID {
    data1: 0x0000_0001,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};
const IID_IDISPATCH: GUID = GUID {
    data1: 0x0002_0400,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};
const IID_NULL: GUID = GUID {
    data1: 0,
    data2: 0,
    data3: 0,
    data4: [0; 8],
};
const IID_ICONNECTIONPOINTCONTAINER: GUID = GUID {
    data1: 0xB196_B284,
    data2: 0xBAB4,
    data3: 0x101A,
    data4: [0xB6, 0x9C, 0x00, 0xAA, 0x00, 0x34, 0x1D, 0x07],
};
const IID_ICONNECTIONPOINT: GUID = GUID {
    data1: 0xB196_B286,
    data2: 0xBAB4,
    data3: 0x101A,
    data4: [0xB6, 0x9C, 0x00, 0xAA, 0x00, 0x34, 0x1D, 0x07],
};
const IID_IENUMCONNECTIONPOINTS: GUID = GUID {
    data1: 0xB196_B285,
    data2: 0xBAB4,
    data3: 0x101A,
    data4: [0xB6, 0x9C, 0x00, 0xAA, 0x00, 0x34, 0x1D, 0x07],
};
const IID_IENUMCONNECTIONS: GUID = GUID {
    data1: 0xB196_B287,
    data2: 0xBAB4,
    data3: 0x101A,
    data4: [0xB6, 0x9C, 0x00, 0xAA, 0x00, 0x34, 0x1D, 0x07],
};

static GLOBAL_REF_COUNT: AtomicU32 = AtomicU32::new(0);
static DESCRIPTOR: OnceLock<Result<ComServerDescriptor, String>> = OnceLock::new();
static mut MODULE_HANDLE: HMODULE = ptr::null_mut();

thread_local! {
    static SESSION: RefCell<Option<ProjectRuntimeSession>> = RefCell::new(None);
    static WRAPPERS: RefCell<HashMap<i32, Vec<*mut DispatchObject>>> = RefCell::new(HashMap::new());
}

#[repr(C)]
struct IClassFactoryVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    create_instance:
        unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, *mut *mut c_void) -> i32,
    lock_server: unsafe extern "system" fn(*mut c_void, i32) -> i32,
}

#[repr(C)]
struct IDispatchVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_type_info_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(*mut c_void, u32, u32, *mut *mut c_void) -> i32,
    get_ids_of_names: unsafe extern "system" fn(
        *mut c_void,
        *const GUID,
        *const *const u16,
        u32,
        u32,
        *mut i32,
    ) -> i32,
    invoke: unsafe extern "system" fn(
        *mut c_void,
        i32,
        *const GUID,
        u32,
        u16,
        *mut DISPPARAMS,
        *mut VARIANT,
        *mut EXCEPINFO,
        *mut u32,
    ) -> i32,
}

#[repr(C)]
struct IConnectionPointContainerVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    enum_connection_points: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    find_connection_point:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
}

#[repr(C)]
struct IConnectionPointVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_connection_interface: unsafe extern "system" fn(*mut c_void, *mut GUID) -> i32,
    get_connection_point_container:
        unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    advise: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut u32) -> i32,
    unadvise: unsafe extern "system" fn(*mut c_void, u32) -> i32,
    enum_connections: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
}

#[repr(C)]
struct IEnumConnectionPointsVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    next: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void, *mut u32) -> i32,
    skip: unsafe extern "system" fn(*mut c_void, u32) -> i32,
    reset: unsafe extern "system" fn(*mut c_void) -> i32,
    clone: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
}

#[repr(C)]
struct IEnumConnectionsVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    next: unsafe extern "system" fn(*mut c_void, u32, *mut ConnectData, *mut u32) -> i32,
    skip: unsafe extern "system" fn(*mut c_void, u32) -> i32,
    reset: unsafe extern "system" fn(*mut c_void) -> i32,
    clone: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
}

#[repr(C)]
struct ConnectData {
    pUnk: *mut c_void,
    dwCookie: u32,
}

#[repr(C)]
struct IUnknownVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
}

#[repr(C)]
struct ClassFactory {
    vtbl: *const IClassFactoryVtbl,
    ref_count: AtomicU32,
    class_index: usize,
}

#[repr(C)]
struct ConnectionPointContainer {
    vtbl: *const IConnectionPointContainerVtbl,
    owner: *mut DispatchObject,
}

#[repr(C)]
struct ConnectionPoint {
    vtbl: *const IConnectionPointVtbl,
    owner: *mut DispatchObject,
}

#[repr(C)]
struct EnumConnectionPoints {
    vtbl: *const IEnumConnectionPointsVtbl,
    ref_count: AtomicU32,
    owner: *mut DispatchObject,
    index: usize,
}

#[repr(C)]
struct EnumConnections {
    vtbl: *const IEnumConnectionsVtbl,
    ref_count: AtomicU32,
    entries: Vec<ConnectionSnapshot>,
    index: usize,
}

struct ConnectionSnapshot {
    cookie: u32,
    dispatch: *mut c_void,
}

#[repr(C)]
struct DualLongReturnInterface {
    vtbl: *const DualLongReturnVtbl,
    owner: *mut DispatchObject,
}

struct ConnectionSink {
    cookie: u32,
    dispatch: *mut c_void,
}

#[repr(C)]
struct DispatchObject {
    vtbl: *const IDispatchVtbl,
    dual: DualLongReturnInterface,
    cpc: ConnectionPointContainer,
    cp: ConnectionPoint,
    ref_count: AtomicU32,
    class_index: usize,
    object: ObjectRef,
    sinks: RefCell<Vec<ConnectionSink>>,
    next_cookie: AtomicU32,
}

#[repr(C)]
struct DualLongReturnVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_type_info_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(*mut c_void, u32, u32, *mut *mut c_void) -> i32,
    get_ids_of_names: unsafe extern "system" fn(
        *mut c_void,
        *const GUID,
        *const *const u16,
        u32,
        u32,
        *mut i32,
    ) -> i32,
    invoke: unsafe extern "system" fn(
        *mut c_void,
        i32,
        *const GUID,
        u32,
        u16,
        *mut DISPPARAMS,
        *mut VARIANT,
        *mut EXCEPINFO,
        *mut u32,
    ) -> i32,
    slot0: unsafe extern "system" fn(*mut c_void, *mut i32) -> i32,
}

static CLASS_FACTORY_VTBL: IClassFactoryVtbl = IClassFactoryVtbl {
    query_interface: factory_query_interface,
    add_ref: factory_add_ref,
    release: factory_release,
    create_instance: factory_create_instance,
    lock_server: factory_lock_server,
};

static DISPATCH_VTBL: IDispatchVtbl = IDispatchVtbl {
    query_interface: dispatch_query_interface,
    add_ref: dispatch_add_ref,
    release: dispatch_release,
    get_type_info_count: dispatch_get_type_info_count,
    get_type_info: dispatch_get_type_info,
    get_ids_of_names: dispatch_get_ids_of_names,
    invoke: dispatch_invoke,
};

static DUAL_LONG_RETURN_VTBL: DualLongReturnVtbl = DualLongReturnVtbl {
    query_interface: dual_query_interface,
    add_ref: dual_add_ref,
    release: dual_release,
    get_type_info_count: dual_get_type_info_count,
    get_type_info: dual_get_type_info,
    get_ids_of_names: dual_get_ids_of_names,
    invoke: dual_invoke,
    slot0: dual_long_return_slot0,
};

static CONNECTION_POINT_CONTAINER_VTBL: IConnectionPointContainerVtbl =
    IConnectionPointContainerVtbl {
        query_interface: cpc_query_interface,
        add_ref: cpc_add_ref,
        release: cpc_release,
        enum_connection_points: cpc_enum_connection_points,
        find_connection_point: cpc_find_connection_point,
    };

static CONNECTION_POINT_VTBL: IConnectionPointVtbl = IConnectionPointVtbl {
    query_interface: cp_query_interface,
    add_ref: cp_add_ref,
    release: cp_release,
    get_connection_interface: cp_get_connection_interface,
    get_connection_point_container: cp_get_connection_point_container,
    advise: cp_advise,
    unadvise: cp_unadvise,
    enum_connections: cp_enum_connections,
};

static ENUM_CONNECTION_POINTS_VTBL: IEnumConnectionPointsVtbl = IEnumConnectionPointsVtbl {
    query_interface: enum_cp_query_interface,
    add_ref: enum_cp_add_ref,
    release: enum_cp_release,
    next: enum_cp_next,
    skip: enum_cp_skip,
    reset: enum_cp_reset,
    clone: enum_cp_clone,
};

static ENUM_CONNECTIONS_VTBL: IEnumConnectionsVtbl = IEnumConnectionsVtbl {
    query_interface: enum_connections_query_interface,
    add_ref: enum_connections_add_ref,
    release: enum_connections_release,
    next: enum_connections_next,
    skip: enum_connections_skip,
    reset: enum_connections_reset,
    clone: enum_connections_clone,
};

#[unsafe(no_mangle)]
unsafe extern "system" fn DllMain(module: HMODULE, reason: u32, _reserved: *mut c_void) -> i32 {
    if reason == DLL_PROCESS_ATTACH {
        MODULE_HANDLE = module;
    }
    1
}

#[unsafe(no_mangle)]
unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if rclsid.is_null() || riid.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();

    let Ok(descriptor) = descriptor() else {
        return E_FAIL;
    };
    let Some((class_index, _class)) = descriptor
        .classes
        .iter()
        .enumerate()
        .find(|(_, class)| class.creatable && guid_matches_text(&*rclsid, &class.clsid))
    else {
        return CLASS_E_CLASSNOTAVAILABLE;
    };

    let factory = allocate_factory(class_index);
    let hr = factory_query_interface(factory, riid, out);
    factory_release(factory);
    hr
}

#[unsafe(no_mangle)]
unsafe extern "system" fn DllCanUnloadNow() -> i32 {
    if GLOBAL_REF_COUNT.load(Ordering::Acquire) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

#[unsafe(no_mangle)]
unsafe extern "system" fn DllRegisterServer() -> i32 {
    match register_server() {
        Ok(()) => S_OK,
        Err(hr) => hr,
    }
}

#[unsafe(no_mangle)]
unsafe extern "system" fn DllUnregisterServer() -> i32 {
    match unregister_server() {
        Ok(()) => S_OK,
        Err(hr) => hr,
    }
}

unsafe extern "system" fn factory_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if this.is_null() || riid.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    if guid_eq(&*riid, &IID_IUNKNOWN) || guid_eq(&*riid, &IID_ICLASSFACTORY) {
        factory_add_ref(this);
        *out = this;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn factory_add_ref(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let factory = this.cast::<ClassFactory>();
    (*factory).ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn factory_release(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let factory = this.cast::<ClassFactory>();
    let previous = (*factory).ref_count.fetch_sub(1, Ordering::Release);
    if previous == 0 {
        return 0;
    }
    let remaining = previous - 1;
    if remaining == 0 {
        fence(Ordering::Acquire);
        GLOBAL_REF_COUNT.fetch_sub(1, Ordering::AcqRel);
        drop(Box::from_raw(factory));
    }
    remaining
}

unsafe extern "system" fn factory_create_instance(
    this: *mut c_void,
    outer: *mut c_void,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if this.is_null() || riid.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    if !outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }

    let factory = &*this.cast::<ClassFactory>();
    let Ok(descriptor) = descriptor() else {
        return E_FAIL;
    };
    let Some(class) = descriptor.classes.get(factory.class_index) else {
        return E_FAIL;
    };

    let runtime_object = match with_session(|session| {
        session
            .create_class_instance(&class.class_name)
            .map_err(|err| err.to_string())
    }) {
        Ok(object) => object,
        Err(_) => return E_FAIL,
    };

    let dispatch = allocate_dispatch_object(factory.class_index, runtime_object);
    let hr = dispatch_query_interface(dispatch, riid, out);
    dispatch_release(dispatch);
    hr
}

unsafe extern "system" fn factory_lock_server(_this: *mut c_void, lock: i32) -> i32 {
    if lock != 0 {
        GLOBAL_REF_COUNT.fetch_add(1, Ordering::AcqRel);
    } else {
        GLOBAL_REF_COUNT.fetch_sub(1, Ordering::AcqRel);
    }
    S_OK
}

unsafe extern "system" fn dispatch_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if this.is_null() || riid.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    let object = &*this.cast::<DispatchObject>();
    let Some(class) = descriptor()
        .ok()
        .and_then(|descriptor| descriptor.classes.get(object.class_index))
    else {
        return E_FAIL;
    };
    let supports_default_interface = guid_matches_text(&*riid, &class.default_interface_iid);
    let supports_connection_points = class.source_interface_iid.is_some();

    if guid_eq(&*riid, &IID_IUNKNOWN) || guid_eq(&*riid, &IID_IDISPATCH) {
        dispatch_add_ref(this);
        *out = this;
        S_OK
    } else if supports_default_interface {
        dispatch_add_ref(this);
        if class_supports_bounded_dual_interface(class) {
            *out = (&mut (*(this.cast::<DispatchObject>())).dual as *mut DualLongReturnInterface)
                .cast();
        } else {
            *out = this;
        }
        S_OK
    } else if guid_eq(&*riid, &IID_ICONNECTIONPOINTCONTAINER) && supports_connection_points {
        dispatch_add_ref(this);
        *out = (&mut (*(this.cast::<DispatchObject>())).cpc as *mut ConnectionPointContainer).cast();
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn dispatch_add_ref(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let object = this.cast::<DispatchObject>();
    (*object).ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn dispatch_release(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let object = this.cast::<DispatchObject>();
    let previous = (*object).ref_count.fetch_sub(1, Ordering::Release);
    if previous == 0 {
        return 0;
    }
    let remaining = previous - 1;
    if remaining == 0 {
        fence(Ordering::Acquire);
        unregister_dispatch_wrapper(object);
        release_connection_sinks(object);
        GLOBAL_REF_COUNT.fetch_sub(1, Ordering::AcqRel);
        drop(Box::from_raw(object));
    }
    remaining
}

unsafe extern "system" fn dispatch_get_type_info_count(
    _this: *mut c_void,
    count: *mut u32,
) -> i32 {
    if count.is_null() {
        return E_POINTER;
    }
    *count = 0;
    S_OK
}

unsafe extern "system" fn dispatch_get_type_info(
    _this: *mut c_void,
    _index: u32,
    _lcid: u32,
    info: *mut *mut c_void,
) -> i32 {
    if !info.is_null() {
        *info = ptr::null_mut();
    }
    E_NOTIMPL
}

unsafe extern "system" fn dispatch_get_ids_of_names(
    this: *mut c_void,
    _riid: *const GUID,
    names: *const *const u16,
    name_count: u32,
    _lcid: u32,
    dispids: *mut i32,
) -> i32 {
    if this.is_null() || names.is_null() || dispids.is_null() || name_count == 0 {
        return E_POINTER;
    }
    let object = &*this.cast::<DispatchObject>();
    let Ok(descriptor) = descriptor() else {
        return E_FAIL;
    };
    let Some(class) = descriptor.classes.get(object.class_index) else {
        return E_FAIL;
    };
    let Some(name) = wide_ptr_to_string(*names) else {
        return DISP_E_UNKNOWNNAME;
    };
    let Some(member) = class
        .members
        .iter()
        .find(|member| member.name.eq_ignore_ascii_case(&name))
    else {
        return DISP_E_UNKNOWNNAME;
    };

    *dispids = member.dispid;
    for index in 1..name_count as usize {
        let Some(param_name) = wide_ptr_to_string(*names.add(index)) else {
            return DISP_E_UNKNOWNNAME;
        };
        if matches!(
            member.invoke_kind,
            ComInvokeKind::PropertyPut | ComInvokeKind::PropertyPutRef
        ) && member
            .parameter_names
            .last()
            .is_some_and(|name| name.eq_ignore_ascii_case(&param_name))
        {
            *dispids.add(index) = oxvba_com::COM_DISPID_PROPERTYPUT;
        } else {
            return DISP_E_UNKNOWNNAME;
        }
    }
    S_OK
}

unsafe extern "system" fn dispatch_invoke(
    this: *mut c_void,
    dispid: i32,
    _riid: *const GUID,
    lcid: u32,
    flags: u16,
    params: *mut DISPPARAMS,
    result: *mut VARIANT,
    excep_info: *mut EXCEPINFO,
    arg_err: *mut u32,
) -> i32 {
    if this.is_null() {
        return E_POINTER;
    }
    if !result.is_null() {
        (*result).Anonymous.Anonymous.vt = VT_EMPTY;
    }

    let object = &*this.cast::<DispatchObject>();
    let Ok(descriptor) = descriptor() else {
        return E_FAIL;
    };
    let Some(class) = descriptor.classes.get(object.class_index) else {
        return E_FAIL;
    };
    let Some(member) = member_for_dispatch(class, dispid, flags) else {
        return DISP_E_MEMBERNOTFOUND;
    };

    // MS-OAUT IDispatch::Invoke defines DISPPARAMS.rgvarg in reverse argument
    // order. Route through oxvba-com's shared normalizer so in-process servers
    // use the same canonical declaration-order frame as every COM boundary.
    let frame = match disp_params_to_runtime_call_frame(dispid, flags, params, lcid) {
        Ok(frame) => frame,
        Err(err) => {
            return write_runtime_exception(
                format!("failed to marshal COM arguments: {}", err.message),
                excep_info,
                arg_err,
                arg_count(params),
            );
        }
    };
    let mut args: Vec<Variant> = frame
        .positional_args
        .into_iter()
        .map(|arg| arg.value)
        .collect();
    if let Some(property_put_arg) = frame.property_put_arg {
        args.push(property_put_arg.value);
    }

    let runtime_value = match with_session(|session| {
        session
            .invoke_member_values(
                object.object.clone(),
                &member.name,
                Some(project_member_kind(member.invoke_kind)),
                args,
            )
            .map_err(|err| err.to_string())
    }) {
        Ok(value) => value,
        Err(message) => {
            return write_runtime_exception(message, excep_info, arg_err, arg_count(params));
        }
    };

    let runtime_result = RuntimeCallResult::value(runtime_value);
    let mut resolve_object = |object: ObjectRef| resolve_runtime_object(object);
    // The resolver returns a freshly allocated COM wrapper with one owned
    // reference for the VARIANT result, so the add-ref hook is intentionally a
    // no-op for this generated server boundary.
    let mut add_ref_dispatch = |_dispatch: *mut c_void| {};
    match runtime_call_result_to_variant(
        &runtime_result,
        result,
        &mut resolve_object,
        &mut add_ref_dispatch,
    ) {
        Ok(()) => S_OK,
        Err(message) => write_runtime_exception(message, excep_info, arg_err, arg_count(params)),
    }
}

unsafe extern "system" fn dual_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    match dual_owner(this) {
        Ok(owner) => dispatch_query_interface(owner.cast(), riid, out),
        Err(hr) => hr,
    }
}

unsafe extern "system" fn dual_add_ref(this: *mut c_void) -> u32 {
    match dual_owner(this) {
        Ok(owner) => dispatch_add_ref(owner.cast()),
        Err(_) => 0,
    }
}

unsafe extern "system" fn dual_release(this: *mut c_void) -> u32 {
    match dual_owner(this) {
        Ok(owner) => dispatch_release(owner.cast()),
        Err(_) => 0,
    }
}

unsafe extern "system" fn dual_get_type_info_count(
    this: *mut c_void,
    count: *mut u32,
) -> i32 {
    match dual_owner(this) {
        Ok(owner) => dispatch_get_type_info_count(owner.cast(), count),
        Err(hr) => hr,
    }
}

unsafe extern "system" fn dual_get_type_info(
    this: *mut c_void,
    index: u32,
    lcid: u32,
    info: *mut *mut c_void,
) -> i32 {
    match dual_owner(this) {
        Ok(owner) => dispatch_get_type_info(owner.cast(), index, lcid, info),
        Err(hr) => hr,
    }
}

unsafe extern "system" fn dual_get_ids_of_names(
    this: *mut c_void,
    riid: *const GUID,
    names: *const *const u16,
    name_count: u32,
    lcid: u32,
    dispids: *mut i32,
) -> i32 {
    match dual_owner(this) {
        Ok(owner) => {
            dispatch_get_ids_of_names(owner.cast(), riid, names, name_count, lcid, dispids)
        }
        Err(hr) => hr,
    }
}

unsafe extern "system" fn dual_invoke(
    this: *mut c_void,
    dispid: i32,
    riid: *const GUID,
    lcid: u32,
    flags: u16,
    params: *mut DISPPARAMS,
    result: *mut VARIANT,
    excep_info: *mut EXCEPINFO,
    arg_err: *mut u32,
) -> i32 {
    match dual_owner(this) {
        Ok(owner) => dispatch_invoke(
            owner.cast(),
            dispid,
            riid,
            lcid,
            flags,
            params,
            result,
            excep_info,
            arg_err,
        ),
        Err(hr) => hr,
    }
}

unsafe extern "system" fn dual_long_return_slot0(this: *mut c_void, out: *mut i32) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    *out = 0;
    let owner = match dual_owner(this) {
        Ok(owner) => owner,
        Err(hr) => return hr,
    };
    let Ok(descriptor) = descriptor() else {
        return E_FAIL;
    };
    let Some(class) = descriptor.classes.get((*owner).class_index) else {
        return E_FAIL;
    };
    let Some(member) = class
        .members
        .iter()
        .find(|member| member_supports_bounded_dual_interface(member))
    else {
        return E_NOTIMPL;
    };
    let value = match with_session(|session| {
        session
            .invoke_member_values(
                (*owner).object.clone(),
                &member.name,
                Some(project_member_kind(member.invoke_kind)),
                Vec::new(),
            )
            .map_err(|err| err.to_string())
    }) {
        Ok(value) => value,
        Err(_) => return E_FAIL,
    };
    let Some(value) = value.as_i32() else {
        return DISP_E_TYPEMISMATCH;
    };
    *out = value;
    S_OK
}

unsafe fn dual_owner(this: *mut c_void) -> Result<*mut DispatchObject, i32> {
    if this.is_null() {
        return Err(E_POINTER);
    }
    let owner = (*(this.cast::<DualLongReturnInterface>())).owner;
    if owner.is_null() {
        Err(E_FAIL)
    } else {
        Ok(owner)
    }
}

unsafe extern "system" fn cpc_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if this.is_null() || riid.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    let owner = (*(this.cast::<ConnectionPointContainer>())).owner;
    if owner.is_null() {
        return E_FAIL;
    }
    if guid_eq(&*riid, &IID_IUNKNOWN) {
        dispatch_add_ref(owner.cast());
        *out = owner.cast();
        S_OK
    } else if guid_eq(&*riid, &IID_ICONNECTIONPOINTCONTAINER) {
        dispatch_add_ref(owner.cast());
        *out = this;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn cpc_add_ref(this: *mut c_void) -> u32 {
    let owner = (*(this.cast::<ConnectionPointContainer>())).owner;
    dispatch_add_ref(owner.cast())
}

unsafe extern "system" fn cpc_release(this: *mut c_void) -> u32 {
    let owner = (*(this.cast::<ConnectionPointContainer>())).owner;
    dispatch_release(owner.cast())
}

unsafe extern "system" fn cpc_enum_connection_points(
    this: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    if this.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    let owner = (*(this.cast::<ConnectionPointContainer>())).owner;
    if owner.is_null() {
        return E_FAIL;
    }
    *out = allocate_enum_connection_points(owner, 0);
    S_OK
}

unsafe extern "system" fn cpc_find_connection_point(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if this.is_null() || iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    let owner = (*(this.cast::<ConnectionPointContainer>())).owner;
    if owner.is_null() {
        return E_FAIL;
    }
    let Some(source_iid) = source_iid_for_object(&*owner) else {
        return E_NOINTERFACE;
    };
    if guid_matches_text(&*iid, source_iid) {
        dispatch_add_ref(owner.cast());
        *out = (&mut (*owner).cp as *mut ConnectionPoint).cast();
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn cp_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if this.is_null() || riid.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    let owner = (*(this.cast::<ConnectionPoint>())).owner;
    if owner.is_null() {
        return E_FAIL;
    }
    if guid_eq(&*riid, &IID_IUNKNOWN) {
        dispatch_add_ref(owner.cast());
        *out = owner.cast();
        S_OK
    } else if guid_eq(&*riid, &IID_ICONNECTIONPOINT) {
        dispatch_add_ref(owner.cast());
        *out = this;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn cp_add_ref(this: *mut c_void) -> u32 {
    let owner = (*(this.cast::<ConnectionPoint>())).owner;
    dispatch_add_ref(owner.cast())
}

unsafe extern "system" fn cp_release(this: *mut c_void) -> u32 {
    let owner = (*(this.cast::<ConnectionPoint>())).owner;
    dispatch_release(owner.cast())
}

unsafe extern "system" fn cp_get_connection_interface(this: *mut c_void, iid: *mut GUID) -> i32 {
    if this.is_null() || iid.is_null() {
        return E_POINTER;
    }
    let owner = (*(this.cast::<ConnectionPoint>())).owner;
    if owner.is_null() {
        return E_FAIL;
    }
    let Some(source_iid) = source_iid_for_object(&*owner).and_then(parse_guid_text) else {
        return E_FAIL;
    };
    *iid = source_iid;
    S_OK
}

unsafe extern "system" fn cp_get_connection_point_container(
    this: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    if this.is_null() || out.is_null() {
        return E_POINTER;
    }
    let owner = (*(this.cast::<ConnectionPoint>())).owner;
    if owner.is_null() {
        return E_FAIL;
    }
    dispatch_add_ref(owner.cast());
    *out = (&mut (*owner).cpc as *mut ConnectionPointContainer).cast();
    S_OK
}

unsafe extern "system" fn cp_advise(
    this: *mut c_void,
    sink_unknown: *mut c_void,
    cookie_out: *mut u32,
) -> i32 {
    if this.is_null() || sink_unknown.is_null() || cookie_out.is_null() {
        return E_POINTER;
    }
    *cookie_out = 0;
    let owner = (*(this.cast::<ConnectionPoint>())).owner;
    if owner.is_null() {
        return E_FAIL;
    }
    let mut dispatch: *mut c_void = ptr::null_mut();
    let hr = query_interface(sink_unknown, &IID_IDISPATCH, &mut dispatch);
    if hr < 0 || dispatch.is_null() {
        return CONNECT_E_CANNOTCONNECT;
    }
    let cookie = (*owner).next_cookie.fetch_add(1, Ordering::Relaxed);
    (*owner).sinks.borrow_mut().push(ConnectionSink { cookie, dispatch });
    *cookie_out = cookie;
    S_OK
}

unsafe extern "system" fn cp_unadvise(this: *mut c_void, cookie: u32) -> i32 {
    if this.is_null() {
        return E_POINTER;
    }
    let owner = (*(this.cast::<ConnectionPoint>())).owner;
    if owner.is_null() {
        return E_FAIL;
    }
    let mut sinks = (*owner).sinks.borrow_mut();
    let Some(index) = sinks.iter().position(|sink| sink.cookie == cookie) else {
        return CONNECT_E_NOCONNECTION;
    };
    let sink = sinks.remove(index);
    release_unknown(sink.dispatch);
    S_OK
}

unsafe extern "system" fn cp_enum_connections(this: *mut c_void, out: *mut *mut c_void) -> i32 {
    if this.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    let owner = (*(this.cast::<ConnectionPoint>())).owner;
    if owner.is_null() {
        return E_FAIL;
    }
    *out = allocate_enum_connections_from_owner(owner, 0);
    S_OK
}

unsafe extern "system" fn enum_cp_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if this.is_null() || riid.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    if guid_eq(&*riid, &IID_IUNKNOWN) || guid_eq(&*riid, &IID_IENUMCONNECTIONPOINTS) {
        enum_cp_add_ref(this);
        *out = this;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn enum_cp_add_ref(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let enumerator = this.cast::<EnumConnectionPoints>();
    (*enumerator).ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn enum_cp_release(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let enumerator = this.cast::<EnumConnectionPoints>();
    let previous = (*enumerator).ref_count.fetch_sub(1, Ordering::Release);
    if previous == 0 {
        return 0;
    }
    let remaining = previous - 1;
    if remaining == 0 {
        fence(Ordering::Acquire);
        let owner = (*enumerator).owner;
        GLOBAL_REF_COUNT.fetch_sub(1, Ordering::AcqRel);
        if !owner.is_null() {
            dispatch_release(owner.cast());
        }
        drop(Box::from_raw(enumerator));
    }
    remaining
}

unsafe extern "system" fn enum_cp_next(
    this: *mut c_void,
    count: u32,
    out: *mut *mut c_void,
    fetched: *mut u32,
) -> i32 {
    if this.is_null() || out.is_null() || (fetched.is_null() && count != 1) {
        return E_POINTER;
    }
    for index in 0..count as usize {
        *out.add(index) = ptr::null_mut();
    }
    if !fetched.is_null() {
        *fetched = 0;
    }

    let enumerator = &mut *this.cast::<EnumConnectionPoints>();
    let mut copied = 0u32;
    while copied < count && enumerator.index == 0 {
        enumerator.index = 1;
        if enumerator.owner.is_null() {
            return E_FAIL;
        }
        if source_iid_for_object(&*enumerator.owner).is_some() {
            dispatch_add_ref(enumerator.owner.cast());
            *out.add(copied as usize) =
                (&mut (*enumerator.owner).cp as *mut ConnectionPoint).cast();
            copied += 1;
        }
    }
    if !fetched.is_null() {
        *fetched = copied;
    }
    if copied == count {
        S_OK
    } else {
        S_FALSE
    }
}

unsafe extern "system" fn enum_cp_skip(this: *mut c_void, count: u32) -> i32 {
    if this.is_null() {
        return E_POINTER;
    }
    let enumerator = &mut *this.cast::<EnumConnectionPoints>();
    let has_remaining_source = enumerator.index == 0
        && !enumerator.owner.is_null()
        && source_iid_for_object(&*enumerator.owner).is_some();
    let remaining = usize::from(has_remaining_source);
    let skipped = remaining.min(count as usize);
    enumerator.index += skipped;
    if skipped == count as usize {
        S_OK
    } else {
        S_FALSE
    }
}

unsafe extern "system" fn enum_cp_reset(this: *mut c_void) -> i32 {
    if this.is_null() {
        return E_POINTER;
    }
    (*this.cast::<EnumConnectionPoints>()).index = 0;
    S_OK
}

unsafe extern "system" fn enum_cp_clone(this: *mut c_void, out: *mut *mut c_void) -> i32 {
    if this.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    let enumerator = &*this.cast::<EnumConnectionPoints>();
    if enumerator.owner.is_null() {
        return E_FAIL;
    }
    *out = allocate_enum_connection_points(enumerator.owner, enumerator.index);
    S_OK
}

unsafe extern "system" fn enum_connections_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if this.is_null() || riid.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    if guid_eq(&*riid, &IID_IUNKNOWN) || guid_eq(&*riid, &IID_IENUMCONNECTIONS) {
        enum_connections_add_ref(this);
        *out = this;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn enum_connections_add_ref(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let enumerator = this.cast::<EnumConnections>();
    (*enumerator).ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn enum_connections_release(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let enumerator = this.cast::<EnumConnections>();
    let previous = (*enumerator).ref_count.fetch_sub(1, Ordering::Release);
    if previous == 0 {
        return 0;
    }
    let remaining = previous - 1;
    if remaining == 0 {
        fence(Ordering::Acquire);
        for entry in (*enumerator).entries.drain(..) {
            release_unknown(entry.dispatch);
        }
        GLOBAL_REF_COUNT.fetch_sub(1, Ordering::AcqRel);
        drop(Box::from_raw(enumerator));
    }
    remaining
}

unsafe extern "system" fn enum_connections_next(
    this: *mut c_void,
    count: u32,
    out: *mut ConnectData,
    fetched: *mut u32,
) -> i32 {
    if this.is_null() || out.is_null() || (fetched.is_null() && count != 1) {
        return E_POINTER;
    }
    for index in 0..count as usize {
        *out.add(index) = ConnectData {
            pUnk: ptr::null_mut(),
            dwCookie: 0,
        };
    }
    if !fetched.is_null() {
        *fetched = 0;
    }

    let enumerator = &mut *this.cast::<EnumConnections>();
    let mut copied = 0u32;
    while copied < count && enumerator.index < enumerator.entries.len() {
        let entry = &enumerator.entries[enumerator.index];
        add_ref_unknown(entry.dispatch);
        *out.add(copied as usize) = ConnectData {
            pUnk: entry.dispatch,
            dwCookie: entry.cookie,
        };
        enumerator.index += 1;
        copied += 1;
    }
    if !fetched.is_null() {
        *fetched = copied;
    }
    if copied == count {
        S_OK
    } else {
        S_FALSE
    }
}

unsafe extern "system" fn enum_connections_skip(this: *mut c_void, count: u32) -> i32 {
    if this.is_null() {
        return E_POINTER;
    }
    let enumerator = &mut *this.cast::<EnumConnections>();
    let remaining = enumerator.entries.len().saturating_sub(enumerator.index);
    let skipped = remaining.min(count as usize);
    enumerator.index += skipped;
    if skipped == count as usize {
        S_OK
    } else {
        S_FALSE
    }
}

unsafe extern "system" fn enum_connections_reset(this: *mut c_void) -> i32 {
    if this.is_null() {
        return E_POINTER;
    }
    (*this.cast::<EnumConnections>()).index = 0;
    S_OK
}

unsafe extern "system" fn enum_connections_clone(this: *mut c_void, out: *mut *mut c_void) -> i32 {
    if this.is_null() || out.is_null() {
        return E_POINTER;
    }
    *out = ptr::null_mut();
    let enumerator = &*this.cast::<EnumConnections>();
    *out = allocate_enum_connections_from_entries(&enumerator.entries, enumerator.index);
    S_OK
}

fn descriptor() -> Result<&'static ComServerDescriptor, i32> {
    DESCRIPTOR
        .get_or_init(|| serde_json::from_str(DESCRIPTOR_JSON).map_err(|err| err.to_string()))
        .as_ref()
        .map_err(|_| E_FAIL)
}

fn with_session<R>(
    f: impl FnOnce(&mut ProjectRuntimeSession) -> Result<R, String>,
) -> Result<R, String> {
    SESSION.with(|slot| {
        let needs_init = slot.borrow().is_none();
        if needs_init {
            let package = oxvba_bundle::BundlePackage::from_bytes(BUNDLE_BYTES)
                .map_err(|err| err.to_string())?;
            let engine = Engine::new(HostConfig { enable_jit: false });
            let mut session = engine
                .prepare_bundle_package_session(package)
                .map_err(|err| err.to_string())?;
            session.set_project_event_sink(|source, event_id, args| unsafe {
                fire_project_event(source, event_id, args)
            });
            *slot.borrow_mut() = Some(session);
        }
        let mut borrowed = slot.borrow_mut();
        let Some(session) = borrowed.as_mut() else {
            return Err("failed to initialize OxVBA runtime session".to_string());
        };
        f(session)
    })
}

fn member_for_dispatch(
    class: &ComClassDescriptor,
    dispid: i32,
    flags: u16,
) -> Option<&ComMemberDescriptor> {
    class
        .members
        .iter()
        .find(|member| member.dispid == dispid && invoke_kind_matches_flags(member.invoke_kind, flags))
        .or_else(|| class.members.iter().find(|member| member.dispid == dispid))
}

fn class_supports_bounded_dual_interface(class: &ComClassDescriptor) -> bool {
    class.members.len() == 1
        && class
            .members
            .first()
            .is_some_and(member_supports_bounded_dual_interface)
}

fn member_supports_bounded_dual_interface(member: &ComMemberDescriptor) -> bool {
    member.invoke_kind == ComInvokeKind::Method
        && member.vtable_slot == Some(7)
        && member.parameter_types.is_empty()
        && member.return_type == Some(ComParamType::Long)
}

fn invoke_kind_matches_flags(kind: ComInvokeKind, flags: u16) -> bool {
    if flags & DISPATCH_PROPERTYPUTREF as u16 != 0 {
        return matches!(kind, ComInvokeKind::PropertyPutRef);
    }
    if flags & DISPATCH_PROPERTYPUT as u16 != 0 {
        return matches!(kind, ComInvokeKind::PropertyPut);
    }
    if flags & DISPATCH_PROPERTYGET as u16 != 0 {
        return matches!(kind, ComInvokeKind::PropertyGet);
    }
    if flags & DISPATCH_METHOD as u16 != 0 {
        return matches!(kind, ComInvokeKind::Method);
    }
    false
}

fn project_member_kind(kind: ComInvokeKind) -> ProjectMemberKind {
    match kind {
        ComInvokeKind::PropertyGet => ProjectMemberKind::PropertyGet,
        ComInvokeKind::Method => ProjectMemberKind::Method,
        ComInvokeKind::PropertyPut => ProjectMemberKind::PropertyLet,
        ComInvokeKind::PropertyPutRef => ProjectMemberKind::PropertySet,
    }
}

unsafe fn resolve_runtime_object(object: ObjectRef) -> Result<*mut c_void, String> {
    if !object.is_project_instance() {
        return Err("runtime object result is not an OxVBA project instance".to_string());
    }
    let class_index = usize::try_from(object.route_key())
        .map_err(|_| "runtime object carried a negative class index".to_string())?;
    let descriptor = descriptor().map_err(|_| "COM descriptor was unavailable".to_string())?;
    if descriptor.classes.get(class_index).is_none() {
        return Err(format!(
            "runtime object class index {class_index} is outside the COM descriptor"
        ));
    }
    Ok(allocate_dispatch_object(class_index, object))
}

unsafe fn allocate_factory(class_index: usize) -> *mut c_void {
    GLOBAL_REF_COUNT.fetch_add(1, Ordering::AcqRel);
    Box::into_raw(Box::new(ClassFactory {
        vtbl: &CLASS_FACTORY_VTBL,
        ref_count: AtomicU32::new(1),
        class_index,
    }))
    .cast()
}

unsafe fn allocate_dispatch_object(class_index: usize, object: ObjectRef) -> *mut c_void {
    GLOBAL_REF_COUNT.fetch_add(1, Ordering::AcqRel);
    let raw = Box::into_raw(Box::new(DispatchObject {
        vtbl: &DISPATCH_VTBL,
        dual: DualLongReturnInterface {
            vtbl: &DUAL_LONG_RETURN_VTBL,
            owner: ptr::null_mut(),
        },
        cpc: ConnectionPointContainer {
            vtbl: &CONNECTION_POINT_CONTAINER_VTBL,
            owner: ptr::null_mut(),
        },
        cp: ConnectionPoint {
            vtbl: &CONNECTION_POINT_VTBL,
            owner: ptr::null_mut(),
        },
        ref_count: AtomicU32::new(1),
        class_index,
        object,
        sinks: RefCell::new(Vec::new()),
        next_cookie: AtomicU32::new(1),
    }));
    (*raw).dual.owner = raw;
    (*raw).cpc.owner = raw;
    (*raw).cp.owner = raw;
    register_dispatch_wrapper(raw);
    raw.cast()
}

unsafe fn allocate_enum_connection_points(owner: *mut DispatchObject, index: usize) -> *mut c_void {
    dispatch_add_ref(owner.cast());
    GLOBAL_REF_COUNT.fetch_add(1, Ordering::AcqRel);
    Box::into_raw(Box::new(EnumConnectionPoints {
        vtbl: &ENUM_CONNECTION_POINTS_VTBL,
        ref_count: AtomicU32::new(1),
        owner,
        index,
    }))
    .cast()
}

unsafe fn allocate_enum_connections_from_owner(
    owner: *mut DispatchObject,
    index: usize,
) -> *mut c_void {
    let entries: Vec<ConnectionSnapshot> = {
        let sinks = (*owner).sinks.borrow();
        sinks
            .iter()
            .map(|sink| {
                add_ref_unknown(sink.dispatch);
                ConnectionSnapshot {
                    cookie: sink.cookie,
                    dispatch: sink.dispatch,
                }
            })
            .collect()
    };
    allocate_enum_connections(entries, index)
}

unsafe fn allocate_enum_connections_from_entries(
    entries: &[ConnectionSnapshot],
    index: usize,
) -> *mut c_void {
    let cloned = entries
        .iter()
        .map(|entry| {
            add_ref_unknown(entry.dispatch);
            ConnectionSnapshot {
                cookie: entry.cookie,
                dispatch: entry.dispatch,
            }
        })
        .collect();
    allocate_enum_connections(cloned, index)
}

unsafe fn allocate_enum_connections(entries: Vec<ConnectionSnapshot>, index: usize) -> *mut c_void {
    GLOBAL_REF_COUNT.fetch_add(1, Ordering::AcqRel);
    Box::into_raw(Box::new(EnumConnections {
        vtbl: &ENUM_CONNECTIONS_VTBL,
        ref_count: AtomicU32::new(1),
        entries,
        index,
    }))
    .cast()
}

unsafe fn register_dispatch_wrapper(object: *mut DispatchObject) {
    if object.is_null() {
        return;
    }
    let raw = (*object).object.raw();
    WRAPPERS.with(|wrappers| {
        wrappers.borrow_mut().entry(raw).or_default().push(object);
    });
}

unsafe fn unregister_dispatch_wrapper(object: *mut DispatchObject) {
    if object.is_null() {
        return;
    }
    let raw = (*object).object.raw();
    WRAPPERS.with(|wrappers| {
        let mut wrappers = wrappers.borrow_mut();
        if let Some(list) = wrappers.get_mut(&raw) {
            list.retain(|item| *item != object);
            if list.is_empty() {
                wrappers.remove(&raw);
            }
        }
    });
}

unsafe fn release_connection_sinks(object: *mut DispatchObject) {
    if object.is_null() {
        return;
    }
    for sink in (*object).sinks.get_mut().drain(..) {
        release_unknown(sink.dispatch);
    }
}

fn source_iid_for_object(object: &DispatchObject) -> Option<&str> {
    descriptor()
        .ok()
        .and_then(|descriptor| descriptor.classes.get(object.class_index))
        .and_then(|class| class.source_interface_iid.as_deref())
}

unsafe fn fire_project_event(
    source: ObjectRef,
    event_id: i32,
    args: Vec<Variant>,
) -> Result<(), String> {
    let wrappers = WRAPPERS.with(|wrappers| {
        wrappers
            .borrow()
            .get(&source.raw())
            .cloned()
            .unwrap_or_default()
    });
    for wrapper in wrappers {
        if !wrapper.is_null() {
            fire_event_on_wrapper(wrapper, event_id, &args)?;
        }
    }
    Ok(())
}

unsafe fn fire_event_on_wrapper(
    object: *mut DispatchObject,
    event_id: i32,
    args: &[Variant],
) -> Result<(), String> {
    let descriptor = descriptor().map_err(|_| "COM descriptor unavailable".to_string())?;
    let Some(class) = descriptor.classes.get((*object).class_index) else {
        return Ok(());
    };
    if !class.events.iter().any(|event| event.dispid == event_id) {
        return Ok(());
    }
    if (*object).sinks.borrow().is_empty() {
        return Ok(());
    }

    let mut variants: Vec<VARIANT> = (0..args.len()).map(|_| std::mem::zeroed()).collect();
    for (logical_index, value) in args.iter().enumerate() {
        let com_index = args.len() - 1 - logical_index;
        let runtime_result = RuntimeCallResult::value(value.clone());
        let mut resolve_object = |object: ObjectRef| resolve_runtime_object(object);
        let mut add_ref_dispatch = |_dispatch: *mut c_void| {};
        if let Err(err) = runtime_call_result_to_variant(
            &runtime_result,
            &mut variants[com_index],
            &mut resolve_object,
            &mut add_ref_dispatch,
        ) {
            for variant in &mut variants {
                VariantClear(variant);
            }
            return Err(err);
        }
    }
    let sinks: Vec<*mut c_void> = {
        let borrowed = (*object).sinks.borrow();
        borrowed
            .iter()
            .map(|sink| {
                add_ref_unknown(sink.dispatch);
                sink.dispatch
            })
            .collect()
    };
    if sinks.is_empty() {
        for variant in &mut variants {
            VariantClear(variant);
        }
        return Ok(());
    }

    let mut params = DISPPARAMS {
        rgvarg: if variants.is_empty() {
            ptr::null_mut()
        } else {
            variants.as_mut_ptr()
        },
        rgdispidNamedArgs: ptr::null_mut(),
        cArgs: variants.len() as u32,
        cNamedArgs: 0,
    };

    let mut first_error: Option<String> = None;
    for sink in sinks {
        let hr = invoke_dispatch_sink(sink, event_id, &mut params);
        release_unknown(sink);
        if hr < 0 && first_error.is_none() {
            first_error = Some(format!(
                "event sink Invoke(dispid={event_id}) failed with HRESULT 0x{:08X}",
                hr as u32
            ));
        }
    }
    for variant in &mut variants {
        VariantClear(variant);
    }
    if let Some(message) = first_error {
        Err(message)
    } else {
        Ok(())
    }
}

unsafe fn invoke_dispatch_sink(
    dispatch: *mut c_void,
    event_id: i32,
    params: *mut DISPPARAMS,
) -> i32 {
    if dispatch.is_null() {
        return E_POINTER;
    }
    let vtbl = *(dispatch.cast::<*const IDispatchVtbl>());
    if vtbl.is_null() {
        return E_POINTER;
    }
    let mut excep_info: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = 0u32;
    ((*vtbl).invoke)(
        dispatch,
        event_id,
        &IID_NULL,
        0,
        DISPATCH_METHOD as u16,
        params,
        ptr::null_mut(),
        &mut excep_info,
        &mut arg_err,
    )
}

unsafe fn write_runtime_exception(
    message: String,
    excep_info: *mut EXCEPINFO,
    arg_err: *mut u32,
    arg_count: usize,
) -> i32 {
    let error = RuntimeCallError::new(5, message, RuntimeCallSource::ExternalComDispatch);
    runtime_call_error_to_excepinfo(&error, excep_info, arg_err, arg_count)
}

unsafe fn arg_count(params: *mut DISPPARAMS) -> usize {
    if params.is_null() {
        0
    } else {
        (*params).cArgs as usize
    }
}

unsafe fn register_server() -> Result<(), i32> {
    let module_path = module_path()?;
    let descriptor = descriptor()?;
    register_typelib()?;
    for class in descriptor.classes.iter().filter(|class| class.creatable) {
        let clsid_key = format!("Software\\Classes\\CLSID\\{{{}}}", class.clsid);
        set_key_default(&clsid_key, class.description.as_deref().unwrap_or(&class.class_name))?;
        set_key_default(
            &format!("{clsid_key}\\InprocServer32"),
            &module_path,
        )?;
        set_key_value(
            &format!("{clsid_key}\\InprocServer32"),
            "ThreadingModel",
            "Apartment",
        )?;
        set_key_default(&format!("{clsid_key}\\ProgID"), &class.prog_id)?;
        set_key_default(
            &format!("{clsid_key}\\TypeLib"),
            &format!("{{{}}}", descriptor.libid),
        )?;
        set_key_default(
            &format!("{clsid_key}\\Version"),
            &format!("{}.{}", descriptor.version_major, descriptor.version_minor),
        )?;
        set_key_default(&format!("Software\\Classes\\{}", class.prog_id), &class.class_name)?;
        set_key_default(
            &format!("Software\\Classes\\{}\\CLSID", class.prog_id),
            &format!("{{{}}}", class.clsid),
        )?;
    }
    Ok(())
}

unsafe fn unregister_server() -> Result<(), i32> {
    let descriptor = descriptor()?;
    for class in descriptor.classes.iter().filter(|class| class.creatable) {
        let clsid_key = format!("Software\\Classes\\CLSID\\{{{}}}", class.clsid);
        delete_tree(&clsid_key);
        delete_tree(&format!("Software\\Classes\\{}", class.prog_id));
    }
    unregister_typelib(descriptor);
    Ok(())
}

unsafe fn register_typelib() -> Result<(), i32> {
    let path_w = wide_null(TLB_PATH);
    let mut typelib: *mut c_void = ptr::null_mut();
    let hr = LoadTypeLibEx(path_w.as_ptr(), REGKIND_NONE, &mut typelib);
    if hr < 0 || typelib.is_null() {
        return Err(SELFREG_E_CLASS);
    }
    let hr = RegisterTypeLibForUser(typelib, path_w.as_ptr(), ptr::null());
    release_unknown(typelib);
    if hr < 0 {
        Err(SELFREG_E_CLASS)
    } else {
        Ok(())
    }
}

unsafe fn unregister_typelib(descriptor: &ComServerDescriptor) {
    let Some(libid) = parse_guid_text(&descriptor.libid) else {
        return;
    };
    UnRegisterTypeLibForUser(
        &libid,
        descriptor.version_major,
        descriptor.version_minor,
        0,
        SYS_WIN64,
    );
}

unsafe fn set_key_default(path: &str, value: &str) -> Result<(), i32> {
    set_key_value_raw(path, None, value)
}

unsafe fn set_key_value(path: &str, name: &str, value: &str) -> Result<(), i32> {
    set_key_value_raw(path, Some(name), value)
}

unsafe fn set_key_value_raw(path: &str, name: Option<&str>, value: &str) -> Result<(), i32> {
    let path_w = wide_null(path);
    let mut key: HKEY = ptr::null_mut();
    let status = RegCreateKeyExW(
        HKEY_CURRENT_USER,
        path_w.as_ptr(),
        0,
        ptr::null(),
        0,
        KEY_WRITE,
        ptr::null(),
        &mut key,
        ptr::null_mut(),
    );
    if status != ERROR_SUCCESS {
        return Err(SELFREG_E_CLASS);
    }

    let name_w = name.map(wide_null);
    let value_w = wide_null(value);
    let status = RegSetValueExW(
        key,
        name_w.as_ref().map_or(ptr::null(), |text| text.as_ptr()),
        0,
        REG_SZ,
        value_w.as_ptr().cast::<u8>(),
        (value_w.len() * std::mem::size_of::<u16>()) as u32,
    );
    RegCloseKey(key);
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(SELFREG_E_CLASS)
    }
}

unsafe fn delete_tree(path: &str) {
    let path_w = wide_null(path);
    RegDeleteTreeW(HKEY_CURRENT_USER, path_w.as_ptr());
    RegDeleteKeyW(HKEY_CURRENT_USER, path_w.as_ptr());
}

unsafe fn module_path() -> Result<String, i32> {
    let mut buffer = vec![0u16; 32768];
    let len = GetModuleFileNameW(MODULE_HANDLE, buffer.as_mut_ptr(), buffer.len() as u32);
    if len == 0 {
        return Err(SELFREG_E_CLASS);
    }
    Ok(String::from_utf16_lossy(&buffer[..len as usize]))
}

fn guid_eq(left: &GUID, right: &GUID) -> bool {
    left.data1 == right.data1
        && left.data2 == right.data2
        && left.data3 == right.data3
        && left.data4 == right.data4
}

fn guid_matches_text(guid: &GUID, text: &str) -> bool {
    guid_to_string(guid).eq_ignore_ascii_case(text.trim_matches(|ch| ch == '{' || ch == '}'))
}

fn parse_guid_text(text: &str) -> Option<GUID> {
    let text = text.trim_matches(|ch| ch == '{' || ch == '}');
    let parts: Vec<&str> = text.split('-').collect();
    if parts.len() != 5 || parts[3].len() != 4 || parts[4].len() != 12 {
        return None;
    }
    let data1 = u32::from_str_radix(parts[0], 16).ok()?;
    let data2 = u16::from_str_radix(parts[1], 16).ok()?;
    let data3 = u16::from_str_radix(parts[2], 16).ok()?;
    let tail = format!("{}{}", parts[3], parts[4]);
    if tail.len() != 16 {
        return None;
    }
    let mut data4 = [0u8; 8];
    for index in 0..8 {
        data4[index] = u8::from_str_radix(&tail[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(GUID {
        data1,
        data2,
        data3,
        data4,
    })
}

fn guid_to_string(guid: &GUID) -> String {
    format!(
        "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7],
    )
}

unsafe fn release_unknown(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let vtbl = *(ptr.cast::<*const IUnknownVtbl>());
    if !vtbl.is_null() {
        ((*vtbl).release)(ptr);
    }
}

unsafe fn add_ref_unknown(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let vtbl = *(ptr.cast::<*const IUnknownVtbl>());
    if !vtbl.is_null() {
        ((*vtbl).add_ref)(ptr);
    }
}

unsafe fn query_interface(ptr: *mut c_void, iid: &GUID, out: *mut *mut c_void) -> i32 {
    if ptr.is_null() || out.is_null() {
        return E_POINTER;
    }
    let vtbl = *(ptr.cast::<*const IUnknownVtbl>());
    if vtbl.is_null() {
        return E_POINTER;
    }
    ((*vtbl).query_interface)(ptr, iid, out)
}

unsafe fn wide_ptr_to_string(ptr: *const u16) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
        if len > 32_768 {
            return None;
        }
    }
    Some(String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len)))
}

fn wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
"##;
