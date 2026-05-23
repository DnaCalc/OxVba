#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_host::DirectHostWatchId;

#[test]
fn handle_update_watch_changes_expression() {
    let handle = support_handle::attach_handle();
    let watch = handle.add_watch("y").expect("add watch");
    let updated = handle
        .update_watch(&DirectHostWatchId::new(watch.id), "z")
        .expect("update watch");
    assert_eq!(updated.expression, "z");
    handle.detach().expect("detach");
}
