#[path = "support_handle/mod.rs"]
mod support_handle;

#[test]
fn handle_breakpoints_lists_current_records() {
    let handle = support_handle::attach_handle();
    let first = handle
        .set_source_breakpoint("Module1", 2, true)
        .expect("set first breakpoint");
    let second = handle
        .set_source_breakpoint("Module1", 6, false)
        .expect("set second breakpoint");
    let breakpoints = handle.breakpoints().expect("list breakpoints");
    assert_eq!(breakpoints.len(), 2);
    assert!(
        breakpoints
            .iter()
            .any(|breakpoint| breakpoint.id == first.id)
    );
    assert!(
        breakpoints
            .iter()
            .any(|breakpoint| breakpoint.id == second.id && !breakpoint.enabled)
    );
    handle.detach().expect("detach");
}
