#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{
    DebugBreakpointBindingStatusView, DebugRunResultView, DebugStopReasonView, DebugWatchStatusView,
};
use oxvba_host::{DirectHostBreakpointId, DirectHostWatchId};

#[test]
fn oxide_cockpit_flow_watch_breakpoint_step_disable_exit() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let handle = attach.handle;

    let watch = handle.add_watch("y").expect("add watch");
    let watch_id = DirectHostWatchId::new(watch.id.clone());
    assert_eq!(watch.expression, "y");

    let bp = handle
        .set_source_breakpoint("Module1", 5, true)
        .expect("set breakpoint");
    assert_eq!(bp.binding_status, DebugBreakpointBindingStatusView::Bound);
    let bp_id = DirectHostBreakpointId::new(bp.id.clone());

    let _ = handle.start().expect("start");
    let stopped = handle.continue_execution().expect("continue to breakpoint");
    let pause = match stopped {
        DebugRunResultView::Paused(pause) => pause,
        other => panic!("expected breakpoint pause, got {other:?}"),
    };
    assert_eq!(pause.reason, DebugStopReasonView::Breakpoint);

    let watches = handle.evaluate_watches().expect("evaluate watches");
    let y_watch = watches
        .iter()
        .find(|candidate| candidate.id == watch_id.as_str())
        .expect("watch y");
    assert_eq!(y_watch.status, DebugWatchStatusView::Evaluated);

    let stepped = handle
        .step_into()
        .expect("step into/over current statement");
    assert!(matches!(
        stepped,
        DebugRunResultView::Paused(_) | DebugRunResultView::Exited(_)
    ));

    let disabled = handle
        .set_breakpoint_enabled(&bp_id, false)
        .expect("disable breakpoint");
    assert!(!disabled.enabled);

    handle
        .update_watch(&watch_id, "z")
        .expect("update watch expression");
    let watches = handle.evaluate_watches().expect("evaluate updated watches");
    assert!(watches.iter().any(|watch| watch.expression == "z"));

    let exit = handle.continue_execution().expect("continue to exit");
    assert!(matches!(exit, DebugRunResultView::Exited(_)));
    handle.remove_watch(&watch_id).expect("remove watch");
    handle
        .clear_source_breakpoint(&bp_id)
        .expect("clear breakpoint");
    handle.detach().expect("detach");
}
