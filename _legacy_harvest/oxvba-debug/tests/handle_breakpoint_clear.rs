#[path = "support_handle/mod.rs"]
mod support_handle;

use oxvba_host::DirectHostBreakpointId;

#[test]
fn handle_clear_source_breakpoint_removes_stop() {
    let handle = support_handle::attach_handle();
    let breakpoint = handle
        .set_source_breakpoint("Module1", 2, true)
        .expect("set breakpoint");
    handle
        .clear_source_breakpoint(&DirectHostBreakpointId::new(breakpoint.id))
        .expect("clear breakpoint");
    assert!(handle.breakpoints().expect("breakpoints").is_empty());
    handle.detach().expect("detach");
}
