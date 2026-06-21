#![allow(unsafe_op_in_unsafe_fn)]

use crate::{
    COM_DISP_E_MEMBERNOTFOUND, COM_DISP_E_UNKNOWNNAME, COM_E_INVALIDARG, COM_E_NOINTERFACE,
    COM_E_NOTIMPL, COM_S_OK, IID_IDISPATCH, IID_IUNKNOWN, RawIDispatch, RawIDispatchVtbl,
    RawIUnknownVtbl, guid_equals,
};
use oxvba_runtime::ObjectRef;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use windows_sys::Win32::System::Com::{DISPPARAMS, EXCEPINFO};
use windows_sys::Win32::System::Variant::VARIANT;

#[repr(C)]
struct RuntimeObjectDispatch {
    dispatch: RawIDispatch,
    ref_count: AtomicU32,
    object: ObjectRef,
}

static RUNTIME_OBJECT_DISPATCH_VTBL: RawIDispatchVtbl = RawIDispatchVtbl {
    unknown: RawIUnknownVtbl {
        query_interface: runtime_object_query_interface,
        add_ref: runtime_object_add_ref,
        release: runtime_object_release,
    },
    get_type_info_count: runtime_object_get_type_info_count,
    get_type_info: runtime_object_get_type_info,
    get_ids_of_names: runtime_object_get_ids_of_names,
    invoke: runtime_object_invoke,
};

pub fn create_runtime_object_dispatch(object: ObjectRef) -> *mut RawIDispatch {
    let wrapper = Box::new(RuntimeObjectDispatch {
        dispatch: RawIDispatch {
            vtbl: &RUNTIME_OBJECT_DISPATCH_VTBL,
        },
        ref_count: AtomicU32::new(1),
        object,
    });
    Box::into_raw(wrapper).cast::<RawIDispatch>()
}

/// Returns the OxVBA runtime object carried by a bridge-owned `IDispatch` wrapper.
///
/// # Safety
/// `dispatch` must be null or a valid live `IDispatch` pointer whose vtable may
/// be read. The returned `ObjectRef` owns a fresh retained reference.
pub unsafe fn runtime_object_from_dispatch(dispatch: *mut RawIDispatch) -> Option<ObjectRef> {
    if dispatch.is_null() {
        return None;
    }
    if !std::ptr::eq((*dispatch).vtbl, &RUNTIME_OBJECT_DISPATCH_VTBL) {
        return None;
    }
    let wrapper = dispatch.cast::<RuntimeObjectDispatch>();
    Some((*wrapper).object.clone())
}

unsafe fn wrapper_from_this<'a>(this: *mut c_void) -> Option<&'a RuntimeObjectDispatch> {
    if this.is_null() {
        return None;
    }
    Some(&*this.cast::<RuntimeObjectDispatch>())
}

unsafe extern "system" fn runtime_object_query_interface(
    this: *mut c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut c_void,
) -> i32 {
    if this.is_null() || riid.is_null() || ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    if guid_equals(riid, &IID_IUNKNOWN) || guid_equals(riid, &IID_IDISPATCH) {
        *ppv = this;
        runtime_object_add_ref(this);
        return COM_S_OK;
    }
    *ppv = std::ptr::null_mut();
    COM_E_NOINTERFACE
}

unsafe extern "system" fn runtime_object_add_ref(this: *mut c_void) -> u32 {
    let Some(wrapper) = wrapper_from_this(this) else {
        return 0;
    };
    wrapper.ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

unsafe extern "system" fn runtime_object_release(this: *mut c_void) -> u32 {
    let Some(wrapper) = wrapper_from_this(this) else {
        return 0;
    };
    let previous = wrapper.ref_count.fetch_sub(1, Ordering::AcqRel);
    let remaining = previous.saturating_sub(1);
    if previous == 1 {
        std::sync::atomic::fence(Ordering::Acquire);
        drop(Box::from_raw(this.cast::<RuntimeObjectDispatch>()));
        return 0;
    }
    remaining
}

unsafe extern "system" fn runtime_object_get_type_info_count(
    _this: *mut c_void,
    pctinfo: *mut u32,
) -> i32 {
    if pctinfo.is_null() {
        return COM_E_INVALIDARG;
    }
    *pctinfo = 0;
    COM_S_OK
}

unsafe extern "system" fn runtime_object_get_type_info(
    _this: *mut c_void,
    _itinfo: u32,
    _lcid: u32,
    pptinfo: *mut *mut c_void,
) -> i32 {
    if !pptinfo.is_null() {
        *pptinfo = std::ptr::null_mut();
    }
    COM_E_NOTIMPL
}

unsafe extern "system" fn runtime_object_get_ids_of_names(
    _this: *mut c_void,
    _riid: *const windows_sys::core::GUID,
    _rgsznames: *mut *mut u16,
    _cnames: u32,
    _lcid: u32,
    rgdispid: *mut i32,
) -> i32 {
    if !rgdispid.is_null() {
        *rgdispid = -1;
    }
    COM_DISP_E_UNKNOWNNAME
}

unsafe extern "system" fn runtime_object_invoke(
    _this: *mut c_void,
    _dispidmember: i32,
    _riid: *const windows_sys::core::GUID,
    _lcid: u32,
    _wflags: u16,
    _pparams: *mut DISPPARAMS,
    _pvarresult: *mut VARIANT,
    _pexcepinfo: *mut EXCEPINFO,
    _puargerr: *mut u32,
) -> i32 {
    COM_DISP_E_MEMBERNOTFOUND
}

#[cfg(test)]
mod tests {
    use super::{create_runtime_object_dispatch, runtime_object_from_dispatch};
    use crate::{IID_IDISPATCH, release_dispatch};
    use oxvba_runtime::ObjectRef;
    use std::ffi::c_void;

    #[test]
    fn runtime_object_dispatch_roundtrips_object_identity() {
        let object = ObjectRef::from_compat_identity(1234);
        let dispatch = create_runtime_object_dispatch(object.clone());

        let roundtrip = unsafe { runtime_object_from_dispatch(dispatch) }
            .expect("wrapper should expose carried object");
        assert_eq!(roundtrip.raw(), object.raw());

        let mut queried: *mut c_void = std::ptr::null_mut();
        let hr = unsafe {
            ((*(*dispatch).vtbl).unknown.query_interface)(
                dispatch.cast(),
                &IID_IDISPATCH,
                &mut queried,
            )
        };
        assert_eq!(hr, 0);
        assert_eq!(queried, dispatch.cast::<c_void>());

        unsafe {
            release_dispatch(queried.cast());
            release_dispatch(dispatch);
        }
    }
}
