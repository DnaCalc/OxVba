use oxvba_host::{Engine, HostConfig};
use oxvba_project::load_basproj_from_str;
use oxvba_runtime::{RuntimeValue, bstr::BStr};

struct TempLoadedProject {
    loaded: oxvba_project::LoadedProject,
    temp_root: std::path::PathBuf,
}

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

fn load_widget_project(
    main_source: &str,
    widget_source: &str,
) -> Result<TempLoadedProject, String> {
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

    Ok(TempLoadedProject { loaded, temp_root })
}

fn run_project_with_widget(main_source: &str, widget_source: &str) -> Result<RuntimeValue, String> {
    let TempLoadedProject { loaded, temp_root } = load_widget_project(main_source, widget_source)?;
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

fn execute_project_with_widget_snapshot(
    main_source: &str,
    widget_source: &str,
    enable_jit: bool,
) -> Result<Vec<RuntimeValue>, String> {
    let TempLoadedProject { loaded, temp_root } = load_widget_project(main_source, widget_source)?;
    let engine = Engine::new(HostConfig {
        enable_jit,
        root_object_name: None,
    });
    let result = engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .map_err(|err| err.to_string());

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    result
}

fn excel_import_newenum_widget_source() -> &'static str {
    concat!(
        "VERSION 1.0 CLASS\n",
        "BEGIN\n",
        "  MultiUse = -1  'True\n",
        "END\n",
        "Attribute VB_Name = \"Widget\"\n",
        "Attribute VB_GlobalNameSpace = False\n",
        "Attribute VB_Creatable = False\n",
        "Attribute VB_PredeclaredId = False\n",
        "Attribute VB_Exposed = False\n",
        "Option Explicit\n",
        "Private items As New Collection\n",
        "\n",
        "Public Sub Class_Initialize()\n",
        "    items.Add 41\n",
        "    items.Add 42\n",
        "End Sub\n",
        "\n",
        "Public Property Get NewEnum() As IUnknown\n",
        "    Set NewEnum = items.[_NewEnum]\n",
        "End Property\n",
        "Attribute NewEnum.VB_UserMemId = -4\n",
        "Attribute NewEnum.VB_MemberFlags = \"40\"\n"
    )
}

const MAIN_FOREACH_WIDGET_FUNCTION_SOURCE: &str = concat!(
    "Attribute VB_Name = \"Main\"\n",
    "Public valueOut As String\n",
    "Public Function Main() As String\n",
    "Dim widget As New Widget\n",
    "Dim item\n",
    "For Each item In widget\n",
    "    valueOut = valueOut & CStr(item) & \",\"\n",
    "Next item\n",
    "Main = valueOut\n",
    "End Function\n"
);

const MAIN_FOREACH_WIDGET_PROJECT_SOURCE: &str = concat!(
    "Attribute VB_Name = \"Main\"\n",
    "Public valueOut As String\n",
    "Public Sub Main()\n",
    "Dim widget As New Widget\n",
    "Dim item\n",
    "For Each item In widget\n",
    "    valueOut = valueOut & CStr(item) & \",\"\n",
    "Next item\n",
    "End Sub\n"
);

#[test]
fn imported_collection_field_newenum_for_each_executes() {
    let result = run_project_with_widget(
        MAIN_FOREACH_WIDGET_FUNCTION_SOURCE,
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

#[test]
fn imported_collection_field_newenum_for_each_executes_with_excel_import_header() {
    let result = run_project_with_widget(
        MAIN_FOREACH_WIDGET_FUNCTION_SOURCE,
        excel_import_newenum_widget_source(),
    )
    .expect("Excel-imported collection-backed NewEnum project should execute");

    assert_eq!(result, RuntimeValue::String(BStr("41,42,".to_string())));
}

#[test]
fn imported_collection_field_newenum_foreach_project_snapshot_matches_vm_and_jit() {
    let widget_source = concat!(
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
    );

    let vm = execute_project_with_widget_snapshot(MAIN_FOREACH_WIDGET_PROJECT_SOURCE, widget_source, false)
        .expect("vm project execution should succeed");
    let jit = execute_project_with_widget_snapshot(MAIN_FOREACH_WIDGET_PROJECT_SOURCE, widget_source, true)
        .expect("jit project execution should succeed");

    assert_eq!(vm, jit, "VM/JIT snapshots should match for project-backed NewEnum For Each");
}
