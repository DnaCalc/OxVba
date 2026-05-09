//! COM server DLL shim generation.
//!
//! Generates Rust source for an in-process COM server DLL that embeds compiled
//! `.oxb` bundles. Includes a functional `IClassFactory` that creates instances
//! backed by OxVba engine runtime sessions, delegating `IDispatch` through
//! `DynamicObjectBridge`.

use std::path::{Path, PathBuf};

use oxvba_project::ComClassExportDescriptor;

use crate::compile::{BuildError, ShimOutputType, compile_shim};
use crate::idl::deterministic_uuid;
use crate::typelib_gen::{TypeLibGenError, generate_typelib};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedComServerBuildOutput {
    pub dll_path: PathBuf,
    pub tlb_path: PathBuf,
}

#[derive(Debug)]
pub enum WrappedComServerBuildError {
    UnsupportedPlatform { target_os: &'static str },
    Build(BuildError),
    TypeLib(TypeLibGenError),
}

impl std::fmt::Display for WrappedComServerBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform { target_os } => {
                write!(
                    f,
                    "WrappedComServer DLL builds require Windows; current target_os={target_os}"
                )
            }
            Self::Build(err) => write!(f, "{err}"),
            Self::TypeLib(err) => write!(f, "{err}"),
        }
    }
}

impl From<BuildError> for WrappedComServerBuildError {
    fn from(value: BuildError) -> Self {
        Self::Build(value)
    }
}

impl From<TypeLibGenError> for WrappedComServerBuildError {
    fn from(value: TypeLibGenError) -> Self {
        Self::TypeLib(value)
    }
}

pub fn compile_wrapped_com_server_shim(
    project_name: &str,
    oxb_path: &str,
    classes: &[ComClassExportDescriptor],
    output_path: &Path,
) -> Result<WrappedComServerBuildOutput, WrappedComServerBuildError> {
    if !cfg!(target_os = "windows") {
        return Err(WrappedComServerBuildError::UnsupportedPlatform {
            target_os: std::env::consts::OS,
        });
    }

    let tlb_path = output_path.with_extension("tlb");
    let tlb_literal = tlb_path.to_string_lossy().replace('\\', "/");
    generate_typelib(project_name, &tlb_literal, classes)?;
    let source = generate_com_server_shim(project_name, oxb_path, Some(&tlb_literal), classes);
    compile_shim(&source, output_path, ShimOutputType::Dll)?;
    Ok(WrappedComServerBuildOutput {
        dll_path: output_path.to_path_buf(),
        tlb_path,
    })
}

/// Generate Rust source code for a COM server in-process DLL.
///
/// The generated DLL includes:
/// - `DllMain` — stores the module handle
/// - `DllGetClassObject` — returns an `IClassFactory` for each registered CLSID
/// - `DllCanUnloadNow` — checks the global reference count
/// - `DllRegisterServer` / `DllUnregisterServer` — registration stubs (see `registration.rs` for real implementation)
/// - `OxVbaClassFactory` — `IClassFactory` implementation per class
/// - `OxVbaDispatchInstance` — `IDispatch` wrapper delegating to the runtime engine
pub fn generate_com_server_shim(
    project_name: &str,
    oxb_path: &str,
    typelib_path: Option<&str>,
    classes: &[ComClassExportDescriptor],
) -> String {
    let mut source = String::new();

    // Header and imports
    source.push_str(&format!(
        r#"//! Auto-generated OxVBA COM server DLL for project "{project_name}".

#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]
#![cfg(target_os = "windows")]

use std::ffi::c_void;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{{AtomicI32, Ordering}};

use oxvba_compiler::OxBundle;
use oxvba_com::{{
    COM_DISP_E_MEMBERNOTFOUND, COM_DISP_E_TYPEMISMATCH, disp_params_to_runtime_call_frame,
    runtime_call_error_to_excepinfo, runtime_call_result_to_variant,
}};
use oxvba_host::{{Engine, HostConfig, ProjectRuntimeSession}};
use oxvba_runtime::{{
    ObjectRef, RuntimeCallError, RuntimeCallKind, RuntimeCallResult, RuntimeCallSource, Variant as RuntimeVariant,
}};
use windows_sys::Win32::System::Com::{{
    DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPATCH_PROPERTYPUTREF,
    DISPPARAMS, EXCEPINFO,
}};
use windows_sys::Win32::System::Variant::VARIANT;

const BUNDLE_BYTES: &[u8] = include_bytes!("{oxb_path}");

static GLOBAL_REF_COUNT: AtomicI32 = AtomicI32::new(0);
static mut H_MODULE: *mut c_void = std::ptr::null_mut();

const S_OK: i32 = 0;
const E_INVALIDARG: i32 = 0x80070057_u32 as i32;
const E_NOINTERFACE: i32 = 0x80004002_u32 as i32;
const E_OUTOFMEMORY: i32 = 0x8007000E_u32 as i32;
const CLASS_E_CLASSNOTAVAILABLE: i32 = 0x80040111_u32 as i32;
const CLASS_E_NOAGGREGATION: i32 = 0x80040110_u32 as i32;
const DISP_E_UNKNOWNNAME: i32 = 0x80020006_u32 as i32;
const E_NOTIMPL: i32 = 0x80004001_u32 as i32;

"#
    ));

    // GUID struct and IID constants
    source.push_str(&generate_guid_definitions(project_name, classes));
    source.push_str(&generate_class_table(classes));
    source.push_str(&generate_member_tables(classes));

    // IUnknown / IDispatch / IClassFactory vtable structs
    source.push_str(generate_vtable_structs());

    // DllMain
    source.push_str(
        r#"
// ── DLL Entry Points ──

#[unsafe(no_mangle)]
pub extern "system" fn DllMain(
    h_instance: *mut c_void,
    dw_reason: u32,
    _lp_reserved: *mut c_void,
) -> i32 {
    if dw_reason == 1 { // DLL_PROCESS_ATTACH
        unsafe { H_MODULE = h_instance; }
    }
    1 // TRUE
}

"#,
    );

    // DllGetClassObject — matches CLSIDs and returns class factories
    source.push_str(&generate_dll_get_class_object(project_name, classes));

    // DllCanUnloadNow
    source.push_str(
        r#"
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> i32 {
    if GLOBAL_REF_COUNT.load(Ordering::SeqCst) == 0 { S_OK } else { 1 }
}

"#,
    );

    // DllRegisterServer / DllUnregisterServer (delegates to registration module)
    source.push_str(&generate_registration_exports(
        project_name,
        typelib_path,
        classes,
    ));

    // IClassFactory implementation
    source.push_str(generate_class_factory_impl());

    // IDispatch instance implementation
    source.push_str(generate_dispatch_instance_impl());

    source
}

