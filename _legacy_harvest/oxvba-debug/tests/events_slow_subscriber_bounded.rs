#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{
    DebugAttachConfig, DebugEventChannelMode, DebugEventDelivery, DebugEventRecvError,
};

#[test]
fn bounded_slow_subscriber_reports_drop_without_blocking_worker() {
    let config = DebugAttachConfig {
        event_channel: DebugEventChannelMode::Bounded(1),
        ..DebugAttachConfig::default()
    };
    let attach = support_handle::attach_with_config(support_handle::call_manifest(), config);

    let lag = attach.events.try_recv_delivery().expect("lag delivery");
    assert_eq!(
        lag,
        DebugEventDelivery::Lag(oxvba_debug::DebugEventLag { dropped: 1 })
    );
    let event = attach.events.recv().expect("surviving startup event");
    assert_eq!(event.seq(), 2);
    assert_eq!(
        attach.events.try_recv_delivery(),
        Err(DebugEventRecvError::Empty)
    );
    attach.handle.detach().expect("detach");
}
