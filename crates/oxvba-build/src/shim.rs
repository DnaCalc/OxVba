use std::path::Path;

use crate::ComServerDescriptor;

pub fn generate_shim_source(
    descriptor: &ComServerDescriptor,
    oxb_path: &Path,
    descriptor_path: &Path,
) -> String {
    SHIM_TEMPLATE
        .replace("__PROJECT_NAME__", &descriptor.project_name)
        .replace("__LIBID__", &descriptor.libid)
        .replace("__OXB_PATH__", &oxb_path.display().to_string())
        .replace(
            "__DESCRIPTOR_PATH__",
            &descriptor_path.display().to_string(),
        )
}

const SHIM_TEMPLATE: &str = r##"//! Auto-generated OxVBA WrappedComServer shim source for `__PROJECT_NAME__`.
//!
//! This is a real in-process COM DLL shim over the clean OxVBA package runtime.
//! It currently implements class factory activation, IUnknown/IDispatch,
//! per-user registration, and late-bound Invoke. Type library emission and
//! connection-point event sources are generated as artifacts but are not yet
//! compiled into this shim.

#![cfg(target_os = "windows")]
#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering, fence};

use oxvba_build::{ComClassDescriptor, ComInvokeKind, ComMemberDescriptor, ComServerDescriptor};
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
    DISPPARAMS, EXCEPINFO,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteKeyW,
    RegDeleteTreeW, RegSetValueExW,
};
use windows_sys::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
use windows_sys::Win32::System::Variant::{VARIANT, VT_EMPTY};
use windows_sys::core::GUID;

const PROJECT_NAME: &str = "__PROJECT_NAME__";
const LIBID: &str = "__LIBID__";
const BUNDLE_BYTES: &[u8] = include_bytes!(r#"__OXB_PATH__"#);
const DESCRIPTOR_JSON: &str = include_str!(r#"__DESCRIPTOR_PATH__"#);

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
const DISP_E_MEMBERNOTFOUND: i32 = 0x8002_0003u32 as i32;
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

static GLOBAL_REF_COUNT: AtomicU32 = AtomicU32::new(0);
static DESCRIPTOR: OnceLock<Result<ComServerDescriptor, String>> = OnceLock::new();
static mut MODULE_HANDLE: HMODULE = ptr::null_mut();

thread_local! {
    static SESSION: RefCell<Option<ProjectRuntimeSession>> = RefCell::new(None);
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
struct ClassFactory {
    vtbl: *const IClassFactoryVtbl,
    ref_count: AtomicU32,
    class_index: usize,
}

#[repr(C)]
struct DispatchObject {
    vtbl: *const IDispatchVtbl,
    ref_count: AtomicU32,
    class_index: usize,
    object: ObjectRef,
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
    let supports_default_interface = descriptor()
        .ok()
        .and_then(|descriptor| descriptor.classes.get(object.class_index))
        .is_some_and(|class| guid_matches_text(&*riid, &class.default_interface_iid));

    if guid_eq(&*riid, &IID_IUNKNOWN) || guid_eq(&*riid, &IID_IDISPATCH) || supports_default_interface
    {
        dispatch_add_ref(this);
        *out = this;
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
            let session = engine
                .prepare_bundle_package_session(package)
                .map_err(|err| err.to_string())?;
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
    Box::into_raw(Box::new(DispatchObject {
        vtbl: &DISPATCH_VTBL,
        ref_count: AtomicU32::new(1),
        class_index,
        object,
    }))
    .cast()
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
    Ok(())
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
