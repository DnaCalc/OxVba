#![cfg(feature = "tokio")]

#[path = "support_handle/mod.rs"]
mod support_handle;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tokio_event_receiver_observes_same_sequence_as_sync_receiver() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let receiver = attach.handle.subscribe();
    let _ = attach.handle.start_async().await.expect("start");
    let event = receiver.recv_async().await.expect("event");
    assert_eq!(event.seq(), 3);
    attach.handle.detach_async().await.expect("detach");
}
