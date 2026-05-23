#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{DebugEvent, DebugStopReasonView};

#[test]
fn step_into_emits_stopped_step() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let receiver = attach.handle.subscribe();
    let _ = attach.handle.start().expect("start");
    let _entry = receiver.recv().expect("entry stopped");
    let _ = attach.handle.step_into().expect("step into");
    let continued = receiver.recv().expect("continued");
    let stopped = receiver.recv().expect("step stopped");
    assert!(matches!(continued, DebugEvent::Continued { .. }));
    assert!(matches!(
        stopped,
        DebugEvent::Stopped {
            reason: DebugStopReasonView::Step,
            ..
        }
    ));
    attach.handle.detach().expect("detach");
}
