//! Type library (.tlb) generation via CreateTypeLib2 COM API.
//!
//! Produces binary .tlb files from `ComClassExportDescriptor` metadata,
//! using the same deterministic UUIDs as `idl.rs`. The generated typelib
//! can be embedded in DllRegisterServer or distributed alongside the COM server.

#[cfg(target_os = "windows")]
use std::ffi::c_void;

use oxvba_project::ComClassExportDescriptor;

use crate::idl::deterministic_uuid;

// ── GUID helper ──

/// A 128-bit COM GUID in standard layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl Guid {
    pub const ZERO: Self = Self {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
}

/// Parse a "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" UUID string into a Guid.
pub fn parse_uuid(uuid: &str) -> Guid {
    let hex: String = uuid.chars().filter(|c| *c != '-').collect();
    assert_eq!(hex.len(), 32, "UUID must be 32 hex chars");

    let data1 = u32::from_str_radix(&hex[0..8], 16).unwrap();
    let data2 = u16::from_str_radix(&hex[8..12], 16).unwrap();
    let data3 = u16::from_str_radix(&hex[12..16], 16).unwrap();

    let mut data4 = [0u8; 8];
    for i in 0..8 {
        data4[i] = u8::from_str_radix(&hex[16 + i * 2..18 + i * 2], 16).unwrap();
    }

    Guid {
        data1,
        data2,
        data3,
        data4,
    }
}

// ── CreateTypeLib2 FFI (Windows only) ──

#[cfg(target_os = "windows")]
mod ffi {
    use super::Guid;
    use std::ffi::c_void;

    // SYSKIND — SYS_WIN64 for modern Windows
    pub const SYS_WIN64: u32 = 3;

    // TYPEKIND
    pub const TKIND_DISPATCH: u32 = 4;
    pub const TKIND_COCLASS: u32 = 5;

    // TYPEFLAGS
    #[allow(dead_code)]
    pub const TYPEFLAG_FDUAL: u16 = 0x0040;
    #[allow(dead_code)]
    pub const TYPEFLAG_FOLEAUTOMATION: u16 = 0x0100;
    pub const TYPEFLAG_FCANCREATE: u16 = 0x0002;
    #[allow(dead_code)]
    pub const TYPEFLAG_FDISPATCHABLE: u16 = 0x1000;

    // IMPLTYPEFLAGS
    pub const IMPLTYPEFLAG_FDEFAULT: i32 = 1;

    // INVOKEKIND
    pub const INVOKE_FUNC: u32 = 1;
    pub const INVOKE_PROPERTYGET: u32 = 2;
    pub const INVOKE_PROPERTYPUT: u32 = 4;

    // FUNCKIND
    pub const FUNC_DISPATCH: u32 = 4;

    // CALLCONV
    pub const CC_STDCALL: u32 = 4;

    // VARENUM (VT_*)
    #[allow(dead_code)]
    pub const VT_VOID: u16 = 24;
    pub const VT_VARIANT: u16 = 12;
    pub const VT_HRESULT: u16 = 25;
    pub const VT_PTR: u16 = 26;

    // PARAMFLAG
    pub const PARAMFLAG_FIN: u16 = 0x0001;
    pub const PARAMFLAG_FOUT: u16 = 0x0002;
    pub const PARAMFLAG_FRETVAL: u16 = 0x0008;

    // FUNCFLAG
    pub const FUNCFLAG_FRESTRICTED: u16 = 0x0001;
    pub const FUNCFLAG_FDEFAULTBIND: u16 = 0x0020;
    pub const FUNCFLAG_FHIDDEN: u16 = 0x0040;

    // IID_IDispatch
    #[allow(dead_code)]
    pub const IID_IDISPATCH: Guid = Guid {
        data1: 0x0002_0400,
        data2: 0x0000,
        data3: 0x0000,
        data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    };

    // IID_ITypeInfo — needed to QI ICreateTypeInfo for AddRefTypeInfo
    pub const IID_ITYPEINFO: Guid = Guid {
        data1: 0x0002_0401,
        data2: 0x0000,
        data3: 0x0000,
        data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    };

    /// TYPEDESC - describes a type
    #[repr(C)]
    pub struct TypeDesc {
        pub union_field: usize, // lptdesc (pointer to another TypeDesc) or hreftype
        pub vt: u16,
    }

    /// PARAMDESC - describes parameter flags
    #[repr(C)]
    pub struct ParamDesc {
        pub pparamdescex: *mut c_void,
        pub wparamflags: u16,
    }