fn generate_guid_definitions(project_name: &str, classes: &[ComClassExportDescriptor]) -> String {
    let mut s = String::new();

    s.push_str(
        r#"// ── GUID Definitions ──

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct GUID {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const IID_IUNKNOWN: GUID = GUID {
    data1: 0x00000000, data2: 0x0000, data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};
const IID_IDISPATCH: GUID = GUID {
    data1: 0x00020400, data2: 0x0000, data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};
const IID_ICLASSFACTORY: GUID = GUID {
    data1: 0x00000001, data2: 0x0000, data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

"#,
    );

    // Per-class CLSID constants
    for class in classes {
        let uuid_str = deterministic_uuid(project_name, &class.class_name);
        let guid = crate::typelib_gen::parse_uuid(&uuid_str);
        s.push_str(&format!(
            "const CLSID_{}: GUID = GUID {{\n    data1: {:#010X}, data2: {:#06X}, data3: {:#06X},\n    data4: [{:#04X}, {:#04X}, {:#04X}, {:#04X}, {:#04X}, {:#04X}, {:#04X}, {:#04X}],\n}};\n\n",
            class.class_name.to_ascii_uppercase(),
            guid.data1, guid.data2, guid.data3,
            guid.data4[0], guid.data4[1], guid.data4[2], guid.data4[3],
            guid.data4[4], guid.data4[5], guid.data4[6], guid.data4[7],
        ));
    }

    s
}

fn generate_class_table(classes: &[ComClassExportDescriptor]) -> String {
    let mut s = String::from("const CLASS_NAMES: &[&str] = &[\n");
    for class in classes {
        s.push_str(&format!("    \"{}\",\n", class.class_name));
    }
    s.push_str("];\n\n");
    s
}

fn generate_member_tables(classes: &[ComClassExportDescriptor]) -> String {
    let mut s = String::from(
        r#"struct MemberDescriptor {
    name: &'static str,
    dispid: i32,
    kind: u8,
    is_default_member: bool,
}

const MEMBER_KIND_METHOD: u8 = 0;
const MEMBER_KIND_PROPERTYGET: u8 = 1;
const MEMBER_KIND_PROPERTYLET: u8 = 2;
const MEMBER_KIND_PROPERTYSET: u8 = 3;

"#,
    );
    for (class_index, class) in classes.iter().enumerate() {
        s.push_str(&format!(
            "static CLASS_{class_index}_MEMBERS: &[MemberDescriptor] = &[\n"
        ));
        for (member_index, member) in class.members.iter().enumerate() {
            let dispid = member.dispatch_id_or(member_index + 1);
            let kind = match member.kind {
                oxvba_compiler::ProjectDynamicMemberKind::Method
                | oxvba_compiler::ProjectDynamicMemberKind::Function => "MEMBER_KIND_METHOD",
                oxvba_compiler::ProjectDynamicMemberKind::PropertyGet => "MEMBER_KIND_PROPERTYGET",
                oxvba_compiler::ProjectDynamicMemberKind::PropertyLet => "MEMBER_KIND_PROPERTYLET",
                oxvba_compiler::ProjectDynamicMemberKind::PropertySet => "MEMBER_KIND_PROPERTYSET",
            };
            let is_default_member = member.is_default_member;
            s.push_str(&format!(
                "    MemberDescriptor {{ name: \"{}\", dispid: {dispid}, kind: {kind}, is_default_member: {is_default_member} }},\n",
                rust_string_literal(&member.member_name)
            ));
        }
        s.push_str("];\n\n");
    }
    s.push_str("static CLASS_MEMBERS: &[&[MemberDescriptor]] = &[\n");
    for class_index in 0..classes.len() {
        s.push_str(&format!("    CLASS_{class_index}_MEMBERS,\n"));
    }
    s.push_str("];\n\n");
    s.push_str(
        r#"fn members_for_class(class_index: usize) -> &'static [MemberDescriptor] {
    CLASS_MEMBERS.get(class_index).copied().unwrap_or(&[])
}

fn member_by_dispid(class_index: usize, dispid: i32) -> Option<&'static MemberDescriptor> {
    members_for_class(class_index)
        .iter()
        .find(|member| member.dispid == dispid)
}

fn member_by_dispid_and_flags(
    class_index: usize,
    dispid: i32,
    flags: u16,
) -> Option<&'static MemberDescriptor> {
    let members = members_for_class(class_index);
    let requested_kind = member_kind_from_dispatch_flags(flags);
    members
        .iter()
        .find(|member| member.dispid == dispid && member.kind == requested_kind)
        .or_else(|| {
            if dispid == 0 {
                members
                    .iter()
                    .find(|member| member.is_default_member && member.kind == requested_kind)
            } else {
                None
            }
        })
        .or_else(|| members.iter().find(|member| member.dispid == dispid))
}

fn member_by_name(class_index: usize, name: &str) -> Option<&'static MemberDescriptor> {
    members_for_class(class_index)
        .iter()
        .find(|member| member.name.eq_ignore_ascii_case(name))
}

fn member_kind_from_dispatch_flags(flags: u16) -> u8 {
    if flags & DISPATCH_PROPERTYPUTREF as u16 != 0 {
        MEMBER_KIND_PROPERTYSET
    } else if flags & DISPATCH_PROPERTYPUT as u16 != 0 {
        MEMBER_KIND_PROPERTYLET
    } else if flags & DISPATCH_PROPERTYGET as u16 != 0 {
        MEMBER_KIND_PROPERTYGET
    } else {
        MEMBER_KIND_METHOD
    }
}

fn runtime_call_kind_from_member_kind(kind: u8) -> RuntimeCallKind {
    match kind {
        MEMBER_KIND_PROPERTYGET => RuntimeCallKind::PropertyGet,
        MEMBER_KIND_PROPERTYLET => RuntimeCallKind::PropertyLet,
        MEMBER_KIND_PROPERTYSET => RuntimeCallKind::PropertySet,
        _ => RuntimeCallKind::Method,
    }
}

unsafe fn wide_name_to_string(ptr: *const u16) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    Some(String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len)))
}

"#,
    );
    s
}

fn rust_string_literal(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn generate_vtable_structs() -> &'static str {
    r#"// ── COM Vtable Structs ──

#[repr(C)]
struct IUnknownVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
}

#[repr(C)]
struct IClassFactoryVtbl {
    // IUnknown
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    // IClassFactory
    create_instance: unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, *mut *mut c_void) -> i32,
    lock_server: unsafe extern "system" fn(*mut c_void, i32) -> i32,
}

#[repr(C)]
struct IDispatchVtbl {
    // IUnknown
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    // IDispatch
    get_type_info_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(*mut c_void, u32, u32, *mut *mut c_void) -> i32,
    get_ids_of_names: unsafe extern "system" fn(*mut c_void, *const GUID, *const *const u16, u32, u32, *mut i32) -> i32,
    invoke: unsafe extern "system" fn(*mut c_void, i32, *const GUID, u32, u16, *mut c_void, *mut c_void, *mut c_void, *mut u32) -> i32,
}

"#
}

fn generate_dll_get_class_object(
    project_name: &str,
    classes: &[ComClassExportDescriptor],
) -> String {
    let mut source = String::from(
        r#"#[unsafe(no_mangle)]
pub extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> i32 {
    if ppv.is_null() || rclsid.is_null() || riid.is_null() {
        return E_INVALIDARG;
    }
    unsafe { *ppv = std::ptr::null_mut(); }

    let clsid = unsafe { &*rclsid };
    let iid = unsafe { &*riid };

    // Only support IUnknown and IClassFactory
    if *iid != IID_IUNKNOWN && *iid != IID_ICLASSFACTORY {
        return E_NOINTERFACE;
    }

"#,
    );

    for (i, class) in classes.iter().enumerate() {
        let const_name = class.class_name.to_ascii_uppercase();
        let _ = (project_name, i);
        source.push_str(&format!(
            r#"    if *clsid == CLSID_{const_name} {{
        let factory = OxVbaClassFactory::new({i});
        let raw = Box::into_raw(Box::new(factory));
        unsafe {{ *ppv = raw as *mut c_void; }}
        GLOBAL_REF_COUNT.fetch_add(1, Ordering::SeqCst);
        return S_OK;
    }}

"#
        ));
    }

    source.push_str(
        r#"    CLASS_E_CLASSNOTAVAILABLE
}

"#,
    );
    source
}

fn generate_registration_exports(
    project_name: &str,
    typelib_path: Option<&str>,
    classes: &[ComClassExportDescriptor],
) -> String {
    let mut s = String::new();

    s.push_str(
        r#"// ── Registration ──

const HKEY_CURRENT_USER: isize = 0x80000001_u32 as isize;
const KEY_ALL_ACCESS: u32 = 0xF003F;
const REG_SZ: u32 = 1;
const ERROR_SUCCESS: u32 = 0;
const MAX_PATH_CHARS: usize = 32768;

#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegCreateKeyExW(
        hkey: isize,
        lpsubkey: *const u16,
        reserved: u32,
        lpclass: *const u16,
        dwoptions: u32,
        samdesired: u32,
        lpsecurityattributes: *const c_void,
        phkresult: *mut isize,
        lpdwdisposition: *mut u32,
    ) -> u32;
    fn RegSetValueExW(
        hkey: isize,
        lpvaluename: *const u16,
        reserved: u32,
        dwtype: u32,
        lpdata: *const u8,
        cbdata: u32,
    ) -> u32;
    fn RegCloseKey(hkey: isize) -> u32;
    fn RegDeleteTreeW(hkey: isize, lpsubkey: *const u16) -> u32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleFileNameW(hmodule: *mut c_void, lpfilename: *mut u16, nsize: u32) -> u32;
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn module_path() -> Option<String> {
    let mut buffer = vec![0u16; MAX_PATH_CHARS];
    let len = unsafe { GetModuleFileNameW(H_MODULE, buffer.as_mut_ptr(), buffer.len() as u32) };
    if len == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..len as usize]))
}

