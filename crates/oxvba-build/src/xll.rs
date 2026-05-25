//! XLL entry point and function registration code generation.

use oxvba_project::NativeExportDescriptor;

use crate::xloper;

/// Generate Rust source code for an XLL add-in shim.
pub fn generate_xll_shim(
    project_name: &str,
    oxb_path: &str,
    exports: &[NativeExportDescriptor],
) -> String {
    let mut source = format!(
        r#"//! Auto-generated OxVBA XLL add-in for project "{project_name}".

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::io::Write;
use std::sync::OnceLock;

use oxvba_compiler::{{DeclareParamType, OxBundle}};
use oxvba_host::{{Engine, HostConfig, ProjectRuntimeSession}};
use oxvba_runtime::{{bstr::BStr, safe_array::{{SafeArray, SafeArrayBound}}, Variant}};

const BUNDLE_BYTES: &[u8] = include_bytes!("{oxb_path}");

type XChar = u16;
type Bool32 = i32;
type Rw = i32;
type Col = i32;
type IdSheet = usize;
type Word = u16;
type DWord = u32;
type Byte = u8;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XLREF12 {{
    pub rw_first: Rw,
    pub rw_last: Rw,
    pub col_first: Col,
    pub col_last: Col,
}}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XLSREF12 {{
    pub count: Word,
    pub ref_: XLREF12,
}}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XLMREF12 {{
    pub count: Word,
    pub reftbl: [XLREF12; 1],
}}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XLMREF12Value {{
    pub lpmref: *mut XLMREF12,
    pub id_sheet: IdSheet,
}}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XLArray12 {{
    pub lparray: *mut XLOPER12,
    pub rows: Rw,
    pub columns: Col,
}}

#[repr(C)]
#[derive(Clone, Copy)]
pub union XLFlowValue {{
    pub level: i32,
    pub tbctrl: i32,
    pub id_sheet: IdSheet,
}}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XLFlow12 {{
    pub valflow: XLFlowValue,
    pub rw: Rw,
    pub col: Col,
    pub xlflow: Byte,
}}

#[repr(C)]
#[derive(Clone, Copy)]
pub union XLBigDataHandle {{
    pub lpb_data: *mut Byte,
    pub hdata: *mut std::ffi::c_void,
}}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XLBigData12 {{
    pub h: XLBigDataHandle,
    pub cb_data: i32,
}}

#[repr(C)]
#[derive(Clone, Copy)]
pub union Xloper12Value {{
    pub num: f64,
    pub str_value: *mut XChar,
    pub xbool: Bool32,
    pub err: i32,
    pub w: i32,
    pub sref: XLSREF12,
    pub mref: XLMREF12Value,
    pub array: XLArray12,
    pub flow: XLFlow12,
    pub bigdata: XLBigData12,
}}

#[repr(C)]
pub struct XLOPER12 {{
    pub val: Xloper12Value,
    pub xltype: DWord,
}}

struct XllRegistration {{
    procedure: &'static str,
    type_text: &'static str,
    function_text: &'static str,
    argument_text: &'static str,
    category: &'static str,
    function_help: &'static str,
}}

const XLF_REGISTER: i32 = 149;
const XL_GET_HWND: i32 = 0x4008;
const XL_GET_NAME: i32 = 0x4009;
const XL_FREE: i32 = 0x4000;
const XL_TYPE_NUM: u32 = 0x0001;
const XL_TYPE_STR: u32 = 0x0002;
const XL_TYPE_BOOL: u32 = 0x0004;
const XL_TYPE_ERR: u32 = 0x0010;
const XL_TYPE_MULTI: u32 = 0x0040;
const XL_TYPE_MISSING: u32 = 0x0080;
const XL_TYPE_NIL: u32 = 0x0100;
const XL_TYPE_INT: u32 = 0x0800;
const XL_BIT_XL_FREE: u32 = 0x1000;
const XL_BIT_DLL_FREE: u32 = 0x4000;

thread_local! {{
    static SESSION: RefCell<Option<(Engine, ProjectRuntimeSession, bool)>> = RefCell::new(None);
}}

fn with_session<F, R>(f: F) -> R
where
    F: FnOnce(&Engine, &mut ProjectRuntimeSession) -> R,
{{
    SESSION.with(|cell| {{
        let mut slot = cell.borrow_mut();
        if slot.is_none() {{
            let bundle = OxBundle::deserialize_from_bytes(BUNDLE_BYTES)
                .expect("failed to deserialize embedded bundle");
            let mut engine = Engine::new(HostConfig {{
                enable_jit: false,
            }});
            let mut host_policy = engine.host_policy().clone();
            host_policy.deterministic_mode = false;
            engine.set_host_policy(host_policy);
            let application_bound = try_bind_excel_application_root(&engine);
            let session = engine
                .compile_and_prepare_session_from_bundle(&bundle)
                .expect("failed to prepare XLL session from bundle");
            *slot = Some((engine, session, application_bound));
        }}
        let (engine, session, application_bound) = slot.as_mut().expect("XLL session initialized");
        if !*application_bound {{
            *application_bound = try_bind_excel_application_root(engine);
        }}
        f(engine, session)
    }})
}}

#[cfg(target_os = "windows")]
fn try_bind_excel_application_root(engine: &Engine) -> bool {{
    match acquire_host_excel_application_dispatch() {{
        Ok(dispatch) => {{
            let result = unsafe {{
                engine.bind_native_dispatch_object(
                    "Excel.Application",
                    dispatch.cast::<std::ffi::c_void>(),
                )
            }};
            match result {{
                Ok(object) => {{
                    trace_xll_event(&format!(
                        "Excel.Application host root bound object={{}}",
                        object.raw()
                    ));
                    true
                }}
                Err(err) => {{
                    trace_xll_event(&format!(
                        "Excel.Application host root bind failed {{}}",
                        err
                    ));
                    false
                }}
            }}
        }}
        Err(err) => {{
            trace_xll_event(&format!(
                "Excel.Application host root unavailable {{}}",
                err
            ));
            false
        }}
    }}
}}

#[cfg(not(target_os = "windows"))]
fn try_bind_excel_application_root(_engine: &Engine) -> bool {{
    trace_xll_event("Excel.Application host root unavailable non-windows");
    false
}}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HostExcelIdentity {{
    hwnd: isize,
    pid: u32,
}}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExcelApplicationCandidate {{
    dispatch: *mut oxvba_com::RawIDispatch,
    hwnd: isize,
    pid: u32,
}}

#[cfg(target_os = "windows")]
fn acquire_host_excel_application_dispatch() -> Result<*mut oxvba_com::RawIDispatch, String> {{
    use windows_sys::Win32::System::Com::{{COINIT_APARTMENTTHREADED, CoInitializeEx}};

    const RPC_E_CHANGED_MODE: i32 = 0x80010106u32 as i32;

    let hr = unsafe {{ CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) }};
    if hr < 0 && hr != RPC_E_CHANGED_MODE {{
            return Err(format!("CoInitializeEx HRESULT={{:#010X}}", hr as u32));
    }}

    let host = host_excel_identity()?;
    trace_xll_event(&format!(
        "Excel.Application host identity hwnd={{}} pid={{}}",
        host.hwnd, host.pid
    ));
    let candidates = enumerate_rot_excel_application_candidates()?;
    trace_xll_event(&format!(
        "Excel.Application ROT candidate count={{}}",
        candidates.len()
    ));
    select_host_excel_candidate(host, candidates)
}}

#[cfg(target_os = "windows")]
fn host_excel_identity() -> Result<HostExcelIdentity, String> {{
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let mut result = XLOPER12 {{ val: Xloper12Value {{ w: 0 }}, xltype: 0 }};
    let status = excel12v(XL_GET_HWND, &mut result, 0, std::ptr::null_mut());
    if status != 0 {{
        return Err(format!("xlGetHwnd status={{}}", status));
    }}
    if result.xltype & XL_TYPE_INT == 0 {{
        return Err(format!("xlGetHwnd returned xltype={{:#X}}", result.xltype));
    }}
    let hwnd = unsafe {{ result.val.w }} as isize;
    if hwnd == 0 {{
        return Err("xlGetHwnd returned null HWND".to_string());
    }}

    let mut hwnd_pid = 0u32;
    let thread_id = unsafe {{
        GetWindowThreadProcessId(hwnd as usize as *mut std::ffi::c_void, &mut hwnd_pid)
    }};
    if thread_id == 0 || hwnd_pid == 0 {{
        return Err(format!("GetWindowThreadProcessId failed for host hwnd={{}}", hwnd));
    }}
    let current_pid = unsafe {{ GetCurrentProcessId() }};
    if current_pid != hwnd_pid {{
        return Err(format!(
            "host HWND pid mismatch current_pid={{}} hwnd_pid={{}} hwnd={{}}",
            current_pid, hwnd_pid, hwnd
        ));
    }}
    Ok(HostExcelIdentity {{ hwnd, pid: current_pid }})
}}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIRunningObjectTable {{
    vtbl: *const RawIRunningObjectTableVtbl,
}}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIRunningObjectTableVtbl {{
    unknown: oxvba_com::RawIUnknownVtbl,
    register: unsafe extern "system" fn(
        this: *mut std::ffi::c_void,
        grf_flags: u32,
        punk_object: *mut std::ffi::c_void,
        pmk_object_name: *mut std::ffi::c_void,
        pdw_register: *mut u32,
    ) -> i32,
    revoke: unsafe extern "system" fn(this: *mut std::ffi::c_void, dw_register: u32) -> i32,
    is_running: unsafe extern "system" fn(
        this: *mut std::ffi::c_void,
        pmk_object_name: *mut std::ffi::c_void,
    ) -> i32,
    get_object: unsafe extern "system" fn(
        this: *mut std::ffi::c_void,
        pmk_object_name: *mut std::ffi::c_void,
        ppunk_object: *mut *mut std::ffi::c_void,
    ) -> i32,
    note_change_time: unsafe extern "system" fn(
        this: *mut std::ffi::c_void,
        dw_register: u32,
        pfiletime: *mut std::ffi::c_void,
    ) -> i32,
    get_time_of_last_change: unsafe extern "system" fn(
        this: *mut std::ffi::c_void,
        pmk_object_name: *mut std::ffi::c_void,
        pfiletime: *mut std::ffi::c_void,
    ) -> i32,
    enum_running: unsafe extern "system" fn(
        this: *mut std::ffi::c_void,
        ppenum_moniker: *mut *mut std::ffi::c_void,
    ) -> i32,
}}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIEnumMoniker {{
    vtbl: *const RawIEnumMonikerVtbl,
}}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIEnumMonikerVtbl {{
    unknown: oxvba_com::RawIUnknownVtbl,
    next: unsafe extern "system" fn(
        this: *mut std::ffi::c_void,
        celt: u32,
        rgelt: *mut *mut std::ffi::c_void,
        pcelt_fetched: *mut u32,
    ) -> i32,
    skip: unsafe extern "system" fn(this: *mut std::ffi::c_void, celt: u32) -> i32,
    reset: unsafe extern "system" fn(this: *mut std::ffi::c_void) -> i32,
    clone: unsafe extern "system" fn(
        this: *mut std::ffi::c_void,
        ppenum: *mut *mut std::ffi::c_void,
    ) -> i32,
}}

#[cfg(target_os = "windows")]
fn enumerate_rot_excel_application_candidates() -> Result<Vec<ExcelApplicationCandidate>, String> {{
    use windows_sys::Win32::System::Com::GetRunningObjectTable;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let mut rot_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = unsafe {{ GetRunningObjectTable(0, &mut rot_ptr) }};
    if hr < 0 || rot_ptr.is_null() {{
        return Err(format!("GetRunningObjectTable HRESULT={{:#010X}}", hr as u32));
    }}
    let rot = rot_ptr.cast::<RawIRunningObjectTable>();
    let mut enum_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = unsafe {{ ((*(*rot).vtbl).enum_running)(rot_ptr, &mut enum_ptr) }};
    if hr < 0 || enum_ptr.is_null() {{
        unsafe {{ oxvba_com::release_unknown(rot_ptr) }};
        return Err(format!("IRunningObjectTable::EnumRunning HRESULT={{:#010X}}", hr as u32));
    }}

    let enum_moniker = enum_ptr.cast::<RawIEnumMoniker>();
    let mut candidates = Vec::new();
    loop {{
        let mut moniker: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut fetched = 0u32;
        let hr = unsafe {{ ((*(*enum_moniker).vtbl).next)(enum_ptr, 1, &mut moniker, &mut fetched) }};
        if hr < 0 {{
            unsafe {{
                oxvba_com::release_unknown(enum_ptr);
                oxvba_com::release_unknown(rot_ptr);
            }}
            release_excel_candidates(candidates);
            return Err(format!("IEnumMoniker::Next HRESULT={{:#010X}}", hr as u32));
        }}
        if fetched == 0 || moniker.is_null() {{
            break;
        }}

        let mut unknown: *mut std::ffi::c_void = std::ptr::null_mut();
        let get_hr = unsafe {{ ((*(*rot).vtbl).get_object)(rot_ptr, moniker, &mut unknown) }};
        unsafe {{ oxvba_com::release_unknown(moniker) }};
        if get_hr < 0 || unknown.is_null() {{
            continue;
        }}

        let dispatch = match unsafe {{
            oxvba_com::query_dispatch_from_unknown(unknown.cast::<oxvba_com::RawIUnknown>())
        }} {{
            Ok(dispatch) => dispatch,
            Err(_) => {{
                unsafe {{ oxvba_com::release_unknown(unknown) }};
                continue;
            }}
        }};
        unsafe {{ oxvba_com::release_unknown(unknown) }};

        let app_dispatch = match excel_application_dispatch_from_candidate(dispatch) {{
            Ok(app_dispatch) => app_dispatch,
            Err(_) => continue,
        }};
        let hwnd = match unsafe {{ dispatch_property_get_i64(app_dispatch, "Hwnd") }} {{
            Ok(hwnd) => hwnd as isize,
            Err(_) => {{
                unsafe {{ oxvba_com::release_dispatch(app_dispatch) }};
                continue;
            }}
        }};
        if hwnd == 0 {{
            unsafe {{ oxvba_com::release_dispatch(app_dispatch) }};
            continue;
        }}
        let mut pid = 0u32;
        let thread_id = unsafe {{
            GetWindowThreadProcessId(hwnd as usize as *mut std::ffi::c_void, &mut pid)
        }};
        if thread_id == 0 || pid == 0 {{
            unsafe {{ oxvba_com::release_dispatch(app_dispatch) }};
            continue;
        }}
        candidates.push(ExcelApplicationCandidate {{
            dispatch: app_dispatch,
            hwnd,
            pid,
        }});
    }}

    unsafe {{
        oxvba_com::release_unknown(enum_ptr);
        oxvba_com::release_unknown(rot_ptr);
    }}
    Ok(candidates)
}}

#[cfg(target_os = "windows")]
fn excel_application_dispatch_from_candidate(
    dispatch: *mut oxvba_com::RawIDispatch,
) -> Result<*mut oxvba_com::RawIDispatch, String> {{
    if unsafe {{ dispatch_property_get_i64(dispatch, "Hwnd") }}.is_ok() {{
        return Ok(dispatch);
    }}
    let app_dispatch = match unsafe {{ dispatch_property_get_dispatch(dispatch, "Application") }} {{
        Ok(app_dispatch) => app_dispatch,
        Err(err) => {{
            unsafe {{ oxvba_com::release_dispatch(dispatch) }};
            return Err(err);
        }}
    }};
    unsafe {{ oxvba_com::release_dispatch(dispatch) }};
    if unsafe {{ dispatch_property_get_i64(app_dispatch, "Hwnd") }}.is_err() {{
        unsafe {{ oxvba_com::release_dispatch(app_dispatch) }};
        return Err("candidate Application object has no Hwnd".to_string());
    }}
    Ok(app_dispatch)
}}

#[cfg(target_os = "windows")]
fn select_host_excel_candidate(
    host: HostExcelIdentity,
    candidates: Vec<ExcelApplicationCandidate>,
) -> Result<*mut oxvba_com::RawIDispatch, String> {{
    let mut selected: Option<ExcelApplicationCandidate> = None;
    let mut duplicate_matches = 0usize;
    let mut pid_only_matches = Vec::new();
    for candidate in candidates {{
        let hwnd_match = candidate.hwnd == host.hwnd;
        let pid_match = candidate.pid == host.pid;
        let is_match = hwnd_match || pid_match;
        trace_xll_event(&format!(
            "Excel.Application ROT candidate hwnd={{}} pid={{}} match={{}}",
            candidate.hwnd, candidate.pid, is_match
        ));
        if !is_match {{
            unsafe {{ oxvba_com::release_dispatch(candidate.dispatch) }};
        }} else if hwnd_match {{
            if selected.is_none() {{
                selected = Some(candidate);
            }} else {{
                duplicate_matches += 1;
                unsafe {{ oxvba_com::release_dispatch(candidate.dispatch) }};
            }}
        }} else {{
            pid_only_matches.push(candidate);
        }}
    }}
    if let Some(candidate) = selected {{
        for candidate in pid_only_matches {{
            duplicate_matches += 1;
            unsafe {{ oxvba_com::release_dispatch(candidate.dispatch) }};
        }}
        if duplicate_matches > 0 {{
            trace_xll_event(&format!(
                "Excel.Application duplicate host ROT candidates collapsed count={{}}",
                duplicate_matches
            ));
        }}
        return Ok(candidate.dispatch);
    }}

    match pid_only_matches.len() {{
        1 => Ok(pid_only_matches.remove(0).dispatch),
        0 => Err(format!(
            "no ROT Excel.Application candidate matched host hwnd={{}} pid={{}}",
            host.hwnd, host.pid
        )),
        count => {{
            for candidate in pid_only_matches {{
                unsafe {{ oxvba_com::release_dispatch(candidate.dispatch) }};
            }}
            Err(format!(
                "ambiguous pid-only ROT Excel.Application candidates matched host hwnd={{}} pid={{}} count={{}}",
                host.hwnd, host.pid, count
            ))
        }}
    }}
}}

#[cfg(target_os = "windows")]
fn release_excel_candidates(candidates: Vec<ExcelApplicationCandidate>) {{
    for candidate in candidates {{
        unsafe {{ oxvba_com::release_dispatch(candidate.dispatch) }};
    }}
}}

#[cfg(target_os = "windows")]
unsafe fn dispatch_property_get_i64(
    dispatch: *mut oxvba_com::RawIDispatch,
    name: &str,
) -> Result<i64, String> {{
    use windows_sys::Win32::System::Com::{{DISPATCH_PROPERTYGET, DISPPARAMS}};
    use windows_sys::Win32::System::Variant::{{VARIANT, VT_I4, VT_I8, VT_INT, VariantClear}};

    let dispid = oxvba_com::get_dispid_by_name(dispatch, name)?;
    let mut params = DISPPARAMS {{
        rgvarg: std::ptr::null_mut(),
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 0,
        cNamedArgs: 0,
    }};
    let mut result: VARIANT = std::mem::zeroed();
    let mut arg_err = 0u32;
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &oxvba_com::IID_NULL,
        0x0400,
        DISPATCH_PROPERTYGET,
        &mut params,
        &mut result,
        std::ptr::null_mut(),
        &mut arg_err,
    );
    if hr < 0 {{
        return Err(format!(
            "IDispatch::Invoke({{}} PropertyGet) HRESULT={{:#010X}}",
            name, hr as u32
        ));
    }}
    let vt = result.Anonymous.Anonymous.vt;
    let value = match vt {{
        VT_I4 => result.Anonymous.Anonymous.Anonymous.lVal as i64,
        VT_I8 => result.Anonymous.Anonymous.Anonymous.llVal,
        VT_INT => result.Anonymous.Anonymous.Anonymous.intVal as i64,
        other => {{
            let _ = VariantClear(&mut result);
            return Err(format!("{{}} returned unsupported vt={{}}", name, other));
        }}
    }};
    let _ = VariantClear(&mut result);
    Ok(value)
}}