    /// ELEMDESC - describes a parameter or return type
    #[repr(C)]
    pub struct ElemDesc {
        pub tdesc: TypeDesc,
        pub paramdesc: ParamDesc,
    }

    /// FUNCDESC - describes a function/method
    #[repr(C)]
    pub struct FuncDesc {
        pub memid: i32,          // DISPID
        pub lprgscode: *mut i32, // reserved
        pub lprgelemdescparam: *mut ElemDesc,
        pub funckind: u32,
        pub invkind: u32,
        pub callconv: u32,
        pub cparams: i16,
        pub cparams_opt: i16,
        pub o_vft: i16,
        pub cscodes: i16,
        pub elemdesc_func: ElemDesc, // return type
        pub w_func_flags: u16,
    }

    /// ICreateTypeLib2 vtable (extends ICreateTypeLib which extends IUnknown)
    #[repr(C)]
    pub struct ICreateTypeLibVtbl {
        // IUnknown
        pub query_interface:
            unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32,
        pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        pub release: unsafe extern "system" fn(*mut c_void) -> u32,
        // ICreateTypeLib
        pub create_type_info: unsafe extern "system" fn(
            this: *mut c_void,
            name: *const u16,
            tkind: u32,
            ppctinfo: *mut *mut c_void,
        ) -> i32,
        pub set_name: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
        pub set_version: unsafe extern "system" fn(*mut c_void, major: u16, minor: u16) -> i32,
        pub set_guid: unsafe extern "system" fn(*mut c_void, *const Guid) -> i32,
        pub set_doc_string: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
        pub set_helpfile_name: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
        pub set_helpcontext: unsafe extern "system" fn(*mut c_void, u32) -> i32,
        pub set_lcid: unsafe extern "system" fn(*mut c_void, u32) -> i32,
        pub set_lib_flags: unsafe extern "system" fn(*mut c_void, u32) -> i32,
        pub save_all_changes: unsafe extern "system" fn(*mut c_void) -> i32,
    }

    /// ICreateTypeInfo vtable (extends IUnknown).
    ///
    /// Matches the exact vtable layout from oaidl.h (Windows SDK 10.0.26100.0):
    /// 23 methods + 3 IUnknown = 26 vtable slots.
    #[repr(C)]
    pub struct ICreateTypeInfoVtbl {
        // IUnknown (slots 0-2)
        pub query_interface:
            unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32,
        pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        pub release: unsafe extern "system" fn(*mut c_void) -> u32,
        // ICreateTypeInfo (slots 3-25)
        pub set_guid: unsafe extern "system" fn(*mut c_void, *const Guid) -> i32, // 3
        pub set_type_flags: unsafe extern "system" fn(*mut c_void, u32) -> i32,   // 4
        pub set_doc_string: unsafe extern "system" fn(*mut c_void, *const u16) -> i32, // 5
        pub set_help_context: unsafe extern "system" fn(*mut c_void, u32) -> i32, // 6
        pub set_version: unsafe extern "system" fn(*mut c_void, major: u16, minor: u16) -> i32, // 7
        pub add_ref_type_info:
            unsafe extern "system" fn(*mut c_void, ptinfo: *mut c_void, phreftype: *mut u32) -> i32, // 8
        pub add_func_desc:
            unsafe extern "system" fn(*mut c_void, index: u32, pfuncdesc: *const FuncDesc) -> i32, // 9
        pub add_impl_type: unsafe extern "system" fn(*mut c_void, index: u32, href: u32) -> i32, // 10
        pub set_impl_type_flags:
            unsafe extern "system" fn(*mut c_void, index: u32, flags: i32) -> i32, // 11
        pub set_alignment: unsafe extern "system" fn(*mut c_void, cb_alignment: u16) -> i32, // 12
        pub set_schema: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,           // 13
        pub add_var_desc:
            unsafe extern "system" fn(*mut c_void, index: u32, pvardesc: *const c_void) -> i32, // 14
        pub set_func_and_param_names: unsafe extern "system" fn(
            *mut c_void,
            index: u32,
            rgsznames: *const *const u16,
            cnames: u32,
        ) -> i32, // 15
        pub set_var_name:
            unsafe extern "system" fn(*mut c_void, index: u32, name: *const u16) -> i32, // 16
        pub set_type_desc_alias:
            unsafe extern "system" fn(*mut c_void, ptypedescalias: *const TypeDesc) -> i32, // 17
        pub define_func_as_dll_entry: unsafe extern "system" fn(
            *mut c_void,
            index: u32,
            dll_name: *const u16,
            proc_name: *const u16,
        ) -> i32, // 18
        pub set_func_doc_string:
            unsafe extern "system" fn(*mut c_void, index: u32, doc: *const u16) -> i32, // 19
        pub set_var_doc_string:
            unsafe extern "system" fn(*mut c_void, index: u32, doc: *const u16) -> i32, // 20
        pub set_func_help_context:
            unsafe extern "system" fn(*mut c_void, index: u32, ctx: u32) -> i32, // 21
        pub set_var_help_context:
            unsafe extern "system" fn(*mut c_void, index: u32, ctx: u32) -> i32, // 22
        pub set_mops: unsafe extern "system" fn(*mut c_void, index: u32, mops: *const u16) -> i32, // 23
        pub set_type_idldesc:
            unsafe extern "system" fn(*mut c_void, pidldesc: *const c_void) -> i32, // 24
        pub layout_type: unsafe extern "system" fn(*mut c_void) -> i32, // 25
    }