fn set_key_default_value(parent: isize, subkey: &str, value: &str) -> bool {
    let subkey_w = to_wide(subkey);
    let mut hkey: isize = 0;
    let mut disposition: u32 = 0;
    unsafe {
        if RegCreateKeyExW(
            parent,
            subkey_w.as_ptr(),
            0,
            std::ptr::null(),
            0,
            KEY_ALL_ACCESS,
            std::ptr::null(),
            &mut hkey,
            &mut disposition,
        ) != ERROR_SUCCESS
        {
            return false;
        }
        let value_w = to_wide(value);
        let byte_len = (value_w.len() * 2) as u32;
        let ok = RegSetValueExW(
            hkey,
            std::ptr::null(),
            0,
            REG_SZ,
            value_w.as_ptr().cast::<u8>(),
            byte_len,
        ) == ERROR_SUCCESS;
        RegCloseKey(hkey);
        ok
    }
}

fn set_key_named_value(parent: isize, subkey: &str, name: &str, value: &str) -> bool {
    let subkey_w = to_wide(subkey);
    let mut hkey: isize = 0;
    let mut disposition: u32 = 0;
    unsafe {
        if RegCreateKeyExW(
            parent,
            subkey_w.as_ptr(),
            0,
            std::ptr::null(),
            0,
            KEY_ALL_ACCESS,
            std::ptr::null(),
            &mut hkey,
            &mut disposition,
        ) != ERROR_SUCCESS
        {
            return false;
        }
        let name_w = to_wide(name);
        let value_w = to_wide(value);
        let byte_len = (value_w.len() * 2) as u32;
        let ok = RegSetValueExW(
            hkey,
            name_w.as_ptr(),
            0,
            REG_SZ,
            value_w.as_ptr().cast::<u8>(),
            byte_len,
        ) == ERROR_SUCCESS;
        RegCloseKey(hkey);
        ok
    }
}

fn delete_key_tree(parent: isize, subkey: &str) {
    let subkey_w = to_wide(subkey);
    unsafe {
        RegDeleteTreeW(parent, subkey_w.as_ptr());
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> i32 {
    let Some(dll_path) = module_path() else {
        return E_INVALIDARG;
    };
"#,
    );

    let libid = format!("{{{}}}", deterministic_uuid(project_name, "__typelib__"));
    let typelib_key = format!("Software\\Classes\\TypeLib\\{libid}\\1.0\\0\\win64");
    let typelib_flags_key = format!("Software\\Classes\\TypeLib\\{libid}\\1.0\\FLAGS");
    let typelib_helpdir_key = format!("Software\\Classes\\TypeLib\\{libid}\\1.0\\HELPDIR");

    for class in classes {
        let default_prog_id = format!("{project_name}.{}", class.class_name);
        let prog_id = class.prog_id.as_deref().unwrap_or(&default_prog_id);
        let clsid = format!(
            "{{{}}}",
            deterministic_uuid(project_name, &class.class_name)
        );
        let description = class
            .description
            .as_deref()
            .unwrap_or(&class.class_name)
            .to_string();
        let clsid_key = format!("Software\\Classes\\CLSID\\{clsid}");
        let clsid_inproc_key = format!("{clsid_key}\\InprocServer32");
        let clsid_progid_key = format!("{clsid_key}\\ProgID");
        let clsid_typelib_key = format!("{clsid_key}\\TypeLib");
        let progid_key = format!("Software\\Classes\\{prog_id}");
        let progid_clsid_key = format!("{progid_key}\\CLSID");
        s.push_str(&format!(
            r#"    if !set_key_default_value(HKEY_CURRENT_USER, "{}", "{}") {{ return E_INVALIDARG; }}
    if !set_key_default_value(HKEY_CURRENT_USER, "{}", &dll_path) {{ return E_INVALIDARG; }}
    if !set_key_named_value(HKEY_CURRENT_USER, "{}", "ThreadingModel", "Apartment") {{ return E_INVALIDARG; }}
    if !set_key_default_value(HKEY_CURRENT_USER, "{}", "{}") {{ return E_INVALIDARG; }}
    if !set_key_default_value(HKEY_CURRENT_USER, "{}", "{}") {{ return E_INVALIDARG; }}
    if !set_key_default_value(HKEY_CURRENT_USER, "{}", "{}") {{ return E_INVALIDARG; }}
    if !set_key_default_value(HKEY_CURRENT_USER, "{}", "{}") {{ return E_INVALIDARG; }}
"#,
            rust_string_literal(&clsid_key),
            rust_string_literal(&description),
            rust_string_literal(&clsid_inproc_key),
            rust_string_literal(&clsid_inproc_key),
            rust_string_literal(&clsid_progid_key),
            rust_string_literal(prog_id),
            rust_string_literal(&clsid_typelib_key),
            rust_string_literal(&libid),
            rust_string_literal(&progid_key),
            rust_string_literal(&description),
            rust_string_literal(&progid_clsid_key),
            rust_string_literal(&clsid),
        ));
    }

    if let Some(typelib_path) = typelib_path {
        s.push_str(&format!(
            r#"    if !set_key_default_value(HKEY_CURRENT_USER, "{}", "{}") {{ return E_INVALIDARG; }}
    if !set_key_default_value(HKEY_CURRENT_USER, "{}", "0") {{ return E_INVALIDARG; }}
    if !set_key_default_value(HKEY_CURRENT_USER, "{}", "") {{ return E_INVALIDARG; }}
"#,
            rust_string_literal(&typelib_key),
            rust_string_literal(typelib_path),
            rust_string_literal(&typelib_flags_key),
            rust_string_literal(&typelib_helpdir_key),
        ));
    }

    s.push_str(
        r#"    S_OK
}

#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> i32 {
"#,
    );

    for class in classes {
        let default_prog_id = format!("{project_name}.{}", class.class_name);
        let prog_id = class.prog_id.as_deref().unwrap_or(&default_prog_id);
        let clsid = format!(
            "{{{}}}",
            deterministic_uuid(project_name, &class.class_name)
        );
        let clsid_key = format!("Software\\Classes\\CLSID\\{clsid}");
        let progid_key = format!("Software\\Classes\\{prog_id}");
        s.push_str(&format!(
            r#"    delete_key_tree(HKEY_CURRENT_USER, "{}");
    delete_key_tree(HKEY_CURRENT_USER, "{}");
"#,
            rust_string_literal(&clsid_key),
            rust_string_literal(&progid_key),
        ));
    }
    s.push_str(&format!(
        r#"    delete_key_tree(HKEY_CURRENT_USER, "{}");
"#,
        rust_string_literal(&format!("Software\\Classes\\TypeLib\\{libid}")),
    ));

    s.push_str(
        r#"    S_OK
}

"#,
    );

    s
}

fn generate_class_factory_impl() -> &'static str {
    r#"// ── IClassFactory Implementation ──

/// An OxVba class factory. Each factory knows its class index into the project's
/// class table and creates instances backed by the embedded bytecode bundle.
#[repr(C)]
struct OxVbaClassFactory {
    vtbl: *const IClassFactoryVtbl,
    ref_count: AtomicI32,
    class_index: usize,
}

static CLASS_FACTORY_VTBL: IClassFactoryVtbl = IClassFactoryVtbl {
    query_interface: cf_query_interface,
    add_ref: cf_add_ref,
    release: cf_release,
    create_instance: cf_create_instance,
    lock_server: cf_lock_server,
};

