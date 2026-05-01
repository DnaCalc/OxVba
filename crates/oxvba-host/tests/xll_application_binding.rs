#![cfg(target_os = "windows")]

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_host::{Engine, HostConfig, compat::RuntimeValueCompatEngineExt};
use oxvba_runtime::compat::RuntimeValue;

#[test]
fn bound_native_host_object_is_consumed_by_project_createobject() {
    let mut engine = Engine::new(HostConfig::default());
    let mut policy = engine.host_policy().clone();
    policy.deterministic_mode = false;
    engine.set_host_policy(policy);

    let dispatch = oxvba_com::create_oxvba_test_dispatch();
    let bound = unsafe {
        engine.bind_native_dispatch_object(
            oxvba_com::OXVBA_TEST_DISPATCH_PROGID,
            dispatch.cast::<core::ffi::c_void>(),
        )
    }
    .expect("native host object should bind");

    let module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nDim obj As Object\nDim afterCount\nSet obj = CreateObject(\"OxVba.TestDispatch\")\nafterCount = DispatchInvoke(obj, \"Count\")\nEnd Sub",
    )
    .expect("module should parse");
    let manifest = ProjectManifest {
        project_name: "XllApplicationBinding".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![module],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: Default::default(),
    };

    let snapshot = engine
        .execute_project_with_value_snapshot_phased(&manifest)
        .expect("project should consume bound object through CreateObject");

    match snapshot.as_slice() {
        [RuntimeValue::Object(object), RuntimeValue::I32(count)]
            if object.raw() == bound.raw() && *count == 7 => {}
        other => panic!(
            "expected CreateObject to return the pre-bound native host object {} and native Count witness 7, got {other:?}",
            bound.raw()
        ),
    }
}