#[cfg(target_os = "windows")]
unsafe fn dispatch_property_get_dispatch(
    dispatch: *mut oxvba_com::RawIDispatch,
    name: &str,
) -> Result<*mut oxvba_com::RawIDispatch, String> {{
    use windows_sys::Win32::System::Com::{{DISPATCH_PROPERTYGET, DISPPARAMS}};
    use windows_sys::Win32::System::Variant::{{
        VARIANT, VT_DISPATCH, VT_UNKNOWN, VariantClear,
    }};

    let dispid = oxvba_com::get_dispid_by_name(dispatch, name)?;
    let mut params = DISPPARAMS {{
        rgvarg: std::ptr::null_mut(),
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 0,
        cNamedArgs: 0,
    }};
    let mut result: VARIANT = std::mem::zeroed();
    let mut arg_err = 0u32;
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &oxvba_com::IID_NULL,
        0x0400,
        DISPATCH_PROPERTYGET,
        &mut params,
        &mut result,
        std::ptr::null_mut(),
        &mut arg_err,
    );
    if hr < 0 {{
        return Err(format!(
            "IDispatch::Invoke({{}} PropertyGet) HRESULT={{:#010X}}",
            name, hr as u32
        ));
    }}

    let vt = result.Anonymous.Anonymous.vt;
    let dispatch_result = match vt {{
        VT_DISPATCH => {{
            let app = result.Anonymous.Anonymous.Anonymous.pdispVal.cast::<oxvba_com::RawIDispatch>();
            if app.is_null() {{
                Err(format!("{{}} returned null IDispatch", name))
            }} else {{
                oxvba_com::add_ref_dispatch(app);
                Ok(app)
            }}
        }}
        VT_UNKNOWN => {{
            let unknown = result.Anonymous.Anonymous.Anonymous.punkVal.cast::<oxvba_com::RawIUnknown>();
            oxvba_com::query_dispatch_from_unknown(unknown)
        }}
        other => Err(format!("{{}} returned unsupported object vt={{}}", name, other)),
    }};
    let _ = VariantClear(&mut result);
    dispatch_result
}}

