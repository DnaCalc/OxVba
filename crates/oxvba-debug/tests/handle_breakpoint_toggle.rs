#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugRunResultView;

#[test]
fn handle_set_breakpoint_enabled_toggles_real_binding() {
    let handle = support_handle::attach_handle();
    let breakpoint = handle
        .set_source_breakpoint("Module1", 5, false)
        .expect("set disabled breakpoint");
    assert!(!breakpoint.enabled);
    let enabled = handle
        .set_breakpoint_enabled(&breakpoint.id.into(), true)
        .expect("enable breakpoint");
    assert!(enabled.enabled);
    let _ = handle.start().expect("entry pause");
    let hit = handle.continue_execution().expect("continue to breakpoint");
    assert!(matches!(hit, DebugRunResultView::Paused(_)));
    handle.detach().expect("detach");
}
