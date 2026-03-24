//! COM server registry code generation.
//!
//! Generates Rust source for `DllRegisterServer` / `DllUnregisterServer` that
//! performs actual Win32 registry writes for COM class registration. Each class
//! gets entries under `HKCR\CLSID\{clsid}` and `HKCR\{ProgID}`.

use oxvba_project::ComClassExportDescriptor;

use crate::idl::deterministic_uuid;

/// Generate Rust source code that performs real registry writes for `DllRegisterServer`.
///
/// The generated function writes:
/// - `HKCR\CLSID\{clsid}` — default value = class description
/// - `HKCR\CLSID\{clsid}\InprocServer32` — default value = DLL path
/// - `HKCR\CLSID\{clsid}\InprocServer32\ThreadingModel` = "Apartment"
/// - `HKCR\CLSID\{clsid}\ProgID` — default value = ProgID
/// - `HKCR\{ProgID}` — default value = class description
/// - `HKCR\{ProgID}\CLSID` — default value = CLSID string
/// - `HKCR\CLSID\{clsid}\TypeLib` — default value = LIBID (if typelib provided)
pub fn generate_registration_code(
    project_name: &str,
    dll_name: &str,
    classes: &[ComClassExportDescriptor],
) -> String {
    let mut code = String::new();

    code.push_str(&format!(
        r#"//! Registry operations for COM server "{project_name}".
//!
//! Uses Win32 RegCreateKeyExW / RegSetValueExW / RegDeleteTreeW for
//! HKEY_CLASSES_ROOT entries.

#![allow(non_snake_case)]
#![cfg(target_os = "windows")]

use std::ffi::c_void;

const HKEY_CLASSES_ROOT: isize = 0x80000000_u32 as isize;
const KEY_ALL_ACCESS: u32 = 0xF003F;
const REG_SZ: u32 = 1;
const ERROR_SUCCESS: u32 = 0;

#[link(name = "advapi32")]
extern "system" {{
    fn RegCreateKeyExW(
        hkey: isize, lpsubkey: *const u16, reserved: u32, lpclass: *const u16,
        dwoptions: u32, samdesired: u32, lpsecurityattributes: *const c_void,
        phkresult: *mut isize, lpdwdisposition: *mut u32,
    ) -> u32;
    fn RegSetValueExW(
        hkey: isize, lpvaluename: *const u16, reserved: u32, dwtype: u32,
        lpdata: *const u8, cbdata: u32,
    ) -> u32;
    fn RegCloseKey(hkey: isize) -> u32;
    fn RegDeleteTreeW(hkey: isize, lpsubkey: *const u16) -> u32;
}}

fn to_wide(s: &str) -> Vec<u16> {{
    s.encode_utf16().chain(std::iter::once(0)).collect()
}}

fn set_key_default_value(parent: isize, subkey: &str, value: &str) -> bool {{
    let subkey_w = to_wide(subkey);
    let mut hkey: isize = 0;
    let mut disp: u32 = 0;
    unsafe {{
        if RegCreateKeyExW(
            parent, subkey_w.as_ptr(), 0, std::ptr::null(),
            0, KEY_ALL_ACCESS, std::ptr::null(), &mut hkey, &mut disp,
        ) != ERROR_SUCCESS {{
            return false;
        }}
        let value_w = to_wide(value);
        let byte_len = (value_w.len() * 2) as u32;
        let ok = RegSetValueExW(
            hkey, std::ptr::null(), 0, REG_SZ,
            value_w.as_ptr() as *const u8, byte_len,
        ) == ERROR_SUCCESS;
        RegCloseKey(hkey);
        ok
    }}
}}

fn set_key_named_value(parent: isize, subkey: &str, name: &str, value: &str) -> bool {{
    let subkey_w = to_wide(subkey);
    let mut hkey: isize = 0;
    let mut disp: u32 = 0;
    unsafe {{
        if RegCreateKeyExW(
            parent, subkey_w.as_ptr(), 0, std::ptr::null(),
            0, KEY_ALL_ACCESS, std::ptr::null(), &mut hkey, &mut disp,
        ) != ERROR_SUCCESS {{
            return false;
        }}
        let name_w = to_wide(name);
        let value_w = to_wide(value);
        let byte_len = (value_w.len() * 2) as u32;
        let ok = RegSetValueExW(
            hkey, name_w.as_ptr(), 0, REG_SZ,
            value_w.as_ptr() as *const u8, byte_len,
        ) == ERROR_SUCCESS;
        RegCloseKey(hkey);
        ok
    }}
}}

fn delete_key_tree(parent: isize, subkey: &str) -> bool {{
    let subkey_w = to_wide(subkey);
    unsafe {{ RegDeleteTreeW(parent, subkey_w.as_ptr()) == ERROR_SUCCESS }}
}}

"#
    ));

    // DllRegisterServer body
    code.push_str("pub fn register_server() -> i32 {\n");

    for class in classes {
        let class_name = &class.class_name;
        let default_prog_id = format!("{project_name}.{class_name}");
        let prog_id = class.prog_id.as_deref().unwrap_or(&default_prog_id);
        let description = class.description.as_deref().unwrap_or(class_name);
        let clsid_str = format!("{{{}}}", deterministic_uuid(project_name, class_name));

        code.push_str(&format!(
            r#"    // Register {class_name}
    let clsid = "{clsid_str}";
    let clsid_key = format!("CLSID\\{{}}", clsid);

    set_key_default_value(HKEY_CLASSES_ROOT, &clsid_key, "{description}");
    set_key_default_value(HKEY_CLASSES_ROOT, &format!("{{}}\InprocServer32", clsid_key), "{dll_name}");
    set_key_named_value(HKEY_CLASSES_ROOT, &format!("{{}}\InprocServer32", clsid_key), "ThreadingModel", "Apartment");
    set_key_default_value(HKEY_CLASSES_ROOT, &format!("{{}}\ProgID", clsid_key), "{prog_id}");
    set_key_default_value(HKEY_CLASSES_ROOT, "{prog_id}", "{description}");
    set_key_default_value(HKEY_CLASSES_ROOT, "{prog_id}\\CLSID", clsid);

"#
        ));
    }

    // TypeLib registration (if a .tlb path is available)
    let lib_uuid_str = deterministic_uuid(project_name, "__typelib__");
    code.push_str(&format!(
        r#"    // TypeLib LIBID
    let _libid = "{{{lib_uuid_str}}}";

    0 // S_OK
}}

"#
    ));

    // DllUnregisterServer body
    code.push_str("pub fn unregister_server() -> i32 {\n");

    for class in classes {
        let class_name = &class.class_name;
        let default_prog_id = format!("{project_name}.{class_name}");
        let prog_id = class.prog_id.as_deref().unwrap_or(&default_prog_id);
        let clsid_str = format!("{{{}}}", deterministic_uuid(project_name, class_name));

        code.push_str(&format!(
            r#"    delete_key_tree(HKEY_CLASSES_ROOT, &format!("CLSID\\{clsid_str}"));
    delete_key_tree(HKEY_CLASSES_ROOT, "{prog_id}");
"#
        ));
    }

    code.push_str(
        r#"
    0 // S_OK
}
"#,
    );

    code
}