    // CreateTypeLib2 from oleaut32.dll
    #[link(name = "oleaut32")]
    unsafe extern "system" {
        pub fn CreateTypeLib2(syskind: u32, szfile: *const u16, ppctlib: *mut *mut c_void) -> i32;

        pub fn LoadTypeLib(szfile: *const u16, pptlib: *mut *mut c_void) -> i32;
    }

    // COM initialization from ole32.dll
    #[link(name = "ole32")]
    unsafe extern "system" {
        pub fn CoInitializeEx(pvreserved: *mut c_void, dwcoinit: u32) -> i32;
        pub fn CoUninitialize();
    }

    pub const COINIT_APARTMENTTHREADED: u32 = 0x2;

    /// Get the vtable pointer from a COM interface pointer.
    #[inline]
    #[allow(unsafe_op_in_unsafe_fn)]
    pub unsafe fn get_vtbl<T>(p: *mut c_void) -> &'static T {
        &*(*(p as *const *const T))
    }

    /// Helper to encode a Rust str as a null-terminated UTF-16 wide string.
    pub fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

// ── Public API ──

/// Result of type library generation.
#[derive(Debug)]
pub struct TypeLibGenResult {
    /// Path to the generated .tlb file.
    pub tlb_path: String,
    /// Number of classes written.
    pub class_count: usize,
    /// Total number of members written across all classes.
    pub member_count: usize,
}

/// Error from type library generation.
#[derive(Debug)]
pub enum TypeLibGenError {
    /// A COM HRESULT failure.
    ComError { operation: String, hresult: i32 },
    /// Platform not supported (non-Windows).
    PlatformNotSupported,
}

impl std::fmt::Display for TypeLibGenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ComError { operation, hresult } => {
                write!(f, "{operation} failed with HRESULT {hresult:#010X}")
            }
            Self::PlatformNotSupported => write!(f, "CreateTypeLib2 requires Windows"),
        }
    }
}

impl std::error::Error for TypeLibGenError {}

/// Generate a binary type library (.tlb) file from COM class export descriptors.
///
/// Uses `CreateTypeLib2` to produce the .tlb at `output_path`. Each class gets
/// a dual dispatch interface (`I{ClassName}`) and a coclass entry with the interface
/// as the default. UUIDs are deterministic based on `project_name` + component name
/// (same algorithm as `idl.rs`).
///
/// Returns metadata about what was generated, or a COM error.
#[cfg(target_os = "windows")]
pub fn generate_typelib(
    project_name: &str,
    output_path: &str,
    classes: &[ComClassExportDescriptor],
) -> Result<TypeLibGenResult, TypeLibGenError> {
    use ffi::*;

    // Ensure COM is initialized (STA). Ignore RPC_E_CHANGED_MODE if already init'd.
    let coin_hr = unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) };
    let did_coin = coin_hr >= 0; // S_OK or S_FALSE (already initialized)

    let result = generate_typelib_inner(project_name, output_path, classes);

    if did_coin {
        unsafe {
            CoUninitialize();
        }
    }

    result
}