#[cfg(target_os = "windows")]
fn wide_null(text: &str) -> Vec<u16> {{
    text.encode_utf16().chain(std::iter::once(0)).collect()
}}

"#
    );

    source.push_str(&generate_registration_table(project_name, exports));

    for export in exports {
        source.push_str(&generate_export_wrapper(export));
    }

    // xlAutoOpen
    source.push_str(&generate_xl_auto_open());

    // xlAutoClose
    source.push_str(
        r#"
#[unsafe(no_mangle)]
pub extern "system" fn xlAutoClose() -> i32 {
    // Cleanup: unregister functions
    1
}

"#,
    );

    // xlAutoFree12
    source.push_str(
        r#"#[unsafe(no_mangle)]
pub extern "system" fn xlAutoFree12(p: *mut XLOPER12) {
    // Free XLOPER12 memory allocated by the add-in
    if !p.is_null() {
        unsafe { drop(Box::from_raw(p.cast::<XllOwnedXloper12>())); }
    }
}

"#,
    );

    source.push_str(generate_xll_runtime_helpers());

    source
}

fn generate_registration_table(project_name: &str, exports: &[NativeExportDescriptor]) -> String {
    let mut source = String::from("const REGISTRATIONS: &[XllRegistration] = &[\n");

    for export in exports {
        let param_types = export.param_types.as_deref().unwrap_or(&[]);
        let return_type = export.return_type.as_ref().and_then(|r| r.as_ref());
        let type_string = xloper::build_type_string(param_types, return_type);
        let category = export.category.as_deref().unwrap_or(project_name);
        let function_help = export.description.as_deref().unwrap_or("");
        let argument_text = export.argument_descriptions.as_deref().unwrap_or("");

        source.push_str(&format!(
            "    XllRegistration {{ procedure: {}, type_text: {}, function_text: {}, argument_text: {}, category: {}, function_help: {} }},\n",
            rust_string_literal(&export.exported_name),
            rust_string_literal(&type_string),
            rust_string_literal(&export.exported_name),
            rust_string_literal(argument_text),
            rust_string_literal(category),
            rust_string_literal(function_help),
        ));
    }

    source.push_str("];\n\n");
    source
}

