use oxvba_debug::{DebugAttachConfig, DebugEventChannelMode};

#[test]
fn default_event_channel_is_bounded_256() {
    assert_eq!(
        DebugAttachConfig::default().event_channel,
        DebugEventChannelMode::Bounded(256)
    );
}
