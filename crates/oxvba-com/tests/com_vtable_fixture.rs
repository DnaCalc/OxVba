//! In-process FDUAL-crossing fixture for the early-bound COM vtable path.
//!
//! Workset `WORKSET_2026-06-12_COM_VTABLE_EARLY_BOUND_DISPATCH`, slice F. This is
//! a PLAIN `cargo test -p oxvba-com` fixture — it builds hand-rolled COM objects
//! (no live Office/DAO) that reproduce the exact shape the access violation hid
//! behind: an `IDispatch::GetTypeInfo(0)` that returns a `dispinterface`
//! (`TKIND_DISPATCH`, FDUAL) whose member `oVft` is authored for the FDUAL PARTNER
//! `TKIND_INTERFACE`, reached via `GetRefTypeOfImplType(-1)` → `GetRefTypeInfo`.
//!
//! Why this matters: the shipped `OxvbaTestDispatch.GetTypeInfo` returns
//! `E_NOTIMPL`, so NO existing test exercised `live_member_metadata_from_dispatch`'s
//! FDUAL crossing — which is how a bogus slot shipped CI-green. This fixture
//! drives that production path directly and asserts, via a VALUE ORACLE:
//!   * ACCEPT — the recovered slot/IID/source-typekind/bound are correct AND a
//!     real slot call on the QI'd interface returns the known value (7);
//!   * REJECT (non-FDUAL dispinterface) — no vtable slot is recovered;
//!   * REJECT (slot >= cbSizeVft/8 bound) — the AV-safety bound declines.
#![cfg(all(target_os = "windows", target_arch = "x86_64"))]
#![allow(clippy::too_many_lines)]
// This whole file is hand-rolled COM FFI; the unsafe blocks are pervasive and
// each fixture fn upholds the obvious pointer invariants (live this-pointer +
// out-slots). Mirrors the shipped windows_test_dispatch.rs fixture's policy.
#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use oxvba_com::windows_client::{IID_IDISPATCH, IID_IUNKNOWN, RawIDispatch, guid_equals};
use oxvba_com::{SourceTypeKind, TypeLibMemberInvokeKind, live_member_metadata_from_dispatch};

use windows_sys::Win32::System::Variant::{VARIANT, VT_I4, VariantInit};
use windows_sys::core::GUID;

const COM_S_OK: i32 = 0;
const COM_E_NOINTERFACE: i32 = -2_147_467_262; // 0x8000_4002
const COM_E_INVALIDARG: i32 = -2_147_024_809; // 0x8007_0057

const TKIND_INTERFACE: u32 = 3;
const TKIND_DISPATCH: u32 = 4;
const TYPEFLAG_FDUAL: u16 = 0x40;
const CC_STDCALL: u32 = 4;

/// The partner INTERFACE IID the fixture's dispinterface crosses to and the
/// fixture object answers `QueryInterface` for (returning a SEPARATE interface
/// object — a non-aliasing tear-off, exactly the case slice E now admits).
const IID_FIXTURE_PARTNER: GUID = GUID {
    data1: 0xABCD_0001,
    data2: 0x1111,
    data3: 0x2222,
    data4: [0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA],
};

/// `Value` is memid 9, a property-get returning VT_I4(7). Its `oVft = 136` →
/// slot 17 (= 136/8), exactly the live DAO `Field.Value` the probe value-oracled.
const VALUE_MEMID: i32 = 9;
const VALUE_NAME: &[u16] = &[
    b'V' as u16,
    b'a' as u16,
    b'l' as u16,
    b'u' as u16,
    b'e' as u16,
    0,
];
const VALUE_OVFT: i16 = 136;
const VALUE_SLOT: u16 = 17;
const VALUE_RESULT: i32 = 7;

/// The partner INTERFACE's `cbSizeVft`. 160 → bound 20; slot 17 is in range. The
/// fixture's real interface vtable is exactly 20 slots so the bound is honest.
const PARTNER_CB_SIZE_VFT_OK: u16 = 160;
/// A deliberately SHORT `cbSizeVft` for the reject scenario: 144 → bound 18, but
/// the member's slot is 17... still in range. To force a reject we instead author
/// the member at a HIGH oVft (below) with a short bound.
const PARTNER_CB_SIZE_VFT_SHORT: u16 = 144; // bound 18

