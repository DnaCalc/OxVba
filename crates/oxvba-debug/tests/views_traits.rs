use oxvba_debug::{
    DebugBreakpointView, DebugExitView, DebugFrameView, DebugModuleView, DebugPauseView,
    DebugRunResultView, DebugSourceLocationView, DebugStopReasonView, DebugValueView,
    DebugWatchView,
};
use serde::{Deserialize, Serialize};
use static_assertions::assert_impl_all;

fn assert_serde_round_trip<T>()
where
    T: Serialize + for<'de> Deserialize<'de>,
{
}

#[test]
fn view_types_are_transport_safe() {
    assert_impl_all!(DebugPauseView: Send, Sync, Clone, std::fmt::Debug, Serialize);
    assert_impl_all!(DebugBreakpointView: Send, Sync, Clone, std::fmt::Debug, Serialize);
    assert_impl_all!(DebugWatchView: Send, Sync, Clone, std::fmt::Debug, Serialize);
    assert_impl_all!(DebugFrameView: Send, Sync, Clone, std::fmt::Debug, Serialize);
    assert_impl_all!(DebugValueView: Send, Sync, Clone, std::fmt::Debug, Serialize);
    assert_impl_all!(DebugSourceLocationView: Send, Sync, Clone, std::fmt::Debug, Serialize);
    assert_impl_all!(DebugStopReasonView: Send, Sync, Clone, std::fmt::Debug, Serialize);
    assert_impl_all!(DebugModuleView: Send, Sync, Clone, std::fmt::Debug, Serialize);
    assert_impl_all!(DebugRunResultView: Send, Sync, Clone, std::fmt::Debug, Serialize);
    assert_impl_all!(DebugExitView: Send, Sync, Clone, std::fmt::Debug, Serialize);

    assert_serde_round_trip::<DebugPauseView>();
    assert_serde_round_trip::<DebugBreakpointView>();
    assert_serde_round_trip::<DebugWatchView>();
    assert_serde_round_trip::<DebugFrameView>();
    assert_serde_round_trip::<DebugValueView>();
    assert_serde_round_trip::<DebugSourceLocationView>();
    assert_serde_round_trip::<DebugStopReasonView>();
    assert_serde_round_trip::<DebugModuleView>();
    assert_serde_round_trip::<DebugRunResultView>();
    assert_serde_round_trip::<DebugExitView>();
}