fn generate_xl_auto_open() -> String {
    String::from(
        r#"#[unsafe(no_mangle)]
pub extern "system" fn xlAutoOpen() -> i32 {
    trace_xll_event("xlAutoOpen start");
    let mut module_text = XLOPER12 { val: Xloper12Value { w: 0 }, xltype: 0 };
    if !initialize_xll_module_text(&mut module_text) {
        trace_xll_event("xlAutoOpen failed xlGetName");
        return 0;
    }
    for registration in REGISTRATIONS {
        if !register_xll_function(&mut module_text, registration) {
            trace_xll_event(&format!("xlAutoOpen failed procedure={}", registration.procedure));
            free_xll_oper(&mut module_text);
            return 0;
        }
    }
    free_xll_oper(&mut module_text);
    trace_xll_event("xlAutoOpen complete");
    1
}

fn trace_xll_event(message: &str) {
    let Ok(path) = std::env::var("OXVBA_XLL_TRACE") else {
        return;
    };
    if path.trim().is_empty() {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
    }
}

#[cfg(target_os = "windows")]
fn initialize_xll_module_text(module_text: &mut XLOPER12) -> bool {
    let get_name_status = excel12v(XL_GET_NAME, module_text, 0, std::ptr::null_mut());
    if get_name_status != 0 {
        trace_xll_event(&format!(
            "xlGetName status={} success=false",
            get_name_status
        ));
        return false;
    }
    trace_xll_event("xlGetName status=0 success=true");
    true
}

#[cfg(target_os = "windows")]
fn register_xll_function(module_text: &mut XLOPER12, registration: &XllRegistration) -> bool {
    let mut result = XLOPER12 { val: Xloper12Value { w: 0 }, xltype: 0 };
    let mut procedure = xll_string(registration.procedure);
    let mut type_text = xll_string(registration.type_text);
    let mut function_text = xll_string(registration.function_text);
    let mut argument_text = xll_string(registration.argument_text);
    let mut macro_type = xll_string("1");
    let mut category = xll_string(registration.category);
    let mut function_help = xll_string(registration.function_help);
    let mut missing_args: [XLOPER12; 24] = std::array::from_fn(|_| XLOPER12 {
        val: Xloper12Value { w: 0 },
        xltype: XL_TYPE_MISSING,
    });
    let mut args = vec![
        module_text as *mut XLOPER12,
        &mut procedure.oper as *mut XLOPER12,
        &mut type_text.oper as *mut XLOPER12,
        &mut function_text.oper as *mut XLOPER12,
        &mut argument_text.oper as *mut XLOPER12,
        &mut macro_type.oper as *mut XLOPER12,
        &mut category.oper as *mut XLOPER12,
        &mut missing_args[0] as *mut XLOPER12,
        &mut missing_args[1] as *mut XLOPER12,
        &mut function_help.oper as *mut XLOPER12,
    ];
    for missing in &mut missing_args[2..] {
        args.push(missing as *mut XLOPER12);
    }
    let status = excel12v(XLF_REGISTER, &mut result, args.len() as i32, args.as_mut_ptr());
    free_xll_oper(&mut result);
    let success = status == 0;
    trace_xll_event(&format!(
        "xlfRegister procedure={} type_text={} status={} success={}",
        registration.procedure, registration.type_text, status, success
    ));
    success
}

#[cfg(target_os = "windows")]
fn free_xll_oper(oper: &mut XLOPER12) {
    let _ = oper;
}

#[cfg(target_os = "windows")]
type Excel12vFn =
    unsafe extern "system" fn(i32, i32, *mut *mut XLOPER12, *mut XLOPER12) -> i32;

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleA(module_name: *const i8) -> *mut std::ffi::c_void;
    fn GetProcAddress(
        module: *mut std::ffi::c_void,
        proc_name: *const i8,
    ) -> *mut std::ffi::c_void;
    fn LoadLibraryA(module_name: *const i8) -> *mut std::ffi::c_void;
}

#[cfg(target_os = "windows")]
fn excel12v(
    xlfn: i32,
    oper_result: *mut XLOPER12,
    count: i32,
    opers: *mut *mut XLOPER12,
) -> i32 {
    static EXCEL12V: OnceLock<Option<Excel12vFn>> = OnceLock::new();
    let Some(function) = *EXCEL12V.get_or_init(resolve_excel12v) else {
        return -1;
    };
    unsafe { function(xlfn, count, opers, oper_result) }
}

#[cfg(target_os = "windows")]
fn resolve_excel12v() -> Option<Excel12vFn> {
    let excel_module = unsafe { GetModuleHandleA(b"EXCEL.EXE\0".as_ptr().cast()) };
    if !excel_module.is_null() {
        let proc = unsafe { GetProcAddress(excel_module, b"MdCallBack12\0".as_ptr().cast()) };
        trace_xll_event(&format!(
            "resolve_excel12v module=EXCEL.EXE symbol=MdCallBack12 found={}",
            !proc.is_null()
        ));
        if !proc.is_null() {
            return Some(unsafe { std::mem::transmute::<*mut std::ffi::c_void, Excel12vFn>(proc) });
        }
    } else {
        trace_xll_event("resolve_excel12v module=EXCEL.EXE found=false");
    }

    let xlcall_module = unsafe { LoadLibraryA(b"XLCALL32.DLL\0".as_ptr().cast()) };
    trace_xll_event(&format!(
        "resolve_excel12v module=XLCALL32.DLL loaded={}",
        !xlcall_module.is_null()
    ));
    None
}

#[cfg(not(target_os = "windows"))]
fn initialize_xll_module_text(_module_text: &mut XLOPER12) -> bool {
    trace_xll_event("xlGetName status=stub success=true");
    true
}

#[cfg(not(target_os = "windows"))]
fn register_xll_function(_module_text: &mut XLOPER12, registration: &XllRegistration) -> bool {
    trace_xll_event(&format!(
        "xlfRegister procedure={} type_text={} status=stub success=true",
        registration.procedure, registration.type_text
    ));
    true
}

#[cfg(not(target_os = "windows"))]
fn free_xll_oper(_oper: &mut XLOPER12) {}

struct XllString {
    oper: XLOPER12,
    _wide: Box<[u16]>,
}

fn xll_string(text: &str) -> XllString {
    let mut wide = counted_wide_string(text);
    let ptr = wide.as_mut_ptr();
    XllString {
        oper: XLOPER12 {
            val: Xloper12Value { str_value: ptr },
            xltype: XL_TYPE_STR,
        },
        _wide: wide,
    }
}

"#,
    )
}

