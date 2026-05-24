use oxvba_compiler::ModuleKind;
use oxvba_host::{
    HostCallContext, ProjectModuleText, ProjectSource, TypedValue, UdfAdmissionPolicy, VbaHost,
};

fn source_project(text: &str) -> ProjectSource {
    ProjectSource::ModuleTexts(vec![ProjectModuleText {
        name_hint: Some("CalcModule".to_string()),
        kind_hint: Some(ModuleKind::Procedural),
        text: text.to_string(),
    }])
}

#[test]
fn dnaonecalc_style_host_loads_reflects_admits_and_invokes_without_registry_mirror() {
    let host = VbaHost::default();
    let loaded = host
        .load_project(source_project(
            "Public Function Add(a As Long, b As Long) As Long\nAdd = a + b\nEnd Function",
        ))
        .expect("load project");

    let policy_report = UdfAdmissionPolicy::default().admit(loaded.reflection());
    assert_eq!(policy_report.admitted.len(), 1);
    let admitted = &policy_report.admitted[0].registration;
    assert_eq!(admitted.callable_metadata.public_name, "add");

    let mut prepared = loaded.prepare().expect("prepare");
    let result = prepared
        .invoke_callable_typed(
            &admitted.source_identity.callable_id,
            HostCallContext::default(),
            &[TypedValue::Long(20), TypedValue::Long(22)],
        )
        .expect("typed invoke");
    assert_eq!(result.value, TypedValue::Long(42));

    // The example intentionally keeps only an admitted request, not an OxVba-owned
    // formula registry or name-precedence mirror.
    assert_eq!(policy_report.admitted.len(), 1);
}

#[test]
fn oxide_style_host_inspects_inventory_without_preparing_execution() {
    let host = VbaHost::default();
    let loaded = host
        .load_project(source_project(
            "Public Function DescribeMe() As String\nDescribeMe = \"ok\"\nEnd Function",
        ))
        .expect("load project");

    let inventory = loaded
        .reflection()
        .procedures
        .iter()
        .map(|procedure| format!("{}.{}", procedure.module_name, procedure.procedure_name))
        .collect::<Vec<_>>();

    assert_eq!(inventory, vec!["CalcModule.describeme".to_string()]);
    assert!(loaded.diagnostics().is_empty());
}

#[test]
fn descriptor_fingerprint_supports_host_cache_invalidation() {
    let host = VbaHost::default();
    let first = host
        .load_project(source_project(
            "Public Function Add(a As Long) As Long\nAdd = a\nEnd Function",
        ))
        .expect("first load");
    let second = host
        .load_project(source_project(
            "Public Function Add(a As Double) As Double\nAdd = a\nEnd Function",
        ))
        .expect("second load");

    assert_ne!(
        first.reflection().procedures[0].descriptor_fingerprint,
        second.reflection().procedures[0].descriptor_fingerprint
    );
}