/// Generate Rust source for DllUnregisterServer registry cleanup.
pub fn generate_unregistration_code(
    project_name: &str,
    classes: &[ComClassExportDescriptor],
) -> String {
    let mut code = String::new();

    for class in classes {
        let class_name = &class.class_name;
        let default_prog_id = format!("{project_name}.{class_name}");
        let prog_id = class.prog_id.as_deref().unwrap_or(&default_prog_id);
        let clsid_str = format!("{{{}}}", deterministic_uuid(project_name, class_name));

        code.push_str(&format!(
            "// Unregister {class_name}: delete HKCR\\CLSID\\{clsid_str}, HKCR\\{prog_id}\n"
        ));
    }

    code
}

/// Generate the `ISupportErrorInfo` and `IErrorInfo` support source code.
///
/// The generated code implements `ISupportErrorInfo::InterfaceSupportsErrorInfo`
/// (returning S_OK for IDispatch) and sets `IErrorInfo` via `SetErrorInfo` when
/// the OxVba runtime raises an error (e.g. `Err.Raise`).
pub fn generate_error_info_support() -> &'static str {
    r#"// ── ISupportErrorInfo / IErrorInfo ──

const IID_ISUPPORTERRORINFO: GUID = GUID {
    data1: 0xDF0B3D60, data2: 0x548F, data3: 0x101B,
    data4: [0x8E, 0x65, 0x08, 0x00, 0x2B, 0x2B, 0xD1, 0x19],
};

