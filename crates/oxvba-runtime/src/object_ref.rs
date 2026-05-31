use core::ffi::c_void;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU32, Ordering};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeInterfaceDescriptor {
    pub id: RuntimeInterfaceId,
    pub identity: RuntimeInterfaceIdentity,
    pub name: &'static str,
    pub members: &'static [RuntimeMemberDescriptor],
    pub dual_dispatch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeClassDescriptor {
    pub name: &'static str,
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
    interfaces: &[RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR],
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeDispatchCacheKey {
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
    unsafe {
        *ppv = core::ptr::null_mut();
    }
    let owner = compat_owner_from_this(this);
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
    unsafe {
        *ppv = this;
    }
    unsafe { compat_add_ref(this) };
    RUNTIME_S_OK
}

unsafe extern "C" fn compat_add_ref(this: *mut c_void) -> u32 {
    let owner = compat_owner_from_this(this);
    unsafe { (*owner).ref_count.fetch_add(1, Ordering::AcqRel) + 1 }
}

unsafe extern "C" fn compat_release(this: *mut c_void) -> u32 {
    let owner = compat_owner_from_this(this);
    let previous = unsafe { (*owner).ref_count.fetch_sub(1, Ordering::AcqRel) };
    let remaining = previous.saturating_sub(1);
    if remaining == 0 {
        unsafe {
            drop(Box::from_raw(owner));
        }
    }
    remaining
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
        // Legacy/template path: identity key and route key coincide.
        Self::from_compat_object(compat_identity, compat_identity, class_descriptor)
    }

    /// Allocates a fresh project-class instance: a distinct `CompatObjectBase` (a distinct
    /// IUnknown, hence a distinct identity for `Is`) with its own per-instance `instance_id`
    /// and the class's `route_key` for member dispatch. Refcount starts at 1; the instance is
    /// not pinned by any route map, so its lifetime tracks real references.
    pub fn from_project_instance(
        instance_id: i32,
        route_key: i32,
        class_descriptor: &'static RuntimeClassDescriptor,
    ) -> Self {
        Self::from_compat_object(instance_id, route_key, class_descriptor)
    }

    fn from_compat_object(
        compat_identity: i32,
        route_key: i32,
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
                class_descriptor,
                lifetime_policy: RuntimeLifetimePolicy::RefCounted,
                apartment_model: RuntimeApartmentModel::Unknown,
            },
            class_descriptor,
        });
        let raw = Box::into_raw(boxed);
        let unknown = unsafe { &mut (*raw).unknown as *mut RawRuntimeIUnknown };
        Self(NonNull::new(unknown).expect("compat object unknown pointer must be non-null"))
    }

    pub fn query_iunknown(&self) -> Self {
        self.clone()
    }

    pub fn compat_identity(&self) -> i32 {
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        unsafe { (*owner).identity.compat_identity }
    }

    pub fn raw(&self) -> i32 {
        self.compat_identity()
    }

    /// Class/route key for project-dynamic dispatch (which class's route this instance uses).
    /// Only valid for compat (project-class) objects, like `compat_identity`/`class_descriptor`.
    pub fn route_key(&self) -> i32 {
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        unsafe { (*owner).identity.route_key }
    }

    pub fn raw_iunknown(&self) -> *mut RawRuntimeIUnknown {
        self.0.as_ptr()
    }

    pub fn class_descriptor(&self) -> &'static RuntimeClassDescriptor {
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        unsafe { (*owner).class_descriptor }
    }

    pub fn object_identity(&self) -> RuntimeObjectIdentity {
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        unsafe { (*owner).identity }
    }

    pub fn query_interface_descriptor(
        &self,
        iid: RuntimeInterfaceId,
    ) -> Option<&'static RuntimeInterfaceDescriptor> {
        self.class_descriptor()
            .interfaces
            .iter()
            .find(|descriptor| descriptor.id == iid)
    }

    pub fn query_interface_descriptor_by_guid(
        &self,
        guid: RuntimeGuid,
    ) -> Option<&'static RuntimeInterfaceDescriptor> {
        self.class_descriptor()
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
            object_identity: self.object_identity(),
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
            object_identity: self.object_identity(),
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
        unsafe {
            let vtbl = (*raw.as_ptr()).vtbl;
            ((*vtbl).add_ref)(raw.as_ptr().cast());
        }
        Some(Self(raw))
    }

    pub fn strong_count_for_test(&self) -> u32 {
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        unsafe { (*owner).ref_count.load(Ordering::Acquire) }
    }
}

impl Clone for ObjectRef {
    fn clone(&self) -> Self {
        unsafe {
            let vtbl = (*self.0.as_ptr()).vtbl;
            ((*vtbl).add_ref)(self.0.as_ptr().cast());
        }
        Self(self.0)
    }
}

impl Drop for ObjectRef {
    fn drop(&mut self) {
        unsafe {
            let vtbl = (*self.0.as_ptr()).vtbl;
            ((*vtbl).release)(self.0.as_ptr().cast());
        }
    }
}

impl core::fmt::Debug for ObjectRef {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ObjectRef")
            .field("compat_identity", &self.compat_identity())
            .field("ptr", &self.0)
            .finish()
    }
}

impl core::fmt::Display for ObjectRef {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.compat_identity())
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

unsafe impl Send for ObjectRef {}
unsafe impl Sync for ObjectRef {}

#[cfg(test)]
mod tests {
    use super::{
        ObjectRef, RUNTIME_E_NOINTERFACE, RUNTIME_GUID_IDISPATCH, RUNTIME_GUID_IUNKNOWN,
        RUNTIME_IDISPATCH_INTERFACE_IDENTITY, RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR, RUNTIME_S_OK,
        RuntimeApartmentModel, RuntimeClassDescriptor, RuntimeDispatchPlanCache, RuntimeGuid,
        RuntimeInterfaceDescriptor, RuntimeInterfaceId, RuntimeInterfaceIdentity,
        RuntimeInterfaceKind, RuntimeLifetimePolicy, RuntimeMemberDescriptor,
        RuntimeMemberInvokeKind, RuntimeParamDescriptor, RuntimeValueType,
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
}