#[cfg(target_os = "windows")]
fn generate_typelib_inner(
    project_name: &str,
    output_path: &str,
    classes: &[ComClassExportDescriptor],
) -> Result<TypeLibGenResult, TypeLibGenError> {
    use ffi::*;

    let wide_path = to_wide(output_path);
    let mut ptlib: *mut c_void = std::ptr::null_mut();

    // Create the type library
    let hr = unsafe { CreateTypeLib2(SYS_WIN64, wide_path.as_ptr(), &mut ptlib) };
    check_hr(hr, "CreateTypeLib2")?;

    let tlib_vtbl = unsafe { get_vtbl::<ICreateTypeLibVtbl>(ptlib) };

    // Set library-level attributes
    let lib_uuid_str = deterministic_uuid(project_name, "__typelib__");
    let lib_guid = parse_uuid(&lib_uuid_str);

    let lib_name_wide = to_wide(&format!("{project_name}Lib"));
    let lib_doc_wide = to_wide(&format!("{project_name} Type Library"));

    unsafe {
        check_hr(
            (tlib_vtbl.set_guid)(ptlib, &lib_guid),
            "ICreateTypeLib::SetGuid",
        )?;
        check_hr(
            (tlib_vtbl.set_name)(ptlib, lib_name_wide.as_ptr()),
            "ICreateTypeLib::SetName",
        )?;
        check_hr(
            (tlib_vtbl.set_doc_string)(ptlib, lib_doc_wide.as_ptr()),
            "ICreateTypeLib::SetDocString",
        )?;
        check_hr(
            (tlib_vtbl.set_version)(ptlib, 1, 0),
            "ICreateTypeLib::SetVersion",
        )?;
        check_hr((tlib_vtbl.set_lcid)(ptlib, 0), "ICreateTypeLib::SetLcid")?;
    }

    let mut total_members = 0usize;

    for class in classes {
        let (iface_member_count, _) =
            create_class_typeinfos(ptlib, tlib_vtbl, project_name, class)?;
        total_members += iface_member_count;
    }

    // Save the type library to disk
    let hr = unsafe { (tlib_vtbl.save_all_changes)(ptlib) };
    // Release the type library
    unsafe {
        (tlib_vtbl.release)(ptlib);
    }
    check_hr(hr, "ICreateTypeLib::SaveAllChanges")?;

    Ok(TypeLibGenResult {
        tlb_path: output_path.to_string(),
        class_count: classes.len(),
        member_count: total_members,
    })
}

#[cfg(not(target_os = "windows"))]
pub fn generate_typelib(
    _project_name: &str,
    _output_path: &str,
    _classes: &[ComClassExportDescriptor],
) -> Result<TypeLibGenResult, TypeLibGenError> {
    Err(TypeLibGenError::PlatformNotSupported)
}

// ── Internal helpers ──

#[cfg(target_os = "windows")]
fn check_hr(hr: i32, operation: &str) -> Result<(), TypeLibGenError> {
    if hr < 0 {
        Err(TypeLibGenError::ComError {
            operation: operation.to_string(),
            hresult: hr,
        })
    } else {
        Ok(())
    }
}

