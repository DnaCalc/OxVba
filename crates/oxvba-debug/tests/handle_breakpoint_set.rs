#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_debug::DebugBreakpointBindingStatusView;

#[test]
fn handle_set_source_breakpoint_binds_real_line() {
    let handle = support_handle::attach_handle();
    let breakpoint = handle
        .set_source_breakpoint("Module1", 6, true)
        .expect("set breakpoint");
    assert_eq!(breakpoint.module, "Module1");
    assert_eq!(breakpoint.file_line, 6);
    assert_eq!(
        breakpoint.binding_status,
        DebugBreakpointBindingStatusView::Bound
    );
    handle.detach().expect("detach");
}
