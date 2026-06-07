#[path = "support_core/mod.rs"]
mod support_core;

use oxvba_debug::HostDebugVariantRunResult;

#[test]
fn core_step_out_returns_to_caller() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let _ = session.start_variants().expect("entry pause");
    let _ = session.step_into_variants().expect("callee pause");
    let result = session.step_out_variants().expect("step out");
    assert!(matches!(
        result,
        HostDebugVariantRunResult::Completed | HostDebugVariantRunResult::Paused(_)
    ));
}