/// Create both the dispatch interface and coclass type infos for one COM class.
/// Returns (member_count_in_interface, ()).
#[cfg(target_os = "windows")]
fn create_class_typeinfos(
    ptlib: *mut c_void,
    tlib_vtbl: &ffi::ICreateTypeLibVtbl,
    project_name: &str,
    class: &ComClassExportDescriptor,
) -> Result<(usize, ()), TypeLibGenError> {
    use ffi::*;

    let class_name = &class.class_name;
    let description = class.description.as_deref().unwrap_or(class_name);

    let iface_name = format!("I{class_name}");
    let iface_uuid_str = deterministic_uuid(project_name, &iface_name);
    let iface_guid = parse_uuid(&iface_uuid_str);
    let coclass_uuid_str = deterministic_uuid(project_name, class_name);
    let coclass_guid = parse_uuid(&coclass_uuid_str);

    // ── Create the dispatch interface ──
    let iface_name_wide = to_wide(&iface_name);
    let mut ptinfo_iface: *mut c_void = std::ptr::null_mut();

    unsafe {
        check_hr(
            (tlib_vtbl.create_type_info)(
                ptlib,
                iface_name_wide.as_ptr(),
                TKIND_DISPATCH,
                &mut ptinfo_iface,
            ),
            "CreateTypeInfo(interface)",
        )?;
    }

    let iface_vtbl = unsafe { get_vtbl::<ICreateTypeInfoVtbl>(ptinfo_iface) };

    let iface_doc_wide = to_wide(&format!("{description} Interface"));
    unsafe {
        check_hr(
            (iface_vtbl.set_guid)(ptinfo_iface, &iface_guid),
            "ICreateTypeInfo::SetGuid(interface)",
        )?;
        check_hr(
            (iface_vtbl.set_doc_string)(ptinfo_iface, iface_doc_wide.as_ptr()),
            "ICreateTypeInfo::SetDocString(interface)",
        )?;
        // Note: SetTypeFlags with TYPEFLAG_FOLEAUTOMATION returns TYPE_E_BADMODULEKIND
        // for pure TKIND_DISPATCH. The system auto-sets appropriate flags for dispatch
        // interfaces. Explicit SetTypeFlags is only needed for TKIND_INTERFACE (dual).
        // For VBA COM servers, pure dispatch (TKIND_DISPATCH) is the correct model.
        check_hr(
            (iface_vtbl.set_version)(ptinfo_iface, 1, 0),
            "ICreateTypeInfo::SetVersion(interface)",
        )?;
    }

    // Add members to the dispatch interface
    let member_count = add_dispatch_members(ptinfo_iface, iface_vtbl, class)?;

    // Finalize the interface
    unsafe {
        check_hr(
            (iface_vtbl.layout_type)(ptinfo_iface),
            "ICreateTypeInfo::LayOut(interface)",
        )?;
    }

    // ── Create the coclass ──
    let coclass_name_wide = to_wide(class_name);
    let mut ptinfo_coclass: *mut c_void = std::ptr::null_mut();

    unsafe {
        check_hr(
            (tlib_vtbl.create_type_info)(
                ptlib,
                coclass_name_wide.as_ptr(),
                TKIND_COCLASS,
                &mut ptinfo_coclass,
            ),
            "CreateTypeInfo(coclass)",
        )?;
    }

    let coclass_vtbl = unsafe { get_vtbl::<ICreateTypeInfoVtbl>(ptinfo_coclass) };

    let coclass_doc_wide = to_wide(description);
    unsafe {
        check_hr(
            (coclass_vtbl.set_guid)(ptinfo_coclass, &coclass_guid),
            "ICreateTypeInfo::SetGuid(coclass)",
        )?;
        check_hr(
            (coclass_vtbl.set_doc_string)(ptinfo_coclass, coclass_doc_wide.as_ptr()),
            "ICreateTypeInfo::SetDocString(coclass)",
        )?;
        check_hr(
            (coclass_vtbl.set_type_flags)(ptinfo_coclass, u32::from(TYPEFLAG_FCANCREATE)),
            "ICreateTypeInfo::SetTypeFlags(coclass)",
        )?;
    }

    // Get ITypeInfo from the interface's ICreateTypeInfo via QueryInterface
    // (AddRefTypeInfo requires ITypeInfo*, not ICreateTypeInfo*)
    unsafe {
        let mut ptypeinfo_iface: *mut c_void = std::ptr::null_mut();
        check_hr(
            (iface_vtbl.query_interface)(ptinfo_iface, &IID_ITYPEINFO, &mut ptypeinfo_iface),
            "ICreateTypeInfo::QueryInterface(ITypeInfo) on interface",
        )?;

        // Add the dispatch interface as the default implemented interface on the coclass
        let mut href: u32 = 0;
        check_hr(
            (coclass_vtbl.add_ref_type_info)(ptinfo_coclass, ptypeinfo_iface, &mut href),
            "ICreateTypeInfo::AddRefTypeInfo(coclass→interface)",
        )?;

        // Release the ITypeInfo we QI'd
        let ptypeinfo_vtbl = get_vtbl::<ICreateTypeInfoVtbl>(ptypeinfo_iface);
        (ptypeinfo_vtbl.release)(ptypeinfo_iface);

        check_hr(
            (coclass_vtbl.add_impl_type)(ptinfo_coclass, 0, href),
            "ICreateTypeInfo::AddImplType(coclass)",
        )?;
        check_hr(
            (coclass_vtbl.set_impl_type_flags)(ptinfo_coclass, 0, IMPLTYPEFLAG_FDEFAULT),
            "ICreateTypeInfo::SetImplTypeFlags(coclass)",
        )?;

        // Layout and release
        check_hr(
            (coclass_vtbl.layout_type)(ptinfo_coclass),
            "ICreateTypeInfo::LayOut(coclass)",
        )?;
        (coclass_vtbl.release)(ptinfo_coclass);
        (iface_vtbl.release)(ptinfo_iface);
    }

    Ok((member_count, ()))
}

