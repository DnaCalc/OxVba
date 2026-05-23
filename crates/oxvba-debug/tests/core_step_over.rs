#[path = "support_core/mod.rs"]
mod support_core;

use oxvba_debug::HostDebugVariantRunResult;

#[test]
fn core_step_over_preserves_call_depth_policy() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let _ = session.start_variants().expect("entry pause");
    let result = session.step_over_variants().expect("step over");
    match result {
        HostDebugVariantRunResult::Paused(pause) => {
            assert!(
                pause.frames.len() <= 1,
                "step-over should not expose a deeper current stack"
            );
        }
        HostDebugVariantRunResult::Completed => {}
    }
}
