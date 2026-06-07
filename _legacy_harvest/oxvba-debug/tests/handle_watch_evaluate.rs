#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{DebugValueKindView, DebugWatchStatusView};

#[test]
fn handle_evaluate_watches_returns_current_values() {
    let handle = support_handle::attach_handle();
    let _watch = handle.add_watch("y").expect("add watch");
    let _ = handle.start().expect("entry pause");
    let _ = handle.step_into().expect("callee pause");
    let watches = handle.evaluate_watches().expect("evaluate watches");
    assert_eq!(watches.len(), 1);
    assert_eq!(watches[0].status, DebugWatchStatusView::Evaluated);
    assert_eq!(watches[0].value.as_ref().expect("value").display_text, "4");
    assert_eq!(
        watches[0].value.as_ref().expect("value").kind,
        DebugValueKindView::Scalar
    );
    handle.detach().expect("detach");
}
