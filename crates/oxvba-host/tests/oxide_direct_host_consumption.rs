use std::collections::BTreeMap;

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_host::{
    Engine, HostConfig, ImmediateEvaluationRequest, ImmediateSession,
    ImmediateVariantEvaluationOutput,
};
use oxvba_runtime::VarType;

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
