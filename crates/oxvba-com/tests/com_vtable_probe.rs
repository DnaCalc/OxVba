//! Crash-isolated diagnostic probe for in-process COM vtable dispatch.
//!
//! This is a DIAGNOSTIC probe, not a production change. Its job is to print the
//! truth about a *live* in-process DAO object's vtable so we can root-cause the
//! access-violation OxVba's early-bound vtable fast path hits when it slot-calls
//! a DAO member.
//!
//! ## Principle: INSPECT, don't INVOKE
//!
//! Everything except ONE final guarded call is access-violation-free: reading a
//! vtable bounded by the OS-reported `cbSizeVft`, `QueryInterface`, `GetTypeInfo`,
//! `GetTypeAttr`, `GetModuleHandleEx` are all safe to call on a live object. The
//! single vtable invocation at the very end is the only AV risk, and it is
//! isolated by the test-process boundary: if it faults, the process dies with
//! `0xC0000005` (exit `-1073741819`), and the flushed inspection dump that
//! preceded it is the deliverable.
//!
//! ## Hypotheses under test
//!
//! - **H-A**: `IDispatch::GetTypeInfo(0)` on a live DAO Recordset returns a
//!   `dispinterface` (`TKIND_DISPATCH`) whose `FUNCDESC.oVft` is garbage
//!   (dispinterface members have no vtable slot), and the dispatch code computes
//!   a bogus slot from it. The decisive evidence is the `typekind`/`FDUAL` read
//!   in section C plus the per-interface funcdesc in section E.
//! - **H-B**: even a true dual interface's QI'd tear-off may have a *physical*
//!   vtable shorter or different from the typelib's design-time `oVft`. The
//!   decisive evidence is the raw vtable walk in section F (which module backs
//!   the `oVft`-derived slot, and whether that slot is in bounds of the live
//!   `cbSizeVft`).
//!
//! ## How to run
//!
//! ```text
//! cargo test -p oxvba-com --test com_vtable_probe -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Each run is fresh (a new process, a new STA, a new temp database). The probe
//! tests are `#[ignore]`d so a normal `cargo test` never touches a live DAO
//! engine and never risks an AV.
#![cfg(all(target_os = "windows", target_arch = "x86_64"))]
// This is an integration test that drives raw COM by hand. The whole file is a
// dense block of FFI; the unsafe blocks each carry a `// SAFETY:` note.
#![allow(clippy::too_many_lines)]

use std::ffi::c_void;
use std::io::{self, Write};
use std::ptr;

use oxvba_com::windows_client::{
    IID_IDISPATCH, RawIDispatch, activate_dispatch_by_prog_id, release_dispatch, release_unknown,
};

use windows_sys::Win32::Foundation::{SysAllocString, SysFreeString};
use windows_sys::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize, DISPATCH_METHOD,
    DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPPARAMS, ELEMDESC, EXCEPINFO, FUNCDESC,
    SYS_WIN32, SYS_WIN64, TKIND_DISPATCH, TKIND_INTERFACE, TLIBATTR, TYPEATTR,
};
use windows_sys::Win32::System::Ole::{
    PARAMFLAG_FIN, PARAMFLAG_FLCID, PARAMFLAG_FOPT, PARAMFLAG_FOUT, PARAMFLAG_FRETVAL,
};
use windows_sys::Win32::System::Variant::{
    VARIANT, VT_BSTR, VT_DISPATCH, VariantClear, VariantInit,
};
use windows_sys::core::GUID;

// ── Locally-declared Win32 / COM imports ─────────────────────────────────────
//
// windows-sys provides the plain-data structs (`TYPEATTR`, `FUNCDESC`,
// `TLIBATTR`, `DISPPARAMS`, `VARIANT`, `EXCEPINFO`) but NOT callable interface
// wrappers for `ITypeInfo` / `ITypeLib` (those live in the heavier `windows`
// crate). We transcribe the small subset of those vtables we call, mirroring the
// `unsafe extern "system"` pattern in windows_vtable.rs:71 / windows_ffi_bridge.rs:194.
// No new windows-sys features are needed.

// SAFETY: kernel32 exports transcribed with the documented stdcall ABI.
// `GetModuleHandleExW` looks up (without changing the refcount) the module that
// owns a code address; `GetModuleFileNameW` writes that module's path. Each call
// site upholds the buffer/pointer invariants.
unsafe extern "system" {
    fn GetModuleHandleExW(
        dw_flags: u32,
        lp_module_name: *const u16,
        ph_module: *mut *mut c_void,
    ) -> i32;
    fn GetModuleFileNameW(h_module: *mut c_void, lp_filename: *mut u16, n_size: u32) -> u32;
}

const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 0x0000_0004;
const GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT: u32 = 0x0000_0002;

const TYPEFLAG_FDUAL: u16 = 0x40;

/// `IProxyManager` — every COM marshaling proxy implements it; an in-process
/// (non-marshaled) object does not. A successful QI ⇒ the object we hold is a
/// proxy (out-of-process or cross-apartment), not a direct interface pointer.
const IID_IPROXYMANAGER: GUID = GUID {
    data1: 0x0000_0008,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

// Well-known DAO interface IIDs (Microsoft Office ACE / DAO 12.0). These are the
// classic `00000035/57/71-0000-0010-8000-00AA006D2EA4` family; we QI for them in
// the matrix so the dump shows which physical interfaces the object exposes.
const IID_DAO_DATABASE: GUID = dao_iid(0x0000_0035);
const IID_DAO_RECORDSET: GUID = dao_iid(0x0000_0057);
const IID_DAO_FIELD: GUID = dao_iid(0x0000_0071);

const fn dao_iid(data1: u32) -> GUID {
    GUID {
        data1,
        data2: 0x0000,
        data3: 0x0010,
        data4: [0x80, 0x00, 0x00, 0xAA, 0x00, 0x6D, 0x2E, 0xA4],
    }
}

/// Excel's `_Application` dual interface IID `{000208D5-0000-0000-C000-000000000046}`.
/// This is the interface marshaled by PSOAInterface `{00020424}` (the oleaut
/// universal/typelib marshaler) — a typelib-aligned vtable proxy that SHOULD be
/// vtable-callable by `slot = oVft / 8`. The Excel `_Application` probe ladder
/// below QIs the live out-of-process IDispatch for this IID.
const IID_EXCEL_APPLICATION: GUID = GUID {
    data1: 0x0002_08D5,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

// ── ITypeInfo (subset) ───────────────────────────────────────────────────────
//
// Method order is the canonical oaidl.h `ITypeInfoVtbl`. We only ever *call* the
// handful we name; the rest are placeholders to keep the slot offsets correct.
#[repr(C)]
struct RawITypeInfoVtbl {
    // IUnknown
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
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
        *mut DISPPARAMS,
        *mut VARIANT,
        *mut EXCEPINFO,
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
        u32,
        *mut *mut u16,
        *mut *mut u16,
        *mut u16,
    ) -> i32,
    get_ref_type_info: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32,
    address_of_member: unsafe extern "system" fn(*mut c_void, i32, u32, *mut *mut c_void) -> i32,
    create_instance:
        unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, *mut *mut c_void) -> i32,
    get_mops: unsafe extern "system" fn(*mut c_void, i32, *mut *mut u16) -> i32,
    get_containing_type_lib:
        unsafe extern "system" fn(*mut c_void, *mut *mut c_void, *mut u32) -> i32,
    release_type_attr: unsafe extern "system" fn(*mut c_void, *mut TYPEATTR),
    release_func_desc: unsafe extern "system" fn(*mut c_void, *mut FUNCDESC),
    release_var_desc: unsafe extern "system" fn(*mut c_void, *mut c_void),
}

#[repr(C)]
struct RawITypeInfo {
    vtbl: *const RawITypeInfoVtbl,
}

// ── ITypeLib (subset) ─────────────────────────────────────────────────────────
#[repr(C)]
struct RawITypeLibVtbl {
    // IUnknown
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    // ITypeLib
    get_type_info_count: unsafe extern "system" fn(*mut c_void) -> u32,
    get_type_info: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32,
    get_type_info_type: unsafe extern "system" fn(*mut c_void, u32, *mut i32) -> i32,
    get_type_info_of_guid:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    get_lib_attr: unsafe extern "system" fn(*mut c_void, *mut *mut TLIBATTR) -> i32,
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
    release_tlib_attr: unsafe extern "system" fn(*mut c_void, *mut TLIBATTR),
}

#[repr(C)]
struct RawITypeLib {
    vtbl: *const RawITypeLibVtbl,
}

// ── small helpers ─────────────────────────────────────────────────────────────

fn fmt_guid(g: &GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7],
    )
}

fn typekind_name(tk: i32) -> &'static str {
    match tk {
        0 => "TKIND_ENUM",
        1 => "TKIND_RECORD",
        2 => "TKIND_MODULE",
        x if x == TKIND_INTERFACE => "TKIND_INTERFACE",
        x if x == TKIND_DISPATCH => "TKIND_DISPATCH(dispinterface)",
        5 => "TKIND_COCLASS",
        6 => "TKIND_ALIAS",
        7 => "TKIND_UNION",
        _ => "TKIND_?",
    }
}

fn invkind_name(ik: i32) -> &'static str {
    match ik {
        1 => "INVOKE_FUNC",
        2 => "INVOKE_PROPERTYGET",
        4 => "INVOKE_PROPERTYPUT",
        8 => "INVOKE_PROPERTYPUTREF",
        _ => "INVOKE_?",
    }
}

/// Module basename (or `<<UNBACKED>>`) that owns a code address. The whole point
/// of the probe's section F: AV-free because `GetModuleHandleExW` with
/// `UNCHANGED_REFCOUNT|FROM_ADDRESS` only *reads* the loader tables.
fn module_of_address(fnptr: *const c_void) -> String {
    if fnptr.is_null() {
        return "<<NULL>>".to_string();
    }
    let mut hmod: *mut c_void = ptr::null_mut();
    // SAFETY: `fnptr` is a code address read from a vtable slot; FROM_ADDRESS lets
    // the loader resolve which module contains it, UNCHANGED_REFCOUNT means we owe
    // no Release. `&mut hmod` is a live out-slot. The call only reads loader state.
    let ok = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            fnptr.cast::<u16>(),
            &mut hmod,
        )
    };
    if ok == 0 || hmod.is_null() {
        return "<<UNBACKED>>".to_string();
    }
    let mut buf = [0u16; 260];
    // SAFETY: `hmod` is a valid module handle just obtained; `buf` is a live
    // 260-u16 buffer and we pass its true length, so GetModuleFileNameW writes at
    // most that many units and returns the count actually written.
    let len = unsafe { GetModuleFileNameW(hmod, buf.as_mut_ptr(), buf.len() as u32) };
    if len == 0 {
        return "<<no-name>>".to_string();
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string()
}

/// Build a `VT_BSTR` VARIANT owning a freshly-allocated BSTR. Caller frees via
/// [`clear_variant`] (or `VariantClear`).
fn bstr_variant(text: &str) -> VARIANT {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `wide` is a NUL-terminated UTF-16 buffer alive across the call;
    // SysAllocString copies it into an owned BSTR. We zero-init the VARIANT and
    // set exactly the `vt`/`bstrVal` fields the VT_BSTR discriminant requires.
    unsafe {
        let mut v: VARIANT = std::mem::zeroed();
        VariantInit(&mut v);
        v.Anonymous.Anonymous.vt = VT_BSTR;
        v.Anonymous.Anonymous.Anonymous.bstrVal = SysAllocString(wide.as_ptr());
        v
    }
}

fn clear_variant(v: &mut VARIANT) {
    // SAFETY: `v` is a live, properly-initialised VARIANT; VariantClear frees any
    // owned payload (BSTR/IDispatch) and resets it to VT_EMPTY.
    unsafe {
        let _ = VariantClear(v);
    }
}

/// Drive `IDispatch::GetIDsOfNames` for a single member name.
fn dispid_of(disp: *mut RawIDispatch, name: &str) -> Result<i32, i32> {
    let mut wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut names = [wide.as_mut_ptr()];
    let mut dispid = 0i32;
    const IID_NULL: GUID = GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    // SAFETY: `disp` is a live IDispatch; `names`/`&mut dispid` are live out-slots,
    // and `wide` (which `names[0]` points into) outlives the call.
    let hr = unsafe {
        ((*(*disp).vtbl).get_ids_of_names)(
            disp.cast(),
            &IID_NULL,
            names.as_mut_ptr(),
            1,
            0x0400,
            &mut dispid,
        )
    };
    if hr < 0 { Err(hr) } else { Ok(dispid) }
}

/// Generic `IDispatch::Invoke`. `args` are passed in DECLARATION order here and
/// reversed into `rgvarg` (COM expects reverse order). Returns the raw VARIANT
/// result and the HRESULT; the caller owns and must clear the result.
fn invoke(
    disp: *mut RawIDispatch,
    dispid: i32,
    flags: u16,
    args: &mut [VARIANT],
) -> (i32, VARIANT) {
    // COM wants rgvarg in reverse argument order.
    let mut reversed: Vec<VARIANT> = args.iter().rev().copied().collect();
    let mut params = DISPPARAMS {
        rgvarg: if reversed.is_empty() {
            ptr::null_mut()
        } else {
            reversed.as_mut_ptr()
        },
        rgdispidNamedArgs: ptr::null_mut(),
        cArgs: reversed.len() as u32,
        cNamedArgs: 0,
    };
    // PROPERTYPUT needs a named DISPID_PROPERTYPUT arg; we never put here.
    // SAFETY: zeroing then VariantInit yields a valid VT_EMPTY VARIANT.
    let mut result: VARIANT = unsafe {
        let mut v: VARIANT = std::mem::zeroed();
        VariantInit(&mut v);
        v
    };
    // SAFETY: EXCEPINFO is plain-data; an all-zero value is a valid empty one.
    let mut excep: EXCEPINFO = unsafe { std::mem::zeroed() };
    let mut arg_err = 0u32;
    const IID_NULL: GUID = GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    // SAFETY: `disp` is a live IDispatch; `params`/`result`/`excep`/`arg_err` are
    // live out/in slots, and `reversed` (which `params.rgvarg` points into)
    // outlives the call.
    let hr = unsafe {
        ((*(*disp).vtbl).invoke)(
            disp.cast(),
            dispid,
            &IID_NULL,
            0x0400,
            flags,
            &mut params,
            &mut result,
            &mut excep,
            &mut arg_err,
        )
    };
    (hr, result)
}

/// Pull the `*mut RawIDispatch` out of a VT_DISPATCH result (and consume the
/// reference: the caller no longer needs to clear the VARIANT for the pointer,
/// they now own the IDispatch and must Release it). Returns null on type mismatch.
fn dispatch_from_variant(v: &VARIANT) -> *mut RawIDispatch {
    // SAFETY: reading the discriminant `vt` and the `pdispVal` union arm of a
    // valid VARIANT.
    unsafe {
        if v.Anonymous.Anonymous.vt == VT_DISPATCH {
            v.Anonymous
                .Anonymous
                .Anonymous
                .pdispVal
                .cast::<RawIDispatch>()
        } else {
            ptr::null_mut()
        }
    }
}

/// Decode a VARIANT result to a short human string for the value-oracle line.
fn decode_variant(v: &VARIANT) -> String {
    // SAFETY: reading the discriminant then the matching union arm of a valid VARIANT.
    unsafe {
        let vt = v.Anonymous.Anonymous.vt;
        let a = &v.Anonymous.Anonymous.Anonymous;
        match vt {
            0 => "VT_EMPTY".to_string(),
            1 => "VT_NULL".to_string(),
            2 => format!("VT_I2({})", a.iVal),
            3 => format!("VT_I4({})", a.lVal),
            4 => format!("VT_R4({})", a.fltVal),
            5 => format!("VT_R8({})", a.dblVal),
            8 => "VT_BSTR".to_string(),
            9 => format!("VT_DISPATCH(0x{:x})", a.pdispVal as usize),
            11 => format!("VT_BOOL({})", a.boolVal),
            x if x == VT_BSTR => "VT_BSTR".to_string(),
            other => format!("vt={other}"),
        }
    }
}

