#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{DebugError, DebugRunResultView};

#[test]
fn handle_continue_past_end_returns_exited_then_completed_errors() {
    let handle = support_handle::attach(support_handle::make_manifest(
        "Sub Main()\nDim x As Long\nx = 1\nEnd Sub",
    ))
    .handle;
    let _ = handle.start().expect("entry pause");
    let result = handle.continue_execution().expect("continue");
    assert!(matches!(result, DebugRunResultView::Exited(_)));
    let second = handle
        .continue_execution()
        .expect_err("completed session rejects another continue");
    assert!(matches!(
        second,
        DebugError::WorkerFailed { .. } | DebugError::Completed
    ));
    handle.detach().expect("detach");
}