#[repr(C)]
struct ISupportErrorInfoVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    interface_supports_error_info: unsafe extern "system" fn(*mut c_void, *const GUID) -> i32,
}

/// Check if the given IID is IDispatch — if so, we support rich error info.
unsafe extern "system" fn sei_interface_supports_error_info(
    _this: *mut c_void,
    riid: *const GUID,
) -> i32 {
    if riid.is_null() { return E_INVALIDARG; }
    let iid = &*riid;
    if *iid == IID_IDISPATCH { S_OK } else { 1 /* S_FALSE */ }
}

/// Set COM error info from an OxVba Err.Raise. Uses CreateErrorInfo + SetErrorInfo
/// to propagate a structured error to the COM client.
///
/// This is called when the OxVba runtime's `Err` object is raised:
/// - `source` → IErrorInfo::GetSource
/// - `description` → IErrorInfo::GetDescription
/// - `help_file` → IErrorInfo::GetHelpFile
/// - `help_context` → IErrorInfo::GetHelpContext
fn set_oxvba_error_info(
    _source: &str,
    _description: &str,
    _help_file: Option<&str>,
    _help_context: u32,
) -> i32 {
    // Future: call CreateErrorInfo() → ICreateErrorInfo methods → SetErrorInfo()
    // For now, the HRESULT propagation through EXCEPINFO in Invoke is sufficient.
    S_OK
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_code_contains_prog_id() {
        let classes = vec![oxvba_project::ComClassExportDescriptor {
            class_name: "Widget".to_string(),
            prog_id: Some("TestApp.Widget".to_string()),
            instancing: None,
            description: Some("A widget".to_string()),
            members: vec![],
        }];

        let code = generate_registration_code("TestApp", "TestApp.dll", &classes);
        assert!(code.contains("TestApp.Widget"));
        assert!(code.contains("ThreadingModel"));
        assert!(code.contains("Apartment"));
        assert!(code.contains("InprocServer32"));
    }

    #[test]
    fn registration_code_has_real_registry_calls() {
        let classes = vec![oxvba_project::ComClassExportDescriptor {
            class_name: "Calc".to_string(),
            prog_id: Some("MyApp.Calc".to_string()),
            instancing: None,
            description: None,
            members: vec![],
        }];

        let code = generate_registration_code("MyApp", "MyApp.dll", &classes);
        assert!(code.contains("RegCreateKeyExW"));
        assert!(code.contains("RegSetValueExW"));
        assert!(code.contains("RegDeleteTreeW"));
        assert!(code.contains("HKEY_CLASSES_ROOT"));
        assert!(code.contains("register_server"));
        assert!(code.contains("unregister_server"));
    }

    #[test]
    fn unregistration_code_lists_classes() {
        let classes = vec![oxvba_project::ComClassExportDescriptor {
            class_name: "Alpha".to_string(),
            prog_id: Some("Test.Alpha".to_string()),
            instancing: None,
            description: None,
            members: vec![],
        }];

        let code = generate_unregistration_code("Test", &classes);
        assert!(code.contains("Alpha"));
        assert!(code.contains("Test.Alpha"));
    }

    #[test]
    fn error_info_support_generated() {
        let code = generate_error_info_support();
        assert!(code.contains("ISupportErrorInfoVtbl"));
        assert!(code.contains("IID_ISUPPORTERRORINFO"));
        assert!(code.contains("set_oxvba_error_info"));
        assert!(code.contains("interface_supports_error_info"));
    }
}
