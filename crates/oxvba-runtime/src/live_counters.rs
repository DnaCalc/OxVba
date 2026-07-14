#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
/// A snapshot of instrumented live runtime carriers.
///
/// The snapshot's scope is determined by the function that produced it. Use
/// [`live_handle_counts`] for a process-wide snapshot and, when the
/// `live-counters` instrumentation feature is enabled, use
/// `current_thread_live_handle_counts` for the current thread only.
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

    fn update_thread(mut update: impl FnMut(&mut LiveHandleCounts)) {
        // A carrier can be dropped from another thread-local destructor after
        // this counter has itself been destroyed. The process-wide accounting
        // below remains authoritative at thread teardown; do not turn
        // instrumentation into a destructor panic.
        let _ = THREAD_LIVE.try_with(|counts| {
            let mut next = counts.get();
            update(&mut next);
            counts.set(next);
        });
    }

    pub(crate) fn bstr_allocated() {
        LIVE_BSTRS.fetch_add(1, Ordering::AcqRel);
        update_thread(|counts| counts.bstrs += 1);
    }

    pub(crate) fn bstr_freed() {
        LIVE_BSTRS.fetch_sub(1, Ordering::AcqRel);
        update_thread(|counts| counts.bstrs -= 1);
    }

    pub(crate) fn object_box_allocated() {
        LIVE_OBJECT_BOXES.fetch_add(1, Ordering::AcqRel);
        update_thread(|counts| counts.object_boxes += 1);
    }

    pub(crate) fn object_box_freed() {
        LIVE_OBJECT_BOXES.fetch_sub(1, Ordering::AcqRel);
        update_thread(|counts| counts.object_boxes -= 1);
    }

    pub(crate) fn safearray_allocated() {
        LIVE_SAFEARRAYS.fetch_add(1, Ordering::AcqRel);
        update_thread(|counts| counts.safearrays += 1);
    }

    pub(crate) fn safearray_freed() {
        LIVE_SAFEARRAYS.fetch_sub(1, Ordering::AcqRel);
        update_thread(|counts| counts.safearrays -= 1);
    }

    pub(crate) fn record_buffer_allocated() {
        LIVE_RECORD_BUFFERS.fetch_add(1, Ordering::AcqRel);
        update_thread(|counts| counts.record_buffers += 1);
    }

    pub(crate) fn record_buffer_freed() {
        LIVE_RECORD_BUFFERS.fetch_sub(1, Ordering::AcqRel);
        update_thread(|counts| counts.record_buffers -= 1);
    }

    /// Return a process-wide snapshot of every instrumented live carrier.
    ///
    /// This includes allocations owned by all threads and is therefore the
    /// right source for subprocess and whole-process lifecycle evidence. It is
    /// not an isolated per-run measurement when sibling threads are active.
    #[must_use]
    pub fn live_handle_counts() -> LiveHandleCounts {
        LiveHandleCounts {
            bstrs: LIVE_BSTRS.load(Ordering::Acquire),
            object_boxes: LIVE_OBJECT_BOXES.load(Ordering::Acquire),
            safearrays: LIVE_SAFEARRAYS.load(Ordering::Acquire),
            record_buffers: LIVE_RECORD_BUFFERS.load(Ordering::Acquire),
        }
    }

    /// Return a snapshot of instrumented carriers allocated or freed on the
    /// current thread.
    ///
    /// This is suitable for a synchronous VM/JIT run whose carrier work and
    /// callbacks remain on the runner thread. A carrier transferred across
    /// threads is intentionally visible as an allocation on one thread and a
    /// free on the other; use [`live_handle_counts`] for whole-process evidence.
    #[must_use]
    pub fn current_thread_live_handle_counts() -> LiveHandleCounts {
        THREAD_LIVE
            .try_with(core::cell::Cell::get)
            .unwrap_or_default()
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

    /// Return the process-wide live-carrier snapshot.
    ///
    /// Instrumentation is disabled in this build, so the snapshot is empty.
    #[must_use]
    pub fn live_handle_counts() -> LiveHandleCounts {
        LiveHandleCounts::default()
    }
}

#[cfg(any(test, feature = "live-counters"))]
pub use imp::current_thread_live_handle_counts;
pub use imp::live_handle_counts;
pub(crate) use imp::{
    bstr_allocated, bstr_freed, object_box_allocated, object_box_freed, record_buffer_allocated,
    record_buffer_freed, safearray_allocated, safearray_freed,
};

#[cfg(test)]
pub(crate) use current_thread_live_handle_counts as thread_live_handle_counts;

#[cfg(test)]
mod tests {
    use super::{HandleBalance, current_thread_live_handle_counts};
    use crate::bstr::BStr;

    #[test]
    fn current_thread_counts_detect_an_outstanding_same_thread_handle() {
        let before = current_thread_live_handle_counts();
        let value = BStr::from("same-thread");

        assert_eq!(
            before.balance_to(current_thread_live_handle_counts()),
            HandleBalance {
                bstrs: 1,
                ..HandleBalance::default()
            },
            "an outstanding same-thread allocation must remain visible"
        );

        drop(value);
        assert_eq!(current_thread_live_handle_counts(), before);
    }

    #[test]
    fn current_thread_counts_ignore_sibling_thread_allocations_and_frees() {
        let before = current_thread_live_handle_counts();
        let (allocated_tx, allocated_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();

        let child = std::thread::spawn(move || {
            let child_before = current_thread_live_handle_counts();
            let value = BStr::from("sibling-thread");
            allocated_tx
                .send(child_before.balance_to(current_thread_live_handle_counts()))
                .expect("parent must receive the live child allocation");
            release_rx
                .recv()
                .expect("parent must release the child allocation");
            drop(value);
            assert_eq!(current_thread_live_handle_counts(), child_before);
        });

        assert_eq!(
            allocated_rx
                .recv()
                .expect("child must publish its live allocation"),
            HandleBalance {
                bstrs: 1,
                ..HandleBalance::default()
            }
        );
        assert_eq!(
            current_thread_live_handle_counts(),
            before,
            "a sibling allocation must not change the parent-thread snapshot"
        );

        release_tx
            .send(())
            .expect("child must remain available for release");
        child.join().expect("counter-isolation child must finish");
        assert_eq!(
            current_thread_live_handle_counts(),
            before,
            "a sibling free must not change the parent-thread snapshot"
        );
    }
}
