#[path = "support_core/mod.rs"]
mod support_core;

use oxvba_debug::{
    DebugBreakpointBindingStatusView, DebugCoreRunResult, DebugStopReasonView, DebugValueKindView,
    DebugWatchStatusView, HostDebugVariantRunResult, breakpoint_view_from_core,
    pause_view_from_core, run_result_view_from_core, value_view_from_core, watch_view_from_core,
};

#[test]
fn pause_state_projects_to_pause_view() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let HostDebugVariantRunResult::Paused(pause) = session.start_variants().expect("start") else {
        panic!("expected pause");
    };
    let view = pause_view_from_core(&pause);
    assert_eq!(view.reason, DebugStopReasonView::Entry);
    assert_eq!(view.frames.len(), 1);
    assert!(view.current_location.is_some());
}

#[test]
fn breakpoint_record_projects_to_breakpoint_view() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let record = session.set_source_breakpoint("Module1", 2);
    let view = breakpoint_view_from_core(&record);
    assert!(view.id.contains(":breakpoint:1"));
    assert_eq!(view.module, "Module1");
    assert_eq!(view.binding_status, DebugBreakpointBindingStatusView::Bound);
}

#[test]
fn watch_record_projects_to_watch_view() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let watch = session.add_watch("y");
    let pending = watch_view_from_core(&session.evaluate_watches()[0]);
    assert_eq!(pending.id, watch.watch_id.as_str());
    assert_eq!(pending.status, DebugWatchStatusView::Pending);

    let _ = session.start_variants().expect("entry pause");
    let _ = session.step_into_variants().expect("callee pause");
    let evaluated = watch_view_from_core(&session.evaluate_watches()[0]);
    assert_eq!(evaluated.status, DebugWatchStatusView::Evaluated);
    assert_eq!(
        evaluated.value.expect("value").kind,
        DebugValueKindView::Scalar
    );
}

#[test]
fn run_result_projects_completion_as_exited() {
    let view = run_result_view_from_core(&DebugCoreRunResult::Completed);
    assert!(matches!(view, oxvba_debug::DebugRunResultView::Exited(_)));
}

#[test]
fn value_projection_does_not_expose_raw_variant() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let _ = session.start_variants().expect("entry pause");
    let HostDebugVariantRunResult::Paused(pause) =
        session.step_into_variants().expect("callee pause")
    else {
        panic!("expected pause");
    };
    let value = pause
        .frames
        .last()
        .expect("frame")
        .values
        .iter()
        .find(|value| value.name.eq_ignore_ascii_case("y"))
        .expect("y value");
    let view = value_view_from_core(value);
    assert_eq!(view.display_text, "4");
    let json = serde_json::to_string(&view).expect("serialize view");
    assert!(!json.contains("variant_value"));
}
