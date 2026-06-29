//! End-to-end clean path from on-disk `.basproj` files: a two-project workspace
//! (App → Lib) loaded with `oxvba_project::load_project_closure`, then run on the
//! new pipeline via `Engine::execute_project_closure_with_variant_snapshot`
//! (`bind_projects` → `linearize` → `oxvba_vm2::Vm::link` → run). Proves the host
//! runs a cross-project workspace straight from disk.

use std::path::{Path, PathBuf};

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig};

fn unique_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "oxvba_clean_path_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp root");
    dir
}

fn write_project(
    root: &Path,
    name: &str,
    output_type: &str,
    entry_point: Option<&str>,
    modules: &[(&str, &str)],
    project_refs: &[&str],
) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).expect("create project dir");
    let module_items: String = modules
        .iter()
        .map(|(file, _)| {
            if file.ends_with(".cls") {
                format!("    <ClassModule Include=\"{file}\" />\n")
            } else {
                format!("    <Module Include=\"{file}\" />\n")
            }
        })
        .collect();
    let ref_items: String = project_refs
        .iter()
        .map(|inc| format!("    <ProjectReference Include=\"{inc}\" />\n"))
        .collect();
    let entry = entry_point
        .map(|e| format!("    <EntryPoint>{e}</EntryPoint>\n"))
        .unwrap_or_default();
    let xml = format!(
        "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n\
         <PropertyGroup>\n\
         <OutputType>{output_type}</OutputType>\n\
         <ProjectName>{name}</ProjectName>\n\
         {entry}</PropertyGroup>\n\
         <ItemGroup>\n{module_items}</ItemGroup>\n\
         <ItemGroup>\n{ref_items}</ItemGroup>\n\
         </Project>\n"
    );
    let basproj_path = dir.join(format!("{name}.basproj"));
    std::fs::write(&basproj_path, xml).expect("write basproj");
    for (file, source) in modules {
        std::fs::write(dir.join(file), source).expect("write module");
    }
    basproj_path
}

