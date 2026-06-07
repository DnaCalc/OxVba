#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugEvent;

#[test]
fn continue_emits_continued_before_stop_or_exit() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let receiver = attach.handle.subscribe();
    let _ = attach.handle.start().expect("start");
    let stopped = receiver.recv().expect("entry stopped");
    assert!(matches!(stopped, DebugEvent::Stopped { .. }));

    let _ = attach.handle.continue_execution().expect("continue");
    let continued = receiver.recv().expect("continued event");
    let terminal = receiver.recv().expect("terminal event");
    assert!(matches!(continued, DebugEvent::Continued { .. }));
    assert!(matches!(
        terminal,
        DebugEvent::Stopped { .. } | DebugEvent::Exited { .. }
    ));
    assert!(continued.seq() < terminal.seq());
    attach.handle.detach().expect("detach");
}
