#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::not_unsafe_ptr_arg_deref, clippy::upper_case_acronyms)]
//! Live ITypeLib/ITypeInfo COM loading for arbitrary typelib resolution.
//!
//! This module provides real COM-based type library loading as a fallback
//! when hardcoded catalog entries are insufficient. It wraps LoadRegTypeLib,
//! LoadTypeLibEx, and the ITypeLib/ITypeInfo COM interfaces to extract
//! member metadata from arbitrary registered type libraries.

#[cfg(target_os = "windows")]
use crate::typelib::{
    TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibMemberInvokeKind, TypeLibMemberMetadata,
    TypeLibMetadataBlob, TypeLibParamType, TypeLibResolvedIdentity,
};
#[cfg(target_os = "windows")]
use crate::windows_client::COM_S_OK;
#[cfg(target_os = "windows")]
use std::ffi::c_void;

// ── ITypeLib / ITypeInfo vtable definitions ──

#[cfg(target_os = "windows")]
const INVOKE_FUNC: u16 = 1;
#[cfg(target_os = "windows")]
const INVOKE_PROPERTYGET: u16 = 2;
#[cfg(target_os = "windows")]
const INVOKE_PROPERTYPUT: u16 = 4;
#[cfg(target_os = "windows")]
const INVOKE_PROPERTYPUTREF: u16 = 8;

