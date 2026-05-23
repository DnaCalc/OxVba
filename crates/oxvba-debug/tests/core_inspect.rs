#[path = "support_core/mod.rs"]
mod support_core;

use oxvba_debug::{DebugEvaluationRequest, HostDebugVariantRunResult};
use oxvba_runtime::Variant;

#[test]
fn core_stack_frames_and_locals_project() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let _ = session.start_variants().expect("entry pause");
    let HostDebugVariantRunResult::Paused(pause) =
        session.step_into_variants().expect("callee pause")
    else {
        panic!("expected callee pause");
    };
    let current = pause.frames.last().expect("current frame");
    assert!(current.procedure_name.eq_ignore_ascii_case("Foo"));
    assert!(
        current
            .values
            .iter()
            .any(|value| value.name.eq_ignore_ascii_case("y"))
    );
}

#[test]
fn core_evaluate_current_frame_identifier() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let _ = session.start_variants().expect("entry pause");
    let _ = session.step_into_variants().expect("callee pause");
    let value = session
        .evaluate_variant(&DebugEvaluationRequest::new("? y"))
        .expect("evaluate y");
    assert_eq!(value.value.variant_value, Variant::from_i32(4));
}
