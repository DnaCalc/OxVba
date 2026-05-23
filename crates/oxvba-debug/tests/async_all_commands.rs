#![cfg(feature = "tokio")]

#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_host::{DirectHostBreakpointId, DirectHostWatchId};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_sync_command_has_async_wrapper() {
    let handle = support_handle::attach_handle();
    let _apartment = handle
        .report_worker_apartment_async()
        .await
        .expect("apartment");
    let bp = handle
        .set_source_breakpoint_async("Module1", 5, true)
        .await
        .expect("set breakpoint");
    let _bps = handle.breakpoints_async().await.expect("breakpoints");
    let id = DirectHostBreakpointId::new(bp.id.clone());
    let _ = handle
        .set_breakpoint_enabled_async(&id, false)
        .await
        .expect("disable breakpoint");
    let watch = handle.add_watch_async("y").await.expect("add watch");
    let watch_id = DirectHostWatchId::new(watch.id);
    let _ = handle
        .update_watch_async(&watch_id, "z")
        .await
        .expect("update watch");
    handle
        .remove_watch_async(&watch_id)
        .await
        .expect("remove watch");
    handle
        .clear_source_breakpoint_async(&id)
        .await
        .expect("clear");
    let _ = handle.start_async().await.expect("start");
    let _ = handle.step_into_async().await.expect("step into");
    let frames = handle.stack_frames_async().await.expect("frames");
    let current = frames.last().expect("frame").id.clone().into();
    let _pause = handle.current_pause_async().await.expect("pause");
    let _locals = handle.frame_locals_async(&current).await.expect("locals");
    let _value = handle
        .evaluate_async(Some(&current), "y")
        .await
        .expect("evaluate");
    let _watches = handle.evaluate_watches_async().await.expect("watches");
    let _ = handle.step_out_async().await.expect("step out");
    let _ = handle.step_over_async().await;
    handle.detach_async().await.expect("detach");
}