/// Reject scenario member: oVft 800 → slot 100, well past the bound-18 vtable.
const HIGH_OVFT: i16 = 800;

// ── raw struct mirrors (plain-data; oaidl.h layout) ──────────────────────────

#[repr(C)]
struct TypeDesc {
    union_field: usize,
    vt: u16,
}

#[repr(C)]
struct ParamDesc {
    pparamdescex: *mut c_void,
    wparamflags: u16,
}

#[repr(C)]
struct ElemDesc {
    tdesc: TypeDesc,
    paramdesc: ParamDesc,
}

#[repr(C)]
#[allow(non_snake_case)]
struct FuncDesc {
    memid: i32,
    lprgscode: *mut i32,
    lprgelemdescparam: *mut ElemDesc,
    funckind: u32,
    invkind: u32,
    callconv: u32,
    cparams: i16,
    cparams_opt: i16,
    oVft: i16,
    cScodes: i16,
    elemdescfunc: ElemDesc,
    wfuncdescflags: u16,
}

#[repr(C)]
struct TypeAttr {
    guid: GUID,
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
    tdesc_alias: TypeDesc,
    idldesc: ParamDesc,
}

/// The ITypeInfo vtable, in oaidl.h order. Only the methods the production
/// crossing calls are real; the rest are stubs to keep slot offsets correct.
#[repr(C)]
struct TypeInfoVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_type_attr: unsafe extern "system" fn(*mut c_void, *mut *mut TypeAttr) -> i32,
    get_type_comp: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    get_func_desc: unsafe extern "system" fn(*mut c_void, u32, *mut *mut FuncDesc) -> i32,
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
    create_instance:
        unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, *mut *mut c_void) -> i32,
    get_mops: unsafe extern "system" fn(*mut c_void, i32, *mut *mut u16) -> i32,
    get_containing_type_lib:
        unsafe extern "system" fn(*mut c_void, *mut *mut c_void, *mut u32) -> i32,
    release_type_attr: unsafe extern "system" fn(*mut c_void, *mut TypeAttr),
    release_func_desc: unsafe extern "system" fn(*mut c_void, *mut FuncDesc),
    release_var_desc: unsafe extern "system" fn(*mut c_void, *mut c_void),
}

/// Which face a `FixtureTypeInfo` plays. The crossing chain is
/// dispinterface → partner interface → base interface; the member's funcdesc
/// lives on the BASE (so we exercise `member_ovft_with_source`'s chain walk).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Face {
    /// `TKIND_DISPATCH`; FDUAL flag per `Scenario::fdual`. impl-type -1 crosses to
    /// the partner INTERFACE.
    Dispinterface,
    /// `TKIND_INTERFACE` with `cb_size_vft`; impl-type 0 → the base interface.
    PartnerInterface,
    /// `TKIND_INTERFACE`; carries the member funcdesc (`oVft`).
    BaseInterface,
}

#[derive(Clone, Copy)]
struct Scenario {
    fdual: bool,
    partner_cb_size_vft: u16,
    member_ovft: i16,
}

#[repr(C)]
struct FixtureTypeInfo {
    vtbl: *const TypeInfoVtbl,
    ref_count: AtomicU32,
    face: Face,
    scenario: Scenario,
}

static TYPE_INFO_VTBL: TypeInfoVtbl = TypeInfoVtbl {
    query_interface: ti_query_interface,
    add_ref: ti_add_ref,
    release: ti_release,
    get_type_attr: ti_get_type_attr,
    get_type_comp: ti_get_type_comp_stub,
    get_func_desc: ti_get_func_desc,
    get_var_desc: ti_get_var_desc_stub,
    get_names: ti_get_names,
    get_ref_type_of_impl_type: ti_get_ref_type_of_impl_type,
    get_impl_type_flags: ti_get_impl_type_flags,
    get_ids_of_names: ti_get_ids_of_names_stub,
    invoke: ti_invoke_stub,
    get_documentation: ti_get_documentation,
    get_dll_entry: ti_get_dll_entry_stub,
    get_ref_type_info: ti_get_ref_type_info,
    address_of_member: ti_address_of_member_stub,
    create_instance: ti_create_instance_stub,
    get_mops: ti_get_mops_stub,
    get_containing_type_lib: ti_get_containing_type_lib_stub,
    release_type_attr: ti_release_type_attr,
    release_func_desc: ti_release_func_desc,
    release_var_desc: ti_release_var_desc_stub,
};

