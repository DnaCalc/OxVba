#[path = "support_core/mod.rs"]
mod support_core;

use oxvba_debug::HostDebugVariantRunResult;
use oxvba_vm::DebugStopReason;

#[test]
fn core_start_stops_on_entry() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let HostDebugVariantRunResult::Paused(pause) = session.start_variants().expect("start") else {
        panic!("expected entry pause");
    };
    assert_eq!(pause.stop.reason, DebugStopReason::Entry);
    assert_eq!(
        pause.stop.location.procedure_name.to_ascii_lowercase(),
        "main"
    );
}
