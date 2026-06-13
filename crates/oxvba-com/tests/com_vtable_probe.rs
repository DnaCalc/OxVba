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
    DISPATCH_PROPERTYGET, DISPPARAMS, EXCEPINFO, FUNCDESC, SYS_WIN32, SYS_WIN64, TKIND_DISPATCH,
    TKIND_INTERFACE, TLIBATTR, TYPEATTR,
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
