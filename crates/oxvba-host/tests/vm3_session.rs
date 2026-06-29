//! W8-c: the vm3-backed host session (`Vm3RuntimeSession`) built from an `OxImage` via
//! `Engine::prepare_image_session` — the path the in-process COM server uses to run on vm3.
//! Mirrors the vm2 `package_session_events` flow (bind a project, then create/invoke class
//! members), but elaborates to OxIR + links on vm3 instead of linearizing + linking on vm2.

use oxvba_bundle::ProjectMemberKind;
use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig};
use oxvba_oxir::OxImage;
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, SymbolProjectManifest,
};

fn class_module(name: &str, source: &str) -> ModuleUnit {
    let mut attributes = ModuleAttributes::named(name);
    attributes.vb_exposed = true;
    attributes.vb_creatable = true;
    ModuleUnit {
        module_name: name.to_string(),
        module_kind: ModuleKind::Class,
        attributes,
        source: source.to_string(),
    }
}

fn vm3_session_for(modules: Vec<ModuleUnit>) -> oxvba_host::Vm3RuntimeSession {
    let manifest = SymbolProjectManifest {
        project_name: "DemoServer".to_string(),
        project_kind: ProjectKind::Library,
        modules,
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: Default::default(),
    };
    let typelibs = oxvba_symbol::CatalogTypeLibResolver;
    let program = oxvba_bind::bind_program(&manifest, &typelibs).expect("bind project");
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate to OxIR");
    let mut engine = Engine::new(HostConfig::default());
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .prepare_image_session(OxImage::new(vec![oxp]))
        .expect("prepare vm3 image session")
}

#[test]
fn vm3_image_session_creates_and_invokes_a_class_member() {
    // The build emits an OxImage (.oxi); the COM server loads it into a Vm3RuntimeSession and
    // drives it exactly as the vm2 ProjectRuntimeSession — create a class instance, invoke a
    // member with by-value args. Widget.Twice(21) = 42, all on vm3.
    let source = r#"
Option Explicit

Public Function Twice(ByVal n As Long) As Long
    Twice = n + n
End Function

Public Property Get Name() As String
    Name = "Widget"
End Property
"#;
    let mut session = vm3_session_for(vec![class_module("Widget", source)]);

    let widget = session
        .create_class_instance("Widget")
        .expect("create Widget on vm3");
    let twice = session
        .invoke_member_values(
            widget.clone(),
            "Twice",
            Some(ProjectMemberKind::Method),
            vec![Variant::from_i32(21)],
        )
        .expect("invoke Twice");
    assert_eq!(twice.as_i32(), Some(42), "vm3-backed session create + invoke");

    let name = session
        .invoke_member_values(widget, "Name", Some(ProjectMemberKind::PropertyGet), Vec::new())
        .expect("invoke Name");
    assert_eq!(name.as_bstr().map(|s| s.as_str()), Some("Widget".to_string()));
}

#[test]
fn vm3_image_session_rejects_an_unknown_class() {
    let mut session = vm3_session_for(vec![class_module(
        "Widget",
        "Public Function Id() As Long\n    Id = 1\nEnd Function\n",
    )]);
    assert!(
        session.create_class_instance("Nope").is_err(),
        "an undeclared class is can't-create-object"
    );
}
