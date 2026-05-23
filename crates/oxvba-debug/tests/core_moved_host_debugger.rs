use std::{collections::BTreeMap, sync::Arc};

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_runtime::VarType;
use oxvba_vm::DebugStopReason;

use oxvba_debug::{
    DebugBreakpointBindingStatus, DebugBreakpointUnresolvedReason, DebugCoreConfig,
    DebugEvaluationRequest, DebugFrameValueKind, DebugSessionCore, DebugSessionError,
    DebugWatchEvaluationStatus, HostDebugVariantRunResult, prepare_debug_session_core,
};
use oxvba_host::{DirectHostSourceSpanStatus, DirectHostSourceUnavailableReason};
use oxvba_host::{Engine, HostConfig};

fn make_manifest(source: &str) -> ProjectManifest {
    ProjectManifest {
        project_name: "DebugHost".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![
            module_unit_from_source("Module1", ModuleKind::Procedural, source)
                .expect("module unit"),
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    }
}
fn prepare_debug_session(manifest: &ProjectManifest) -> DebugSessionCore {
    let engine = Arc::new(Engine::new(HostConfig::default()));
    prepare_debug_session_core(engine, manifest.clone(), DebugCoreConfig::default())
        .expect("debug session should prepare")
}

#[test]
fn prepare_debug_session_wraps_live_runtime_session() {
    let manifest = make_manifest("Sub Main()\nEnd Sub");
    let session = prepare_debug_session(&manifest);
    assert_eq!(session.manifest().project_name, "DebugHost");
    assert!(
        session
            .runtime()
            .procedure_metadata()
            .keys()
            .any(|name| name.ends_with("_main") || name.eq_ignore_ascii_case("main"))
    );
}

#[test]
fn debug_session_projects_frames_and_bounded_identifier_evaluation() {
    let manifest = make_manifest(
        "Sub Main()\n\
         Call Foo(4)\n\
         End Sub\n\
         \n\
         Sub Foo(ByVal y As Long)\n\
         Dim z As Long\n\
         z = y + 1\n\
         End Sub",
    );
    let mut session = prepare_debug_session(&manifest);

    let HostDebugVariantRunResult::Paused(entry_pause) =
        session.start_variants().expect("debug start should pause")
    else {
        panic!("expected entry pause");
    };
    assert_eq!(entry_pause.stop.reason, DebugStopReason::Entry);
    assert_eq!(entry_pause.frames.len(), 1);
    assert!(matches!(
        &entry_pause.current_source,
        DirectHostSourceSpanStatus::Known(span)
            if span.document_id.as_str() == "Module1"
                && span.start.line == 2
                && span.end.line == 3
    ));
    let HostDebugVariantRunResult::Paused(callee_pause) = session
        .step_into_variants()
        .expect("step into should pause in callee")
    else {
        panic!("expected callee pause");
    };
    assert_eq!(callee_pause.stop.reason, DebugStopReason::Step);
    assert_eq!(callee_pause.frames.len(), 2);
    assert!(matches!(
        &callee_pause.current_source,
        DirectHostSourceSpanStatus::Known(span)
            if span.document_id.as_str() == "Module1" && span.start.line > 0
    ));
    let current = callee_pause.frames.last().expect("current frame");
    assert!(current.procedure_name.eq_ignore_ascii_case("Foo"));
    assert!(matches!(
        &current.source,
        DirectHostSourceSpanStatus::Known(span)
            if span.document_id.as_str() == "Module1"
                && span.start.line > 0
                && span.end.line > span.start.line
    ));
    let y = session
        .evaluate_variant(&DebugEvaluationRequest::new("y"))
        .expect("y should be visible in callee");
    assert_eq!(y.value.variant_value.as_i32(), Some(4));
    assert_eq!(y.value.kind, DebugFrameValueKind::Parameter);
    let y_slot = current
        .values
        .iter()
        .find(|value| value.name.eq_ignore_ascii_case("y"))
        .expect("y frame value")
        .slot;
    let y_variant = session.runtime().read_variant_slot(y_slot);
    assert_eq!(y_variant.vtype(), VarType::Long);
    assert_eq!(y_variant.as_i32(), Some(4));
}

#[test]
fn debug_session_exposes_variant_frames_and_identifier_evaluation_before_projection() {
    let manifest = make_manifest(
        "Sub Main()\n\
         Call Foo(4)\n\
         End Sub\n\
         \n\
         Sub Foo(ByVal y As Long)\n\
         Dim z As Long\n\
         z = y + 1\n\
         End Sub",
    );
    let mut session = prepare_debug_session(&manifest);

    let HostDebugVariantRunResult::Paused(entry_pause) = session
        .start_variants()
        .expect("debug variant start should pause")
    else {
        panic!("expected entry pause");
    };
    assert_eq!(entry_pause.stop.reason, DebugStopReason::Entry);
    let HostDebugVariantRunResult::Paused(callee_pause) = session
        .step_into_variants()
        .expect("variant step into should pause in callee")
    else {
        panic!("expected callee pause");
    };
    let current = callee_pause.frames.last().expect("current frame");
    let y = session
        .evaluate_variant(&DebugEvaluationRequest::new("y"))
        .expect("y should be visible in callee");
    assert_eq!(y.value.variant_value.as_i32(), Some(4));
    assert_eq!(y.value.kind, DebugFrameValueKind::Parameter);
    assert!(current.values.iter().any(|value| {
        value.name.eq_ignore_ascii_case("y") && value.variant_value.as_i32() == Some(4)
    }));
}

#[test]
fn debug_session_watch_registry_reports_unavailable_error_and_value_states() {
    let manifest = make_manifest(
        "Sub Main()\n\
         Call Foo(4)\n\
         End Sub\n\
         \n\
         Sub Foo(ByVal y As Long)\n\
         Dim z As Long\n\
         z = y + 1\n\
         End Sub",
    );
    let mut session = prepare_debug_session(&manifest);
    let watch = session.add_watch("y");
    assert!(watch.watch_id.as_str().contains(":watch:1"));

    let before_start = session.evaluate_watches();
    assert_eq!(before_start.len(), 1);
    assert!(matches!(
        &before_start[0].status,
        DebugWatchEvaluationStatus::Unavailable(issue)
            if issue.stable_code == "DH-NOT-PAUSED"
    ));
    assert!(matches!(
        before_start[0].source,
        DirectHostSourceSpanStatus::Unavailable(
            DirectHostSourceUnavailableReason::NoSourceLocation
        )
    ));

    let HostDebugVariantRunResult::Paused(_) = session.start_variants().expect("entry pause")
    else {
        panic!("expected entry pause");
    };
    let entry_eval = session.evaluate_watches();
    assert!(matches!(
        &entry_eval[0].status,
        DebugWatchEvaluationStatus::Error(issue)
            if issue.stable_code == "DH-WATCH-EVALUATION-FAILED"
    ));
    assert!(matches!(
        &entry_eval[0].source,
        DirectHostSourceSpanStatus::Known(span)
            if span.document_id.as_str() == "Module1"
                && span.start.line == 2
                && span.end.line == 3
    ));

    let HostDebugVariantRunResult::Paused(_) = session.step_into_variants().expect("callee pause")
    else {
        panic!("expected callee pause");
    };
    let values = session.evaluate_watches();
    assert!(matches!(
        &values[0].status,
        DebugWatchEvaluationStatus::Value(value)
            if value.name.eq_ignore_ascii_case("y")
                && value.variant_value.as_i32() == Some(4)
    ));
    assert!(matches!(
        &values[0].source,
        DirectHostSourceSpanStatus::Known(span)
            if span.document_id.as_str() == "Module1" && span.start.line > 0
    ));

    let updated = session
        .update_watch(&watch.watch_id, "missing")
        .expect("update watch");
    assert_eq!(updated.expression_text, "missing");
    assert!(matches!(
        &session.evaluate_watches()[0].status,
        DebugWatchEvaluationStatus::Error(issue)
            if issue.stable_code == "DH-WATCH-EVALUATION-FAILED"
    ));
    assert!(matches!(
        &session.evaluate_watches()[0].source,
        DirectHostSourceSpanStatus::Known(span)
            if span.document_id.as_str() == "Module1" && span.start.line > 0
    ));
    let removed = session.remove_watch(&watch.watch_id).expect("remove watch");
    assert_eq!(removed.watch_id, watch.watch_id);
    assert!(session.watches().is_empty());
}

#[test]
fn debug_session_breakpoint_records_bind_disable_clear_and_count_hits() {
    let manifest = make_manifest(
        "Sub Main()\n\
         Dim x As Long\n\
         x = 1\n\
         End Sub",
    );
    let mut session = prepare_debug_session(&manifest);

    let missing = session.set_source_breakpoint("Missing", 2);
    assert_eq!(
        missing.binding_status,
        DebugBreakpointBindingStatus::Unbound
    );
    assert_eq!(
        missing.unresolved_reason,
        Some(DebugBreakpointUnresolvedReason::NoMatchingModule)
    );
    assert!(matches!(
        missing.source,
        DirectHostSourceSpanStatus::Unavailable(
            DirectHostSourceUnavailableReason::NoMatchingDocument
        )
    ));
    let invalid_line = session.set_source_breakpoint("Module1", 99);
    assert_eq!(
        invalid_line.binding_status,
        DebugBreakpointBindingStatus::Unbound
    );
    assert_eq!(
        invalid_line.unresolved_reason,
        Some(DebugBreakpointUnresolvedReason::NoExecutableStatementOnLine)
    );
    assert!(matches!(
        &invalid_line.source,
        DirectHostSourceSpanStatus::Known(span)
            if span.document_id.as_str() == "Module1"
                && span.start.line == 99
                && span.end.line == 100
    ));

    let bound = session.set_source_breakpoint("Module1", 3);
    assert_eq!(bound.binding_status, DebugBreakpointBindingStatus::Bound);
    assert!(bound.unresolved_reason.is_none());
    assert!(bound.enabled);
    assert!(bound.breakpoint_id.as_str().contains(":breakpoint:3"));
    assert!(matches!(
        &bound.source,
        DirectHostSourceSpanStatus::Known(span)
            if span.document_id.as_str() == "Module1"
                && span.start.line == 3
                && span.end.line == 4
    ));

    let HostDebugVariantRunResult::Paused(entry_pause) =
        session.start_variants().expect("entry pause")
    else {
        panic!("expected entry pause");
    };
    assert_eq!(entry_pause.stop.reason, DebugStopReason::Entry);
    assert!(
        entry_pause.frames[0]
            .frame_id
            .as_str()
            .contains(":frame:1:")
    );
    assert!(matches!(
        &entry_pause.current_source,
        DirectHostSourceSpanStatus::Known(span)
            if span.document_id.as_str() == "Module1"
                && span.start.line == 2
                && span.end.line == 3
    ));
    let _ = session
        .continue_execution_variants()
        .expect("continuing with a bound breakpoint should be valid");
    let bound_after_continue = session
        .source_breakpoints()
        .iter()
        .find(|record| record.breakpoint_id == bound.breakpoint_id)
        .expect("bound breakpoint");
    assert_eq!(
        bound_after_continue.binding_status,
        DebugBreakpointBindingStatus::Bound
    );

    let disabled = session
        .set_breakpoint_enabled(&bound.breakpoint_id, false)
        .expect("disable breakpoint");
    assert!(!disabled.enabled);
    let cleared = session
        .clear_source_breakpoint(&bound.breakpoint_id)
        .expect("clear breakpoint");
    assert_eq!(cleared.breakpoint_id, bound.breakpoint_id);
}

#[test]
fn debug_session_pause_state_is_absent_before_start_and_after_completion() {
    let manifest = make_manifest("Sub Main()\nEnd Sub");
    let mut session = prepare_debug_session(&manifest);
    assert_eq!(
        session
            .current_variant_pause_state()
            .expect("pause query should succeed"),
        None
    );
    assert!(matches!(
        session
            .start_variants()
            .expect("debug start should complete"),
        HostDebugVariantRunResult::Completed
    ));
    assert_eq!(
        session
            .current_variant_pause_state()
            .expect("pause query should succeed"),
        None
    );
}

#[test]
fn debug_session_rejects_non_identifier_and_unknown_name_evaluation() {
    let manifest = make_manifest("Sub Main()\nDim answer As Long\nanswer = 42\nEnd Sub");
    let mut session = prepare_debug_session(&manifest);
    let HostDebugVariantRunResult::Paused(_) =
        session.start_variants().expect("debug start should pause")
    else {
        panic!("expected entry pause");
    };
    let unsupported = session
        .evaluate_variant(&DebugEvaluationRequest::new("answer + 1"))
        .expect_err("non-identifier expression should be rejected");
    assert!(matches!(
        unsupported,
        DebugSessionError::UnsupportedEvaluation { .. }
    ));
    let unknown = session
        .evaluate_variant(&DebugEvaluationRequest::new("missingValue"))
        .expect_err("unknown name should be rejected");
    assert!(matches!(
        unknown,
        DebugSessionError::UnknownVisibleName { .. }
    ));
}
