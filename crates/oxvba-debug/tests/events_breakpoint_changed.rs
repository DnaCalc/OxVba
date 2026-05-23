#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{DebugBreakpointChangeKind, DebugEvent};
use oxvba_host::DirectHostBreakpointId;

#[test]
fn breakpoint_add_toggle_clear_emit_changed_events() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let receiver = attach.handle.subscribe();

    let breakpoint = attach
        .handle
        .set_source_breakpoint("Module1", 5, true)
        .expect("add breakpoint");
    let add = receiver.recv().expect("add event");
    assert!(matches!(
        add,
        DebugEvent::BreakpointChanged { change: DebugBreakpointChangeKind::Added, breakpoint: ref observed, .. }
            if observed.id == breakpoint.id
    ));

    let id = DirectHostBreakpointId::new(breakpoint.id);
    attach
        .handle
        .set_breakpoint_enabled(&id, false)
        .expect("toggle breakpoint");
    let changed = receiver.recv().expect("changed event");
    assert!(matches!(
        changed,
        DebugEvent::BreakpointChanged {
            change: DebugBreakpointChangeKind::Changed,
            ..
        }
    ));

    attach.handle.clear_source_breakpoint(&id).expect("clear");
    let removed = receiver.recv().expect("removed event");
    assert!(matches!(
        removed,
        DebugEvent::BreakpointChanged {
            change: DebugBreakpointChangeKind::Removed,
            ..
        }
    ));
    assert!(add.seq() < changed.seq() && changed.seq() < removed.seq());
    attach.handle.detach().expect("detach");
}
