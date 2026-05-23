#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugError;

#[test]
fn worker_panic_marks_handle_failed_without_deadlock() {
    let handle = support_handle::attach_handle();
    handle
        .panic_worker_for_test()
        .expect("panic command should enqueue before worker exits");

    let err = handle
        .start()
        .expect_err("subsequent call reports worker failure");
    assert!(matches!(
        err,
        DebugError::WorkerFailed { stage: "panic", .. }
    ));

    let err = handle
        .detach()
        .expect_err("detach reports recorded worker failure");
    assert!(matches!(
        err,
        DebugError::WorkerFailed { stage: "panic", .. }
    ));
}