/// `QueryInterface` for an arbitrary IID directly (the windows_client helper
/// returns Err on failure, but for the matrix we want the raw HRESULT/pointer).
fn raw_qi(obj: *mut c_void, iid: &GUID) -> (i32, *mut c_void) {
    if obj.is_null() {
        return (i32::MIN, ptr::null_mut());
    }
    let mut out: *mut c_void = ptr::null_mut();
    // SAFETY: `obj` is a live COM interface pointer whose first field is an
    // IUnknown vtable; `&mut out` is a live out-slot. We read the vtable base then
    // call slot 0 (QueryInterface).
    let hr = unsafe {
        let vtbl = *obj.cast::<*const *const c_void>();
        let qi: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32 =
            std::mem::transmute(*vtbl);
        qi(obj, iid, &mut out)
    };
    (hr, out)
}

/// `GetTypeInfo(0)` on an IDispatch → its `*mut RawITypeInfo` (or null).
fn type_info_of(disp: *mut RawIDispatch) -> *mut RawITypeInfo {
    let mut ti: *mut c_void = ptr::null_mut();
    // SAFETY: `disp` is a live IDispatch; slot 4 is GetTypeInfo. `&mut ti` is a
    // live out-slot receiving one owned ITypeInfo reference on success.
    let hr = unsafe { ((*(*disp).vtbl).get_type_info)(disp.cast(), 0, 0x0400, &mut ti) };
    if hr < 0 { ptr::null_mut() } else { ti.cast() }
}

/// Print section C/E for a single ITypeInfo and return its `(typekind, cbSizeVft,
/// granularity)`. AV-free: only GetTypeAttr / GetContainingTypeLib / GetLibAttr.
fn dump_typeinfo(label: &str, ti: *mut RawITypeInfo) -> Option<(i32, u16, u16)> {
    if ti.is_null() {
        println!("  [{label}] ITypeInfo = NULL (GetTypeInfo failed)");
        return None;
    }
    let vt = ti.cast::<RawITypeInfo>();
    let mut pattr: *mut TYPEATTR = ptr::null_mut();
    // SAFETY: `ti` is a live ITypeInfo; GetTypeAttr fills `pattr` with an owned
    // TYPEATTR we must release via ReleaseTypeAttr.
    let hr = unsafe { ((*(*vt).vtbl).get_type_attr)(vt.cast(), &mut pattr) };
    if hr < 0 || pattr.is_null() {
        println!("  [{label}] GetTypeAttr failed hr=0x{:08X}", hr as u32);
        return None;
    }
    // SAFETY: `pattr` is a valid TYPEATTR just produced by GetTypeAttr.
    let attr = unsafe { *pattr };
    let typekind = attr.typekind;
    let cb_size_vft = attr.cbSizeVft;
    let fdual = (attr.wTypeFlags & TYPEFLAG_FDUAL) != 0;

    // Containing typelib → syskind → granularity.
    let mut granularity: u16 = if cfg!(target_arch = "x86_64") { 8 } else { 4 };
    let mut syskind_str = "<unknown>".to_string();
    let mut tlib: *mut c_void = ptr::null_mut();
    let mut index = 0u32;
    // SAFETY: `ti` is a live ITypeInfo; GetContainingTypeLib yields an owned
    // ITypeLib (and the type's index) on success.
    let hr_lib =
        unsafe { ((*(*vt).vtbl).get_containing_type_lib)(vt.cast(), &mut tlib, &mut index) };
    if hr_lib >= 0 && !tlib.is_null() {
        let lib = tlib.cast::<RawITypeLib>();
        let mut plib: *mut TLIBATTR = ptr::null_mut();
        // SAFETY: `lib` is a live ITypeLib; GetLibAttr yields an owned TLIBATTR.
        let hr_la = unsafe { ((*(*lib).vtbl).get_lib_attr)(lib.cast(), &mut plib) };
        if hr_la >= 0 && !plib.is_null() {
            // SAFETY: `plib` is a valid TLIBATTR just produced by GetLibAttr.
            let la = unsafe { *plib };
            granularity = match la.syskind {
                x if x == SYS_WIN64 => 8,
                x if x == SYS_WIN32 => 4,
                _ => granularity,
            };
            syskind_str = match la.syskind {
                x if x == SYS_WIN32 => "SYS_WIN32".to_string(),
                x if x == SYS_WIN64 => "SYS_WIN64".to_string(),
                other => format!("syskind={other}"),
            };
            // SAFETY: releasing the TLIBATTR back to its owning ITypeLib.
            unsafe { ((*(*lib).vtbl).release_tlib_attr)(lib.cast(), plib) };
        }
        // SAFETY: releasing the owned ITypeLib reference.
        unsafe { ((*(*lib).vtbl).release)(lib.cast()) };
    }

    println!(
        "  [{label}] typekind={tk}({tkn}) FDUAL={fdual} guid={guid} cFuncs={cf} \
         cbSizeVft={cb} ver={maj}.{min} syskind={sk} oVft-granularity={gran}",
        tk = typekind,
        tkn = typekind_name(typekind),
        fdual = fdual,
        guid = fmt_guid(&attr.guid),
        cf = attr.cFuncs,
        cb = cb_size_vft,
        maj = attr.wMajorVerNum,
        min = attr.wMinorVerNum,
        sk = syskind_str,
        gran = granularity,
    );

    // Section E: per-member FUNCDESC for the members we care about.
    let interesting = ["Close", "Value", "Fields", "OpenRecordset", "Execute"];
    for fi in 0..attr.cFuncs {
        let mut pfd: *mut FUNCDESC = ptr::null_mut();
        // SAFETY: `ti` is a live ITypeInfo; GetFuncDesc(i) yields an owned FUNCDESC
        // for a valid index i < cFuncs.
        let hr_fd = unsafe { ((*(*vt).vtbl).get_func_desc)(vt.cast(), u32::from(fi), &mut pfd) };
        if hr_fd < 0 || pfd.is_null() {
            continue;
        }
        // SAFETY: `pfd` is a valid FUNCDESC just produced by GetFuncDesc.
        let fd = unsafe { *pfd };
        // Member name via GetDocumentation(memid).
        let name = get_member_name(vt, fd.memid);
        if let Some(n) = &name
            && interesting.iter().any(|w| w.eq_ignore_ascii_case(n))
        {
            let ovft = fd.oVft;
            let slot = if granularity != 0 {
                i64::from(ovft) / i64::from(granularity)
            } else {
                -1
            };
            println!(
                "      member `{n}` memid={memid} oVft={ovft} invkind={ik}({ikn}) \
                 cParams={cp} callconv={cc} derived-slot={slot}",
                memid = fd.memid,
                ovft = ovft,
                ik = fd.invkind,
                ikn = invkind_name(fd.invkind),
                cp = fd.cParams,
                cc = fd.callconv,
                slot = slot,
            );
        }
        // SAFETY: releasing the FUNCDESC back to its owning ITypeInfo.
        unsafe { ((*(*vt).vtbl).release_func_desc)(vt.cast(), pfd) };
    }

    // SAFETY: releasing the TYPEATTR back to its owning ITypeInfo.
    unsafe { ((*(*vt).vtbl).release_type_attr)(vt.cast(), pattr) };
    Some((typekind, cb_size_vft, granularity))
}

