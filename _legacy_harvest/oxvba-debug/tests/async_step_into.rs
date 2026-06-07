#![cfg(feature = "tokio")]

#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugRunResultView;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn step_into_async_matches_sync_step_into() {
    let handle = support_handle::attach_handle();
    let _ = handle.start_async().await.expect("start");
    let result = handle.step_into_async().await.expect("step into");
    assert!(matches!(result, DebugRunResultView::Paused(_)));
    handle.detach_async().await.expect("detach");
}
