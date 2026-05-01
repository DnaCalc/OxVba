use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use oxvba_compiler::{
    ModuleKind, ProjectKind, ProjectManifest, ProjectReference, ReferenceKind, compile_project,
    module_unit_from_source,
};
use oxvba_hal::model::{ComInvocationStrategy, HostPolicy};
use oxvba_host::engine::DiagnosticPhase;
use oxvba_host::{Engine, HostConfig, compat::RuntimeValueCompatEngineExt};
use oxvba_project::load_basproj;
use oxvba_runtime::{ObjectRef, RuntimeValue};

fn canonical_snapshot_objects() -> &'static Mutex<HashMap<i32, ObjectRef>> {
    static CANONICAL: OnceLock<Mutex<HashMap<i32, ObjectRef>>> = OnceLock::new();
    CANONICAL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn canonicalize_runtime_value(value: RuntimeValue) -> RuntimeValue {
    match value {
        RuntimeValue::Object(object) => {
            let raw = object.raw();
            let canonical = canonical_snapshot_objects()
                .lock()
                .expect("canonical object snapshot map should not be poisoned")
                .entry(raw)
                .or_insert_with(|| ObjectRef::from_compat_identity(raw))
                .clone();
            RuntimeValue::Object(canonical)
        }
        RuntimeValue::ArrayIntent(array) => {
            let array = match array.elements() {
                Some(elements) => array
                    .replace_elements(
                        elements
                            .into_iter()
                            .map(canonicalize_runtime_value)
                            .collect(),
                    )
                    .expect("canonical snapshot array rewrite should preserve SAFEARRAY shape"),
                None => array,
            };
            RuntimeValue::ArrayIntent(array)
        }
        other => other,
    }
}

fn canonicalize_snapshot(values: Vec<RuntimeValue>) -> Vec<RuntimeValue> {
    values.into_iter().map(canonicalize_runtime_value).collect()
}

fn manifest_with_reference(referenced_project_name: &str, main_source: &str) -> ProjectManifest {
    let main_module = module_unit_from_source("MainModule", ModuleKind::Procedural, main_source)
        .expect("main module should parse");
    ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module],
        references: vec![ProjectReference {
            referenced_project_name: referenced_project_name.to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    }
}

fn manifest_with_typelib(main_source: &str) -> ProjectManifest {
    manifest_with_reference("OxVba", main_source)
}

#[cfg(target_os = "windows")]
fn run_project_windows_hosted(manifest: &ProjectManifest, enable_jit: bool) -> Vec<RuntimeValue> {
    run_project_windows_hosted_with_policy(manifest, enable_jit, HostPolicy::interactive_dev())
}

#[cfg(target_os = "windows")]
fn run_project_windows_hosted_with_policy(
    manifest: &ProjectManifest,
    enable_jit: bool,
    policy: HostPolicy,
) -> Vec<RuntimeValue> {
    let mut engine = Engine::new(HostConfig {
        enable_jit,
        root_object_name: None,
    });
    engine.set_host_policy(policy);
    canonicalize_snapshot(
        engine
            .execute_project_with_snapshot_phased(manifest)
            .expect("project should execute"),
    )
}

#[cfg(target_os = "windows")]
fn run_project_windows_hosted_error(manifest: &ProjectManifest, enable_jit: bool) -> String {
    let mut engine = Engine::new(HostConfig {
        enable_jit,
        root_object_name: None,
    });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let err = engine
        .execute_project_with_snapshot_phased(manifest)
        .expect_err("project should fail deterministically");
    format!("{:?}: {}", err.phase(), err.message())
}

fn expect_object_handle(value: &RuntimeValue) -> ObjectRef {
    match value {
        RuntimeValue::Object(handle) => handle.clone(),
        other => panic!("expected object handle, got {:?}", other),
    }
}

fn assert_same_object_identity(values: &[RuntimeValue], indices: &[usize], context: &str) {
    let first = expect_object_handle(&values[indices[0]]);
    for index in indices.iter().copied().skip(1) {
        let next = expect_object_handle(&values[index]);
        assert_eq!(
            first, next,
            "{context}: expected identical retained ObjectRef identity at indices {indices:?}, got values={values:?}"
        );
    }
}

