use std::fs;

use oxvba_compiler::{ModuleKind, OxBundle, ProjectKind, ProjectManifest, compile_project};
use oxvba_host::{
    HostCallContext, HostCaller, HostContextValue, HostDiagnosticPhase, ProjectFile,
    ProjectFileSet, ProjectModuleText, ProjectSource, TypedValue, VbaHost, VbaHostOptions,
};
use oxvba_runtime::{Variant, bstr::BStr};

#[test]
fn vba_host_loads_in_memory_reflects_before_prepare_and_invokes_after_prepare() {
    let host = VbaHost::new(VbaHostOptions::default());
    let loaded = host
        .load_project(ProjectSource::ModuleTexts(vec![ProjectModuleText {
            name_hint: Some("Math".to_string()),
            kind_hint: Some(ModuleKind::Procedural),
            text: "Public Function Add(ByVal a As Long, ByVal b As Long) As Long\nAdd = a + b\nEnd Function".to_string(),
        }]))
        .expect("load text project");

    let add = loaded
        .reflection()
        .procedures
        .iter()
        .find(|procedure| procedure.procedure_name == "add")
        .expect("reflection before prepare");
    assert_eq!(add.module_name, "Math");
    assert_eq!(add.signature.parameters.len(), 2);

    let mut prepared = loaded.prepare().expect("prepare");
    assert_eq!(
        prepared.reflection().procedures.len(),
        loaded.reflection().procedures.len()
    );
    let result = prepared
        .invoke_by_name_variant("Math", "Add", &[Variant::from_i32(2), Variant::from_i32(5)])
        .expect("invoke");
    assert_eq!(result, Variant::from_i32(7));
}

#[test]
fn vba_host_invokes_by_callable_id_with_context_observation_and_typed_lane() {
    let host = VbaHost::default();
    let loaded = host
        .load_project(ProjectSource::ModuleTexts(vec![ProjectModuleText {
            name_hint: Some("Math".to_string()),
            kind_hint: None,
            text: "Public Function Add(ByVal a As Long, ByVal b As Long) As Long\nAdd = a + b\nEnd Function".to_string(),
        }]))
        .expect("load");
    let callable_id = loaded.reflection().procedures[0].callable_id.clone();
    let mut prepared = loaded.prepare().expect("prepare");
    let mut metadata = std::collections::BTreeMap::new();
    metadata.insert(
        "trace".to_string(),
        HostContextValue::String("abc".to_string()),
    );
    let context = HostCallContext {
        caller: Some(HostCaller {
            source_system: "test-harness".to_string(),
            display_text: Some("A1".to_string()),
            stable_id: Some("cell-a1".to_string()),
            metadata: Default::default(),
        }),
        locale_id: Some(1033),
        metadata,
    };

    let variant_result = prepared
        .invoke_callable_variant(
            &callable_id,
            context.clone(),
            &[Variant::from_i32(2), Variant::from_i32(3)],
        )
        .expect("variant callable invoke");
    assert_eq!(variant_result.value, Variant::from_i32(5));
    assert_eq!(variant_result.conversion_lane, "VariantPositional");
    assert_eq!(
        variant_result
            .context_observations
            .caller_source_system
            .as_deref(),
        Some("test-harness")
    );
    assert_eq!(
        prepared.last_context_observations().unwrap().locale_id,
        Some(1033)
    );

    let typed_result = prepared
        .invoke_callable_typed(
            &callable_id,
            context,
            &[TypedValue::Long(4), TypedValue::Long(6)],
        )
        .expect("typed callable invoke");
    assert_eq!(typed_result.value, TypedValue::Long(10));
    assert_eq!(typed_result.conversion_lane, "TypedScalarFirstTier");
}