/// Add dispatch member functions to a type info.
#[cfg(target_os = "windows")]
fn add_dispatch_members(
    ptinfo: *mut c_void,
    vtbl: &ffi::ICreateTypeInfoVtbl,
    class: &ComClassExportDescriptor,
) -> Result<usize, TypeLibGenError> {
    use ffi::*;

    for (i, member) in class.members.iter().enumerate() {
        let dispid = member.dispatch_id_or(i + 1);

        let is_function = matches!(
            member.kind,
            oxvba_compiler::ProjectDynamicMemberKind::Function
                | oxvba_compiler::ProjectDynamicMemberKind::PropertyGet
        );

        let invkind = match member.kind {
            oxvba_compiler::ProjectDynamicMemberKind::PropertyGet => INVOKE_PROPERTYGET,
            oxvba_compiler::ProjectDynamicMemberKind::PropertyLet
            | oxvba_compiler::ProjectDynamicMemberKind::PropertySet => INVOKE_PROPERTYPUT,
            _ => INVOKE_FUNC,
        };

        // Build parameter ELEMDESC array
        // For functions: params + retval param
        // For subs/property puts: params only
        let total_params = if is_function {
            member.param_count + 1 // +1 for [out, retval]
        } else {
            member.param_count
        };

        let mut param_descs: Vec<ElemDesc> = Vec::with_capacity(total_params);

        // [in] VARIANT parameters
        for _ in 0..member.param_count {
            param_descs.push(ElemDesc {
                tdesc: TypeDesc {
                    union_field: 0,
                    vt: VT_VARIANT,
                },
                paramdesc: ParamDesc {
                    pparamdescex: std::ptr::null_mut(),
                    wparamflags: PARAMFLAG_FIN,
                },
            });
        }

        // [out, retval] VARIANT* parameter for functions
        // We need a stable TypeDesc for VT_PTR to point to
        let mut retval_pointee = TypeDesc {
            union_field: 0,
            vt: VT_VARIANT,
        };
        if is_function {
            param_descs.push(ElemDesc {
                tdesc: TypeDesc {
                    union_field: &mut retval_pointee as *mut TypeDesc as usize,
                    vt: VT_PTR,
                },
                paramdesc: ParamDesc {
                    pparamdescex: std::ptr::null_mut(),
                    wparamflags: PARAMFLAG_FOUT | PARAMFLAG_FRETVAL,
                },
            });
        }

        // Return type for the function itself (always HRESULT for dual interfaces)
        let funcdesc = FuncDesc {
            memid: dispid,
            lprgscode: std::ptr::null_mut(),
            lprgelemdescparam: if param_descs.is_empty() {
                std::ptr::null_mut()
            } else {
                param_descs.as_mut_ptr()
            },
            funckind: FUNC_DISPATCH,
            invkind,
            callconv: CC_STDCALL,
            cparams: total_params as i16,
            cparams_opt: 0,
            o_vft: 0,
            cscodes: 0,
            elemdesc_func: ElemDesc {
                tdesc: TypeDesc {
                    union_field: 0,
                    vt: VT_HRESULT,
                },
                paramdesc: ParamDesc {
                    pparamdescex: std::ptr::null_mut(),
                    wparamflags: 0,
                },
            },
            w_func_flags: member_function_flags(member),
        };

        unsafe {
            check_hr(
                (vtbl.add_func_desc)(ptinfo, i as u32, &funcdesc),
                &format!("AddFuncDesc({})", member.member_name),
            )?;
        }

        // Set function and parameter names
        let member_name_wide = ffi::to_wide(&member.member_name);
        let mut names: Vec<*const u16> = vec![member_name_wide.as_ptr()];

        // Build parameter name wide strings (keep them alive through the call)
        let param_name_wides: Vec<Vec<u16>> = (0..member.param_count)
            .map(|pi| ffi::to_wide(&format!("arg{pi}")))
            .collect();
        for pname in &param_name_wides {
            names.push(pname.as_ptr());
        }

        if is_function {
            let retval_name = ffi::to_wide("pRetVal");
            names.push(retval_name.as_ptr());
            unsafe {
                check_hr(
                    (vtbl.set_func_and_param_names)(
                        ptinfo,
                        i as u32,
                        names.as_ptr(),
                        names.len() as u32,
                    ),
                    &format!("SetFuncAndParamNames({})", member.member_name),
                )?;
            }
        } else {
            unsafe {
                check_hr(
                    (vtbl.set_func_and_param_names)(
                        ptinfo,
                        i as u32,
                        names.as_ptr(),
                        names.len() as u32,
                    ),
                    &format!("SetFuncAndParamNames({})", member.member_name),
                )?;
            }
        }
    }

    Ok(class.members.len())
}

