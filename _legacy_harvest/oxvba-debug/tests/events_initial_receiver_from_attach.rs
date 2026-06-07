#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugEvent;

#[test]
fn initial_receiver_observes_startup_events() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let first = attach.events.recv().expect("startup module event");
    let second = attach.events.recv().expect("startup thread event");
    assert!(matches!(first, DebugEvent::ModuleLoaded { .. }));
    assert!(matches!(second, DebugEvent::ThreadStarted { .. }));
    assert_eq!(first.seq(), 1);
    assert_eq!(second.seq(), 2);
    assert_eq!(first.session_id(), attach.handle.session_id().as_str());
    attach.handle.detach().expect("detach");
}