#[test]
fn vba_host_callable_invocation_reports_structured_diagnostics() {
    let host = VbaHost::default();
    let loaded = host
        .load_project(ProjectSource::ModuleTexts(vec![ProjectModuleText {
            name_hint: Some("Math".to_string()),
            kind_hint: None,
            text: "Public Function Add(ByVal a As Long, ByVal b As Long) As Long\nAdd = a + b\nEnd Function".to_string(),
        }]))
        .expect("load");
    let callable_id = loaded.reflection().procedures[0].callable_id.clone();
    let mut prepared = loaded.prepare().expect("prepare");

    let arity = prepared
        .invoke_callable_variant(
            &callable_id,
            HostCallContext::default(),
            &[Variant::from_i32(1)],
        )
        .expect_err("arity diagnostic");
    assert_eq!(arity.phase, HostDiagnosticPhase::ValidateCall);
    assert_eq!(arity.code, "HOST-CALL-ARITY");

    let type_error = prepared
        .invoke_callable_typed(
            &callable_id,
            HostCallContext::default(),
            &[TypedValue::String("bad".to_string()), TypedValue::Long(1)],
        )
        .expect_err("type diagnostic");
    assert_eq!(type_error.phase, HostDiagnosticPhase::ValidateCall);
    assert_eq!(type_error.code, "HOST-CALL-TYPE");

    let missing = prepared
        .invoke_callable_variant("missing", HostCallContext::default(), &[])
        .expect_err("missing diagnostic");
    assert_eq!(missing.code, "HOST-CALL-NOT-FOUND");
}

#[test]
fn vba_host_loads_file_set() {
    let temp_dir = std::env::temp_dir().join(format!("oxvba-vbahost-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");
    let module_path = temp_dir.join("FileMod.bas");
    fs::write(
        &module_path,
        "Public Function Hello() As String\nHello = \"hi\"\nEnd Function",
    )
    .expect("write module");

    let host = VbaHost::default();
    let loaded = host
        .load_project(ProjectSource::FileSet(ProjectFileSet {
            project_name: "FileProject".to_string(),
            files: vec![ProjectFile {
                path: module_path,
                module_name: None,
                module_kind: Some(ModuleKind::Procedural),
            }],
        }))
        .expect("load file set");

    assert_eq!(loaded.reflection().identity.project_name, "FileProject");
    assert!(
        loaded
            .reflection()
            .procedures
            .iter()
            .any(|procedure| procedure.procedure_name == "hello")
    );
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn vba_host_loads_bundle_bytes_and_invokes_after_prepare() {
    let manifest = ProjectManifest {
        project_name: "BundleProject".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![
            oxvba_compiler::module_unit_from_source(
                "BundleMod",
                ModuleKind::Procedural,
                "Public Function Echo() As String\nEcho = \"bundle\"\nEnd Function",
            )
            .expect("module"),
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: Default::default(),
    };
    let compiled = compile_project(&manifest).expect("compile");
    let bundle = OxBundle::from_compiled_project(&compiled, &manifest.project_name);
    let bytes = bundle.serialize_to_bytes().expect("serialize bundle");

    let host = VbaHost::default();
    let loaded = host.load_bundle(&bytes).expect("load bundle");
    assert!(
        loaded
            .reflection()
            .procedures
            .iter()
            .any(|procedure| procedure.procedure_name == "echo")
    );

    let mut prepared = loaded.prepare().expect("prepare bundle");
    let result = prepared
        .invoke_by_name_variant("BundleMod", "Echo", &[])
        .expect("invoke bundle");
    assert_eq!(result, Variant::from_string(BStr::from("bundle")));
}

#[test]
fn vba_host_loaded_projects_remain_isolated() {
    let host = VbaHost::default();
    let left = host
        .load_project(ProjectSource::ModuleTexts(vec![ProjectModuleText {
            name_hint: Some("LeftMod".to_string()),
            kind_hint: None,
            text: "Public Function Value() As Long\nValue = 11\nEnd Function".to_string(),
        }]))
        .expect("load left");
    let right = host
        .load_project(ProjectSource::ModuleTexts(vec![ProjectModuleText {
            name_hint: Some("RightMod".to_string()),
            kind_hint: None,
            text: "Public Function Value() As Long\nValue = 22\nEnd Function".to_string(),
        }]))
        .expect("load right");

    assert_ne!(
        left.reflection().procedures[0].module_id,
        right.reflection().procedures[0].module_id
    );
    let mut left_prepared = left.prepare().expect("prepare left");
    let mut right_prepared = right.prepare().expect("prepare right");
    assert_eq!(
        left_prepared
            .invoke_by_name_variant("LeftMod", "Value", &[])
            .expect("left invoke"),
        Variant::from_i32(11)
    );
    assert_eq!(
        right_prepared
            .invoke_by_name_variant("RightMod", "Value", &[])
            .expect("right invoke"),
        Variant::from_i32(22)
    );
}