fn generate_export_wrapper(export: &NativeExportDescriptor) -> String {
    let name = &export.exported_name;
    let params = export.param_types.as_deref().unwrap_or(&[]);
    let module = &export.module_name;
    let procedure = &export.procedure_name;

    let signature_params = params
        .iter()
        .enumerate()
        .map(|(i, _)| format!("arg{i}: *const XLOPER12"))
        .collect::<Vec<_>>()
        .join(", ");
    let marshal_args = params
        .iter()
        .enumerate()
        .map(|(i, ty)| format!("        xll_arg_to_variant(arg{i}, DeclareParamType::{ty:?})"))
        .collect::<Vec<_>>()
        .join(",\n");

    format!(
        r#"#[unsafe(no_mangle)]
pub extern "system" fn {name}({signature_params}) -> *mut XLOPER12 {{
    let args: Vec<Variant> = vec![
{marshal_args}
    ];
    let result = with_session(|engine, session| {{
        engine
            .invoke_procedure_with_variants(session, "{module}", "{procedure}", &args)
            .expect("XLL procedure invocation failed")
    }});
    variant_to_xll(result)
}}

"#
    )
}

fn generate_xll_runtime_helpers() -> &'static str {
    r#"#[repr(C)]
struct XllOwnedXloper12 {
    oper: XLOPER12,
    _wide: Option<Box<[u16]>>,
    _array: Option<Box<[XLOPER12]>>,
    _array_wide: Vec<Box<[u16]>>,
}