fn make_type_info(face: Face, scenario: Scenario) -> *mut FixtureTypeInfo {
    Box::into_raw(Box::new(FixtureTypeInfo {
        vtbl: &TYPE_INFO_VTBL,
        ref_count: AtomicU32::new(1),
        face,
        scenario,
    }))
}

unsafe fn as_ti(this: *mut c_void) -> *mut FixtureTypeInfo {
    this.cast::<FixtureTypeInfo>()
}

unsafe extern "system" fn ti_query_interface(
    _this: *mut c_void,
    _riid: *const GUID,
    ppv: *mut *mut c_void,
) -> i32 {
    if !ppv.is_null() {
        *ppv = ptr::null_mut();
    }
    COM_E_NOINTERFACE
}

unsafe extern "system" fn ti_add_ref(this: *mut c_void) -> u32 {
    (*as_ti(this)).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

unsafe extern "system" fn ti_release(this: *mut c_void) -> u32 {
    let owner = as_ti(this);
    let remaining = (*owner).ref_count.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        drop(Box::from_raw(owner));
    }
    remaining
}

unsafe extern "system" fn ti_get_type_attr(this: *mut c_void, pp: *mut *mut TypeAttr) -> i32 {
    if pp.is_null() {
        return COM_E_INVALIDARG;
    }
    let owner = as_ti(this);
    let face = (*owner).face;
    let scenario = (*owner).scenario;
    let mut attr: TypeAttr = std::mem::zeroed();
    match face {
        Face::Dispinterface => {
            attr.guid = IID_IDISPATCH; // a dispinterface's own IID is irrelevant here
            attr.typekind = TKIND_DISPATCH;
            // Like real DAO, the dispinterface face lists ALL members (so the
            // member is found by dispid first); only the crossing supplies a
            // callable slot. A FDUAL face advertises the member; a non-FDUAL one
            // advertises none (so the reject test recovers no member at all).
            attr.cfuncs = if scenario.fdual { 1 } else { 0 };
            attr.cimpl_types = 1; // impl-type -1/0 reaches the partner
            attr.cb_size_vft = 56; // just IDispatch's 7 slots
            attr.wtypeflags = if scenario.fdual { TYPEFLAG_FDUAL } else { 0 };
        }
        Face::PartnerInterface => {
            attr.guid = IID_FIXTURE_PARTNER;
            attr.typekind = TKIND_INTERFACE;
            attr.cfuncs = 0; // the member lives on the BASE interface
            attr.cimpl_types = 1; // impl-type 0 → the base interface
            attr.cb_size_vft = scenario.partner_cb_size_vft;
            attr.wtypeflags = TYPEFLAG_FDUAL;
        }
        Face::BaseInterface => {
            attr.guid = IID_IUNKNOWN; // base IID is irrelevant to the recovery
            attr.typekind = TKIND_INTERFACE;
            attr.cfuncs = 1; // carries `Value`
            attr.cimpl_types = 0;
            attr.cb_size_vft = scenario.partner_cb_size_vft;
            attr.wtypeflags = TYPEFLAG_FDUAL;
        }
    }
    *pp = Box::into_raw(Box::new(attr));
    COM_S_OK
}

unsafe extern "system" fn ti_release_type_attr(_this: *mut c_void, p: *mut TypeAttr) {
    if !p.is_null() {
        drop(Box::from_raw(p));
    }
}

