#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::not_unsafe_ptr_arg_deref, clippy::upper_case_acronyms)]
//! Live ITypeLib/ITypeInfo COM loading for arbitrary typelib resolution.
//!
//! This module provides real COM-based type library loading for registered
//! and path-addressed COM type libraries. It wraps LoadRegTypeLib,
//! LoadTypeLibEx, and the ITypeLib/ITypeInfo COM interfaces to extract
//! member metadata from arbitrary registered type libraries. Test fixture
//! typelibs are handled separately behind `cfg(test)` / `fixture-typelibs`.

#[cfg(target_os = "windows")]
use crate::typelib::{
    TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibMemberInvokeKind, TypeLibMemberMetadata,
    TypeLibMetadataBlob, TypeLibParamType, TypeLibResolvedIdentity,
};
#[cfg(target_os = "windows")]
use crate::windows_client::COM_S_OK;
#[cfg(target_os = "windows")]
use std::collections::BTreeMap;
#[cfg(target_os = "windows")]
use std::convert::TryFrom;
#[cfg(target_os = "windows")]
use std::ffi::c_void;
#[cfg(target_os = "windows")]
use std::ptr::null_mut;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::ERROR_NO_MORE_ITEMS;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Com::CLSIDFromProgID;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CLASSES_ROOT, KEY_READ, REG_SZ, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW,
    RegQueryValueExW,
};

// ── ITypeLib / ITypeInfo vtable definitions ──

#[cfg(target_os = "windows")]
const INVOKE_FUNC: u32 = 1;
#[cfg(target_os = "windows")]
const INVOKE_PROPERTYGET: u32 = 2;
#[cfg(target_os = "windows")]
const INVOKE_PROPERTYPUT: u32 = 4;
#[cfg(target_os = "windows")]
const INVOKE_PROPERTYPUTREF: u32 = 8;

#[cfg(target_os = "windows")]
const TKIND_ENUM: u32 = 0;
#[cfg(target_os = "windows")]
const TKIND_RECORD: u32 = 1;
#[cfg(target_os = "windows")]
const TKIND_MODULE: u32 = 2;
#[cfg(target_os = "windows")]
const TKIND_INTERFACE: u32 = 3;
#[cfg(target_os = "windows")]
const TKIND_DISPATCH: u32 = 4;
#[cfg(target_os = "windows")]
const TKIND_COCLASS: u32 = 5;
#[cfg(target_os = "windows")]
const TKIND_ALIAS: u32 = 6;

/// `CALLCONV::CC_STDCALL` from oaidl.h — the only calling convention the x64
/// vtable marshaller may dispatch through.
#[cfg(target_os = "windows")]
const CC_STDCALL: u32 = 4;

/// `TYPEFLAG_FDUAL` from oaidl.h — set on a type that is both an `IDispatch`
/// dispinterface and a custom-interface vtable (so its members are reachable
/// both ways). Informational for the early-bound vtable gate.
#[cfg(target_os = "windows")]
const TYPEFLAG_FDUAL: u16 = 0x0040;

#[cfg(target_os = "windows")]
#[allow(dead_code)]
const IMPLTYPEFLAG_FDEFAULT: i32 = 1;
#[cfg(target_os = "windows")]
#[allow(dead_code)]
const IMPLTYPEFLAG_FSOURCE: i32 = 2;

// VT_ constants for parameter type extraction
#[cfg(target_os = "windows")]
const VT_I2: u16 = 2;
#[cfg(target_os = "windows")]
const VT_I4: u16 = 3;
#[cfg(target_os = "windows")]
const VT_R4: u16 = 4;
#[cfg(target_os = "windows")]
const VT_R8: u16 = 5;
#[cfg(target_os = "windows")]
const VT_CY: u16 = 6;
#[cfg(target_os = "windows")]
const VT_DATE: u16 = 7;
#[cfg(target_os = "windows")]
const VT_BSTR: u16 = 8;
#[cfg(target_os = "windows")]
const VT_DISPATCH: u16 = 9;
#[cfg(target_os = "windows")]
const VT_BOOL: u16 = 11;
#[cfg(target_os = "windows")]
#[allow(dead_code)]
const VT_VARIANT: u16 = 12;
#[cfg(target_os = "windows")]
const VT_UNKNOWN: u16 = 13;
#[cfg(target_os = "windows")]
const VT_DECIMAL: u16 = 14;
#[cfg(target_os = "windows")]
const VT_UI1: u16 = 17;
#[cfg(target_os = "windows")]
const VT_I8: u16 = 20;
#[cfg(target_os = "windows")]
const VT_HRESULT: u16 = 25;
#[cfg(target_os = "windows")]
const VT_PTR: u16 = 26;
#[cfg(target_os = "windows")]
const VT_VOID: u16 = 24;
#[cfg(target_os = "windows")]
const VT_INT: u16 = 22;
#[cfg(target_os = "windows")]
const VT_I1: u16 = 16;
#[cfg(target_os = "windows")]
const VT_UI2: u16 = 18;
#[cfg(target_os = "windows")]
const VT_UI4: u16 = 19;
#[cfg(target_os = "windows")]
const VT_UI8: u16 = 21;
#[cfg(target_os = "windows")]
const VT_UINT: u16 = 23;
#[cfg(target_os = "windows")]
const VT_SAFEARRAY: u16 = 27;
#[cfg(target_os = "windows")]
const VT_CARRAY: u16 = 28;
#[cfg(target_os = "windows")]
const VT_USERDEFINED: u16 = 29;
#[cfg(target_os = "windows")]
const VT_LPSTR: u16 = 30;
#[cfg(target_os = "windows")]
const VT_LPWSTR: u16 = 31;

// ── FUNCDESC / ELEMDESC / TYPEDESC raw structs ──

