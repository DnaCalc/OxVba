use core::ffi::c_void;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU32, Ordering};
use std::hash::{Hash, Hasher};

use crate::runtime_value::ObjectHandle;

pub const RUNTIME_S_OK: i32 = 0;
pub const RUNTIME_E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeInterfaceId {
    IUnknown,
    Unsupported,
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
        let boxed = Box::new(CompatObjectBase {
            unknown: RawRuntimeIUnknown {
                vtbl: &COMPAT_OBJECT_VTBL,
            },
            ref_count: AtomicU32::new(1),
            compat_identity,
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

    pub fn strong_count_for_test(&self) -> u32 {
        let owner = compat_owner_from_unknown(self.0.as_ptr());
        unsafe { (*owner).ref_count.load(Ordering::Acquire) }
    }
}

impl From<ObjectHandle> for ObjectRef {
    fn from(value: ObjectHandle) -> Self {
        Self::from_compat_identity(value.raw())
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
    use super::{ObjectRef, RUNTIME_E_NOINTERFACE, RuntimeInterfaceId};

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
