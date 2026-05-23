#[path = "support_core/mod.rs"]
mod support_core;

use oxvba_debug::HostDebugVariantRunResult;

#[test]
fn core_continue_runs_to_completion() {
    let manifest = support_core::make_manifest("Sub Main()\nDim x As Long\nx = 1\nEnd Sub");
    let mut session = support_core::prepare(&manifest);
    let _ = session.start_variants().expect("entry pause");
    let result = session.continue_execution_variants().expect("continue");
    assert!(matches!(result, HostDebugVariantRunResult::Completed));
}