fn get_member_name(vt: *mut RawITypeInfo, memid: i32) -> Option<String> {
    let mut pname: *mut u16 = ptr::null_mut();
    // SAFETY: `vt` is a live ITypeInfo; GetDocumentation(memid) fills `pname` with
    // an owned BSTR name (the other out-args are null = not requested).
    let hr = unsafe {
        ((*(*vt).vtbl).get_documentation)(
            vt.cast(),
            memid,
            &mut pname,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if hr < 0 || pname.is_null() {
        return None;
    }
    // SAFETY: `pname` is an owned BSTR (NUL-terminated UTF-16). We read until NUL,
    // then free it with SysFreeString exactly once.
    let s = unsafe {
        let mut len = 0usize;
        while *pname.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(pname, len);
        let s = String::from_utf16_lossy(slice);
        SysFreeString(pname);
        s
    };
    Some(s)
}

/// Section F: raw vtable walk, bounded by the LIVE `cbSizeVft`.
fn walk_vtable(
    label: &str,
    p: *mut c_void,
    cb_size_vft: u16,
    granularity: u16,
    annotate: &[(i64, &str)],
) {
    if p.is_null() {
        println!("  [F:{label}] pointer is NULL, skipping vtable walk");
        return;
    }
    let ptr_size = std::mem::size_of::<usize>() as u16;
    let slots = if ptr_size == 0 {
        0
    } else {
        usize::from(cb_size_vft / ptr_size)
    };
    println!(
        "  [F:{label}] vtable walk: cbSizeVft={cb} ptr_size={ps} slots={slots} (ptr=0x{ptr:x})",
        cb = cb_size_vft,
        ps = ptr_size,
        slots = slots,
        ptr = p as usize,
    );
    // SAFETY: `p` is a live interface pointer; its first field is the vtable base.
    // We read exactly `slots` entries, and `slots` is derived from the OS-reported
    // cbSizeVft, so every `vtbl.add(i)` is in bounds of the live vtable. We only
    // READ the function pointers (and resolve their owning module) — never call them.
    unsafe {
        let vtbl = *p.cast::<*const *const c_void>();
        for i in 0..slots {
            let fnptr = *vtbl.add(i);
            let module = module_of_address(fnptr);
            let mark = annotate
                .iter()
                .find(|(s, _)| *s == i as i64)
                .map(|(_, m)| format!("   <<< typelib oVft slot for {m}"))
                .unwrap_or_default();
            println!(
                "      slot={i} fnptr=0x{fnptr:x} module={module}{mark}",
                i = i,
                fnptr = fnptr as usize,
                module = module,
                mark = mark,
            );
        }
    }
    let _ = granularity;
}

/// Full inspection dump for one target object. Returns nothing; everything here
/// is AV-free.
fn inspect_target(label: &str, obj: *mut RawIDispatch, dao_iids: &[(&str, GUID)]) {
    println!("================= INSPECT: {label} =================");
    // A. bound IDispatch pointer.
    println!("  [A] bound IDispatch = 0x{:x}", obj as usize);

    // B. proxy detection.
    let (hr_proxy, proxy_ptr) = raw_qi(obj.cast(), &IID_IPROXYMANAGER);
    let is_proxy = hr_proxy >= 0 && !proxy_ptr.is_null();
    println!(
        "  [B] QI(IProxyManager) hr=0x{:08X} proxy={is_proxy}",
        hr_proxy as u32
    );
    if !proxy_ptr.is_null() {
        // SAFETY: `proxy_ptr` is an owned interface pointer from a successful QI.
        unsafe { release_unknown(proxy_ptr) };
    }

    // C / E for the IDispatch's own typeinfo (the DECISIVE H-A read).
    println!("  [C/E] IDispatch::GetTypeInfo(0) typeinfo:");
    let ti = type_info_of(obj);
    let disp_attr = dump_typeinfo("IDispatch.GetTypeInfo(0)", ti);

    // Capture the dual GUID from C for the QI matrix.
    let mut dual_guid: Option<GUID> = None;
    if !ti.is_null() {
        let vt = ti.cast::<RawITypeInfo>();
        let mut pattr: *mut TYPEATTR = ptr::null_mut();
        // SAFETY: `ti` is a live ITypeInfo; GetTypeAttr yields an owned TYPEATTR.
        let hr = unsafe { ((*(*vt).vtbl).get_type_attr)(vt.cast(), &mut pattr) };
        if hr >= 0 && !pattr.is_null() {
            // SAFETY: valid TYPEATTR just produced.
            dual_guid = Some(unsafe { (*pattr).guid });
            // SAFETY: releasing the TYPEATTR.
            unsafe { ((*(*vt).vtbl).release_type_attr)(vt.cast(), pattr) };
        }
    }

    // D. QI matrix.
    println!("  [D] QI matrix:");
    let mut qi_targets: Vec<(String, GUID)> = Vec::new();
    if let Some(g) = dual_guid {
        qi_targets.push(("dual-from-C".to_string(), g));
    }
    qi_targets.push(("IDispatch".to_string(), IID_IDISPATCH));
    for (name, iid) in dao_iids {
        qi_targets.push(((*name).to_string(), *iid));
    }
    // Collected (name, ptr, guid) for section F.
    let mut walkable: Vec<(String, *mut c_void, GUID)> = Vec::new();
    for (name, iid) in &qi_targets {
        let (hr, ptr) = raw_qi(obj.cast(), iid);
        let same = !ptr.is_null() && ptr::eq(ptr.cast::<c_void>(), obj.cast::<c_void>());
        println!(
            "      QI({name} {guid}) hr=0x{hr:08X} ptr=0x{p:x} same_as_idispatch={same}",
            name = name,
            guid = fmt_guid(iid),
            hr = hr as u32,
            p = ptr as usize,
            same = same,
        );
        if !ptr.is_null() {
            walkable.push((name.clone(), ptr, *iid));
        }
    }

    // E (already partly done for IDispatch typeinfo above): per QI'd interface's
    // OWN typeinfo. For each walkable interface, GetTypeInfo via IDispatch isn't
    // valid (the QI'd pointer may not be IDispatch); we already dumped the
    // IDispatch typeinfo. For the walk we rely on the IDispatch cbSizeVft for the
    // bound pointer and on a fresh GetTypeAttr where the QI'd ptr is IDispatch.

    // F. raw vtable walk for each candidate pointer.
    println!("  [F] raw vtable walk (bounded by live cbSizeVft):");
    // Annotate Close/Value slots derived from the IDispatch typeinfo.
    let mut annotate: Vec<(i64, &str)> = Vec::new();
    let close_slot = member_slot(ti, "Close", disp_attr.map(|a| a.2).unwrap_or(8));
    let value_slot = member_slot(ti, "Value", disp_attr.map(|a| a.2).unwrap_or(8));
    if let Some(s) = close_slot {
        annotate.push((s, "Close"));
    }
    if let Some(s) = value_slot {
        annotate.push((s, "Value"));
    }

    for (name, ptr, _iid) in &walkable {
        // Determine the cbSizeVft to bound by: prefer the QI'd pointer's own
        // typeinfo if it is itself an IDispatch we can GetTypeInfo on; otherwise
        // reuse the bound IDispatch's cbSizeVft (still a real, OS-reported bound).
        let cb = qi_cb_size_vft(*ptr)
            .or_else(|| disp_attr.map(|a| a.1))
            .unwrap_or(0);
        if cb == 0 {
            println!("      [{name}] no cbSizeVft available; skipping (cannot bound safely)");
            continue;
        }
        walk_vtable(
            name,
            *ptr,
            cb,
            disp_attr.map(|a| a.2).unwrap_or(8),
            &annotate,
        );
    }

    // Release QI'd pointers (skip the ones identity-equal to obj — we don't own
    // an extra ref beyond what QI added; QI always AddRefs, so release each).
    for (_name, ptr, _iid) in &walkable {
        // SAFETY: each `ptr` came from a successful QI that AddRef'd it; we owe
        // exactly one Release. We never use the pointer after this.
        unsafe { release_unknown(*ptr) };
    }
    if !ti.is_null() {
        // SAFETY: `ti` is an owned ITypeInfo reference from GetTypeInfo.
        unsafe { ((*(*ti.cast::<RawITypeInfo>()).vtbl).release)(ti.cast()) };
    }
    println!("================= END INSPECT: {label} =================\n");
}

/// Best-effort: if the QI'd pointer is itself an IDispatch, read its typeinfo's
/// cbSizeVft; else None.
fn qi_cb_size_vft(ptr: *mut c_void) -> Option<u16> {
    // Try QI(IDispatch) on the pointer; if it is one, read GetTypeInfo(0) attr.
    let (hr, disp) = raw_qi(ptr, &IID_IDISPATCH);
    if hr < 0 || disp.is_null() {
        return None;
    }
    let ti = type_info_of(disp.cast());
    let mut result = None;
    if !ti.is_null() {
        let vt = ti.cast::<RawITypeInfo>();
        let mut pattr: *mut TYPEATTR = ptr::null_mut();
        // SAFETY: `ti` is a live ITypeInfo; GetTypeAttr yields an owned TYPEATTR.
        let hr2 = unsafe { ((*(*vt).vtbl).get_type_attr)(vt.cast(), &mut pattr) };
        if hr2 >= 0 && !pattr.is_null() {
            // SAFETY: valid TYPEATTR just produced.
            result = Some(unsafe { (*pattr).cbSizeVft });
            // SAFETY: releasing the TYPEATTR then the owned ITypeInfo.
            unsafe {
                ((*(*vt).vtbl).release_type_attr)(vt.cast(), pattr);
                ((*(*vt).vtbl).release)(vt.cast());
            }
        }
    }
    // SAFETY: `disp` came from a QI that AddRef'd it.
    unsafe { release_unknown(disp) };
    result
}

/// The vtable slot the typelib assigns a member, from its `oVft`/granularity.
fn member_slot(ti: *mut RawITypeInfo, name: &str, granularity: u16) -> Option<i64> {
    if ti.is_null() || granularity == 0 {
        return None;
    }
    let vt = ti.cast::<RawITypeInfo>();
    let mut pattr: *mut TYPEATTR = ptr::null_mut();
    // SAFETY: `ti` is a live ITypeInfo; GetTypeAttr yields an owned TYPEATTR.
    let hr = unsafe { ((*(*vt).vtbl).get_type_attr)(vt.cast(), &mut pattr) };
    if hr < 0 || pattr.is_null() {
        return None;
    }
    // SAFETY: valid TYPEATTR.
    let cfuncs = unsafe { (*pattr).cFuncs };
    let mut slot = None;
    for fi in 0..cfuncs {
        let mut pfd: *mut FUNCDESC = ptr::null_mut();
        // SAFETY: GetFuncDesc(i) for i < cFuncs yields an owned FUNCDESC.
        let hr_fd = unsafe { ((*(*vt).vtbl).get_func_desc)(vt.cast(), u32::from(fi), &mut pfd) };
        if hr_fd < 0 || pfd.is_null() {
            continue;
        }
        // SAFETY: valid FUNCDESC.
        let (memid, ovft) = unsafe { ((*pfd).memid, (*pfd).oVft) };
        if let Some(n) = get_member_name(vt, memid)
            && n.eq_ignore_ascii_case(name)
        {
            slot = Some(i64::from(ovft) / i64::from(granularity));
        }
        // SAFETY: releasing the FUNCDESC.
        unsafe { ((*(*vt).vtbl).release_func_desc)(vt.cast(), pfd) };
        if slot.is_some() {
            break;
        }
    }
    // SAFETY: releasing the TYPEATTR.
    unsafe { ((*(*vt).vtbl).release_type_attr)(vt.cast(), pattr) };
    slot
}

// ── object-graph setup ────────────────────────────────────────────────────────

struct TempDb(std::path::PathBuf);
impl TempDb {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("oxvba_probe_{}.accdb", std::process::id()));
        let _ = std::fs::remove_file(&p);
        Self(p)
    }
    fn as_string(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}
impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn is_not_registered(err: &str) -> bool {
    let l = err.to_ascii_lowercase();
    l.contains("80040154") || l.contains("8004_0154") || l.contains("not registered")
}

/// Built object graph + the temp DB guard kept alive for the test's duration.
struct DaoGraph {
    eng: *mut RawIDispatch,
    db: *mut RawIDispatch,
    rs: *mut RawIDispatch,
    fields: *mut RawIDispatch,
    field0: *mut RawIDispatch,
    _tmp: TempDb,
}

impl Drop for DaoGraph {
    fn drop(&mut self) {
        // Best-effort release of the whole graph (leaf → root).
        // SAFETY: each pointer is either null or a live IDispatch we own one ref on.
        unsafe {
            for p in [self.field0, self.fields, self.rs, self.db, self.eng] {
                if !p.is_null() {
                    release_dispatch(p);
                }
            }
        }
    }
}

/// Build the live DAO object graph via raw GetIDsOfNames + Invoke. Returns
/// `Ok(None)` when DAO is not registered (an environment skip).
fn build_dao_graph() -> Result<Option<DaoGraph>, String> {
    let eng = match activate_dispatch_by_prog_id("DAO.DBEngine.120") {
        Ok(p) => p,
        Err(e) if is_not_registered(&e) => {
            println!("SKIP: DAO not registered ({e})");
            return Ok(None);
        }
        Err(e) => return Err(format!("activate DAO.DBEngine.120 failed: {e}")),
    };

    let tmp = TempDb::new();

    // eng.CreateDatabase(path, options)
    let create_id = dispid_of(eng, "CreateDatabase")
        .map_err(|hr| format!("GetIDsOfNames(CreateDatabase) hr=0x{:08X}", hr as u32))?;
    let mut a_path = bstr_variant(&tmp.as_string());
    let mut a_opts = bstr_variant(";LANGID=0x0409;CP=1252;COUNTRY=0");
    let (hr, db_v) = invoke(
        eng,
        create_id,
        DISPATCH_METHOD | DISPATCH_PROPERTYGET,
        &mut [a_path, a_opts],
    );
    clear_variant(&mut a_path);
    clear_variant(&mut a_opts);
    if hr < 0 {
        return Err(format!("CreateDatabase hr=0x{:08X}", hr as u32));
    }
    let db = dispatch_from_variant(&db_v);
    if db.is_null() {
        return Err("CreateDatabase did not return a Database".to_string());
    }

    // db.Execute "CREATE TABLE ..."
    let exec_id = dispid_of(db, "Execute")
        .map_err(|hr| format!("GetIDsOfNames(Execute) hr=0x{:08X}", hr as u32))?;
    for sql in ["CREATE TABLE T (N LONG)", "INSERT INTO T (N) VALUES (7)"] {
        let mut a_sql = bstr_variant(sql);
        let (hr, mut r) = invoke(db, exec_id, DISPATCH_METHOD, &mut [a_sql]);
        clear_variant(&mut a_sql);
        clear_variant(&mut r);
        if hr < 0 {
            return Err(format!("Execute `{sql}` hr=0x{:08X}", hr as u32));
        }
    }

    // rs = db.OpenRecordset("SELECT N FROM T")
    let open_id = dispid_of(db, "OpenRecordset")
        .map_err(|hr| format!("GetIDsOfNames(OpenRecordset) hr=0x{:08X}", hr as u32))?;
    let mut a_q = bstr_variant("SELECT N FROM T");
    let (hr, rs_v) = invoke(
        db,
        open_id,
        DISPATCH_METHOD | DISPATCH_PROPERTYGET,
        &mut [a_q],
    );
    clear_variant(&mut a_q);
    if hr < 0 {
        return Err(format!("OpenRecordset hr=0x{:08X}", hr as u32));
    }
    let rs = dispatch_from_variant(&rs_v);
    if rs.is_null() {
        return Err("OpenRecordset did not return a Recordset".to_string());
    }

    // fields = rs.Fields ; field0 = fields.Item(0)
    let fields_id = dispid_of(rs, "Fields")
        .map_err(|hr| format!("GetIDsOfNames(Fields) hr=0x{:08X}", hr as u32))?;
    let (hr, fields_v) = invoke(rs, fields_id, DISPATCH_PROPERTYGET, &mut []);
    if hr < 0 {
        return Err(format!("rs.Fields hr=0x{:08X}", hr as u32));
    }
    let fields = dispatch_from_variant(&fields_v);
    if fields.is_null() {
        return Err("rs.Fields did not return a Fields collection".to_string());
    }
    let item_id = dispid_of(fields, "Item")
        .map_err(|hr| format!("GetIDsOfNames(Item) hr=0x{:08X}", hr as u32))?;
    // SAFETY: zero then set a VT_I4(0) discriminant/arm of a fresh VARIANT.
    let mut a_idx = unsafe {
        let mut v: VARIANT = std::mem::zeroed();
        VariantInit(&mut v);
        v.Anonymous.Anonymous.vt = 3; // VT_I4
        v.Anonymous.Anonymous.Anonymous.lVal = 0;
        v
    };
    let (hr, field_v) = invoke(
        fields,
        item_id,
        DISPATCH_METHOD | DISPATCH_PROPERTYGET,
        &mut [a_idx],
    );
    clear_variant(&mut a_idx);
    if hr < 0 {
        return Err(format!("fields.Item(0) hr=0x{:08X}", hr as u32));
    }
    let field0 = dispatch_from_variant(&field_v);
    if field0.is_null() {
        return Err("fields.Item(0) did not return a Field".to_string());
    }

    Ok(Some(DaoGraph {
        eng,
        db,
        rs,
        fields,
        field0,
        _tmp: tmp,
    }))
}

// ── The probe tests ─────────────────────────────────────────────────────────

#[test]
#[ignore = "live DAO; run explicitly; may access-violate the process by design"]
fn probe_recordset_close_vtable_call() {
    // SAFETY: STA matches the host (oxvba-hal/src/adapters/standard/mod.rs:1004).
    let hr_init = unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    println!("CoInitializeEx(STA) hr=0x{:08X}", hr_init as u32);

    let graph = match build_dao_graph() {
        Ok(Some(g)) => g,
        Ok(None) => {
            // SAFETY: balancing CoInitializeEx; no live COM objects remain.
            unsafe { CoUninitialize() };
            return; // SKIP printed inside build_dao_graph.
        }
        Err(e) => {
            // SAFETY: balancing CoInitializeEx.
            unsafe { CoUninitialize() };
            panic!("DAO graph setup failed: {e}");
        }
    };

    let dao_iids = [
        ("DAO.Database?", IID_DAO_DATABASE),
        ("DAO.Recordset?", IID_DAO_RECORDSET),
        ("DAO.Field?", IID_DAO_FIELD),
    ];

    // ── FULL INSPECTION DUMP (AV-safe) ──
    inspect_target("Recordset rs", graph.rs, &dao_iids);
    inspect_target("Database db", graph.db, &dao_iids);
    inspect_target("Field field0", graph.field0, &dao_iids);

    // Compute the Recordset.Close slot from its IDispatch typeinfo for the call.
    let rs_ti = type_info_of(graph.rs);
    // Determine granularity from the typelib for a faithful slot.
    let granularity = type_info_granularity(rs_ti);
    let close_slot = member_slot(rs_ti, "Close", granularity);
    let close_dispid = dispid_of(graph.rs, "Close").ok();
    if !rs_ti.is_null() {
        // SAFETY: owned ITypeInfo from GetTypeInfo.
        unsafe { ((*(*rs_ti.cast::<RawITypeInfo>()).vtbl).release)(rs_ti.cast()) };
    }

    println!("--------- VALUE-ORACLE: Recordset.Close ---------");
    println!(
        "  derived Close slot={:?} dispid={:?} granularity={granularity}",
        close_slot, close_dispid
    );

    io::stdout().flush().ok();
    io::stderr().flush().ok();

    // ── THE SINGLE GUARDED INVOCATION ──
    //
    // (1) SAFE oracle via IDispatch::Invoke (Close is void → VT_EMPTY on success).
    let Some(dispid) = close_dispid else {
        println!("Close dispid unavailable; aborting oracle");
        drop(graph);
        // SAFETY: balancing CoInitializeEx.
        unsafe { CoUninitialize() };
        return;
    };
    let (invoke_hr, mut invoke_result) = invoke(graph.rs, dispid, DISPATCH_METHOD, &mut []);
    println!(
        "  IDispatch::Invoke(Close) hr=0x{:08X} retval={}",
        invoke_hr as u32,
        decode_variant(&invoke_result)
    );
    clear_variant(&mut invoke_result);
    io::stdout().flush().ok();

    let Some(slot) = close_slot else {
        println!("Close slot unavailable; aborting vtable call");
        drop(graph);
        // SAFETY: balancing CoInitializeEx.
        unsafe { CoUninitialize() };
        return;
    };

    // (2) ONE vtable slot call — the exact production ABI deref
    // (windows_vtable.rs:256). For a no-arg void HRESULT method the slot fn is
    // `extern "system" fn(*mut c_void) -> i32`. We do NOT use call_via_libffi: it
    // is `pub(crate)` and not reachable from an integration test (tests/ sees only
    // pub items), and for a scalar/void signature the typed transmute is the
    // identical ABI. NO resuming exception handler — an AV terminates the process
    // (exit -1073741819 / 0xC0000005) and that is itself the datum.
    println!("  >>> ABOUT TO VTABLE-CALL Recordset.Close at slot={slot} (this is the AV risk) <<<");
    io::stdout().flush().ok();

    // SAFETY: this is the deliberate, isolated risk. `graph.rs` is a live interface
    // pointer; we read its vtable base and the slot-`slot` entry exactly as the
    // production path does. If the typelib oVft is bogus for this physical vtable
    // the call faults and kills the process — by design. The transmuted signature
    // is the no-arg HRESULT method ABI (`Close` takes no parameters and returns an
    // HRESULT). No use of the value after a potential fault.
    let vtable_hr: i32 = unsafe {
        let this: *mut c_void = graph.rs.cast();
        let vtbl = *this.cast::<*const *const c_void>();
        let fnptr = *vtbl.add(slot as usize);
        let f: extern "system" fn(*mut c_void) -> i32 = std::mem::transmute(fnptr);
        f(this)
    };

    println!(
        "  VTABLE CALL slot={slot} hr=0x{:08X} retval=<void HRESULT>",
        vtable_hr as u32
    );
    // (3) Value oracle: both are void HRESULT methods; success oracle is HRESULT
    // equality (S_OK both). A wrong in-module function would still return some
    // HRESULT, so this is the weakest oracle for void — the strong signal is "did
    // we survive AND get the same hr".
    if vtable_hr == invoke_hr {
        println!(
            "  ORACLE: MATCH (vtable hr == invoke hr == 0x{:08X})",
            invoke_hr as u32
        );
    } else {
        println!(
            "  ORACLE: MISMATCH (vtable hr=0x{:08X} vs invoke hr=0x{:08X}) — silent corruption",
            vtable_hr as u32, invoke_hr as u32
        );
    }
    io::stdout().flush().ok();

    // Note: rs.Close was already called via Invoke; calling it again via vtable is
    // idempotent for DAO (Close on a closed recordset returns an error HRESULT, not
    // an AV). The point is the SLOT DEREF + CALL, not the side effect.
    drop(graph);
    // SAFETY: balancing CoInitializeEx; the graph's objects were released in Drop.
    unsafe { CoUninitialize() };
}

#[test]
#[ignore = "live DAO; run explicitly; may access-violate the process by design"]
fn probe_field_value_vtable_call() {
    // A SEPARATE test so a crash here doesn't hide the Recordset.Close dump (and
    // vice-versa). Field.Value is a property-get returning a VARIANT* — a richer
    // ABI than void Close; here we use the VALUE oracle properly: the vtable call
    // must write the SAME VARIANT the IDispatch path produced.
    // SAFETY: STA matches the host.
    let hr_init = unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    println!("CoInitializeEx(STA) hr=0x{:08X}", hr_init as u32);

    let graph = match build_dao_graph() {
        Ok(Some(g)) => g,
        Ok(None) => {
            // SAFETY: balancing CoInitializeEx.
            unsafe { CoUninitialize() };
            return;
        }
        Err(e) => {
            // SAFETY: balancing CoInitializeEx.
            unsafe { CoUninitialize() };
            panic!("DAO graph setup failed: {e}");
        }
    };

    // We must move to the first record before Value is meaningful; OpenRecordset
    // on a table/select normally lands on the first row, so Value is readable.
    let value_dispid = match dispid_of(graph.field0, "Value") {
        Ok(id) => id,
        Err(hr) => {
            println!("GetIDsOfNames(Value) hr=0x{:08X}; aborting", hr as u32);
            drop(graph);
            // SAFETY: balancing CoInitializeEx.
            unsafe { CoUninitialize() };
            return;
        }
    };

    let field_ti = type_info_of(graph.field0);
    let granularity = type_info_granularity(field_ti);
    let value_slot = member_slot(field_ti, "Value", granularity);
    if !field_ti.is_null() {
        // SAFETY: owned ITypeInfo.
        unsafe { ((*(*field_ti.cast::<RawITypeInfo>()).vtbl).release)(field_ti.cast()) };
    }
    println!(
        "--------- VALUE-ORACLE: Field.Value --------- slot={:?} dispid={} granularity={granularity}",
        value_slot, value_dispid
    );

    // (1) SAFE oracle via Invoke (property-get).
    let (invoke_hr, mut invoke_result) =
        invoke(graph.field0, value_dispid, DISPATCH_PROPERTYGET, &mut []);
    let invoke_decoded = decode_variant(&invoke_result);
    // SAFETY: reading the VT_I4 arm if applicable for a strong numeric oracle.
    let invoke_i4 = unsafe {
        if invoke_result.Anonymous.Anonymous.vt == 3 {
            Some(invoke_result.Anonymous.Anonymous.Anonymous.lVal)
        } else {
            None
        }
    };
    println!(
        "  IDispatch::Invoke(Value) hr=0x{:08X} retval={invoke_decoded} i4={invoke_i4:?}",
        invoke_hr as u32
    );
    clear_variant(&mut invoke_result);
    io::stdout().flush().ok();

    let Some(slot) = value_slot else {
        println!("Value slot unavailable; aborting vtable call");
        drop(graph);
        // SAFETY: balancing CoInitializeEx.
        unsafe { CoUninitialize() };
        return;
    };

    // (2) ONE vtable slot call. Field.Value is a property-get; the dual-interface
    // ABI is `HRESULT get_Value(this, VARIANT* pRet)`. We transmute to that and
    // pass a fresh out-VARIANT.
    println!("  >>> ABOUT TO VTABLE-CALL Field.Value at slot={slot} (this is the AV risk) <<<");
    io::stdout().flush().ok();

    // SAFETY: fresh zeroed VT_EMPTY out-slot.
    let mut out: VARIANT = unsafe {
        let mut v: VARIANT = std::mem::zeroed();
        VariantInit(&mut v);
        v
    };
    // SAFETY: the deliberate isolated risk. `graph.field0` is a live interface
    // pointer; we read its vtable base and the slot entry exactly as production
    // does, then call the property-get ABI `fn(this, *mut VARIANT) -> HRESULT`
    // with a live out-VARIANT. If the slot is bogus this faults and kills the
    // process by design.
    let vtable_hr: i32 = unsafe {
        let this: *mut c_void = graph.field0.cast();
        let vtbl = *this.cast::<*const *const c_void>();
        let fnptr = *vtbl.add(slot as usize);
        let f: extern "system" fn(*mut c_void, *mut VARIANT) -> i32 = std::mem::transmute(fnptr);
        f(this, &mut out)
    };
    let vtable_decoded = decode_variant(&out);
    // SAFETY: reading the VT_I4 arm if applicable.
    let vtable_i4 = unsafe {
        if out.Anonymous.Anonymous.vt == 3 {
            Some(out.Anonymous.Anonymous.Anonymous.lVal)
        } else {
            None
        }
    };
    println!(
        "  VTABLE CALL slot={slot} hr=0x{:08X} retval={vtable_decoded} i4={vtable_i4:?}",
        vtable_hr as u32
    );
    clear_variant(&mut out);

    // (3) VALUE oracle — both should be VT_I4(7).
    match (invoke_i4, vtable_i4) {
        (Some(a), Some(b)) if a == b => {
            println!("  ORACLE: MATCH (both VT_I4({a}))");
        }
        (a, b) => {
            println!(
                "  ORACLE: MISMATCH (invoke i4={a:?} vs vtable i4={b:?}) — \
                 vtable returned different/garbage value"
            );
        }
    }
    io::stdout().flush().ok();

    drop(graph);
    // SAFETY: balancing CoInitializeEx.
    unsafe { CoUninitialize() };
}

/// Granularity (oVft units) from an ITypeInfo's containing typelib syskind.
fn type_info_granularity(ti: *mut RawITypeInfo) -> u16 {
    if ti.is_null() {
        return if cfg!(target_arch = "x86_64") { 8 } else { 4 };
    }
    let vt = ti.cast::<RawITypeInfo>();
    let mut tlib: *mut c_void = ptr::null_mut();
    let mut index = 0u32;
    // SAFETY: live ITypeInfo; GetContainingTypeLib yields an owned ITypeLib.
    let hr = unsafe { ((*(*vt).vtbl).get_containing_type_lib)(vt.cast(), &mut tlib, &mut index) };
    let mut gran = if cfg!(target_arch = "x86_64") { 8 } else { 4 };
    if hr >= 0 && !tlib.is_null() {
        let lib = tlib.cast::<RawITypeLib>();
        let mut plib: *mut TLIBATTR = ptr::null_mut();
        // SAFETY: live ITypeLib; GetLibAttr yields an owned TLIBATTR.
        let hr2 = unsafe { ((*(*lib).vtbl).get_lib_attr)(lib.cast(), &mut plib) };
        if hr2 >= 0 && !plib.is_null() {
            // SAFETY: valid TLIBATTR.
            let sk = unsafe { (*plib).syskind };
            gran = match sk {
                x if x == SYS_WIN64 => 8,
                x if x == SYS_WIN32 => 4,
                _ => gran,
            };
            // SAFETY: releasing the TLIBATTR.
            unsafe { ((*(*lib).vtbl).release_tlib_attr)(lib.cast(), plib) };
        }
        // SAFETY: releasing the owned ITypeLib.
        unsafe { ((*(*lib).vtbl).release)(lib.cast()) };
    }
    gran
}

// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  DUAL-PARTNER FACE PROBE                                                   ║
// ║                                                                            ║
// ║  The first probe (above) inspected and called the DISPINTERFACE face that  ║
// ║  IDispatch::GetTypeInfo(0) returns for a DAO object. For a *dual* interface ║
// ║  that face is `TKIND_DISPATCH` with FDUAL=true, a SHORT 7-IDispatch-slot    ║
// ║  vtable (cbSizeVft=56), and FUNCDESC.oVft values authored for the PARTNER   ║
// ║  vtable interface. Calling the oVft-derived slot on the 7-slot dispinterface ║
// ║  pointer reads out of bounds → garbage.                                     ║
// ║                                                                            ║
// ║  This section crosses to the FDUAL PARTNER INTERFACE typeinfo (a           ║
// ║  TKIND_INTERFACE with a DIFFERENT IID and the FULL ~99-slot vtable) via the ║
// ║  canonical `GetRefTypeOfImplType(-1)` → `GetRefTypeInfo`, then QIs the live  ║
// ║  object for that INTERFACE IID and tests whether the oVft-derived slot on    ║
// ║  *that* pointer is the real method. THIS is the canonical correct way to     ║
// ║  call a dual's vtable (the design's S3 premise).                            ║
// ╚══════════════════════════════════════════════════════════════════════════╝

/// Cross from a dual `dispinterface` ITypeInfo to its FDUAL PARTNER
/// `TKIND_INTERFACE` ITypeInfo. The canonical crossing is
/// `GetRefTypeOfImplType(-1)` → `GetRefTypeInfo`; some authoring uses impl-type
/// index 0 instead, so we fall back to 0. Returns the owned partner ITypeInfo
/// (caller releases) plus which index worked. AV-free: pure ITypeInfo reads.
fn cross_to_partner_interface(ti: *mut RawITypeInfo) -> (Option<*mut RawITypeInfo>, &'static str) {
    if ti.is_null() {
        return (None, "<dispinterface ti is NULL>");
    }
    let vt = ti.cast::<RawITypeInfo>();
    // Try index -1 (== u32::MAX bit pattern) first, then 0.
    for (idx, label) in [(u32::MAX, "-1"), (0u32, "0")] {
        let mut href: u32 = 0;
        // SAFETY: `ti` is a live ITypeInfo; GetRefTypeOfImplType fills `href` with
        // an HREFTYPE we then resolve. `-1` requests the FDUAL partner; an invalid
        // index returns a failure HRESULT (no out-of-bounds read).
        let hr_ref =
            unsafe { ((*(*vt).vtbl).get_ref_type_of_impl_type)(vt.cast(), idx, &mut href) };
        if hr_ref < 0 {
            println!(
                "      GetRefTypeOfImplType({label}) failed hr=0x{:08X}",
                hr_ref as u32
            );
            continue;
        }
        let mut partner: *mut c_void = ptr::null_mut();
        // SAFETY: `ti` is a live ITypeInfo; GetRefTypeInfo(href) yields an owned
        // ITypeInfo for the HREFTYPE just produced by GetRefTypeOfImplType.
        let hr_ti = unsafe { ((*(*vt).vtbl).get_ref_type_info)(vt.cast(), href, &mut partner) };
        if hr_ti < 0 || partner.is_null() {
            println!(
                "      GetRefTypeInfo(href=0x{href:08X} from idx {label}) failed hr=0x{:08X}",
                hr_ti as u32
            );
            continue;
        }
        let worked: &'static str = if idx == u32::MAX {
            "GetRefTypeOfImplType(-1)"
        } else {
            "GetRefTypeOfImplType(0)"
        };
        return (Some(partner.cast::<RawITypeInfo>()), worked);
    }
    (None, "<no impl-type crossing worked>")
}

