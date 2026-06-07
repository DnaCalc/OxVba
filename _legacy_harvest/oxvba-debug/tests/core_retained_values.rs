#[path = "support_core/mod.rs"]
mod support_core;

use oxvba_debug::HostDebugVariantRunResult;
use oxvba_runtime::Variant;

#[test]
fn core_pause_retains_variant_values_for_inspection() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let _ = session.start_variants().expect("entry pause");
    let HostDebugVariantRunResult::Paused(pause) =
        session.step_into_variants().expect("callee pause")
    else {
        panic!("expected callee pause");
    };
    let y = pause
        .frames
        .last()
        .expect("current frame")
        .values
        .iter()
        .find(|value| value.name.eq_ignore_ascii_case("y"))
        .expect("y value");
    assert_eq!(y.variant_value, Variant::from_i32(4));
}
