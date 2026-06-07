#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{DebugEvent, DebugStopReasonView};

#[test]
fn continue_to_breakpoint_emits_stopped_breakpoint() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let receiver = attach.handle.subscribe();
    let _ = attach
        .handle
        .set_source_breakpoint("Module1", 5, true)
        .expect("breakpoint");
    let _bp_event = receiver.recv().expect("breakpoint changed");
    let _ = attach.handle.start().expect("start");
    let _entry = receiver.recv().expect("entry stopped");
    let _ = attach.handle.continue_execution().expect("continue");
    let _continued = receiver.recv().expect("continued");
    let stopped = receiver.recv().expect("breakpoint stopped");
    assert!(matches!(
        stopped,
        DebugEvent::Stopped {
            reason: DebugStopReasonView::Breakpoint,
            ..
        }
    ));
    attach.handle.detach().expect("detach");
}
