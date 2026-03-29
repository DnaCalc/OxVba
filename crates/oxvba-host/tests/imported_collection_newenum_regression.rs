use oxvba_host::{Engine, HostConfig};
use oxvba_project::load_basproj_from_str;
use oxvba_runtime::{RuntimeValue, bstr::BStr};

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    ))
}

fn run_project_with_widget(main_source: &str, widget_source: &str) -> Result<RuntimeValue, String> {
    let temp_root = unique_temp_dir("oxvba_imported_collection_newenum");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");
    std::fs::write(temp_root.join("Main.bas"), main_source).expect("write main module");
    std::fs::write(temp_root.join("Widget.cls"), widget_source).expect("write widget class");

    let loaded = load_basproj_from_str(
        "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ImportedCollectionNewEnumProject</ProjectName>
    <EntryPoint>Main.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"Main.bas\" />
    <ClassModule Include=\"Widget.cls\" />
  </ItemGroup>
</Project>
",
        &temp_root,
    )
    .map_err(|err| err.to_string())?;
    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let mut session = engine
        .compile_and_prepare_session(&loaded.manifest)
        .map_err(|err| err.to_string())?;
    let result = engine
        .invoke_procedure(&mut session, "Main", "Main", &[])
        .map_err(|err| err.to_string());

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    result
}

#[test]
fn imported_collection_field_newenum_for_each_executes() {
    let result = run_project_with_widget(
        concat!(
            "Attribute VB_Name = \"Main\"\n",
            "Public Function Main() As String\n",
            "Dim widget As New Widget\n",
            "Dim item\n",
            "Dim valueOut\n",
            "For Each item In widget\n",
            "    valueOut = valueOut & CStr(item) & \",\"\n",
            "Next item\n",
            "Main = valueOut\n",
            "End Function\n"
        ),
        concat!(
            "Attribute VB_Name = \"Widget\"\n",
            "Option Explicit\n",
            "Private items As New Collection\n",
            "Public Sub Class_Initialize()\n",
            "items.Add 41\n",
            "items.Add 42\n",
            "End Sub\n",
            "Public Property Get NewEnum() As IUnknown\n",
            "Set NewEnum = items.[_NewEnum]\n",
            "End Property\n",
            "Attribute NewEnum.VB_UserMemId = -4\n",
            "Attribute NewEnum.VB_MemberFlags = \"40\"\n"
        ),
    )
    .expect("collection-backed NewEnum project should execute");

    assert_eq!(result, RuntimeValue::String(BStr("41,42,".to_string())));
}