#[test]
fn cross_project_workspace_runs_from_disk() {
    let root = unique_root("app_lib");
    // Lib: a referenced library exporting `Add`.
    write_project(
        &root,
        "Lib",
        "Library",
        None,
        &[(
            "LibMod.bas",
            "Public Function Add(ByVal a As Long, ByVal b As Long) As Long\nAdd = a + b\nEnd Function\n",
        )],
        &[],
    );
    // App: references Lib, computes `r = Add(20, 22)` into a module global. The
    // module is named `Program` (not `Main`) so it does not collide with the
    // auto-injected startup shim's `Sub Main`.
    let app = write_project(
        &root,
        "App",
        "Exe",
        Some("Program.Run"),
        &[(
            "Program.bas",
            "Public r As Long\nPublic Sub Run()\nr = Add(20, 22)\nEnd Sub\n",
        )],
        &["../Lib/Lib.basproj"],
    );

    let closure = oxvba_project::load_project_closure(&app).expect("load closure");
    assert_eq!(closure.len(), 2, "Lib + App");

    let engine = Engine::new(HostConfig { enable_jit: false });
    let values = engine
        .execute_project_closure_with_variant_snapshot(&closure)
        .expect("clean-path run");

    // The App project's first global `r` holds Lib.Add(20, 22) = 42, computed across
    // the two linked bundles.
    assert_eq!(values.first().and_then(|v| v.as_i32()), Some(42));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn cross_project_workspace_runs_from_disk_on_vm3() {
    // The `run-project` vm3 path (W8): the same cross-project workspace, run via
    // execute_project_closure_with_variant_snapshot_vm3 — bind the closure, elaborate each
    // project, link the image on vm3, run the entry. App.r = Lib.Add(20, 22) = 42 across the two
    // linked OxPrograms (exercises the W2 cross-project executor through the real on-disk loader).
    let root = unique_root("app_lib_vm3");
    write_project(
        &root,
        "Lib",
        "Library",
        None,
        &[(
            "LibMod.bas",
            "Public Function Add(ByVal a As Long, ByVal b As Long) As Long\nAdd = a + b\nEnd Function\n",
        )],
        &[],
    );
    let app = write_project(
        &root,
        "App",
        "Exe",
        Some("Program.Run"),
        &[(
            "Program.bas",
            "Public r As Long\nPublic Sub Run()\nr = Add(20, 22)\nEnd Sub\n",
        )],
        &["../Lib/Lib.basproj"],
    );

    let closure = oxvba_project::load_project_closure(&app).expect("load closure");
    assert_eq!(closure.len(), 2, "Lib + App");

    let engine = Engine::new(HostConfig { enable_jit: false });
    match engine.execute_project_closure_with_variant_snapshot_vm3(&closure) {
        oxvba_host::Vm3Snapshot::Ran(values) => {
            assert_eq!(
                values.first().and_then(|v| v.as_i32()),
                Some(42),
                "Lib.Add(20, 22) across projects on vm3"
            );
        }
        oxvba_host::Vm3Snapshot::Unsupported(what) => panic!("vm3 run-project unsupported: {what}"),
        oxvba_host::Vm3Snapshot::Failed(msg) => panic!("vm3 run-project failed: {msg}"),
    }

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn class_enum_fields_are_scalar_across_project_closure() {
    let root = unique_root("class_enum_field");
    let app = write_project(
        &root,
        "App",
        "Exe",
        Some("Program.Run"),
        &[
            (
                "Program.bas",
                "Public r As Long\n\
                 Public Enum WebFormat\n\
                 Json = 0\n\
                 UrlEncoded = 1\n\
                 End Enum\n\
                 Public Sub Run()\n\
                 Dim request As New WebRequest\n\
                 r = request.RequestFormat\n\
                 End Sub\n",
            ),
            (
                "WebRequest.cls",
                "VERSION 1.0 CLASS\n\
                 BEGIN\n\
                   MultiUse = -1  'True\n\
                 END\n\
                 Attribute VB_Name = \"WebRequest\"\n\
                 Attribute VB_GlobalNameSpace = False\n\
                 Attribute VB_Creatable = False\n\
                 Attribute VB_PredeclaredId = False\n\
                 Attribute VB_Exposed = True\n\
                 Private web_pRequestFormat As WebFormat\n\
                 Public Property Get RequestFormat() As WebFormat\n\
                 RequestFormat = web_pRequestFormat\n\
                 End Property\n\
                 Public Property Let RequestFormat(ByVal Value As WebFormat)\n\
                 web_pRequestFormat = Value\n\
                 End Property\n\
                 Private Sub Class_Initialize()\n\
                 Me.RequestFormat = WebFormat.UrlEncoded\n\
                 End Sub\n",
            ),
        ],
        &[],
    );

    let closure = oxvba_project::load_project_closure(&app).expect("load closure");
    let engine = Engine::new(HostConfig { enable_jit: false });
    let values = engine
        .execute_project_closure_with_variant_snapshot(&closure)
        .expect("enum field class run");

    assert_eq!(values.first().and_then(|v| v.as_i32()), Some(1));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn class_property_get_returns_me_public_field_across_project_closure() {
    let root = unique_root("class_me_public_field_get");
    let app = write_project(
        &root,
        "App",
        "Exe",
        Some("Program.Run"),
        &[
            (
                "Program.bas",
                "Public result As String\n\
                 Public Sub Run()\n\
                 Dim request As New WebRequest\n\
                 request.Resource = \"orders/{id}\"\n\
                 result = request.FormattedResource\n\
                 End Sub\n",
            ),
            (
                "WebRequest.cls",
                "VERSION 1.0 CLASS\n\
                 BEGIN\n\
                   MultiUse = -1  'True\n\
                 END\n\
                 Attribute VB_Name = \"WebRequest\"\n\
                 Attribute VB_GlobalNameSpace = False\n\
                 Attribute VB_Creatable = False\n\
                 Attribute VB_PredeclaredId = False\n\
                 Attribute VB_Exposed = True\n\
                 Public Resource As String\n\
                 Public Property Get FormattedResource() As String\n\
                 FormattedResource = Me.Resource\n\
                 End Property\n",
            ),
        ],
        &[],
    );

    let closure = oxvba_project::load_project_closure(&app).expect("load closure");
    let engine = Engine::new(HostConfig { enable_jit: false });
    let values = engine
        .execute_project_closure_with_variant_snapshot(&closure)
        .expect("public field property get run");

    assert_eq!(
        values
            .first()
            .and_then(|v| v.as_bstr())
            .map(|s| s.to_string()),
        Some("orders/{id}".to_string())
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
#[cfg(target_os = "windows")]
#[ignore = "live COM; requires Scripting.Dictionary registration"]
fn class_method_indexed_put_on_com_field_mutates_dictionary() {
    let root = unique_root("class_com_field_indexed_put");
    let dir = root.join("App");
    std::fs::create_dir_all(&dir).expect("create project dir");
    let basproj = dir.join("App.basproj");
    std::fs::write(
        &basproj,
        "<Project Sdk=\"OxVba.Sdk/0.1.0\">\n\
         <PropertyGroup>\n\
         <OutputType>Exe</OutputType>\n\
         <ProjectName>App</ProjectName>\n\
         <EntryPoint>Program.Run</EntryPoint>\n\
         </PropertyGroup>\n\
         <ItemGroup>\n\
         <Module Include=\"Program.bas\" />\n\
         <ClassModule Include=\"Holder.cls\" />\n\
         </ItemGroup>\n\
         <ItemGroup>\n\
         <COMReference Include=\"Scripting\">\n\
         <Guid>{420B2830-E718-11CF-893D-00A0C9054228}</Guid>\n\
         <VersionMajor>1</VersionMajor>\n\
         <VersionMinor>0</VersionMinor>\n\
         <Lcid>0</Lcid>\n\
         <ImportLib>scrrun.dll</ImportLib>\n\
         </COMReference>\n\
         </ItemGroup>\n\
         </Project>\n",
    )
    .expect("write basproj");
    std::fs::write(
        dir.join("Program.bas"),
        "Public result As Long\n\
         Public Sub Run()\n\
         Dim h As New Holder\n\
         h.PutValue \"id\", \"A + B\"\n\
         If h.Values.Count = 1 And h.Values(\"id\") = \"A + B\" And VBA.CStr(h.Values(\"id\")) = \"A + B\" And h.Echo(h.Values(\"id\")) = \"A + B\" And h.Formatted = \"orders/A%20B\" Then result = 42\n\
         End Sub\n",
    )
    .expect("write program");
    std::fs::write(
        dir.join("Holder.cls"),
        "VERSION 1.0 CLASS\n\
         BEGIN\n\
           MultiUse = -1  'True\n\
         END\n\
         Attribute VB_Name = \"Holder\"\n\
         Attribute VB_GlobalNameSpace = False\n\
         Attribute VB_Creatable = False\n\
         Attribute VB_PredeclaredId = False\n\
         Attribute VB_Exposed = True\n\
         Public Values As Dictionary\n\
         Private Sub Class_Initialize()\n\
         Set Me.Values = New Dictionary\n\
         End Sub\n\
         Public Sub PutValue(ByVal Key As String, ByVal Value As Variant)\n\
         Me.Values.Item(Key) = Value\n\
         End Sub\n\
         Public Property Get Formatted() As String\n\
         Dim key As Variant\n\
         Formatted = \"orders/{id}\"\n\
         For Each key In Me.Values.Keys\n\
         Formatted = VBA.Replace(Formatted, \"{\" & key & \"}\", \"A%20B\")\n\
         Next key\n\
         End Property\n\
         Public Function Echo(ByRef Text As Variant) As String\n\
         Echo = VBA.CStr(Text)\n\
         End Function\n",
    )
    .expect("write holder");

    let closure = oxvba_project::load_project_closure(&basproj).expect("load closure");
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let values = engine
        .execute_project_closure_with_variant_snapshot(&closure)
        .expect("indexed put class field run");

    assert_eq!(values.first().and_then(|v| v.as_i32()), Some(42));

    std::fs::remove_dir_all(&root).ok();
}