/// One member's `(oVft, invkind, cParams, callconv)` from an ITypeInfo, matched
/// by name. AV-free.
struct MemberDesc {
    o_vft: i16,
    invkind: i32,
    c_params: i16,
    callconv: i32,
}

fn member_desc(ti: *mut RawITypeInfo, name: &str) -> Option<MemberDesc> {
    if ti.is_null() {
        return None;
    }
    let vt = ti.cast::<RawITypeInfo>();
    let mut pattr: *mut TYPEATTR = ptr::null_mut();
    // SAFETY: live ITypeInfo; GetTypeAttr yields an owned TYPEATTR.
    let hr = unsafe { ((*(*vt).vtbl).get_type_attr)(vt.cast(), &mut pattr) };
    if hr < 0 || pattr.is_null() {
        return None;
    }
    // SAFETY: valid TYPEATTR.
    let cfuncs = unsafe { (*pattr).cFuncs };
    let mut found = None;
    for fi in 0..cfuncs {
        let mut pfd: *mut FUNCDESC = ptr::null_mut();
        // SAFETY: GetFuncDesc(i) for i < cFuncs yields an owned FUNCDESC.
        let hr_fd = unsafe { ((*(*vt).vtbl).get_func_desc)(vt.cast(), u32::from(fi), &mut pfd) };
        if hr_fd < 0 || pfd.is_null() {
            continue;
        }
        // SAFETY: valid FUNCDESC.
        let fd = unsafe { *pfd };
        if let Some(n) = get_member_name(vt, fd.memid)
            && n.eq_ignore_ascii_case(name)
        {
            found = Some(MemberDesc {
                o_vft: fd.oVft,
                invkind: fd.invkind,
                c_params: fd.cParams,
                callconv: fd.callconv,
            });
        }
        // SAFETY: releasing the FUNCDESC.
        unsafe { ((*(*vt).vtbl).release_func_desc)(vt.cast(), pfd) };
        if found.is_some() {
            break;
        }
    }
    // SAFETY: releasing the TYPEATTR.
    unsafe { ((*(*vt).vtbl).release_type_attr)(vt.cast(), pattr) };
    found
}

/// Find a member's FUNCDESC by name, searching (in order) the partner INTERFACE
/// itself, its base-interface chain (`GetRefTypeOfImplType(0)` → `GetRefTypeInfo`,
/// repeated), then the dispinterface face (which lists ALL members with their
/// oVft). Returns the desc plus a human label of which face supplied it. AV-free.
fn member_desc_with_source(
    partner_ti: *mut RawITypeInfo,
    disp_ti: *mut RawITypeInfo,
    name: &str,
) -> Option<(MemberDesc, String)> {
    // 1. the partner INTERFACE's own funcs.
    if let Some(md) = member_desc(partner_ti, name) {
        return Some((md, "partner-INTERFACE".to_string()));
    }
    // 2. walk the base-interface chain via impl-type index 0.
    let mut cur = partner_ti;
    let mut owned_chain: Vec<*mut RawITypeInfo> = Vec::new();
    let mut depth = 0u32;
    let result = loop {
        depth += 1;
        if depth > 8 {
            break None; // guard against cycles.
        }
        let vt = cur.cast::<RawITypeInfo>();
        let mut href: u32 = 0;
        // SAFETY: `cur` is a live ITypeInfo; impl-type 0 is the base interface for a
        // TKIND_INTERFACE. An invalid index returns a failure HRESULT.
        let hr_ref = unsafe { ((*(*vt).vtbl).get_ref_type_of_impl_type)(vt.cast(), 0, &mut href) };
        if hr_ref < 0 {
            break None;
        }
        let mut base: *mut c_void = ptr::null_mut();
        // SAFETY: `cur` is a live ITypeInfo; GetRefTypeInfo(href) yields an owned base.
        let hr_ti = unsafe { ((*(*vt).vtbl).get_ref_type_info)(vt.cast(), href, &mut base) };
        if hr_ti < 0 || base.is_null() {
            break None;
        }
        let base_ti = base.cast::<RawITypeInfo>();
        owned_chain.push(base_ti);
        if let Some(md) = member_desc(base_ti, name) {
            break Some((md, format!("base-INTERFACE depth={depth}")));
        }
        cur = base_ti;
    };
    // Release everything we crossed into.
    for ti in owned_chain {
        // SAFETY: each base ITypeInfo came from GetRefTypeInfo (owned).
        unsafe { ((*(*ti.cast::<RawITypeInfo>()).vtbl).release)(ti.cast()) };
    }
    if let Some(found) = result {
        return Some(found);
    }
    // 3. the dispinterface face lists ALL members with oVft authored for the
    //    partner vtable.
    if let Some(md) = member_desc(disp_ti, name) {
        return Some((md, "dispinterface-face".to_string()));
    }
    None
}

/// Read `(guid, typekind, fdual, cFuncs, cbSizeVft, syskind-granularity)` from an
/// ITypeInfo. AV-free.
struct IfaceAttr {
    guid: GUID,
    typekind: i32,
    fdual: bool,
    c_funcs: u16,
    cb_size_vft: u16,
    granularity: u16,
    syskind: String,
}

fn iface_attr(ti: *mut RawITypeInfo) -> Option<IfaceAttr> {
    if ti.is_null() {
        return None;
    }
    let vt = ti.cast::<RawITypeInfo>();
    let mut pattr: *mut TYPEATTR = ptr::null_mut();
    // SAFETY: live ITypeInfo; GetTypeAttr yields an owned TYPEATTR.
    let hr = unsafe { ((*(*vt).vtbl).get_type_attr)(vt.cast(), &mut pattr) };
    if hr < 0 || pattr.is_null() {
        return None;
    }
    // SAFETY: valid TYPEATTR.
    let attr = unsafe { *pattr };
    let granularity = type_info_granularity(ti);
    let syskind = match granularity {
        8 => "SYS_WIN64".to_string(),
        4 => "SYS_WIN32".to_string(),
        other => format!("gran={other}"),
    };
    let out = IfaceAttr {
        guid: attr.guid,
        typekind: attr.typekind,
        fdual: (attr.wTypeFlags & TYPEFLAG_FDUAL) != 0,
        c_funcs: attr.cFuncs,
        cb_size_vft: attr.cbSizeVft,
        granularity,
        syskind,
    };
    // SAFETY: releasing the TYPEATTR.
    unsafe { ((*(*vt).vtbl).release_type_attr)(vt.cast(), pattr) };
    Some(out)
}

