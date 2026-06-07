#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugRunResultView;

#[test]
fn handle_step_into_matches_core_flow() {
    let handle = support_handle::attach_handle();
    let _ = handle.start().expect("entry pause");
    let result = handle.step_into().expect("step into");
    let DebugRunResultView::Paused(pause) = result else {
        panic!("expected callee pause");
    };
    assert!(
        pause
            .frames
            .iter()
            .any(|frame| frame.name.to_ascii_lowercase().contains("foo"))
    );
    handle.detach().expect("detach");
}