impl OxVbaClassFactory {
    fn new(class_index: usize) -> Self {
        Self {
            vtbl: &CLASS_FACTORY_VTBL,
            ref_count: AtomicI32::new(1),
            class_index,
        }
    }
}

unsafe extern "system" fn cf_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> i32 {
    if ppv.is_null() { return E_INVALIDARG; }
    *ppv = std::ptr::null_mut();
    let iid = &*riid;
    if *iid == IID_IUNKNOWN || *iid == IID_ICLASSFACTORY {
        *ppv = this;
        cf_add_ref(this);
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn cf_add_ref(this: *mut c_void) -> u32 {
    let factory = &*(this as *const OxVbaClassFactory);
    factory.ref_count.fetch_add(1, Ordering::SeqCst) as u32 + 1
}

unsafe extern "system" fn cf_release(this: *mut c_void) -> u32 {
    let factory = &*(this as *const OxVbaClassFactory);
    let prev = factory.ref_count.fetch_sub(1, Ordering::SeqCst);
    if prev <= 1 {
        drop(Box::from_raw(this as *mut OxVbaClassFactory));
        GLOBAL_REF_COUNT.fetch_sub(1, Ordering::SeqCst);
        0
    } else {
        (prev - 1) as u32
    }
}

unsafe extern "system" fn cf_create_instance(
    this: *mut c_void,
    p_unk_outer: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> i32 {
    if ppv.is_null() { return E_INVALIDARG; }
    *ppv = std::ptr::null_mut();

    // No aggregation support
    if !p_unk_outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }

    let iid = &*riid;
    if *iid != IID_IUNKNOWN && *iid != IID_IDISPATCH {
        return E_NOINTERFACE;
    }

    let factory = &*(this as *const OxVbaClassFactory);
    let instance = match OxVbaDispatchInstance::new(factory.class_index) {
        Ok(instance) => instance,
        Err(hr) => return hr,
    };
    let raw = Box::into_raw(Box::new(instance));
    *ppv = raw as *mut c_void;
    GLOBAL_REF_COUNT.fetch_add(1, Ordering::SeqCst);
    S_OK
}

unsafe extern "system" fn cf_lock_server(
    _this: *mut c_void,
    f_lock: i32,
) -> i32 {
    if f_lock != 0 {
        GLOBAL_REF_COUNT.fetch_add(1, Ordering::SeqCst);
    } else {
        GLOBAL_REF_COUNT.fetch_sub(1, Ordering::SeqCst);
    }
    S_OK
}

"#
}

fn generate_dispatch_instance_impl() -> &'static str {
    r#"// ── IDispatch Instance Implementation ──

/// An OxVba COM object instance. Wraps a class index and delegates IDispatch
/// calls to the OxVba runtime engine session loaded from BUNDLE_BYTES.
#[repr(C)]
struct OxVbaDispatchInstance {
    vtbl: *const IDispatchVtbl,
    ref_count: AtomicI32,
    class_index: usize,
    engine: Rc<Engine>,
    session: Rc<RefCell<ProjectRuntimeSession>>,
    object: ObjectRef,
}

static DISPATCH_VTBL: IDispatchVtbl = IDispatchVtbl {
    query_interface: di_query_interface,
    add_ref: di_add_ref,
    release: di_release,
    get_type_info_count: di_get_type_info_count,
    get_type_info: di_get_type_info,
    get_ids_of_names: di_get_ids_of_names,
    invoke: di_invoke,
};

impl OxVbaDispatchInstance {
    fn new(class_index: usize) -> Result<Self, i32> {
        let Some(class_name) = CLASS_NAMES.get(class_index).copied() else {
            return Err(CLASS_E_CLASSNOTAVAILABLE);
        };
        let bundle = OxBundle::deserialize_from_bytes(BUNDLE_BYTES)
            .map_err(|_| E_OUTOFMEMORY)?;
        let engine = Rc::new(Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        }));
        let mut session = engine
            .compile_and_prepare_session_from_bundle(&bundle)
            .map_err(|_| E_OUTOFMEMORY)?;
        let object = engine
            .create_class_instance(&mut session, class_name)
            .map_err(|_| CLASS_E_CLASSNOTAVAILABLE)?;
        Ok(Self {
            vtbl: &DISPATCH_VTBL,
            ref_count: AtomicI32::new(1),
            class_index,
            engine,
            session: Rc::new(RefCell::new(session)),
            object,
        })
    }

    fn from_existing(
        class_index: usize,
        engine: Rc<Engine>,
        session: Rc<RefCell<ProjectRuntimeSession>>,
        object: ObjectRef,
        ref_count: i32,
    ) -> Self {
        Self {
            vtbl: &DISPATCH_VTBL,
            ref_count: AtomicI32::new(ref_count),
            class_index,
            engine,
            session,
            object,
        }
    }
}