/// Walk a QI'd interface pointer's raw vtable and report how many slots are
/// backed by ACEDAO.DLL (the true DAO method count). Bounds the walk by
/// `cb_size_vft / authored_ptr_size` (the typelib-authored slot count) but reads
/// each slot at the LIVE 8-byte stride (64-bit process). Annotates the candidate
/// slots passed in. AV-free (only reads fnptrs + resolves their module).
fn walk_iface_vtable(
    label: &str,
    p: *mut c_void,
    cb_size_vft: u16,
    authored_granularity: u16,
    annotate: &[(i64, String)],
) {
    if p.is_null() {
        println!("    [walk:{label}] pointer is NULL, skipping");
        return;
    }
    let count_at_8 = if cb_size_vft == 0 {
        0
    } else {
        usize::from(cb_size_vft / 8)
    };
    let count_at_4 = if cb_size_vft == 0 {
        0
    } else {
        usize::from(cb_size_vft / 4)
    };
    if count_at_8 != count_at_4 {
        println!(
            "    [walk:{label}] cbSizeVft={cb} → count@8={c8} count@4={c4} (DISAGREE — \
             authored granularity={ag})",
            cb = cb_size_vft,
            c8 = count_at_8,
            c4 = count_at_4,
            ag = authored_granularity,
        );
    }
    // Walk using the LIVE 8-byte stride (64-bit), up to (cbSizeVft / authored_ptr_size)
    // slots — i.e. the number of method entries the typelib authored.
    let authored_ptr_size = if authored_granularity == 0 {
        8
    } else {
        authored_granularity
    };
    let slots = if cb_size_vft == 0 {
        0
    } else {
        usize::from(cb_size_vft / authored_ptr_size)
    };
    println!(
        "    [walk:{label}] live-stride=8 authored_ptr_size={aps} slots={slots} (ptr=0x{ptr:x})",
        aps = authored_ptr_size,
        slots = slots,
        ptr = p as usize,
    );
    let mut acedao_backed = 0usize;
    let mut last_backed: i64 = -1;
    // SAFETY: `p` is a live interface pointer; its first field is the vtable base.
    // We read exactly `slots` entries at the 8-byte live stride; `slots` is bounded
    // by the OS-reported cbSizeVft, so every `vtbl.add(i)` is in bounds. We only
    // READ the fnptrs and resolve their owning module — never call them.
    unsafe {
        let vtbl = *p.cast::<*const *const c_void>();
        for i in 0..slots {
            let fnptr = *vtbl.add(i);
            let module = module_of_address(fnptr);
            let is_acedao = module.eq_ignore_ascii_case("acedao.dll");
            if is_acedao {
                acedao_backed += 1;
                last_backed = i as i64;
            }
            let mark = annotate
                .iter()
                .find(|(s, _)| *s == i as i64)
                .map(|(_, m)| format!("   <<< candidate slot for {m}"))
                .unwrap_or_default();
            println!(
                "        slot={i} fnptr=0x{fnptr:x} module={module}{mark}",
                i = i,
                fnptr = fnptr as usize,
                module = module,
                mark = mark,
            );
        }
    }
    println!(
        "    [walk:{label}] ACEDAO.DLL-backed slots={acedao_backed} (last backed slot index={last_backed}) \
         → true in-module method count ≈ {acedao_backed}",
    );
}

/// Inspect ONE target object's dual-partner INTERFACE face. AV-free: crossing,
/// dump, QI, and the bounded vtable walk only. Returns the QI'd INTERFACE
/// pointer + the partner's `(member oVft, granularity)` map for the value-oracle
/// tests — but here we just dump; the oracle tests re-derive independently so a
/// crash in one doesn't depend on shared state.
fn inspect_partner_interface(label: &str, obj: *mut RawIDispatch, members: &[&str]) {
    println!("================= PARTNER-INTERFACE INSPECT: {label} =================");

    // Step 0: the dispinterface typeinfo (GetTypeInfo(0)).
    let disp_ti = type_info_of(obj);
    if let Some(a) = iface_attr(disp_ti) {
        println!(
            "  [dispinterface face] guid={g} typekind={tk}({tkn}) FDUAL={fd} cFuncs={cf} cbSizeVft={cb}",
            g = fmt_guid(&a.guid),
            tk = a.typekind,
            tkn = typekind_name(a.typekind),
            fd = a.fdual,
            cf = a.c_funcs,
            cb = a.cb_size_vft,
        );
    } else {
        println!("  [dispinterface face] GetTypeInfo(0)/GetTypeAttr failed");
    }

    // Step 1: cross to the FDUAL partner INTERFACE typeinfo.
    let (partner_opt, which) = cross_to_partner_interface(disp_ti);
    println!("  [1] dual-partner crossing via: {which}");
    let Some(partner_ti) = partner_opt else {
        println!("  [1] could not reach the partner INTERFACE typeinfo; aborting this target");
        if !disp_ti.is_null() {
            // SAFETY: owned ITypeInfo from GetTypeInfo(0).
            unsafe { ((*(*disp_ti.cast::<RawITypeInfo>()).vtbl).release)(disp_ti.cast()) };
        }
        println!("================= END PARTNER-INTERFACE INSPECT: {label} =================\n");
        return;
    };

    // Step 2: dump the partner INTERFACE typeinfo attributes + member descs.
    let Some(pa) = iface_attr(partner_ti) else {
        println!("  [2] partner GetTypeAttr failed; aborting");
        // SAFETY: owned ITypeInfos.
        unsafe {
            ((*(*partner_ti.cast::<RawITypeInfo>()).vtbl).release)(partner_ti.cast());
            if !disp_ti.is_null() {
                ((*(*disp_ti.cast::<RawITypeInfo>()).vtbl).release)(disp_ti.cast());
            }
        }
        return;
    };
    println!(
        "  [2] INTERFACE typeinfo: guid={g} typekind={tk}({tkn}) FDUAL={fd} cFuncs={cf} \
         cbSizeVft={cb} syskind={sk} oVft-granularity={gran}",
        g = fmt_guid(&pa.guid),
        tk = pa.typekind,
        tkn = typekind_name(pa.typekind),
        fd = pa.fdual,
        cf = pa.c_funcs,
        cb = pa.cb_size_vft,
        sk = pa.syskind,
        gran = pa.granularity,
    );
    // candidate slots per member, at BOTH granularities. NOTE: a dual's leaf
    // INTERFACE typeinfo lists only its OWN funcs (cFuncs is small); inherited
    // members (Close/Fields/Value) live on a base interface or are described on
    // the dispinterface face (which lists ALL members with their oVft). We search,
    // in order: the partner INTERFACE, its base-interface chain, then the
    // dispinterface face — and report which face supplied the oVft.
    let mut annotate: Vec<(i64, String)> = Vec::new();
    for m in members {
        match member_desc_with_source(partner_ti, disp_ti, m) {
            Some((md, src)) => {
                let slot4 = i64::from(md.o_vft) / 4;
                let slot8 = i64::from(md.o_vft) / 8;
                println!(
                    "      member `{m}` [from {src}] oVft={ov} invkind={ik}({ikn}) cParams={cp} \
                     callconv={cc} slot@oVft/4={s4} slot@oVft/8={s8}",
                    src = src,
                    ov = md.o_vft,
                    ik = md.invkind,
                    ikn = invkind_name(md.invkind),
                    cp = md.c_params,
                    cc = md.callconv,
                    s4 = slot4,
                    s8 = slot8,
                );
                annotate.push((slot4, format!("{m}@oVft/4")));
                annotate.push((slot8, format!("{m}@oVft/8")));
            }
            None => {
                println!(
                    "      member `{m}` NOT FOUND on partner INTERFACE, its base chain, or the \
                     dispinterface face"
                );
            }
        }
    }

    // Step 3: QI the live object for the INTERFACE IID from step 2.
    let (hr_qi, iface_ptr) = raw_qi(obj.cast(), &pa.guid);
    let same = !iface_ptr.is_null() && ptr::eq(iface_ptr.cast::<c_void>(), obj.cast::<c_void>());
    println!(
        "  [3] QI(INTERFACE {g}) hr=0x{hr:08X} ptr=0x{p:x} same_as_idispatch={same}",
        g = fmt_guid(&pa.guid),
        hr = hr_qi as u32,
        p = iface_ptr as usize,
        same = same,
    );

    // Step 4: raw vtable walk of the QI'd INTERFACE pointer.
    if !iface_ptr.is_null() {
        println!("  [4] raw vtable walk of the QI'd INTERFACE pointer:");
        walk_iface_vtable(label, iface_ptr, pa.cb_size_vft, pa.granularity, &annotate);
        // SAFETY: `iface_ptr` came from a successful QI that AddRef'd it.
        unsafe { release_unknown(iface_ptr) };
    } else {
        println!("  [4] no INTERFACE pointer (QI failed); skipping vtable walk");
    }

    // SAFETY: owned ITypeInfos from the crossing + GetTypeInfo(0).
    unsafe {
        ((*(*partner_ti.cast::<RawITypeInfo>()).vtbl).release)(partner_ti.cast());
        if !disp_ti.is_null() {
            ((*(*disp_ti.cast::<RawITypeInfo>()).vtbl).release)(disp_ti.cast());
        }
    }
    println!("================= END PARTNER-INTERFACE INSPECT: {label} =================\n");
}

/// Cross to the partner INTERFACE, read a member's oVft + the INTERFACE IID, and
/// QI the live object for that IID. Returns `(qi'd interface ptr, oVft)` (caller
/// releases the ptr). AV-free. Used by the value-oracle tests so each test
/// re-derives independently (no shared state across process-isolated tests).
fn partner_iface_member(obj: *mut RawIDispatch, member: &str) -> Option<(*mut c_void, GUID, i32)> {
    let disp_ti = type_info_of(obj);
    let (partner_opt, _which) = cross_to_partner_interface(disp_ti);
    let partner_ti = partner_opt?;
    let attr = iface_attr(partner_ti);
    // Source the oVft from whichever face describes the member (partner INTERFACE,
    // its base chain, or the dispinterface face) — inherited members like
    // Close/Value are NOT on the leaf INTERFACE typeinfo.
    let md_src = member_desc_with_source(partner_ti, disp_ti, member);
    // SAFETY: owned ITypeInfos.
    unsafe {
        ((*(*partner_ti.cast::<RawITypeInfo>()).vtbl).release)(partner_ti.cast());
        if !disp_ti.is_null() {
            ((*(*disp_ti.cast::<RawITypeInfo>()).vtbl).release)(disp_ti.cast());
        }
    }
    let attr = attr?;
    let (md, src) = md_src?;
    println!("  member `{member}` oVft sourced from: {src}");
    let (hr, iface_ptr) = raw_qi(obj.cast(), &attr.guid);
    if hr < 0 || iface_ptr.is_null() {
        println!(
            "  QI(INTERFACE {g}) failed hr=0x{:08X}",
            hr as u32,
            g = fmt_guid(&attr.guid),
        );
        return None;
    }
    Some((iface_ptr, attr.guid, i32::from(md.o_vft)))
}

// ── PARTNER-FACE INSPECTION TEST (AV-free) ───────────────────────────────────

#[test]
#[ignore = "live DAO; run explicitly; AV-free inspection of the dual-partner INTERFACE face"]
fn probe_partner_interface_inspect() {
    // SAFETY: STA matches the host.
    let hr_init = unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    println!("CoInitializeEx(STA) hr=0x{:08X}", hr_init as u32);

    let graph = match build_dao_graph() {
        Ok(Some(g)) => g,
        Ok(None) => {
            // SAFETY: balancing CoInitializeEx.
            unsafe { CoUninitialize() };
            return;
        }
        Err(e) => {
            // SAFETY: balancing CoInitializeEx.
            unsafe { CoUninitialize() };
            panic!("DAO graph setup failed: {e}");
        }
    };

    inspect_partner_interface("Recordset rs", graph.rs, &["Close", "Fields"]);
    inspect_partner_interface("Field field0", graph.field0, &["Value"]);

    io::stdout().flush().ok();
    drop(graph);
    // SAFETY: balancing CoInitializeEx.
    unsafe { CoUninitialize() };
}

// ── VALUE-ORACLE TESTS (each its own process via --test-threads=1) ───────────

/// Shared body for the Field.Value partner-face value-oracle. `divisor` chooses
/// the slot derivation granularity (oVft/4 or oVft/8). Always calls the SAFE
/// IDispatch::Invoke oracle FIRST, then the ONE guarded vtable call on the QI'd
/// INTERFACE pointer.
fn field_value_partner_oracle(divisor: i32) {
    // SAFETY: STA matches the host.
    let hr_init = unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    println!("CoInitializeEx(STA) hr=0x{:08X}", hr_init as u32);

    let graph = match build_dao_graph() {
        Ok(Some(g)) => g,
        Ok(None) => {
            // SAFETY: balancing CoInitializeEx.
            unsafe { CoUninitialize() };
            return;
        }
        Err(e) => {
            // SAFETY: balancing CoInitializeEx.
            unsafe { CoUninitialize() };
            panic!("DAO graph setup failed: {e}");
        }
    };

    // (A) SAFE oracle via IDispatch::Invoke FIRST.
    let value_dispid = match dispid_of(graph.field0, "Value") {
        Ok(id) => id,
        Err(hr) => {
            println!("GetIDsOfNames(Value) hr=0x{:08X}; aborting", hr as u32);
            drop(graph);
            // SAFETY: balancing CoInitializeEx.
            unsafe { CoUninitialize() };
            return;
        }
    };
    let (invoke_hr, mut invoke_result) =
        invoke(graph.field0, value_dispid, DISPATCH_PROPERTYGET, &mut []);
    let invoke_decoded = decode_variant(&invoke_result);
    // SAFETY: reading the VT_I4 arm for the strong numeric oracle.
    let invoke_i4 = unsafe {
        if invoke_result.Anonymous.Anonymous.vt == 3 {
            Some(invoke_result.Anonymous.Anonymous.Anonymous.lVal)
        } else {
            None
        }
    };
    println!(
        "  ORACLE IDispatch::Invoke(Value) hr=0x{:08X} retval={invoke_decoded} i4={invoke_i4:?}",
        invoke_hr as u32
    );
    clear_variant(&mut invoke_result);
    io::stdout().flush().ok();

    // (B) Cross to the partner INTERFACE, QI it, derive the slot.
    let Some((iface_ptr, iface_guid, o_vft)) = partner_iface_member(graph.field0, "Value") else {
        println!("  partner INTERFACE / Value member unavailable; aborting vtable call");
        drop(graph);
        // SAFETY: balancing CoInitializeEx.
        unsafe { CoUninitialize() };
        return;
    };
    let slot = (o_vft / divisor) as usize;
    println!(
        "  partner INTERFACE iid={g} ptr=0x{p:x} oVft={ov} divisor={divisor} → slot={slot}",
        g = fmt_guid(&iface_guid),
        p = iface_ptr as usize,
        ov = o_vft,
        divisor = divisor,
        slot = slot,
    );
    println!(
        "  >>> ABOUT TO VTABLE-CALL Field.Value at slot={slot} on the INTERFACE face (AV risk) <<<"
    );
    io::stdout().flush().ok();

    // (C) ONE guarded vtable call — production deref shape. Field.Value is a
    // property-get returning a VARIANT*: `HRESULT get_Value(this, VARIANT* pRet)`.
    // SAFETY: fresh zeroed VT_EMPTY out-slot.
    let mut out: VARIANT = unsafe {
        let mut v: VARIANT = std::mem::zeroed();
        VariantInit(&mut v);
        v
    };
    // SAFETY: the deliberate isolated risk. `iface_ptr` is a live INTERFACE pointer
    // from a successful QI; we read its vtable base and the slot entry exactly as
    // production does (windows_vtable.rs:256), then call the property-get ABI
    // `fn(this, *mut VARIANT) -> HRESULT` with a live out-VARIANT. A bogus slot
    // faults and kills the process by design.
    let vtable_hr: i32 = unsafe {
        let this: *mut c_void = iface_ptr;
        let vtbl = *this.cast::<*const *const c_void>();
        let fnptr = *vtbl.add(slot);
        let f: extern "system" fn(*mut c_void, *mut VARIANT) -> i32 = std::mem::transmute(fnptr);
        f(this, &mut out)
    };
    let vtable_decoded = decode_variant(&out);
    // SAFETY: reading the VT_I4 arm if applicable.
    let vtable_i4 = unsafe {
        if out.Anonymous.Anonymous.vt == 3 {
            Some(out.Anonymous.Anonymous.Anonymous.lVal)
        } else {
            None
        }
    };
    println!(
        "  VTABLE CALL slot={slot} hr=0x{:08X} retval={vtable_decoded} i4={vtable_i4:?}",
        vtable_hr as u32
    );
    clear_variant(&mut out);

    // (D) VALUE oracle — success is VALUE EQUALITY (both VT_I4(7)), not "no AV".
    match (invoke_i4, vtable_i4) {
        (Some(a), Some(b)) if a == b => {
            println!(
                "  ORACLE: MATCH (both VT_I4({a})) — partner-face vtable dispatch WORKS at this slot"
            );
        }
        (a, b) => {
            println!(
                "  ORACLE: MISMATCH (invoke i4={a:?} vs vtable i4={b:?}) — \
                 partner-face vtable returned a different/garbage value at this slot"
            );
        }
    }
    io::stdout().flush().ok();

    // SAFETY: `iface_ptr` came from a successful QI that AddRef'd it.
    unsafe { release_unknown(iface_ptr) };
    drop(graph);
    // SAFETY: balancing CoInitializeEx.
    unsafe { CoUninitialize() };
}

