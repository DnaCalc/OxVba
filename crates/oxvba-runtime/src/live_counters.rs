#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LiveHandleCounts {
    pub bstrs: isize,
    pub object_boxes: isize,
    pub safearrays: isize,
    pub record_buffers: isize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HandleBalance {
    pub bstrs: isize,
    pub object_boxes: isize,
    pub safearrays: isize,
    pub record_buffers: isize,
}

impl LiveHandleCounts {
    pub fn balance_to(self, after: Self) -> HandleBalance {
        HandleBalance {
            bstrs: after.bstrs - self.bstrs,
            object_boxes: after.object_boxes - self.object_boxes,
            safearrays: after.safearrays - self.safearrays,
            record_buffers: after.record_buffers - self.record_buffers,
        }
    }
}

impl HandleBalance {
    pub fn is_zero(self) -> bool {
        self.bstrs == 0
            && self.object_boxes == 0
            && self.safearrays == 0
            && self.record_buffers == 0
    }
}

#[cfg(any(test, feature = "live-counters"))]
mod imp {
    use super::LiveHandleCounts;
    use core::sync::atomic::{AtomicIsize, Ordering};

    static LIVE_BSTRS: AtomicIsize = AtomicIsize::new(0);
    static LIVE_OBJECT_BOXES: AtomicIsize = AtomicIsize::new(0);
    static LIVE_SAFEARRAYS: AtomicIsize = AtomicIsize::new(0);
    static LIVE_RECORD_BUFFERS: AtomicIsize = AtomicIsize::new(0);

    #[cfg(test)]
    std::thread_local! {
        static THREAD_LIVE: core::cell::Cell<LiveHandleCounts> = const {
            core::cell::Cell::new(LiveHandleCounts {
                bstrs: 0,
                object_boxes: 0,
                safearrays: 0,
                record_buffers: 0,
            })
        };
    }

    #[cfg(test)]
    fn update_thread(mut update: impl FnMut(&mut LiveHandleCounts)) {
        THREAD_LIVE.with(|counts| {
            let mut next = counts.get();
            update(&mut next);
            counts.set(next);
        });
    }

    pub(crate) fn bstr_allocated() {
        LIVE_BSTRS.fetch_add(1, Ordering::AcqRel);
        #[cfg(test)]
        update_thread(|counts| counts.bstrs += 1);
    }

    pub(crate) fn bstr_freed() {
        LIVE_BSTRS.fetch_sub(1, Ordering::AcqRel);
        #[cfg(test)]
        update_thread(|counts| counts.bstrs -= 1);
    }

    pub(crate) fn object_box_allocated() {
        LIVE_OBJECT_BOXES.fetch_add(1, Ordering::AcqRel);
        #[cfg(test)]
        update_thread(|counts| counts.object_boxes += 1);
    }

    pub(crate) fn object_box_freed() {
        LIVE_OBJECT_BOXES.fetch_sub(1, Ordering::AcqRel);
        #[cfg(test)]
        update_thread(|counts| counts.object_boxes -= 1);
    }

    pub(crate) fn safearray_allocated() {
        LIVE_SAFEARRAYS.fetch_add(1, Ordering::AcqRel);
        #[cfg(test)]
        update_thread(|counts| counts.safearrays += 1);
    }

    pub(crate) fn safearray_freed() {
        LIVE_SAFEARRAYS.fetch_sub(1, Ordering::AcqRel);
        #[cfg(test)]
        update_thread(|counts| counts.safearrays -= 1);
    }

    pub(crate) fn record_buffer_allocated() {
        LIVE_RECORD_BUFFERS.fetch_add(1, Ordering::AcqRel);
        #[cfg(test)]
        update_thread(|counts| counts.record_buffers += 1);
    }

    pub(crate) fn record_buffer_freed() {
        LIVE_RECORD_BUFFERS.fetch_sub(1, Ordering::AcqRel);
        #[cfg(test)]
        update_thread(|counts| counts.record_buffers -= 1);
    }

    pub fn live_handle_counts() -> LiveHandleCounts {
        LiveHandleCounts {
            bstrs: LIVE_BSTRS.load(Ordering::Acquire),
            object_boxes: LIVE_OBJECT_BOXES.load(Ordering::Acquire),
            safearrays: LIVE_SAFEARRAYS.load(Ordering::Acquire),
            record_buffers: LIVE_RECORD_BUFFERS.load(Ordering::Acquire),
        }
    }

    #[cfg(test)]
    pub(crate) fn thread_live_handle_counts() -> LiveHandleCounts {
        THREAD_LIVE.with(core::cell::Cell::get)
    }
}

#[cfg(not(any(test, feature = "live-counters")))]
mod imp {
    use super::LiveHandleCounts;

    pub(crate) fn bstr_allocated() {}
    pub(crate) fn bstr_freed() {}
    pub(crate) fn object_box_allocated() {}
    pub(crate) fn object_box_freed() {}
    pub(crate) fn safearray_allocated() {}
    pub(crate) fn safearray_freed() {}
    pub(crate) fn record_buffer_allocated() {}
    pub(crate) fn record_buffer_freed() {}

    pub fn live_handle_counts() -> LiveHandleCounts {
        LiveHandleCounts::default()
    }
}

pub use imp::live_handle_counts;
#[cfg(test)]
pub(crate) use imp::thread_live_handle_counts;
pub(crate) use imp::{
    bstr_allocated, bstr_freed, object_box_allocated, object_box_freed, record_buffer_allocated,
    record_buffer_freed, safearray_allocated, safearray_freed,
};
