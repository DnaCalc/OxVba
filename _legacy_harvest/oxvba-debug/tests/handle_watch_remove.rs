#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_host::DirectHostWatchId;

#[test]
fn handle_remove_watch_deletes_record() {
    let handle = support_handle::attach_handle();
    let watch = handle.add_watch("y").expect("add watch");
    handle
        .remove_watch(&DirectHostWatchId::new(watch.id))
        .expect("remove watch");
    assert!(handle.evaluate_watches().expect("watches").is_empty());
    handle.detach().expect("detach");
}