#[test]
#[ignore = "live DAO; run explicitly; may access-violate the process by design"]
fn probe_v4_field_value_partner_ovft_div4() {
    // Fn V4: Field.Value on the QI'd partner INTERFACE pointer, slot = oVft/4.
    field_value_partner_oracle(4);
}

#[test]
#[ignore = "live DAO; run explicitly; may access-violate the process by design"]
fn probe_v8_field_value_partner_ovft_div8() {
    // Fn V8: Field.Value on the QI'd partner INTERFACE pointer, slot = oVft/8.
    field_value_partner_oracle(8);
}

/// Shared body for the Recordset.Close partner-face call. Close is void
/// (`HRESULT Close(this)`). Calls the SAFE IDispatch oracle FIRST, then the ONE
/// guarded vtable call on the QI'd INTERFACE pointer at slot = oVft/`divisor`.
fn recordset_close_partner_oracle(divisor: i32) {
    // SAFETY: STA matches the host.
    let hr_init = unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    println!("CoInitializeEx(STA) hr=0x{:08X}", hr_init as u32);

    let graph = match build_dao_graph() {
        Ok(Some(g)) => g,
        Ok(None) => {
            // SAFETY: balancing CoInitializeEx.
            unsafe { CoUninitialize() };
            return;
        }
        Err(e) => {
            // SAFETY: balancing CoInitializeEx.
            unsafe { CoUninitialize() };
            panic!("DAO graph setup failed: {e}");
        }
    };

    // (A) Cross to the partner INTERFACE, QI it, derive the slot BEFORE we close
    // via Invoke (so the INTERFACE-face call below targets the same live object;
    // Close-on-closed returns an error HRESULT, not an AV — the point is the deref).
    let Some((iface_ptr, iface_guid, o_vft)) = partner_iface_member(graph.rs, "Close") else {
        println!("  partner INTERFACE / Close member unavailable; aborting");
        drop(graph);
        // SAFETY: balancing CoInitializeEx.
        unsafe { CoUninitialize() };
        return;
    };
    let slot = (o_vft / divisor) as usize;
    println!(
        "  partner INTERFACE iid={g} ptr=0x{p:x} Close oVft={ov} divisor={divisor} → slot={slot}",
        g = fmt_guid(&iface_guid),
        p = iface_ptr as usize,
        ov = o_vft,
        divisor = divisor,
        slot = slot,
    );

    // (B) SAFE oracle via IDispatch::Invoke FIRST (void → VT_EMPTY on success).
    let close_dispid = dispid_of(graph.rs, "Close").ok();
    let invoke_hr = if let Some(id) = close_dispid {
        let (hr, mut r) = invoke(graph.rs, id, DISPATCH_METHOD, &mut []);
        println!(
            "  ORACLE IDispatch::Invoke(Close) hr=0x{:08X} retval={}",
            hr as u32,
            decode_variant(&r)
        );
        clear_variant(&mut r);
        Some(hr)
    } else {
        println!("  Close dispid unavailable; no Invoke oracle");
        None
    };
    io::stdout().flush().ok();

    // Re-open a fresh recordset so the INTERFACE-face Close targets a live (not
    // already-closed) cursor — gives the cleanest hr comparison. We reuse the same
    // partner-face derivation on the fresh rs.
    println!(
        "  >>> ABOUT TO VTABLE-CALL Recordset.Close at slot={slot} on the INTERFACE face (AV risk) <<<"
    );
    io::stdout().flush().ok();

    // (C) ONE guarded vtable call — production deref shape, void HRESULT ABI.
    // SAFETY: the deliberate isolated risk. `iface_ptr` is a live INTERFACE pointer
    // from a successful QI; we read its vtable base and the slot entry exactly as
    // production does, then call the no-arg `fn(this) -> HRESULT` ABI. A bogus slot
    // faults and kills the process by design.
    let vtable_hr: i32 = unsafe {
        let this: *mut c_void = iface_ptr;
        let vtbl = *this.cast::<*const *const c_void>();
        let fnptr = *vtbl.add(slot);
        let f: extern "system" fn(*mut c_void) -> i32 = std::mem::transmute(fnptr);
        f(this)
    };
    println!(
        "  VTABLE CALL slot={slot} hr=0x{:08X} retval=<void HRESULT>",
        vtable_hr as u32
    );

    // (D) Oracle for void: survival + a plausible HRESULT. (Close-on-closed yields
    // an error HRESULT; the strong signal is "did we survive AND land in ACEDAO".)
    match invoke_hr {
        Some(ih) if ih == vtable_hr => {
            println!(
                "  ORACLE: MATCH (vtable hr == invoke hr == 0x{:08X})",
                ih as u32
            );
        }
        Some(ih) => {
            println!(
                "  ORACLE: hr differ (vtable=0x{:08X} invoke=0x{:08X}); survival without AV is \
                 the partner-face signal for void Close",
                vtable_hr as u32, ih as u32
            );
        }
        None => {
            println!(
                "  ORACLE: no invoke baseline; vtable Close hr=0x{:08X} (survival is the signal)",
                vtable_hr as u32
            );
        }
    }
    io::stdout().flush().ok();

    // SAFETY: `iface_ptr` came from a successful QI that AddRef'd it.
    unsafe { release_unknown(iface_ptr) };
    drop(graph);
    // SAFETY: balancing CoInitializeEx.
    unsafe { CoUninitialize() };
}

#[test]
#[ignore = "live DAO; run explicitly; may access-violate the process by design"]
fn probe_c4_recordset_close_partner_ovft_div4() {
    // Fn C (oVft/4): Recordset.Close on the QI'd partner INTERFACE pointer.
    recordset_close_partner_oracle(4);
}

#[test]
#[ignore = "live DAO; run explicitly; may access-violate the process by design"]
fn probe_c8_recordset_close_partner_ovft_div8() {
    // Fn C (oVft/8): Recordset.Close on the QI'd partner INTERFACE pointer.
    recordset_close_partner_oracle(8);
}

// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  EXCEL  _Application  OUT-OF-PROCESS  VTABLE  ISOLATION  LADDER            ║
// ║                                                                            ║
// ║  Excel's `_Application` {000208D5-…} is marshaled by PSOAInterface         ║
// ║  {00020424} = the oleaut universal/typelib marshaler (oleaut32 hosted in   ║
// ║  combase). That marshaler builds a TYPELIB-ALIGNED vtable proxy: the proxy ║
// ║  pointer we QI for {000208D5} IS vtable-callable by `slot = oVft / 8`.     ║
// ║  (Contrast Range {00020846}, marshaled by PSDispatch {00020420} = IDispatch║
// ║  only — NOT vtable-callable. We never touch Range here.)                   ║
// ║                                                                            ║
// ║  A prior worker called `_Application.Build` on the QI'd proxy and it AV'd.  ║
// ║  The conclusion "fundamental NDR" was wrong (combase ≠ NDR; this is        ║
// ║  oleaut). So the AV is a SLOT or INVOCATION bug. This ladder calls slots   ║
// ║  in increasing complexity on the LIVE out-of-process proxy to localize it: ║
// ║                                                                            ║
// ║    Rung A  IUnknown::AddRef  (slot 1)        — local proxy op, no RPC      ║
// ║    Rung B  IDispatch::GetTypeInfoCount (slot 3)                            ║
// ║    Rung C  the custom slot `Build` at oVft/8 and (separately) oVft/4       ║
// ║    Rung D  the custom propget `Version` (BSTR* out) at oVft/8              ║
// ║                                                                            ║
// ║  Each rung is its OWN #[ignore] test → its OWN process under               ║
// ║  --test-threads=1, so an AV in one rung (exit -1073741819 / 0xC0000005)    ║
// ║  cannot hide the result of another. Every guarded call is preceded by a    ║
// ║  flushed banner; there is NO resuming exception handler.                   ║
// ╚══════════════════════════════════════════════════════════════════════════╝

/// A live Excel automation graph: the activated `IDispatch` (used for the Quit
/// teardown and the IDispatch::Invoke oracles) plus the QI'd `_Application`
/// vtable proxy pointer (the subject under test). Drop quits Excel and releases.
struct ExcelApp {
    /// The `IDispatch` returned by `CoCreateInstance` (out-of-process proxy).
    disp: *mut RawIDispatch,
    /// The QI'd `{000208D5}` `_Application` proxy pointer (== `disp` value or a
    /// sibling tear-off, depending on the marshaler) or null if QI failed.
    app: *mut c_void,
}

impl Drop for ExcelApp {
    fn drop(&mut self) {
        // Best-effort: quit Excel so no orphan EXCEL.EXE lingers. Quit is a no-arg
        // method on the Application; we drive it via the SAFE IDispatch path.
        if !self.disp.is_null()
            && let Ok(quit_id) = dispid_of(self.disp, "Quit")
        {
            let (_hr, mut r) = invoke(self.disp, quit_id, DISPATCH_METHOD, &mut []);
            clear_variant(&mut r);
        }
        // SAFETY: `app` (if non-null) came from a successful QI that AddRef'd it; we
        // owe one Release. `disp` is the activated IDispatch we own one ref on.
        unsafe {
            if !self.app.is_null() {
                release_unknown(self.app);
            }
            if !self.disp.is_null() {
                release_dispatch(self.disp);
            }
        }
    }
}

/// Set a no-argument `BOOL`/`VARIANT_BOOL` property by name via IDispatch::Invoke
/// with the named `DISPID_PROPERTYPUT` argument (the COM put protocol). Used to
/// set `Application.Visible = False` so no Excel window flashes. AV-free.
fn invoke_put_bool(disp: *mut RawIDispatch, name: &str, value: bool) -> i32 {
    let Ok(dispid) = dispid_of(disp, name) else {
        return i32::MIN;
    };
    // VT_BOOL argument: VARIANT_TRUE = -1, VARIANT_FALSE = 0.
    // SAFETY: zero + VariantInit yields a valid VARIANT; we set the VT_BOOL arm.
    let mut arg: VARIANT = unsafe {
        let mut v: VARIANT = std::mem::zeroed();
        VariantInit(&mut v);
        v.Anonymous.Anonymous.vt = 11; // VT_BOOL
        v.Anonymous.Anonymous.Anonymous.boolVal = if value { -1 } else { 0 };
        v
    };
    let mut rgvarg = [arg];
    // DISPID_PROPERTYPUT is -3; the put value is the single named arg.
    let mut named = [-3i32];
    let mut params = DISPPARAMS {
        rgvarg: rgvarg.as_mut_ptr(),
        rgdispidNamedArgs: named.as_mut_ptr(),
        cArgs: 1,
        cNamedArgs: 1,
    };
    // SAFETY: EXCEPINFO plain-data zero is valid empty.
    let mut excep: EXCEPINFO = unsafe { std::mem::zeroed() };
    let mut arg_err = 0u32;
    const IID_NULL: GUID = GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    // SAFETY: `disp` is a live IDispatch; `params` references live `rgvarg`/`named`
    // arrays that outlive the call; the out slots are live.
    let hr = unsafe {
        ((*(*disp).vtbl).invoke)(
            disp.cast(),
            dispid,
            &IID_NULL,
            0x0400,
            DISPATCH_PROPERTYPUT,
            &mut params,
            ptr::null_mut(),
            &mut excep,
            &mut arg_err,
        )
    };
    clear_variant(&mut arg);
    hr
}

/// Activate out-of-process Excel, hide it, and QI for the `_Application` interface.
/// Returns `Ok(None)` if Excel is not registered (environment skip). The returned
/// guard quits Excel on Drop. AV-free.
fn build_excel_app() -> Result<Option<ExcelApp>, String> {
    let disp = match activate_dispatch_by_prog_id("Excel.Application") {
        Ok(p) => p,
        Err(e) if is_not_registered(&e) => {
            println!("SKIP: Excel not registered ({e})");
            return Ok(None);
        }
        Err(e) => return Err(format!("activate Excel.Application failed: {e}")),
    };
    println!(
        "  activated Excel.Application IDispatch = 0x{:x}",
        disp as usize
    );

    // Hide the window (best-effort; a non-fatal failure just means a flash).
    let put_hr = invoke_put_bool(disp, "Visible", false);
    println!(
        "  Application.Visible = False (put hr=0x{:08X})",
        put_hr as u32
    );

    // QI the IDispatch for the `_Application` dual IID.
    let (hr_qi, app) = raw_qi(disp.cast(), &IID_EXCEL_APPLICATION);
    let same = !app.is_null() && ptr::eq(app, disp.cast::<c_void>());
    println!(
        "  QI(_Application {g}) hr=0x{hr:08X} ptr=0x{p:x} same_as_idispatch={same}",
        g = fmt_guid(&IID_EXCEL_APPLICATION),
        hr = hr_qi as u32,
        p = app as usize,
        same = same,
    );
    if hr_qi < 0 || app.is_null() {
        // Still return the graph so Drop quits Excel; the rung will abort.
        return Ok(Some(ExcelApp {
            disp,
            app: ptr::null_mut(),
        }));
    }

    Ok(Some(ExcelApp { disp, app }))
}

/// Boilerplate used by every Excel rung: CoInitialize STA, build the app, and run
/// `body` with the live [`ExcelApp`]. Skips cleanly when Excel is absent.
fn with_excel_app(body: impl FnOnce(&ExcelApp)) {
    // SAFETY: STA matches the host apartment model.
    let hr_init = unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    println!("CoInitializeEx(STA) hr=0x{:08X}", hr_init as u32);

    let app = match build_excel_app() {
        Ok(Some(a)) => a,
        Ok(None) => {
            // SAFETY: balancing CoInitializeEx; no live COM objects remain.
            unsafe { CoUninitialize() };
            return;
        }
        Err(e) => {
            // SAFETY: balancing CoInitializeEx.
            unsafe { CoUninitialize() };
            panic!("Excel setup failed: {e}");
        }
    };

    body(&app);

    io::stdout().flush().ok();
    drop(app);
    // SAFETY: balancing CoInitializeEx; ExcelApp::drop released/quit the objects.
    unsafe { CoUninitialize() };
}

