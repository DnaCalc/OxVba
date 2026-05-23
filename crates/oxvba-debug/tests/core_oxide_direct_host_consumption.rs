use std::{collections::BTreeMap, sync::Arc};

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_debug::{
    DebugCoreConfig, DebugEvaluationRequest, HostDebugVariantRunResult, prepare_debug_session_core,
};
use oxvba_host::{Engine, HostConfig};
use oxvba_runtime::Variant;
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
fn oxide_direct_debug_seam_consumes_variant_pause_and_eval_without_cli_lsp_or_placeholder() {
    let manifest = make_manifest(
        "Sub Main()
         Call Foo(4)
         End Sub
         
         Sub Foo(ByVal y As Long)
         Dim z As Long
         z = y + 1
         End Sub",
    );
    let engine = Arc::new(Engine::new(HostConfig::default()));
    let mut debug =
        prepare_debug_session_core(engine, manifest.clone(), DebugCoreConfig::default())
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
