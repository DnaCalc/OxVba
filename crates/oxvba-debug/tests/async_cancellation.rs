#![cfg(feature = "tokio")]

#[path = "support_handle/mod.rs"]
mod support_handle;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_future_does_not_poison_worker() {
    let handle = support_handle::attach_handle();
    let future = handle.set_source_breakpoint_async("Module1", 5, true);
    drop(future);
    let _ = handle
        .start_async()
        .await
        .expect("worker still accepts start");
    let _ = handle
        .step_into_async()
        .await
        .expect("worker still accepts step");
    handle.detach_async().await.expect("detach");
}
