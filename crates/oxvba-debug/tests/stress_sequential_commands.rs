#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugRunResultView;

#[test]
fn one_thousand_sequential_commands_stay_bounded() {
    let handle = support_handle::attach(support_handle::multi_module_manifest()).handle;
    let start = handle.start().expect("start");
    assert!(matches!(start, DebugRunResultView::Paused(_)));
    let bp = handle
        .set_source_breakpoint("Module1", 2, true)
        .expect("set breakpoint");
    let bp_id = oxvba_host::DirectHostBreakpointId::new(bp.id);
    let watch = handle.add_watch("y").expect("add watch");
    let watch_id = oxvba_host::DirectHostWatchId::new(watch.id);

    for index in 0..1000 {
        match index % 8 {
            0 => {
                let _ = handle.current_pause().expect("current pause");
            }
            1 => {
                let _ = handle.stack_frames().expect("stack frames");
            }
            2 => {
                let _ = handle.breakpoints().expect("breakpoints");
            }
            3 => {
                let _ = handle.evaluate_watches().expect("evaluate watches");
            }
            4 => {
                let _ = handle
                    .set_breakpoint_enabled(&bp_id, index % 16 == 0)
                    .expect("toggle breakpoint");
            }
            5 => {
                let _ = handle.update_watch(&watch_id, "y").expect("update watch");
            }
            6 => {
                let _ = handle.evaluate(None, "y");
            }
            _ => {
                let _ = handle.report_worker_apartment().expect("apartment");
            }
        }
    }

    handle.remove_watch(&watch_id).expect("remove watch");
    handle
        .clear_source_breakpoint(&bp_id)
        .expect("clear breakpoint");
    handle.detach().expect("detach");
}