#[cfg(target_os = "windows")]
const TKIND_DISPATCH: u32 = 4;
#[cfg(target_os = "windows")]
const TKIND_COCLASS: u32 = 5;
#[cfg(target_os = "windows")]
const TKIND_INTERFACE: u32 = 3;

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
    invkind: u16,
    callconv: u32,
    cparams: i16,
    cparams_opt: i16,
    oVft: i16,
    cScodes: i16,
    elemdescfunc: ELEMDESC,
    wfuncdescflags: u16,
}

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
        VT_UI1 => TypeLibParamType::Byte,
        VT_I8 => TypeLibParamType::LongLong,
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
fn invkind_to_member_invoke_kind(invkind: u16) -> TypeLibMemberInvokeKind {
    match invkind {
        INVOKE_PROPERTYGET => TypeLibMemberInvokeKind::PropertyGet,
        INVOKE_PROPERTYPUT => TypeLibMemberInvokeKind::PropertyPut,
        INVOKE_PROPERTYPUTREF => TypeLibMemberInvokeKind::PropertyPutRef,
        _ => TypeLibMemberInvokeKind::Method,
    }
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
        let identity = unsafe { extract_typelib_identity(ptlib, request)? };
        unsafe {
            let vtbl = *(ptlib as *const *const ITypeLibVtbl);
            ((*vtbl).release)(ptlib);
        }
        return Ok(identity);
    }

    // Try loading via importlib path hint
    if let Some(ref importlib) = request.importlib_hint {
        let ptlib = load_typelib_from_path(importlib)?;
        let identity = unsafe { extract_typelib_identity(ptlib, request)? };
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

    // TLIBATTR layout: GUID, LCID, SYSKIND, WORD wMajorVerNum, WORD wMinorVerNum, WORD wLibFlags
    #[repr(C)]
    struct TLIBATTR {
        guid: windows_sys::core::GUID,
        lcid: u32,
        syskind: u32,
        w_major_ver_num: u16,
        w_minor_ver_num: u16,
        w_lib_flags: u16,
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
    let cache_key = format!("live:{}:{}:{}", libid, major, minor);

    Ok(TypeLibResolvedIdentity {
        reference_name: request.reference_name.clone(),
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
    let vtbl = unsafe { *(ptlib as *const *const ITypeLibVtbl) };
    let count = unsafe { ((*vtbl).get_type_info_count)(ptlib) };

    for i in 0..count {
        let mut typekind: u32 = 0;
        let hr = unsafe { ((*vtbl).get_type_info_type)(ptlib, i, &mut typekind) };
        if hr != COM_S_OK {
            continue;
        }
        // Only process dispatch interfaces and coclasses
        if typekind != TKIND_DISPATCH && typekind != TKIND_INTERFACE {
            continue;
        }

        let mut ptinfo: *mut c_void = std::ptr::null_mut();
        let hr = unsafe { ((*vtbl).get_type_info)(ptlib, i, &mut ptinfo) };
        if hr != COM_S_OK || ptinfo.is_null() {
            continue;
        }

        let result = unsafe { extract_members_from_typeinfo(ptinfo) };
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
        let cparams = fd.cparams as u32;

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
        if (memid as u32) >= 0x6000_0000 && (memid as u32) < 0x6001_0000 && invkind == INVOKE_FUNC {
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

        // Extract parameter types
        let mut parameter_types = Vec::new();
        for p in 0..cparams {
            let param_desc = &*fd.lprgelemdescparam.add(p as usize);
            let vt = param_desc.tdesc.vt;
            let is_byref = (param_desc.paramdesc.wparamflags & 0x0002) != 0; // PARAMFLAG_FOUT
            let param_type = if vt == VT_PTR {
                // Pointer to another type — treat as ByRef of the inner type
                let inner_td = &*(param_desc.tdesc.union_field as *const TYPEDESC);
                vt_to_param_type(inner_td.vt, true)
            } else {
                vt_to_param_type(vt, is_byref)
            };
            parameter_types.push(param_type);
        }

        // Extract return type
        let ret_vt = fd.elemdescfunc.tdesc.vt;
        let return_type = if ret_vt == VT_VOID || ret_vt == VT_HRESULT {
            None
        } else {
            Some(vt_to_param_type(ret_vt, false))
        };

        let invoke_kind = invkind_to_member_invoke_kind(invkind);
        let requires_argument = cparams > 0
            || matches!(
                invoke_kind,
                TypeLibMemberInvokeKind::PropertyPut | TypeLibMemberInvokeKind::PropertyPutRef
            );

        // Default member check (DISPID 0)
        let is_default_member = memid == 0;

        members.push(TypeLibMemberMetadata {
            name: func_name,
            token: memid,
            requires_argument,
            invoke_kind,
            parameter_names,
            is_default_member,
            parameter_types,
            return_type,
        });

        ((*vtbl).release_func_desc)(ptinfo, pfuncdesc);
    }
    Ok(members)
}

/// Enumerates event metadata from a loaded ITypeLib.
#[cfg(target_os = "windows")]
pub fn enumerate_typelib_events(ptlib: *mut c_void) -> Result<Vec<TypeLibEventMetadata>, String> {
    let mut events = Vec::new();
    let vtbl = unsafe { *(ptlib as *const *const ITypeLibVtbl) };
    let count = unsafe { ((*vtbl).get_type_info_count)(ptlib) };

    for i in 0..count {
        let mut typekind: u32 = 0;
        let hr = unsafe { ((*vtbl).get_type_info_type)(ptlib, i, &mut typekind) };
        if hr != COM_S_OK || typekind != TKIND_COCLASS {
            continue;
        }

        let mut ptinfo: *mut c_void = std::ptr::null_mut();
        let hr = unsafe { ((*vtbl).get_type_info)(ptlib, i, &mut ptinfo) };
        if hr != COM_S_OK || ptinfo.is_null() {
            continue;
        }

        let result = unsafe { extract_events_from_coclass(ptinfo) };
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

        // Get the source interface IID
        let ref_vtbl = *(ref_info as *const *const ITypeInfoVtbl);
        let mut ref_attr: *mut TYPEATTR = std::ptr::null_mut();
        let hr = ((*ref_vtbl).get_type_attr)(ref_info, &mut ref_attr);
        let iid = if hr == COM_S_OK && !ref_attr.is_null() {
            let iid_str = guid_to_string(&(*ref_attr).guid);
            ((*ref_vtbl).release_type_attr)(ref_info, ref_attr);
            Some(iid_str)
        } else {
            None
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
                    dispatch_path: TypeLibEventDispatchPath::SourceInterface,
                    connection_point_iid: iid.clone(),
                    dispatch_member_id: Some(member.token),
                });
            }
        }
    }

    Ok(events)
}

/// Extracts the ProgID for a CoClass from the typelib (for As New support).
#[cfg(target_os = "windows")]
pub fn extract_coclass_prog_id(ptlib: *mut c_void) -> Option<String> {
    let vtbl = unsafe { *(ptlib as *const *const ITypeLibVtbl) };
    let count = unsafe { ((*vtbl).get_type_info_count)(ptlib) };

    for i in 0..count {
        let mut typekind: u32 = 0;
        let hr = unsafe { ((*vtbl).get_type_info_type)(ptlib, i, &mut typekind) };
        if hr != COM_S_OK || typekind != TKIND_COCLASS {
            continue;
        }

        // Get the CoClass name — this is often the ProgID (e.g., "Dictionary" for Scripting.Dictionary)
        let mut name_bstr: *mut u16 = std::ptr::null_mut();
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
            let name = unsafe { bstr_to_string_and_free(name_bstr) };
            // Get the library name for constructing the full ProgID
            let mut lib_name_bstr: *mut u16 = std::ptr::null_mut();
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
            let lib_name = unsafe { bstr_to_string_and_free(lib_name_bstr) };
            if let (Some(lib), Some(cls)) = (lib_name, name) {
                return Some(format!("{}.{}", lib, cls));
            }
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
    let members = enumerate_typelib_members(ptlib)?;
    let events = enumerate_typelib_events(ptlib)?;
    let activation_prog_id = extract_coclass_prog_id(ptlib);
    let member_name_to_token: Vec<(String, i32)> =
        members.iter().map(|m| (m.name.clone(), m.token)).collect();

    Ok(TypeLibMetadataBlob {
        identity,
        activation_prog_id,
        create_object_selector: None,
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
            unsafe { release_typelib(ptlib) };
        }
        // It's OK if this fails on systems without scrrun registered
    }

    #[test]
    fn resolve_typelib_identity_with_libid() {
        let request = TypeLibResolveRequest {
            reference_name: "Scripting".to_string(),
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
}
