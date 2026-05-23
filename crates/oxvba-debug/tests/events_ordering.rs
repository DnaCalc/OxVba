#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugEvent;
use oxvba_host::DirectHostBreakpointId;

#[test]
fn event_seq_precedes_command_response_for_state_change() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let receiver = attach.handle.subscribe();
    let breakpoint = attach
        .handle
        .set_source_breakpoint("Module1", 5, true)
        .expect("set breakpoint");
    let add_event = receiver.recv().expect("add event");
    assert!(matches!(add_event, DebugEvent::BreakpointChanged { .. }));

    let id = DirectHostBreakpointId::new(breakpoint.id);
    attach
        .handle
        .set_breakpoint_enabled(&id, false)
        .expect("toggle breakpoint");
    let change_event = receiver.recv().expect("change event");
    assert!(change_event.seq() > add_event.seq());
    attach.handle.detach().expect("detach");
}
