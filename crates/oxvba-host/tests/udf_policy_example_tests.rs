use oxvba_compiler::ModuleKind;
use oxvba_host::{ProjectModuleText, ProjectSource, UdfAdmissionPolicy, VbaHost};

fn load(source: &str) -> oxvba_host::LoadedVbaProject {
    VbaHost::default()
        .load_project(ProjectSource::ModuleTexts(vec![ProjectModuleText {
            name_hint: Some("Main".to_string()),
            kind_hint: Some(ModuleKind::Procedural),
            text: source.to_string(),
        }]))
        .expect("load")
}

#[test]
fn host_owned_policy_admits_public_functions_and_projects_w093_shape() {
    let loaded =
        load("Public Function Add(a As Long, b As Long) As Long\nAdd = a + b\nEnd Function");
    let report = UdfAdmissionPolicy::default().admit(loaded.reflection());

    assert_eq!(report.admitted.len(), 1);
    let request = &report.admitted[0].registration;
    assert!(!request.source_identity.callable_id.is_empty());
    assert_eq!(request.callable_metadata.public_name, "add");
    assert_eq!(request.callable_metadata.parameter_count, 2);
    assert_eq!(
        request.invocation_target.conversion_lane,
        "TypedScalarFirstTier"
    );
    assert_eq!(
        request.capability.policy_name,
        "example-host-owned-udf-admission"
    );
    assert!(
        request
            .change_facts
            .contains(&"host-udf-policy".to_string())
    );
}

#[test]
fn host_owned_policy_rejects_non_admitted_shapes() {
    let host = VbaHost::default();
    let loaded = host
        .load_project(ProjectSource::ModuleTexts(vec![
            ProjectModuleText {
                name_hint: Some("Main".to_string()),
                kind_hint: Some(ModuleKind::Procedural),
                text: concat!(
                    "Public Sub Helper()\nEnd Sub\n",
                    "Private Function Hidden() As Long\nHidden = 1\nEnd Function\n",
                    "Public Function Unsupported(x As Variant) As Variant\nUnsupported = x\nEnd Function"
                )
                .to_string(),
            },
            ProjectModuleText {
                name_hint: Some("Widget".to_string()),
                kind_hint: Some(ModuleKind::Class),
                text: "Public Function ClassAdd(a As Long) As Long\nClassAdd = a\nEnd Function".to_string(),
            },
        ]))
        .expect("load");

    let report = UdfAdmissionPolicy::default().admit(loaded.reflection());
    assert!(report.admitted.is_empty());
    let codes = report
        .rejected
        .iter()
        .map(|item| item.reason_code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"POLICY-NOT-FUNCTION"));
    assert!(codes.contains(&"POLICY-NOT-PUBLIC"));
    assert!(codes.contains(&"POLICY-CLASS-MEMBER"));
    assert!(codes.contains(&"POLICY-RETURN-TYPE"));
}

#[test]
fn changing_host_policy_changes_admission_without_changing_descriptors() {
    let host = VbaHost::default();
    let loaded = host
        .load_project(ProjectSource::ModuleTexts(vec![ProjectModuleText {
            name_hint: Some("PrivateMod".to_string()),
            kind_hint: Some(ModuleKind::Procedural),
            text: "Option Private Module\nPublic Function Add(a As Long) As Long\nAdd = a\nEnd Function".to_string(),
        }]))
        .expect("load");
    let before_fingerprint = loaded.reflection().procedures[0]
        .descriptor_fingerprint
        .clone();

    let default_report = UdfAdmissionPolicy::default().admit(loaded.reflection());
    assert!(default_report.admitted.is_empty());

    let permissive = UdfAdmissionPolicy {
        allow_option_private_modules: true,
        ..Default::default()
    };
    let permissive_report = permissive.admit(loaded.reflection());
    assert_eq!(permissive_report.admitted.len(), 1);
    assert_eq!(
        loaded.reflection().procedures[0].descriptor_fingerprint,
        before_fingerprint
    );
}
