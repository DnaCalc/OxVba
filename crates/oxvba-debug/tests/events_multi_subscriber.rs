#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugEvent;

#[test]
fn multiple_subscribers_receive_same_sequence() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let first = attach.handle.subscribe();
    let second = attach.handle.subscribe();
    let breakpoint = attach
        .handle
        .set_source_breakpoint("Module1", 5, true)
        .expect("set breakpoint");

    let a = first.recv().expect("first subscriber event");
    let b = second.recv().expect("second subscriber event");
    assert_eq!(a, b);
    assert!(matches!(
        a,
        DebugEvent::BreakpointChanged { breakpoint: observed, .. } if observed.id == breakpoint.id
    ));
    attach.handle.detach().expect("detach");
}
