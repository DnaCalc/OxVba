use std::{cell::RefCell, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOutputChannel {
    Stdout,
    Stderr,
    Host,
}

pub trait HostOutputTap: Send + Sync {
    fn on_output(&self, channel: HostOutputChannel, text: &str);
}

impl<F> HostOutputTap for F
where
    F: Fn(HostOutputChannel, &str) + Send + Sync,
{
    fn on_output(&self, channel: HostOutputChannel, text: &str) {
        self(channel, text);
    }
}

thread_local! {
    static OUTPUT_TAPS: RefCell<Vec<Arc<dyn HostOutputTap>>> = RefCell::new(Vec::new());
}

pub struct ThreadOutputTapGuard;

impl Drop for ThreadOutputTapGuard {
    fn drop(&mut self) {
        OUTPUT_TAPS.with(|taps| {
            taps.borrow_mut().pop();
        });
    }
}

pub fn install_thread_output_tap(tap: Arc<dyn HostOutputTap>) -> ThreadOutputTapGuard {
    OUTPUT_TAPS.with(|taps| {
        taps.borrow_mut().push(tap);
    });
    ThreadOutputTapGuard
}

pub(crate) fn emit_thread_output_tap(channel: HostOutputChannel, text: &str) {
    OUTPUT_TAPS.with(|taps| {
        for tap in taps.borrow().iter() {
            tap.on_output(channel, text);
        }
    });
}