fn xll_arg_to_variant(arg: *const XLOPER12, ty: DeclareParamType) -> Variant {
    if arg.is_null() {
        return Variant::empty();
    }
    let value = unsafe { &*arg };
    let xltype = base_xltype(value.xltype);
    match ty {
        DeclareParamType::Double => Variant::from_f64(xll_to_f64(value, xltype)),
        DeclareParamType::Date => Variant::from_date_f64(xll_to_f64(value, xltype)),
        DeclareParamType::Single => Variant::from_f32(xll_to_f64(value, xltype) as f32),
        DeclareParamType::Boolean => Variant::from_bool(xll_to_bool(value, xltype)),
        DeclareParamType::LongLong | DeclareParamType::LongPtr => {
            Variant::from_i64(xll_to_i64(value, xltype))
        }
        DeclareParamType::String => {
            if xltype == XL_TYPE_STR {
                Variant::from_string(BStr::from(read_counted_wide_string(unsafe {
                    value.val.str_value
                })))
            } else {
                Variant::from_string(BStr::from(""))
            }
        }
        DeclareParamType::Currency => {
            Variant::from_currency_scaled_i64((xll_to_f64(value, xltype) * 10_000.0).round() as i64)
        }
        DeclareParamType::Byte
        | DeclareParamType::Integer
        | DeclareParamType::Long => Variant::from_i32(xll_to_i64(value, xltype) as i32),
        DeclareParamType::Variant | DeclareParamType::Any => xll_to_variant(value, xltype),
    }
}

fn variant_to_xll(value: Variant) -> *mut XLOPER12 {
    if let Some(value) = value.as_i16() {
        boxed_xll_oper(xll_int(value as i32, XL_BIT_DLL_FREE), None, None, Vec::new())
    } else if let Some(value) = value.as_i32() {
        boxed_xll_oper(xll_int(value, XL_BIT_DLL_FREE), None, None, Vec::new())
    } else if let Some(value) = value.as_i64() {
        boxed_xll_oper(xll_int(value as i32, XL_BIT_DLL_FREE), None, None, Vec::new())
    } else if let Some(value) = value.as_u8() {
        boxed_xll_oper(xll_int(value as i32, XL_BIT_DLL_FREE), None, None, Vec::new())
    } else if let Some(value) = value.as_date_f64() {
        boxed_xll_oper(xll_num(value, XL_BIT_DLL_FREE), None, None, Vec::new())
    } else if let Some(value) = value.as_currency_scaled_i64() {
        boxed_xll_oper(
            xll_num(value as f64 / 10_000.0, XL_BIT_DLL_FREE),
            None,
            None,
            Vec::new(),
        )
    } else if let Some(value) = value.as_f64() {
        boxed_xll_oper(xll_num(value, XL_BIT_DLL_FREE), None, None, Vec::new())
    } else if let Some(value) = value.as_f32() {
        boxed_xll_oper(
            xll_num(value as f64, XL_BIT_DLL_FREE),
            None,
            None,
            Vec::new(),
        )
    } else if let Some(value) = value.as_bool() {
        boxed_xll_oper(xll_bool(value, XL_BIT_DLL_FREE), None, None, Vec::new())
    } else if let Some(value) = value.as_error_code() {
        boxed_xll_oper(xll_err(value, XL_BIT_DLL_FREE), None, None, Vec::new())
    } else if let Some(text) = value.as_bstr() {
        let rendered = text.as_str();
        let mut wide = counted_wide_string(&rendered);
        let ptr = wide.as_mut_ptr();
        boxed_xll_oper(
            XLOPER12 {
                val: Xloper12Value { str_value: ptr },
                xltype: XL_TYPE_STR | XL_BIT_DLL_FREE,
            },
            Some(wide),
            None,
            Vec::new(),
        )
    } else if let Some(array) = value.as_safearray() {
        safe_array_to_xll_multi(array)
    } else {
        boxed_xll_oper(xll_nil(XL_BIT_DLL_FREE), None, None, Vec::new())
    }
}

fn base_xltype(xltype: u32) -> u32 {
    xltype & !(XL_BIT_XL_FREE | XL_BIT_DLL_FREE)
}

fn xll_to_variant(value: &XLOPER12, xltype: u32) -> Variant {
    match xltype {
        XL_TYPE_NUM => Variant::from_f64(unsafe { value.val.num }),
        XL_TYPE_STR => Variant::from_string(BStr::from(read_counted_wide_string(unsafe {
            value.val.str_value
        }))),
        XL_TYPE_BOOL => Variant::from_bool(unsafe { value.val.xbool } != 0),
        XL_TYPE_INT => Variant::from_i32(unsafe { value.val.w }),
        XL_TYPE_ERR => Variant::from_error_code(unsafe { value.val.err }),
        XL_TYPE_MULTI => xll_multi_to_variant(value),
        XL_TYPE_NIL | XL_TYPE_MISSING => Variant::empty(),
        _ => Variant::empty(),
    }
}

fn xll_multi_to_variant(value: &XLOPER12) -> Variant {
    let array = unsafe { value.val.array };
    if array.lparray.is_null() || array.rows <= 0 || array.columns <= 0 {
        return Variant::from_safearray(SafeArray::from_variants_nd(
            vec![
                SafeArrayBound { lower: 1, count: 0 },
                SafeArrayBound { lower: 1, count: 0 },
            ],
            Vec::new(),
        ));
    }
    let rows = array.rows as usize;
    let columns = array.columns as usize;
    let len = rows.saturating_mul(columns);
    let elements = unsafe { std::slice::from_raw_parts(array.lparray, len) };
    let values = elements
        .iter()
        .map(|element| xll_to_variant(element, base_xltype(element.xltype)))
        .collect::<Vec<_>>();
    Variant::from_safearray(SafeArray::from_variants_nd(
        vec![
            SafeArrayBound { lower: 1, count: rows as u32 },
            SafeArrayBound { lower: 1, count: columns as u32 },
        ],
        values,
    ))
}

fn xll_to_f64(value: &XLOPER12, xltype: u32) -> f64 {
    match xltype {
        XL_TYPE_NUM => unsafe { value.val.num },
        XL_TYPE_INT => unsafe { value.val.w as f64 },
        XL_TYPE_BOOL => {
            if unsafe { value.val.xbool } != 0 { 1.0 } else { 0.0 }
        }
        _ => 0.0,
    }
}

fn xll_to_i64(value: &XLOPER12, xltype: u32) -> i64 {
    match xltype {
        XL_TYPE_INT => unsafe { value.val.w as i64 },
        XL_TYPE_NUM => unsafe { value.val.num as i64 },
        XL_TYPE_BOOL => {
            if unsafe { value.val.xbool } != 0 { -1 } else { 0 }
        }
        _ => 0,
    }
}

fn xll_to_bool(value: &XLOPER12, xltype: u32) -> bool {
    match xltype {
        XL_TYPE_BOOL => (unsafe { value.val.xbool }) != 0,
        XL_TYPE_INT => (unsafe { value.val.w }) != 0,
        XL_TYPE_NUM => (unsafe { value.val.num }) != 0.0,
        _ => false,
    }
}

fn xll_num(value: f64, flags: u32) -> XLOPER12 {
    XLOPER12 {
        val: Xloper12Value { num: value },
        xltype: XL_TYPE_NUM | flags,
    }
}

