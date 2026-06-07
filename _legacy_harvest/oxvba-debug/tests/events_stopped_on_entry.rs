#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{DebugEvent, DebugStopReasonView};

#[test]
fn attach_stop_on_entry_emits_stopped_entry() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let receiver = attach.handle.subscribe();
    let _ = attach.handle.start().expect("start");
    let stopped = receiver.recv().expect("stopped event");
    assert!(matches!(
        stopped,
        DebugEvent::Stopped {
            reason: DebugStopReasonView::Entry,
            ..
        }
    ));
    attach.handle.detach().expect("detach");
}
