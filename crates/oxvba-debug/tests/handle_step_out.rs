#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugRunResultView;

#[test]
fn handle_step_out_matches_core_flow() {
    let handle = support_handle::attach_handle();
    let _ = handle.start().expect("entry pause");
    let _ = handle.step_into().expect("callee pause");
    let result = handle.step_out().expect("step out");
    assert!(matches!(
        result,
        DebugRunResultView::Paused(_) | DebugRunResultView::Exited(_)
    ));
    handle.detach().expect("detach");
}
