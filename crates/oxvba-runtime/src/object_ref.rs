use core::ffi::c_void;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU32, Ordering};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use crate::Variant;

pub const RUNTIME_S_OK: i32 = 0;
pub const RUNTIME_E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuntimeInterfaceId {
    IUnknown,
    IDispatch,
    IConnectionPointContainer,
    IConnectionPoint,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct RuntimeGuid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl RuntimeGuid {
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self {
            data1,
            data2,
            data3,
            data4,
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        let body = trimmed
            .strip_prefix('{')
            .and_then(|inner| inner.strip_suffix('}'))
            .unwrap_or(trimmed);
        let mut parts = body.split('-');
        let data1 = parse_hex_u32(parts.next()?, 8)?;
        let data2 = parse_hex_u16(parts.next()?, 4)?;
        let data3 = parse_hex_u16(parts.next()?, 4)?;
        let data4_hi = parts.next()?;
        let data4_lo = parts.next()?;
        if parts.next().is_some() || data4_hi.len() != 4 || data4_lo.len() != 12 {
            return None;
        }
        let mut data4 = [0u8; 8];
        data4[0] = parse_hex_u8(&data4_hi[0..2])?;
        data4[1] = parse_hex_u8(&data4_hi[2..4])?;
        for index in 0..6 {
            let start = index * 2;
            data4[index + 2] = parse_hex_u8(&data4_lo[start..start + 2])?;
        }
        Some(Self::new(data1, data2, data3, data4))
    }

    pub fn to_canonical_string(self) -> String {
        format!(
            "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            self.data1,
            self.data2,
            self.data3,
            self.data4[0],
            self.data4[1],
            self.data4[2],
            self.data4[3],
            self.data4[4],
            self.data4[5],
            self.data4[6],
            self.data4[7]
        )
    }
}

impl core::fmt::Display for RuntimeGuid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.to_canonical_string())
    }
}

fn parse_hex_u32(input: &str, expected_len: usize) -> Option<u32> {
    if input.len() != expected_len {
        return None;
    }
    u32::from_str_radix(input, 16).ok()
}

fn parse_hex_u16(input: &str, expected_len: usize) -> Option<u16> {
    if input.len() != expected_len {
        return None;
    }
    u16::from_str_radix(input, 16).ok()
}

fn parse_hex_u8(input: &str) -> Option<u8> {
    if input.len() != 2 {
        return None;
    }
    u8::from_str_radix(input, 16).ok()
}

