#[path = "support_core/mod.rs"]
mod support_core;

use oxvba_debug::{DebugBreakpointBindingStatus, HostDebugVariantRunResult};

#[test]
fn core_set_line_breakpoint_binds() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let breakpoint = session.set_source_breakpoint("Module1", 6);
    assert_eq!(
        breakpoint.binding_status,
        DebugBreakpointBindingStatus::Bound
    );
    assert!(session.source_breakpoints().iter().any(|record| {
        record.breakpoint_id == breakpoint.breakpoint_id
            && record.binding_status == DebugBreakpointBindingStatus::Bound
    }));
}

#[test]
fn core_disabled_breakpoint_does_not_stop() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let breakpoint = session.set_source_breakpoint("Module1", 2);
    session
        .set_breakpoint_enabled(&breakpoint.breakpoint_id, false)
        .expect("toggle breakpoint");
    let _ = session.start_variants().expect("entry pause");
    let result = session.continue_execution_variants().expect("continue");
    assert!(matches!(result, HostDebugVariantRunResult::Completed));
}

#[test]
fn core_clear_breakpoint_removes_binding() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let breakpoint = session.set_source_breakpoint("Module1", 2);
    let removed = session
        .clear_source_breakpoint(&breakpoint.breakpoint_id)
        .expect("clear breakpoint");
    assert_eq!(removed.breakpoint_id, breakpoint.breakpoint_id);
    assert!(session.source_breakpoints().is_empty());
}