fn xll_bool(value: bool, flags: u32) -> XLOPER12 {
    XLOPER12 {
        val: Xloper12Value { xbool: if value { 1 } else { 0 } },
        xltype: XL_TYPE_BOOL | flags,
    }
}

fn xll_int(value: i32, flags: u32) -> XLOPER12 {
    XLOPER12 {
        val: Xloper12Value { w: value },
        xltype: XL_TYPE_INT | flags,
    }
}

fn xll_err(value: i32, flags: u32) -> XLOPER12 {
    XLOPER12 {
        val: Xloper12Value { err: value },
        xltype: XL_TYPE_ERR | flags,
    }
}

fn xll_nil(flags: u32) -> XLOPER12 {
    XLOPER12 {
        val: Xloper12Value { w: 0 },
        xltype: XL_TYPE_NIL | flags,
    }
}

fn safe_array_to_xll_multi(array: SafeArray) -> *mut XLOPER12 {
    let values = array
        .variant_elements()
        .unwrap_or_else(|| vec![Variant::empty(); array.len()]);
    let bounds = array.bounds().unwrap_or_else(|| {
        vec![SafeArrayBound {
            lower: 1,
            count: values.len() as u32,
        }]
    });
    let (rows, columns) = match bounds.as_slice() {
        [row, column, ..] => (row.count as i32, column.count as i32),
        [row] => (row.count as i32, 1),
        [] => (values.len() as i32, 1),
    };
    let mut array_wide = Vec::new();
    let mut elements = values
        .into_iter()
        .map(|value| variant_to_xll_array_element(value, &mut array_wide))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let lparray = if elements.is_empty() {
        std::ptr::null_mut()
    } else {
        elements.as_mut_ptr()
    };
    boxed_xll_oper(
        XLOPER12 {
            val: Xloper12Value {
                array: XLArray12 {
                    lparray,
                    rows,
                    columns,
                },
            },
            xltype: XL_TYPE_MULTI | XL_BIT_DLL_FREE,
        },
        None,
        Some(elements),
        array_wide,
    )
}

fn variant_to_xll_array_element(value: Variant, array_wide: &mut Vec<Box<[u16]>>) -> XLOPER12 {
    if let Some(value) = value.as_i16() {
        xll_int(value as i32, 0)
    } else if let Some(value) = value.as_i32() {
        xll_int(value, 0)
    } else if let Some(value) = value.as_i64() {
        xll_int(value as i32, 0)
    } else if let Some(value) = value.as_u8() {
        xll_int(value as i32, 0)
    } else if let Some(value) = value.as_date_f64() {
        xll_num(value, 0)
    } else if let Some(value) = value.as_currency_scaled_i64() {
        xll_num(value as f64 / 10_000.0, 0)
    } else if let Some(value) = value.as_f64() {
        xll_num(value, 0)
    } else if let Some(value) = value.as_f32() {
        xll_num(value as f64, 0)
    } else if let Some(value) = value.as_bool() {
        xll_bool(value, 0)
    } else if let Some(value) = value.as_error_code() {
        xll_err(value, 0)
    } else if let Some(text) = value.as_bstr() {
        let rendered = text.as_str();
        let mut wide = counted_wide_string(&rendered);
        let ptr = wide.as_mut_ptr();
        array_wide.push(wide);
        XLOPER12 {
            val: Xloper12Value { str_value: ptr },
            xltype: XL_TYPE_STR,
        }
    } else {
        xll_nil(0)
    }
}

fn counted_wide_string(text: &str) -> Box<[u16]> {
    let char_count = text.encode_utf16().count().min(u16::MAX as usize);
    let mut wide = Vec::with_capacity(char_count + 1);
    wide.push(char_count as u16);
    wide.extend(text.encode_utf16().take(char_count));
    wide.into_boxed_slice()
}

fn read_counted_wide_string(ptr: *const XChar) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let len = unsafe { *ptr as usize };
    let slice = unsafe { std::slice::from_raw_parts(ptr.add(1), len) };
    String::from_utf16_lossy(slice)
}

fn boxed_xll_oper(
    oper: XLOPER12,
    wide: Option<Box<[u16]>>,
    array: Option<Box<[XLOPER12]>>,
    array_wide: Vec<Box<[u16]>>,
) -> *mut XLOPER12 {
    Box::into_raw(Box::new(XllOwnedXloper12 {
        oper,
        _wide: wide,
        _array: array,
        _array_wide: array_wide,
    }))
    .cast::<XLOPER12>()
}

"#
}

