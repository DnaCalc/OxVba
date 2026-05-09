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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedComServerBuildOutput {
    pub dll_path: PathBuf,
}

#[derive(Debug)]
pub enum WrappedComServerBuildError {
    UnsupportedPlatform { target_os: &'static str },
    Build(BuildError),
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
        }
    }
}

impl From<BuildError> for WrappedComServerBuildError {
    fn from(value: BuildError) -> Self {
        Self::Build(value)
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

    let source = generate_com_server_shim(project_name, oxb_path, classes);
    compile_shim(&source, output_path, ShimOutputType::Dll)?;
    Ok(WrappedComServerBuildOutput {
        dll_path: output_path.to_path_buf(),
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
use std::sync::atomic::{{AtomicI32, Ordering}};

use oxvba_compiler::OxBundle;
use oxvba_host::{{Engine, HostConfig, ProjectRuntimeSession}};
use oxvba_runtime::ObjectRef;

const BUNDLE_BYTES: &[u8] = include_bytes!("{oxb_path}");

static GLOBAL_REF_COUNT: AtomicI32 = AtomicI32::new(0);
static mut H_MODULE: *mut c_void = std::ptr::null_mut();

const S_OK: i32 = 0;
const E_INVALIDARG: i32 = 0x80070057_u32 as i32;
const E_NOINTERFACE: i32 = 0x80004002_u32 as i32;
const E_OUTOFMEMORY: i32 = 0x8007000E_u32 as i32;
const CLASS_E_CLASSNOTAVAILABLE: i32 = 0x80040111_u32 as i32;
const CLASS_E_NOAGGREGATION: i32 = 0x80040110_u32 as i32;

"#
    ));

    // GUID struct and IID constants
    source.push_str(&generate_guid_definitions(project_name, classes));
    source.push_str(&generate_class_table(classes));

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
    source.push_str(&generate_registration_exports(project_name, classes));

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
    classes: &[ComClassExportDescriptor],
) -> String {
    let mut s = String::new();

    s.push_str(
        r#"// ── Registration ──

#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> i32 {
    // Real registration writes HKCR entries for each class.
    // See oxvba_build::registration for the full implementation.
"#,
    );

    for class in classes {
        let default_prog_id = format!("{project_name}.{}", class.class_name);
        let prog_id = class.prog_id.as_deref().unwrap_or(&default_prog_id);
        s.push_str(&format!(
            "    // Register: {} as {prog_id}\n",
            class.class_name
        ));
    }

    s.push_str(
        r#"    S_OK
}

#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> i32 {
    S_OK
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
    engine: Engine,
    session: ProjectRuntimeSession,
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
        let engine = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        });
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
            session,
            object,
        })
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
    // Type info not provided (typelib-based type info can be added in S3)
    0x80004001_u32 as i32 // E_NOTIMPL
}

unsafe extern "system" fn di_get_ids_of_names(
    _this: *mut c_void,
    _riid: *const GUID,
    _rgsznames: *const *const u16,
    _cnames: u32,
    _lcid: u32,
    _rgdispid: *mut i32,
) -> i32 {
    // Future: look up member names from the compiled project's dispatch table
    // and return DISPIDs. For now, return DISP_E_UNKNOWNNAME.
    0x80020006_u32 as i32 // DISP_E_UNKNOWNNAME
}

unsafe extern "system" fn di_invoke(
    _this: *mut c_void,
    _dispid_member: i32,
    _riid: *const GUID,
    _lcid: u32,
    _w_flags: u16,
    _p_disp_params: *mut c_void,
    _p_var_result: *mut c_void,
    _p_excep_info: *mut c_void,
    _pu_arg_err: *mut u32,
) -> i32 {
    // Future: Translate DISPPARAMS → DynamicCallRequest, execute via the
    // OxVba runtime engine session, translate result back to VARIANT.
    // For now, return DISP_E_MEMBERNOTFOUND.
    0x80020003_u32 as i32 // DISP_E_MEMBERNOTFOUND
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

        let source = generate_com_server_shim("TestProj", "test.oxb", &classes);
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

        let source = generate_com_server_shim("TestApp", "test.oxb", &classes);
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

        let source = generate_com_server_shim("TestApp", "test.oxb", &classes);
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

        let source = generate_com_server_shim("Multi", "test.oxb", &classes);
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
            members: vec![],
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
                source: "Private stored As Long\nPrivate Sub Class_Initialize()\nstored = 7\nEnd Sub\nPublic Function Ping() As Long\nPing = stored\nEnd Function\n"
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
        let classes = vec![ComClassExportDescriptor {
            class_name: "Widget".to_string(),
            prog_id: Some("TestProj.Widget".to_string()),
            instancing: Some(Instancing::MultiUse),
            description: None,
            members: vec![],
        }];

        let output =
            compile_wrapped_com_server_shim("TestProj", &bundle_literal, &classes, &dll_path)
                .expect("WrappedComServer DLL build should succeed");
        assert_eq!(output.dll_path, dll_path);
        assert!(output.dll_path.exists());

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
            use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

            #[repr(C)]
            #[derive(Clone, Copy, PartialEq, Eq)]
            struct TestGuid {
                data1: u32,
                data2: u16,
                data3: u16,
                data4: [u8; 8],
            }

            #[repr(C)]
            struct TestUnknownVtbl {
                query_interface: unsafe extern "system" fn(
                    *mut core::ffi::c_void,
                    *const TestGuid,
                    *mut *mut core::ffi::c_void,
                ) -> i32,
                add_ref: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
                release: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
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
                vtbl: *const TestUnknownVtbl,
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
            assert_eq!(((*dispatch.vtbl).release)(dispatch_ptr), 0);
            assert_eq!(((*factory.vtbl).release)(factory_ptr), 0);
            assert_eq!(can_unload(), S_OK);
        }
    }
}