/// Read the `_Application` interface typeinfo `(guid, typekind, FDUAL, cFuncs,
/// cbSizeVft, granularity)` by QIing the live object's IDispatch for its typeinfo
/// and crossing to the dual partner. For Excel the IDispatch GetTypeInfo(0) gives
/// the `_Application` dispinterface; the partner crossing yields the vtable
/// INTERFACE (same IID {000208D5} for Excel's dual). AV-free.
fn excel_app_iface_attr(disp: *mut RawIDispatch) -> Option<IfaceAttr> {
    let disp_ti = type_info_of(disp);
    if disp_ti.is_null() {
        return None;
    }
    // Prefer the dual partner INTERFACE typeinfo (the real vtable face). Fall back
    // to the dispinterface face if the crossing fails.
    let (partner_opt, which) = cross_to_partner_interface(disp_ti);
    println!("  [_Application typeinfo] partner crossing via: {which}");
    let chosen = partner_opt.unwrap_or(disp_ti);
    let attr = iface_attr(chosen);
    // SAFETY: release the partner (if a distinct owned crossing) and the
    // dispinterface typeinfo. `chosen != disp_ti` ⇒ `partner_opt` was Some and
    // owned; releasing the dispinterface is always owed.
    unsafe {
        if chosen != disp_ti {
            ((*(*chosen.cast::<RawITypeInfo>()).vtbl).release)(chosen.cast());
        }
        ((*(*disp_ti.cast::<RawITypeInfo>()).vtbl).release)(disp_ti.cast());
    }
    attr
}

/// Recover a member's `(oVft, invkind, cParams, callconv)` from the `_Application`
/// dual face by name, searching partner INTERFACE → base chain → dispinterface
/// face (reusing [`member_desc_with_source`]). AV-free.
fn excel_app_member(disp: *mut RawIDispatch, name: &str) -> Option<(MemberDesc, String)> {
    let disp_ti = type_info_of(disp);
    if disp_ti.is_null() {
        return None;
    }
    let (partner_opt, _which) = cross_to_partner_interface(disp_ti);
    let partner_ti = partner_opt.unwrap_or(disp_ti);
    let found = member_desc_with_source(partner_ti, disp_ti, name);
    // SAFETY: release crossings.
    unsafe {
        if partner_opt.is_some() && partner_ti != disp_ti {
            ((*(*partner_ti.cast::<RawITypeInfo>()).vtbl).release)(partner_ti.cast());
        }
        ((*(*disp_ti.cast::<RawITypeInfo>()).vtbl).release)(disp_ti.cast());
    }
    found
}

/// Decode a PARAMFLAGS bitmask to a short human string (FIN/FOUT/FRETVAL/FLCID/…).
fn paramflags_str(flags: u16) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if flags & PARAMFLAG_FIN != 0 {
        parts.push("IN");
    }
    if flags & PARAMFLAG_FOUT != 0 {
        parts.push("OUT");
    }
    if flags & PARAMFLAG_FRETVAL != 0 {
        parts.push("RETVAL");
    }
    if flags & PARAMFLAG_FLCID != 0 {
        parts.push("LCID");
    }
    if flags & PARAMFLAG_FOPT != 0 {
        parts.push("OPT");
    }
    if parts.is_empty() {
        format!("0x{flags:04X}")
    } else {
        parts.join("|")
    }
}

/// Dump the per-parameter ELEMDESC (vt + PARAMFLAGS) of one named member from the
/// `_Application` dual face. This reveals the TRUE vtable arity the proxy's NDR
/// forwarder expects — the decisive evidence for the `RPC_X_NULL_REF_POINTER`
/// (0x800706F4) diagnosis. AV-free: pure ITypeInfo reads.
fn dump_member_params(disp: *mut RawIDispatch, name: &str) {
    let disp_ti = type_info_of(disp);
    if disp_ti.is_null() {
        println!("  [params {name}] no IDispatch typeinfo");
        return;
    }
    let (partner_opt, _which) = cross_to_partner_interface(disp_ti);
    let partner_ti = partner_opt.unwrap_or(disp_ti);
    // Find the member's FUNCDESC on whichever typeinfo declares it.
    for (ti, face) in [
        (partner_ti, "partner-INTERFACE"),
        (disp_ti, "dispinterface"),
    ] {
        if ti.is_null() {
            continue;
        }
        let vt = ti.cast::<RawITypeInfo>();
        let mut pattr: *mut TYPEATTR = ptr::null_mut();
        // SAFETY: live ITypeInfo; GetTypeAttr yields an owned TYPEATTR.
        let hr = unsafe { ((*(*vt).vtbl).get_type_attr)(vt.cast(), &mut pattr) };
        if hr < 0 || pattr.is_null() {
            continue;
        }
        // SAFETY: valid TYPEATTR.
        let cfuncs = unsafe { (*pattr).cFuncs };
        let mut found = false;
        for fi in 0..cfuncs {
            let mut pfd: *mut FUNCDESC = ptr::null_mut();
            // SAFETY: GetFuncDesc(i) for i < cFuncs yields an owned FUNCDESC.
            let hr_fd =
                unsafe { ((*(*vt).vtbl).get_func_desc)(vt.cast(), u32::from(fi), &mut pfd) };
            if hr_fd < 0 || pfd.is_null() {
                continue;
            }
            // SAFETY: valid FUNCDESC.
            let fd = unsafe { *pfd };
            if let Some(n) = get_member_name(vt, fd.memid)
                && n.eq_ignore_ascii_case(name)
            {
                found = true;
                // Return-type vt of the function itself (elemdescFunc.tdesc.vt).
                let ret_vt = fd.elemdescFunc.tdesc.vt;
                println!(
                    "  [params {name} on {face}] memid={memid} oVft={ov} invkind={ik}({ikn}) \
                     cParams={cp} cParamsOpt={cpo} callconv={cc} funcRetVt={rv}",
                    memid = fd.memid,
                    ov = fd.oVft,
                    ik = fd.invkind,
                    ikn = invkind_name(fd.invkind),
                    cp = fd.cParams,
                    cpo = fd.cParamsOpt,
                    cc = fd.callconv,
                    rv = ret_vt,
                );
                // Walk the cParams ELEMDESCs.
                if !fd.lprgelemdescParam.is_null() && fd.cParams > 0 {
                    // SAFETY: `lprgelemdescParam` points to `cParams` ELEMDESCs per the
                    // FUNCDESC contract; we read each in bounds.
                    let params: &[ELEMDESC] = unsafe {
                        std::slice::from_raw_parts(fd.lprgelemdescParam, fd.cParams as usize)
                    };
                    for (pi, ed) in params.iter().enumerate() {
                        // SAFETY: reading the paramdesc arm of a valid ELEMDESC union.
                        let flags = unsafe { ed.Anonymous.paramdesc.wParamFlags };
                        let pvt = ed.tdesc.vt;
                        println!(
                            "      param[{pi}] vt={pvt} flags={fs}",
                            pvt = pvt,
                            fs = paramflags_str(flags),
                        );
                    }
                }
            }
            // SAFETY: releasing the FUNCDESC.
            unsafe { ((*(*vt).vtbl).release_func_desc)(vt.cast(), pfd) };
            if found {
                break;
            }
        }
        // SAFETY: releasing the TYPEATTR.
        unsafe { ((*(*vt).vtbl).release_type_attr)(vt.cast(), pattr) };
        if found {
            break;
        }
    }
    // SAFETY: release crossings.
    unsafe {
        if partner_opt.is_some() && partner_ti != disp_ti {
            ((*(*partner_ti.cast::<RawITypeInfo>()).vtbl).release)(partner_ti.cast());
        }
        ((*(*disp_ti.cast::<RawITypeInfo>()).vtbl).release)(disp_ti.cast());
    }
}

// ── Rung 0/2 — object + AV-FREE vtable walk ──────────────────────────────────

#[test]
#[ignore = "live out-of-process Excel; AV-free inspection of the _Application proxy vtable"]
fn probe_excel_app_inspect() {
    with_excel_app(|app| {
        println!("================= INSPECT: Excel _Application proxy =================");
        if app.app.is_null() {
            println!("  _Application QI failed earlier; nothing to walk");
            return;
        }

        // Is the bound IDispatch a marshaling proxy? (out-of-process ⇒ yes.)
        let (hr_proxy, proxy_ptr) = raw_qi(app.disp.cast(), &IID_IPROXYMANAGER);
        let is_proxy = hr_proxy >= 0 && !proxy_ptr.is_null();
        println!(
            "  [proxy?] QI(IProxyManager) hr=0x{:08X} is_proxy={is_proxy} (expect true OOP)",
            hr_proxy as u32
        );
        if !proxy_ptr.is_null() {
            // SAFETY: owned interface pointer from a successful QI.
            unsafe { release_unknown(proxy_ptr) };
        }

        // The _Application interface typeinfo: cbSizeVft bounds the walk.
        let attr = excel_app_iface_attr(app.disp);
        let (cb, gran) = match &attr {
            Some(a) => {
                println!(
                    "  [_Application iface] guid={g} typekind={tk}({tkn}) FDUAL={fd} cFuncs={cf} \
                     cbSizeVft={cb} syskind={sk} granularity={gr} → slots={n}",
                    g = fmt_guid(&a.guid),
                    tk = a.typekind,
                    tkn = typekind_name(a.typekind),
                    fd = a.fdual,
                    cf = a.c_funcs,
                    cb = a.cb_size_vft,
                    sk = a.syskind,
                    gr = a.granularity,
                    n = if a.granularity == 0 {
                        0
                    } else {
                        a.cb_size_vft / a.granularity
                    },
                );
                (a.cb_size_vft, a.granularity)
            }
            None => {
                println!("  [_Application iface] typeinfo unavailable; cannot bound the walk");
                (0, 8)
            }
        };

        // Annotate the Build / Version slots so the walk marks them.
        let mut annotate: Vec<(i64, String)> = Vec::new();
        for m in ["Build", "Version"] {
            if let Some((md, src)) = excel_app_member(app.disp, m) {
                let s8 = i64::from(md.o_vft) / 8;
                let s4 = i64::from(md.o_vft) / 4;
                println!(
                    "  member `{m}` [from {src}] oVft={ov} invkind={ik}({ikn}) cParams={cp} \
                     slot@oVft/8={s8} slot@oVft/4={s4}",
                    ov = md.o_vft,
                    ik = md.invkind,
                    ikn = invkind_name(md.invkind),
                    cp = md.c_params,
                );
                annotate.push((s8, format!("{m}@oVft/8")));
                annotate.push((s4, format!("{m}@oVft/4")));
            }
        }

        // Decisive: the TRUE vtable arity each member's NDR forwarder expects.
        println!("  --- member parameter shapes (true vtable arity) ---");
        dump_member_params(app.disp, "Build");
        dump_member_params(app.disp, "Version");

        if cb == 0 {
            println!("  no cbSizeVft → skipping the bounded walk");
        } else {
            println!("  raw vtable walk of the QI'd _Application proxy (slots 0..N, bounded):");
            walk_iface_vtable("_Application", app.app, cb, gran, &annotate);
        }
        println!("================= END INSPECT: Excel _Application proxy =================\n");
    });
}

// ── Rung A — IUnknown::AddRef (slot 1), a LOCAL proxy op (no RPC) ─────────────

#[test]
#[ignore = "live out-of-process Excel; vtable-calls IUnknown::AddRef on the _Application proxy"]
fn probe_excel_app_rung_a_addref() {
    with_excel_app(|app| {
        println!("--------- RUNG A: IUnknown::AddRef (slot 1) on the _Application proxy ---------");
        if app.app.is_null() {
            println!("  _Application QI failed; aborting rung A");
            return;
        }
        println!(
            "  >>> ABOUT TO VTABLE-CALL AddRef (slot 1) on _Application proxy 0x{:x} <<<",
            app.app as usize
        );
        io::stdout().flush().ok();

        // SAFETY: AddRef at IUnknown slot 1 is present on EVERY COM interface,
        // including any proxy; its ABI is `fn(this) -> u32` and it never RPCs (the
        // proxy bumps a LOCAL refcount). Reading the vtable base + slot 1 and calling
        // it is the same deref the production path uses; an AV here would mean the
        // raw-vtable-call MECHANISM itself fails on an oleaut proxy.
        let rc = unsafe {
            let this = app.app;
            let vtbl = *this.cast::<*const *const c_void>();
            let fnptr = *vtbl.add(1);
            let f: extern "system" fn(*mut c_void) -> u32 = std::mem::transmute(fnptr);
            f(this)
        };
        println!("  AddRef returned refcount={rc} (no AV → raw vtable-call mechanism WORKS)");
        io::stdout().flush().ok();

        // Rebalance: call Release (slot 2, same ABI) exactly once.
        // SAFETY: Release at IUnknown slot 2; balances the AddRef above.
        let rc2 = unsafe {
            let this = app.app;
            let vtbl = *this.cast::<*const *const c_void>();
            let fnptr = *vtbl.add(2);
            let f: extern "system" fn(*mut c_void) -> u32 = std::mem::transmute(fnptr);
            f(this)
        };
        println!("  Release returned refcount={rc2} (rebalanced)");
        io::stdout().flush().ok();
    });
}

// ── Rung B — IDispatch::GetTypeInfoCount (slot 3) ────────────────────────────

#[test]
#[ignore = "live out-of-process Excel; vtable-calls IDispatch::GetTypeInfoCount on the proxy"]
fn probe_excel_app_rung_b_gettypeinfocount() {
    with_excel_app(|app| {
        println!(
            "--------- RUNG B: IDispatch::GetTypeInfoCount (slot 3) on the _Application proxy ---------"
        );
        if app.app.is_null() {
            println!("  _Application QI failed; aborting rung B");
            return;
        }
        println!(
            "  >>> ABOUT TO VTABLE-CALL GetTypeInfoCount (slot 3) on _Application proxy 0x{:x} <<<",
            app.app as usize
        );
        io::stdout().flush().ok();

        let mut count: u32 = 0xFFFF_FFFF;
        // SAFETY: GetTypeInfoCount is IDispatch slot 3 (after QI/AddRef/Release); its
        // ABI is `fn(this, *mut u32) -> HRESULT`. `_Application` is a dual interface,
        // so its vtable layers IDispatch over IUnknown and slot 3 is valid. `&mut
        // count` is a live out-slot. A bogus result here would mean an IDispatch-level
        // vtable slot is mis-dispatched on the proxy.
        let hr = unsafe {
            let this = app.app;
            let vtbl = *this.cast::<*const *const c_void>();
            let fnptr = *vtbl.add(3);
            let f: extern "system" fn(*mut c_void, *mut u32) -> i32 = std::mem::transmute(fnptr);
            f(this, &mut count)
        };
        println!(
            "  GetTypeInfoCount hr=0x{:08X} count={count} (expect hr=0 count=1, no AV)",
            hr as u32
        );
        io::stdout().flush().ok();
    });
}

// ── Rung C — the custom slot `Build` (no-arg Long retval) ────────────────────

