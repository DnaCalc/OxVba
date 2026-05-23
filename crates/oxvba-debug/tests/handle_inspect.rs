#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugValueKindView;

#[test]
fn handle_current_pause_stack_locals_and_evaluate_work() {
    let handle = support_handle::attach_handle();
    let _ = handle.start().expect("entry pause");
    let _ = handle.step_into().expect("callee pause");

    let pause = handle
        .current_pause()
        .expect("current pause")
        .expect("paused");
    let frames = handle.stack_frames().expect("stack frames");
    assert_eq!(frames, pause.frames);
    let current = frames.last().expect("current frame");
    assert!(current.name.to_ascii_lowercase().contains("foo"));

    let locals = handle
        .frame_locals(&current.id.clone().into())
        .expect("locals");
    assert!(
        locals
            .iter()
            .any(|value| value.name.as_deref() == Some("y"))
    );

    let y = handle
        .evaluate(Some(&current.id.clone().into()), "y")
        .expect("evaluate y");
    assert_eq!(y.display_text, "4");
    assert_eq!(y.kind, DebugValueKindView::Scalar);
    handle.detach().expect("detach");
}
