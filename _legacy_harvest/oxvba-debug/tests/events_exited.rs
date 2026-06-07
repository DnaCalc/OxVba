#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugEvent;

#[test]
fn completion_emits_exited() {
    let attach = support_handle::attach(support_handle::make_manifest(
        "Sub Main()\nDim x As Long\nx = 1\nEnd Sub",
    ));
    let receiver = attach.handle.subscribe();
    let _ = attach.handle.start().expect("start");
    let _entry = receiver.recv().expect("entry stopped");
    let _ = attach.handle.continue_execution().expect("continue");
    let _continued = receiver.recv().expect("continued");
    let exited = receiver.recv().expect("exited");
    assert!(matches!(
        exited,
        DebugEvent::Exited {
            exit_code: None,
            ..
        }
    ));
    attach.handle.detach().expect("detach");
}
