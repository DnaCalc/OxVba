#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugError;

#[test]
fn detach_last_handle_joins_worker_cleanly() {
    let handle = support_handle::attach_handle();
    handle.detach().expect("last handle detaches and joins");
}

#[test]
fn detach_with_clones_returns_outstanding_handles() {
    let handle = support_handle::attach_handle();
    let clone = handle.clone();
    let err = handle
        .detach()
        .expect_err("clone keeps session outstanding");
    assert_eq!(err, DebugError::OutstandingHandles { count: 1 });
    clone.detach().expect("last clone detaches");
}
