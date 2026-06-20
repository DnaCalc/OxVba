use core::ffi::c_void;
use std::rc::Rc;

pub type ComRecordCloneFn =
    unsafe fn(
        record_info: *mut c_void,
        record_data: *const c_void,
    ) -> Result<(*mut c_void, *mut c_void), String>;
pub type ComRecordDestroyFn = unsafe fn(record_info: *mut c_void, record_data: *mut c_void);

struct ComRecordInner {
    record_info: *mut c_void,
    record_data: *mut c_void,
    clone_record: ComRecordCloneFn,
    destroy_record: ComRecordDestroyFn,
}

impl Drop for ComRecordInner {
    fn drop(&mut self) {
        if !self.record_data.is_null() {
            // SAFETY: `ComRecord` is constructed only from a live COM record payload and
            // its matching record-info owner. The paired destroy callback is the same
            // ownership domain that produced the payload.
            unsafe {
                (self.destroy_record)(self.record_info, self.record_data);
            }
        }
    }
}

#[derive(Clone)]
pub struct ComRecord {
    inner: Rc<ComRecordInner>,
}

impl ComRecord {
    /// Adopts a COM record payload and its matching record-info owner.
    ///
    /// # Safety
    /// `record_data` must be an owned record allocation compatible with `record_info`.
    /// `destroy_record` must release exactly that allocation, and `clone_record` must
    /// produce a distinct owned copy compatible with the same `record_info`.
    pub unsafe fn from_raw_parts(
        record_info: *mut c_void,
        record_data: *mut c_void,
        clone_record: ComRecordCloneFn,
        destroy_record: ComRecordDestroyFn,
    ) -> Result<Self, String> {
        if record_info.is_null() {
            return Err("COM record carried null IRecordInfo".to_string());
        }
        if record_data.is_null() {
            return Err("COM record carried null record data".to_string());
        }
        Ok(Self {
            inner: Rc::new(ComRecordInner {
                record_info,
                record_data,
                clone_record,
                destroy_record,
            }),
        })
    }

    pub fn record_info_ptr(&self) -> *mut c_void {
        self.inner.record_info
    }

    pub fn record_data_ptr(&self) -> *mut c_void {
        self.inner.record_data
    }

    pub fn deep_clone(&self) -> Result<Self, String> {
        // SAFETY: `self.inner` owns a live record payload and matching record-info
        // callbacks for the lifetime of the Arc.
        let (record_info, record_data) = unsafe {
            (self.inner.clone_record)(self.inner.record_info, self.inner.record_data.cast_const())?
        };
        // SAFETY: the clone callback returned a fresh owned payload compatible with
        // the returned record-info owner and destroy callback.
        unsafe {
            Self::from_raw_parts(
                record_info,
                record_data,
                self.inner.clone_record,
                self.inner.destroy_record,
            )
        }
    }

    pub fn into_raw_parts(mut self) -> Result<(*mut c_void, *mut c_void), String> {
        let Some(inner) = Rc::get_mut(&mut self.inner) else {
            return Err("cannot transfer shared COM record handle".to_string());
        };
        let record_info = inner.record_info;
        let record_data = inner.record_data;
        inner.record_info = core::ptr::null_mut();
        inner.record_data = core::ptr::null_mut();
        Ok((record_info, record_data))
    }
}

impl core::fmt::Debug for ComRecord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ComRecord")
            .field("record_info", &self.record_info_ptr())
            .field("record_data", &self.record_data_ptr())
            .finish()
    }
}

impl PartialEq for ComRecord {
    fn eq(&self, other: &Self) -> bool {
        self.record_info_ptr() == other.record_info_ptr()
            && self.record_data_ptr() == other.record_data_ptr()
    }
}

impl Eq for ComRecord {}
