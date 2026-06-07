#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugWatchStatusView;

#[test]
fn handle_add_watch_records_expression() {
    let handle = support_handle::attach_handle();
    let watch = handle.add_watch("y").expect("add watch");
    assert_eq!(watch.expression, "y");
    assert_eq!(watch.status, DebugWatchStatusView::Pending);
    assert!(
        handle
            .evaluate_watches()
            .expect("watches")
            .iter()
            .any(|item| item.id == watch.id)
    );
    handle.detach().expect("detach");
}
