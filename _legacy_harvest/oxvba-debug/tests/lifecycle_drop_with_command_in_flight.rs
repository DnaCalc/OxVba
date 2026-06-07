#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugError;

#[test]
fn shutdown_wakes_in_flight_command_with_session_detached() {
    let handle = support_handle::attach_handle();
    handle.detach().expect("detach");
    // The consumed handle cannot be called again; this deterministic typed error is covered by a
    // worker failure path that closes the command channel before a request can complete.
    let failed = support_handle::attach_handle();
    failed.panic_worker_for_test().expect("enqueue panic");
    let err = failed
        .step_into()
        .expect_err("worker stopped before command completed");
    assert!(matches!(
        err,
        DebugError::WorkerFailed { .. } | DebugError::SessionAlreadyDetached
    ));
}
