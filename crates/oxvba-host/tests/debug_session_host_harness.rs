use std::collections::BTreeMap;

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_host::{
    DebugEvaluationRequest, DebugFrameValueKind, Engine, HostConfig, HostDebugRunResult,
};
use oxvba_runtime::RuntimeValue;
use oxvba_vm::DebugStopReason;

fn make_manifest(source: &str) -> ProjectManifest {
    ProjectManifest {
        project_name: "DebugHarness".to_string(),
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
fn direct_host_debug_harness_reports_pause_frames_and_evaluation_transcript() {
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
    let mut session = engine
        .prepare_debug_session(&manifest)
        .expect("debug session should prepare");

    let mut transcript = Vec::<String>::new();

    let HostDebugRunResult::Paused(entry_pause) =
        session.start().expect("debug start should pause on entry")
    else {
        panic!("expected entry pause");
    };
    transcript.push(format!(
        "start:{}:{}:{}:{}",
        match entry_pause.stop.reason {
            DebugStopReason::Entry => "entry",
            DebugStopReason::Breakpoint => "breakpoint",
            DebugStopReason::Step => "step",
        },
        entry_pause.stop.location.module_name,
        entry_pause.stop.location.procedure_name,
        entry_pause.stop.location.line_number.unwrap_or_default()
    ));

    let HostDebugRunResult::Paused(callee_pause) =
        session.step_into().expect("step into should pause in callee")
    else {
        panic!("expected callee pause");
    };
    transcript.push(format!(
        "step:{}:{}:{}",
        callee_pause.frames.len(),
        callee_pause.frames[0].procedure_name,
        callee_pause.frames.last().expect("current frame").procedure_name
    ));

    let value = session
        .evaluate(&DebugEvaluationRequest::new("y"))
        .expect("y should evaluate while paused");
    transcript.push(format!("eval:y={}", value.value.display_text));

    let current_frame = callee_pause.frames.last().expect("current frame");
    let z = current_frame
        .values
        .iter()
        .find(|value| value.name.eq_ignore_ascii_case("z"))
        .expect("z local should be present");
    transcript.push(format!(
        "local:{}:{}",
        z.name,
        match z.kind {
            DebugFrameValueKind::Parameter => "param",
            DebugFrameValueKind::Local => "local",
            DebugFrameValueKind::ReturnValue => "return",
        }
    ));

    let completion = session
        .step_out()
        .expect("step out should finish bounded sample");
    transcript.push(match completion {
        HostDebugRunResult::Paused(pause) => format!(
            "step_out:paused:{}:{}",
            pause.frames.len(),
            pause.stop.location.procedure_name
        ),
        HostDebugRunResult::Completed => "step_out:completed".to_string(),
    });

    assert_eq!(
        transcript,
        vec![
            "start:entry:module1:main:2".to_string(),
            "step:2:main:foo".to_string(),
            "eval:y=4".to_string(),
            "local:z:local".to_string(),
            "step_out:completed".to_string(),
        ]
    );
    assert_eq!(value.value.runtime_value, RuntimeValue::I32(4));
}

#[test]
fn direct_host_debug_harness_reports_completed_session_without_pause_state() {
    let manifest = make_manifest("Sub Main()\nEnd Sub");
    let engine = Engine::new(HostConfig::default());
    let mut session = engine
        .prepare_debug_session(&manifest)
        .expect("debug session should prepare");

    let result = session.start().expect("empty mainline debug start should finish");
    assert!(matches!(result, HostDebugRunResult::Completed));
    assert_eq!(
        session
            .current_pause_state()
            .expect("pause state query should succeed"),
        None
    );
}
