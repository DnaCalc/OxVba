#[path = "support_core/mod.rs"]
mod support_core;

use oxvba_debug::DebugWatchEvaluationStatus;

#[test]
fn core_add_evaluate_update_remove_watch() {
    let manifest = support_core::call_manifest();
    let mut session = support_core::prepare(&manifest);
    let watch = session.add_watch("y");
    assert_eq!(session.watches().len(), 1);
    assert!(matches!(
        session.evaluate_watches()[0].status,
        DebugWatchEvaluationStatus::Unavailable(_)
    ));
    session
        .update_watch(&watch.watch_id, "z")
        .expect("update watch");
    assert_eq!(session.watches()[0].expression_text, "z");
    session.remove_watch(&watch.watch_id).expect("remove watch");
    assert!(session.watches().is_empty());
}