#[cfg(target_os = "windows")]
fn registered_scripting_dictionary_available() -> bool {
    let manifest = manifest_with_reference(
        "Scripting",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New Scripting.Dictionary
Dim countValue
countValue = obj.Count
End Sub
"#,
    );
    std::panic::catch_unwind(|| run_project_windows_hosted(&manifest, false))
        .ok()
        .and_then(|snapshot| snapshot.first().cloned())
        .map(|value| expect_object_handle(&value).raw() >= 20_001)
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn registered_scripting_filesystemobject_available() -> bool {
    let manifest = manifest_with_reference(
        "Scripting",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New Scripting.FileSystemObject
Dim extValue
extValue = obj.GetExtensionName("C:\temp\demo.txt")
End Sub
"#,
    );
    std::panic::catch_unwind(|| run_project_windows_hosted(&manifest, false))
        .ok()
        .and_then(|snapshot| snapshot.first().cloned())
        .map(|value| expect_object_handle(&value).raw() >= 20_001)
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn registered_testdispatch_available() -> bool {
    let manifest = manifest_with_reference(
        "OxVba",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj
obj = CreateObject("OxVba.TestDispatch")
End Sub
"#,
    );
    std::panic::catch_unwind(|| run_project_windows_hosted(&manifest, false))
        .ok()
        .and_then(|snapshot| snapshot.first().cloned())
        .map(|value| expect_object_handle(&value).raw() >= 20_001)
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn registered_testeventserver_available() -> bool {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestEventServer
Dim value
value = obj.Ping()
End Sub
"#,
    );
    std::panic::catch_unwind(|| run_project_windows_hosted(&manifest, false))
        .ok()
        .and_then(|snapshot| snapshot.first().cloned())
        .map(|value| expect_object_handle(&value).raw() >= 20_001)
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
struct BasprojComRefSpec<'a> {
    include: &'a str,
    guid: Option<&'a str>,
    major: Option<u16>,
    minor: Option<u16>,
    lcid: Option<u32>,
    importlib: Option<&'a str>,
}

#[cfg(target_os = "windows")]
fn load_typelib_basproj_with_ref_specs(
    temp_leaf: &str,
    main_source: &str,
    com_refs: &[BasprojComRefSpec<'_>],
) -> oxvba_project::LoadedProject {
    let temp_root = std::env::current_dir()
        .expect("cwd")
        .join("temp")
        .join(temp_leaf);
    std::fs::create_dir_all(&temp_root).expect("create temp root");

    let basproj_path = temp_root.join("ProjectA.basproj");
    let main_path = temp_root.join("Main.bas");
    std::fs::write(&main_path, main_source).expect("write main module");

    let mut basproj = String::from(
        "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n\
  <PropertyGroup>\n\
    <OutputType>Exe</OutputType>\n\
    <ProjectName>ProjectA</ProjectName>\n\
    <EntryPoint>Main.Main</EntryPoint>\n\
  </PropertyGroup>\n\
  <ItemGroup>\n\
    <Module Include=\"Main.bas\" />\n\
  </ItemGroup>\n\
  <ItemGroup>\n",
    );
    for com_ref in com_refs {
        basproj.push_str(&format!(
            "    <COMReference Include=\"{}\">\n",
            com_ref.include
        ));
        if let Some(guid) = com_ref.guid {
            basproj.push_str(&format!("      <Guid>{guid}</Guid>\n"));
        }
        if let Some(major) = com_ref.major {
            basproj.push_str(&format!("      <VersionMajor>{major}</VersionMajor>\n"));
        }
        if let Some(minor) = com_ref.minor {
            basproj.push_str(&format!("      <VersionMinor>{minor}</VersionMinor>\n"));
        }
        if let Some(lcid) = com_ref.lcid {
            basproj.push_str(&format!("      <Lcid>{lcid}</Lcid>\n"));
        }
        if let Some(importlib) = com_ref.importlib {
            basproj.push_str(&format!("      <ImportLib>{importlib}</ImportLib>\n"));
        }
        basproj.push_str("    </COMReference>\n");
    }
    basproj.push_str("  </ItemGroup>\n</Project>\n");

    std::fs::write(&basproj_path, basproj).expect("write basproj");
    load_basproj(&basproj_path).expect("basproj should load")
}

#[cfg(target_os = "windows")]
fn load_typelib_basproj_with_refs(
    temp_leaf: &str,
    main_source: &str,
    com_refs: &[(&str, &str, u16, u16, u32)],
) -> oxvba_project::LoadedProject {
    let specs = com_refs
        .iter()
        .map(|(include, guid, major, minor, lcid)| BasprojComRefSpec {
            include,
            guid: Some(*guid),
            major: Some(*major),
            minor: Some(*minor),
            lcid: Some(*lcid),
            importlib: Some(match *include {
                "OxVba" => "OxVba.TestEventServer.tlb",
                "OxVbaAlt" => "OxVba.TestEventServerAlt.tlb",
                "OxVbaAlt2" => "OxVba.TestEventServerAlt2.tlb",
                other => panic!("unexpected COMReference include `{other}`"),
            }),
        })
        .collect::<Vec<_>>();
    load_typelib_basproj_with_ref_specs(temp_leaf, main_source, &specs)
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_with_typed_declarations_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim countValue
Dim existsValue
Dim lookupValue
Dim echoValue
countValue = obj.Count()
existsValue = obj.Exists(42)
lookupValue = obj.Lookup(42)
echoValue = obj(42)
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(7),
        "Count should map through early-bind rewrite lane"
    );
    assert_eq!(
        out[2],
        RuntimeValue::Bool(true),
        "Exists(42) should map through early-bind rewrite lane"
    );
    assert_eq!(
        out[3],
        RuntimeValue::I32(1_042),
        "Lookup(42) should map through metadata-backed property-get lane"
    );
    assert_eq!(
        out[4],
        RuntimeValue::I32(42),
        "obj(42) should map through metadata-backed default-member lane"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_newenum_foreach_transport() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim item
Dim valueOut
For Each item In obj
    valueOut = valueOut & CStr(item) & ","
Next item
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[2],
        RuntimeValue::String(oxvba_runtime::bstr::BStr::from("41,42,")),
        "imported COM NewEnum VT_UNKNOWN/IEnumVARIANT transport should materialize through the runtime For Each lane"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_imported_newenum_foreach_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim item
Dim valueOut
For Each item In obj
    valueOut = valueOut & CStr(item) & ","
Next item
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported COM NewEnum/IEnumVARIANT For Each transport"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_registered_scripting_dictionary_anchor() {
    if !registered_scripting_dictionary_available() {
        return;
    }
    let manifest = manifest_with_reference(
        "Scripting",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New Scripting.Dictionary
Dim countValue
countValue = obj.Count
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(out[1], RuntimeValue::I32(0));
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_registered_scripting_dictionary_member_subset() {
    if !registered_scripting_dictionary_available() {
        return;
    }
    let manifest = manifest_with_reference(
        "Scripting",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New Scripting.Dictionary
Dim countValue
Dim existsValue
Call obj.Add("a", 1)
countValue = obj.Count
existsValue = obj.Exists("a")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(out[1], RuntimeValue::I32(1));
    assert_eq!(out[2], RuntimeValue::Bool(true));
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_registered_scripting_filesystemobject_member_subset() {
    if !registered_scripting_filesystemobject_available() {
        return;
    }
    let manifest = manifest_with_reference(
        "Scripting",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New Scripting.FileSystemObject
Dim extValue
Dim baseValue
extValue = obj.GetExtensionName("C:\temp\demo.txt")
baseValue = obj.GetBaseName("C:\temp\demo.txt")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::String(oxvba_runtime::bstr::BStr::from("txt"))
    );
    assert_eq!(
        out[2],
        RuntimeValue::String(oxvba_runtime::bstr::BStr::from("demo"))
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_registered_scripting_dictionary_member_subset_prefer_vtable_matches_dispatch()
 {
    if !registered_scripting_dictionary_available() {
        return;
    }
    let manifest = manifest_with_reference(
        "Scripting",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New Scripting.Dictionary
Dim countValue
Dim existsValue
Call obj.Add("a", 1)
countValue = obj.Count
existsValue = obj.Exists("a")
End Sub
"#,
    );

    let dispatch = run_project_windows_hosted(&manifest, false);
    let mut policy = HostPolicy::interactive_dev();
    policy.com_invocation_strategy = ComInvocationStrategy::PreferVtable;
    let vtable = run_project_windows_hosted_with_policy(&manifest, false, policy);
    assert_eq!(dispatch, vtable);
    assert!(expect_object_handle(&vtable[0]).raw() >= 20_001);
    assert_eq!(vtable[1], RuntimeValue::I32(1));
    assert_eq!(vtable[2], RuntimeValue::Bool(true));
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_registered_testdispatch_foreach_transport() {
    if !registered_testdispatch_available() {
        return;
    }
    let manifest = manifest_with_reference(
        "OxVba",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj
Dim item
Dim valueOut
obj = CreateObject("OxVba.TestDispatch")
For Each item In obj
    valueOut = valueOut & CStr(item) & ","
Next item
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[2],
        RuntimeValue::String(oxvba_runtime::bstr::BStr::from("41,42,")),
        "registered OxVba.TestDispatch For Each transport should materialize through the runtime lane"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_registered_testdispatch_foreach_vm_jit_snapshots_match() {
    if !registered_testdispatch_available() {
        return;
    }
    let manifest = manifest_with_reference(
        "OxVba",
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj
Dim item
Dim valueOut
obj = CreateObject("OxVba.TestDispatch")
For Each item In obj
    valueOut = valueOut & CStr(item) & ","
Next item
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for registered OxVba.TestDispatch For Each transport"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_registered_testeventserver_ping() {
    if !registered_testeventserver_available() {
        return;
    }
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestEventServer
Dim value
value = obj.Ping()
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(out[1], RuntimeValue::I32(42));
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_registered_testeventserver_ping_prefer_vtable_matches_dispatch() {
    if !registered_testeventserver_available() {
        return;
    }
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestEventServer
Dim value
value = obj.Ping()
End Sub
"#,
    );

    let dispatch = run_project_windows_hosted(&manifest, false);
    let mut policy = HostPolicy::interactive_dev();
    policy.com_invocation_strategy = ComInvocationStrategy::PreferVtable;
    let vtable = run_project_windows_hosted_with_policy(&manifest, false, policy);
    assert_eq!(dispatch, vtable);
    assert!(expect_object_handle(&vtable[0]).raw() >= 20_001);
    assert_eq!(vtable[1], RuntimeValue::I32(42));
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_executes_registered_testeventserver_ping() {
    if !registered_testeventserver_available() {
        return;
    }

    let loaded = load_typelib_basproj_with_refs(
        "basproj-typelib-oracle-test",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New OxVba.TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[("OxVba", "{E2A30001-0001-0001-0001-000000000001}", 1, 0, 0)],
    );
    assert!(
        loaded.manifest.references.iter().any(|reference| reference
            .referenced_project_name
            .eq_ignore_ascii_case("OxVba")),
        "expected loaded manifest to retain the OxVba typelib reference"
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("loaded basproj should compile through the OxVba typelib lane");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserver\")")
            && out.contains("value = dispatchinvoke(obj, 104)"),
        "expected loaded basproj binding to lower through the registered OxVba typelib dispatch lane, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_prefers_first_typelib_reference_for_unqualified_testeventserver() {
    let loaded = load_typelib_basproj_with_refs(
        "basproj-typelib-order-a",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            ("OxVba", "{E2A30001-0001-0001-0001-000000000001}", 1, 0, 0),
            (
                "OxVbaAlt",
                "{E2A30001-0001-0001-0001-000000000101}",
                1,
                0,
                0,
            ),
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("first reference should drive unqualified imported binding");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserver_testeventserver_ping(obj)"),
        "expected first reference to lower to the base synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_prefers_reversed_first_typelib_reference_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_refs(
        "basproj-typelib-order-b",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            (
                "OxVbaAlt",
                "{E2A30001-0001-0001-0001-000000000101}",
                1,
                0,
                0,
            ),
            ("OxVba", "{E2A30001-0001-0001-0001-000000000001}", 1, 0, 0),
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("reversed first reference should drive unqualified imported binding");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserveralt_testeventserver_ping(obj)"),
        "expected reversed first reference to lower to the alt synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_prefers_first_of_three_typelib_references_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_refs(
        "basproj-typelib-order-three-a",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            ("OxVba", "{E2A30001-0001-0001-0001-000000000001}", 1, 0, 0),
            (
                "OxVbaAlt",
                "{E2A30001-0001-0001-0001-000000000101}",
                1,
                0,
                0,
            ),
            (
                "OxVbaAlt2",
                "{E2A30001-0001-0001-0001-000000000201}",
                1,
                0,
                0,
            ),
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("first of three references should drive unqualified imported binding");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserver_testeventserver_ping(obj)"),
        "expected first of three references to lower to the base synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_prefers_middle_first_of_three_typelib_references_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_refs(
        "basproj-typelib-order-three-b",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            (
                "OxVbaAlt",
                "{E2A30001-0001-0001-0001-000000000101}",
                1,
                0,
                0,
            ),
            ("OxVba", "{E2A30001-0001-0001-0001-000000000001}", 1, 0, 0),
            (
                "OxVbaAlt2",
                "{E2A30001-0001-0001-0001-000000000201}",
                1,
                0,
                0,
            ),
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("alt first of three references should drive unqualified imported binding");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserveralt_testeventserver_ping(obj)"),
        "expected first of three references to lower to the alt synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_prefers_third_variant_when_first_of_three_typelib_references_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_refs(
        "basproj-typelib-order-three-c",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            (
                "OxVbaAlt2",
                "{E2A30001-0001-0001-0001-000000000201}",
                1,
                0,
                0,
            ),
            ("OxVba", "{E2A30001-0001-0001-0001-000000000001}", 1, 0, 0),
            (
                "OxVbaAlt",
                "{E2A30001-0001-0001-0001-000000000101}",
                1,
                0,
                0,
            ),
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("alt2 first of three references should drive unqualified imported binding");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserveralt2_testeventserver_ping(obj)"),
        "expected first of three references to lower to the alt2 synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_reports_unresolved_typelib_libid_identity() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-libid-unresolved",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[BasprojComRefSpec {
            include: "OxVbaMissing",
            guid: Some("{E2A30001-0001-0001-0001-000000009999}"),
            major: Some(1),
            minor: Some(0),
            lcid: Some(0),
            importlib: None,
        }],
    );

    let err = run_project_windows_hosted_error(&loaded.manifest, false);
    assert!(
        err.contains("PMR-E-TYPELIB-LIBID-UNRESOLVED"),
        "expected unresolved LIBID diagnostic, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_reports_unresolved_typelib_importlib_identity() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-importlib-unresolved",
        concat!(
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[BasprojComRefSpec {
            include: "OxVbaMissingFile",
            guid: None,
            major: None,
            minor: None,
            lcid: None,
            importlib: Some("C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\NoSuchTypeLib.tlb"),
        }],
    );

    let err = run_project_windows_hosted_error(&loaded.manifest, false);
    assert!(
        err.contains("PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"),
        "expected unresolved importlib diagnostic, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_mixed_broken_base_then_valid_alt_reports_unresolved_importlib() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-mixed-base-missing-alt-valid",
        concat!(
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
        ],
    );

    let err = run_project_windows_hosted_error(&loaded.manifest, false);
    assert!(
        err.contains("PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"),
        "expected unresolved importlib diagnostic, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_mixed_broken_alt_then_valid_base_reports_unresolved_importlib() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-mixed-alt-missing-base-valid",
        concat!(
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAltMissing",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
        ],
    );

    let err = run_project_windows_hosted_error(&loaded.manifest, false);
    assert!(
        err.contains("PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"),
        "expected unresolved importlib diagnostic, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_mixed_broken_base_then_valid_alt_then_valid_alt2_reports_unresolved_importlib()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-mixed-base-missing-alt-valid-alt2-valid",
        concat!(
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt2",
                guid: Some("{E2A30001-0001-0001-0001-000000000201}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt2.tlb"),
            },
        ],
    );

    let err = run_project_windows_hosted_error(&loaded.manifest, false);
    assert!(
        err.contains("PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"),
        "expected unresolved importlib diagnostic, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_mixed_broken_alt2_then_valid_base_then_valid_alt_reports_unresolved_importlib()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-mixed-alt2-missing-base-valid-alt-valid",
        concat!(
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAlt2Missing",
                guid: Some("{E2A30001-0001-0001-0001-000000000201}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt2.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
        ],
    );

    let err = run_project_windows_hosted_error(&loaded.manifest, false);
    assert!(
        err.contains("PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"),
        "expected unresolved importlib diagnostic, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_broken_base_then_valid_alt_qualified_target_resolves_alt_binding() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-broken-base-valid-alt-qualified-alt",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As OxVba_TestEventServerAlt.TestEventServer\n",
            "Dim value\n",
            "Set obj = New OxVba_TestEventServerAlt.TestEventServer\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest).expect(
        "qualified later-valid alt reference should compile despite earlier broken reference",
    );
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserveralt\")"),
        "expected qualified later-valid alt binding to lower to the alt ProgID, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_broken_alt_then_valid_base_qualified_target_resolves_base_binding() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-broken-alt-valid-base-qualified-base",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As OxVba_TestEventServer.TestEventServer\n",
            "Dim value\n",
            "Set obj = New OxVba_TestEventServer.TestEventServer\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAltMissing",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest).expect(
        "qualified later-valid base reference should compile despite earlier broken reference",
    );
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserver\")"),
        "expected qualified later-valid base binding to lower to the base ProgID, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_valid_base_then_broken_alt_resolves_qualified_base_binding() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-valid-base-broken-alt-qualified-base",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As OxVba_TestEventServer.TestEventServer\n",
            "Dim value\n",
            "Set obj = New OxVba_TestEventServer.TestEventServer\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAltMissing",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt.tlb",
                ),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("qualified valid base reference should compile despite later broken reference");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserver\")"),
        "expected qualified valid base binding to lower to the base ProgID, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_valid_alt_then_broken_base_resolves_qualified_alt_binding() {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-valid-alt-broken-base-qualified-alt",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As OxVba_TestEventServerAlt.TestEventServer\n",
            "Dim value\n",
            "Set obj = New OxVba_TestEventServerAlt.TestEventServer\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("qualified valid alt reference should compile despite later broken reference");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserveralt\")"),
        "expected qualified valid alt binding to lower to the alt ProgID, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_valid_base_then_broken_alt_prefers_base_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-valid-base-broken-alt-unqualified",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAltMissing",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt.tlb",
                ),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest).expect(
        "valid first reference should drive unqualified binding despite later broken reference",
    );
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserver_testeventserver_ping(obj)"),
        "expected unqualified valid-first binding to lower to the base synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_valid_alt_then_broken_base_prefers_alt_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-valid-alt-broken-base-unqualified",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest).expect(
        "valid first alt reference should drive unqualified binding despite later broken reference",
    );
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserveralt_testeventserver_ping(obj)"),
        "expected unqualified valid-first binding to lower to the alt synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_valid_base_then_broken_alt_then_valid_alt2_prefers_base_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-valid-base-broken-alt-valid-alt2-unqualified",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAltMissing",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt2",
                guid: Some("{E2A30001-0001-0001-0001-000000000201}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt2.tlb"),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("valid first reference should drive unqualified binding despite later broken middle reference");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserver_testeventserver_ping(obj)"),
        "expected unqualified valid-first binding to lower to the base synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_valid_alt2_then_broken_base_then_valid_alt_prefers_alt2_for_unqualified_testeventserver()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-valid-alt2-broken-base-valid-alt-unqualified",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As New TestEventServer\n",
            "Dim value\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAlt2",
                guid: Some("{E2A30001-0001-0001-0001-000000000201}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt2.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest)
        .expect("valid first alt2 reference should drive unqualified binding despite later broken middle reference");
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("value = pmr_oxvba_testeventserveralt2_testeventserver_ping(obj)"),
        "expected unqualified valid-first binding to lower to the alt2 synthetic typelib target, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_broken_base_then_valid_alt_then_valid_alt2_qualified_target_resolves_alt2_binding()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-broken-base-valid-alt-valid-alt2-qualified-alt2",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As OxVbaAlt2.TestEventServer\n",
            "Dim value\n",
            "Set obj = New OxVbaAlt2.TestEventServer\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaMissingBase",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServer.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt2",
                guid: Some("{E2A30001-0001-0001-0001-000000000201}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt2.tlb"),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest).expect(
        "qualified later-valid alt2 reference should compile despite earlier broken reference",
    );
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserveralt2\")"),
        "expected qualified later-valid alt2 binding to lower to the alt2 ProgID, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_loaded_basproj_broken_alt2_then_valid_base_then_valid_alt_qualified_target_resolves_alt_binding()
 {
    let loaded = load_typelib_basproj_with_ref_specs(
        "basproj-typelib-broken-alt2-valid-base-valid-alt-qualified-alt",
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Sub Main()\n",
            "Dim obj As OxVbaAlt.TestEventServer\n",
            "Dim value\n",
            "Set obj = New OxVbaAlt.TestEventServer\n",
            "value = obj.Ping()\n",
            "End Sub\n"
        ),
        &[
            BasprojComRefSpec {
                include: "OxVbaAlt2Missing",
                guid: Some("{E2A30001-0001-0001-0001-000000000201}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some(
                    "C:\\Work\\DnaCalc\\OxVba\\temp\\missing\\OxVba.TestEventServerAlt2.tlb",
                ),
            },
            BasprojComRefSpec {
                include: "OxVba",
                guid: Some("{E2A30001-0001-0001-0001-000000000001}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServer.tlb"),
            },
            BasprojComRefSpec {
                include: "OxVbaAlt",
                guid: Some("{E2A30001-0001-0001-0001-000000000101}"),
                major: Some(1),
                minor: Some(0),
                lcid: Some(0),
                importlib: Some("OxVba.TestEventServerAlt.tlb"),
            },
        ],
    );

    let compiled = compile_project(&loaded.manifest).expect(
        "qualified later-valid alt reference should compile despite earlier broken alt2 reference",
    );
    let out = compiled.rewritten_source.to_ascii_lowercase();
    assert!(
        out.contains("set obj = createobject(\"oxvba.testeventserveralt\")"),
        "expected qualified later-valid alt binding to lower to the alt ProgID, got: {out}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_registered_testeventserver_withevents_callback_invokes_handler_body() {
    if !registered_testeventserver_available() {
        return;
    }

    let class_module = module_unit_from_source(
        "Sink",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Sink"
Private WithEvents src As OxVba.TestEventServer

Private Sub Class_Initialize()
    Set src = New OxVba.TestEventServer
    Call src.FireValueChanged(7)
End Sub

Private Sub src_OnValueChanged(ByVal value)
    Err.Raise 77
End Sub

Public Function Touch() As Long
    Touch = 1
End Sub
"#,
    )
    .expect("class module should parse");
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim s As New Sink
Dim touched
touched = s.Touch()
End Sub
"#,
    )
    .expect("main module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let err = run_project_windows_hosted_error(&manifest, false);
    assert!(
        err.contains("runtime error: 77"),
        "expected callback handler Err.Raise 77, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_registered_testeventserver_withevents_callback_preserves_value_payload() {
    if !registered_testeventserver_available() {
        return;
    }

    let class_module = module_unit_from_source(
        "Sink",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Sink"
Private WithEvents src As OxVba.TestEventServer

Private Sub Class_Initialize()
    Set src = New OxVba.TestEventServer
    Call src.FireValueChanged(7)
End Sub

Private Sub src_OnValueChanged(ByVal value)
    If value = 7 Then
        Err.Raise 7007
    Else
        Err.Raise 7999
    End If
End Sub

Public Function Touch() As Long
    Touch = 1
End Function
"#,
    )
    .expect("class module should parse");
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
    Dim s As New Sink
    Dim touched
    touched = s.Touch()
End Sub
"#,
    )
    .expect("main module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let err = run_project_windows_hosted_error(&manifest, false);
    assert!(
        err.contains("runtime error: 7007"),
        "expected callback payload encoded in Err.Raise 7007, got: {err}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_vm_jit_snapshots_match_for_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim countValue
Dim existsValue
Dim lookupValue
Dim echoValue
countValue = obj.Count()
existsValue = obj.Exists(41)
lookupValue = obj.Lookup(41)
echoValue = obj(41)
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for early-binding subset"
    );
}

#[test]
fn early_bound_project_executes_imported_call_statements_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.Count()
Call obj.Exists(42)
Call obj.Lookup(42)
Call obj.Value()
Call obj(42)
afterValue = 19
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(19),
        "Call-form imported positional method/property/default-member invokes should execute without degrading the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_call_statement_subset_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.Count()
Call obj.Exists(42)
Call obj.Lookup(42)
Call obj.Value()
Call obj(42)
afterValue = 19
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported Call-form positional member invokes"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_named_argument_call_statements() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.SumPair(rhs := 14, lhs := 3)
Call obj.LookupPair(rhs := 9, lhs := 5)
Call obj(value := 41)
afterValue = 29
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(29),
        "Call-form imported named-argument method/property/default-member invokes should execute without degrading metadata-backed canonicalization"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_call_statements_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.SumPair(rhs := 14, lhs := 3)
Call obj.LookupPair(rhs := 9, lhs := 5)
Call obj(value := 41)
afterValue = 29
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported Call-form named-argument member invokes"
    );
}

#[test]
fn early_bound_project_executes_imported_no_paren_call_statements_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.Count
Call obj.Exists 42
Call obj.Lookup 42
Call obj.Value
Call obj 42
afterValue = 43
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(43),
        "no-paren Call-form imported positional method/property/default-member invokes should execute on the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_no_paren_call_statement_subset_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.Count
Call obj.Exists 42
Call obj.Lookup 42
Call obj.Value
Call obj 42
afterValue = 43
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported no-paren Call-form positional member invokes"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_no_paren_named_argument_call_statements() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.SumPair rhs := 14, lhs := 3
Call obj.LookupPair rhs := 9, lhs := 5
Call obj value := 41
afterValue = 47
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(47),
        "no-paren Call-form imported named-argument method/property/default-member invokes should execute on the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_no_paren_named_argument_call_statements_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
Call obj.SumPair rhs := 14, lhs := 3
Call obj.LookupPair rhs := 9, lhs := 5
Call obj value := 41
afterValue = 47
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported no-paren Call-form named-argument member invokes"
    );
}

#[test]
fn early_bound_project_executes_imported_statement_context_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.Count()
obj.Exists(42)
obj.Lookup(42)
obj.Value()
obj(42)
afterValue = 31
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(31),
        "statement-context imported positional method/property/default-member invokes should execute without degrading the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_statement_context_subset_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.Count()
obj.Exists(42)
obj.Lookup(42)
obj.Value()
obj(42)
afterValue = 31
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported statement-context positional member invokes"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_named_argument_statement_context() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.SumPair(rhs := 14, lhs := 3)
obj.LookupPair(rhs := 9, lhs := 5)
obj(value := 41)
afterValue = 37
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(37),
        "statement-context imported named-argument method/property/default-member invokes should execute without degrading metadata-backed canonicalization"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_statement_context_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.SumPair(rhs := 14, lhs := 3)
obj.LookupPair(rhs := 9, lhs := 5)
obj(value := 41)
afterValue = 37
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported statement-context named-argument member invokes"
    );
}

#[test]
fn early_bound_project_executes_imported_no_paren_statement_context_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.Exists 42
obj.Lookup 42
obj 42
afterValue = 53
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(53),
        "no-paren statement-context imported positional method/property/default-member invokes should execute on the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_no_paren_statement_context_subset_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.Exists 42
obj.Lookup 42
obj 42
afterValue = 53
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported no-paren statement-context positional member invokes"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_no_paren_named_argument_statement_context() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.SumPair rhs := 14, lhs := 3
obj.LookupPair rhs := 9, lhs := 5
obj value := 41
afterValue = 59
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(59),
        "no-paren statement-context imported named-argument method/property/default-member invokes should execute on the metadata-backed subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_no_paren_named_argument_statement_context_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterValue
obj.SumPair rhs := 14, lhs := 3
obj.LookupPair rhs := 9, lhs := 5
obj value := 41
afterValue = 59
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported no-paren statement-context named-argument member invokes"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_reports_runtime_error_for_imported_raise_exception_call_statement() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Call obj.RaiseException()
End Sub
"#,
    );

    let vm = run_project_windows_hosted_error(&manifest, false);
    let jit = run_project_windows_hosted_error(&manifest, true);
    assert!(
        vm.contains("com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;")
            && jit.contains(
                "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;"
            ),
        "expected stable imported exception prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
    assert!(
        vm.contains("excep_source=\"OxVba.TestDispatch\"")
            && vm.contains("excep_description=\"controlled dispatch exception\"")
            && jit.contains("excep_source=\"OxVba.TestDispatch\"")
            && jit.contains("excep_description=\"controlled dispatch exception\""),
        "expected imported EXCEPINFO source/description across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_reports_runtime_error_for_imported_raise_exception_statement_context() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
obj.RaiseException()
End Sub
"#,
    );

    let vm = run_project_windows_hosted_error(&manifest, false);
    let jit = run_project_windows_hosted_error(&manifest, true);
    assert!(
        vm.contains("com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;")
            && jit.contains(
                "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;"
            ),
        "expected stable imported exception prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
    assert!(
        vm.contains("excep_source=\"OxVba.TestDispatch\"")
            && vm.contains("excep_description=\"controlled dispatch exception\"")
            && jit.contains("excep_source=\"OxVba.TestDispatch\"")
            && jit.contains("excep_description=\"controlled dispatch exception\""),
        "expected imported EXCEPINFO source/description across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_reports_runtime_error_for_imported_no_paren_raise_exception_call_statement()
{
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Call obj.RaiseException
End Sub
"#,
    );

    let vm = run_project_windows_hosted_error(&manifest, false);
    let jit = run_project_windows_hosted_error(&manifest, true);
    assert!(
        vm.contains("com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;")
            && jit.contains(
                "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;"
            ),
        "expected stable imported exception prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
    assert!(
        vm.contains("excep_source=\"OxVba.TestDispatch\"")
            && vm.contains("excep_description=\"controlled dispatch exception\"")
            && jit.contains("excep_source=\"OxVba.TestDispatch\"")
            && jit.contains("excep_description=\"controlled dispatch exception\""),
        "expected imported EXCEPINFO source/description across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_reports_runtime_error_for_imported_no_paren_raise_exception_statement_context()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
obj.RaiseException
End Sub
"#,
    );

    let vm = run_project_windows_hosted_error(&manifest, false);
    let jit = run_project_windows_hosted_error(&manifest, true);
    assert!(
        vm.contains("com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;")
            && jit.contains(
                "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009;"
            ),
        "expected stable imported exception prefix across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
    assert!(
        vm.contains("excep_source=\"OxVba.TestDispatch\"")
            && vm.contains("excep_description=\"controlled dispatch exception\"")
            && jit.contains("excep_source=\"OxVba.TestDispatch\"")
            && jit.contains("excep_description=\"controlled dispatch exception\""),
        "expected imported EXCEPINFO source/description across VM/JIT, got vm={vm:?} jit={jit:?}"
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_unsupported_member_shape() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Call obj.SetValue(9)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("unsupported member shape should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-MEMBER-SHAPE-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_property_put_assignments_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterSetValue
Dim afterSetIndexedValue
obj.SetValue = 9
afterSetValue = DispatchInvoke(obj, "Value")
obj.SetIndexedValue(7) = 11
afterSetIndexedValue = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(9),
        "imported property-put assignment should lower into the deterministic setter lane"
    );
    assert_eq!(
        out[2],
        RuntimeValue::I32(307_011),
        "imported indexed property-put assignment should preserve index and value"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_property_put_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterSetValue
Dim afterSetIndexedValue
obj.SetValue = 9
afterSetValue = DispatchInvoke(obj, "Value")
obj.SetIndexedValue(7) = 11
afterSetIndexedValue = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported property-put assignment subset"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_named_argument_property_put_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterSetIndexedValue
obj.SetIndexedValue(lhs := 7) = 11
afterSetIndexedValue = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(307_011),
        "imported named-argument property-put assignment should preserve metadata-backed parameter naming"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_property_put_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim afterSetIndexedValue
obj.SetIndexedValue(lhs := 7) = 11
afterSetIndexedValue = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported named-argument property-put assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_named_argument_property_putref_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim other As New OxVba.TestDispatch
Dim otherCount
Dim afterSetIndexedValueRef
otherCount = DispatchInvoke(other, "Count")
Set obj.SetIndexedValueRef(lhs := 8) = other
afterSetIndexedValueRef = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(
        expect_object_handle(&out[0]).raw() >= 20_001,
        "receiver should remain a controlled object handle"
    );
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert_eq!(
        out[2],
        RuntimeValue::I32(7),
        "controlled property-putref object lane should interrogate the bound object deterministically"
    );
    assert_eq!(
        out[3],
        RuntimeValue::I32(408_007),
        "named-argument property-putref assignment should preserve index and bounded object-derived token"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_property_putref_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim other As New OxVba.TestDispatch
Dim otherCount
Dim afterSetIndexedValueRef
otherCount = DispatchInvoke(other, "Count")
Set obj.SetIndexedValueRef(lhs := 8) = other
afterSetIndexedValueRef = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported named-argument property-putref assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_zero_arg_property_get_read_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim implicitValue
Dim explicitValue
obj.SetValue = 9
implicitValue = obj.Value
Let explicitValue = obj.Value
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(9),
        "imported zero-arg property-get read-assignment should lower through metadata-backed getter syntax"
    );
    assert_eq!(
        out[2],
        RuntimeValue::I32(9),
        "explicit Let should preserve imported zero-arg property-get read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_zero_arg_method_read_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim implicitValue
Dim explicitValue
implicitValue = obj.Ping
Let explicitValue = obj.Ping
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(123),
        "imported zero-arg method read-assignment should lower through metadata-backed invoke syntax"
    );
    assert_eq!(
        out[2],
        RuntimeValue::I32(123),
        "explicit Let should preserve imported zero-arg method read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_zero_arg_method_read_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim implicitValue
Dim explicitValue
implicitValue = obj.Ping
Let explicitValue = obj.Ping
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported zero-arg method read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_zero_arg_property_get_read_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim implicitValue
Dim explicitValue
obj.SetValue = 9
implicitValue = obj.Value
Let explicitValue = obj.Value
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported zero-arg property-get read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_parenthesized_zero_arg_property_get_read_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim implicitValue
Dim explicitValue
obj.SetValue = 9
implicitValue = obj.Value()
Let explicitValue = obj.Value()
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(9),
        "imported parenthesized zero-arg property-get read-assignment should lower through metadata-backed getter syntax"
    );
    assert_eq!(
        out[2],
        RuntimeValue::I32(9),
        "explicit Let should preserve imported parenthesized zero-arg property-get read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_parenthesized_zero_arg_property_get_read_assignment_vm_jit_snapshots_match()
{
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim implicitValue
Dim explicitValue
obj.SetValue = 9
implicitValue = obj.Value()
Let explicitValue = obj.Value()
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported parenthesized zero-arg property-get read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_object_property_get_read_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.SelfDispatch
Set childUnknown = obj.SelfUnknown
wrappedDispatch = obj.SelfDispatch
Let wrappedUnknown = obj.SelfUnknown
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert!(expect_object_handle(&out[3]).raw() >= 20_001);
    assert!(expect_object_handle(&out[4]).raw() >= 20_001);
    assert_eq!(
        out[5],
        RuntimeValue::I32(7),
        "direct imported object-valued property-get should preserve VT_DISPATCH rebinding on Object targets"
    );
    assert_eq!(
        out[6],
        RuntimeValue::I32(7),
        "direct imported object-valued property-get should preserve VT_UNKNOWN rebinding on Object targets"
    );
    assert_eq!(
        out[7],
        RuntimeValue::I32(7),
        "direct imported object-valued property-get should preserve VT_DISPATCH rebinding on Variant targets"
    );
    assert_eq!(
        out[8],
        RuntimeValue::I32(7),
        "direct imported object-valued property-get should preserve VT_UNKNOWN rebinding on explicit-Let Variant targets"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_object_property_get_read_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.SelfDispatch
Set childUnknown = obj.SelfUnknown
wrappedDispatch = obj.SelfDispatch
Let wrappedUnknown = obj.SelfUnknown
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported object-valued property-get read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_parenthesized_object_property_get_read_assignments() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.SelfDispatch()
Set childUnknown = obj.SelfUnknown()
wrappedDispatch = obj.SelfDispatch()
Let wrappedUnknown = obj.SelfUnknown()
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert!(expect_object_handle(&out[3]).raw() >= 20_001);
    assert!(expect_object_handle(&out[4]).raw() >= 20_001);
    assert_eq!(
        out[5],
        RuntimeValue::I32(7),
        "parenthesized imported object-valued property-get should preserve VT_DISPATCH rebinding on Object targets"
    );
    assert_eq!(
        out[6],
        RuntimeValue::I32(7),
        "parenthesized imported object-valued property-get should preserve VT_UNKNOWN rebinding on Object targets"
    );
    assert_eq!(
        out[7],
        RuntimeValue::I32(7),
        "parenthesized imported object-valued property-get should preserve VT_DISPATCH rebinding on Variant targets"
    );
    assert_eq!(
        out[8],
        RuntimeValue::I32(7),
        "parenthesized imported object-valued property-get should preserve VT_UNKNOWN rebinding on explicit-Let Variant targets"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_parenthesized_object_property_get_read_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.SelfDispatch()
Set childUnknown = obj.SelfUnknown()
wrappedDispatch = obj.SelfDispatch()
Let wrappedUnknown = obj.SelfUnknown()
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for parenthesized imported object-valued property-get read-assignment syntax"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_object_result_member_calls() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim child As Object
Dim wrapped
Dim childCount
Dim wrappedCount
Set child = obj.ReturnSelfDispatch()
wrapped = obj.ReturnSelfUnknown()
childCount = DispatchInvoke(child, "Count")
wrappedCount = DispatchInvoke(wrapped, "Count")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert_eq!(
        out[3],
        RuntimeValue::I32(7),
        "VT_DISPATCH imported member result should rebind into an invokable object handle"
    );
    assert_eq!(
        out[4],
        RuntimeValue::I32(7),
        "VT_UNKNOWN imported member result should rebind through IDispatch into an invokable object handle"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_reuses_imported_object_identity_for_dispatch_and_unknown_results() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim firstDispatch As Object
Dim secondDispatch As Object
Dim firstUnknown
Dim secondUnknown
Set firstDispatch = obj.ReturnSelfDispatch()
Set secondDispatch = obj.ReturnSelfDispatch()
firstUnknown = obj.ReturnSelfUnknown()
secondUnknown = obj.ReturnSelfUnknown()
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_same_object_identity(
        &vm,
        &[1, 2, 3, 4],
        "VM imported repeated object-result identity",
    );
    assert_same_object_identity(
        &jit,
        &[1, 2, 3, 4],
        "JIT imported repeated object-result identity",
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_named_argument_calls() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim sumPair
Dim lookupPair
Dim echoValue
sumPair = obj.SumPair(rhs := 14, lhs := 3)
lookupPair = obj.LookupPair(rhs := 9, lhs := 5)
echoValue = obj(value := 41)
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(3_014),
        "imported method call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
    assert_eq!(
        out[2],
        RuntimeValue::I32(205_009),
        "imported property-get call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
    assert_eq!(
        out[3],
        RuntimeValue::I32(41),
        "imported default-member call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_named_argument_calls_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim sumPair
Dim lookupPair
Dim echoValue
sumPair = obj.SumPair(rhs := 14, lhs := 3)
lookupPair = obj.LookupPair(rhs := 9, lhs := 5)
echoValue = obj(value := 41)
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported named-argument calls"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_explicit_let_named_argument_calls() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim sumPair
Dim lookupPair
Dim echoValue
Let sumPair = obj.SumPair(rhs := 14, lhs := 3)
Let lookupPair = obj.LookupPair(rhs := 9, lhs := 5)
Let echoValue = obj(value := 41)
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(3_014),
        "explicit Let imported method call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
    assert_eq!(
        out[2],
        RuntimeValue::I32(205_009),
        "explicit Let imported property-get call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
    assert_eq!(
        out[3],
        RuntimeValue::I32(41),
        "explicit Let imported default-member call should preserve named-argument canonicalization through metadata-backed dispatch"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_explicit_let_named_argument_calls_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim sumPair
Dim lookupPair
Dim echoValue
Let sumPair = obj.SumPair(rhs := 14, lhs := 3)
Let lookupPair = obj.LookupPair(rhs := 9, lhs := 5)
Let echoValue = obj(value := 41)
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for explicit Let imported named-argument calls"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_explicit_let_positional_calls() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim countValue
Dim existsValue
Dim lookupValue
Dim echoValue
Let countValue = obj.Count()
Let existsValue = obj.Exists(42)
Let lookupValue = obj.Lookup(42)
Let echoValue = obj(42)
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(
        out[1],
        RuntimeValue::I32(7),
        "explicit Let imported zero-arg call should preserve metadata-backed lowering"
    );
    assert_eq!(
        out[2],
        RuntimeValue::Bool(true),
        "explicit Let imported method call should preserve metadata-backed lowering"
    );
    assert_eq!(
        out[3],
        RuntimeValue::I32(1_042),
        "explicit Let imported property-get call should preserve metadata-backed lowering"
    );
    assert_eq!(
        out[4],
        RuntimeValue::I32(42),
        "explicit Let imported default-member call should preserve metadata-backed lowering"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_explicit_let_positional_calls_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim countValue
Dim existsValue
Dim lookupValue
Dim echoValue
Let countValue = obj.Count()
Let existsValue = obj.Exists(42)
Let lookupValue = obj.Lookup(42)
Let echoValue = obj(42)
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for explicit Let imported positional calls"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_object_result_member_calls_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim child As Object
Dim wrapped
Dim childCount
Dim wrappedCount
Set child = obj.ReturnSelfDispatch()
wrapped = obj.ReturnSelfUnknown()
childCount = DispatchInvoke(child, "Count")
wrappedCount = DispatchInvoke(wrapped, "Count")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported object-result member calls"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_object_result_assignment_intents() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.ReturnSelfDispatch()
Set childUnknown = obj.ReturnSelfUnknown()
wrappedDispatch = obj.ReturnSelfDispatch()
Let wrappedUnknown = obj.ReturnSelfUnknown()
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert!(expect_object_handle(&out[3]).raw() >= 20_001);
    assert!(expect_object_handle(&out[4]).raw() >= 20_001);
    assert_eq!(
        out[5],
        RuntimeValue::I32(7),
        "explicit Set should preserve VT_DISPATCH object-result rebinding on Object targets"
    );
    assert_eq!(
        out[6],
        RuntimeValue::I32(7),
        "explicit Set should preserve VT_UNKNOWN object-result rebinding on Object targets"
    );
    assert_eq!(
        out[7],
        RuntimeValue::I32(7),
        "implicit Variant-target assignment should preserve VT_DISPATCH object-result rebinding"
    );
    assert_eq!(
        out[8],
        RuntimeValue::I32(7),
        "explicit Let Variant-target assignment should preserve VT_UNKNOWN object-result rebinding"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_zero_arg_object_result_assignment_intents_without_parentheses()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.ReturnSelfDispatch
Set childUnknown = obj.ReturnSelfUnknown
wrappedDispatch = obj.ReturnSelfDispatch
Let wrappedUnknown = obj.ReturnSelfUnknown
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert!(expect_object_handle(&out[2]).raw() >= 20_001);
    assert!(expect_object_handle(&out[3]).raw() >= 20_001);
    assert!(expect_object_handle(&out[4]).raw() >= 20_001);
    assert_eq!(
        out[5],
        RuntimeValue::I32(7),
        "explicit Set should preserve VT_DISPATCH zero-arg method rebinding on Object targets without parentheses"
    );
    assert_eq!(
        out[6],
        RuntimeValue::I32(7),
        "explicit Set should preserve VT_UNKNOWN zero-arg method rebinding on Object targets without parentheses"
    );
    assert_eq!(
        out[7],
        RuntimeValue::I32(7),
        "implicit Variant-target assignment should preserve VT_DISPATCH zero-arg method rebinding without parentheses"
    );
    assert_eq!(
        out[8],
        RuntimeValue::I32(7),
        "explicit Let Variant-target assignment should preserve VT_UNKNOWN zero-arg method rebinding without parentheses"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_zero_arg_object_result_assignment_intents_without_parentheses_vm_jit_snapshots_match()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.ReturnSelfDispatch
Set childUnknown = obj.ReturnSelfUnknown
wrappedDispatch = obj.ReturnSelfDispatch
Let wrappedUnknown = obj.ReturnSelfUnknown
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported zero-arg object-result assignment intents without parentheses"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_object_result_assignment_intents_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim childDispatch As Object
Dim childUnknown As Object
Dim wrappedDispatch
Dim wrappedUnknown
Dim childDispatchCount
Dim childUnknownCount
Dim wrappedDispatchCount
Dim wrappedUnknownCount
Set childDispatch = obj.ReturnSelfDispatch()
Set childUnknown = obj.ReturnSelfUnknown()
wrappedDispatch = obj.ReturnSelfDispatch()
Let wrappedUnknown = obj.ReturnSelfUnknown()
childDispatchCount = DispatchInvoke(childDispatch, "Count")
childUnknownCount = DispatchInvoke(childUnknown, "Count")
wrappedDispatchCount = DispatchInvoke(wrappedDispatch, "Count")
wrappedUnknownCount = DispatchInvoke(wrappedUnknown, "Count")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported object-result assignment-intent lanes"
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_missing_member() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim value
value = obj.UnknownMember()
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("missing member should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-NOT-FOUND"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_executes_imported_property_putref_assignments_subset() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim other As New OxVba.TestDispatch
Dim otherCount
Dim afterSetValueRef
otherCount = DispatchInvoke(other, "Count")
Set obj.SetValueRef = other
afterSetValueRef = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(
        expect_object_handle(&out[0]).raw() >= 20_001,
        "receiver should remain a controlled object handle"
    );
    assert!(expect_object_handle(&out[1]).raw() >= 20_001);
    assert_eq!(
        out[2],
        RuntimeValue::I32(7),
        "controlled property-putref object lane should interrogate the bound object deterministically"
    );
    assert_eq!(
        out[3],
        RuntimeValue::I32(100_007),
        "property-putref assignment should preserve bounded object-derived token on the deterministic setter lane"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_bound_project_property_putref_assignment_vm_jit_snapshots_match() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim other As New OxVba.TestDispatch
Dim otherCount
Dim afterSetValueRef
otherCount = DispatchInvoke(other, "Count")
Set obj.SetValueRef = other
afterSetValueRef = DispatchInvoke(obj, "Value")
End Sub
"#,
    );

    let vm = run_project_windows_hosted(&manifest, false);
    let jit = run_project_windows_hosted(&manifest, true);
    assert_eq!(
        vm, jit,
        "VM/JIT snapshots should match for imported property-putref assignment syntax"
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_wrong_property_put_arity() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
obj.SetIndexedValue = 11
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong property-put arity should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_wrong_arity() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim value
value = obj.Exists()
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong-arity member should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_wrong_default_member_arity() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim value
value = obj()
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong default-member arity should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_accepts_imported_withevents_source() {
    let class_module = module_unit_from_source(
        "Sink",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Sink"
Private WithEvents src As OxVba.TestEventServer
Public Sub Attach()
End Sub
"#,
    )
    .expect("class module should parse");
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
End Sub
"#,
    )
    .expect("main module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_withevents_source() {
    let class_module = module_unit_from_source(
        "Sink",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Sink"
Private WithEvents src As TestEventServer
Public Sub Attach()
End Sub
"#,
    )
    .expect("class module should parse");
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
End Sub
"#,
    )
    .expect("main module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_type_declaration() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As TestDispatch
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_as_new_declaration() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New TestDispatch
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_imported_module_scope_declaration() {
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Private obj As OxVba.TestDispatch
Public Sub Main()
End Sub
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_imported_procedure_param_type_signature() {
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Public Sub Observe(ByVal obj As OxVba.TestDispatch)
End Sub
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_imported_implements_directive() {
    let class_module = module_unit_from_source(
        "ThingImpl",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "ThingImpl"
Implements OxVba.TestDispatch
Public Sub Main()
End Sub
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_implements_directive() {
    let class_module = module_unit_from_source(
        "ThingImpl",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "ThingImpl"
Implements TestDispatch
Public Sub Main()
End Sub
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_imported_event_declaration_type() {
    let class_module = module_unit_from_source(
        "Emitter",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Emitter"
Public Event Changed(ByVal value As OxVba.TestDispatch)
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_event_declaration_type() {
    let class_module = module_unit_from_source(
        "Emitter",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Emitter"
Public Event Changed(ByVal value As TestDispatch)
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_procedure_return_type_signature() {
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Public Function Make() As TestDispatch
End Function
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_accepts_unqualified_imported_module_scope_declaration() {
    let class_module = module_unit_from_source(
        "Widget",
        ModuleKind::Class,
        r#"
Attribute VB_Name = "Widget"
Private obj As TestDispatch
Public Sub Main()
End Sub
"#,
    )
    .expect("class module should parse");
    let manifest = ProjectManifest {
        project_name: "ProjectA".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![class_module],
        references: vec![ProjectReference {
            referenced_project_name: "OxVba".to_string(),
            reference_kind: ReferenceKind::TypeLibrary,
        }],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect("should compile");
}

#[test]
fn early_bound_project_reports_compile_error_for_missing_default_member() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchNoDefault
Dim value
value = obj(41)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("missing default member should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-NOT-FOUND"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_ambiguous_default_member() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchAmbiguousDefault
Dim value
value = obj(41)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("ambiguous default member should fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-AMBIGUOUS"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_wrong_default_member_arity_no_paren_call_statement()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Call obj
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong zero-arg no-paren Call default-member arity should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_wrong_default_member_arity_no_paren_statement() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
obj
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong zero-arg no-paren statement default-member arity should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_missing_default_member_no_paren_call_statement() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchNoDefault
Call obj 41
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("missing no-paren Call default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-NOT-FOUND"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_missing_default_member_no_paren_statement() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchNoDefault
obj 41
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("missing no-paren statement default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-NOT-FOUND"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_ambiguous_default_member_no_paren_call_statement()
{
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchAmbiguousDefault
Call obj 41
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("ambiguous no-paren Call default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-AMBIGUOUS"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_ambiguous_default_member_no_paren_statement() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchAmbiguousDefault
obj 41
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("ambiguous no-paren statement default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-AMBIGUOUS"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_wrong_default_member_arity_parenthesized_call_statement()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Call obj()
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong parenthesized Call default-member arity should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_wrong_default_member_arity_parenthesized_statement()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
obj()
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("wrong parenthesized statement default-member arity should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_missing_default_member_parenthesized_call_statement()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchNoDefault
Call obj(41)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("missing parenthesized Call default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-NOT-FOUND"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_missing_default_member_parenthesized_statement() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchNoDefault
obj(41)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("missing parenthesized statement default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-NOT-FOUND"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_ambiguous_default_member_parenthesized_call_statement()
 {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchAmbiguousDefault
Call obj(41)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("ambiguous parenthesized Call default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-AMBIGUOUS"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn early_bound_project_reports_compile_error_for_ambiguous_default_member_parenthesized_statement()
{
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatchAmbiguousDefault
obj(41)
End Sub
"#,
    );

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&manifest)
        .expect_err("ambiguous parenthesized statement default member should fail");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message().contains("BIND-E-TYPELIB-MEMBER-AMBIGUOUS"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[cfg(target_os = "windows")]
#[test]
fn early_and_late_dispatch_paths_can_mix_in_one_project() {
    let manifest = manifest_with_typelib(
        r#"
Attribute VB_Name = "MainModule"
Public Sub Main()
Dim obj As New OxVba.TestDispatch
Dim a
Dim b
a = obj.Count()
b = DispatchInvoke(obj, "Exists", 42)
End Sub
"#,
    );

    let out = run_project_windows_hosted(&manifest, false);
    assert!(expect_object_handle(&out[0]).raw() >= 20_001);
    assert_eq!(out[1], RuntimeValue::I32(7));
    assert_eq!(out[2], RuntimeValue::Bool(true));
}