/// Shared body for the `Build` custom-slot rung. `divisor` chooses oVft/8 vs
/// oVft/4. Always runs the SAFE IDispatch::Invoke(Build) oracle FIRST (expect a
/// Long like 20026), then ONE guarded vtable call `fn(this, *mut i32) -> HRESULT`.
fn excel_build_rung(divisor: i32) {
    with_excel_app(|app| {
        println!("--------- RUNG C: _Application.Build custom slot (oVft/{divisor}) ---------");
        if app.app.is_null() {
            println!("  _Application QI failed; aborting rung C");
            return;
        }

        // (A) SAFE oracle via IDispatch::Invoke(Build) FIRST.
        let oracle_i4 = match dispid_of(app.disp, "Build") {
            Ok(id) => {
                let (hr, mut r) = invoke(
                    app.disp,
                    id,
                    DISPATCH_PROPERTYGET | DISPATCH_METHOD,
                    &mut [],
                );
                let decoded = decode_variant(&r);
                // SAFETY: read the VT_I4 arm if applicable (Build is a Long).
                let i4 = unsafe {
                    match r.Anonymous.Anonymous.vt {
                        3 => Some(r.Anonymous.Anonymous.Anonymous.lVal),
                        2 => Some(i32::from(r.Anonymous.Anonymous.Anonymous.iVal)),
                        _ => None,
                    }
                };
                println!(
                    "  ORACLE IDispatch::Invoke(Build) hr=0x{:08X} retval={decoded} i4={i4:?}",
                    hr as u32
                );
                clear_variant(&mut r);
                i4
            }
            Err(hr) => {
                println!("  GetIDsOfNames(Build) hr=0x{:08X}; no oracle", hr as u32);
                None
            }
        };
        io::stdout().flush().ok();

        // (B) Recover Build's oVft from the _Application typeinfo and compute slots.
        let Some((md, src)) = excel_app_member(app.disp, "Build") else {
            println!("  Build member not found on the _Application dual face; aborting");
            return;
        };
        let attr = excel_app_iface_attr(app.disp);
        let vtbl_len = attr
            .as_ref()
            .map(|a| {
                if a.granularity == 0 {
                    0
                } else {
                    a.cb_size_vft / a.granularity
                }
            })
            .unwrap_or(0);
        let slot8 = i64::from(md.o_vft) / 8;
        let slot4 = i64::from(md.o_vft) / 4;
        let slot = (i64::from(md.o_vft) / i64::from(divisor)) as usize;
        println!(
            "  Build [from {src}] oVft={ov} invkind={ik}({ikn}) cParams={cp} \
             slot@oVft/8={s8} slot@oVft/4={s4} chosen(divisor={divisor})={slot} \
             proxy-vtable-len(cbSizeVft/{gr})={vl} in_range={inr}",
            ov = md.o_vft,
            ik = md.invkind,
            ikn = invkind_name(md.invkind),
            cp = md.c_params,
            s8 = slot8,
            s4 = slot4,
            gr = attr.as_ref().map(|a| a.granularity).unwrap_or(0),
            vl = vtbl_len,
            inr = (slot as u16) < vtbl_len.max(1),
        );
        println!(
            "  >>> ABOUT TO VTABLE-CALL Build at slot={slot} (oVft/{divisor}) on _Application proxy (AV risk) <<<"
        );
        io::stdout().flush().ok();

        // (C) ONE guarded vtable call. Build is a no-arg Long property/method; the
        // dual ABI is `HRESULT get_Build(this, long* pRet)`.
        let mut build_out: i32 = -1;
        // SAFETY: the deliberate isolated risk. `app.app` is the live QI'd
        // _Application proxy; we read its vtable base + the chosen slot exactly as
        // production does, then call `fn(this, *mut i32) -> HRESULT` with a live
        // out-slot. A wrong slot/granularity faults and kills the process by design.
        let hr = unsafe {
            let this = app.app;
            let vtbl = *this.cast::<*const *const c_void>();
            let fnptr = *vtbl.add(slot);
            let f: extern "system" fn(*mut c_void, *mut i32) -> i32 = std::mem::transmute(fnptr);
            f(this, &mut build_out)
        };
        println!(
            "  VTABLE CALL Build slot={slot} hr=0x{:08X} build={build_out}",
            hr as u32
        );

        // (D) Oracle: value equality vs the IDispatch::Invoke result.
        match (oracle_i4, hr >= 0) {
            (Some(a), true) if a == build_out => {
                println!("  ORACLE: MATCH (vtable Build == invoke Build == {a}) — slot CORRECT")
            }
            (Some(a), _) => println!(
                "  ORACLE: MISMATCH (invoke={a} vtable hr=0x{:08X} build={build_out})",
                hr as u32
            ),
            (None, _) => println!(
                "  ORACLE: no invoke baseline; vtable hr=0x{:08X} build={build_out}",
                hr as u32
            ),
        }
        io::stdout().flush().ok();
    });
}

#[test]
#[ignore = "live out-of-process Excel; vtable-calls _Application.Build at oVft/8 (may AV)"]
fn probe_excel_app_rung_c_build_ovft_div8() {
    excel_build_rung(8);
}

// ── Rung C-FIXED — Build with the FULL `[propget, lcid]` vtable signature ─────
//
// The inspect rung proved Build is declared `[propget, lcid]`: its vtable ABI is
// `HRESULT get_Build(this, LCID lcid, long* pRet)` — THREE slots, not two. The
// earlier two-arg call `(this, long* pRet)` mis-placed the out-pointer into the
// LCID register and left the retval register garbage → RPC_X_NULL_REF_POINTER
// (0x800706F4). This rung calls the CORRECT 3-arg signature at slot oVft/8 and
// expects the same value the IDispatch oracle returns (Build 20026).

#[test]
#[ignore = "live out-of-process Excel; vtable-calls _Application.Build with the LCID-aware signature"]
fn probe_excel_app_rung_c_build_fixed_lcid() {
    with_excel_app(|app| {
        println!(
            "--------- RUNG C-FIXED: _Application.Build with full [propget,lcid] ABI (oVft/8) ---------"
        );
        if app.app.is_null() {
            println!("  _Application QI failed; aborting");
            return;
        }

        // (A) SAFE oracle via IDispatch::Invoke(Build) FIRST.
        let oracle = match dispid_of(app.disp, "Build") {
            Ok(id) => {
                let (hr, mut r) = invoke(
                    app.disp,
                    id,
                    DISPATCH_PROPERTYGET | DISPATCH_METHOD,
                    &mut [],
                );
                let decoded = decode_variant(&r);
                println!(
                    "  ORACLE IDispatch::Invoke(Build) hr=0x{:08X} retval={decoded}",
                    hr as u32
                );
                clear_variant(&mut r);
                Some(decoded)
            }
            Err(hr) => {
                println!("  GetIDsOfNames(Build) hr=0x{:08X}; no oracle", hr as u32);
                None
            }
        };
        io::stdout().flush().ok();

        let Some((md, src)) = excel_app_member(app.disp, "Build") else {
            println!("  Build member not found; aborting");
            return;
        };
        let slot = (i64::from(md.o_vft) / 8) as usize;
        // LCID 0 = LOCALE_NEUTRAL; the proxy accepts it for an lcid propget.
        let lcid: u32 = 0;
        println!(
            "  Build [from {src}] oVft={ov} slot@oVft/8={slot} → calling get_Build(this, lcid={lcid}, long* pRet)",
            ov = md.o_vft,
        );
        println!(
            "  >>> ABOUT TO VTABLE-CALL Build (3-arg LCID ABI) at slot={slot} on _Application proxy <<<"
        );
        io::stdout().flush().ok();

        let mut build_out: i32 = -1;
        // SAFETY: the deliberate isolated call with the CORRECT vtable arity proven
        // by the inspect rung: `fn(this, LCID, *mut i32) -> HRESULT`. `app.app` is the
        // live QI'd proxy; slot is oVft/8 (the real combase forwarder, in range);
        // `&mut build_out` is the live `[out,retval]` slot.
        let hr = unsafe {
            let this = app.app;
            let vtbl = *this.cast::<*const *const c_void>();
            let fnptr = *vtbl.add(slot);
            let f: extern "system" fn(*mut c_void, u32, *mut i32) -> i32 =
                std::mem::transmute(fnptr);
            f(this, lcid, &mut build_out)
        };
        println!(
            "  VTABLE CALL Build(lcid) slot={slot} hr=0x{:08X} build={build_out}",
            hr as u32
        );
        match &oracle {
            Some(o) if o.contains(&build_out.to_string()) && hr >= 0 => println!(
                "  ORACLE: MATCH (vtable Build={build_out} matches invoke {o}) — \
                 LCID-AWARE VTABLE DISPATCH WORKS"
            ),
            other => println!(
                "  ORACLE: vtable hr=0x{:08X} build={build_out} vs invoke {other:?}",
                hr as u32
            ),
        }
        io::stdout().flush().ok();
    });
}

// ── Rung D-FIXED — Version with the FULL `[propget, lcid]` BSTR signature ─────
//
// Same lcid proof for the BSTR propget: `HRESULT get_Version(this, LCID lcid,
// BSTR* pRet)`. Calls the correct 3-arg ABI at slot oVft/8 and expects "16.0".

#[test]
#[ignore = "live out-of-process Excel; vtable-calls _Application.Version with the LCID-aware signature"]
fn probe_excel_app_rung_d_version_fixed_lcid() {
    with_excel_app(|app| {
        println!(
            "--------- RUNG D-FIXED: _Application.Version with full [propget,lcid] BSTR ABI (oVft/8) ---------"
        );
        if app.app.is_null() {
            println!("  _Application QI failed; aborting");
            return;
        }

        // (A) SAFE oracle FIRST (expect "16.0").
        let oracle = match dispid_of(app.disp, "Version") {
            Ok(id) => {
                let (hr, mut r) = invoke(app.disp, id, DISPATCH_PROPERTYGET, &mut []);
                let s = bstr_of_variant(&r);
                println!(
                    "  ORACLE IDispatch::Invoke(Version) hr=0x{:08X} retval={s:?}",
                    hr as u32
                );
                clear_variant(&mut r);
                s
            }
            Err(hr) => {
                println!("  GetIDsOfNames(Version) hr=0x{:08X}; no oracle", hr as u32);
                None
            }
        };
        io::stdout().flush().ok();

        let Some((md, src)) = excel_app_member(app.disp, "Version") else {
            println!("  Version member not found; aborting");
            return;
        };
        let slot = (i64::from(md.o_vft) / 8) as usize;
        let lcid: u32 = 0;
        println!(
            "  Version [from {src}] oVft={ov} slot@oVft/8={slot} → get_Version(this, lcid={lcid}, BSTR* pRet)",
            ov = md.o_vft,
        );
        println!(
            "  >>> ABOUT TO VTABLE-CALL Version (3-arg LCID ABI) at slot={slot} on _Application proxy <<<"
        );
        io::stdout().flush().ok();

        let mut bstr_out: *mut u16 = ptr::null_mut();
        // SAFETY: correct vtable arity `fn(this, LCID, *mut BSTR) -> HRESULT` proven
        // by the inspect rung. `app.app` is the live proxy; slot is oVft/8 (in range);
        // `&mut bstr_out` is the live `[out,retval]` BSTR slot the marshaler fills.
        let hr = unsafe {
            let this = app.app;
            let vtbl = *this.cast::<*const *const c_void>();
            let fnptr = *vtbl.add(slot);
            let f: extern "system" fn(*mut c_void, u32, *mut *mut u16) -> i32 =
                std::mem::transmute(fnptr);
            f(this, lcid, &mut bstr_out)
        };
        let decoded = if bstr_out.is_null() {
            None
        } else {
            // SAFETY: NUL-terminated UTF-16 BSTR the marshaler allocated; read to NUL
            // then free exactly once.
            unsafe {
                let mut len = 0usize;
                while *bstr_out.add(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(bstr_out, len);
                let s = String::from_utf16_lossy(slice);
                SysFreeString(bstr_out);
                Some(s)
            }
        };
        println!(
            "  VTABLE CALL Version(lcid) slot={slot} hr=0x{:08X} retval={decoded:?}",
            hr as u32
        );
        match (&oracle, &decoded) {
            (Some(a), Some(b)) if a == b => println!(
                "  ORACLE: MATCH (vtable Version={b:?} matches invoke {a:?}) — \
                 LCID-AWARE BSTR VTABLE DISPATCH WORKS"
            ),
            (a, b) => println!("  ORACLE: invoke={a:?} vtable={b:?} hr=0x{:08X}", hr as u32),
        }
        io::stdout().flush().ok();
    });
}

#[test]
#[ignore = "live out-of-process Excel; vtable-calls _Application.Build at oVft/4 (may AV)"]
fn probe_excel_app_rung_c_build_ovft_div4() {
    excel_build_rung(4);
}

// ── Rung D — the custom propget `Version` (BSTR* out) at oVft/8 ───────────────

#[test]
#[ignore = "live out-of-process Excel; vtable-calls _Application.Version (BSTR out) at oVft/8 (may AV)"]
fn probe_excel_app_rung_d_version() {
    with_excel_app(|app| {
        println!(
            "--------- RUNG D: _Application.Version custom propget (BSTR out, oVft/8) ---------"
        );
        if app.app.is_null() {
            println!("  _Application QI failed; aborting rung D");
            return;
        }

        // (A) SAFE oracle via IDispatch::Invoke(Version) FIRST (expect "16.0").
        let oracle = match dispid_of(app.disp, "Version") {
            Ok(id) => {
                let (hr, mut r) = invoke(app.disp, id, DISPATCH_PROPERTYGET, &mut []);
                let s = bstr_of_variant(&r);
                println!(
                    "  ORACLE IDispatch::Invoke(Version) hr=0x{:08X} retval={s:?}",
                    hr as u32
                );
                clear_variant(&mut r);
                s
            }
            Err(hr) => {
                println!("  GetIDsOfNames(Version) hr=0x{:08X}; no oracle", hr as u32);
                None
            }
        };
        io::stdout().flush().ok();

        // (B) Recover Version's oVft; slot = oVft/8.
        let Some((md, src)) = excel_app_member(app.disp, "Version") else {
            println!("  Version member not found on the _Application dual face; aborting");
            return;
        };
        let slot = (i64::from(md.o_vft) / 8) as usize;
        println!(
            "  Version [from {src}] oVft={ov} invkind={ik}({ikn}) cParams={cp} → slot@oVft/8={slot}",
            ov = md.o_vft,
            ik = md.invkind,
            ikn = invkind_name(md.invkind),
            cp = md.c_params,
        );
        println!(
            "  >>> ABOUT TO VTABLE-CALL Version at slot={slot} (BSTR* out) on _Application proxy (AV risk) <<<"
        );
        io::stdout().flush().ok();

        // (C) ONE guarded vtable call. Version is a propget returning a BSTR; the
        // dual ABI is `HRESULT get_Version(this, BSTR* pRet)` where BSTR == *mut u16.
        let mut bstr_out: *mut u16 = ptr::null_mut();
        // SAFETY: the deliberate isolated risk. `app.app` is the live QI'd
        // _Application proxy; we read its vtable base + slot (oVft/8) exactly as the
        // production marshaller does for a BSTR-returning propget, then call
        // `fn(this, *mut BSTR) -> HRESULT` with a live out-slot. A wrong slot faults
        // and kills the process by design. On success the marshaler allocated the
        // BSTR; we free it below.
        let hr = unsafe {
            let this = app.app;
            let vtbl = *this.cast::<*const *const c_void>();
            let fnptr = *vtbl.add(slot);
            let f: extern "system" fn(*mut c_void, *mut *mut u16) -> i32 =
                std::mem::transmute(fnptr);
            f(this, &mut bstr_out)
        };
        // Decode + free the returned BSTR.
        let decoded = if bstr_out.is_null() {
            None
        } else {
            // SAFETY: `bstr_out` is a NUL-terminated UTF-16 BSTR the marshaler
            // allocated; read to NUL then free exactly once with SysFreeString.
            unsafe {
                let mut len = 0usize;
                while *bstr_out.add(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(bstr_out, len);
                let s = String::from_utf16_lossy(slice);
                SysFreeString(bstr_out);
                Some(s)
            }
        };
        println!(
            "  VTABLE CALL Version slot={slot} hr=0x{:08X} retval={decoded:?}",
            hr as u32
        );

        // (D) Oracle: value equality vs the IDispatch::Invoke BSTR.
        match (&oracle, &decoded) {
            (Some(a), Some(b)) if a == b => {
                println!(
                    "  ORACLE: MATCH (vtable Version == invoke Version == {a:?}) — slot CORRECT"
                )
            }
            (a, b) => println!(
                "  ORACLE: MISMATCH/incomplete (invoke={a:?} vtable={b:?} hr=0x{:08X})",
                hr as u32
            ),
        }
        io::stdout().flush().ok();
    });
}

/// Decode a `VT_BSTR` VARIANT to a Rust `String` (does NOT free; caller clears the
/// VARIANT). Returns None if not a BSTR or null.
fn bstr_of_variant(v: &VARIANT) -> Option<String> {
    // SAFETY: read the discriminant then the bstrVal arm of a valid VARIANT.
    unsafe {
        if v.Anonymous.Anonymous.vt != VT_BSTR {
            return None;
        }
        let p = v.Anonymous.Anonymous.Anonymous.bstrVal;
        if p.is_null() {
            return None;
        }
        let mut len = 0usize;
        while *p.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(p, len);
        Some(String::from_utf16_lossy(slice))
    }
}
