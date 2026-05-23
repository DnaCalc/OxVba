#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::{DebugEvent, DebugRunResultView, DebugStopReasonView};
use oxvba_host::DirectHostBreakpointId;

#[test]
fn dap_style_flow_attach_breakpoint_stack_evaluate_exit() {
    let attach = support_handle::attach(support_handle::call_manifest());
    let receiver = attach.events;
    let handle = attach.handle;

    let startup = drain_events(&receiver);
    assert!(startup.iter().any(
        |event| matches!(event, DebugEvent::ModuleLoaded { module, .. } if module.name == "Module1")
    ));

    assert!(
        startup
            .iter()
            .any(|event| matches!(event, DebugEvent::ThreadStarted { thread_id: 1, .. }))
    );

    let bp = handle
        .set_source_breakpoint("Module1", 5, true)
        .expect("set breakpoint");
    assert_eq!(bp.module, "Module1");
    assert_eq!(bp.file_line, 5);
    assert!(bp.enabled);

    let start = handle.start().expect("start");
    let pause = match start {
        DebugRunResultView::Paused(pause) => pause,
        other => panic!("expected start pause, got {other:?}"),
    };
    assert_eq!(pause.reason, DebugStopReasonView::Entry);
    let frames = handle.stack_frames().expect("stack frames");
    assert!(!frames.is_empty());

    let stopped = handle.continue_execution().expect("continue to breakpoint");
    let pause = match stopped {
        DebugRunResultView::Paused(pause) => pause,
        other => panic!("expected breakpoint pause, got {other:?}"),
    };
    assert_eq!(pause.reason, DebugStopReasonView::Breakpoint);
    assert_eq!(
        pause
            .current_location
            .as_ref()
            .map(|location| location.module.as_str()),
        Some("Module1")
    );
    assert_eq!(
        pause
            .current_location
            .as_ref()
            .map(|location| location.file_line),
        Some(5)
    );

    let frames = handle.stack_frames().expect("stack frames at breakpoint");
    let frame_id = frames.last().expect("current frame").id.clone().into();
    let locals = handle.frame_locals(&frame_id).expect("locals");
    assert!(
        locals
            .iter()
            .any(|value| value.name.as_deref() == Some("y"))
    );
    let y = handle.evaluate(Some(&frame_id), "y").expect("evaluate y");
    assert_eq!(y.name.as_deref(), Some("y"));

    let bp_id = DirectHostBreakpointId::new(bp.id);
    handle
        .clear_source_breakpoint(&bp_id)
        .expect("clear breakpoint");
    let exit = handle.continue_execution().expect("continue to exit");
    assert!(matches!(exit, DebugRunResultView::Exited(_)));

    let events = drain_events(&receiver);
    assert!(events.iter().any(|event| matches!(
        event,
        DebugEvent::Stopped {
            reason: DebugStopReasonView::Breakpoint,
            ..
        }
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, DebugEvent::Exited { .. }))
    );
    handle.detach().expect("detach");
}

fn drain_events(receiver: &oxvba_debug::DebugEventReceiver) -> Vec<DebugEvent> {
    let mut events = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        events.push(event);
    }
    events
}