unsafe extern "system" fn di_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> i32 {
    if ppv.is_null() { return E_INVALIDARG; }
    *ppv = std::ptr::null_mut();
    let iid = &*riid;
    if *iid == IID_IUNKNOWN || *iid == IID_IDISPATCH {
        *ppv = this;
        di_add_ref(this);
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn di_add_ref(this: *mut c_void) -> u32 {
    let inst = &*(this as *const OxVbaDispatchInstance);
    inst.ref_count.fetch_add(1, Ordering::SeqCst) as u32 + 1
}

unsafe extern "system" fn di_release(this: *mut c_void) -> u32 {
    let inst = &*(this as *const OxVbaDispatchInstance);
    let prev = inst.ref_count.fetch_sub(1, Ordering::SeqCst);
    if prev <= 1 {
        // Future: trigger Class_Terminate here before deallocation
        drop(Box::from_raw(this as *mut OxVbaDispatchInstance));
        GLOBAL_REF_COUNT.fetch_sub(1, Ordering::SeqCst);
        0
    } else {
        (prev - 1) as u32
    }
}

unsafe extern "system" fn di_get_type_info_count(
    _this: *mut c_void,
    pctinfo: *mut u32,
) -> i32 {
    if !pctinfo.is_null() { *pctinfo = 0; }
    S_OK
}

unsafe extern "system" fn di_get_type_info(
    _this: *mut c_void,
    _i_tinfo: u32,
    _lcid: u32,
    _pp_tinfo: *mut *mut c_void,
) -> i32 {
    // Type info not provided until the typelib bead is wired into this path.
    E_NOTIMPL
}

unsafe extern "system" fn di_get_ids_of_names(
    this: *mut c_void,
    _riid: *const GUID,
    rgsznames: *const *const u16,
    cnames: u32,
    _lcid: u32,
    rgdispid: *mut i32,
) -> i32 {
    if rgsznames.is_null() || rgdispid.is_null() || cnames == 0 {
        return E_INVALIDARG;
    }
    let inst = &*(this as *const OxVbaDispatchInstance);
    let Some(name) = wide_name_to_string(*rgsznames) else {
        return E_INVALIDARG;
    };
    let Some(member) = member_by_name(inst.class_index, &name) else {
        return DISP_E_UNKNOWNNAME;
    };
    *rgdispid = member.dispid;
    S_OK
}

unsafe extern "system" fn di_invoke(
    this: *mut c_void,
    dispid_member: i32,
    _riid: *const GUID,
    lcid: u32,
    w_flags: u16,
    p_disp_params: *mut c_void,
    p_var_result: *mut c_void,
    p_excep_info: *mut c_void,
    pu_arg_err: *mut u32,
) -> i32 {
    let inst = &mut *(this as *mut OxVbaDispatchInstance);
    let Some(member) = member_by_dispid_and_flags(inst.class_index, dispid_member, w_flags) else {
        return COM_DISP_E_MEMBERNOTFOUND;
    };
    let params = p_disp_params as *mut DISPPARAMS;
    let frame = match disp_params_to_runtime_call_frame(dispid_member, w_flags, params, lcid) {
        Ok(frame) => frame,
        Err(err) => {
            let runtime_error = RuntimeCallError::new(
                COM_DISP_E_TYPEMISMATCH,
                err.message,
                RuntimeCallSource::ExternalComDispatch,
            );
            return runtime_call_error_to_excepinfo(
                &runtime_error,
                p_excep_info as *mut EXCEPINFO,
                pu_arg_err,
                if params.is_null() { 0 } else { (*params).cArgs as usize },
            );
        }
    };
    let mut args = frame
        .positional_args
        .into_iter()
        .map(|arg| arg.value)
        .collect::<Vec<RuntimeVariant>>();
    if let Some(property_put_arg) = frame.property_put_arg {
        args.push(property_put_arg.value);
    }
    let mut session = inst.session.borrow_mut();
    let value = match inst.engine.invoke_member_on_object_with_kind(
        &mut session,
        inst.object.clone(),
        member.name,
        Some(runtime_call_kind_from_member_kind(member.kind)),
        &args,
    ) {
        Ok(value) => value,
        Err(err) => {
            let runtime_error = RuntimeCallError::new(
                COM_DISP_E_MEMBERNOTFOUND,
                err.to_string(),
                RuntimeCallSource::ExternalComDispatch,
            );
            return runtime_call_error_to_excepinfo(
                &runtime_error,
                p_excep_info as *mut EXCEPINFO,
                pu_arg_err,
                if params.is_null() { 0 } else { (*params).cArgs as usize },
            );
        }
    };
    let value = match value.as_i32() {
        Some(handle) => {
            let object = ObjectRef::from_compat_identity(handle);
            if inst.engine.class_name_for_object(&session, &object).is_some() {
                RuntimeVariant::from_object_ref(object)
            } else {
                value
            }
        }
        None => value,
    };
    if !p_var_result.is_null() {
        if let Err(message) = runtime_call_result_to_variant(
            &RuntimeCallResult::value(value),
            p_var_result as *mut VARIANT,
            &mut |object| {
                if object.raw() == inst.object.raw() {
                    return Ok(this);
                }
                let Some(class_name) = inst.engine.class_name_for_object(&session, &object) else {
                    return Err("WrappedComServer object result did not map to a project class".to_string());
                };
                let Some(class_index) = CLASS_NAMES
                    .iter()
                    .position(|candidate| candidate.eq_ignore_ascii_case(class_name))
                else {
                    return Err(format!(
                        "WrappedComServer object result class `{class_name}` is not exported"
                    ));
                };
                let child = OxVbaDispatchInstance::from_existing(
                    class_index,
                    Rc::clone(&inst.engine),
                    Rc::clone(&inst.session),
                    object,
                    0,
                );
                let raw = Box::into_raw(Box::new(child)) as *mut c_void;
                GLOBAL_REF_COUNT.fetch_add(1, Ordering::SeqCst);
                Ok(raw)
            },
            &mut |dispatch| {
                if !dispatch.is_null() {
                    di_add_ref(dispatch);
                }
            },
        ) {
            let runtime_error = RuntimeCallError::new(
                COM_DISP_E_TYPEMISMATCH,
                message,
                RuntimeCallSource::ExternalComDispatch,
            );
            return runtime_call_error_to_excepinfo(
                &runtime_error,
                p_excep_info as *mut EXCEPINFO,
                pu_arg_err,
                if params.is_null() { 0 } else { (*params).cArgs as usize },
            );
        }
    }
    S_OK
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_project::{ComClassExportDescriptor, DispatchMemberInfo, Instancing};
    use std::path::{Path, PathBuf};

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "{prefix}_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("unix epoch")
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn com_server_shim_structure() {
        let classes = vec![ComClassExportDescriptor {
            class_name: "Widget".to_string(),
            prog_id: Some("TestProj.Widget".to_string()),
            instancing: Some(Instancing::MultiUse),
            description: Some("A widget".to_string()),
            members: vec![],
        }];

        let source = generate_com_server_shim("TestProj", "test.oxb", None, &classes);
        assert!(source.contains("DllMain"));
        assert!(source.contains("DllGetClassObject"));
        assert!(source.contains("DllCanUnloadNow"));
        assert!(source.contains("DllRegisterServer"));
        assert!(source.contains("DllUnregisterServer"));
        assert!(source.contains("Widget"));
        assert!(source.contains("TestProj.Widget"));
    }

    #[test]
    fn com_server_has_class_factory() {
        let classes = vec![ComClassExportDescriptor {
            class_name: "Calculator".to_string(),
            prog_id: Some("TestApp.Calculator".to_string()),
            instancing: None,
            description: None,
            members: vec![DispatchMemberInfo {
                member_name: "Add".to_string(),
                kind: oxvba_compiler::ProjectDynamicMemberKind::Function,
                param_count: 2,
                dispatch_id: None,
                member_flags: None,
                is_default_member: false,
            }],
        }];

        let source = generate_com_server_shim("TestApp", "test.oxb", None, &classes);
        assert!(source.contains("OxVbaClassFactory"));
        assert!(source.contains("IClassFactoryVtbl"));
        assert!(source.contains("cf_create_instance"));
        assert!(source.contains("CLSID_CALCULATOR"));
    }

    #[test]
    fn com_server_has_dispatch_instance() {
        let classes = vec![ComClassExportDescriptor {
            class_name: "Widget".to_string(),
            prog_id: None,
            instancing: None,
            description: None,
            members: vec![],
        }];

        let source = generate_com_server_shim("TestApp", "test.oxb", None, &classes);
        assert!(source.contains("OxVbaDispatchInstance"));
        assert!(source.contains("compile_and_prepare_session_from_bundle"));
        assert!(source.contains("create_class_instance"));
        assert!(source.contains("IDispatchVtbl"));
        assert!(source.contains("di_query_interface"));
        assert!(source.contains("di_invoke"));
        assert!(source.contains("di_get_ids_of_names"));
    }

    #[test]
    fn com_server_multiple_classes() {
        let classes = vec![
            ComClassExportDescriptor {
                class_name: "Alpha".to_string(),
                prog_id: Some("Multi.Alpha".to_string()),
                instancing: None,
                description: None,
                members: vec![],
            },
            ComClassExportDescriptor {
                class_name: "Beta".to_string(),
                prog_id: Some("Multi.Beta".to_string()),
                instancing: None,
                description: None,
                members: vec![],
            },
        ];

        let source = generate_com_server_shim("Multi", "test.oxb", None, &classes);
        assert!(source.contains("CLSID_ALPHA"));
        assert!(source.contains("CLSID_BETA"));
        assert!(source.contains("Multi.Alpha"));
        assert!(source.contains("Multi.Beta"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn wrapped_com_server_build_reports_typed_non_windows_unsupported() {
        let classes = vec![ComClassExportDescriptor {
            class_name: "Widget".to_string(),
            prog_id: Some("TestProj.Widget".to_string()),
            instancing: Some(Instancing::MultiUse),
            description: None,
            members: vec![DispatchMemberInfo {
                member_name: "Ping".to_string(),
                kind: oxvba_compiler::ProjectDynamicMemberKind::Function,
                param_count: 0,
                dispatch_id: Some(1),
                member_flags: None,
                is_default_member: false,
            }],
        }];
        let err = compile_wrapped_com_server_shim(
            "TestProj",
            "test.oxb",
            &classes,
            Path::new("TestProj.dll"),
        )
        .expect_err("non-Windows build should be typed unsupported");
        assert!(matches!(
            err,
            WrappedComServerBuildError::UnsupportedPlatform { .. }
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn wrapped_com_server_build_compiles_dll_with_standard_exports() {
        use oxvba_compiler::{
            ModuleAttributes, ModuleKind, ModuleUnit, OxBundle, ProjectKind, ProjectManifest,
            compile_project,
        };

        let temp = TempDirGuard::new("oxvba_wrapped_com_server_compile");
        let bundle_path = temp.path().join("bundle.oxb");
        let manifest = ProjectManifest {
            project_name: "TestProj".to_string(),
            project_kind: ProjectKind::Library,
            modules: vec![ModuleUnit {
                module_name: "Widget".to_string(),
                module_kind: ModuleKind::Class,
                attributes: ModuleAttributes {
                    vb_name: "Widget".to_string(),
                    vb_creatable: true,
                    vb_exposed: true,
                    ..Default::default()
                },
                source: "Private stored As Long\nPrivate Sub Class_Initialize()\nstored = 7\nEnd Sub\nPublic Function Ping() As Long\nPing = stored\nEnd Function\nPublic Property Get Value() As Long\nValue = stored\nEnd Property\nPublic Property Let Value(ByVal n As Long)\nstored = n\nEnd Property\nAttribute Value.VB_UserMemId = 0\nPublic Function ReturnChild() As Object\nDim c As New Child\nSet ReturnChild = c\nEnd Function\nPublic Function Numbers() As Variant\nNumbers = Array(2, 4, 6)\nEnd Function\nPublic Function Boom()\nErr.Raise 77\nEnd Function\n"
                    .to_string(),
            },
            ModuleUnit {
                module_name: "Child".to_string(),
                module_kind: ModuleKind::Class,
                attributes: ModuleAttributes {
                    vb_name: "Child".to_string(),
                    vb_creatable: true,
                    vb_exposed: true,
                    ..Default::default()
                },
                source: "Public Property Get Value() As Long\nValue = 19\nEnd Property\nAttribute Value.VB_UserMemId = 0\n"
                    .to_string(),
            }],
            references: vec![],
            reference_projects: vec![],
            conditional_constants: std::collections::BTreeMap::new(),
        };
        let compiled = compile_project(&manifest).expect("compile wrapped COM project");
        let bundle = OxBundle::from_compiled_project(&compiled, "TestProj");
        std::fs::write(
            &bundle_path,
            bundle.serialize_to_bytes().expect("serialize bundle"),
        )
        .expect("write bundle");
        let bundle_literal = bundle_path.to_string_lossy().replace('\\', "/");
        let dll_path = temp.path().join("TestProj.dll");
        let classes = vec![
            ComClassExportDescriptor {
                class_name: "Widget".to_string(),
                prog_id: Some("TestProj.Widget".to_string()),
                instancing: Some(Instancing::MultiUse),
                description: None,
                members: vec![
                    DispatchMemberInfo {
                        member_name: "Ping".to_string(),
                        kind: oxvba_compiler::ProjectDynamicMemberKind::Function,
                        param_count: 0,
                        dispatch_id: Some(1),
                        member_flags: None,
                        is_default_member: false,
                    },
                    DispatchMemberInfo {
                        member_name: "Value".to_string(),
                        kind: oxvba_compiler::ProjectDynamicMemberKind::PropertyGet,
                        param_count: 0,
                        dispatch_id: Some(0),
                        member_flags: None,
                        is_default_member: true,
                    },
                    DispatchMemberInfo {
                        member_name: "Value".to_string(),
                        kind: oxvba_compiler::ProjectDynamicMemberKind::PropertyLet,
                        param_count: 1,
                        dispatch_id: Some(0),
                        member_flags: None,
                        is_default_member: true,
                    },
                    DispatchMemberInfo {
                        member_name: "ReturnChild".to_string(),
                        kind: oxvba_compiler::ProjectDynamicMemberKind::Function,
                        param_count: 0,
                        dispatch_id: Some(2),
                        member_flags: None,
                        is_default_member: false,
                    },
                    DispatchMemberInfo {
                        member_name: "Numbers".to_string(),
                        kind: oxvba_compiler::ProjectDynamicMemberKind::Function,
                        param_count: 0,
                        dispatch_id: Some(3),
                        member_flags: None,
                        is_default_member: false,
                    },
                    DispatchMemberInfo {
                        member_name: "Boom".to_string(),
                        kind: oxvba_compiler::ProjectDynamicMemberKind::Function,
                        param_count: 0,
                        dispatch_id: Some(4),
                        member_flags: None,
                        is_default_member: false,
                    },
                ],
            },
            ComClassExportDescriptor {
                class_name: "Child".to_string(),
                prog_id: Some("TestProj.Child".to_string()),
                instancing: Some(Instancing::MultiUse),
                description: None,
                members: vec![DispatchMemberInfo {
                    member_name: "Value".to_string(),
                    kind: oxvba_compiler::ProjectDynamicMemberKind::PropertyGet,
                    param_count: 0,
                    dispatch_id: Some(0),
                    member_flags: None,
                    is_default_member: true,
                }],
            },
        ];

        let output =
            compile_wrapped_com_server_shim("TestProj", &bundle_literal, &classes, &dll_path)
                .expect("WrappedComServer DLL build should succeed");
        assert_eq!(output.dll_path, dll_path);
        assert!(output.dll_path.exists());
        assert_eq!(output.tlb_path, dll_path.with_extension("tlb"));
        assert!(output.tlb_path.exists());
        crate::typelib_gen::verify_typelib_roundtrip(
            &output.tlb_path.to_string_lossy(),
            "TestProj",
            &classes,
        )
        .expect("generated WrappedComServer typelib should load");

        let bytes = std::fs::read(&output.dll_path).expect("read built DLL");
        for export in [
            b"DllGetClassObject".as_slice(),
            b"DllCanUnloadNow".as_slice(),
            b"DllRegisterServer".as_slice(),
            b"DllUnregisterServer".as_slice(),
        ] {
            assert!(
                bytes.windows(export.len()).any(|window| window == export),
                "expected PE export name {}",
                String::from_utf8_lossy(export)
            );
        }

        unsafe {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::Foundation::{SysFreeString, SysStringLen};
            use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
            use windows_sys::Win32::System::Ole::{
                SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound,
            };

            #[repr(C)]
            #[derive(Clone, Copy, PartialEq, Eq)]
            struct TestGuid {
                data1: u32,
                data2: u16,
                data3: u16,
                data4: [u8; 8],
            }

            #[repr(C)]
            struct TestDispatchVtbl {
                query_interface: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *const TestGuid,
                    *mut *mut core::ffi::c_void,
                ) -> i32,
                add_ref: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
                release: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
                get_type_info_count:
                    unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> i32,
                get_type_info: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    u32,
                    u32,
                    *mut *mut core::ffi::c_void,
                ) -> i32,
                get_ids_of_names: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *const TestGuid,
                    *const *const u16,
                    u32,
                    u32,
                    *mut i32,
                ) -> i32,
                invoke: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    i32,
                    *const TestGuid,
                    u32,
                    u16,
                    *mut core::ffi::c_void,
                    *mut core::ffi::c_void,
                    *mut core::ffi::c_void,
                    *mut u32,
                ) -> i32,
            }

            #[repr(C)]
            struct TestClassFactoryVtbl {
                query_interface: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *const TestGuid,
                    *mut *mut core::ffi::c_void,
                ) -> i32,
                add_ref: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
                release: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
                create_instance: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *mut core::ffi::c_void,
                    *const TestGuid,
                    *mut *mut core::ffi::c_void,
                ) -> i32,
                lock_server: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> i32,
            }

            #[repr(C)]
            struct TestComObject {
                vtbl: *const TestDispatchVtbl,
            }

            #[repr(C)]
            struct TestClassFactory {
                vtbl: *const TestClassFactoryVtbl,
            }

            type DllGetClassObjectFn = unsafe extern "system" fn(
                *const TestGuid,
                *const TestGuid,
                *mut *mut core::ffi::c_void,
            ) -> i32;
            type DllCanUnloadNowFn = unsafe extern "system" fn() -> i32;
            type DllRegisterServerFn = unsafe extern "system" fn() -> i32;
            type DllUnregisterServerFn = unsafe extern "system" fn() -> i32;

            const S_OK: i32 = 0;
            const IID_IDISPATCH: TestGuid = TestGuid {
                data1: 0x00020400,
                data2: 0x0000,
                data3: 0x0000,
                data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
            };
            const IID_ICLASSFACTORY: TestGuid = TestGuid {
                data1: 0x00000001,
                data2: 0x0000,
                data3: 0x0000,
                data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
            };

            let mut wide_path: Vec<u16> = output
                .dll_path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let library = LoadLibraryW(wide_path.as_mut_ptr());
            assert!(!library.is_null(), "LoadLibraryW should load generated DLL");

            let get_class_object: DllGetClassObjectFn = std::mem::transmute(GetProcAddress(
                library,
                c"DllGetClassObject".as_ptr().cast(),
            ));
            let can_unload: DllCanUnloadNowFn =
                std::mem::transmute(GetProcAddress(library, c"DllCanUnloadNow".as_ptr().cast()));
            let register_server: DllRegisterServerFn = std::mem::transmute(GetProcAddress(
                library,
                c"DllRegisterServer".as_ptr().cast(),
            ));
            let unregister_server: DllUnregisterServerFn = std::mem::transmute(GetProcAddress(
                library,
                c"DllUnregisterServer".as_ptr().cast(),
            ));

            let uuid = deterministic_uuid("TestProj", "Widget");
            let parsed = crate::typelib_gen::parse_uuid(&uuid);
            let clsid = TestGuid {
                data1: parsed.data1,
                data2: parsed.data2,
                data3: parsed.data3,
                data4: parsed.data4,
            };

            let mut factory_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
            assert_eq!(
                get_class_object(&clsid, &IID_ICLASSFACTORY, &mut factory_ptr),
                S_OK
            );
            assert!(!factory_ptr.is_null());
            assert_ne!(can_unload(), S_OK);

            let factory = &*(factory_ptr as *const TestClassFactory);
            assert_eq!(((*factory.vtbl).lock_server)(factory_ptr, 1), S_OK);
            assert_ne!(can_unload(), S_OK);
            assert_eq!(((*factory.vtbl).lock_server)(factory_ptr, 0), S_OK);

            let mut dispatch_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
            assert_eq!(
                ((*factory.vtbl).create_instance)(
                    factory_ptr,
                    std::ptr::null_mut(),
                    &IID_IDISPATCH,
                    &mut dispatch_ptr,
                ),
                S_OK
            );
            assert!(!dispatch_ptr.is_null());

            let dispatch = &*(dispatch_ptr as *const TestComObject);
            let ping_name: Vec<u16> = "Ping".encode_utf16().chain(std::iter::once(0)).collect();
            let names = [ping_name.as_ptr()];
            let mut dispid = i32::MIN;
            assert_eq!(
                ((*dispatch.vtbl).get_ids_of_names)(
                    dispatch_ptr,
                    std::ptr::null(),
                    names.as_ptr(),
                    1,
                    0,
                    &mut dispid,
                ),
                S_OK
            );
            assert_ne!(dispid, i32::MIN);

            let mut params = windows_sys::Win32::System::Com::DISPPARAMS {
                rgvarg: std::ptr::null_mut(),
                rgdispidNamedArgs: std::ptr::null_mut(),
                cArgs: 0,
                cNamedArgs: 0,
            };
            let mut result: windows_sys::Win32::System::Variant::VARIANT = std::mem::zeroed();
            let mut arg_err = u32::MAX;
            assert_eq!(
                ((*dispatch.vtbl).invoke)(
                    dispatch_ptr,
                    dispid,
                    std::ptr::null(),
                    0,
                    windows_sys::Win32::System::Com::DISPATCH_METHOD as u16,
                    (&mut params as *mut windows_sys::Win32::System::Com::DISPPARAMS).cast(),
                    (&mut result as *mut windows_sys::Win32::System::Variant::VARIANT).cast(),
                    std::ptr::null_mut(),
                    &mut arg_err,
                ),
                S_OK
            );
            assert_eq!(
                result.Anonymous.Anonymous.vt,
                windows_sys::Win32::System::Variant::VT_I4
            );
            assert_eq!(result.Anonymous.Anonymous.Anonymous.lVal, 7);
            windows_sys::Win32::System::Variant::VariantClear(&mut result);

            let value_name: Vec<u16> = "Value".encode_utf16().chain(std::iter::once(0)).collect();
            let names = [value_name.as_ptr()];
            let mut value_dispid = i32::MIN;
            assert_eq!(
                ((*dispatch.vtbl).get_ids_of_names)(
                    dispatch_ptr,
                    std::ptr::null(),
                    names.as_ptr(),
                    1,
                    0,
                    &mut value_dispid,
                ),
                S_OK
            );
            assert_eq!(value_dispid, 0);

            let mut put_arg: windows_sys::Win32::System::Variant::VARIANT = std::mem::zeroed();
            put_arg.Anonymous.Anonymous.vt = windows_sys::Win32::System::Variant::VT_I4;
            put_arg.Anonymous.Anonymous.Anonymous.lVal = 41;
            let mut property_put_dispid = oxvba_com::windows_client::COM_DISPID_PROPERTYPUT;
            let mut put_params = windows_sys::Win32::System::Com::DISPPARAMS {
                rgvarg: &mut put_arg,
                rgdispidNamedArgs: &mut property_put_dispid,
                cArgs: 1,
                cNamedArgs: 1,
            };
            assert_eq!(
                ((*dispatch.vtbl).invoke)(
                    dispatch_ptr,
                    value_dispid,
                    std::ptr::null(),
                    0,
                    windows_sys::Win32::System::Com::DISPATCH_PROPERTYPUT as u16,
                    (&mut put_params as *mut windows_sys::Win32::System::Com::DISPPARAMS).cast(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut arg_err,
                ),
                S_OK
            );

            let mut get_params = windows_sys::Win32::System::Com::DISPPARAMS {
                rgvarg: std::ptr::null_mut(),
                rgdispidNamedArgs: std::ptr::null_mut(),
                cArgs: 0,
                cNamedArgs: 0,
            };
            let mut value_result: windows_sys::Win32::System::Variant::VARIANT = std::mem::zeroed();
            assert_eq!(
                ((*dispatch.vtbl).invoke)(
                    dispatch_ptr,
                    value_dispid,
                    std::ptr::null(),
                    0,
                    windows_sys::Win32::System::Com::DISPATCH_PROPERTYGET as u16,
                    (&mut get_params as *mut windows_sys::Win32::System::Com::DISPPARAMS).cast(),
                    (&mut value_result as *mut windows_sys::Win32::System::Variant::VARIANT).cast(),
                    std::ptr::null_mut(),
                    &mut arg_err,
                ),
                S_OK
            );
            assert_eq!(
                value_result.Anonymous.Anonymous.vt,
                windows_sys::Win32::System::Variant::VT_I4
            );
            assert_eq!(value_result.Anonymous.Anonymous.Anonymous.lVal, 41);
            windows_sys::Win32::System::Variant::VariantClear(&mut value_result);

            let self_name: Vec<u16> = "ReturnChild"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let names = [self_name.as_ptr()];
            let mut self_dispid = i32::MIN;
            assert_eq!(
                ((*dispatch.vtbl).get_ids_of_names)(
                    dispatch_ptr,
                    std::ptr::null(),
                    names.as_ptr(),
                    1,
                    0,
                    &mut self_dispid,
                ),
                S_OK
            );
            let mut self_result: windows_sys::Win32::System::Variant::VARIANT = std::mem::zeroed();
            let mut self_excep: windows_sys::Win32::System::Com::EXCEPINFO = std::mem::zeroed();
            let self_hr = ((*dispatch.vtbl).invoke)(
                dispatch_ptr,
                self_dispid,
                std::ptr::null(),
                0,
                windows_sys::Win32::System::Com::DISPATCH_METHOD as u16,
                (&mut get_params as *mut windows_sys::Win32::System::Com::DISPPARAMS).cast(),
                (&mut self_result as *mut windows_sys::Win32::System::Variant::VARIANT).cast(),
                (&mut self_excep as *mut windows_sys::Win32::System::Com::EXCEPINFO).cast(),
                &mut arg_err,
            );
            if self_hr != S_OK {
                let description = if self_excep.bstrDescription.is_null() {
                    String::new()
                } else {
                    let len = SysStringLen(self_excep.bstrDescription) as usize;
                    String::from_utf16_lossy(std::slice::from_raw_parts(
                        self_excep.bstrDescription,
                        len,
                    ))
                };
                SysFreeString(self_excep.bstrSource);
                SysFreeString(self_excep.bstrDescription);
                panic!("ReturnChild Invoke failed: hr={self_hr:#010X}; {description}");
            }
            assert_eq!(
                self_result.Anonymous.Anonymous.vt,
                windows_sys::Win32::System::Variant::VT_DISPATCH
            );
            assert!(!self_result.Anonymous.Anonymous.Anonymous.pdispVal.is_null());
            windows_sys::Win32::System::Variant::VariantClear(&mut self_result);

            let numbers_name: Vec<u16> =
                "Numbers".encode_utf16().chain(std::iter::once(0)).collect();
            let names = [numbers_name.as_ptr()];
            let mut numbers_dispid = i32::MIN;
            assert_eq!(
                ((*dispatch.vtbl).get_ids_of_names)(
                    dispatch_ptr,
                    std::ptr::null(),
                    names.as_ptr(),
                    1,
                    0,
                    &mut numbers_dispid,
                ),
                S_OK
            );
            let mut numbers_result: windows_sys::Win32::System::Variant::VARIANT =
                std::mem::zeroed();
            assert_eq!(
                ((*dispatch.vtbl).invoke)(
                    dispatch_ptr,
                    numbers_dispid,
                    std::ptr::null(),
                    0,
                    windows_sys::Win32::System::Com::DISPATCH_METHOD as u16,
                    (&mut get_params as *mut windows_sys::Win32::System::Com::DISPPARAMS).cast(),
                    (&mut numbers_result as *mut windows_sys::Win32::System::Variant::VARIANT)
                        .cast(),
                    std::ptr::null_mut(),
                    &mut arg_err,
                ),
                S_OK
            );
            assert_eq!(
                numbers_result.Anonymous.Anonymous.vt,
                windows_sys::Win32::System::Variant::VT_ARRAY
                    | windows_sys::Win32::System::Variant::VT_VARIANT
            );
            let psa = numbers_result.Anonymous.Anonymous.Anonymous.parray;
            let mut lower = i32::MIN;
            let mut upper = i32::MIN;
            assert_eq!(SafeArrayGetLBound(psa.cast_const(), 1, &mut lower), S_OK);
            assert_eq!(SafeArrayGetUBound(psa.cast_const(), 1, &mut upper), S_OK);
            assert_eq!((lower, upper), (0, 2));
            let mut first: windows_sys::Win32::System::Variant::VARIANT = std::mem::zeroed();
            let first_index = 0;
            assert_eq!(
                SafeArrayGetElement(
                    psa.cast_const(),
                    &first_index,
                    (&mut first as *mut windows_sys::Win32::System::Variant::VARIANT).cast()
                ),
                S_OK
            );
            assert_eq!(
                first.Anonymous.Anonymous.vt,
                windows_sys::Win32::System::Variant::VT_I4
            );
            assert_eq!(first.Anonymous.Anonymous.Anonymous.lVal, 2);
            windows_sys::Win32::System::Variant::VariantClear(&mut first);
            windows_sys::Win32::System::Variant::VariantClear(&mut numbers_result);

            let boom_name: Vec<u16> = "Boom".encode_utf16().chain(std::iter::once(0)).collect();
            let names = [boom_name.as_ptr()];
            let mut boom_dispid = i32::MIN;
            assert_eq!(
                ((*dispatch.vtbl).get_ids_of_names)(
                    dispatch_ptr,
                    std::ptr::null(),
                    names.as_ptr(),
                    1,
                    0,
                    &mut boom_dispid,
                ),
                S_OK
            );
            let mut excep: windows_sys::Win32::System::Com::EXCEPINFO = std::mem::zeroed();
            let hr = ((*dispatch.vtbl).invoke)(
                dispatch_ptr,
                boom_dispid,
                std::ptr::null(),
                0,
                windows_sys::Win32::System::Com::DISPATCH_METHOD as u16,
                (&mut get_params as *mut windows_sys::Win32::System::Com::DISPPARAMS).cast(),
                std::ptr::null_mut(),
                (&mut excep as *mut windows_sys::Win32::System::Com::EXCEPINFO).cast(),
                &mut arg_err,
            );
            assert_eq!(hr, oxvba_com::windows_client::COM_DISP_E_EXCEPTION);
            assert!(!excep.bstrDescription.is_null());
            SysFreeString(excep.bstrSource);
            SysFreeString(excep.bstrDescription);

            assert_eq!(((*dispatch.vtbl).release)(dispatch_ptr), 0);
            assert_eq!(((*factory.vtbl).release)(factory_ptr), 0);
            assert_eq!(can_unload(), S_OK);

            assert_eq!(unregister_server(), S_OK);
            assert_eq!(register_server(), S_OK);

            use windows_sys::Win32::System::Com::{
                CLSCTX_INPROC_SERVER, CLSIDFromProgID, COINIT_APARTMENTTHREADED, CoCreateInstance,
                CoInitializeEx, CoUninitialize,
            };
            const RPC_E_CHANGED_MODE: i32 = 0x80010106_u32 as i32;
            let prog_id: Vec<u16> = "TestProj.Widget"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut registered_clsid: windows_sys::core::GUID = std::mem::zeroed();
            assert_eq!(
                CLSIDFromProgID(prog_id.as_ptr(), &mut registered_clsid),
                S_OK
            );
            let coinitialize_hr =
                CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED as u32);
            let should_uninitialize = coinitialize_hr == S_OK;
            assert!(
                coinitialize_hr == S_OK || coinitialize_hr == RPC_E_CHANGED_MODE,
                "CoInitializeEx failed with {coinitialize_hr:#010X}"
            );
            let mut cocreated_dispatch: *mut core::ffi::c_void = std::ptr::null_mut();
            assert_eq!(
                CoCreateInstance(
                    &registered_clsid,
                    std::ptr::null_mut(),
                    CLSCTX_INPROC_SERVER,
                    (&IID_IDISPATCH as *const TestGuid).cast(),
                    &mut cocreated_dispatch,
                ),
                S_OK
            );
            assert!(!cocreated_dispatch.is_null());
            let cocreated = &*(cocreated_dispatch as *const TestComObject);
            let ping_name: Vec<u16> = "Ping".encode_utf16().chain(std::iter::once(0)).collect();
            let names = [ping_name.as_ptr()];
            let mut cocreated_ping_dispid = i32::MIN;
            assert_eq!(
                ((*cocreated.vtbl).get_ids_of_names)(
                    cocreated_dispatch,
                    std::ptr::null(),
                    names.as_ptr(),
                    1,
                    0,
                    &mut cocreated_ping_dispid,
                ),
                S_OK
            );
            let mut cocreated_result: windows_sys::Win32::System::Variant::VARIANT =
                std::mem::zeroed();
            let mut cocreated_params = windows_sys::Win32::System::Com::DISPPARAMS {
                rgvarg: std::ptr::null_mut(),
                rgdispidNamedArgs: std::ptr::null_mut(),
                cArgs: 0,
                cNamedArgs: 0,
            };
            assert_eq!(
                ((*cocreated.vtbl).invoke)(
                    cocreated_dispatch,
                    cocreated_ping_dispid,
                    std::ptr::null(),
                    0,
                    windows_sys::Win32::System::Com::DISPATCH_METHOD as u16,
                    (&mut cocreated_params as *mut windows_sys::Win32::System::Com::DISPPARAMS)
                        .cast(),
                    (&mut cocreated_result as *mut windows_sys::Win32::System::Variant::VARIANT)
                        .cast(),
                    std::ptr::null_mut(),
                    &mut arg_err,
                ),
                S_OK
            );
            assert_eq!(
                cocreated_result.Anonymous.Anonymous.vt,
                windows_sys::Win32::System::Variant::VT_I4
            );
            assert_eq!(cocreated_result.Anonymous.Anonymous.Anonymous.lVal, 7);
            windows_sys::Win32::System::Variant::VariantClear(&mut cocreated_result);
            assert_eq!(((*cocreated.vtbl).release)(cocreated_dispatch), 0);
            if should_uninitialize {
                CoUninitialize();
            }
            assert_eq!(unregister_server(), S_OK);
        }
    }
}
