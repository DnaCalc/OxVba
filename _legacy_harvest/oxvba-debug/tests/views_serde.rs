use oxvba_debug::{
    DebugBreakpointBindingStatusView, DebugBreakpointView, DebugExitView, DebugFrameView,
    DebugPauseView, DebugRunResultView, DebugSourceLocationView, DebugStopReasonView,
    DebugValueKindView, DebugValueView, DebugWatchStatusView, DebugWatchView,
};

fn roundtrip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    serde_json::from_str(&serde_json::to_string(value).expect("serialize")).expect("deserialize")
}

#[test]
fn pause_view_round_trips_json() {
    let value = DebugPauseView {
        reason: DebugStopReasonView::Entry,
        frame_id: "f1".to_string(),
        current_location: Some(DebugSourceLocationView {
            module: "Module1".to_string(),
            file_line: 2,
            runtime_line: Some(2),
        }),
        frames: vec![DebugFrameView {
            id: "f1".to_string(),
            name: "Module1::Main".to_string(),
            location: None,
        }],
    };
    assert_eq!(roundtrip(&value), value);
}

#[test]
fn breakpoint_view_round_trips_json() {
    let value = DebugBreakpointView {
        id: "bp1".to_string(),
        module: "Module1".to_string(),
        file_line: 7,
        enabled: true,
        binding_status: DebugBreakpointBindingStatusView::Bound,
    };
    assert_eq!(roundtrip(&value), value);
}

#[test]
fn watch_view_round_trips_json() {
    let value = DebugWatchView {
        id: "w1".to_string(),
        expression: "answer".to_string(),
        status: DebugWatchStatusView::Evaluated,
        value: None,
        error: None,
    };
    assert_eq!(roundtrip(&value), value);
}

#[test]
fn frame_view_round_trips_json() {
    let value = DebugFrameView {
        id: "f1".to_string(),
        name: "Module1::Main".to_string(),
        location: None,
    };
    assert_eq!(roundtrip(&value), value);
}

#[test]
fn value_view_round_trips_json() {
    let value = DebugValueView {
        name: Some("answer".to_string()),
        display_text: "42".to_string(),
        type_label: "Long".to_string(),
        kind: DebugValueKindView::Scalar,
        raw_repr: Some("03000000000000002a00000000000000".to_string()),
    };
    assert_eq!(roundtrip(&value), value);

    let run = DebugRunResultView::Exited(DebugExitView { exit_code: None });
    assert_eq!(roundtrip(&run), run);
}
