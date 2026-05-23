#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugRunResultView;

#[test]
fn handle_step_over_matches_core_flow() {
    let handle = support_handle::attach_handle();
    let _ = handle.start().expect("entry pause");
    let result = handle.step_over().expect("step over");
    match result {
        DebugRunResultView::Paused(pause) => assert!(pause.frames.len() <= 1),
        DebugRunResultView::Exited(_) => {}
    }
    handle.detach().expect("detach");
}
