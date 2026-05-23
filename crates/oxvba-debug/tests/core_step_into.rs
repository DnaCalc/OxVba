#[path = "support_core/mod.rs"]
mod support_core;

use oxvba_debug::HostDebugVariantRunResult;
use oxvba_vm::DebugStopReason;

#[test]
fn core_step_into_advances_to_next_statement() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let _ = session.start_variants().expect("entry pause");
    let HostDebugVariantRunResult::Paused(pause) = session.step_into_variants().expect("step into")
    else {
        panic!("expected step pause");
    };
    assert_eq!(pause.stop.reason, DebugStopReason::Step);
    assert_eq!(
        pause
            .frames
            .last()
            .expect("current frame")
            .procedure_name
            .to_ascii_lowercase(),
        "foo"
    );
}