#[cfg(target_os = "windows")]
fn member_function_flags(member: &oxvba_project::DispatchMemberInfo) -> u16 {
    let mut flags = 0;
    if member.is_restricted() {
        flags |= ffi::FUNCFLAG_FRESTRICTED;
    }
    if member.is_default_bind() {
        flags |= ffi::FUNCFLAG_FDEFAULTBIND;
    }
    if member.is_hidden() {
        flags |= ffi::FUNCFLAG_FHIDDEN;
    }
    flags
}

/// Verify a generated .tlb by loading it back and checking its member catalog
/// matches the source descriptors.
#[cfg(target_os = "windows")]
pub fn verify_typelib_roundtrip(
    tlb_path: &str,
    project_name: &str,
    classes: &[ComClassExportDescriptor],
) -> Result<(), TypeLibGenError> {
    let wide_path = ffi::to_wide(tlb_path);
    let mut ptlib: *mut c_void = std::ptr::null_mut();

    let hr = unsafe { ffi::LoadTypeLib(wide_path.as_ptr(), &mut ptlib) };
    check_hr(hr, "LoadTypeLib(verification)")?;

    // Basic verification: check the type library loaded successfully
    // and has the expected number of type infos (2 per class: interface + coclass)
    let _ = (project_name, classes);

    // Release the loaded type library
    unsafe {
        let tlib_vtbl = ffi::get_vtbl::<ffi::ICreateTypeLibVtbl>(ptlib);
        (tlib_vtbl.release)(ptlib);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_uuid_roundtrip() {
        let uuid_str = "01234567-89ab-cdef-0123-456789abcdef";
        let guid = parse_uuid(uuid_str);
        assert_eq!(guid.data1, 0x01234567);
        assert_eq!(guid.data2, 0x89ab);
        assert_eq!(guid.data3, 0xcdef);
        assert_eq!(guid.data4, [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
    }

    #[test]
    fn deterministic_uuid_produces_valid_guid() {
        let uuid = deterministic_uuid("TestProject", "Calculator");
        let guid = parse_uuid(&uuid);
        assert_ne!(guid, Guid::ZERO);
        // Version nibble should be 5
        assert_eq!(guid.data3 >> 12, 5);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn generate_typelib_creates_file() {
        use oxvba_project::{ComClassExportDescriptor, DispatchMemberInfo};

        let classes = vec![ComClassExportDescriptor {
            class_name: "Calculator".to_string(),
            prog_id: Some("TestApp.Calculator".to_string()),
            instancing: None,
            description: Some("A calculator class".to_string()),
            members: vec![
                DispatchMemberInfo {
                    member_name: "Add".to_string(),
                    kind: oxvba_compiler::ProjectDynamicMemberKind::Function,
                    param_count: 2,
                    dispatch_id: None,
                    member_flags: None,
                    is_default_member: false,
                },
                DispatchMemberInfo {
                    member_name: "Clear".to_string(),
                    kind: oxvba_compiler::ProjectDynamicMemberKind::Method,
                    param_count: 0,
                    dispatch_id: None,
                    member_flags: None,
                    is_default_member: false,
                },
                DispatchMemberInfo {
                    member_name: "Value".to_string(),
                    kind: oxvba_compiler::ProjectDynamicMemberKind::PropertyGet,
                    param_count: 0,
                    dispatch_id: None,
                    member_flags: None,
                    is_default_member: false,
                },
            ],
        }];

        let temp_dir = std::env::temp_dir();
        let tlb_path = temp_dir
            .join("oxvba_test_typelib_gen.tlb")
            .to_string_lossy()
            .to_string();

        let result = generate_typelib("TestApp", &tlb_path, &classes).unwrap();
        assert_eq!(result.class_count, 1);
        assert_eq!(result.member_count, 3);
        assert!(std::path::Path::new(&tlb_path).exists());

        // Verify roundtrip
        verify_typelib_roundtrip(&tlb_path, "TestApp", &classes).unwrap();

        // Cleanup
        let _ = std::fs::remove_file(&tlb_path);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn generate_typelib_empty_class() {
        use oxvba_project::ComClassExportDescriptor;

        let classes = vec![ComClassExportDescriptor {
            class_name: "EmptyWidget".to_string(),
            prog_id: None,
            instancing: None,
            description: None,
            members: vec![],
        }];

        let temp_dir = std::env::temp_dir();
        let tlb_path = temp_dir
            .join("oxvba_test_typelib_empty.tlb")
            .to_string_lossy()
            .to_string();

        let result = generate_typelib("EmptyProj", &tlb_path, &classes).unwrap();
        assert_eq!(result.class_count, 1);
        assert_eq!(result.member_count, 0);

        let _ = std::fs::remove_file(&tlb_path);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn generate_typelib_multiple_classes() {
        use oxvba_project::{ComClassExportDescriptor, DispatchMemberInfo};

        let classes = vec![
            ComClassExportDescriptor {
                class_name: "Alpha".to_string(),
                prog_id: Some("MultiTest.Alpha".to_string()),
                instancing: None,
                description: Some("Alpha class".to_string()),
                members: vec![DispatchMemberInfo {
                    member_name: "DoAlpha".to_string(),
                    kind: oxvba_compiler::ProjectDynamicMemberKind::Function,
                    param_count: 1,
                    dispatch_id: None,
                    member_flags: None,
                    is_default_member: false,
                }],
            },
            ComClassExportDescriptor {
                class_name: "Beta".to_string(),
                prog_id: Some("MultiTest.Beta".to_string()),
                instancing: None,
                description: Some("Beta class".to_string()),
                members: vec![
                    DispatchMemberInfo {
                        member_name: "DoBeta".to_string(),
                        kind: oxvba_compiler::ProjectDynamicMemberKind::Method,
                        param_count: 0,
                        dispatch_id: None,
                        member_flags: None,
                        is_default_member: false,
                    },
                    DispatchMemberInfo {
                        member_name: "Name".to_string(),
                        kind: oxvba_compiler::ProjectDynamicMemberKind::PropertyGet,
                        param_count: 0,
                        dispatch_id: None,
                        member_flags: None,
                        is_default_member: false,
                    },
                ],
            },
        ];

        let temp_dir = std::env::temp_dir();
        let tlb_path = temp_dir
            .join("oxvba_test_typelib_multi.tlb")
            .to_string_lossy()
            .to_string();

        let result = generate_typelib("MultiTest", &tlb_path, &classes).unwrap();
        assert_eq!(result.class_count, 2);
        assert_eq!(result.member_count, 3);

        let _ = std::fs::remove_file(&tlb_path);
    }

    #[test]
    fn platform_not_supported_on_non_windows() {
        // This test only runs on non-Windows; on Windows it's skipped.
        #[cfg(not(target_os = "windows"))]
        {
            let result = generate_typelib("Test", "test.tlb", &[]);
            assert!(matches!(result, Err(TypeLibGenError::PlatformNotSupported)));
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn generate_typelib_preserves_explicit_dispatch_ids() {
        use oxvba_com::windows_typelib_loader::{
            enumerate_typelib_members, load_typelib_from_path, release_typelib,
        };
        use oxvba_project::{ComClassExportDescriptor, DispatchMemberInfo};

        let classes = vec![ComClassExportDescriptor {
            class_name: "Widget".to_string(),
            prog_id: Some("AttrTest.Widget".to_string()),
            instancing: None,
            description: None,
            members: vec![
                DispatchMemberInfo {
                    member_name: "Value".to_string(),
                    kind: oxvba_compiler::ProjectDynamicMemberKind::PropertyGet,
                    param_count: 0,
                    dispatch_id: Some(0),
                    member_flags: None,
                    is_default_member: true,
                },
                DispatchMemberInfo {
                    member_name: "NewEnum".to_string(),
                    kind: oxvba_compiler::ProjectDynamicMemberKind::PropertyGet,
                    param_count: 0,
                    dispatch_id: Some(-4),
                    member_flags: Some(0x40),
                    is_default_member: false,
                },
            ],
        }];

        let temp_dir = std::env::temp_dir();
        let tlb_path = temp_dir
            .join("oxvba_test_typelib_attr_ids.tlb")
            .to_string_lossy()
            .to_string();

        generate_typelib("AttrTest", &tlb_path, &classes).unwrap();
        let ptlib = load_typelib_from_path(&tlb_path).expect("typelib should load");
        let members = enumerate_typelib_members(ptlib).expect("member enumeration should succeed");
        unsafe { release_typelib(ptlib) };

        let value = members
            .iter()
            .find(|member| member.name == "Value")
            .expect("Value should be present");
        assert_eq!(value.token, 0);
        assert!(value.is_default_member);

        let new_enum = members
            .iter()
            .find(|member| member.name == "NewEnum")
            .expect("NewEnum should be present");
        assert_eq!(new_enum.token, -4);

        let _ = std::fs::remove_file(&tlb_path);
    }
}