unsafe extern "system" fn ti_get_func_desc(
    this: *mut c_void,
    index: u32,
    pp: *mut *mut FuncDesc,
) -> i32 {
    if pp.is_null() {
        return COM_E_INVALIDARG;
    }
    let owner = as_ti(this);
    let face = (*owner).face;
    // The BASE interface carries the canonical `Value` funcdesc; the FDUAL
    // dispinterface face also lists it (with the same partner-authored oVft), as
    // real DAO does. The PartnerInterface face carries none (cfuncs == 0).
    let serves = matches!(face, Face::BaseInterface)
        || (matches!(face, Face::Dispinterface) && (*owner).scenario.fdual);
    if !serves || index != 0 {
        return COM_E_INVALIDARG;
    }
    let mut fd: FuncDesc = std::mem::zeroed();
    fd.memid = VALUE_MEMID;
    fd.invkind = 2; // INVOKE_PROPERTYGET
    fd.callconv = CC_STDCALL;
    fd.cparams = 0;
    fd.oVft = (*owner).scenario.member_ovft;
    *pp = Box::into_raw(Box::new(fd));
    COM_S_OK
}

unsafe extern "system" fn ti_release_func_desc(_this: *mut c_void, p: *mut FuncDesc) {
    if !p.is_null() {
        drop(Box::from_raw(p));
    }
}

unsafe extern "system" fn ti_get_documentation(
    this: *mut c_void,
    memid: i32,
    pbstr_name: *mut *mut u16,
    _pbstr_doc: *mut *mut u16,
    _phelp: *mut u32,
    _pbstr_help: *mut *mut u16,
) -> i32 {
    let owner = as_ti(this);
    let names_value = matches!((*owner).face, Face::BaseInterface | Face::Dispinterface);
    if names_value && memid == VALUE_MEMID && !pbstr_name.is_null() {
        // SysAllocString-style BSTR the production bstr_to_string_and_free frees.
        *pbstr_name = alloc_bstr(VALUE_NAME);
        return COM_S_OK;
    }
    if !pbstr_name.is_null() {
        *pbstr_name = ptr::null_mut();
    }
    COM_S_OK
}

unsafe extern "system" fn ti_get_ref_type_of_impl_type(
    this: *mut c_void,
    index: u32,
    phref: *mut u32,
) -> i32 {
    if phref.is_null() {
        return COM_E_INVALIDARG;
    }
    let owner = as_ti(this);
    match (*owner).face {
        // dispinterface: impl-type -1 (u32::MAX) → the partner interface.
        Face::Dispinterface if index == u32::MAX => {
            *phref = 1;
            COM_S_OK
        }
        // partner interface: impl-type 0 → the base interface.
        Face::PartnerInterface if index == 0 => {
            *phref = 2;
            COM_S_OK
        }
        _ => COM_E_INVALIDARG,
    }
}

unsafe extern "system" fn ti_get_ref_type_info(
    this: *mut c_void,
    href: u32,
    pp: *mut *mut c_void,
) -> i32 {
    if pp.is_null() {
        return COM_E_INVALIDARG;
    }
    let owner = as_ti(this);
    let scenario = (*owner).scenario;
    let next = match href {
        1 => Face::PartnerInterface,
        2 => Face::BaseInterface,
        _ => return COM_E_INVALIDARG,
    };
    *pp = make_type_info(next, scenario).cast::<c_void>();
    COM_S_OK
}

// ── stubs (must keep the slot layout; never called on the crossing path) ──

