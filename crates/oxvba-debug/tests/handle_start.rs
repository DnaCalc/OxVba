#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{DebugRunResultView, DebugStopReasonView};

#[test]
fn handle_start_matches_core_start() {
    let handle = support_handle::attach_handle();
    let result = handle.start().expect("start");
    let DebugRunResultView::Paused(pause) = result else {
        panic!("expected entry pause");
    };
    assert_eq!(pause.reason, DebugStopReasonView::Entry);
    assert_eq!(pause.frames.len(), 1);
    handle.detach().expect("detach");
}
