#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugEvent;

#[test]
fn attach_emits_primary_thread_started() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let _module = attach.events.recv().expect("module loaded");
    let thread = attach.events.recv().expect("thread started");
    assert!(matches!(
        thread,
        DebugEvent::ThreadStarted { thread_id: 1, .. }
    ));
    attach.handle.detach().expect("detach");
}