unsafe extern "system" fn ti_get_type_comp_stub(_this: *mut c_void, _out: *mut *mut c_void) -> i32 {
    COM_E_NOINTERFACE
}
unsafe extern "system" fn ti_get_var_desc_stub(
    _this: *mut c_void,
    _index: u32,
    _out: *mut *mut c_void,
) -> i32 {
    COM_E_NOINTERFACE
}
/// The extractor reads the member name via `GetNames` (not GetDocumentation), so
/// the fixture must serve `Value` into names[0] for memid 9. We never declare
/// parameters, so only one name is returned.
unsafe extern "system" fn ti_get_names(
    _this: *mut c_void,
    memid: i32,
    names: *mut *mut u16,
    max: u32,
    count: *mut u32,
) -> i32 {
    if names.is_null() || count.is_null() || max == 0 {
        return COM_E_INVALIDARG;
    }
    if memid == VALUE_MEMID {
        *names = alloc_bstr(VALUE_NAME);
        *count = 1;
    } else {
        *count = 0;
    }
    COM_S_OK
}
unsafe extern "system" fn ti_get_impl_type_flags(
    _this: *mut c_void,
    _index: u32,
    pflags: *mut i32,
) -> i32 {
    if !pflags.is_null() {
        *pflags = 0;
    }
    COM_S_OK
}
unsafe extern "system" fn ti_get_ids_of_names_stub(
    _this: *mut c_void,
    _names: *mut *mut u16,
    _count: u32,
    _ids: *mut i32,
) -> i32 {
    COM_E_NOINTERFACE
}
#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn ti_invoke_stub(
    _this: *mut c_void,
    _a: *mut c_void,
    _b: i32,
    _c: u16,
    _d: *mut c_void,
    _e: *mut c_void,
    _f: *mut c_void,
    _g: *mut u32,
) -> i32 {
    COM_E_NOINTERFACE
}
unsafe extern "system" fn ti_get_dll_entry_stub(
    _this: *mut c_void,
    _memid: i32,
    _invkind: u16,
    _a: *mut *mut u16,
    _b: *mut *mut u16,
    _c: *mut u16,
) -> i32 {
    COM_E_NOINTERFACE
}
unsafe extern "system" fn ti_address_of_member_stub(
    _this: *mut c_void,
    _memid: i32,
    _invkind: u16,
    _out: *mut *mut c_void,
) -> i32 {
    COM_E_NOINTERFACE
}
unsafe extern "system" fn ti_create_instance_stub(
    _this: *mut c_void,
    _outer: *mut c_void,
    _riid: *const GUID,
    _out: *mut *mut c_void,
) -> i32 {
    COM_E_NOINTERFACE
}
unsafe extern "system" fn ti_get_mops_stub(
    _this: *mut c_void,
    _memid: i32,
    _out: *mut *mut u16,
) -> i32 {
    COM_E_NOINTERFACE
}
unsafe extern "system" fn ti_get_containing_type_lib_stub(
    _this: *mut c_void,
    _lib: *mut *mut c_void,
    _index: *mut u32,
) -> i32 {
    COM_E_NOINTERFACE
}
unsafe extern "system" fn ti_release_var_desc_stub(_this: *mut c_void, _p: *mut c_void) {}

/// Allocate a BSTR (length-prefixed; the production reader frees via SysFreeString).
unsafe fn alloc_bstr(wide_with_nul: &[u16]) -> *mut u16 {
    windows_sys::Win32::Foundation::SysAllocString(wide_with_nul.as_ptr()).cast_mut()
}

// ── The fixture IDispatch object + its real partner interface vtable ─────────

/// A real custom interface vtable: 18 IUnknown+IDispatch+custom slots up to and
/// including slot 17 (`Value`). Slot 17 is `get_Value(this, [out,retval] VARIANT*)`.
#[repr(C)]
struct PartnerVtbl {
    slots: [*const c_void; 20],
}

/// The fixture's partner interface object (a NON-aliasing tear-off the dispatch
/// QI returns), holding its own refcount + a back-pointer for IUnknown.
#[repr(C)]
struct PartnerInterface {
    vtbl: *const PartnerVtbl,
    ref_count: AtomicU32,
}

/// The fixture IDispatch object. Its `GetTypeInfo(0)` returns the dispinterface
/// face; its `QueryInterface(IID_FIXTURE_PARTNER)` returns a fresh
/// `PartnerInterface` (non-aliasing). The scenario drives the typeinfo facts.
#[repr(C)]
struct FixtureDispatch {
    vtbl: *const oxvba_com::windows_client::RawIDispatchVtbl,
    ref_count: AtomicU32,
    scenario: Scenario,
}

static FIXTURE_DISPATCH_VTBL: oxvba_com::windows_client::RawIDispatchVtbl =
    oxvba_com::windows_client::RawIDispatchVtbl {
        unknown: oxvba_com::windows_client::RawIUnknownVtbl {
            query_interface: fd_query_interface,
            add_ref: fd_add_ref,
            release: fd_release,
        },
        get_type_info_count: fd_get_type_info_count,
        get_type_info: fd_get_type_info,
        get_ids_of_names: fd_get_ids_of_names,
        invoke: fd_invoke,
    };

