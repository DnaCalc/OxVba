#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugRunResultView;

#[test]
fn handle_continue_matches_core_flow() {
    let handle = support_handle::attach(support_handle::make_manifest(
        "Sub Main()\nDim x As Long\nx = 1\nEnd Sub",
    ))
    .handle;
    let _ = handle.start().expect("entry pause");
    let result = handle.continue_execution().expect("continue");
    assert!(matches!(result, DebugRunResultView::Exited(_)));
    handle.detach().expect("detach");
}
