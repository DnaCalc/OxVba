use oxvba_com::known_typelib_identity_for_prog_id_name;
use oxvba_host::{Engine, HostConfig, RuntimeProfileId};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ProjectReference;

fn test_dispatch_ref() -> ProjectReference {
    let identity =
        known_typelib_identity_for_prog_id_name("OxVba.TestDispatch").expect("fixture typelib");

    ProjectReference::TypeLibrary {
        name: identity.reference_name,
        guid: identity.libid,
        version_major: Some(identity.major_version),
        version_minor: Some(identity.minor_version),
        lcid: identity.lcid,
        import_lib: Some(identity.importlib),
    }
}

fn long_at(values: &[Variant], index: usize) -> i32 {
    values
        .get(index)
        .and_then(Variant::as_i32)
        .unwrap_or_else(|| panic!("expected Long at snapshot slot {index}: {values:?}"))
}

#[test]
fn portable_projection_callbyname_lowers_through_typelib_metadata() {
    let source = r#"
Public result As Long
Public lateValue As Long
Public earlyValue As Long

Sub Main()
    Dim obj As OxVba.TestDispatch
    Set obj = New OxVba.TestDispatch

    lateValue = CallByName(obj, "Count", VbGet)
    earlyValue = obj.Count
    result = lateValue - earlyValue
End Sub
"#;

    let engine =
        Engine::new(HostConfig::vm3()).with_runtime_profile(RuntimeProfileId::WindowsHeadless);
    let values = engine
        .execute_source_with_references_and_snapshot(source, vec![test_dispatch_ref()])
        .expect("portable COM projection should support CallByName by typelib metadata");

    let result = long_at(&values, 0);
    let late_value = long_at(&values, 1);
    let early_value = long_at(&values, 2);

    assert_eq!(
        result, 0,
        "CallByName and direct property dispatch should agree"
    );
    assert!(late_value > 0, "projection fixture should return a value");
    assert_eq!(late_value, early_value);
}
