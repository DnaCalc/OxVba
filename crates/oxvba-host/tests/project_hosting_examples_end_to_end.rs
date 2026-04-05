use oxvba_host::{Engine, HostConfig};

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

#[test]
fn example_direct_file_top_level_mainline_executes() {
    let source = concat!(
        "Option Explicit\n",
        "Public valueOut As Long\n",
        "Private counter As Long\n",
        "counter = 41\n",
        "valueOut = counter\n",
        "Call Bump(valueOut)\n",
        "Public Sub Bump(ByRef value)\n",
        "    value = value + 1\n",
        "End Sub\n",
    );
    let values = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    })
    .execute_source_with_value_snapshot(source)
    .expect("direct-file example should execute");
    assert_eq!(values[0].project_compat_slot_i32(), Ok(42));
}

#[test]
fn example_basproj_exe_with_explicit_entrypoint_executes() {
    let temp_root = unique_temp_dir("oxvba_host_example_basproj_entry");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");
    std::fs::write(
        temp_root.join("Main.bas"),
        "Public Sub Main()\nError 1\nEnd Sub\n",
    )
    .expect("write main module");
    std::fs::write(
        temp_root.join("Startup.bas"),
        "Public valueOut As Long\nPublic Sub Boot()\nvalueOut = 42\nEnd Sub\n",
    )
    .expect("write startup module");
    std::fs::write(
        temp_root.join("ProjectA.basproj"),
        "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
    <EntryPoint>Startup.Boot</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"Main.bas\" />
    <Module Include=\"Startup.bas\" />
  </ItemGroup>
</Project>
",
    )
    .expect("write basproj");

    let loaded =
        oxvba_project::load_basproj(&temp_root.join("ProjectA.basproj")).expect("load basproj");
    let values = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    })
    .execute_project_with_snapshot_phased(&loaded.manifest)
    .expect("basproj example should execute");
    assert!(
        values.is_empty(),
        "explicit-entry project example should execute with the current empty snapshot slot shape: {values:?}"
    );

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn example_convention_directory_top_level_mainline_executes() {
    let temp_root = unique_temp_dir("oxvba_host_example_convention");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");
    std::fs::write(
        temp_root.join("ScriptModule.bas"),
        concat!(
            "Option Explicit\n",
            "Public valueOut As Long\n",
            "Private counter As Long\n",
            "counter = 41\n",
            "valueOut = counter\n",
            "Call Bump(valueOut)\n",
            "Public Sub Bump(ByRef value)\n",
            "    value = value + 1\n",
            "End Sub\n",
        ),
    )
    .expect("write script module");

    let loaded = oxvba_project::load_basproj_from_str(
        "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ConventionProject</ProjectName>
  </PropertyGroup>
</Project>
",
        &temp_root,
    )
    .expect("load convention project");
    Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    })
    .execute_project_with_snapshot_phased(&loaded.manifest)
    .expect("convention example should execute");

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn example_vbp_sub_main_executes() {
    let temp_root = unique_temp_dir("oxvba_host_example_vbp");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");
    std::fs::write(
        temp_root.join("Main.bas"),
        "Public valueOut As Long\nPublic Sub Main()\nvalueOut = 42\nEnd Sub\n",
    )
    .expect("write main module");
    std::fs::write(
        temp_root.join("Project1.vbp"),
        "Type=Exe\nName=\"Project1\"\nStartup=\"Sub Main\"\nModule=Main; Main.bas\n",
    )
    .expect("write vbp");

    let loaded = oxvba_project::load_vbp(&temp_root.join("Project1.vbp")).expect("load vbp");
    let values = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    })
    .execute_project_with_snapshot_phased(&loaded.manifest)
    .expect("vbp example should execute");
    assert!(
        values.is_empty(),
        "VBP Sub Main example should execute with the current empty snapshot slot shape: {values:?}"
    );

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn example_library_project_compiles_without_startup_mainline() {
    let temp_root = unique_temp_dir("oxvba_host_example_library");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");
    std::fs::write(
        temp_root.join("LibraryModule.bas"),
        "Public Function ExampleValue() As Long\nExampleValue = 42\nEnd Function\n",
    )
    .expect("write library module");
    let loaded = oxvba_project::load_basproj_from_str(
        "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Library</OutputType>
    <ProjectName>ExampleLibrary</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"LibraryModule.bas\" />
  </ItemGroup>
</Project>
",
        &temp_root,
    )
    .expect("library project should load");
    oxvba_compiler::compile_project(&loaded.manifest)
        .expect("library example should compile without startup mainline");

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}