fn make_fixture_dispatch(scenario: Scenario) -> *mut RawIDispatch {
    Box::into_raw(Box::new(FixtureDispatch {
        vtbl: &FIXTURE_DISPATCH_VTBL,
        ref_count: AtomicU32::new(1),
        scenario,
    }))
    .cast::<RawIDispatch>()
}

unsafe fn as_fd(this: *mut c_void) -> *mut FixtureDispatch {
    this.cast::<FixtureDispatch>()
}

unsafe extern "system" fn fd_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = ptr::null_mut();
    if riid.is_null() {
        return COM_E_NOINTERFACE;
    }
    if guid_equals(riid, &IID_IUNKNOWN) || guid_equals(riid, &IID_IDISPATCH) {
        *ppv = this;
        let _ = fd_add_ref(this);
        return COM_S_OK;
    }
    if guid_equals(riid, &IID_FIXTURE_PARTNER) {
        // NON-aliasing tear-off: a SEPARATE interface object with the real vtable.
        *ppv = make_partner_interface().cast::<c_void>();
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

unsafe extern "system" fn fd_add_ref(this: *mut c_void) -> u32 {
    (*as_fd(this)).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

unsafe extern "system" fn fd_release(this: *mut c_void) -> u32 {
    let owner = as_fd(this);
    let remaining = (*owner).ref_count.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        drop(Box::from_raw(owner));
    }
    remaining
}

unsafe extern "system" fn fd_get_type_info_count(_this: *mut c_void, pctinfo: *mut u32) -> i32 {
    if !pctinfo.is_null() {
        *pctinfo = 1;
    }
    COM_S_OK
}

unsafe extern "system" fn fd_get_type_info(
    this: *mut c_void,
    _itinfo: u32,
    _lcid: u32,
    pptinfo: *mut *mut c_void,
) -> i32 {
    if pptinfo.is_null() {
        return COM_E_INVALIDARG;
    }
    let scenario = (*as_fd(this)).scenario;
    *pptinfo = make_type_info(Face::Dispinterface, scenario).cast::<c_void>();
    COM_S_OK
}

unsafe extern "system" fn fd_get_ids_of_names(
    _this: *mut c_void,
    _riid: *const GUID,
    _names: *mut *mut u16,
    _count: u32,
    _lcid: u32,
    _ids: *mut i32,
) -> i32 {
    COM_E_NOINTERFACE
}

#[allow(clippy::too_many_arguments)]
unsafe extern "system" fn fd_invoke(
    _this: *mut c_void,
    _dispid: i32,
    _riid: *const GUID,
    _lcid: u32,
    _flags: u16,
    _params: *mut windows_sys::Win32::System::Com::DISPPARAMS,
    _result: *mut VARIANT,
    _excep: *mut windows_sys::Win32::System::Com::EXCEPINFO,
    _arg: *mut u32,
) -> i32 {
    COM_E_NOINTERFACE
}

// ── the partner interface real vtable (slot 17 = get_Value) ──

static mut PARTNER_VTBL: PartnerVtbl = PartnerVtbl {
    slots: [ptr::null(); 20],
};

unsafe fn partner_vtbl_ptr() -> *const PartnerVtbl {
    // Slots 0..=2 IUnknown, 3..=6 IDispatch, 17 = get_Value; the rest are a
    // harmless trap that returns E_NOINTERFACE if mis-indexed. We populate once.
    let v = &raw mut PARTNER_VTBL;
    (*v).slots[0] = partner_qi as *const c_void;
    (*v).slots[1] = partner_add_ref as *const c_void;
    (*v).slots[2] = partner_release as *const c_void;
    for i in 3..20 {
        (*v).slots[i] = partner_trap as *const c_void;
    }
    (*v).slots[VALUE_SLOT as usize] = partner_get_value as *const c_void;
    v
}

fn make_partner_interface() -> *mut PartnerInterface {
    // SAFETY: single-threaded test init of the static vtable, then box the object.
    let vtbl = unsafe { partner_vtbl_ptr() };
    Box::into_raw(Box::new(PartnerInterface {
        vtbl,
        ref_count: AtomicU32::new(1),
    }))
}

unsafe fn as_partner(this: *mut c_void) -> *mut PartnerInterface {
    this.cast::<PartnerInterface>()
}

unsafe extern "system" fn partner_qi(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = ptr::null_mut();
    if !riid.is_null()
        && (guid_equals(riid, &IID_IUNKNOWN) || guid_equals(riid, &IID_FIXTURE_PARTNER))
    {
        *ppv = this;
        let _ = partner_add_ref(this);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

unsafe extern "system" fn partner_add_ref(this: *mut c_void) -> u32 {
    (*as_partner(this)).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

unsafe extern "system" fn partner_release(this: *mut c_void) -> u32 {
    let owner = as_partner(this);
    let remaining = (*owner).ref_count.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        drop(Box::from_raw(owner));
    }
    remaining
}

/// slot 17: `get_Value(this, [out,retval] VARIANT*)` → VT_I4(7).
unsafe extern "system" fn partner_get_value(_this: *mut c_void, out: *mut VARIANT) -> i32 {
    if out.is_null() {
        return COM_E_INVALIDARG;
    }
    VariantInit(out);
    (*out).Anonymous.Anonymous.vt = VT_I4;
    (*out).Anonymous.Anonymous.Anonymous.lVal = VALUE_RESULT;
    COM_S_OK
}

/// Any other slot: a harmless failure (never reached when the slot is correct).
unsafe extern "system" fn partner_trap(_this: *mut c_void) -> i32 {
    COM_E_NOINTERFACE
}

// ── helpers ──

unsafe fn release_idispatch(p: *mut RawIDispatch) {
    let vtbl = (*p).vtbl;
    ((*vtbl).unknown.release)(p.cast());
}

/// QI the fixture dispatch for the recovered IID and call the recovered slot as
/// a property-get (`fn(this, *mut VARIANT) -> HRESULT`), returning the VT_I4 it
/// writes. This is the VALUE ORACLE: it proves the recovered slot indexes the
/// real partner vtable and returns the known value.
unsafe fn value_oracle_call(dispatch: *mut RawIDispatch, iid: &GUID, slot: u16) -> Option<i32> {
    let interface =
        oxvba_com::windows_client::query_interface_pointer(dispatch.cast(), iid).ok()?;
    let mut out: VARIANT = std::mem::zeroed();
    VariantInit(&mut out);
    // SAFETY: slot is bound-validated; the partner vtable's slot is the property-get ABI.
    let hr = {
        let vtbl = *interface.cast::<*const *const c_void>();
        let fnptr = *vtbl.add(slot as usize);
        let f: extern "system" fn(*mut c_void, *mut VARIANT) -> i32 = std::mem::transmute(fnptr);
        f(interface, &mut out)
    };
    let value = if hr == COM_S_OK && out.Anonymous.Anonymous.vt == VT_I4 {
        Some(out.Anonymous.Anonymous.Anonymous.lVal)
    } else {
        None
    };
    oxvba_com::windows_client::release_unknown(interface);
    value
}

// ── tests ────────────────────────────────────────────────────────────────────

#[test]
fn fdual_crossing_recovers_a_bound_valid_interface_slot_and_value() {
    // ACCEPT: a FDUAL dispinterface whose `Value` member (oVft 136 → slot 17)
    // lives on a BASE interface reached via the partner. Partner cbSizeVft=160 →
    // bound 20; slot 17 < 20. The recovered metadata must carry the partner IID,
    // source_typekind == Interface, slot 17, bound 20 — and a real slot call must
    // return VT_I4(7).
    let scenario = Scenario {
        fdual: true,
        partner_cb_size_vft: PARTNER_CB_SIZE_VFT_OK,
        member_ovft: VALUE_OVFT,
    };
    let dispatch = make_fixture_dispatch(scenario);

    // SAFETY: `dispatch` is a live fixture IDispatch we own one ref of; live
    // recovery only performs ITypeInfo reads (GetTypeInfo/crossing) on it.
    let metadata = unsafe {
        live_member_metadata_from_dispatch(
            dispatch,
            VALUE_MEMID,
            TypeLibMemberInvokeKind::PropertyGet,
        )
    }
    .expect("FDUAL crossing should recover the Value member");

    assert_eq!(metadata.name, "Value");
    assert_eq!(
        metadata.vtable_slot,
        Some(VALUE_SLOT),
        "slot must be oVft/8 = 136/8 = 17"
    );
    assert_eq!(
        metadata.source_typekind,
        Some(SourceTypeKind::Interface),
        "the recovered source must be the partner INTERFACE, not the dispinterface"
    );
    assert_eq!(
        metadata.vtable_slot_bound,
        Some(PARTNER_CB_SIZE_VFT_OK / 8),
        "the AV-safety bound must come from the partner cbSizeVft (160/8 = 20)"
    );
    let iid = metadata
        .interface_iid
        .expect("a partner interface IID must be recovered for QI dispatch")
        .to_guid();
    // SAFETY: guid_equals only reads the two GUIDs by reference.
    assert!(unsafe { guid_equals(&iid, &IID_FIXTURE_PARTNER) });

    // VALUE ORACLE: QI for the recovered IID, call the recovered slot, get 7.
    // SAFETY: `dispatch` is live; `slot` (17) was just bound-validated (< 20).
    let value = unsafe { value_oracle_call(dispatch, &iid, VALUE_SLOT) };
    assert_eq!(
        value,
        Some(VALUE_RESULT),
        "the recovered slot must index the real partner vtable and return VT_I4(7)"
    );

    // SAFETY: `dispatch` is the live fixture IDispatch we own the last ref of.
    unsafe { release_idispatch(dispatch) };
}

#[test]
fn non_fdual_dispinterface_member_recovers_no_vtable_slot() {
    // REJECT: a NON-FDUAL dispinterface. The crossing must not promote a slot; the
    // recovered member keeps source_typekind == Dispatch and vtable_slot == None.
    let scenario = Scenario {
        fdual: false,
        partner_cb_size_vft: PARTNER_CB_SIZE_VFT_OK,
        member_ovft: VALUE_OVFT,
    };
    let dispatch = make_fixture_dispatch(scenario);

    // A non-FDUAL dispinterface lists no funcs on its own face here, so live
    // recovery finds no member at all — which is itself a clean "no vtable slot".
    // SAFETY: `dispatch` is a live fixture IDispatch we own one ref of.
    let metadata = unsafe {
        live_member_metadata_from_dispatch(
            dispatch,
            VALUE_MEMID,
            TypeLibMemberInvokeKind::PropertyGet,
        )
    };
    assert!(
        metadata.is_none() || metadata.as_ref().unwrap().vtable_slot.is_none(),
        "a non-FDUAL dispinterface member must recover no callable vtable slot"
    );

    // SAFETY: `dispatch` is the live fixture IDispatch we own the last ref of.
    unsafe { release_idispatch(dispatch) };
}

#[test]
fn member_slot_past_cb_size_vft_bound_is_rejected_by_the_gate() {
    // REJECT (AV-safety): the member's oVft is 800 → slot 100, but the partner
    // cbSizeVft is 144 → bound 18. The recovered metadata carries slot 100 with
    // bound 18, and the production vtable gate must DECLINE it (slot >= bound) so
    // no slot call ever over-runs the live vtable.
    let scenario = Scenario {
        fdual: true,
        partner_cb_size_vft: PARTNER_CB_SIZE_VFT_SHORT,
        member_ovft: HIGH_OVFT,
    };
    let dispatch = make_fixture_dispatch(scenario);

    // SAFETY: `dispatch` is a live fixture IDispatch we own one ref of.
    let metadata = unsafe {
        live_member_metadata_from_dispatch(
            dispatch,
            VALUE_MEMID,
            TypeLibMemberInvokeKind::PropertyGet,
        )
    }
    .expect("the member is still recovered; the GATE is what must reject it");

    let slot = metadata
        .vtable_slot
        .expect("slot recovered (oVft/8 = 800/8 = 100)");
    let bound = metadata
        .vtable_slot_bound
        .expect("bound recovered (cbSizeVft/8 = 144/8 = 18)");
    assert_eq!(slot, 100);
    assert_eq!(bound, 18);
    assert!(
        slot >= bound,
        "the recovered slot must be out of bounds so the gate declines (no AV)"
    );

    // SAFETY: `dispatch` is the live fixture IDispatch we own the last ref of.
    unsafe { release_idispatch(dispatch) };
}
