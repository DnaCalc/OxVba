#![cfg(feature = "tokio")]

#[path = "support_handle/mod.rs"]
mod support_handle;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_async_commands_serialize_at_worker() {
    let handle = support_handle::attach_handle();
    let mut tasks = Vec::new();
    for index in 0..8 {
        let handle = handle.clone();
        tasks.push(tokio::spawn(async move {
            handle
                .set_source_breakpoint_async("Module1", if index % 2 == 0 { 5 } else { 6 }, true)
                .await
                .expect("set breakpoint")
        }));
    }
    for task in tasks {
        task.await.expect("task join");
    }
    assert_eq!(
        handle.breakpoints_async().await.expect("breakpoints").len(),
        8
    );
    handle.detach_async().await.expect("detach");
}