fn rust_string_literal(value: &str) -> String {
    format!("{value:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_compiler::DeclareParamType;
    use oxvba_project::CallingConvention;

    #[test]
    fn xll_shim_has_required_entry_points() {
        let exports = vec![NativeExportDescriptor {
            exported_name: "MyFunc".to_string(),
            module_name: "Mod1".to_string(),
            procedure_name: "MyFunc".to_string(),
            calling_convention: CallingConvention::Stdcall,
            ordinal: None,
            kind: Some(oxvba_compiler::ExportKind::Function),
            param_types: Some(vec![DeclareParamType::Double]),
            return_type: Some(Some(DeclareParamType::Double)),
            category: Some("Pricing".to_string()),
            description: Some("Calculates a value".to_string()),
            argument_descriptions: Some("spot".to_string()),
        }];

        let source = generate_xll_shim("TestAddin", "test.oxb", &exports);
        assert!(source.contains("xlAutoOpen"));
        assert!(source.contains("xlAutoClose"));
        assert!(source.contains("xlAutoFree12"));
        assert!(source.contains("MyFunc"));
        assert!(source.contains("XLF_REGISTER"));
        assert!(source.contains("XL_GET_NAME"));
        assert!(source.contains("XL_FREE"));
        assert!(source.contains("resolve_excel12v"));
        assert!(source.contains("GetProcAddress"));
        assert!(source.contains("MdCallBack12"));
        assert!(source.contains("LoadLibraryA"));
        assert!(source.contains("xlAutoFree12(p: *mut XLOPER12)"));
        assert!(source.contains("drop(Box::from_raw(p.cast::<XllOwnedXloper12>()))"));
        assert!(source.contains("#[unsafe(no_mangle)]"));
        assert!(source.contains("REGISTRATIONS"));
        assert!(source.contains("type_text: \"QQ\""));
        assert!(source.contains("category: \"Pricing\""));
        assert!(source.contains("function_help: \"Calculates a value\""));
        assert!(source.contains("argument_text: \"spot\""));
        assert!(source.contains("pub extern \"system\" fn MyFunc(arg0: *const XLOPER12)"));
        assert!(source.contains("xll_arg_to_variant(arg0, DeclareParamType::Double)"));
        assert!(source.contains("let mut macro_type = xll_string(\"1\")"));
        assert!(source.contains("module_text as *mut XLOPER12"));
        assert!(source.contains("let mut missing_args: [XLOPER12; 24] = std::array::from_fn"));
        assert!(source.contains("for missing in &mut missing_args[2..]"));
        assert!(source.contains("fn trace_xll_event(message: &str)"));
        assert!(source.contains("std::env::var(\"OXVBA_XLL_TRACE\")"));
        assert!(source.contains("fn try_bind_excel_application_root(engine: &Engine)"));
        assert!(source.contains("const XL_GET_HWND: i32 = 0x4008;"));
        assert!(source.contains("fn acquire_host_excel_application_dispatch()"));
        assert!(source.contains("fn host_excel_identity()"));
        assert!(source.contains("GetWindowThreadProcessId"));
        assert!(source.contains("GetCurrentProcessId"));
        assert!(source.contains("GetRunningObjectTable"));
        assert!(source.contains("IRunningObjectTable::EnumRunning"));
        assert!(source.contains("fn select_host_excel_candidate("));
        assert!(source.contains("duplicate host ROT candidates collapsed"));
        assert!(source.contains("ambiguous pid-only ROT Excel.Application candidates"));
        assert!(!source.contains("GetActiveObject("));
        assert!(source.contains("engine.bind_native_dispatch_object("));
        assert!(source.contains("\"Excel.Application\""));
        assert!(source.contains("Excel.Application host root unavailable"));
        assert!(source.contains("xlfRegister procedure={} type_text={} status={} success={}"));
        assert!(source.contains("let args: Vec<Variant>"));
        assert!(
            source
                .contains(".invoke_procedure_with_variants(session, \"Mod1\", \"MyFunc\", &args)")
        );
        assert!(source.contains("fn variant_to_xll(value: Variant) -> *mut XLOPER12"));
        assert!(source.contains("XL_TYPE_MULTI => xll_multi_to_variant(value)"));
        assert!(source.contains("fn xll_multi_to_variant(value: &XLOPER12) -> Variant"));
        assert!(source.contains("SafeArray::from_variants_nd"));
        assert!(source.contains("fn safe_array_to_xll_multi(array: SafeArray) -> *mut XLOPER12"));
        assert!(source.contains("xltype: XL_TYPE_MULTI | XL_BIT_DLL_FREE"));
        assert!(source.contains("_array: Option<Box<[XLOPER12]>>"));
        assert!(source.contains("_array_wide: Vec<Box<[u16]>>"));
        assert!(source.contains("pub union Xloper12Value"));
        assert!(source.contains(
            "pub struct XLOPER12 {\n    pub val: Xloper12Value,\n    pub xltype: DWord,"
        ));
        assert!(source.contains("const XL_TYPE_INT: u32 = 0x0800;"));
        assert!(source.contains("const XL_BIT_DLL_FREE: u32 = 0x4000;"));
        assert!(source.contains("fn counted_wide_string(text: &str) -> Box<[u16]>"));
        assert!(source.contains("fn read_counted_wide_string(ptr: *const XChar) -> String"));
        assert!(!source.contains(&format!("{}{}", "value:", " usize")));
    }

    #[test]
    fn xll_registration_type_string() {
        let exports = vec![NativeExportDescriptor {
            exported_name: "Add".to_string(),
            module_name: "Math".to_string(),
            procedure_name: "Add".to_string(),
            calling_convention: CallingConvention::Stdcall,
            ordinal: None,
            kind: Some(oxvba_compiler::ExportKind::Function),
            param_types: Some(vec![DeclareParamType::Double, DeclareParamType::Double]),
            return_type: Some(Some(DeclareParamType::Double)),
            category: None,
            description: None,
            argument_descriptions: None,
        }];

        let source = generate_xll_shim("Math", "math.oxb", &exports);
        assert!(source.contains("type_text: \"QQQ\""));
    }

    #[test]
    fn xll_registration_strings_are_counted_wide_and_owned_during_call() {
        let source = generate_xll_shim("Math", "math.oxb", &[]);
        assert!(source.contains("struct XllString"));
        assert!(source.contains("wide.push(char_count as u16);"));
        assert!(source.contains("wide.extend(text.encode_utf16().take(char_count));"));
        assert!(source.contains("&mut procedure.oper as *mut XLOPER12"));
        assert!(source.contains("xltype: XL_TYPE_STR,"));
        assert!(!source.contains(&format!("{}{}", "text.as_ptr()", " as usize")));
    }

    #[test]
    fn xll_argument_and_return_helpers_use_xltype_union_fields() {
        let exports = vec![NativeExportDescriptor {
            exported_name: "Echo".to_string(),
            module_name: "Mod1".to_string(),
            procedure_name: "Echo".to_string(),
            calling_convention: CallingConvention::Stdcall,
            ordinal: None,
            kind: Some(oxvba_compiler::ExportKind::Function),
            param_types: Some(vec![
                DeclareParamType::String,
                DeclareParamType::Boolean,
                DeclareParamType::Double,
                DeclareParamType::Long,
            ]),
            return_type: Some(Some(DeclareParamType::String)),
            category: None,
            description: None,
            argument_descriptions: None,
        }];

        let source = generate_xll_shim("EchoAddin", "echo.oxb", &exports);
        assert!(source.contains("read_counted_wide_string(unsafe"));
        assert!(source.contains("value.val.str_value"));
        assert!(source.contains("value.val.xbool"));
        assert!(source.contains("value.val.num"));
        assert!(source.contains("value.val.w"));
        assert!(source.contains("Xloper12Value { str_value: ptr }"));
        assert!(source.contains("xltype: XL_TYPE_STR | XL_BIT_DLL_FREE"));
        assert!(source.contains("base_xltype(value.xltype)"));
        assert!(!source.contains(&format!("{}{}", "value", ".value")));
    }

    #[test]
    fn xll_shim_compiles_to_xll_artifact() {
        let temp_root =
            std::env::temp_dir().join(format!("oxvba_xll_compile_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp_root).expect("create temp root");
        let bundle_path = temp_root.join("dummy.oxb");
        std::fs::write(&bundle_path, b"dummy bundle bytes").expect("write dummy bundle");
        let bundle_literal = bundle_path.to_string_lossy().replace('\\', "/");
        let source = generate_xll_shim("CompileProbe", &bundle_literal, &[]);
        let output_path = temp_root.join("CompileProbe.xll");

        crate::compile::compile_shim(&source, &output_path, crate::compile::ShimOutputType::Xll)
            .expect("compile generated XLL shim");

        assert!(output_path.exists());
        assert!(std::fs::metadata(&output_path).expect("xll metadata").len() > 0);
        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
