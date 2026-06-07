#[path = "support_handle/mod.rs"]
mod support_handle;

use crossbeam_channel::TryRecvError;
use oxvba_debug::DebugEvent;

#[test]
fn late_subscriber_does_not_receive_replay() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let late = attach.handle.subscribe();
    assert_eq!(late.try_recv(), Err(TryRecvError::Empty));

    let breakpoint = attach
        .handle
        .set_source_breakpoint("Module1", 5, true)
        .expect("set breakpoint");
    let event = late.recv().expect("future breakpoint event");
    assert!(matches!(
        event,
        DebugEvent::BreakpointChanged { breakpoint: observed, .. } if observed.id == breakpoint.id
    ));
    attach.handle.detach().expect("detach");
}
