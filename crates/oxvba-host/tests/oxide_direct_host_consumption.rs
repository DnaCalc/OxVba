use std::collections::BTreeMap;

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_host::{
    DebugEvaluationRequest, Engine, HostConfig, HostDebugVariantRunResult,
    ImmediateEvaluationRequest, ImmediateSession, ImmediateVariantEvaluationOutput,
};
use oxvba_runtime::{VarType, Variant};
use oxvba_vm::DebugStopReason;

fn make_manifest(source: &str) -> ProjectManifest {
    ProjectManifest {
        project_name: "OxIdeDirectHost".to_string(),
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

#[test]
fn oxide_direct_immediate_window_consumes_live_session_without_cli_lsp_or_placeholder() {
    let manifest = make_manifest(
        r#"
Dim lastText As String
Dim counter As Integer

Public Function EchoAndCount(ByVal value As String) As String
    counter = counter + 1
    lastText = value
    EchoAndCount = value
End Function

Public Function GetCounter() As Integer
    GetCounter = counter
End Function
"#,
    );
    let engine = Engine::new(HostConfig::default());
    let runtime = engine
        .compile_and_prepare_session(&manifest)
        .expect("runtime session");
    let mut immediate = ImmediateSession::new(&engine, manifest, runtime);
    immediate.set_default_target_module(Some("Module1"));

    let first = immediate
        .evaluate_variant(&ImmediateEvaluationRequest::query(
            r#"EchoAndCount("alpha")"#,
        ))
        .expect("first immediate evaluation");
    let ImmediateVariantEvaluationOutput::Value(first_value) = first.output else {
        panic!("expected retained Variant value from direct immediate evaluation");
    };
    assert_eq!(first_value.variant_value.vtype(), VarType::String);
    assert_eq!(first_value.variant_value.as_bstr(), Some("alpha".into()));
    assert_eq!(first_value.display_text, "alpha");

    let second = immediate
        .evaluate_variant(&ImmediateEvaluationRequest::query("GetCounter()"))
        .expect("counter evaluation");
    let ImmediateVariantEvaluationOutput::Value(counter_value) = second.output else {
        panic!("expected retained Variant counter value");
    };
    assert_eq!(counter_value.variant_value.as_i32(), Some(1));

    let snapshot = immediate.snapshot_variants();
    assert!(snapshot.iter().any(|value| {
        value.vtype() == VarType::String && value.as_bstr() == Some("alpha".into())
    }));
}

#[test]
fn oxide_direct_debug_seam_consumes_variant_pause_and_eval_without_cli_lsp_or_placeholder() {
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
    let engine = Engine::new(HostConfig::default());
    let mut debug = engine
        .prepare_debug_session(&manifest)
        .expect("debug session");

    let HostDebugVariantRunResult::Paused(entry_pause) =
        debug.start_variants().expect("entry pause")
    else {
        panic!("expected entry pause");
    };
    assert_eq!(entry_pause.stop.reason, DebugStopReason::Entry);
    assert_eq!(entry_pause.frames.len(), 1);

    let HostDebugVariantRunResult::Paused(callee_pause) =
        debug.step_into_variants().expect("callee pause")
    else {
        panic!("expected callee pause");
    };
    assert_eq!(callee_pause.stop.reason, DebugStopReason::Step);
    assert_eq!(callee_pause.frames.len(), 2);
    let current = callee_pause.frames.last().expect("current frame");
    assert!(current.procedure_name.eq_ignore_ascii_case("Foo"));
    assert!(current.values.iter().any(|value| {
        value.name.eq_ignore_ascii_case("y") && value.variant_value.as_i32() == Some(4)
    }));

    let evaluated = debug
        .evaluate_variant(&DebugEvaluationRequest::new("? y"))
        .expect("paused y evaluation");
    assert_eq!(evaluated.value.variant_value, Variant::from_i32(4));
    assert_eq!(evaluated.value.display_text, "4");
}