pub const RUNTIME_GUID_IUNKNOWN: RuntimeGuid =
    RuntimeGuid::new(0x0000_0000, 0x0000, 0x0000, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
pub const RUNTIME_GUID_IDISPATCH: RuntimeGuid =
    RuntimeGuid::new(0x0002_0400, 0x0000, 0x0000, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
pub const RUNTIME_GUID_ICONNECTIONPOINTCONTAINER: RuntimeGuid = RuntimeGuid::new(
    0xB196_B284,
    0xBAB4,
    0x101A,
    [0xB6, 0x9C, 0x00, 0xAA, 0x00, 0x34, 0x1D, 0x07],
);
pub const RUNTIME_GUID_ICONNECTIONPOINT: RuntimeGuid = RuntimeGuid::new(
    0xB196_B286,
    0xBAB4,
    0x101A,
    [0xB6, 0x9C, 0x00, 0xAA, 0x00, 0x34, 0x1D, 0x07],
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuntimeInterfaceKind {
    Unknown,
    Dispatch,
    Dual,
    Source,
    ConnectionPointContainer,
    ConnectionPoint,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimeInterfaceIdentity {
    pub id: RuntimeInterfaceId,
    pub guid: RuntimeGuid,
    pub name: &'static str,
    pub kind: RuntimeInterfaceKind,
    pub major_version: Option<u16>,
    pub minor_version: Option<u16>,
    pub lcid: Option<u32>,
}

impl RuntimeInterfaceIdentity {
    pub const fn new(
        id: RuntimeInterfaceId,
        guid: RuntimeGuid,
        name: &'static str,
        kind: RuntimeInterfaceKind,
        major_version: Option<u16>,
        minor_version: Option<u16>,
        lcid: Option<u32>,
    ) -> Self {
        Self {
            id,
            guid,
            name,
            kind,
            major_version,
            minor_version,
            lcid,
        }
    }

    pub const fn custom(
        guid: RuntimeGuid,
        name: &'static str,
        kind: RuntimeInterfaceKind,
        major_version: Option<u16>,
        minor_version: Option<u16>,
        lcid: Option<u32>,
    ) -> Self {
        Self::new(
            RuntimeInterfaceId::Unsupported,
            guid,
            name,
            kind,
            major_version,
            minor_version,
            lcid,
        )
    }
}

pub const RUNTIME_IUNKNOWN_INTERFACE_IDENTITY: RuntimeInterfaceIdentity =
    RuntimeInterfaceIdentity::new(
        RuntimeInterfaceId::IUnknown,
        RUNTIME_GUID_IUNKNOWN,
        "IUnknown",
        RuntimeInterfaceKind::Unknown,
        None,
        None,
        None,
    );
pub const RUNTIME_IDISPATCH_INTERFACE_IDENTITY: RuntimeInterfaceIdentity =
    RuntimeInterfaceIdentity::new(
        RuntimeInterfaceId::IDispatch,
        RUNTIME_GUID_IDISPATCH,
        "IDispatch",
        RuntimeInterfaceKind::Dispatch,
        None,
        None,
        None,
    );
pub const RUNTIME_ICONNECTIONPOINTCONTAINER_INTERFACE_IDENTITY: RuntimeInterfaceIdentity =
    RuntimeInterfaceIdentity::new(
        RuntimeInterfaceId::IConnectionPointContainer,
        RUNTIME_GUID_ICONNECTIONPOINTCONTAINER,
        "IConnectionPointContainer",
        RuntimeInterfaceKind::ConnectionPointContainer,
        None,
        None,
        None,
    );
pub const RUNTIME_ICONNECTIONPOINT_INTERFACE_IDENTITY: RuntimeInterfaceIdentity =
    RuntimeInterfaceIdentity::new(
        RuntimeInterfaceId::IConnectionPoint,
        RUNTIME_GUID_ICONNECTIONPOINT,
        "IConnectionPoint",
        RuntimeInterfaceKind::ConnectionPoint,
        None,
        None,
        None,
    );

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeLifetimePolicy {
    RefCounted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeApartmentModel {
    Unknown,
    SingleThreaded,
    MultiThreaded,
    Both,
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeObjectIdentity {
    pub stable_object_id: u64,
    /// Per-instance identity key (unique per allocated object). For a fresh project-class
    /// instance this is a VM-allocated unique id; the object's true identity for `Is` is its
    /// IUnknown pointer, this is the internal lookup handle 1:1 with it.
    pub compat_identity: i32,
    /// Class/route key: which project-dynamic route (class dispatch table) this instance
    /// belongs to. Distinct from `compat_identity` once instances are per-`New` allocations —
    /// many instances share one `route_key` but each has its own `compat_identity`.
    pub route_key: i32,
    /// The bundle (".NET assembly") that owns this instance's class. `(bundle_id,
    /// route_key)` together select the class dispatch table for a multi-bundle VM;
    /// `0` for single-bundle runs. Identity (`Is`) remains the IUnknown pointer.
    pub bundle_id: i32,
    /// True when this instance's class has a `Class_Terminate` — so the last `Release`
    /// (`compat_release` at refcount 0) enqueues it for the VM to run `Class_Terminate`.
    pub terminates_on_release: bool,
    pub class_descriptor: &'static RuntimeClassDescriptor,
    pub lifetime_policy: RuntimeLifetimePolicy,
    pub apartment_model: RuntimeApartmentModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeInterfaceProjection {
    pub object_identity: RuntimeObjectIdentity,
    pub interface_identity: RuntimeInterfaceIdentity,
    pub interface_descriptor: &'static RuntimeInterfaceDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeMemberInvokeKind {
    Method,
    PropertyGet,
    PropertyLet,
    PropertySet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeValueType {
    Variant,
    Long,
    Integer,
    String,
    Boolean,
    Double,
    Single,
    Currency,
    Date,
    Decimal,
    Object,
    Byte,
    LongLong,
    LongPtr,
    Record,
    Array,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeParamDescriptor {
    pub name: &'static str,
    pub value_type: RuntimeValueType,
    pub by_ref: bool,
    pub optional: bool,
    pub param_array: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeMemberDescriptor {
    pub name: &'static str,
    pub dispatch_id: i32,
    pub vtable_slot: Option<u16>,
    pub invoke_kind: RuntimeMemberInvokeKind,
    pub arity: usize,
    pub params: &'static [RuntimeParamDescriptor],
    pub return_type: Option<RuntimeValueType>,
    pub is_default_member: bool,
    pub is_enumerator_member: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeClassFieldDescriptor {
    pub name: &'static str,
    pub token: i32,
    pub value_type: RuntimeValueType,
    pub array_element_type: Option<RuntimeValueType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeClassActivationDescriptor {
    ProjectClass { class_index: usize },
    ExternClass { import_index: usize },
    ComClass { prog_id: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeClassAsNewFieldDescriptor {
    pub field_token: i32,
    pub activation: RuntimeClassActivationDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeClassLifecycleDescriptor {
    pub has_initialize: bool,
    pub has_terminate: bool,
}

pub const RUNTIME_CLASS_LIFECYCLE_NONE: RuntimeClassLifecycleDescriptor =
    RuntimeClassLifecycleDescriptor {
        has_initialize: false,
        has_terminate: false,
    };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeInterfaceDescriptor {
    pub id: RuntimeInterfaceId,
    pub identity: RuntimeInterfaceIdentity,
    pub name: &'static str,
    pub members: &'static [RuntimeMemberDescriptor],
    pub dual_dispatch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProjectClassIdentity {
    pub unit_name: &'static str,
    pub class_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeClassDescriptor {
    pub name: &'static str,
    pub project_identity: Option<RuntimeProjectClassIdentity>,
    pub predeclared: bool,
    pub lifecycle: RuntimeClassLifecycleDescriptor,
    pub fields: &'static [RuntimeClassFieldDescriptor],
    pub as_new_fields: &'static [RuntimeClassAsNewFieldDescriptor],
    pub implements: &'static [&'static str],
    pub interfaces: &'static [RuntimeInterfaceDescriptor],
}

pub const RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR: RuntimeInterfaceDescriptor =
    RuntimeInterfaceDescriptor {
        id: RuntimeInterfaceId::IUnknown,
        identity: RUNTIME_IUNKNOWN_INTERFACE_IDENTITY,
        name: "IUnknown",
        members: &[],
        dual_dispatch: false,
    };

pub const COMPAT_OBJECT_CLASS_DESCRIPTOR: RuntimeClassDescriptor = RuntimeClassDescriptor {
    name: "OxVba.CompatObject",
    project_identity: None,
    predeclared: false,
    lifecycle: RUNTIME_CLASS_LIFECYCLE_NONE,
    fields: &[],
    as_new_fields: &[],
    implements: &[],
    interfaces: &[RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR],
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeDispatchCacheKey {
    pub interface_descriptor: usize,
    pub interface_id: RuntimeInterfaceId,
    pub normalized_member_name: String,
    pub invoke_kind: RuntimeMemberInvokeKind,
    pub arity: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDispatchPlan {
    pub interface_id: RuntimeInterfaceId,
    pub member_index: usize,
    pub dispatch_id: i32,
    pub vtable_slot: Option<u16>,
    pub invoke_kind: RuntimeMemberInvokeKind,
    pub is_default_member: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuntimeDispatchPlanCache {
    entries: BTreeMap<RuntimeDispatchCacheKey, RuntimeDispatchPlan>,
}

impl RuntimeDispatchPlanCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn resolve_member(
        &mut self,
        interface: &RuntimeInterfaceDescriptor,
        member_name: &str,
        invoke_kind: RuntimeMemberInvokeKind,
        arity: usize,
    ) -> Option<RuntimeDispatchPlan> {
        let key = RuntimeDispatchCacheKey {
            interface_descriptor: runtime_interface_descriptor_key(interface),
            interface_id: interface.id,
            normalized_member_name: normalize_runtime_member_name(member_name),
            invoke_kind,
            arity,
        };
        self.resolve_with_key(interface, key, |candidate, key| {
            normalize_runtime_member_name(candidate.name) == key.normalized_member_name
                && candidate.invoke_kind == key.invoke_kind
                && candidate.arity == key.arity
        })
    }

    pub fn resolve_default_member(
        &mut self,
        interface: &RuntimeInterfaceDescriptor,
        invoke_kind: RuntimeMemberInvokeKind,
        arity: usize,
    ) -> Option<RuntimeDispatchPlan> {
        let key = RuntimeDispatchCacheKey {
            interface_descriptor: runtime_interface_descriptor_key(interface),
            interface_id: interface.id,
            normalized_member_name: "<default>".to_string(),
            invoke_kind,
            arity,
        };
        self.resolve_with_key(interface, key, |candidate, key| {
            candidate.is_default_member
                && candidate.invoke_kind == key.invoke_kind
                && candidate.arity == key.arity
        })
    }

    pub fn resolve_member_unhinted(
        &mut self,
        interface: &RuntimeInterfaceDescriptor,
        member_name: &str,
        arity: usize,
    ) -> Option<RuntimeDispatchPlan> {
        let normalized_member_name = normalize_runtime_member_name(member_name);
        self.resolve_unhinted_with_name(
            interface,
            normalized_member_name,
            arity,
            |candidate, name| normalize_runtime_member_name(candidate.name) == *name,
        )
    }

    pub fn resolve_default_member_unhinted(
        &mut self,
        interface: &RuntimeInterfaceDescriptor,
        arity: usize,
    ) -> Option<RuntimeDispatchPlan> {
        self.resolve_unhinted_with_name(
            interface,
            "<default>".to_string(),
            arity,
            |candidate, _| candidate.is_default_member,
        )
    }

    fn resolve_unhinted_with_name(
        &mut self,
        interface: &RuntimeInterfaceDescriptor,
        normalized_member_name: String,
        arity: usize,
        matches_name: impl Fn(&RuntimeMemberDescriptor, &String) -> bool,
    ) -> Option<RuntimeDispatchPlan> {
        let mut matches = interface
            .members
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                matches_name(candidate, &normalized_member_name) && candidate.arity == arity
            });
        let (member_index, member) = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let key = RuntimeDispatchCacheKey {
            interface_descriptor: runtime_interface_descriptor_key(interface),
            interface_id: interface.id,
            normalized_member_name,
            invoke_kind: member.invoke_kind,
            arity,
        };
        self.resolve_with_key(interface, key, |candidate, key| {
            candidate.invoke_kind == key.invoke_kind
                && candidate.arity == key.arity
                && candidate.name == interface.members[member_index].name
                && candidate.dispatch_id == interface.members[member_index].dispatch_id
        })
    }

    fn resolve_with_key(
        &mut self,
        interface: &RuntimeInterfaceDescriptor,
        key: RuntimeDispatchCacheKey,
        matches: impl Fn(&RuntimeMemberDescriptor, &RuntimeDispatchCacheKey) -> bool,
    ) -> Option<RuntimeDispatchPlan> {
        if let Some(plan) = self.entries.get(&key).copied() {
            return Some(plan);
        }
        let mut matches = interface
            .members
            .iter()
            .enumerate()
            .filter(|(_, candidate)| matches(candidate, &key));
        let (member_index, member) = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let plan = RuntimeDispatchPlan {
            interface_id: interface.id,
            member_index,
            dispatch_id: member.dispatch_id,
            vtable_slot: member.vtable_slot,
            invoke_kind: member.invoke_kind,
            is_default_member: member.is_default_member,
        };
        self.entries.insert(key, plan);
        Some(plan)
    }
}

fn normalize_runtime_member_name(member_name: &str) -> String {
    member_name.trim().to_ascii_lowercase()
}

fn runtime_interface_descriptor_key(interface: &RuntimeInterfaceDescriptor) -> usize {
    interface as *const RuntimeInterfaceDescriptor as usize
}

#[repr(C)]
pub struct RawRuntimeIUnknownVtbl {
    pub query_interface:
        unsafe extern "C" fn(this: *mut c_void, iid: RuntimeGuid, ppv: *mut *mut c_void) -> i32,
    pub add_ref: unsafe extern "C" fn(this: *mut c_void) -> u32,
    pub release: unsafe extern "C" fn(this: *mut c_void) -> u32,
}

#[repr(C)]
pub struct RawRuntimeIUnknown {
    pub vtbl: *const RawRuntimeIUnknownVtbl,
}

#[repr(C)]
struct CompatObjectBase {
    unknown: RawRuntimeIUnknown,
    ref_count: AtomicU32,
    identity: RuntimeObjectIdentity,
    class_descriptor: &'static RuntimeClassDescriptor,
    fields: RefCell<BTreeMap<i32, Variant>>,
    /// Built-in native-object state carried on the box itself (currently a `VBA.Collection`'s
    /// contents). Lazily created on first use via [`ObjectRef::with_native_collection`]; freed
    /// with the box, so there is no VM-side store and no leak. `None` for ordinary project
    /// instances that hold no native collection.
    native_state: RefCell<Option<crate::collection::CollectionData>>,
    parked_for_termination: Cell<bool>,
    running_termination: Cell<bool>,
    terminated: Cell<bool>,
}

static COMPAT_OBJECT_VTBL: RawRuntimeIUnknownVtbl = RawRuntimeIUnknownVtbl {
    query_interface: compat_query_interface,
    add_ref: compat_add_ref,
    release: compat_release,
};

unsafe extern "C" fn compat_query_interface(
    this: *mut c_void,
    iid: RuntimeGuid,
    ppv: *mut *mut c_void,
) -> i32 {
    if ppv.is_null() {
        return RUNTIME_E_NOINTERFACE;
    }
    // SAFETY: per the QueryInterface ABI, `ppv` (checked non-null above) points to a writable
    // out-slot owned by the caller for the duration of this call.
    unsafe {
        *ppv = core::ptr::null_mut();
    }
    let owner = compat_owner_from_this(this);
    // SAFETY: this vtable entry is only installed via `COMPAT_OBJECT_VTBL`, which is only
    // attached to `CompatObjectBase` boxes minted by `from_compat_object` (`#[repr(C)]` with
    // `unknown` as the first field), so `owner` is that live box; the caller's outstanding COM
    // reference keeps it alive across the call.
    let supports_iid = unsafe {
        (*owner)
            .class_descriptor
            .interfaces
            .iter()
            .any(|interface| interface.identity.guid == iid)
    };
    if !supports_iid {
        return RUNTIME_E_NOINTERFACE;
    }
    // SAFETY: same caller-owned writable out-slot as above (QueryInterface ABI, non-null checked).
    unsafe {
        *ppv = this;
    }
    // SAFETY: `this` was validated above as a live `CompatObjectBase`, which is exactly
    // `compat_add_ref`'s requirement; the reference taken here is owned by the `*ppv` we
    // just handed to the caller.
    unsafe { compat_add_ref(this) };
    RUNTIME_S_OK
}

/// Pending `Class_Terminate` work, fed by `compat_release` and drained by the VM at statement
/// boundaries. A pending project object is parked: its refcount reached zero, but the
/// `CompatObjectBase` is still owned by this queue so `Class_Terminate` can run against the
/// original box and its fields can be released afterwards.
#[derive(Default)]
struct TerminationQueue {
    /// `(instance_id, bundle_id, route_key)` — the bundle + class needed to find
    /// the parked instance's `Class_Terminate` at drain time.
    pending: Vec<(i32, i32, i32)>,
    parked: BTreeMap<i32, *mut CompatObjectBase>,
}

thread_local! {
    static TERMINATIONS: RefCell<TerminationQueue> = RefCell::new(TerminationQueue::default());
}

/// True when at least one project instance has reached refcount 0 and awaits `Class_Terminate`.
/// Cheap gate the interpreter checks at statement boundaries.
pub fn has_pending_terminations() -> bool {
    TERMINATIONS.with(|q| !q.borrow().pending.is_empty())
}

/// Drains and returns the queued `(instance_id, bundle_id, route_key)` terminations. The parked
/// object boxes remain owned by the termination queue until `finish_pending_termination` is called.
pub fn take_pending_terminations() -> Vec<(i32, i32, i32)> {
    TERMINATIONS.with(|q| std::mem::take(&mut q.borrow_mut().pending))
}

/// Returns a retained reference to the original parked object whose `Class_Terminate` is about to
/// run. The returned `ObjectRef` is the same boxed object, not a rematerialized identity clone.
pub fn retained_parked_termination_object(instance_id: i32) -> Option<ObjectRef> {
    TERMINATIONS.with(|q| {
        let q = q.borrow();
        let owner = *q.parked.get(&instance_id)?;
        // SAFETY: `owner` came from the `parked` map, which owns the box that `compat_release`
        // parked instead of freeing — it stays allocated until `finish_pending_termination` or
        // `reset_pending_terminations` removes it — so this is a live `CompatObjectBase`.
        // `compat_add_ref` retains it for the returned `ObjectRef`, and the `unknown` field
        // pointer projected from the non-null box pointer cannot be null.
        unsafe {
            (*owner).running_termination.set(true);
            compat_add_ref((&mut (*owner).unknown as *mut RawRuntimeIUnknown).cast());
            Some(ObjectRef(NonNull::new_unchecked(
                &mut (*owner).unknown as *mut RawRuntimeIUnknown,
            )))
        }
    })
}

/// Finishes teardown for a parked project object after `Class_Terminate` has returned.
///
/// If `Class_Terminate` resurrected `Me`, the object remains alive and its fields/subscriptions
/// stay intact. Otherwise the field store is dropped before the box is freed; releasing those
/// field Variants may enqueue cascaded child terminations.
pub fn finish_pending_termination(instance_id: i32) -> bool {
    let owner = TERMINATIONS.with(|q| q.borrow_mut().parked.remove(&instance_id));
    let Some(owner) = owner else {
        return false;
    };
    // SAFETY: `owner` was just removed from the `parked` map, transferring the queue's sole
    // ownership of the box `compat_release` parked (the pointer originally came from
    // `Box::into_raw` in `from_compat_object`). If `Class_Terminate` resurrected the object
    // (refcount > 0) we un-park it and leave it to the outstanding references; otherwise nothing
    // else can reach it, and removal from the map guarantees `Box::from_raw` frees it exactly once.
    unsafe {
        (*owner).running_termination.set(false);
        (*owner).terminated.set(true);
        if (*owner).ref_count.load(Ordering::Acquire) > 0 {
            (*owner).parked_for_termination.set(false);
            return false;
        }
        (*owner).fields.borrow_mut().clear();
        (*owner).native_state.borrow_mut().take();
        crate::live_counters::object_box_freed();
        drop(Box::from_raw(owner));
        true
    }
}

/// Resets all pending termination state (used when a VM run starts/ends).
pub fn reset_pending_terminations() {
    TERMINATIONS.with(|q| {
        let mut q = q.borrow_mut();
        q.pending.clear();
        for (_, owner) in std::mem::take(&mut q.parked) {
            // SAFETY: `std::mem::take` moved each parked entry out of the queue, transferring its
            // sole ownership of the box `compat_release` parked (pointer from `Box::into_raw` in
            // `from_compat_object`). Resurrected boxes (refcount > 0) are un-parked and left to
            // their outstanding references; the rest are unreachable and freed exactly once here.
            unsafe {
                (*owner).running_termination.set(false);
                (*owner).terminated.set(true);
                if (*owner).ref_count.load(Ordering::Acquire) > 0 {
                    (*owner).parked_for_termination.set(false);
                    continue;
                }
                (*owner).fields.borrow_mut().clear();
                (*owner).native_state.borrow_mut().take();
                crate::live_counters::object_box_freed();
                drop(Box::from_raw(owner));
            }
        }
    });
}

unsafe extern "C" fn compat_add_ref(this: *mut c_void) -> u32 {
    let owner = compat_owner_from_this(this);
    // SAFETY: this vtable entry only exists on `CompatObjectBase` boxes (`COMPAT_OBJECT_VTBL` is
    // installed solely by `from_compat_object`; `#[repr(C)]` puts `unknown` first), and the
    // caller holds a reference that keeps the box alive across the call; `ref_count` is atomic.
    unsafe { (*owner).ref_count.fetch_add(1, Ordering::AcqRel) + 1 }
}

unsafe extern "C" fn compat_release(this: *mut c_void) -> u32 {
    let owner = compat_owner_from_this(this);
    // SAFETY: as in `compat_add_ref`, `this` is a live `CompatObjectBase` kept alive by the
    // reference the caller is releasing right now.
    let previous = unsafe { (*owner).ref_count.fetch_sub(1, Ordering::AcqRel) };
    let remaining = previous.saturating_sub(1);
    if remaining == 0 {
        // For a class with a `Class_Terminate`, the last release schedules per-instance
        // teardown for the VM to run at the next statement boundary (VBA timing). The box is
        // parked, not freed, so teardown runs against the original field-owning object.
        // SAFETY: even at refcount 0 the box is still allocated — this call alone decides below
        // whether to park or free it — so `owner` stays valid throughout this function.
        let identity = unsafe { (*owner).identity };
        let should_park = identity.terminates_on_release
            // SAFETY: same still-allocated box as above; the `Cell` accesses are single-threaded
            // (`ObjectRef` is neither `Send` nor `Sync`, so these vtable fns only run on the
            // owning VM thread, matching the thread-local termination queue).
            && unsafe { !(*owner).terminated.get() && !(*owner).running_termination.get() };
        if should_park {
            TERMINATIONS.with(|q| {
                let mut q = q.borrow_mut();
                // SAFETY: the box is still allocated (nothing frees it before this point in the
                // function) and the `Cell` flip is single-threaded as noted above; setting the
                // flag before inserting hands the queue ownership of the box exactly once.
                if !unsafe { (*owner).parked_for_termination.replace(true) } {
                    q.pending.push((
                        identity.compat_identity,
                        identity.bundle_id,
                        identity.route_key,
                    ));
                    q.parked.insert(identity.compat_identity, owner);
                }
            });
            return remaining;
        }
        // SAFETY: same still-allocated box; if the flag is set, the termination queue owns the
        // box and will free it (`finish_pending_termination`/`reset_pending_terminations`), so
        // this path must only read the flag and return without freeing.
        if unsafe { (*owner).parked_for_termination.get() } {
            return remaining;
        }
        // SAFETY: refcount reached 0 and the box is neither park-eligible nor parked, so this
        // call released the final reference to the allocation minted by `Box::into_raw` in
        // `from_compat_object` and nothing else can reach it. Clearing `fields` first releases
        // child Variants (which may enqueue cascaded terminations, never touching this box),
        // then `Box::from_raw` reclaims the allocation exactly once.
        unsafe {
            (*owner).fields.borrow_mut().clear();
            (*owner).native_state.borrow_mut().take();
            crate::live_counters::object_box_freed();
            drop(Box::from_raw(owner));
        }
    }
    remaining
}

fn compat_field_get(owner: *mut CompatObjectBase, token: i32) -> Option<Variant> {
    // SAFETY: the only caller (`ObjectRef::project_field_get`) guards with
    // `is_project_instance()` (a vtbl-identity check), so `owner` is a live `CompatObjectBase`
    // kept alive by that caller's retained `ObjectRef` for the duration of the borrow.
    unsafe { (*owner).fields.borrow().get(&token).cloned() }
}

fn compat_field_set(owner: *mut CompatObjectBase, token: i32, value: Variant) {
    // SAFETY: as in `compat_field_get`, the sole caller (`ObjectRef::project_field_set`)
    // verified via `is_project_instance()` that `owner` is a live `CompatObjectBase` kept
    // alive by the caller's retained `ObjectRef`.
    unsafe {
        if matches!(value.vtype(), crate::VarType::Empty) {
            (*owner).fields.borrow_mut().remove(&token);
        } else {
            (*owner).fields.borrow_mut().insert(token, value);
        }
    }
}

fn compat_owner_from_unknown(unknown: *mut RawRuntimeIUnknown) -> *mut CompatObjectBase {
    unknown.cast::<CompatObjectBase>()
}

fn compat_owner_from_this(this: *mut c_void) -> *mut CompatObjectBase {
    compat_owner_from_unknown(this.cast::<RawRuntimeIUnknown>())
}

#[repr(transparent)]
pub struct ObjectRef(NonNull<RawRuntimeIUnknown>);

impl ObjectRef {
    pub fn from_compat_identity(compat_identity: i32) -> Self {
        Self::from_compat_identity_with_descriptor(compat_identity, &COMPAT_OBJECT_CLASS_DESCRIPTOR)
    }

    pub fn from_compat_identity_with_descriptor(
        compat_identity: i32,
        class_descriptor: &'static RuntimeClassDescriptor,
    ) -> Self {
        // Legacy/template path: identity key and route key coincide; no terminate hook.
        Self::from_compat_object(compat_identity, compat_identity, 0, false, class_descriptor)
    }

    /// Allocates a fresh project-class instance: a distinct `CompatObjectBase` (a distinct
    /// IUnknown, hence a distinct identity for `Is`) with its own per-instance `instance_id`
    /// and the class's `route_key` for member dispatch. Refcount starts at 1; the instance is
    /// not pinned by any route map, so its lifetime tracks real references. `has_terminate`
    /// marks classes with a `Class_Terminate`, so the last release enqueues it for the VM.
    pub fn from_project_instance(
        instance_id: i32,
        route_key: i32,
        bundle_id: i32,
        has_terminate: bool,
        class_descriptor: &'static RuntimeClassDescriptor,
    ) -> Self {
        Self::from_compat_object(
            instance_id,
            route_key,
            bundle_id,
            has_terminate,
            class_descriptor,
        )
    }

    fn from_compat_object(
        compat_identity: i32,
        route_key: i32,
        bundle_id: i32,
        terminates_on_release: bool,
        class_descriptor: &'static RuntimeClassDescriptor,
    ) -> Self {
        let stable_object_id = compat_identity as u32 as u64;
        let boxed = Box::new(CompatObjectBase {
            unknown: RawRuntimeIUnknown {
                vtbl: &COMPAT_OBJECT_VTBL,
            },
            ref_count: AtomicU32::new(1),
            identity: RuntimeObjectIdentity {
                stable_object_id,
                compat_identity,
                route_key,
                bundle_id,
                terminates_on_release,
                class_descriptor,
                lifetime_policy: RuntimeLifetimePolicy::RefCounted,
                apartment_model: RuntimeApartmentModel::Unknown,
            },
            class_descriptor,
            fields: RefCell::new(BTreeMap::new()),
            native_state: RefCell::new(None),
            parked_for_termination: Cell::new(false),
            running_termination: Cell::new(false),
            terminated: Cell::new(false),
        });
        let raw = Box::into_raw(boxed);
        crate::live_counters::object_box_allocated();
        // SAFETY: `raw` came from `Box::into_raw` of the freshly allocated `CompatObjectBase`
        // above, so projecting a pointer to its first field (`unknown`) is in-bounds and
        // non-null; the box stays alive until the refcount started at 1 here drains to zero.
        let unknown = unsafe { &mut (*raw).unknown as *mut RawRuntimeIUnknown };
        Self(NonNull::new(unknown).expect("compat object unknown pointer must be non-null"))
    }

    pub fn query_iunknown(&self) -> Self {
        self.clone()
    }

    pub fn compat_identity(&self) -> i32 {
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        // SAFETY: `self.0` points at a live `CompatObjectBase`. Every `ObjectRef` in the
        // workspace is such a box: the safe constructors mint them, the COM bridge wraps even
        // foreign IDispatch/IUnknown objects in compat boxes keyed by a binding handle
        // (`retained_runtime_object` in oxvba-com), and the unsafe raw constructors only
        // round-trip pointers `raw_iunknown()` produced from those boxes (Variant payloads,
        // SAFEARRAY object slots). The two entry points that previously reached here without an
        // `is_compat_object` check — the `Debug`/`Display` impls below — now guard on it, so the
        // remaining callers (`raw`/`route_key`/`is_project_instance`/HAL/vm2 dispatch) all sit on
        // compat objects. This `ObjectRef`'s retained reference keeps the box alive for the read.
        unsafe { (*owner).identity.compat_identity }
    }

    pub fn raw(&self) -> i32 {
        self.compat_identity()
    }

    /// Class/route key for project-dynamic dispatch (which class's route this instance uses).
    /// Only valid for compat (project-class) objects, like `compat_identity`/`class_descriptor`.
    pub fn route_key(&self) -> i32 {
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        // SAFETY: documented contract (doc above and `is_compat_object`): callers only invoke
        // this on compat objects, so `owner` is the live `CompatObjectBase` (`#[repr(C)]`,
        // `unknown` first) kept alive by this `ObjectRef`'s retained reference.
        unsafe { (*owner).identity.route_key }
    }

    /// The bundle (".NET assembly") that owns this instance's class — `(bundle_id,
    /// route_key)` selects the class dispatch table in a multi-bundle VM. `0` for
    /// single-bundle runs.
    pub fn bundle_id(&self) -> i32 {
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        // SAFETY: documented contract as for `route_key`: only called on compat objects, so
        // `owner` is the live `CompatObjectBase` kept alive by this `ObjectRef`'s reference.
        unsafe { (*owner).identity.bundle_id }
    }

    pub fn is_project_instance(&self) -> bool {
        self.is_compat_object() && self.compat_identity() != self.route_key()
    }

    pub fn raw_iunknown(&self) -> *mut RawRuntimeIUnknown {
        self.0.as_ptr()
    }

    /// True when this `ObjectRef` wraps one of our `CompatObjectBase` boxes (a project-class or
    /// internal object), as opposed to a foreign COM interface obtained via
    /// `from_raw_iunknown_addref`. Reading the vtbl pointer is safe for any IUnknown (every COM
    /// object's first field is its vtbl); comparing it to our static vtbl tells the two apart.
    /// `compat_identity`/`route_key`/`class_descriptor`/`terminates_on_release` are only valid
    /// when this returns true.
    pub fn is_compat_object(&self) -> bool {
        // SAFETY: every COM-shaped object's first pointer-sized field is its vtbl (see doc
        // above), so this read is valid for compat boxes and foreign IUnknowns alike; `self.0`
        // is non-null and the object is alive because this `ObjectRef` holds a retained
        // reference to it.
        let vtbl = unsafe { (*self.0.as_ptr()).vtbl };
        std::ptr::eq(vtbl, &COMPAT_OBJECT_VTBL)
    }

    /// True when releasing this instance's last reference should run a `Class_Terminate`. Safe for
    /// any `ObjectRef`: foreign COM objects (which carry no such flag) report `false`.
    pub fn terminates_on_release(&self) -> bool {
        if !self.is_compat_object() {
            return false;
        }
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        // SAFETY: guarded by the `is_compat_object()` check above (vtbl identity), so `owner` is
        // the live `CompatObjectBase` kept alive by this `ObjectRef`'s retained reference.
        unsafe { (*owner).identity.terminates_on_release }
    }

    pub fn class_descriptor(&self) -> &'static RuntimeClassDescriptor {
        debug_assert!(
            self.is_compat_object(),
            "class_descriptor is only valid for runtime compat objects"
        );
        self.try_class_descriptor()
            .expect("class_descriptor is only valid for runtime compat objects")
    }

    pub fn try_class_descriptor(&self) -> Option<&'static RuntimeClassDescriptor> {
        if !self.is_compat_object() {
            return None;
        }
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        // SAFETY: documented contract (`is_compat_object`): only valid on compat objects, so
        // `owner` is the live `CompatObjectBase` kept alive by this `ObjectRef`'s retained
        // reference; the descriptor read out of it is `&'static`.
        Some(unsafe { (*owner).class_descriptor })
    }

    pub fn object_identity(&self) -> RuntimeObjectIdentity {
        debug_assert!(
            self.is_compat_object(),
            "object_identity is only valid for runtime compat objects"
        );
        self.try_object_identity()
            .expect("object_identity is only valid for runtime compat objects")
    }

    pub fn try_object_identity(&self) -> Option<RuntimeObjectIdentity> {
        if !self.is_compat_object() {
            return None;
        }
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        // SAFETY: documented contract as for `class_descriptor`: only called on compat objects,
        // so `owner` is the live `CompatObjectBase` kept alive by this `ObjectRef`'s reference.
        Some(unsafe { (*owner).identity })
    }

    pub fn project_field_get(&self, token: i32) -> Option<Variant> {
        if !self.is_project_instance() {
            return None;
        }
        compat_field_get(compat_owner_from_unknown(self.0.as_ptr()), token)
    }

    pub fn project_field_set(&self, token: i32, value: Variant) -> bool {
        if !self.is_project_instance() {
            return false;
        }
        compat_field_set(compat_owner_from_unknown(self.0.as_ptr()), token, value);
        true
    }

    /// Borrow instance field `token` IN PLACE (no clone) and run `f` against it
    /// (`None` if the field is absent). This is the O(1) field-element-access
    /// counterpart to [`Self::project_field_get`]: a field-held array can be indexed
    /// without cloning the whole array. Returns `None` if this is not a project
    /// instance. The closure must not call back into this object's fields (it holds a
    /// shared `RefCell` borrow for its duration).
    pub fn with_project_field<R>(
        &self,
        token: i32,
        f: impl FnOnce(Option<&Variant>) -> R,
    ) -> Option<R> {
        if !self.is_project_instance() {
            return None;
        }
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        // SAFETY: `is_project_instance()` (a vtbl-identity check) guards that `owner` is a
        // live `CompatObjectBase` kept alive by this `ObjectRef`'s retained reference; the
        // `RefCell` borrow is single-threaded (`ObjectRef` is neither `Send` nor `Sync`).
        unsafe {
            let guard = (*owner).fields.borrow();
            Some(f(guard.get(&token)))
        }
    }

    /// Mutably borrow instance field `token` IN PLACE (no clone / store-back) and run
    /// `f` against it — the write counterpart to [`Self::with_project_field`]. `None`
    /// if this is not a project instance, or if the field is absent (it must already
    /// hold the array — class init dimensions array fields before they are indexed).
    pub fn with_project_field_mut<R>(
        &self,
        token: i32,
        f: impl FnOnce(&mut Variant) -> R,
    ) -> Option<R> {
        if !self.is_project_instance() {
            return None;
        }
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        // SAFETY: as in `with_project_field` — `owner` is a live `CompatObjectBase` held
        // alive by this `ObjectRef`; the exclusive `RefCell` borrow is single-threaded.
        unsafe { (*owner).fields.borrow_mut().get_mut(&token).map(f) }
    }

    /// Run `f` against this instance's built-in native-`Collection` state, lazily creating an
    /// empty [`crate::collection::CollectionData`] on first use. Returns `None` if this
    /// `ObjectRef` is not one of our compat boxes (a foreign COM object has no native state).
    ///
    /// The state rides the object box's `native_state` slot, so `Set c2 = c1` (which clones the
    /// `ObjectRef` to the same box) shares one collection — VBA reference semantics — and box
    /// teardown reclaims it, so there is no VM-side store and no leak. The shared keyed-method
    /// logic (`oxvba_eval::collection::dispatch_collection`) runs inside `f`.
    pub fn with_native_collection<R>(
        &self,
        f: impl FnOnce(&mut crate::collection::CollectionData) -> R,
    ) -> Option<R> {
        if !self.is_compat_object() {
            return None;
        }
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        // SAFETY: guarded by `is_compat_object()` (vtbl identity) above, so `owner` is the live
        // `CompatObjectBase` kept alive by this `ObjectRef`'s retained reference; the `RefCell`
        // borrow is single-threaded (`ObjectRef` is neither `Send` nor `Sync`).
        unsafe {
            let mut guard = (*owner).native_state.borrow_mut();
            let data = guard.get_or_insert_with(crate::collection::CollectionData::default);
            Some(f(data))
        }
    }

    /// A snapshot of this instance's native `Collection` values in insertion order, or `None`
    /// if it has no native collection — a foreign COM object, or a compat object that never had
    /// one (a project instance, or a `Collection` with no items added yet). Unlike
    /// [`Self::with_native_collection`] this does NOT lazily create a collection, so it is safe
    /// to probe any object (e.g. `For Each`) without planting empty state on project instances.
    pub fn native_collection_snapshot(&self) -> Option<Vec<Variant>> {
        if !self.is_compat_object() {
            return None;
        }
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        // SAFETY: guarded by `is_compat_object()` (vtbl identity), so `owner` is the live
        // `CompatObjectBase` kept alive by this `ObjectRef`'s retained reference; the borrow is
        // single-threaded (`ObjectRef` is neither `Send` nor `Sync`).
        unsafe { (*owner).native_state.borrow().as_ref().map(|c| c.values()) }
    }

    pub fn query_interface_descriptor(
        &self,
        iid: RuntimeInterfaceId,
    ) -> Option<&'static RuntimeInterfaceDescriptor> {
        self.try_class_descriptor()?
            .interfaces
            .iter()
            .find(|descriptor| descriptor.id == iid)
    }

    pub fn query_interface_descriptor_by_guid(
        &self,
        guid: RuntimeGuid,
    ) -> Option<&'static RuntimeInterfaceDescriptor> {
        self.try_class_descriptor()?
            .interfaces
            .iter()
            .find(|descriptor| descriptor.identity.guid == guid)
    }

    pub fn query_interface_projection(
        &self,
        iid: RuntimeInterfaceId,
    ) -> Option<RuntimeInterfaceProjection> {
        let descriptor = self.query_interface_descriptor(iid)?;
        Some(RuntimeInterfaceProjection {
            object_identity: self.try_object_identity()?,
            interface_identity: descriptor.identity,
            interface_descriptor: descriptor,
        })
    }

    pub fn query_interface_projection_by_guid(
        &self,
        guid: RuntimeGuid,
    ) -> Option<RuntimeInterfaceProjection> {
        let descriptor = self.query_interface_descriptor_by_guid(guid)?;
        Some(RuntimeInterfaceProjection {
            object_identity: self.try_object_identity()?,
            interface_identity: descriptor.identity,
            interface_descriptor: descriptor,
        })
    }

    /// Construct an object reference from an owned raw `IUnknown` pointer.
    ///
    /// # Safety
    ///
    /// `raw` must either be null or point to a valid runtime `IUnknown` pointer
    /// whose ownership is transferred to the returned `ObjectRef`.
    pub unsafe fn from_raw_iunknown_owned(raw: *mut RawRuntimeIUnknown) -> Option<Self> {
        NonNull::new(raw).map(Self)
    }

    /// Construct an object reference from a borrowed raw `IUnknown` pointer.
    ///
    /// # Safety
    ///
    /// `raw` must either be null or point to a valid runtime `IUnknown` pointer.
    /// When non-null, the pointed object must support `AddRef`, and this
    /// function will retain it for the returned `ObjectRef`.
    pub unsafe fn from_raw_iunknown_addref(raw: *mut RawRuntimeIUnknown) -> Option<Self> {
        let raw = NonNull::new(raw)?;
        // SAFETY: this fn's caller contract guarantees `raw` (non-null here) points to a valid
        // IUnknown whose first field is its vtbl and whose `add_ref` may be invoked; the
        // reference taken here is owned by the returned `ObjectRef` and balanced in `Drop`.
        unsafe {
            let vtbl = (*raw.as_ptr()).vtbl;
            ((*vtbl).add_ref)(raw.as_ptr().cast());
        }
        Some(Self(raw))
    }

    pub fn strong_count_for_test(&self) -> u32 {
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        // SAFETY: documented compat-object contract as for `class_descriptor`; the box is alive
        // because this `ObjectRef` holds a retained reference, and `ref_count` is atomic.
        unsafe { (*owner).ref_count.load(Ordering::Acquire) }
    }
}

impl Clone for ObjectRef {
    fn clone(&self) -> Self {
        // SAFETY: this `ObjectRef` owns a retained reference (every constructor either starts
        // the count at 1, AddRefs, or takes ownership of an existing reference), so `self.0`
        // points at a live IUnknown; reading its vtbl and calling `add_ref` retains one more
        // reference, which the clone owns.
        unsafe {
            let vtbl = (*self.0.as_ptr()).vtbl;
            ((*vtbl).add_ref)(self.0.as_ptr().cast());
        }
        Self(self.0)
    }
}

impl Drop for ObjectRef {
    fn drop(&mut self) {
        // SAFETY: releases the one reference this `ObjectRef` owns; the object is live up to
        // this call precisely because that reference exists, and the pointer is never touched
        // afterwards even if `release` frees the box.
        unsafe {
            let vtbl = (*self.0.as_ptr()).vtbl;
            ((*vtbl).release)(self.0.as_ptr().cast());
        }
    }
}

impl core::fmt::Debug for ObjectRef {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // `compat_identity` is only meaningful on our own `CompatObjectBase` boxes; gate on the
        // vtbl-identity check so a (future) foreign IUnknown wrapper formats without an
        // out-of-bounds read past its vtbl. `is_compat_object` is safe for any IUnknown.
        let compat_identity = self.is_compat_object().then(|| self.compat_identity());
        f.debug_struct("ObjectRef")
            .field("compat_identity", &compat_identity)
            .field("ptr", &self.0)
            .finish()
    }
}

impl core::fmt::Display for ObjectRef {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Gate on `is_compat_object` (see `Debug` above): only compat boxes carry a
        // `compat_identity`; a foreign IUnknown wrapper renders as `<foreign>` rather than
        // reading uninitialized identity bytes past its vtbl.
        if self.is_compat_object() {
            write!(f, "{}", self.compat_identity())
        } else {
            write!(f, "<foreign>")
        }
    }
}

impl PartialEq for ObjectRef {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for ObjectRef {}

impl Hash for ObjectRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[cfg(test)]
// Test-support code exercising the documented production vtable paths above.
#[allow(clippy::undocumented_unsafe_blocks)]
mod tests {
    use core::ffi::c_void;
    use core::sync::atomic::{AtomicU32, Ordering};

    use super::{
        ObjectRef, RUNTIME_CLASS_LIFECYCLE_NONE, RUNTIME_E_NOINTERFACE, RUNTIME_GUID_IDISPATCH,
        RUNTIME_GUID_IUNKNOWN, RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
        RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR, RUNTIME_S_OK, RawRuntimeIUnknown,
        RawRuntimeIUnknownVtbl, RuntimeApartmentModel, RuntimeClassDescriptor,
        RuntimeDispatchPlanCache, RuntimeGuid, RuntimeInterfaceDescriptor, RuntimeInterfaceId,
        RuntimeInterfaceIdentity, RuntimeInterfaceKind, RuntimeLifetimePolicy,
        RuntimeMemberDescriptor, RuntimeMemberInvokeKind, RuntimeParamDescriptor, RuntimeValueType,
    };

    #[test]
    fn object_ref_clone_tracks_refcount_and_identity() {
        let object = ObjectRef::from_compat_identity(42);
        assert_eq!(object.compat_identity(), 42);
        assert_eq!(object.strong_count_for_test(), 1);

        let clone = object.clone();
        assert_eq!(object, clone);
        assert_eq!(object.compat_identity(), clone.compat_identity());
        assert_eq!(object.strong_count_for_test(), 2);

        drop(clone);
        assert_eq!(object.strong_count_for_test(), 1);
    }

    #[test]
    fn object_ref_query_iunknown_returns_same_identity() {
        let object = ObjectRef::from_compat_identity(77);
        let unknown = object.query_iunknown();
        assert_eq!(object, unknown);
        assert_eq!(object.compat_identity(), unknown.compat_identity());
        assert_eq!(object.strong_count_for_test(), 2);
    }

    #[test]
    fn compat_object_exposes_descriptor_backed_iunknown_interface() {
        let object = ObjectRef::from_compat_identity(9);
        let class_descriptor = object.class_descriptor();
        assert_eq!(class_descriptor.name, "OxVba.CompatObject");
        assert_eq!(class_descriptor.project_identity, None);
        assert!(!class_descriptor.predeclared);
        assert_eq!(class_descriptor.lifecycle, RUNTIME_CLASS_LIFECYCLE_NONE);
        assert!(class_descriptor.fields.is_empty());
        assert!(class_descriptor.as_new_fields.is_empty());
        assert!(class_descriptor.implements.is_empty());
        assert_eq!(class_descriptor.interfaces.len(), 1);
        let iunknown = object
            .query_interface_descriptor(RuntimeInterfaceId::IUnknown)
            .expect("compat object should expose internal IUnknown descriptor");
        assert_eq!(iunknown.name, "IUnknown");
        assert!(!iunknown.dual_dispatch);
        assert!(iunknown.members.is_empty());
        assert!(
            object
                .query_interface_descriptor(RuntimeInterfaceId::IDispatch)
                .is_none(),
            "compat object floor must not falsely claim dual dispatch support"
        );
    }

    #[test]
    fn runtime_guid_parses_and_formats_canonical_identity() {
        let parsed = RuntimeGuid::parse("{00020400-0000-0000-c000-000000000046}")
            .expect("IDispatch GUID should parse with braces and mixed case");
        assert_eq!(parsed, RUNTIME_GUID_IDISPATCH);
        assert_eq!(parsed.to_string(), "00020400-0000-0000-C000-000000000046");
        assert_eq!(
            RuntimeGuid::parse("00000000-0000-0000-C000-000000000046"),
            Some(RUNTIME_GUID_IUNKNOWN)
        );
        assert!(RuntimeGuid::parse("00020400-0000-0000-C000").is_none());
        assert!(RuntimeGuid::parse("not-a-guid").is_none());
    }

    #[test]
    fn runtime_interface_identity_carries_custom_iid_metadata() {
        const CUSTOM_GUID: RuntimeGuid = RuntimeGuid::new(
            0x1111_1111,
            0x2222,
            0x3333,
            [0x44, 0x44, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55],
        );
        const CUSTOM_IDENTITY: RuntimeInterfaceIdentity = RuntimeInterfaceIdentity::custom(
            CUSTOM_GUID,
            "Project._Widget",
            RuntimeInterfaceKind::Dual,
            Some(1),
            Some(0),
            Some(1033),
        );
        static CUSTOM_INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::Unsupported,
            identity: CUSTOM_IDENTITY,
            name: "Project._Widget",
            members: &[],
            dual_dispatch: true,
        };
        static TEST_CLASS: RuntimeClassDescriptor = RuntimeClassDescriptor {
            name: "Project.Widget",
            project_identity: None,
            predeclared: false,
            lifecycle: RUNTIME_CLASS_LIFECYCLE_NONE,
            fields: &[],
            as_new_fields: &[],
            implements: &[],
            interfaces: &[RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR, CUSTOM_INTERFACE],
        };

        let object = ObjectRef::from_compat_identity_with_descriptor(202, &TEST_CLASS);
        let descriptor = object
            .query_interface_descriptor_by_guid(CUSTOM_GUID)
            .expect("custom interface should be discoverable by GUID");
        assert_eq!(descriptor.identity.guid, CUSTOM_GUID);
        assert_eq!(descriptor.identity.name, "Project._Widget");
        assert_eq!(descriptor.identity.kind, RuntimeInterfaceKind::Dual);
        assert_eq!(descriptor.identity.major_version, Some(1));
        assert_eq!(descriptor.identity.minor_version, Some(0));
        assert_eq!(descriptor.identity.lcid, Some(1033));
    }

    #[test]
    fn native_collection_state_is_shared_across_clones() {
        use crate::Variant;
        // W1-a: a built-in Collection's CollectionData rides the object box's `native_state`
        // slot, reached via `with_native_collection`. Cloning the ObjectRef (`Set c2 = c1`)
        // points at the SAME box, so both names share one CollectionData — VBA reference
        // semantics — and box teardown reclaims it (no VM-side store, no leak).
        static TEST_CLASS: RuntimeClassDescriptor = RuntimeClassDescriptor {
            name: "VBA.Collection",
            project_identity: None,
            predeclared: false,
            lifecycle: RUNTIME_CLASS_LIFECYCLE_NONE,
            fields: &[],
            as_new_fields: &[],
            implements: &[],
            interfaces: &[RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR],
        };
        let c1 = ObjectRef::from_compat_identity_with_descriptor(404, &TEST_CLASS);
        c1.with_native_collection(|d| {
            d.add(Variant::from_i32(10), None, None, None).unwrap();
            d.add(Variant::from_i32(20), None, None, None).unwrap();
        })
        .expect("a compat object has native-collection state");
        let c2 = c1.clone();
        let (count, first) = c2
            .with_native_collection(|d| {
                (
                    d.count(),
                    d.item(&crate::collection::Selector::Index(1))
                        .ok()
                        .and_then(|v| v.as_i32()),
                )
            })
            .expect("the clone sees the same box's native state");
        assert_eq!(count, 2, "both ObjectRef clones share one CollectionData");
        assert_eq!(first, Some(10));
        drop(c2);
        drop(c1); // refcount-0 frees the box and, with it, native_state — no leak
    }

    #[test]
    fn interface_projections_share_runtime_object_identity() {
        const CUSTOM_GUID: RuntimeGuid = RuntimeGuid::new(
            0x2222_2222,
            0x3333,
            0x4444,
            [0x55, 0x55, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66],
        );
        const CUSTOM_IDENTITY: RuntimeInterfaceIdentity = RuntimeInterfaceIdentity::custom(
            CUSTOM_GUID,
            "Project._WidgetEvents",
            RuntimeInterfaceKind::Source,
            Some(1),
            Some(0),
            Some(1033),
        );
        static CUSTOM_INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::Unsupported,
            identity: CUSTOM_IDENTITY,
            name: "Project._WidgetEvents",
            members: &[],
            dual_dispatch: false,
        };
        static DISPATCH_INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
            identity: RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
            name: "Project._Widget",
            members: &[],
            dual_dispatch: true,
        };
        static TEST_CLASS: RuntimeClassDescriptor = RuntimeClassDescriptor {
            name: "Project.Widget",
            project_identity: None,
            predeclared: false,
            lifecycle: RUNTIME_CLASS_LIFECYCLE_NONE,
            fields: &[],
            as_new_fields: &[],
            implements: &[],
            interfaces: &[
                RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR,
                DISPATCH_INTERFACE,
                CUSTOM_INTERFACE,
            ],
        };

        let object = ObjectRef::from_compat_identity_with_descriptor(303, &TEST_CLASS);
        let unknown = object
            .query_interface_projection(RuntimeInterfaceId::IUnknown)
            .expect("IUnknown projection");
        let dispatch = object
            .query_interface_projection(RuntimeInterfaceId::IDispatch)
            .expect("IDispatch projection");
        let source = object
            .query_interface_projection_by_guid(CUSTOM_GUID)
            .expect("custom source projection");

        assert_eq!(unknown.object_identity, dispatch.object_identity);
        assert_eq!(dispatch.object_identity, source.object_identity);
        assert_eq!(unknown.object_identity.compat_identity, 303);
        assert_eq!(unknown.object_identity.stable_object_id, 303);
        assert_eq!(
            unknown.object_identity.lifetime_policy,
            RuntimeLifetimePolicy::RefCounted
        );
        assert_eq!(
            unknown.object_identity.apartment_model,
            RuntimeApartmentModel::Unknown
        );
        assert_eq!(unknown.interface_identity.guid, RUNTIME_GUID_IUNKNOWN);
        assert_eq!(dispatch.interface_identity.guid, RUNTIME_GUID_IDISPATCH);
        assert_eq!(source.interface_identity.guid, CUSTOM_GUID);
        assert_eq!(object.strong_count_for_test(), 1);
    }

    #[test]
    fn descriptor_backed_object_can_advertise_dual_dispatch_shape() {
        static VALUE_MEMBER: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Value",
            dispatch_id: 0,
            vtable_slot: Some(7),
            invoke_kind: RuntimeMemberInvokeKind::PropertyGet,
            arity: 0,
            params: &[],
            return_type: Some(RuntimeValueType::Variant),
            is_default_member: true,
            is_enumerator_member: false,
        };
        static DISPATCH_INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
            identity: RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
            name: "ITestDual",
            members: &[VALUE_MEMBER],
            dual_dispatch: true,
        };
        static TEST_CLASS: RuntimeClassDescriptor = RuntimeClassDescriptor {
            name: "Project.Widget",
            project_identity: None,
            predeclared: false,
            lifecycle: RUNTIME_CLASS_LIFECYCLE_NONE,
            fields: &[],
            as_new_fields: &[],
            implements: &[],
            interfaces: &[RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR, DISPATCH_INTERFACE],
        };

        let object = ObjectRef::from_compat_identity_with_descriptor(101, &TEST_CLASS);
        assert_eq!(object.class_descriptor().name, "Project.Widget");
        let dispatch = object
            .query_interface_descriptor(RuntimeInterfaceId::IDispatch)
            .expect("test class should advertise a dual dispatch descriptor");
        assert!(dispatch.dual_dispatch);
        assert_eq!(dispatch.members.len(), 1);
        assert_eq!(dispatch.members[0].name, "Value");
        assert_eq!(dispatch.members[0].vtable_slot, Some(7));
        assert!(dispatch.members[0].is_default_member);
    }

    #[test]
    fn runtime_dispatch_plan_cache_normalizes_and_reuses_member_lookup() {
        static VALUE_MEMBER: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Value",
            dispatch_id: 0,
            vtable_slot: Some(3),
            invoke_kind: RuntimeMemberInvokeKind::PropertyGet,
            arity: 0,
            params: &[],
            return_type: Some(RuntimeValueType::Variant),
            is_default_member: true,
            is_enumerator_member: false,
        };
        static SET_VALUE_MEMBER: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Value",
            dispatch_id: 0,
            vtable_slot: Some(4),
            invoke_kind: RuntimeMemberInvokeKind::PropertyLet,
            arity: 1,
            params: &[RuntimeParamDescriptor {
                name: "value",
                value_type: RuntimeValueType::Variant,
                by_ref: false,
                optional: false,
                param_array: false,
            }],
            return_type: None,
            is_default_member: true,
            is_enumerator_member: false,
        };
        static INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
            identity: RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
            name: "ITestDual",
            members: &[VALUE_MEMBER, SET_VALUE_MEMBER],
            dual_dispatch: true,
        };

        let mut cache = RuntimeDispatchPlanCache::new();
        let first = cache
            .resolve_member(
                &INTERFACE,
                " VALUE ",
                RuntimeMemberInvokeKind::PropertyGet,
                0,
            )
            .expect("property get should resolve");
        assert_eq!(first.dispatch_id, 0);
        assert_eq!(first.vtable_slot, Some(3));
        assert_eq!(first.member_index, 0);
        assert!(first.is_default_member);
        assert_eq!(cache.len(), 1);

        let second = cache
            .resolve_member(&INTERFACE, "value", RuntimeMemberInvokeKind::PropertyGet, 0)
            .expect("cached property get should resolve case-insensitively");
        assert_eq!(first, second);
        assert_eq!(cache.len(), 1, "second lookup should reuse the cached plan");

        let put = cache
            .resolve_member(&INTERFACE, "value", RuntimeMemberInvokeKind::PropertyLet, 1)
            .expect("property let should use a distinct call-kind/arity plan");
        assert_eq!(put.vtable_slot, Some(4));
        assert_eq!(put.member_index, 1);
        assert_eq!(cache.len(), 2);
        assert!(
            cache
                .resolve_member(&INTERFACE, "value", RuntimeMemberInvokeKind::PropertyGet, 1)
                .is_none(),
            "arity participates in descriptor plan resolution"
        );

        let default_get = cache
            .resolve_default_member(&INTERFACE, RuntimeMemberInvokeKind::PropertyGet, 0)
            .expect("default property get should resolve through the descriptor cache");
        assert_eq!(default_get.member_index, 0);
        assert_eq!(cache.len(), 3);
        let default_get_again = cache
            .resolve_default_member(&INTERFACE, RuntimeMemberInvokeKind::PropertyGet, 0)
            .expect("cached default property get should resolve");
        assert_eq!(default_get, default_get_again);
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn runtime_dispatch_plan_cache_is_scoped_to_descriptor_identity() {
        static FIRST_VALUE_MEMBER: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Value",
            dispatch_id: 10,
            vtable_slot: Some(3),
            invoke_kind: RuntimeMemberInvokeKind::PropertyGet,
            arity: 0,
            params: &[],
            return_type: Some(RuntimeValueType::Variant),
            is_default_member: true,
            is_enumerator_member: false,
        };
        static SECOND_VALUE_MEMBER: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Value",
            dispatch_id: 20,
            vtable_slot: Some(9),
            invoke_kind: RuntimeMemberInvokeKind::PropertyGet,
            arity: 0,
            params: &[],
            return_type: Some(RuntimeValueType::Variant),
            is_default_member: true,
            is_enumerator_member: false,
        };
        static FIRST_INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
            identity: RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
            name: "IFirst",
            members: &[FIRST_VALUE_MEMBER],
            dual_dispatch: true,
        };
        static SECOND_INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
            identity: RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
            name: "ISecond",
            members: &[SECOND_VALUE_MEMBER],
            dual_dispatch: true,
        };

        let mut cache = RuntimeDispatchPlanCache::new();
        let first = cache
            .resolve_member(
                &FIRST_INTERFACE,
                "Value",
                RuntimeMemberInvokeKind::PropertyGet,
                0,
            )
            .expect("first interface member should resolve");
        let second = cache
            .resolve_member(
                &SECOND_INTERFACE,
                "Value",
                RuntimeMemberInvokeKind::PropertyGet,
                0,
            )
            .expect("second interface member should resolve independently");

        assert_eq!(first.dispatch_id, 10);
        assert_eq!(first.vtable_slot, Some(3));
        assert_eq!(second.dispatch_id, 20);
        assert_eq!(second.vtable_slot, Some(9));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn runtime_dispatch_plan_cache_caches_unhinted_unique_member_lookup() {
        static VALUE_MEMBER: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Value",
            dispatch_id: 0,
            vtable_slot: Some(3),
            invoke_kind: RuntimeMemberInvokeKind::PropertyGet,
            arity: 0,
            params: &[],
            return_type: Some(RuntimeValueType::Variant),
            is_default_member: true,
            is_enumerator_member: false,
        };
        static PUT_VALUE_MEMBER: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Value",
            dispatch_id: 0,
            vtable_slot: Some(4),
            invoke_kind: RuntimeMemberInvokeKind::PropertyLet,
            arity: 1,
            params: &[RuntimeParamDescriptor {
                name: "value",
                value_type: RuntimeValueType::Variant,
                by_ref: false,
                optional: false,
                param_array: false,
            }],
            return_type: None,
            is_default_member: true,
            is_enumerator_member: false,
        };
        static INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
            identity: RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
            name: "ITestDual",
            members: &[VALUE_MEMBER, PUT_VALUE_MEMBER],
            dual_dispatch: true,
        };

        let mut cache = RuntimeDispatchPlanCache::new();
        let get = cache
            .resolve_member_unhinted(&INTERFACE, " value ", 0)
            .expect("unique unhinted get should resolve and cache");
        assert_eq!(get.member_index, 0);
        assert_eq!(get.invoke_kind, RuntimeMemberInvokeKind::PropertyGet);
        assert_eq!(cache.len(), 1);
        let default_get = cache
            .resolve_default_member_unhinted(&INTERFACE, 0)
            .expect("unique unhinted default get should resolve and cache separately");
        assert_eq!(default_get.member_index, 0);
        assert_eq!(cache.len(), 2);
        assert!(
            cache
                .resolve_member_unhinted(&INTERFACE, "Value", 1)
                .is_some(),
            "different arity can resolve to the put member"
        );
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn runtime_dispatch_plan_cache_rejects_unhinted_ambiguous_member_lookup() {
        static GET_VALUE: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Value",
            dispatch_id: 0,
            vtable_slot: Some(3),
            invoke_kind: RuntimeMemberInvokeKind::PropertyGet,
            arity: 0,
            params: &[],
            return_type: Some(RuntimeValueType::Variant),
            is_default_member: true,
            is_enumerator_member: false,
        };
        static METHOD_VALUE: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Value",
            dispatch_id: 1,
            vtable_slot: Some(4),
            invoke_kind: RuntimeMemberInvokeKind::Method,
            arity: 0,
            params: &[],
            return_type: Some(RuntimeValueType::Variant),
            is_default_member: false,
            is_enumerator_member: false,
        };
        static INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
            identity: RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
            name: "IAmbiguous",
            members: &[GET_VALUE, METHOD_VALUE],
            dual_dispatch: true,
        };

        let mut cache = RuntimeDispatchPlanCache::new();
        assert!(
            cache
                .resolve_member_unhinted(&INTERFACE, "Value", 0)
                .is_none()
        );
        assert!(
            cache
                .resolve_default_member_unhinted(&INTERFACE, 0)
                .is_some()
        );
        assert_eq!(
            cache.len(),
            1,
            "only the unambiguous default lookup should cache"
        );
    }

    #[test]
    fn runtime_dispatch_plan_cache_rejects_ambiguous_default_member() {
        static FIRST: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Value",
            dispatch_id: 0,
            vtable_slot: Some(3),
            invoke_kind: RuntimeMemberInvokeKind::PropertyGet,
            arity: 0,
            params: &[],
            return_type: Some(RuntimeValueType::Variant),
            is_default_member: true,
            is_enumerator_member: false,
        };
        static SECOND: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Item",
            dispatch_id: 0,
            vtable_slot: Some(4),
            invoke_kind: RuntimeMemberInvokeKind::PropertyGet,
            arity: 0,
            params: &[],
            return_type: Some(RuntimeValueType::Variant),
            is_default_member: true,
            is_enumerator_member: false,
        };
        static INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
            identity: RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
            name: "IAmbiguousDefault",
            members: &[FIRST, SECOND],
            dual_dispatch: true,
        };

        let mut cache = RuntimeDispatchPlanCache::new();
        assert!(
            cache
                .resolve_default_member(&INTERFACE, RuntimeMemberInvokeKind::PropertyGet, 0)
                .is_none(),
            "ambiguous default member metadata must not be cached as a single plan"
        );
        assert!(cache.is_empty());
    }

    #[test]
    fn descriptor_backed_object_supports_raw_query_interface_projection() {
        const CUSTOM_GUID: RuntimeGuid = RuntimeGuid::new(
            0x3333_3333,
            0x4444,
            0x5555,
            [0x66, 0x66, 0x77, 0x77, 0x77, 0x77, 0x77, 0x77],
        );
        const CUSTOM_IDENTITY: RuntimeInterfaceIdentity = RuntimeInterfaceIdentity::custom(
            CUSTOM_GUID,
            "Project._WidgetCustom",
            RuntimeInterfaceKind::Dual,
            Some(1),
            Some(0),
            Some(1033),
        );
        static VALUE_MEMBER: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Value",
            dispatch_id: 0,
            vtable_slot: Some(7),
            invoke_kind: RuntimeMemberInvokeKind::PropertyGet,
            arity: 0,
            params: &[],
            return_type: Some(RuntimeValueType::Variant),
            is_default_member: true,
            is_enumerator_member: false,
        };
        static DISPATCH_INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
            identity: RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
            name: "ITestDual",
            members: &[VALUE_MEMBER],
            dual_dispatch: true,
        };
        static CUSTOM_INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::Unsupported,
            identity: CUSTOM_IDENTITY,
            name: "Project._WidgetCustom",
            members: &[VALUE_MEMBER],
            dual_dispatch: true,
        };
        static TEST_CLASS: RuntimeClassDescriptor = RuntimeClassDescriptor {
            name: "Project.Widget",
            project_identity: None,
            predeclared: false,
            lifecycle: RUNTIME_CLASS_LIFECYCLE_NONE,
            fields: &[],
            as_new_fields: &[],
            implements: &[],
            interfaces: &[
                RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR,
                DISPATCH_INTERFACE,
                CUSTOM_INTERFACE,
            ],
        };

        let object = ObjectRef::from_compat_identity_with_descriptor(12, &TEST_CLASS);
        let mut dispatch_out = core::ptr::null_mut();
        let dispatch_hr = unsafe {
            ((*(*object.raw_iunknown()).vtbl).query_interface)(
                object.raw_iunknown().cast(),
                RUNTIME_GUID_IDISPATCH,
                &mut dispatch_out,
            )
        };
        assert_eq!(dispatch_hr, RUNTIME_S_OK);
        assert_eq!(dispatch_out, object.raw_iunknown().cast());
        assert_eq!(object.strong_count_for_test(), 2);

        let mut custom_out = core::ptr::null_mut();
        let custom_hr = unsafe {
            ((*(*object.raw_iunknown()).vtbl).query_interface)(
                object.raw_iunknown().cast(),
                CUSTOM_GUID,
                &mut custom_out,
            )
        };
        assert_eq!(custom_hr, RUNTIME_S_OK);
        assert_eq!(custom_out, object.raw_iunknown().cast());
        assert_eq!(object.strong_count_for_test(), 3);

        unsafe {
            ((*(*object.raw_iunknown()).vtbl).release)(dispatch_out);
            ((*(*object.raw_iunknown()).vtbl).release)(custom_out);
        }
        assert_eq!(object.strong_count_for_test(), 1);
    }

    #[test]
    fn object_ref_only_supports_iunknown_query() {
        let object = ObjectRef::from_compat_identity(9);
        let mut out = core::ptr::null_mut();
        let vtbl = unsafe { (*object.raw_iunknown()).vtbl };
        let hr = unsafe {
            ((*vtbl).query_interface)(
                object.raw_iunknown().cast(),
                RUNTIME_GUID_IUNKNOWN,
                &mut out,
            )
        };
        assert_eq!(hr, 0);
        assert_eq!(out, object.raw_iunknown().cast());
        unsafe {
            ((*vtbl).release)(out);
        }

        let hr = unsafe {
            ((*vtbl).query_interface)(
                object.raw_iunknown().cast(),
                RuntimeGuid::new(
                    0xFFFF_FFFF,
                    0xFFFF,
                    0xFFFF,
                    [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                ),
                &mut out,
            )
        };
        assert_eq!(hr, RUNTIME_E_NOINTERFACE);
        assert!(out.is_null());
        assert_eq!(object.strong_count_for_test(), 1);
    }

    #[repr(C)]
    struct FakeForeignObject {
        unknown: RawRuntimeIUnknown,
        ref_count: AtomicU32,
    }

    static FAKE_FOREIGN_VTBL: RawRuntimeIUnknownVtbl = RawRuntimeIUnknownVtbl {
        query_interface: fake_foreign_query_interface,
        add_ref: fake_foreign_add_ref,
        release: fake_foreign_release,
    };

    unsafe extern "C" fn fake_foreign_query_interface(
        _this: *mut c_void,
        _iid: RuntimeGuid,
        ppv: *mut *mut c_void,
    ) -> i32 {
        if !ppv.is_null() {
            unsafe {
                *ppv = core::ptr::null_mut();
            }
        }
        RUNTIME_E_NOINTERFACE
    }

    unsafe extern "C" fn fake_foreign_add_ref(this: *mut c_void) -> u32 {
        let object = this.cast::<FakeForeignObject>();
        unsafe { (*object).ref_count.fetch_add(1, Ordering::AcqRel) + 1 }
    }

    unsafe extern "C" fn fake_foreign_release(this: *mut c_void) -> u32 {
        let object = this.cast::<FakeForeignObject>();
        let previous = unsafe { (*object).ref_count.fetch_sub(1, Ordering::AcqRel) };
        let remaining = previous.saturating_sub(1);
        if remaining == 0 {
            unsafe {
                drop(Box::from_raw(object));
            }
        }
        remaining
    }

    fn fake_foreign_object() -> ObjectRef {
        let boxed = Box::new(FakeForeignObject {
            unknown: RawRuntimeIUnknown {
                vtbl: &FAKE_FOREIGN_VTBL,
            },
            ref_count: AtomicU32::new(1),
        });
        let raw = Box::into_raw(boxed);
        unsafe {
            ObjectRef::from_raw_iunknown_owned(&mut (*raw).unknown as *mut RawRuntimeIUnknown)
                .expect("fake foreign object pointer is non-null")
        }
    }

    #[test]
    fn foreign_iunknown_has_no_runtime_descriptor_projection() {
        let object = fake_foreign_object();
        assert!(!object.is_compat_object());
        assert!(object.try_class_descriptor().is_none());
        assert!(object.try_object_identity().is_none());
        assert!(
            object
                .query_interface_descriptor(RuntimeInterfaceId::IUnknown)
                .is_none()
        );
        assert!(
            object
                .query_interface_descriptor_by_guid(RUNTIME_GUID_IUNKNOWN)
                .is_none()
        );
        assert!(
            object
                .query_interface_projection(RuntimeInterfaceId::IUnknown)
                .is_none()
        );
        assert!(
            object
                .query_interface_projection_by_guid(RUNTIME_GUID_IUNKNOWN)
                .is_none()
        );
        assert_eq!(object.to_string(), "<foreign>");
    }
}
