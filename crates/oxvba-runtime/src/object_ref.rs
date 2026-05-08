use core::ffi::c_void;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU32, Ordering};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

pub const RUNTIME_S_OK: i32 = 0;
pub const RUNTIME_E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeInterfaceId {
    IUnknown,
    IDispatch,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeMemberInvokeKind {
    Method,
    PropertyGet,
    PropertyLet,
    PropertySet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeMemberDescriptor {
    pub name: &'static str,
    pub dispatch_id: i32,
    pub vtable_slot: Option<u16>,
    pub invoke_kind: RuntimeMemberInvokeKind,
    pub is_default_member: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeInterfaceDescriptor {
    pub id: RuntimeInterfaceId,
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
        if let Some(plan) = self.entries.get(&key).copied() {
            return Some(plan);
        }
        let (member_index, member) =
            interface
                .members
                .iter()
                .enumerate()
                .find(|(_, candidate)| {
                    normalize_runtime_member_name(candidate.name) == key.normalized_member_name
                        && candidate.invoke_kind == invoke_kind
                })?;
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
    pub query_interface: unsafe extern "C" fn(
        this: *mut c_void,
        iid: RuntimeInterfaceId,
        ppv: *mut *mut c_void,
    ) -> i32,
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
    compat_identity: i32,
    class_descriptor: &'static RuntimeClassDescriptor,
}

static COMPAT_OBJECT_VTBL: RawRuntimeIUnknownVtbl = RawRuntimeIUnknownVtbl {
    query_interface: compat_query_interface,
    add_ref: compat_add_ref,
    release: compat_release,
};

unsafe extern "C" fn compat_query_interface(
    this: *mut c_void,
    iid: RuntimeInterfaceId,
    ppv: *mut *mut c_void,
) -> i32 {
    if ppv.is_null() {
        return RUNTIME_E_NOINTERFACE;
    }
    unsafe {
        *ppv = core::ptr::null_mut();
    }
    if iid != RuntimeInterfaceId::IUnknown {
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
        let boxed = Box::new(CompatObjectBase {
            unknown: RawRuntimeIUnknown {
                vtbl: &COMPAT_OBJECT_VTBL,
            },
            ref_count: AtomicU32::new(1),
            compat_identity,
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
        unsafe { (*owner).compat_identity }
    }

    pub fn raw(&self) -> i32 {
        self.compat_identity()
    }

    pub fn raw_iunknown(&self) -> *mut RawRuntimeIUnknown {
        self.0.as_ptr()
    }

    pub fn class_descriptor(&self) -> &'static RuntimeClassDescriptor {
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        unsafe { (*owner).class_descriptor }
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
        ObjectRef, RUNTIME_E_NOINTERFACE, RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR,
        RuntimeClassDescriptor, RuntimeDispatchPlanCache, RuntimeInterfaceDescriptor,
        RuntimeInterfaceId, RuntimeMemberDescriptor, RuntimeMemberInvokeKind,
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
    fn descriptor_backed_object_can_advertise_dual_dispatch_shape() {
        static VALUE_MEMBER: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Value",
            dispatch_id: 0,
            vtable_slot: Some(7),
            invoke_kind: RuntimeMemberInvokeKind::PropertyGet,
            is_default_member: true,
        };
        static DISPATCH_INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
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
            is_default_member: true,
        };
        static SET_VALUE_MEMBER: RuntimeMemberDescriptor = RuntimeMemberDescriptor {
            name: "Value",
            dispatch_id: 0,
            vtable_slot: Some(4),
            invoke_kind: RuntimeMemberInvokeKind::PropertyLet,
            is_default_member: true,
        };
        static INTERFACE: RuntimeInterfaceDescriptor = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
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
    }

    #[test]
    fn object_ref_only_supports_iunknown_query() {
        let object = ObjectRef::from_compat_identity(9);
        let mut out = core::ptr::null_mut();
        let vtbl = unsafe { (*object.raw_iunknown()).vtbl };
        let hr = unsafe {
            ((*vtbl).query_interface)(
                object.raw_iunknown().cast(),
                RuntimeInterfaceId::IUnknown,
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
                RuntimeInterfaceId::Unsupported,
                &mut out,
            )
        };
        assert_eq!(hr, RUNTIME_E_NOINTERFACE);
    }
}