#[cfg(target_os = "windows")]
#[repr(C)]
struct TYPEDESC {
    union_field: usize, // lptdesc or hreftype depending on vt
    vt: u16,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct PARAMDESC {
    pparamdescex: *mut c_void,
    wparamflags: u16,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct ELEMDESC {
    tdesc: TYPEDESC,
    paramdesc: PARAMDESC,
}

#[cfg(target_os = "windows")]
#[repr(C)]
#[allow(non_snake_case)]
struct FUNCDESC {
    memid: i32,
    lprgscode: *mut i32,
    lprgelemdescparam: *mut ELEMDESC,
    funckind: u32,
    // INVOKEKIND is a 4-byte C enum in oaidl.h. This was declared u16, which
    // only matched the real layout through accidental tail padding plus
    // little-endian low-half reads (W1-hal-003); the static asserts below pin
    // the layout against the OS ABI structurally.
    invkind: u32,
    callconv: u32,
    cparams: i16,
    cparams_opt: i16,
    oVft: i16,
    cScodes: i16,
    elemdescfunc: ELEMDESC,
    wfuncdescflags: u16,
}

// FUNCDESC is read from COM-owned memory (ITypeInfo::GetFuncDesc), so its
// layout must match oaidl.h exactly — pin the fields after each historically
// fragile spot.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const _: () = {
    assert!(core::mem::offset_of!(FUNCDESC, invkind) == 28);
    assert!(core::mem::offset_of!(FUNCDESC, callconv) == 32);
    assert!(core::mem::offset_of!(FUNCDESC, cparams) == 36);
    assert!(core::mem::offset_of!(FUNCDESC, oVft) == 40);
    assert!(core::mem::offset_of!(FUNCDESC, elemdescfunc) == 48);
};

#[cfg(target_os = "windows")]
#[repr(C)]
struct TYPEATTR {
    guid: windows_sys::core::GUID,
    lcid: u32,
    dw_reserved: u32,
    memid_constructor: i32,
    memid_destructor: i32,
    lp_str_schema: *mut u16,
    cb_size_instance: u32,
    typekind: u32,
    cfuncs: u16,
    cvars: u16,
    cimpl_types: u16,
    cb_size_vft: u16,
    cb_alignment: u16,
    wtypeflags: u16,
    w_major_ver_num: u16,
    w_minor_ver_num: u16,
    tdesc_alias: TYPEDESC,
    idldesc: PARAMDESC,
}

// ── ITypeLib / ITypeInfo vtable layout ──
// We only declare the vtable function pointers we need.

#[cfg(target_os = "windows")]
#[repr(C)]
struct ITypeLibVtbl {
    // IUnknown
    query_interface: unsafe extern "system" fn(
        *mut c_void,
        *const windows_sys::core::GUID,
        *mut *mut c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    // ITypeLib
    get_type_info_count: unsafe extern "system" fn(*mut c_void) -> u32,
    get_type_info: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32,
    get_type_info_type: unsafe extern "system" fn(*mut c_void, u32, *mut u32) -> i32,
    get_type_info_of_guid: unsafe extern "system" fn(
        *mut c_void,
        *const windows_sys::core::GUID,
        *mut *mut c_void,
    ) -> i32,
    get_lib_attr: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    get_type_comp: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    get_documentation: unsafe extern "system" fn(
        *mut c_void,
        i32,
        *mut *mut u16,
        *mut *mut u16,
        *mut u32,
        *mut *mut u16,
    ) -> i32,
    is_name: unsafe extern "system" fn(*mut c_void, *mut u16, u32, *mut i32) -> i32,
    find_name: unsafe extern "system" fn(
        *mut c_void,
        *mut u16,
        u32,
        *mut *mut c_void,
        *mut i32,
        *mut u16,
    ) -> i32,
    release_t_lib_attr: unsafe extern "system" fn(*mut c_void, *mut c_void),
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct ITypeInfoVtbl {
    // IUnknown
    query_interface: unsafe extern "system" fn(
        *mut c_void,
        *const windows_sys::core::GUID,
        *mut *mut c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    // ITypeInfo
    get_type_attr: unsafe extern "system" fn(*mut c_void, *mut *mut TYPEATTR) -> i32,
    get_type_comp: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    get_func_desc: unsafe extern "system" fn(*mut c_void, u32, *mut *mut FUNCDESC) -> i32,
    get_var_desc: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32,
    get_names: unsafe extern "system" fn(*mut c_void, i32, *mut *mut u16, u32, *mut u32) -> i32,
    get_ref_type_of_impl_type: unsafe extern "system" fn(*mut c_void, u32, *mut u32) -> i32,
    get_impl_type_flags: unsafe extern "system" fn(*mut c_void, u32, *mut i32) -> i32,
    get_ids_of_names: unsafe extern "system" fn(*mut c_void, *mut *mut u16, u32, *mut i32) -> i32,
    invoke: unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        i32,
        u16,
        *mut c_void,
        *mut c_void,
        *mut c_void,
        *mut u32,
    ) -> i32,
    get_documentation: unsafe extern "system" fn(
        *mut c_void,
        i32,
        *mut *mut u16,
        *mut *mut u16,
        *mut u32,
        *mut *mut u16,
    ) -> i32,
    get_dll_entry: unsafe extern "system" fn(
        *mut c_void,
        i32,
        u16,
        *mut *mut u16,
        *mut *mut u16,
        *mut u16,
    ) -> i32,
    get_ref_type_info: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32,
    address_of_member: unsafe extern "system" fn(*mut c_void, i32, u16, *mut *mut c_void) -> i32,
    create_instance: unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        *const windows_sys::core::GUID,
        *mut *mut c_void,
    ) -> i32,
    get_mops: unsafe extern "system" fn(*mut c_void, i32, *mut *mut u16) -> i32,
    get_containing_type_lib:
        unsafe extern "system" fn(*mut c_void, *mut *mut c_void, *mut u32) -> i32,
    release_type_attr: unsafe extern "system" fn(*mut c_void, *mut TYPEATTR),
    release_func_desc: unsafe extern "system" fn(*mut c_void, *mut FUNCDESC),
    release_var_desc: unsafe extern "system" fn(*mut c_void, *mut c_void),
}

// ── FFI declarations ──

#[cfg(target_os = "windows")]
// SAFETY: signatures transcribed from oleauto.h (oleaut32 exports) with the stdcall
// `system` ABI; each call site documents the argument invariants it upholds.
unsafe extern "system" {
    fn LoadRegTypeLib(
        rguid: *const windows_sys::core::GUID,
        w_ver_major: u16,
        w_ver_minor: u16,
        lcid: u32,
        pptlib: *mut *mut c_void,
    ) -> i32;

    fn LoadTypeLibEx(
        sz_file: *const u16,
        regkind: u32, // REGKIND_NONE = 2
        pptlib: *mut *mut c_void,
    ) -> i32;

    fn SysFreeString(bstr_string: *mut u16);
}

// ── Helper: BSTR to String ──

#[cfg(target_os = "windows")]
unsafe fn bstr_to_string(bstr: *mut u16) -> Option<String> {
    if bstr.is_null() {
        return None;
    }
    // SAFETY: `bstr` was checked non-null above and the caller passes a live BSTR.
    // BSTR layout guarantees a UTF-16 NUL terminator at index SysStringLen (the
    // 4-byte byte-length prefix sits at ptr-4), so the NUL scan terminates inside
    // the allocation and `from_raw_parts(bstr, len)` covers only in-bounds,
    // initialized units. Embedded NULs are legal in BSTRs and merely truncate the
    // converted text — a fidelity limit, not a memory-safety hazard.
    unsafe {
        let mut len = 0usize;
        while *bstr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(bstr, len);
        Some(String::from_utf16_lossy(slice))
    }
}

#[cfg(target_os = "windows")]
unsafe fn bstr_to_string_and_free(bstr: *mut u16) -> Option<String> {
    // SAFETY: the caller transfers ownership of `bstr` (a live BSTR or null).
    // `bstr_to_string` only reads it under that same precondition, and
    // SysFreeString is called at most once on a non-null pointer, returning the
    // allocation to the OS BSTR allocator; the pointer is not used afterward.
    unsafe {
        let result = bstr_to_string(bstr);
        if !bstr.is_null() {
            SysFreeString(bstr);
        }
        result
    }
}

// ── GUID helpers ──

#[cfg(target_os = "windows")]
fn guid_to_string(guid: &windows_sys::core::GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
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

#[cfg(target_os = "windows")]
fn reg_query_default_string(subkey: &str) -> Result<String, String> {
    let wide_subkey: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: `wide_subkey` is a live NUL-terminated UTF-16 buffer and `key` a live
    // out-slot; RegOpenKeyExW only writes the opened HKEY through `key` on success.
    let open_status = unsafe {
        RegOpenKeyExW(
            HKEY_CLASSES_ROOT,
            wide_subkey.as_ptr(),
            0,
            KEY_READ,
            &mut key,
        )
    };
    if open_status != 0 {
        return Err(format!(
            "RegOpenKeyExW failed for `HKCR\\{subkey}` with status 0x{open_status:08X}"
        ));
    }

    // SAFETY: `key` was opened successfully above (the `open_status != 0` early return is the
    // only other exit and `key` is never reassigned), so it is a valid HKEY until the
    // RegCloseKey below. The first RegQueryValueExW call only sizes the value (null data
    // pointer). For the second call, `bytes` declares the buffer capacity passed to the API,
    // and the buffer is sized to `ceil(bytes / 2)` u16s so that its byte length always meets or
    // exceeds the declared `bytes` capacity — even for an odd-length value, the API can never
    // write past the real allocation. The API writes at most `bytes` bytes (ERROR_MORE_DATA if
    // the value grew between calls).
    let result = unsafe {
        let mut value_type = 0u32;
        let mut bytes = 0u32;
        let status = RegQueryValueExW(
            key,
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut value_type,
            std::ptr::null_mut(),
            &mut bytes,
        );
        if status != 0 {
            Err(format!(
                "RegQueryValueExW size lookup failed for `HKCR\\{subkey}` with status 0x{status:08X}"
            ))
        } else if value_type != REG_SZ {
            Err(format!(
                "RegQueryValueExW returned non-string type {value_type} for `HKCR\\{subkey}`"
            ))
        } else {
            // Round byte length up to whole u16s so the allocation covers the full declared
            // `bytes` capacity even when the registry value's byte length is odd.
            let char_len = usize::try_from(bytes.div_ceil(2))
                .map_err(|_| format!("registry string too large for `HKCR\\{subkey}`"))?;
            let mut buffer = vec![0u16; char_len];
            let status = RegQueryValueExW(
                key,
                std::ptr::null(),
                std::ptr::null_mut(),
                &mut value_type,
                buffer.as_mut_ptr() as *mut u8,
                &mut bytes,
            );
            if status != 0 {
                Err(format!(
                    "RegQueryValueExW value lookup failed for `HKCR\\{subkey}` with status 0x{status:08X}"
                ))
            } else {
                while matches!(buffer.last(), Some(0)) {
                    buffer.pop();
                }
                Ok(String::from_utf16_lossy(&buffer))
            }
        }
    };

    // SAFETY: `key` was opened successfully above (early return otherwise) and the
    // queries are finished; this closes it exactly once.
    unsafe {
        RegCloseKey(key);
    }
    result
}

#[cfg(target_os = "windows")]
fn parse_registry_typelib_version(version_text: &str) -> Result<(u16, u16), String> {
    let trimmed = version_text.trim();
    let mut parts = trimmed.split('.');
    let major = parts
        .next()
        .ok_or_else(|| format!("missing typelib major version in `{trimmed}`"))?
        .parse::<u16>()
        .map_err(|_| format!("invalid typelib major version in `{trimmed}`"))?;
    let minor = parts
        .next()
        .unwrap_or("0")
        .parse::<u16>()
        .map_err(|_| format!("invalid typelib minor version in `{trimmed}`"))?;
    Ok((major, minor))
}

#[cfg(target_os = "windows")]
fn split_prog_id_name(prog_id_name: &str) -> Result<(String, Option<String>), String> {
    let trimmed = prog_id_name.trim();
    if trimmed.is_empty() {
        return Err("empty ProgID name".to_string());
    }
    // ProgIDs come in version-independent (`Program.Component`) and versioned
    // (`Program.Component.Version`, e.g. `DAO.DBEngine.120`) forms. Strip a purely
    // numeric trailing version component first, otherwise the coclass would resolve to
    // the version digits (`120`) instead of the class (`DBEngine`), defeating member scoping.
    let core = match trimmed.rsplit_once('.') {
        Some((head, tail))
            if !head.is_empty() && !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) =>
        {
            head.trim()
        }
        _ => trimmed,
    };
    if let Some((reference_name, coclass_name)) = core.rsplit_once('.') {
        let reference_name = reference_name.trim();
        let coclass_name = coclass_name.trim();
        if !reference_name.is_empty() && !coclass_name.is_empty() {
            return Ok((reference_name.to_string(), Some(coclass_name.to_string())));
        }
    }
    Ok((core.to_string(), None))
}

#[cfg(target_os = "windows")]
fn reg_enum_subkeys(subkey: &str) -> Result<Vec<String>, String> {
    let wide_subkey: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let mut key: HKEY = null_mut();
    // SAFETY: `wide_subkey` is a live NUL-terminated UTF-16 buffer and `key` a live
    // out-slot; RegOpenKeyExW only writes the opened HKEY through `key` on success.
    let open_status = unsafe {
        RegOpenKeyExW(
            HKEY_CLASSES_ROOT,
            wide_subkey.as_ptr(),
            0,
            KEY_READ,
            &mut key,
        )
    };
    if open_status != 0 {
        return Err(format!(
            "RegOpenKeyExW failed for `HKCR\\{subkey}` with status 0x{open_status:08X}"
        ));
    }

    let mut names = Vec::new();
    let mut index = 0u32;
    loop {
        let mut buffer = vec![0u16; 512];
        let mut len = (buffer.len() - 1) as u32;
        // SAFETY: `key` is a valid open HKEY (open_status == 0 above); `buffer` and
        // `len` are live locals with `len` set to one less than the buffer
        // capacity, so RegEnumKeyExW cannot write past the allocation and always
        // has room for the terminating NUL.
        let status = unsafe {
            RegEnumKeyExW(
                key,
                index,
                buffer.as_mut_ptr(),
                &mut len,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        if status != 0 {
            // SAFETY: `key` was opened successfully above; this early-exit path
            // closes it exactly once and does not use it afterward.
            unsafe { RegCloseKey(key) };
            return Err(format!(
                "RegEnumKeyExW failed for `HKCR\\{subkey}` at index {index} with status 0x{status:08X}"
            ));
        }
        names.push(String::from_utf16_lossy(&buffer[..len as usize]));
        index += 1;
    }

    // SAFETY: `key` was opened successfully above and enumeration is finished;
    // this closes it exactly once.
    unsafe { RegCloseKey(key) };
    Ok(names)
}

#[cfg(target_os = "windows")]
fn parse_guid_canonical_or_registry(guid_text: &str) -> Option<String> {
    crate::windows_client::parse_guid_canonical(guid_text).map(|guid| guid_to_string(&guid))
}

#[cfg(target_os = "windows")]
fn registry_typelib_importlib_for_version(base_subkey: &str) -> Option<String> {
    for lcid in reg_enum_subkeys(base_subkey).ok()? {
        for platform in ["win64", "win32"] {
            let path = format!(r"{base_subkey}\{lcid}\{platform}");
            if let Ok(importlib) = reg_query_default_string(&path)
                && !importlib.trim().is_empty()
            {
                return Some(importlib.trim().to_string());
            }
        }
    }
    None
}

// ── VT to TypeLibParamType ──

#[cfg(target_os = "windows")]
fn vt_to_param_type(vt: u16, is_byref: bool) -> TypeLibParamType {
    let base = match vt {
        VT_I2 => TypeLibParamType::Integer,
        VT_I4 | VT_INT => TypeLibParamType::Long,
        VT_R4 => TypeLibParamType::Single,
        VT_R8 => TypeLibParamType::Double,
        VT_CY => TypeLibParamType::Currency,
        VT_DATE => TypeLibParamType::Date,
        VT_BSTR => TypeLibParamType::String,
        VT_DISPATCH | VT_UNKNOWN => TypeLibParamType::Object,
        VT_BOOL => TypeLibParamType::Boolean,
        VT_DECIMAL => TypeLibParamType::Decimal,
        VT_I1 | VT_UI1 => TypeLibParamType::Byte,
        VT_UI2 | VT_UINT => TypeLibParamType::Long,
        VT_UI4 | VT_I8 | VT_UI8 => TypeLibParamType::LongLong,
        VT_LPSTR | VT_LPWSTR => TypeLibParamType::String,
        VT_SAFEARRAY | VT_CARRAY => TypeLibParamType::Variant,
        _ => TypeLibParamType::Variant,
    };
    if is_byref {
        match base {
            TypeLibParamType::Long => TypeLibParamType::ByRefLong,
            TypeLibParamType::Integer => TypeLibParamType::ByRefInteger,
            TypeLibParamType::String => TypeLibParamType::ByRefString,
            TypeLibParamType::Double => TypeLibParamType::ByRefDouble,
            TypeLibParamType::Single => TypeLibParamType::ByRefSingle,
            TypeLibParamType::Currency => TypeLibParamType::ByRefCurrency,
            TypeLibParamType::Date => TypeLibParamType::ByRefDate,
            TypeLibParamType::Decimal => TypeLibParamType::ByRefDecimal,
            TypeLibParamType::Object => TypeLibParamType::ByRefObject,
            TypeLibParamType::Byte => TypeLibParamType::ByRefByte,
            TypeLibParamType::Boolean => TypeLibParamType::ByRefBoolean,
            TypeLibParamType::LongLong => TypeLibParamType::ByRefLongLong,
            TypeLibParamType::LongPtr => TypeLibParamType::ByRefLongPtr,
            _ => TypeLibParamType::ByRefVariant,
        }
    } else {
        base
    }
}

#[cfg(target_os = "windows")]
unsafe fn apply_byref_param_type(base: TypeLibParamType, is_byref: bool) -> TypeLibParamType {
    if !is_byref {
        return base;
    }
    match base {
        TypeLibParamType::Long => TypeLibParamType::ByRefLong,
        TypeLibParamType::Integer => TypeLibParamType::ByRefInteger,
        TypeLibParamType::String => TypeLibParamType::ByRefString,
        TypeLibParamType::Double => TypeLibParamType::ByRefDouble,
        TypeLibParamType::Single => TypeLibParamType::ByRefSingle,
        TypeLibParamType::Currency => TypeLibParamType::ByRefCurrency,
        TypeLibParamType::Date => TypeLibParamType::ByRefDate,
        TypeLibParamType::Decimal => TypeLibParamType::ByRefDecimal,
        TypeLibParamType::Object => TypeLibParamType::ByRefObject,
        TypeLibParamType::Byte => TypeLibParamType::ByRefByte,
        TypeLibParamType::Boolean => TypeLibParamType::ByRefBoolean,
        TypeLibParamType::LongLong => TypeLibParamType::ByRefLongLong,
        TypeLibParamType::LongPtr => TypeLibParamType::ByRefLongPtr,
        _ => TypeLibParamType::ByRefVariant,
    }
}

#[cfg(target_os = "windows")]
unsafe fn typedesc_to_param_type(
    owner_ptinfo: *mut c_void,
    tdesc: &TYPEDESC,
    is_byref: bool,
) -> TypeLibParamType {
    if tdesc.vt == VT_PTR && tdesc.union_field != 0 {
        let inner = &*(tdesc.union_field as *const TYPEDESC);
        return typedesc_to_param_type(owner_ptinfo, inner, true);
    }

    if tdesc.vt == VT_USERDEFINED && tdesc.union_field != 0 {
        let href = u32::try_from(tdesc.union_field).unwrap_or(0);
        let vtbl = *(owner_ptinfo as *const *const ITypeInfoVtbl);
        let mut ref_ptinfo: *mut c_void = std::ptr::null_mut();
        if ((*vtbl).get_ref_type_info)(owner_ptinfo, href, &mut ref_ptinfo) == COM_S_OK
            && !ref_ptinfo.is_null()
        {
            let ref_vtbl = *(ref_ptinfo as *const *const ITypeInfoVtbl);
            let mut ref_attr: *mut TYPEATTR = std::ptr::null_mut();
            let resolved = if ((*ref_vtbl).get_type_attr)(ref_ptinfo, &mut ref_attr) == COM_S_OK
                && !ref_attr.is_null()
            {
                let typekind = (*ref_attr).typekind;
                let param_type = if typekind == TKIND_ENUM {
                    TypeLibParamType::Long
                } else if typekind == TKIND_ALIAS {
                    typedesc_to_param_type(ref_ptinfo, &(*ref_attr).tdesc_alias, false)
                } else {
                    TypeLibParamType::Object
                };
                ((*ref_vtbl).release_type_attr)(ref_ptinfo, ref_attr);
                param_type
            } else {
                TypeLibParamType::Variant
            };
            ((*ref_vtbl).release)(ref_ptinfo);
            return apply_byref_param_type(resolved, is_byref);
        }
        return apply_byref_param_type(TypeLibParamType::Variant, is_byref);
    }

    apply_byref_param_type(vt_to_param_type(tdesc.vt, false), is_byref)
}

/// Resolve a `[out,retval]` parameter's INNER TYPEDESC to the member's by-VALUE
/// language return type. A dual member encodes its return as `[out,retval] T*`,
/// so the retval handler strips the outer pointer and passes the inner here.
/// Unlike [`typedesc_to_param_type`], this NEVER produces a `ByRef*` type: a
/// further pointer-to-interface (`IDispatch**` → inner `IDispatch*`) is the
/// interface return `Object`, not a by-ref object, and a pointer-to-scalar is
/// the scalar return. Mis-classifying an interface retval as `ByRefObject` made
/// every object-returning member (Excel `Range`, `Workbooks`; DAO `Fields`,
/// `Field`) fail the v1 vtable gate.
///
/// # Safety
/// `owner_ptinfo` must be a live ITypeInfo for the duration of any
/// `GetRefTypeInfo` resolution this performs.
#[cfg(target_os = "windows")]
unsafe fn retval_typedesc_to_param_type(
    owner_ptinfo: *mut c_void,
    tdesc: &TYPEDESC,
) -> TypeLibParamType {
    // A pointer-to-interface (or pointer-to-anything) in retval position is the
    // by-value language type of the pointee; strip the pointer without byref.
    if tdesc.vt == VT_PTR && tdesc.union_field != 0 {
        let inner = &*(tdesc.union_field as *const TYPEDESC);
        return retval_typedesc_to_param_type(owner_ptinfo, inner);
    }
    if tdesc.vt == VT_USERDEFINED {
        // A user-defined type (interface/enum/alias) by value: resolve to Object/
        // Long/the aliased scalar, never byref.
        return typedesc_to_param_type(owner_ptinfo, tdesc, false);
    }
    vt_to_param_type(tdesc.vt, false)
}

#[cfg(target_os = "windows")]
fn invkind_to_member_invoke_kind(invkind: u32) -> TypeLibMemberInvokeKind {
    match invkind {
        INVOKE_PROPERTYGET => TypeLibMemberInvokeKind::PropertyGet,
        INVOKE_PROPERTYPUT => TypeLibMemberInvokeKind::PropertyPut,
        INVOKE_PROPERTYPUTREF => TypeLibMemberInvokeKind::PropertyPutRef,
        _ => TypeLibMemberInvokeKind::Method,
    }
}

#[cfg(target_os = "windows")]
fn requested_coclass_name(identity: &TypeLibResolvedIdentity) -> Option<&str> {
    identity.requested_coclass.as_deref()
}

#[cfg(target_os = "windows")]
unsafe fn typeinfo_name(ptinfo: *mut c_void) -> Option<String> {
    let vtbl = *(ptinfo as *const *const ITypeInfoVtbl);
    let mut name_bstr: *mut u16 = std::ptr::null_mut();
    let hr = ((*vtbl).get_documentation)(
        ptinfo,
        -1,
        &mut name_bstr,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    if hr != COM_S_OK {
        return None;
    }
    bstr_to_string_and_free(name_bstr)
}

// ── Typelib shape audit ──

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeLibShapeAudit {
    pub type_count: u32,
    pub function_count: u32,
    pub variable_count: u32,
    pub typekind_counts: BTreeMap<String, u32>,
    pub invkind_counts: BTreeMap<String, u32>,
    pub vt_counts: BTreeMap<String, u32>,
    pub unsupported_vt_counts: BTreeMap<String, u32>,
    pub optional_param_count: u32,
    pub byref_param_count: u32,
    pub param_array_like_count: u32,
}

#[cfg(target_os = "windows")]
impl TypeLibShapeAudit {
    pub fn unsupported_total(&self) -> u32 {
        self.unsupported_vt_counts.values().sum()
    }

    pub fn csv_rows(&self, label: &str) -> Vec<String> {
        let mut rows = vec![format!(
            "summary,{label},{},{},{},{},{},{}",
            self.type_count,
            self.function_count,
            self.variable_count,
            self.optional_param_count,
            self.byref_param_count,
            self.param_array_like_count
        )];
        for (name, count) in &self.typekind_counts {
            rows.push(format!("typekind,{label},{name},{count}"));
        }
        for (name, count) in &self.invkind_counts {
            rows.push(format!("invkind,{label},{name},{count}"));
        }
        for (name, count) in &self.vt_counts {
            rows.push(format!("vt,{label},{name},{count}"));
        }
        for (name, count) in &self.unsupported_vt_counts {
            rows.push(format!("unsupported_vt,{label},{name},{count}"));
        }
        rows
    }
}

#[cfg(target_os = "windows")]
fn increment_count(map: &mut BTreeMap<String, u32>, key: impl Into<String>) {
    *map.entry(key.into()).or_default() += 1;
}

#[cfg(target_os = "windows")]
fn typekind_label(typekind: u32) -> String {
    match typekind {
        TKIND_ENUM => "enum".to_string(),
        TKIND_RECORD => "record".to_string(),
        TKIND_MODULE => "module".to_string(),
        TKIND_INTERFACE => "interface".to_string(),
        TKIND_DISPATCH => "dispatch".to_string(),
        TKIND_COCLASS => "coclass".to_string(),
        TKIND_ALIAS => "alias".to_string(),
        other => format!("kind_{other}"),
    }
}

#[cfg(target_os = "windows")]
fn invkind_label(invkind: u32) -> String {
    match invkind {
        INVOKE_FUNC => "func".to_string(),
        INVOKE_PROPERTYGET => "property_get".to_string(),
        INVOKE_PROPERTYPUT => "property_put".to_string(),
        INVOKE_PROPERTYPUTREF => "property_putref".to_string(),
        other => format!("invkind_{other}"),
    }
}

#[cfg(target_os = "windows")]
fn vt_label(vt: u16) -> String {
    match vt {
        VT_VOID => "VT_VOID".to_string(),
        VT_I2 => "VT_I2".to_string(),
        VT_I4 => "VT_I4".to_string(),
        VT_R4 => "VT_R4".to_string(),
        VT_R8 => "VT_R8".to_string(),
        VT_CY => "VT_CY".to_string(),
        VT_DATE => "VT_DATE".to_string(),
        VT_BSTR => "VT_BSTR".to_string(),
        VT_DISPATCH => "VT_DISPATCH".to_string(),
        VT_BOOL => "VT_BOOL".to_string(),
        VT_VARIANT => "VT_VARIANT".to_string(),
        VT_UNKNOWN => "VT_UNKNOWN".to_string(),
        VT_DECIMAL => "VT_DECIMAL".to_string(),
        VT_I1 => "VT_I1".to_string(),
        VT_UI1 => "VT_UI1".to_string(),
        VT_UI2 => "VT_UI2".to_string(),
        VT_UI4 => "VT_UI4".to_string(),
        VT_I8 => "VT_I8".to_string(),
        VT_UI8 => "VT_UI8".to_string(),
        VT_INT => "VT_INT".to_string(),
        VT_UINT => "VT_UINT".to_string(),
        VT_HRESULT => "VT_HRESULT".to_string(),
        VT_PTR => "VT_PTR".to_string(),
        VT_SAFEARRAY => "VT_SAFEARRAY".to_string(),
        VT_CARRAY => "VT_CARRAY".to_string(),
        VT_USERDEFINED => "VT_USERDEFINED".to_string(),
        VT_LPSTR => "VT_LPSTR".to_string(),
        VT_LPWSTR => "VT_LPWSTR".to_string(),
        other => format!("VT_{other}"),
    }
}

#[cfg(target_os = "windows")]
fn vt_supported_directly(vt: u16) -> bool {
    matches!(
        vt,
        0 | VT_VOID
            | VT_HRESULT
            | VT_I2
            | VT_I4
            | VT_R4
            | VT_R8
            | VT_CY
            | VT_DATE
            | VT_BSTR
            | VT_DISPATCH
            | VT_BOOL
            | VT_VARIANT
            | VT_UNKNOWN
            | VT_DECIMAL
            | VT_I1
            | VT_UI1
            | VT_UI2
            | VT_UI4
            | VT_I8
            | VT_UI8
            | VT_INT
            | VT_UINT
            | VT_PTR
            | VT_SAFEARRAY
            | VT_CARRAY
            | VT_LPSTR
            | VT_LPWSTR
    )
}

#[cfg(target_os = "windows")]
unsafe fn userdefined_typedesc_supported(owner_ptinfo: *mut c_void, tdesc: &TYPEDESC) -> bool {
    if tdesc.union_field == 0 {
        return true;
    }
    let href = u32::try_from(tdesc.union_field).unwrap_or(0);
    let vtbl = *(owner_ptinfo as *const *const ITypeInfoVtbl);
    let mut ref_ptinfo: *mut c_void = std::ptr::null_mut();
    if ((*vtbl).get_ref_type_info)(owner_ptinfo, href, &mut ref_ptinfo) != COM_S_OK
        || ref_ptinfo.is_null()
    {
        return true;
    }
    let ref_vtbl = *(ref_ptinfo as *const *const ITypeInfoVtbl);
    let mut ref_attr: *mut TYPEATTR = std::ptr::null_mut();
    let supported = if ((*ref_vtbl).get_type_attr)(ref_ptinfo, &mut ref_attr) == COM_S_OK
        && !ref_attr.is_null()
    {
        let typekind = (*ref_attr).typekind;
        let ok = matches!(
            typekind,
            TKIND_ENUM
                | TKIND_ALIAS
                | TKIND_RECORD
                | TKIND_INTERFACE
                | TKIND_DISPATCH
                | TKIND_COCLASS
        );
        ((*ref_vtbl).release_type_attr)(ref_ptinfo, ref_attr);
        ok
    } else {
        true
    };
    ((*ref_vtbl).release)(ref_ptinfo);
    supported
}

#[cfg(target_os = "windows")]
unsafe fn audit_typedesc(
    owner_ptinfo: *mut c_void,
    tdesc: &TYPEDESC,
    audit: &mut TypeLibShapeAudit,
) {
    increment_count(&mut audit.vt_counts, vt_label(tdesc.vt));
    let supported = if tdesc.vt == VT_USERDEFINED {
        userdefined_typedesc_supported(owner_ptinfo, tdesc)
    } else {
        vt_supported_directly(tdesc.vt)
    };
    if !supported {
        increment_count(&mut audit.unsupported_vt_counts, vt_label(tdesc.vt));
    }
    if tdesc.vt == VT_PTR && tdesc.union_field != 0 {
        let inner = &*(tdesc.union_field as *const TYPEDESC);
        audit_typedesc(owner_ptinfo, inner, audit);
    }
}

#[cfg(target_os = "windows")]
pub fn audit_typelib_shapes(ptlib: *mut c_void) -> Result<TypeLibShapeAudit, String> {
    let mut audit = TypeLibShapeAudit::default();
    // SAFETY: callers obtain `ptlib` from `load_typelib_from_registry`/
    // `load_typelib_from_path` and release it via `release_typelib` only after this
    // call returns, so it is a live ITypeLib*; COM guarantees an interface
    // pointer's first pointer-sized field is its vtable pointer, and ITypeLibVtbl
    // mirrors the oaidl.h vtable prefix.
    let vtbl = unsafe { *(ptlib as *const *const ITypeLibVtbl) };
    // SAFETY: `vtbl` was read from the live ITypeLib just above; GetTypeInfoCount
    // is a plain vtable call on that same interface pointer with no out-params.
    let count = unsafe { ((*vtbl).get_type_info_count)(ptlib) };
    audit.type_count = count;

    for i in 0..count {
        let mut typekind: u32 = 0;
        // SAFETY: vtable call on the live ITypeLib; `typekind` is a live local
        // out-slot that the OS writes before returning.
        let hr = unsafe { ((*vtbl).get_type_info_type)(ptlib, i, &mut typekind) };
        if hr != COM_S_OK {
            continue;
        }
        increment_count(&mut audit.typekind_counts, typekind_label(typekind));

        let mut ptinfo: *mut c_void = std::ptr::null_mut();
        // SAFETY: vtable call on the live ITypeLib; on S_OK the OS stores a
        // retained ITypeInfo* into the live `ptinfo` slot, and this loop iteration
        // releases that reference after use.
        let hr = unsafe { ((*vtbl).get_type_info)(ptlib, i, &mut ptinfo) };
        if hr != COM_S_OK || ptinfo.is_null() {
            continue;
        }
        // SAFETY: `ptinfo` was checked S_OK and non-null above, so it is a live
        // ITypeInfo whose first field is its vtable (ITypeInfoVtbl prefix per
        // oaidl.h). GetTypeAttr/GetFuncDesc hand out COM-owned descriptors that
        // stay valid until the matching ReleaseTypeAttr/ReleaseFuncDesc calls
        // below, and both are S_OK/null-checked before being dereferenced.
        // `lprgelemdescparam` holds `cParams` ELEMDESCs per the FUNCDESC contract,
        // with `cparams` clamped non-negative before indexing (W1-hal-002); the
        // FUNCDESC field layout itself is pinned by the static asserts above the
        // struct. The trailing Release balances GetTypeInfo's retained reference.
        unsafe {
            let ti_vtbl = *(ptinfo as *const *const ITypeInfoVtbl);
            let mut pattr: *mut TYPEATTR = std::ptr::null_mut();
            if ((*ti_vtbl).get_type_attr)(ptinfo, &mut pattr) == COM_S_OK && !pattr.is_null() {
                audit.function_count += (*pattr).cfuncs as u32;
                audit.variable_count += (*pattr).cvars as u32;
                if (*pattr).typekind == TKIND_ALIAS {
                    audit_typedesc(ptinfo, &(*pattr).tdesc_alias, &mut audit);
                }
                for func_idx in 0..((*pattr).cfuncs as u32) {
                    let mut pfuncdesc: *mut FUNCDESC = std::ptr::null_mut();
                    if ((*ti_vtbl).get_func_desc)(ptinfo, func_idx, &mut pfuncdesc) == COM_S_OK
                        && !pfuncdesc.is_null()
                    {
                        let fd = &*pfuncdesc;
                        increment_count(&mut audit.invkind_counts, invkind_label(fd.invkind));
                        audit_typedesc(ptinfo, &fd.elemdescfunc.tdesc, &mut audit);
                        let cparams = fd.cparams.max(0) as u32;
                        let optional_count = fd.cparams_opt.max(0) as u32;
                        audit.optional_param_count += optional_count;
                        for p in 0..cparams {
                            let param_desc = &*fd.lprgelemdescparam.add(p as usize);
                            let flags = param_desc.paramdesc.wparamflags;
                            if (flags & 0x0002) != 0 || param_desc.tdesc.vt == VT_PTR {
                                audit.byref_param_count += 1;
                            }
                            if (flags & 0x0020) != 0 {
                                audit.param_array_like_count += 1;
                            }
                            audit_typedesc(ptinfo, &param_desc.tdesc, &mut audit);
                        }
                        ((*ti_vtbl).release_func_desc)(ptinfo, pfuncdesc);
                    }
                }
                ((*ti_vtbl).release_type_attr)(ptinfo, pattr);
            }
            ((*ti_vtbl).release)(ptinfo);
        }
    }

    Ok(audit)
}

// ── Public API ──

/// Loads a type library from the Windows registry by LIBID.
#[cfg(target_os = "windows")]
pub fn load_typelib_from_registry(
    libid: &windows_sys::core::GUID,
    major: u16,
    minor: u16,
    lcid: u32,
) -> Result<*mut c_void, String> {
    let mut ptlib: *mut c_void = std::ptr::null_mut();
    // SAFETY: FFI into oleaut32. `libid` is a live reference for the duration of
    // the call and `ptlib` a live out-slot; on success LoadRegTypeLib writes one
    // retained ITypeLib* that the caller owns and must release via
    // `release_typelib`.
    let hr = unsafe { LoadRegTypeLib(libid, major, minor, lcid, &mut ptlib) };
    if hr != COM_S_OK || ptlib.is_null() {
        return Err(format!(
            "LoadRegTypeLib failed for LIBID {} major={} minor={} lcid={}: HRESULT=0x{:08X}",
            guid_to_string(libid),
            major,
            minor,
            lcid,
            hr as u32
        ));
    }
    Ok(ptlib)
}

/// Loads a type library from a file path.
#[cfg(target_os = "windows")]
pub fn load_typelib_from_path(path: &str) -> Result<*mut c_void, String> {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut ptlib: *mut c_void = std::ptr::null_mut();
    // SAFETY: FFI into oleaut32. `wide` is a live NUL-terminated UTF-16 path and
    // `ptlib` a live out-slot; on success LoadTypeLibEx writes one retained
    // ITypeLib* that the caller owns and must release via `release_typelib`.
    let hr = unsafe {
        LoadTypeLibEx(wide.as_ptr(), 2 /* REGKIND_NONE */, &mut ptlib)
    };
    if hr != COM_S_OK || ptlib.is_null() {
        return Err(format!(
            "LoadTypeLibEx failed for path `{}`: HRESULT=0x{:08X}",
            path, hr as u32
        ));
    }
    Ok(ptlib)
}

/// Resolves a typelib identity from the registry by searching for the reference name.
#[cfg(target_os = "windows")]
pub fn resolve_typelib_identity_from_registry(
    request: &crate::typelib::TypeLibResolveRequest,
) -> Result<TypeLibResolvedIdentity, String> {
    // Try loading via LIBID if provided
    if let Some(ref libid_str) = request.libid_hint {
        let guid = crate::windows_client::parse_guid_canonical(libid_str)
            .ok_or_else(|| format!("invalid LIBID GUID: {}", libid_str))?;
        let major = request.major_version_hint.unwrap_or(1);
        let minor = request.minor_version_hint.unwrap_or(0);
        let lcid = request.lcid_hint.unwrap_or(0);

        let ptlib = load_typelib_from_registry(&guid, major, minor, lcid)?;
        // SAFETY: `ptlib` was loaded and null-checked by the load helper just
        // above and is not released until the block below, so
        // `extract_typelib_identity` operates on a live ITypeLib throughout.
        let identity = unsafe { extract_typelib_identity(ptlib, request)? };
        // SAFETY: `ptlib` is the live ITypeLib returned by the load helper above;
        // this Release balances the load's retained reference and the pointer is
        // not used afterward.
        unsafe {
            let vtbl = *(ptlib as *const *const ITypeLibVtbl);
            ((*vtbl).release)(ptlib);
        }
        return Ok(identity);
    }

    // Try loading via importlib path hint
    if let Some(ref importlib) = request.importlib_hint {
        let ptlib = load_typelib_from_path(importlib)?;
        // SAFETY: `ptlib` was loaded and null-checked by the load helper just
        // above and is not released until the block below, so
        // `extract_typelib_identity` operates on a live ITypeLib throughout.
        let identity = unsafe { extract_typelib_identity(ptlib, request)? };
        // SAFETY: `ptlib` is the live ITypeLib returned by the load helper above;
        // this Release balances the load's retained reference and the pointer is
        // not used afterward.
        unsafe {
            let vtbl = *(ptlib as *const *const ITypeLibVtbl);
            ((*vtbl).release)(ptlib);
        }
        return Ok(identity);
    }

    Err(format!(
        "cannot resolve typelib identity for `{}`: no LIBID or importlib path provided",
        request.reference_name
    ))
}

#[cfg(target_os = "windows")]
pub fn resolve_typelib_identity_from_prog_id(
    prog_id_name: &str,
) -> Result<TypeLibResolvedIdentity, String> {
    let wide_prog_id: Vec<u16> = prog_id_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut clsid = windows_sys::core::GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    // SAFETY: `wide_prog_id` is a live NUL-terminated UTF-16 buffer and `clsid` a
    // live local; CLSIDFromProgID only reads the string and writes the GUID
    // through the out pointer.
    let hr = unsafe { CLSIDFromProgID(wide_prog_id.as_ptr(), &mut clsid) };
    if hr < 0 {
        return Err(format!(
            "CLSIDFromProgID failed for `{prog_id_name}` with HRESULT 0x{:08X}",
            hr as u32
        ));
    }

    let clsid_text = guid_to_string(&clsid);
    let libid_text = reg_query_default_string(&format!("CLSID\\{clsid_text}\\TypeLib"))?;
    let version_text = reg_query_default_string(&format!("CLSID\\{clsid_text}\\Version"))?;
    let (major, minor) = parse_registry_typelib_version(&version_text)?;
    let (reference_name, requested_coclass) = split_prog_id_name(prog_id_name)?;
    let request = crate::typelib::TypeLibResolveRequest {
        reference_name,
        requested_coclass,
        importlib_hint: None,
        libid_hint: Some(libid_text.trim().to_string()),
        major_version_hint: Some(major),
        minor_version_hint: Some(minor),
        lcid_hint: Some(0),
    };
    resolve_typelib_identity_from_registry(&request)
}

#[cfg(target_os = "windows")]
pub fn discover_registered_typelib_identities_by_name(
    reference_name: &str,
) -> Result<Vec<TypeLibResolvedIdentity>, String> {
    let reference_name = reference_name.trim();
    if reference_name.is_empty() {
        return Err("empty reference name".to_string());
    }

    let mut matches = Vec::new();
    for libid_key in reg_enum_subkeys("TypeLib")? {
        let Some(libid) = parse_guid_canonical_or_registry(&libid_key) else {
            continue;
        };
        let version_root = format!(r"TypeLib\{libid_key}");
        for version_key in reg_enum_subkeys(&version_root)? {
            let version_subkey = format!(r"{version_root}\{version_key}");
            let library_name = match reg_query_default_string(&version_subkey) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if !library_name.trim().eq_ignore_ascii_case(reference_name) {
                continue;
            }
            let Some(importlib) = registry_typelib_importlib_for_version(&version_subkey) else {
                continue;
            };
            let Ok((major, minor)) = parse_registry_typelib_version(&version_key) else {
                continue;
            };
            matches.push(TypeLibResolvedIdentity {
                reference_name: reference_name.to_string(),
                requested_coclass: None,
                importlib: importlib.clone(),
                libid: Some(libid.clone()),
                major_version: major,
                minor_version: minor,
                lcid: Some(0),
                cache_key: format!("registry:{}:{}:{}", libid, major, minor),
            });
        }
    }

    matches.sort_by(|left, right| {
        left.reference_name
            .to_ascii_lowercase()
            .cmp(&right.reference_name.to_ascii_lowercase())
            .then_with(|| left.libid.cmp(&right.libid))
            .then_with(|| left.major_version.cmp(&right.major_version))
            .then_with(|| left.minor_version.cmp(&right.minor_version))
            .then_with(|| {
                left.importlib
                    .to_ascii_lowercase()
                    .cmp(&right.importlib.to_ascii_lowercase())
            })
    });
    matches.dedup();
    Ok(matches)
}

/// Extracts identity metadata from a loaded ITypeLib pointer.
#[cfg(target_os = "windows")]
unsafe fn extract_typelib_identity(
    ptlib: *mut c_void,
    request: &crate::typelib::TypeLibResolveRequest,
) -> Result<TypeLibResolvedIdentity, String> {
    let vtbl = *(ptlib as *const *const ITypeLibVtbl);
    let mut pattr: *mut c_void = std::ptr::null_mut();
    let hr = ((*vtbl).get_lib_attr)(ptlib, &mut pattr);
    if hr != COM_S_OK || pattr.is_null() {
        return Err(format!(
            "ITypeLib::GetLibAttr failed: HRESULT=0x{:08X}",
            hr as u32
        ));
    }

    let attr = &*(pattr as *const TLIBATTR);
    let libid = guid_to_string(&attr.guid);
    let major = attr.w_major_ver_num;
    let minor = attr.w_minor_ver_num;
    let lcid = attr.lcid;

    // Get library name
    let mut name_bstr: *mut u16 = std::ptr::null_mut();
    let _ = ((*vtbl).get_documentation)(
        ptlib,
        -1, // MEMBERID_NIL
        &mut name_bstr,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    let lib_name =
        bstr_to_string_and_free(name_bstr).unwrap_or_else(|| request.reference_name.clone());

    ((*vtbl).release_t_lib_attr)(ptlib, pattr);

    let importlib = request
        .importlib_hint
        .clone()
        .unwrap_or_else(|| lib_name.clone());
    let cache_key = if let Some(coclass) = request.requested_coclass.as_deref() {
        format!(
            "live:{}:{}:{}:{}",
            libid,
            major,
            minor,
            coclass.trim().to_ascii_lowercase()
        )
    } else {
        format!("live:{}:{}:{}", libid, major, minor)
    };

    Ok(TypeLibResolvedIdentity {
        reference_name: request.reference_name.clone(),
        requested_coclass: request.requested_coclass.clone(),
        importlib,
        libid: Some(libid),
        major_version: major,
        minor_version: minor,
        lcid: Some(lcid),
        cache_key,
    })
}

/// Enumerates all dispatch members from a loaded ITypeLib.
#[cfg(target_os = "windows")]
pub fn enumerate_typelib_members(ptlib: *mut c_void) -> Result<Vec<TypeLibMemberMetadata>, String> {
    let mut members = Vec::new();
    // SAFETY: callers obtain `ptlib` from `load_typelib_from_registry`/
    // `load_typelib_from_path` and release it via `release_typelib` only after this
    // call returns, so it is a live ITypeLib*; COM guarantees an interface
    // pointer's first pointer-sized field is its vtable pointer, and ITypeLibVtbl
    // mirrors the oaidl.h vtable prefix.
    let vtbl = unsafe { *(ptlib as *const *const ITypeLibVtbl) };
    // SAFETY: `vtbl` was read from the live ITypeLib just above; GetTypeInfoCount
    // is a plain vtable call on that same interface pointer with no out-params.
    let count = unsafe { ((*vtbl).get_type_info_count)(ptlib) };

    for i in 0..count {
        let mut typekind: u32 = 0;
        // SAFETY: vtable call on the live ITypeLib; `typekind` is a live local
        // out-slot that the OS writes before returning.
        let hr = unsafe { ((*vtbl).get_type_info_type)(ptlib, i, &mut typekind) };
        if hr != COM_S_OK {
            continue;
        }
        // Process dispatch/interfaces and module-scoped typelib functions (for example VBA runtime modules).
        if typekind != TKIND_DISPATCH && typekind != TKIND_INTERFACE && typekind != TKIND_MODULE {
            continue;
        }

        let mut ptinfo: *mut c_void = std::ptr::null_mut();
        // SAFETY: vtable call on the live ITypeLib; on S_OK the OS stores a
        // retained ITypeInfo* into the live `ptinfo` slot, and this loop iteration
        // releases that reference after use.
        let hr = unsafe { ((*vtbl).get_type_info)(ptlib, i, &mut ptinfo) };
        if hr != COM_S_OK || ptinfo.is_null() {
            continue;
        }

        // SAFETY: `ptinfo` was checked S_OK and non-null above, so it is the live
        // ITypeInfo that `extract_members_from_typeinfo` requires; it stays alive
        // until the release block below.
        let result = unsafe { extract_members_from_typeinfo(ptinfo) };
        // SAFETY: `ptinfo` was obtained S_OK/non-null from GetTypeInfo above and is
        // still live; its first field is its vtable (ITypeInfoVtbl prefix per
        // oaidl.h). This Release balances GetTypeInfo's retained reference and the
        // pointer is not used afterward.
        unsafe {
            let ti_vtbl = *(ptinfo as *const *const ITypeInfoVtbl);
            ((*ti_vtbl).release)(ptinfo);
        }
        if let Ok(mut type_members) = result {
            members.append(&mut type_members);
        }
    }
    Ok(members)
}

#[cfg(target_os = "windows")]
pub fn enumerate_typelib_members_for_coclass(
    ptlib: *mut c_void,
    coclass_name: &str,
) -> Result<Vec<TypeLibMemberMetadata>, String> {
    // SAFETY: callers obtain `ptlib` from `load_typelib_from_registry`/
    // `load_typelib_from_path` and release it via `release_typelib` only after this
    // call returns, so it is a live ITypeLib*; COM guarantees an interface
    // pointer's first pointer-sized field is its vtable pointer, and ITypeLibVtbl
    // mirrors the oaidl.h vtable prefix.
    let vtbl = unsafe { *(ptlib as *const *const ITypeLibVtbl) };
    // SAFETY: `vtbl` was read from the live ITypeLib just above; GetTypeInfoCount
    // is a plain vtable call on that same interface pointer with no out-params.
    let count = unsafe { ((*vtbl).get_type_info_count)(ptlib) };

    for i in 0..count {
        let mut typekind: u32 = 0;
        // SAFETY: vtable call on the live ITypeLib; `typekind` is a live local
        // out-slot that the OS writes before returning.
        let hr = unsafe { ((*vtbl).get_type_info_type)(ptlib, i, &mut typekind) };
        if hr != COM_S_OK || typekind != TKIND_COCLASS {
            continue;
        }

        let mut ptinfo: *mut c_void = std::ptr::null_mut();
        // SAFETY: vtable call on the live ITypeLib; on S_OK the OS stores a
        // retained ITypeInfo* into the live `ptinfo` slot, and this loop iteration
        // releases that reference after use.
        let hr = unsafe { ((*vtbl).get_type_info)(ptlib, i, &mut ptinfo) };
        if hr != COM_S_OK || ptinfo.is_null() {
            continue;
        }

        // SAFETY: `ptinfo` was checked S_OK and non-null above; `typeinfo_name`
        // only performs vtable calls on that live ITypeInfo and frees the BSTR it
        // receives.
        let is_match = unsafe { typeinfo_name(ptinfo) }
            .is_some_and(|name| name.eq_ignore_ascii_case(coclass_name));
        let result = if is_match {
            // SAFETY: `ptinfo` is the live coclass ITypeInfo checked S_OK/non-null
            // above; the callee only performs vtable calls on it and releases
            // every descriptor and referenced ITypeInfo it acquires.
            unsafe { extract_members_from_coclass_default_interface(ptinfo) }
        } else {
            Ok(Vec::new())
        };
        // SAFETY: `ptinfo` was obtained S_OK/non-null from GetTypeInfo above and is
        // still live; its first field is its vtable (ITypeInfoVtbl prefix per
        // oaidl.h). This Release balances GetTypeInfo's retained reference and the
        // pointer is not used afterward.
        unsafe {
            let ti_vtbl = *(ptinfo as *const *const ITypeInfoVtbl);
            ((*ti_vtbl).release)(ptinfo);
        }
        if is_match {
            return result;
        }
    }

    Ok(Vec::new())
}

/// Scopes member enumeration to a named dispinterface/interface (e.g. DAO `Database`,
/// `Recordset`), rather than a coclass. Many libraries (DAO, ADO `Field`, WMI) expose
/// objects you never `CoCreate` — they are obtained from method calls — so they appear
/// only as `TKIND_DISPATCH`/`TKIND_INTERFACE`, not as coclasses. Binding such a type must
/// stay scoped to that one interface; flattening the whole library would make every shared
/// member name (DAO `Execute`/`OpenRecordset`/`Fields`) collide across unrelated objects.
#[cfg(target_os = "windows")]
pub fn enumerate_typelib_members_for_interface(
    ptlib: *mut c_void,
    interface_name: &str,
) -> Result<Vec<TypeLibMemberMetadata>, String> {
    // SAFETY: callers obtain `ptlib` from `load_typelib_from_registry`/
    // `load_typelib_from_path` and release it via `release_typelib` only after this
    // call returns, so it is a live ITypeLib*; COM guarantees an interface
    // pointer's first pointer-sized field is its vtable pointer, and ITypeLibVtbl
    // mirrors the oaidl.h vtable prefix.
    let vtbl = unsafe { *(ptlib as *const *const ITypeLibVtbl) };
    // SAFETY: `vtbl` was read from the live ITypeLib just above; GetTypeInfoCount
    // is a plain vtable call on that same interface pointer with no out-params.
    let count = unsafe { ((*vtbl).get_type_info_count)(ptlib) };

    for i in 0..count {
        let mut typekind: u32 = 0;
        // SAFETY: vtable call on the live ITypeLib; `typekind` is a live local
        // out-slot that the OS writes before returning.
        let hr = unsafe { ((*vtbl).get_type_info_type)(ptlib, i, &mut typekind) };
        if hr != COM_S_OK || (typekind != TKIND_DISPATCH && typekind != TKIND_INTERFACE) {
            continue;
        }

        let mut ptinfo: *mut c_void = std::ptr::null_mut();
        // SAFETY: vtable call on the live ITypeLib; on S_OK the OS stores a
        // retained ITypeInfo* into the live `ptinfo` slot, and this loop iteration
        // releases that reference after use.
        let hr = unsafe { ((*vtbl).get_type_info)(ptlib, i, &mut ptinfo) };
        if hr != COM_S_OK || ptinfo.is_null() {
            continue;
        }

        // SAFETY: `ptinfo` was checked S_OK and non-null above; `typeinfo_name`
        // only performs vtable calls on that live ITypeInfo and frees the BSTR it
        // receives.
        let is_match = unsafe { typeinfo_name(ptinfo) }
            .is_some_and(|name| name.eq_ignore_ascii_case(interface_name));
        let result = if is_match {
            // SAFETY: `ptinfo` is the live ITypeInfo checked S_OK/non-null above;
            // it stays alive until the release block below.
            unsafe { extract_members_from_typeinfo(ptinfo) }
        } else {
            Ok(Vec::new())
        };
        // SAFETY: `ptinfo` was obtained S_OK/non-null from GetTypeInfo above and is
        // still live; its first field is its vtable (ITypeInfoVtbl prefix per
        // oaidl.h). This Release balances GetTypeInfo's retained reference and the
        // pointer is not used afterward.
        unsafe {
            let ti_vtbl = *(ptinfo as *const *const ITypeInfoVtbl);
            ((*ti_vtbl).release)(ptinfo);
        }
        if is_match {
            return result;
        }
    }

    Ok(Vec::new())
}

/// TLIBATTR layout (oaidl.h): GUID, LCID, SYSKIND, wMajorVerNum, wMinorVerNum,
/// wLibFlags. `syskind` distinguishes the typelib's authored word size, which is
/// the granularity of `FUNCDESC::oVft`.
#[cfg(target_os = "windows")]
#[repr(C)]
struct TLIBATTR {
    guid: windows_sys::core::GUID,
    lcid: u32,
    syskind: u32,
    w_major_ver_num: u16,
    w_minor_ver_num: u16,
    w_lib_flags: u16,
}

/// The LIVE vtable stride: oleaut reports `FUNCDESC::oVft` in pointer-size units
/// of the RUNNING process, so a slot advances `size_of::<*const c_void>()` bytes
/// (8 on x64). This is the only correct divisor (see [`vtable_slot_index_from_ovft`]).
#[cfg(target_os = "windows")]
const LIVE_VTABLE_STRIDE: u16 = core::mem::size_of::<*const c_void>() as u16;

/// Convert a `FUNCDESC::oVft` byte offset into a vtable **slot index**.
///
/// THE ROOT-CAUSE FIX (workset `WORKSET_2026-06-12_COM_VTABLE_EARLY_BOUND_DISPATCH`):
/// `slot = oVft / size_of::<*const c_void>()` — i.e. `oVft / 8` on x64, the LIVE
/// pointer-size stride, NOT the typelib's authored syskind granularity.
///
/// The prior code divided by the CONTAINING typelib's `syskind` pointer size
/// (4 for `SYS_WIN32`). That was wrong: oleaut already reports `oVft` in LIVE
/// pointer-size units regardless of the typelib's authored word size, so a `/4`
/// DOUBLED every slot index. The crash-isolated value-oracle probe
/// (`crates/oxvba-com/tests/com_vtable_probe.rs`) proved it on a live ACE DAO
/// engine: `Field.Value` has `oVft=136` → slot `17` (= 136/8) returns the correct
/// `VT_I4(7)`, whereas slot `34` (= 136/4) returns garbage `VT_EMPTY`; and
/// `Recordset.Close`'s `/4` slot `98` over-ran the 92-slot vtable, which is the
/// access violation that shipped CI-green. A negative or mis-aligned offset is
/// malformed: return `None` so the member is treated as having no vtable slot.
#[cfg(target_os = "windows")]
fn vtable_slot_index_from_ovft(ovft: i16) -> Option<u16> {
    let raw = u16::try_from(ovft).ok()?;
    if LIVE_VTABLE_STRIDE == 0 || raw % LIVE_VTABLE_STRIDE != 0 {
        return None;
    }
    Some(raw / LIVE_VTABLE_STRIDE)
}

/// The vtable **slot-count bound** of an interface from its `TYPEATTR::cbSizeVft`
/// (the vtable byte size), in LIVE pointer units (`cbSizeVft / 8` on x64). This
/// is the AV-safety net the probe proved: a slot index `>= bound` would over-read
/// the live vtable (the access violation). `None` when `cbSizeVft` is zero.
///
/// `cbSizeVft` MUST come from the FDUAL partner INTERFACE typeinfo, NOT the
/// dispinterface (whose `cbSizeVft = 56` is just IDispatch's 7 slots). The probe
/// measured Recordset's interface `cbSizeVft = 736` → bound 92, and Field's
/// interface `cbSizeVft = 464` → bound 58.
#[cfg(target_os = "windows")]
fn vtable_slot_bound_from_cb_size_vft(cb_size_vft: u16) -> Option<u16> {
    if cb_size_vft == 0 || LIVE_VTABLE_STRIDE == 0 {
        return None;
    }
    Some(cb_size_vft / LIVE_VTABLE_STRIDE)
}

/// Extracts member metadata from a single ITypeInfo.
#[cfg(target_os = "windows")]
unsafe fn extract_members_from_typeinfo(
    ptinfo: *mut c_void,
) -> Result<Vec<TypeLibMemberMetadata>, String> {
    let vtbl = *(ptinfo as *const *const ITypeInfoVtbl);
    let mut pattr: *mut TYPEATTR = std::ptr::null_mut();
    let hr = ((*vtbl).get_type_attr)(ptinfo, &mut pattr);
    if hr != COM_S_OK || pattr.is_null() {
        return Err("ITypeInfo::GetTypeAttr failed".to_string());
    }
    let func_count = (*pattr).cfuncs as u32;
    let typekind = (*pattr).typekind;
    // TYPEFLAG_FDUAL on the containing type tells us its members are reachable
    // both via IDispatch::Invoke and a custom-interface vtable slot.
    let is_dual = ((*pattr).wtypeflags & TYPEFLAG_FDUAL) != 0;
    // The TKIND this typeinfo describes. Only a real custom interface
    // (TKIND_INTERFACE) carries a callable vtable; a dispinterface
    // (TKIND_DISPATCH) does not (its FUNCDESC.oVft is authored for the FDUAL
    // PARTNER interface, so a slot call on the dispinterface pointer over-reads).
    let source_typekind = match typekind {
        TKIND_INTERFACE => Some(crate::SourceTypeKind::Interface),
        TKIND_DISPATCH => Some(crate::SourceTypeKind::Dispatch),
        _ => None,
    };
    // AV-safety bound: the slot count of THIS typeinfo's vtable, in LIVE pointer
    // units (cbSizeVft / 8 on x64). For a TKIND_INTERFACE this is the real vtable
    // length; the gate requires slot < bound so a slot can never over-run it.
    let vtable_slot_bound = vtable_slot_bound_from_cb_size_vft((*pattr).cb_size_vft);
    // The containing ITypeInfo's GUID IS the dual interface IID (S5a): for a dual
    // the dispinterface and the vtable interface share the same IID, so this is
    // the exact interface we QueryInterface for at the dispatch site to obtain a
    // real-vtable pointer that works in-process AND out-of-process. We capture it
    // for every typeinfo (an interface/dispinterface always has a meaningful
    // GUID); an all-zero IID is treated as "absent" by the dispatch-site gate.
    let interface_iid = {
        let iid = crate::ComInterfaceIid::from_guid(&(*pattr).guid);
        (!iid.is_null()).then_some(iid)
    };
    ((*vtbl).release_type_attr)(ptinfo, pattr);

    let mut members = Vec::new();
    for func_idx in 0..func_count {
        let mut pfuncdesc: *mut FUNCDESC = std::ptr::null_mut();
        let hr = ((*vtbl).get_func_desc)(ptinfo, func_idx, &mut pfuncdesc);
        if hr != COM_S_OK || pfuncdesc.is_null() {
            continue;
        }

        let fd = &*pfuncdesc;
        let memid = fd.memid;
        let invkind = fd.invkind;
        let callconv_is_stdcall = fd.callconv == CC_STDCALL;
        // Clamp like the audit path: `cParams` is i16 in COM-owned memory, and a
        // corrupt typelib reporting a negative count would sign-extend into a
        // multi-billion ELEMDESC walk / allocation (W1-hal-002).
        let cparams = fd.cparams.max(0) as u32;

        // Get function name and parameter names
        let max_names = cparams + 1;
        let mut names: Vec<*mut u16> = vec![std::ptr::null_mut(); max_names as usize];
        let mut name_count: u32 = 0;
        let _ = ((*vtbl).get_names)(
            ptinfo,
            memid,
            names.as_mut_ptr(),
            max_names,
            &mut name_count,
        );

        let func_name = if name_count > 0 && !names[0].is_null() {
            bstr_to_string_and_free(names[0]).unwrap_or_default()
        } else {
            ((*vtbl).release_func_desc)(ptinfo, pfuncdesc);
            continue;
        };

        // Skip IUnknown/IDispatch inherited methods (FUNCKIND_DISPATCH with
        // DISPIDs in the hidden-member range 0x60000000..0x60010000).
        if typekind != TKIND_MODULE
            && (memid as u32) >= 0x6000_0000
            && (memid as u32) < 0x6001_0000
            && invkind == INVOKE_FUNC
        {
            for name in names.iter().take(name_count as usize).skip(1) {
                if !name.is_null() {
                    SysFreeString(*name);
                }
            }
            ((*vtbl).release_func_desc)(ptinfo, pfuncdesc);
            continue;
        }

        let mut parameter_names = Vec::new();
        for name in names.iter().take(name_count as usize).skip(1) {
            let pname = bstr_to_string_and_free(*name).unwrap_or_default();
            parameter_names.push(pname);
        }

        // Extract parameter types and optional flags. Dual interfaces commonly
        // encode the language-level return value as a final [out, retval] T*
        // parameter while the ABI return is HRESULT; expose that as return_type
        // and do not count it as a callable input parameter.
        let mut parameter_types = Vec::new();
        let mut parameter_optional = Vec::new();
        let mut retval_return_type = None;
        let optional_count = u32::try_from(fd.cparams_opt.max(0)).unwrap_or(0);
        let optional_start = cparams.saturating_sub(optional_count);
        for p in 0..cparams {
            let param_desc = &*fd.lprgelemdescparam.add(p as usize);
            let flags = param_desc.paramdesc.wparamflags;
            let vt = param_desc.tdesc.vt;
            if (flags & 0x0008) != 0 {
                // `[out,retval] T*`: strip the outer out-pointer, then resolve the
                // pointee to its by-VALUE language type (an interface → Object,
                // never ByRefObject).
                let retval_type = if vt == VT_PTR && param_desc.tdesc.union_field != 0 {
                    let inner = &*(param_desc.tdesc.union_field as *const TYPEDESC);
                    retval_typedesc_to_param_type(ptinfo, inner)
                } else if vt == VT_USERDEFINED {
                    retval_typedesc_to_param_type(ptinfo, &param_desc.tdesc)
                } else {
                    vt_to_param_type(vt, false)
                };
                retval_return_type = Some(retval_type);
                continue;
            }
            let is_byref = (flags & 0x0002) != 0; // PARAMFLAG_FOUT
            let param_type = if vt == VT_PTR || vt == VT_USERDEFINED {
                typedesc_to_param_type(ptinfo, &param_desc.tdesc, is_byref)
            } else {
                vt_to_param_type(vt, is_byref)
            };
            let is_optional = (flags & 0x0010) != 0 // PARAMFLAG_FOPT
                || p >= optional_start;
            parameter_types.push(param_type);
            parameter_optional.push(is_optional);
        }

        // Extract return type. A `[out,retval]` param (above) wins; otherwise the
        // function's own declared return drives it. Some typelibs (notably DAO)
        // declare the language return type directly here as `T*` rather than the
        // HRESULT+retval-param idiom, so resolve it by VALUE (an interface `T*`
        // return is `Object`, a `T*` scalar is the scalar) — NOT via the by-ref
        // pointer path, which would mis-type an object return as `ByRefObject` and
        // make it fail the v1 vtable gate.
        let ret_vt = fd.elemdescfunc.tdesc.vt;
        let return_type = retval_return_type.or_else(|| {
            if ret_vt == VT_VOID || ret_vt == VT_HRESULT {
                None
            } else {
                Some(retval_typedesc_to_param_type(
                    ptinfo,
                    &fd.elemdescfunc.tdesc,
                ))
            }
        });

        if parameter_names.len() > parameter_types.len() {
            parameter_names.truncate(parameter_types.len());
        }

        let invoke_kind = invkind_to_member_invoke_kind(invkind);
        let requires_argument = !parameter_types.is_empty()
            || matches!(
                invoke_kind,
                TypeLibMemberInvokeKind::PropertyPut | TypeLibMemberInvokeKind::PropertyPutRef
            );

        // Default member check (DISPID 0)
        let is_default_member = memid == 0;

        // `FUNCDESC::oVft` is a BYTE OFFSET into the vtable. We store the slot
        // INDEX (not the raw offset) in `vtable_slot` so the `vtable_invoke`
        // marshaller can index `(*(*this))[slot]` directly. oleaut reports `oVft`
        // in LIVE pointer-size units, so the index is `oVft / 8` on x64 (see
        // `vtable_slot_index_from_ovft` for the value-oracle evidence behind the
        // divisor). A mis-aligned or negative oVft yields `None`.
        //
        // GATE (workset slice D): a vtable slot is only callable when sourced from
        // a real custom interface — `FDUAL && TKIND_INTERFACE`. A pure
        // dispinterface (`TKIND_DISPATCH`) member's oVft is authored for the FDUAL
        // PARTNER vtable, so it does NOT index the dispinterface's own 7-slot
        // vtable; calling it there over-reads. The live-recovery path crosses to
        // the partner INTERFACE (slice B) to source a callable slot; here, on a
        // bare dispinterface typeinfo, we drop the slot.
        let vtable_slot = if is_dual && typekind == TKIND_INTERFACE {
            vtable_slot_index_from_ovft(fd.oVft)
        } else {
            None
        };

        members.push(TypeLibMemberMetadata {
            name: func_name,
            token: memid,
            vtable_slot,
            requires_argument,
            invoke_kind,
            parameter_names,
            parameter_optional,
            is_default_member,
            parameter_types,
            return_type,
            callconv_is_stdcall,
            is_dual,
            interface_iid,
            source_typekind,
            vtable_slot_bound,
        });

        ((*vtbl).release_func_desc)(ptinfo, pfuncdesc);
    }
    Ok(members)
}

// Workset S5a removed `dispatch_is_marshaling_proxy` (and its `IID_IProxyManager`
// probe). The vtable fast path no longer calls a slot on the raw IDispatch
// pointer — it QueryInterfaces the object for the member's dual interface IID and
// calls on that (real-vtable) pointer, which works for an out-of-process
// marshaling proxy too via the oleaut universal marshaler. So a proxy is no longer
// a host-AV risk to be blanket-rejected; the only safety predicate that remains is
// "QI for the exact dual IID must succeed before any slot call", enforced inside
// `try_vtable_member_spec_invoke_with_shared_state`.

/// A live dispinterface's FDUAL PARTNER `TKIND_INTERFACE` facts, recovered by
/// crossing the dispinterface typeinfo to its partner interface (workset slice B,
/// ported from the value-oracle probe `com_vtable_probe.rs`). These are exactly
/// the inputs the vtable gate + dispatch site need to slot-call safely:
/// the partner INTERFACE IID to `QueryInterface` for, and its `cbSizeVft`-derived
/// AV-safety slot bound.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
struct PartnerInterfaceFacts {
    /// The partner INTERFACE's GUID — the IID the dispatch site QIs for.
    interface_iid: crate::ComInterfaceIid,
    /// AV-safety bound: `cbSizeVft / size_of::<*const c_void>()` of the PARTNER
    /// interface (NOT the dispinterface, whose cbSizeVft=56 is just IDispatch).
    vtable_slot_bound: Option<u16>,
}

/// Cross from a dual `dispinterface` ITypeInfo to its FDUAL PARTNER
/// `TKIND_INTERFACE` ITypeInfo. The canonical crossing is
/// `GetRefTypeOfImplType(-1)` → `GetRefTypeInfo`; some authoring uses impl-type
/// index 0, so we fall back to 0. Returns the owned partner ITypeInfo (caller
/// Releases it). AV-free: pure ITypeInfo reads. (Ported from the probe's
/// `cross_to_partner_interface`.)
///
/// # Safety
/// `disp_ti` must be a live ITypeInfo pointer for the duration of the call.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
unsafe fn cross_to_partner_interface(disp_ti: *mut c_void) -> Option<*mut c_void> {
    if disp_ti.is_null() {
        return None;
    }
    let vtbl = *(disp_ti as *const *const ITypeInfoVtbl);
    // Try impl-type index -1 (u32::MAX bit pattern — the FDUAL partner) first,
    // then 0. An invalid index returns a failure HRESULT (no out-of-bounds read).
    for idx in [u32::MAX, 0u32] {
        let mut href: u32 = 0;
        let hr_ref = ((*vtbl).get_ref_type_of_impl_type)(disp_ti, idx, &mut href);
        if hr_ref != COM_S_OK {
            continue;
        }
        let mut partner: *mut c_void = null_mut();
        let hr_ti = ((*vtbl).get_ref_type_info)(disp_ti, href, &mut partner);
        if hr_ti == COM_S_OK && !partner.is_null() {
            return Some(partner);
        }
    }
    None
}

/// Read a member's `oVft` (FUNCDESC byte offset into the vtable) by name from an
/// ITypeInfo, matched case-insensitively. AV-free. (Ported from the probe's
/// `member_desc` oVft read.)
///
/// # Safety
/// `ptinfo` must be a live ITypeInfo pointer for the duration of the call.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
unsafe fn member_ovft_by_name(ptinfo: *mut c_void, name: &str) -> Option<i16> {
    if ptinfo.is_null() {
        return None;
    }
    let vtbl = *(ptinfo as *const *const ITypeInfoVtbl);
    let mut pattr: *mut TYPEATTR = null_mut();
    if ((*vtbl).get_type_attr)(ptinfo, &mut pattr) != COM_S_OK || pattr.is_null() {
        return None;
    }
    let cfuncs = (*pattr).cfuncs;
    ((*vtbl).release_type_attr)(ptinfo, pattr);

    let mut found = None;
    for fi in 0..u32::from(cfuncs) {
        let mut pfuncdesc: *mut FUNCDESC = null_mut();
        if ((*vtbl).get_func_desc)(ptinfo, fi, &mut pfuncdesc) != COM_S_OK || pfuncdesc.is_null() {
            continue;
        }
        let memid = (*pfuncdesc).memid;
        let ovft = (*pfuncdesc).oVft;
        // Member name via GetDocumentation(memid): name into the first out-arg.
        let mut pname: *mut u16 = null_mut();
        let hr_doc = ((*vtbl).get_documentation)(
            ptinfo,
            memid,
            &mut pname,
            null_mut(),
            null_mut(),
            null_mut(),
        );
        if hr_doc == COM_S_OK
            && let Some(member_name) = bstr_to_string_and_free(pname)
            && member_name.eq_ignore_ascii_case(name)
        {
            found = Some(ovft);
        }
        ((*vtbl).release_func_desc)(ptinfo, pfuncdesc);
        if found.is_some() {
            break;
        }
    }
    found
}

/// Find a member's `oVft` by name, searching (in order) the partner INTERFACE
/// itself, its base-interface chain (`GetRefTypeOfImplType(0)` → `GetRefTypeInfo`,
/// repeated), then the dispinterface face (which lists ALL members with their
/// oVft authored for the partner vtable). Inherited members like `Close`/`Value`
/// live on a BASE interface, not the leaf. AV-free. (Ported from the probe's
/// `member_desc_with_source`.)
///
/// # Safety
/// `partner_ti` and `disp_ti` must be live ITypeInfo pointers for the call.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
unsafe fn member_ovft_with_source(
    partner_ti: *mut c_void,
    disp_ti: *mut c_void,
    name: &str,
) -> Option<i16> {
    // 1. the partner INTERFACE's own funcs.
    if let Some(ovft) = member_ovft_by_name(partner_ti, name) {
        return Some(ovft);
    }
    // 2. walk the base-interface chain via impl-type index 0.
    let mut cur = partner_ti;
    let mut owned_chain: Vec<*mut c_void> = Vec::new();
    let mut result = None;
    let mut depth = 0u32;
    loop {
        depth += 1;
        if depth > 8 {
            break; // guard against cycles.
        }
        let vtbl = *(cur as *const *const ITypeInfoVtbl);
        let mut href: u32 = 0;
        if ((*vtbl).get_ref_type_of_impl_type)(cur, 0, &mut href) != COM_S_OK {
            break;
        }
        let mut base: *mut c_void = null_mut();
        if ((*vtbl).get_ref_type_info)(cur, href, &mut base) != COM_S_OK || base.is_null() {
            break;
        }
        owned_chain.push(base);
        if let Some(ovft) = member_ovft_by_name(base, name) {
            result = Some(ovft);
            break;
        }
        cur = base;
    }
    for ti in owned_chain {
        let ti_vtbl = *(ti as *const *const ITypeInfoVtbl);
        ((*ti_vtbl).release)(ti);
    }
    if result.is_some() {
        return result;
    }
    // 3. the dispinterface face lists ALL members with oVft authored for the
    //    partner vtable.
    member_ovft_by_name(disp_ti, name)
}

/// Cross a live FDUAL `dispinterface` typeinfo to its PARTNER `TKIND_INTERFACE`
/// and recover the facts needed to vtable-call `member_name`: the partner
/// INTERFACE IID (to QI for), the member's recovered vtable SLOT (`oVft / 8` from
/// the inherited chain), and the partner's `cbSizeVft`-derived AV-safety bound.
/// Returns `None` when the crossing fails, the member's oVft is not found on any
/// face, or the slot is malformed. AV-free.
///
/// # Safety
/// `disp_ti` must be a live ITypeInfo pointer for the duration of the call.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
unsafe fn partner_interface_slot_facts(
    disp_ti: *mut c_void,
    member_name: &str,
) -> Option<(u16, PartnerInterfaceFacts)> {
    let partner_ti = cross_to_partner_interface(disp_ti)?;
    // Read the partner INTERFACE's guid + cbSizeVft (the bound MUST come from the
    // partner, NOT the dispinterface whose cbSizeVft=56 is just IDispatch).
    let vtbl = *(partner_ti as *const *const ITypeInfoVtbl);
    let mut pattr: *mut TYPEATTR = null_mut();
    let attrs = if ((*vtbl).get_type_attr)(partner_ti, &mut pattr) == COM_S_OK && !pattr.is_null() {
        let iid = crate::ComInterfaceIid::from_guid(&(*pattr).guid);
        let bound = vtable_slot_bound_from_cb_size_vft((*pattr).cb_size_vft);
        ((*vtbl).release_type_attr)(partner_ti, pattr);
        Some((iid, bound))
    } else {
        None
    };
    // Resolve the member's oVft from whichever face describes it (partner
    // INTERFACE, its base chain, or the dispinterface face).
    let ovft = member_ovft_with_source(partner_ti, disp_ti, member_name);

    // Release the partner ITypeInfo (the base chain released itself inside the
    // resolver).
    let partner_vtbl = *(partner_ti as *const *const ITypeInfoVtbl);
    ((*partner_vtbl).release)(partner_ti);

    let (interface_iid, vtable_slot_bound) = attrs?;
    if interface_iid.is_null() {
        return None;
    }
    let slot = vtable_slot_index_from_ovft(ovft?)?;
    Some((
        slot,
        PartnerInterfaceFacts {
            interface_iid,
            vtable_slot_bound,
        },
    ))
}

/// Extract the FUNCDESC vtable signature for a single member of a LIVE COM
/// object, by asking the object for its own `ITypeInfo` (`IDispatch::GetTypeInfo`)
/// rather than a registered typelib. This is what lets early-bound member calls
/// on objects with no prog-id-resolvable typelib (Excel `Range`, DAO `Field`,
/// and every `::<invoke-result>` object in a member chain) still recover the
/// dual-interface vtable slot so the vtable fast path can fire.
///
/// Returns the matching member's metadata — including `vtable_slot`,
/// `parameter_types`, `return_type`, and `callconv_is_stdcall` — selected by
/// `dispid` (FUNCDESC `memid`) and, when several FUNCDESCs share a memid
/// (propget/propput pairs), the requested `invoke_kind`. `None` if the object
/// exposes no type info, the member is absent, or the lookup fails.
///
/// # Safety
/// `dispatch` must be a live `IDispatch` pointer held alive for the duration of
/// this call (the bindings map's retained reference satisfies this).
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pub unsafe fn live_member_metadata_from_dispatch(
    dispatch: *mut crate::windows_client::RawIDispatch,
    dispid: i32,
    invoke_kind: TypeLibMemberInvokeKind,
) -> Option<TypeLibMemberMetadata> {
    if dispatch.is_null() {
        return None;
    }
    // SAFETY: `dispatch` is a live IDispatch per this fn's `# Safety`; its first
    // field is the vtable, and GetTypeInfo writes either null (no type info,
    // S_FALSE/typeinfo-not-available) or one owned ITypeInfo reference.
    let ptinfo = unsafe {
        let vtbl = &*(*dispatch).vtbl;
        let mut ptinfo: *mut c_void = null_mut();
        let hr = (vtbl.get_type_info)(dispatch.cast::<c_void>(), 0, 0, &mut ptinfo);
        if hr != COM_S_OK || ptinfo.is_null() {
            return None;
        }
        ptinfo
    };
    // SAFETY: `ptinfo` is one owned ITypeInfo reference. We extract its members
    // and, when it is the FDUAL dispinterface face Office/DAO objects return from
    // GetTypeInfo(0), cross to the partner INTERFACE to recover a callable slot —
    // ALL while `ptinfo` is still live — then Release it exactly once.
    unsafe {
        let members = match extract_members_from_typeinfo(ptinfo) {
            Ok(members) => members,
            Err(_) => {
                let ti_vtbl = *(ptinfo as *const *const ITypeInfoVtbl);
                ((*ti_vtbl).release)(ptinfo);
                return None;
            }
        };
        // Prefer an exact (memid, invoke_kind) match; fall back to memid alone so
        // a member whose recorded kind differs (e.g. a method surfaced as a
        // property-get default) still resolves.
        let selected = members
            .iter()
            .find(|m| m.token == dispid && m.invoke_kind == invoke_kind)
            .or_else(|| members.iter().find(|m| m.token == dispid))
            .cloned();

        // FDUAL CROSSING (workset slice B): GetTypeInfo(0) returned the
        // DISPINTERFACE face, so the base extractor dropped the slot
        // (source_typekind == Dispatch). For a dual member, cross to the partner
        // INTERFACE to recover the real vtable slot + the partner IID to QI for +
        // the AV-safety bound from the partner's cbSizeVft. The probe proved this
        // is what makes an in-process slot call return the correct value (DAO
        // Field.Value oVft=136 → slot 17 → VT_I4(7)).
        let patched = selected.map(|mut member| {
            if member.is_dual
                && member.source_typekind == Some(crate::SourceTypeKind::Dispatch)
                && let Some((slot, facts)) = partner_interface_slot_facts(ptinfo, &member.name)
            {
                member.vtable_slot = Some(slot);
                member.interface_iid = Some(facts.interface_iid);
                member.vtable_slot_bound = facts.vtable_slot_bound;
                member.source_typekind = Some(crate::SourceTypeKind::Interface);
            }
            member
        });

        let ti_vtbl = *(ptinfo as *const *const ITypeInfoVtbl);
        ((*ti_vtbl).release)(ptinfo);
        patched
    }
}

#[cfg(target_os = "windows")]
unsafe fn extract_members_from_coclass_default_interface(
    coclass_ptinfo: *mut c_void,
) -> Result<Vec<TypeLibMemberMetadata>, String> {
    let vtbl = *(coclass_ptinfo as *const *const ITypeInfoVtbl);
    let mut pattr: *mut TYPEATTR = std::ptr::null_mut();
    let hr = ((*vtbl).get_type_attr)(coclass_ptinfo, &mut pattr);
    if hr != COM_S_OK || pattr.is_null() {
        return Err("ITypeInfo::GetTypeAttr failed for coclass".to_string());
    }
    let impl_count = (*pattr).cimpl_types as u32;
    ((*vtbl).release_type_attr)(coclass_ptinfo, pattr);

    let mut fallback_members: Option<Vec<TypeLibMemberMetadata>> = None;
    for impl_idx in 0..impl_count {
        let mut flags: i32 = 0;
        if ((*vtbl).get_impl_type_flags)(coclass_ptinfo, impl_idx, &mut flags) != COM_S_OK {
            continue;
        }
        if (flags & IMPLTYPEFLAG_FSOURCE) != 0 {
            continue;
        }

        let mut href: u32 = 0;
        if ((*vtbl).get_ref_type_of_impl_type)(coclass_ptinfo, impl_idx, &mut href) != COM_S_OK {
            continue;
        }

        let mut pref: *mut c_void = std::ptr::null_mut();
        if ((*vtbl).get_ref_type_info)(coclass_ptinfo, href, &mut pref) != COM_S_OK
            || pref.is_null()
        {
            continue;
        }

        let result = extract_members_from_typeinfo(pref);
        let release_vtbl = *(pref as *const *const ITypeInfoVtbl);
        ((*release_vtbl).release)(pref);

        let Ok(members) = result else {
            continue;
        };
        if (flags & IMPLTYPEFLAG_FDEFAULT) != 0 {
            return Ok(members);
        }
        if fallback_members.is_none() {
            fallback_members = Some(members);
        }
    }

    Ok(fallback_members.unwrap_or_default())
}

/// Enumerates event metadata from a loaded ITypeLib.
#[cfg(target_os = "windows")]
pub fn enumerate_typelib_events(ptlib: *mut c_void) -> Result<Vec<TypeLibEventMetadata>, String> {
    let mut events = Vec::new();
    // SAFETY: callers obtain `ptlib` from `load_typelib_from_registry`/
    // `load_typelib_from_path` and release it via `release_typelib` only after this
    // call returns, so it is a live ITypeLib*; COM guarantees an interface
    // pointer's first pointer-sized field is its vtable pointer, and ITypeLibVtbl
    // mirrors the oaidl.h vtable prefix.
    let vtbl = unsafe { *(ptlib as *const *const ITypeLibVtbl) };
    // SAFETY: `vtbl` was read from the live ITypeLib just above; GetTypeInfoCount
    // is a plain vtable call on that same interface pointer with no out-params.
    let count = unsafe { ((*vtbl).get_type_info_count)(ptlib) };

    for i in 0..count {
        let mut typekind: u32 = 0;
        // SAFETY: vtable call on the live ITypeLib; `typekind` is a live local
        // out-slot that the OS writes before returning.
        let hr = unsafe { ((*vtbl).get_type_info_type)(ptlib, i, &mut typekind) };
        if hr != COM_S_OK || typekind != TKIND_COCLASS {
            continue;
        }

        let mut ptinfo: *mut c_void = std::ptr::null_mut();
        // SAFETY: vtable call on the live ITypeLib; on S_OK the OS stores a
        // retained ITypeInfo* into the live `ptinfo` slot, and this loop iteration
        // releases that reference after use.
        let hr = unsafe { ((*vtbl).get_type_info)(ptlib, i, &mut ptinfo) };
        if hr != COM_S_OK || ptinfo.is_null() {
            continue;
        }

        // SAFETY: `ptinfo` was checked S_OK and non-null above, so it is the live
        // coclass ITypeInfo that `extract_events_from_coclass` requires; it stays
        // alive until the release block below.
        let result = unsafe { extract_events_from_coclass(ptinfo) };
        // SAFETY: `ptinfo` was obtained S_OK/non-null from GetTypeInfo above and is
        // still live; its first field is its vtable (ITypeInfoVtbl prefix per
        // oaidl.h). This Release balances GetTypeInfo's retained reference and the
        // pointer is not used afterward.
        unsafe {
            let ti_vtbl = *(ptinfo as *const *const ITypeInfoVtbl);
            ((*ti_vtbl).release)(ptinfo);
        }
        if let Ok(mut coclass_events) = result {
            events.append(&mut coclass_events);
        }
    }
    Ok(events)
}

#[cfg(target_os = "windows")]
pub fn enumerate_typelib_events_for_coclass(
    ptlib: *mut c_void,
    coclass_name: &str,
) -> Result<Vec<TypeLibEventMetadata>, String> {
    // SAFETY: callers obtain `ptlib` from `load_typelib_from_registry`/
    // `load_typelib_from_path` and release it via `release_typelib` only after this
    // call returns, so it is a live ITypeLib*; COM guarantees an interface
    // pointer's first pointer-sized field is its vtable pointer, and ITypeLibVtbl
    // mirrors the oaidl.h vtable prefix.
    let vtbl = unsafe { *(ptlib as *const *const ITypeLibVtbl) };
    // SAFETY: `vtbl` was read from the live ITypeLib just above; GetTypeInfoCount
    // is a plain vtable call on that same interface pointer with no out-params.
    let count = unsafe { ((*vtbl).get_type_info_count)(ptlib) };

    for i in 0..count {
        let mut typekind: u32 = 0;
        // SAFETY: vtable call on the live ITypeLib; `typekind` is a live local
        // out-slot that the OS writes before returning.
        let hr = unsafe { ((*vtbl).get_type_info_type)(ptlib, i, &mut typekind) };
        if hr != COM_S_OK || typekind != TKIND_COCLASS {
            continue;
        }

        let mut ptinfo: *mut c_void = std::ptr::null_mut();
        // SAFETY: vtable call on the live ITypeLib; on S_OK the OS stores a
        // retained ITypeInfo* into the live `ptinfo` slot, and this loop iteration
        // releases that reference after use.
        let hr = unsafe { ((*vtbl).get_type_info)(ptlib, i, &mut ptinfo) };
        if hr != COM_S_OK || ptinfo.is_null() {
            continue;
        }

        // SAFETY: `ptinfo` was checked S_OK and non-null above; `typeinfo_name`
        // only performs vtable calls on that live ITypeInfo and frees the BSTR it
        // receives.
        let is_match = unsafe { typeinfo_name(ptinfo) }
            .is_some_and(|name| name.eq_ignore_ascii_case(coclass_name));
        let result = if is_match {
            // SAFETY: `ptinfo` is the live coclass ITypeInfo checked S_OK/non-null
            // above; it stays alive until the release block below.
            unsafe { extract_events_from_coclass(ptinfo) }
        } else {
            Ok(Vec::new())
        };
        // SAFETY: `ptinfo` was obtained S_OK/non-null from GetTypeInfo above and is
        // still live; its first field is its vtable (ITypeInfoVtbl prefix per
        // oaidl.h). This Release balances GetTypeInfo's retained reference and the
        // pointer is not used afterward.
        unsafe {
            let ti_vtbl = *(ptinfo as *const *const ITypeInfoVtbl);
            ((*ti_vtbl).release)(ptinfo);
        }
        if is_match {
            return result;
        }
    }

    Ok(Vec::new())
}

/// Extracts event metadata from a coclass ITypeInfo by walking source interfaces.
#[cfg(target_os = "windows")]
unsafe fn extract_events_from_coclass(
    ptinfo: *mut c_void,
) -> Result<Vec<TypeLibEventMetadata>, String> {
    let vtbl = *(ptinfo as *const *const ITypeInfoVtbl);
    let mut pattr: *mut TYPEATTR = std::ptr::null_mut();
    let hr = ((*vtbl).get_type_attr)(ptinfo, &mut pattr);
    if hr != COM_S_OK || pattr.is_null() {
        return Ok(Vec::new());
    }

    let impl_count = (*pattr).cimpl_types;
    ((*vtbl).release_type_attr)(ptinfo, pattr);

    let mut events = Vec::new();
    for impl_idx in 0..impl_count as u32 {
        let mut impl_flags: i32 = 0;
        let hr = ((*vtbl).get_impl_type_flags)(ptinfo, impl_idx, &mut impl_flags);
        if hr != COM_S_OK {
            continue;
        }

        // Check if this is a source interface (event source)
        if (impl_flags & IMPLTYPEFLAG_FSOURCE) == 0 {
            continue;
        }

        let mut href: u32 = 0;
        let hr = ((*vtbl).get_ref_type_of_impl_type)(ptinfo, impl_idx, &mut href);
        if hr != COM_S_OK {
            continue;
        }

        let mut ref_info: *mut c_void = std::ptr::null_mut();
        let hr = ((*vtbl).get_ref_type_info)(ptinfo, href, &mut ref_info);
        if hr != COM_S_OK || ref_info.is_null() {
            continue;
        }

        // Get the source interface IID and typekind. The typekind selects the event
        // dispatch path: a `dispinterface` source (TKIND_DISPATCH — the shape Office
        // event interfaces and our dispinterface fixtures use) delivers events as
        // late-bound IDispatch::Invoke calls on the sink, so it takes the fully
        // functional `Dispatch` path (any arity). A true vtable source interface
        // (TKIND_INTERFACE) takes the `SourceInterface` path. Defaulting an unknown
        // kind to `Dispatch` keeps the arity-flexible path rather than the
        // arity-1-only `SourceInterface` one.
        let ref_vtbl = *(ref_info as *const *const ITypeInfoVtbl);
        let mut ref_attr: *mut TYPEATTR = std::ptr::null_mut();
        let hr = ((*ref_vtbl).get_type_attr)(ref_info, &mut ref_attr);
        let (iid, dispatch_path) = if hr == COM_S_OK && !ref_attr.is_null() {
            let iid_str = guid_to_string(&(*ref_attr).guid);
            let typekind = (*ref_attr).typekind;
            ((*ref_vtbl).release_type_attr)(ref_info, ref_attr);
            (Some(iid_str), source_dispatch_path_for_typekind(typekind))
        } else {
            (None, TypeLibEventDispatchPath::Dispatch)
        };

        // Walk the source interface functions as events
        let source_members = extract_members_from_typeinfo(ref_info);
        ((*ref_vtbl).release)(ref_info);

        if let Ok(source_members) = source_members {
            for member in source_members {
                events.push(TypeLibEventMetadata {
                    name: member.name,
                    token: member.token,
                    callback_arity: member.parameter_names.len() as u8,
                    dispatch_path,
                    connection_point_iid: iid.clone(),
                    dispatch_member_id: Some(member.token),
                });
            }
        }
    }

    Ok(events)
}

/// The event dispatch path implied by a source interface's `TYPEKIND`. A
/// `dispinterface` source (`TKIND_DISPATCH`) delivers events via late-bound
/// `IDispatch::Invoke` on the sink → the fully functional `Dispatch` path (any
/// arity). A true vtable source interface (`TKIND_INTERFACE`) → the
/// `SourceInterface` path. Any other kind falls back to `Dispatch` (the
/// arity-flexible path) rather than the arity-1-only `SourceInterface`.
#[cfg(target_os = "windows")]
fn source_dispatch_path_for_typekind(typekind: u32) -> TypeLibEventDispatchPath {
    match typekind {
        TKIND_INTERFACE => TypeLibEventDispatchPath::SourceInterface,
        _ => TypeLibEventDispatchPath::Dispatch,
    }
}

/// Extracts the ProgID for a CoClass from the typelib (for As New support).
#[cfg(target_os = "windows")]
pub fn extract_coclass_prog_id(ptlib: *mut c_void) -> Option<String> {
    // SAFETY: callers obtain `ptlib` from `load_typelib_from_registry`/
    // `load_typelib_from_path` and release it via `release_typelib` only after this
    // call returns, so it is a live ITypeLib*; COM guarantees an interface
    // pointer's first pointer-sized field is its vtable pointer, and ITypeLibVtbl
    // mirrors the oaidl.h vtable prefix.
    let vtbl = unsafe { *(ptlib as *const *const ITypeLibVtbl) };
    // SAFETY: `vtbl` was read from the live ITypeLib just above; GetTypeInfoCount
    // is a plain vtable call on that same interface pointer with no out-params.
    let count = unsafe { ((*vtbl).get_type_info_count)(ptlib) };

    for i in 0..count {
        let mut typekind: u32 = 0;
        // SAFETY: vtable call on the live ITypeLib; `typekind` is a live local
        // out-slot that the OS writes before returning.
        let hr = unsafe { ((*vtbl).get_type_info_type)(ptlib, i, &mut typekind) };
        if hr != COM_S_OK || typekind != TKIND_COCLASS {
            continue;
        }

        // Get the CoClass name — this is often the ProgID (e.g., "Dictionary" for Scripting.Dictionary)
        let mut name_bstr: *mut u16 = std::ptr::null_mut();
        // SAFETY: vtable call on the live ITypeLib with a valid type index
        // (`i < count`); `name_bstr` is a live out-slot. On S_OK the OS allocates a
        // BSTR that we then own and free via `bstr_to_string_and_free`.
        let hr = unsafe {
            ((*vtbl).get_documentation)(
                ptlib,
                i as i32,
                &mut name_bstr,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if hr == COM_S_OK {
            // SAFETY: GetDocumentation returned S_OK, so `name_bstr` is either
            // null or a BSTR we now own; the helper frees it exactly once.
            let name = unsafe { bstr_to_string_and_free(name_bstr) };
            // Get the library name for constructing the full ProgID
            let mut lib_name_bstr: *mut u16 = std::ptr::null_mut();
            // SAFETY: vtable call on the live ITypeLib; index -1 is MEMBERID_NIL
            // (library-level documentation) and `lib_name_bstr` is a live out-slot
            // whose BSTR, if any, is freed just below.
            let _ = unsafe {
                ((*vtbl).get_documentation)(
                    ptlib,
                    -1,
                    &mut lib_name_bstr,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            // SAFETY: `lib_name_bstr` is either null or a BSTR we own from the
            // GetDocumentation call above; the helper frees it exactly once.
            let lib_name = unsafe { bstr_to_string_and_free(lib_name_bstr) };
            if let (Some(lib), Some(cls)) = (lib_name, name) {
                return Some(format!("{}.{}", lib, cls));
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
pub fn extract_coclass_prog_id_for_name(ptlib: *mut c_void, coclass_name: &str) -> Option<String> {
    // SAFETY: callers obtain `ptlib` from `load_typelib_from_registry`/
    // `load_typelib_from_path` and release it via `release_typelib` only after this
    // call returns, so it is a live ITypeLib*; COM guarantees an interface
    // pointer's first pointer-sized field is its vtable pointer, and ITypeLibVtbl
    // mirrors the oaidl.h vtable prefix.
    let vtbl = unsafe { *(ptlib as *const *const ITypeLibVtbl) };
    // SAFETY: `vtbl` was read from the live ITypeLib just above; GetTypeInfoCount
    // is a plain vtable call on that same interface pointer with no out-params.
    let count = unsafe { ((*vtbl).get_type_info_count)(ptlib) };

    for i in 0..count {
        let mut typekind: u32 = 0;
        // SAFETY: vtable call on the live ITypeLib; `typekind` is a live local
        // out-slot that the OS writes before returning.
        let hr = unsafe { ((*vtbl).get_type_info_type)(ptlib, i, &mut typekind) };
        if hr != COM_S_OK || typekind != TKIND_COCLASS {
            continue;
        }

        let mut name_bstr: *mut u16 = std::ptr::null_mut();
        // SAFETY: vtable call on the live ITypeLib with a valid type index
        // (`i < count`); `name_bstr` is a live out-slot. On S_OK the OS allocates a
        // BSTR that we then own and free via `bstr_to_string_and_free`.
        let hr = unsafe {
            ((*vtbl).get_documentation)(
                ptlib,
                i as i32,
                &mut name_bstr,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if hr != COM_S_OK {
            continue;
        }

        // SAFETY: GetDocumentation returned S_OK, so `name_bstr` is either null or
        // a BSTR we now own; the helper frees it exactly once.
        let Some(name) = (unsafe { bstr_to_string_and_free(name_bstr) }) else {
            continue;
        };
        if !name.eq_ignore_ascii_case(coclass_name) {
            continue;
        }

        let mut lib_name_bstr: *mut u16 = std::ptr::null_mut();
        // SAFETY: vtable call on the live ITypeLib; index -1 is MEMBERID_NIL
        // (library-level documentation) and `lib_name_bstr` is a live out-slot
        // whose BSTR, if any, is freed just below.
        let _ = unsafe {
            ((*vtbl).get_documentation)(
                ptlib,
                -1,
                &mut lib_name_bstr,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        // SAFETY: `lib_name_bstr` is either null or a BSTR we own from the
        // GetDocumentation call above; the helper frees it exactly once.
        let lib_name = unsafe { bstr_to_string_and_free(lib_name_bstr) };
        if let Some(lib) = lib_name {
            return Some(format!("{}.{}", lib, name));
        }
    }

    None
}

/// Builds a full TypeLibMetadataBlob from a loaded ITypeLib pointer.
#[cfg(target_os = "windows")]
pub fn build_metadata_blob_from_typelib(
    ptlib: *mut c_void,
    identity: TypeLibResolvedIdentity,
) -> Result<TypeLibMetadataBlob, String> {
    let members = if let Some(coclass_name) = requested_coclass_name(&identity) {
        let scoped = enumerate_typelib_members_for_coclass(ptlib, coclass_name)?;
        if !scoped.is_empty() {
            scoped
        } else {
            // The requested type is not a coclass. Before giving up to a whole-library
            // flatten (which collides shared member names across unrelated objects),
            // scope to the dispinterface/interface of the same name (DAO Database/Recordset).
            let interface_scoped = enumerate_typelib_members_for_interface(ptlib, coclass_name)?;
            if !interface_scoped.is_empty() {
                interface_scoped
            } else {
                enumerate_typelib_members(ptlib)?
            }
        }
    } else {
        enumerate_typelib_members(ptlib)?
    };
    let events = if let Some(coclass_name) = requested_coclass_name(&identity) {
        let scoped = enumerate_typelib_events_for_coclass(ptlib, coclass_name)?;
        if scoped.is_empty() {
            enumerate_typelib_events(ptlib)?
        } else {
            scoped
        }
    } else {
        enumerate_typelib_events(ptlib)?
    };
    let activation_prog_id = if let Some(coclass_name) = requested_coclass_name(&identity) {
        extract_coclass_prog_id_for_name(ptlib, coclass_name)
            .or_else(|| Some(identity.reference_name.clone()))
    } else {
        extract_coclass_prog_id(ptlib)
    };
    let member_name_to_token: Vec<(String, i32)> =
        members.iter().map(|m| (m.name.clone(), m.token)).collect();

    Ok(TypeLibMetadataBlob {
        identity,
        activation_prog_id,
        member_name_to_token,
        members,
        events,
    })
}

/// Releases an ITypeLib pointer obtained from `load_typelib_from_registry` or `load_typelib_from_path`.
#[cfg(target_os = "windows")]
/// # Safety
/// `ptlib` must be a live `ITypeLib*` previously obtained from this module's load helpers and
/// not yet released. Passing any other pointer, or releasing the same pointer twice, is invalid.
pub unsafe fn release_typelib(ptlib: *mut c_void) {
    if !ptlib.is_null() {
        let vtbl = *(ptlib as *const *const ITypeLibVtbl);
        ((*vtbl).release)(ptlib);
    }
}

#[cfg(not(target_os = "windows"))]
pub unsafe fn release_typelib(_ptlib: *mut std::ffi::c_void) {}

// ── Non-Windows stubs ──

#[cfg(not(target_os = "windows"))]
pub fn resolve_typelib_identity_from_registry(
    request: &crate::typelib::TypeLibResolveRequest,
) -> Result<TypeLibResolvedIdentity, String> {
    Err(format!(
        "live typelib loading not available on this platform for `{}`",
        request.reference_name
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn resolve_typelib_identity_from_prog_id(
    prog_id_name: &str,
) -> Result<TypeLibResolvedIdentity, String> {
    Err(format!(
        "live ProgID typelib resolution not available on this platform for `{}`",
        prog_id_name.trim()
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn discover_registered_typelib_identities_by_name(
    reference_name: &str,
) -> Result<Vec<TypeLibResolvedIdentity>, String> {
    Err(format!(
        "registered typelib discovery not available on this platform for `{}`",
        reference_name.trim()
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn enumerate_typelib_members(
    _ptlib: *mut std::ffi::c_void,
) -> Result<Vec<TypeLibMemberMetadata>, String> {
    Err("live typelib loading not available on this platform".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn enumerate_typelib_events(
    _ptlib: *mut std::ffi::c_void,
) -> Result<Vec<TypeLibEventMetadata>, String> {
    Err("live typelib loading not available on this platform".to_string())
}

// ── Non-Windows type stubs ──

#[cfg(not(target_os = "windows"))]
use crate::typelib::{TypeLibEventMetadata, TypeLibMemberMetadata, TypeLibResolvedIdentity};

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;
    use crate::typelib::TypeLibResolveRequest;

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn vtable_slot_index_uses_live_pointer_stride() {
        // THE ROOT-CAUSE FIX: slot = oVft / size_of::<*const c_void>() (= /8 on
        // x64), the LIVE pointer-size stride oleaut reports oVft in — NOT the
        // typelib's authored syskind granularity. Value-oracle evidence from
        // com_vtable_probe.rs (live ACE DAO):
        //   Field.Value     oVft=136 → slot 17 (returned VT_I4(7), MATCH)
        //   Recordset member oVft=392 → slot 49
        //   another member   oVft=360 → slot 45
        assert_eq!(vtable_slot_index_from_ovft(136), Some(17));
        assert_eq!(vtable_slot_index_from_ovft(392), Some(49));
        assert_eq!(vtable_slot_index_from_ovft(360), Some(45));
        // Slot 7 (first dual custom slot after the 7 IUnknown+IDispatch slots) is
        // oVft 56; slot 9 is oVft 72.
        assert_eq!(vtable_slot_index_from_ovft(56), Some(7));
        assert_eq!(vtable_slot_index_from_ovft(72), Some(9));
        // Malformed / mis-aligned offsets carry no usable slot.
        assert_eq!(vtable_slot_index_from_ovft(-8), None);
        assert_eq!(vtable_slot_index_from_ovft(6), None);
    }

    #[test]
    fn load_scrrun_typelib_from_registry() {
        // Scripting Runtime LIBID: {420B2830-E718-11CF-893D-00A0C9054228}
        let guid = windows_sys::core::GUID {
            data1: 0x420B_2830,
            data2: 0xE718,
            data3: 0x11CF,
            data4: [0x89, 0x3D, 0x00, 0xA0, 0xC9, 0x05, 0x42, 0x28],
        };
        let result = load_typelib_from_registry(&guid, 1, 0, 0);
        if let Ok(ptlib) = result {
            let members = enumerate_typelib_members(ptlib);
            assert!(members.is_ok(), "should enumerate scrrun members");
            let members = members.unwrap();
            assert!(!members.is_empty(), "scrrun should have members");
            // Look for Dictionary.Add
            let has_add = members.iter().any(|m| m.name == "Add");
            assert!(has_add, "scrrun should have Add member");
            // SAFETY: `ptlib` came from `load_typelib_from_registry` above and is
            // released exactly once here, satisfying `release_typelib`'s contract.
            unsafe { release_typelib(ptlib) };
        }
        // It's OK if this fails on systems without scrrun registered
    }

    #[test]
    fn resolve_typelib_identity_with_libid() {
        let request = TypeLibResolveRequest {
            reference_name: "Scripting".to_string(),
            requested_coclass: None,
            importlib_hint: None,
            libid_hint: Some("{420B2830-E718-11CF-893D-00A0C9054228}".to_string()),
            major_version_hint: Some(1),
            minor_version_hint: Some(0),
            lcid_hint: Some(0),
        };
        let result = resolve_typelib_identity_from_registry(&request);
        if let Ok(identity) = result {
            assert_eq!(identity.reference_name, "Scripting");
            assert!(identity.libid.is_some());
        }
        // It's OK if this fails on systems without scrrun
    }

    #[test]
    fn discover_registered_typelib_identities_by_name_accepts_ole_automation() {
        let result = discover_registered_typelib_identities_by_name("OLE Automation");
        if let Ok(matches) = result {
            assert!(
                matches.iter().any(|identity| identity.libid.as_deref()
                    == Some("{00020430-0000-0000-C000-000000000046}")),
                "expected stdole registry discovery to include the stdole LIBID"
            );
        }
        // It's OK if this fails on hosts where stdole registry data is unavailable.
    }

    #[test]
    fn source_dispatch_path_classifies_typekind() {
        // A dispinterface source (TKIND_DISPATCH — Office event interfaces and our
        // dispinterface fixtures) → the arity-flexible Dispatch path.
        assert_eq!(
            source_dispatch_path_for_typekind(TKIND_DISPATCH),
            TypeLibEventDispatchPath::Dispatch
        );
        // A true vtable source interface → the SourceInterface path.
        assert_eq!(
            source_dispatch_path_for_typekind(TKIND_INTERFACE),
            TypeLibEventDispatchPath::SourceInterface
        );
        // Any other kind falls back to the arity-flexible Dispatch path.
        assert_eq!(
            source_dispatch_path_for_typekind(TKIND_COCLASS),
            TypeLibEventDispatchPath::Dispatch
        );
    }
}
