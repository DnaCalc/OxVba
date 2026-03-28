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
fn basproj_exe_executes_unique_top_level_mainline() {
    let temp_root = unique_temp_dir("oxvba_host_top_level_mainline");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");

    let script_path = temp_root.join("ScriptModule.bas");
    std::fs::write(
        &script_path,
        "valueOut = 41\nCall Bump(valueOut)\nSub Bump(ByRef value)\nvalue = value + 1\nEnd Sub\n",
    )
    .expect("write script module");

    let basproj_path = temp_root.join("ProjectA.basproj");
    std::fs::write(
        &basproj_path,
        "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"ScriptModule.bas\" />
  </ItemGroup>
</Project>
",
    )
    .expect("write basproj");

    let loaded = oxvba_project::load_basproj(&basproj_path).expect("basproj should load");
    assert_eq!(
        loaded.entry_point.as_deref(),
        Some("ScriptModule.__OxVbaTopLevelMainline")
    );
    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .expect("project execution should succeed");

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn basproj_exe_top_level_mainline_preserves_option_private_module_and_module_state() {
    let temp_root = unique_temp_dir("oxvba_host_top_level_option_private");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");

    let script_path = temp_root.join("ScriptModule.bas");
    std::fs::write(
        &script_path,
        "Option Private Module\nPrivate counter As Long\ncounter = 41\nCall Bump\nPublic Sub Bump()\ncounter = counter + 1\nvalueOut = counter\nEnd Sub\n",
    )
    .expect("write script module");

    let basproj_path = temp_root.join("ProjectA.basproj");
    std::fs::write(
        &basproj_path,
        "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"ScriptModule.bas\" />
  </ItemGroup>
</Project>
",
    )
    .expect("write basproj");

    let loaded = oxvba_project::load_basproj(&basproj_path).expect("basproj should load");
    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .expect("project execution should succeed with module-scope state preserved");

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn vbp_exe_sub_main_fallback_executes_supported_project() {
    let temp_root = unique_temp_dir("oxvba_host_vbp_sub_main");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");

    std::fs::write(temp_root.join("Main.bas"), "Public Sub Main()\nEnd Sub\n")
        .expect("write main module");
    let vbp_path = temp_root.join("Project1.vbp");
    std::fs::write(
        &vbp_path,
        "Type=Exe\nName=\"Project1\"\nStartup=\"Sub Main\"\nModule=Main; Main.bas\n",
    )
    .expect("write vbp");

    let loaded = oxvba_project::vbp::load_vbp(&vbp_path).expect("vbp should load");
    assert_eq!(loaded.entry_point.as_deref(), Some("Main.Main"));
    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let snapshot = engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .expect("Sub Main fallback project should execute");
    assert!(
        snapshot.is_empty(),
        "startup shim should leave no user slots in empty startup path: {snapshot:?}"
    );

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn vbp_exe_unique_top_level_mainline_executes_supported_project() {
    let temp_root = unique_temp_dir("oxvba_host_vbp_top_level_mainline");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");

    std::fs::write(
        temp_root.join("ScriptModule.bas"),
        "valueOut = 41\nCall Bump(valueOut)\nSub Bump(ByRef value)\nvalue = value + 1\nEnd Sub\n",
    )
    .expect("write script module");
    let vbp_path = temp_root.join("Project1.vbp");
    std::fs::write(
        &vbp_path,
        "Type=Exe\nName=\"Project1\"\nModule=ScriptModule; ScriptModule.bas\n",
    )
    .expect("write vbp");

    let loaded = oxvba_project::vbp::load_vbp(&vbp_path).expect("vbp should load");
    assert_eq!(
        loaded.entry_point.as_deref(),
        Some("ScriptModule.__OxVbaTopLevelMainline")
    );
    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .expect("top-level mainline vbp project should execute");

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn vbp_exe_resolves_project_reference_to_referenced_vbp_project() {
    let temp_root = unique_temp_dir("oxvba_host_vbp_project_reference");
    let lib_root = temp_root.join("LibScale");
    let app_root = temp_root.join("MainApp");
    std::fs::create_dir_all(&lib_root).expect("create temp library root");
    std::fs::create_dir_all(&app_root).expect("create temp app root");

    std::fs::write(
        lib_root.join("M01.bas"),
        "Option Explicit\nPublic Function Value() As Long\nValue = 42\nEnd Function\n",
    )
    .expect("write library module");
    std::fs::write(
        lib_root.join("LibScale.vbp"),
        "Type=OleDll\nName=\"LibScale\"\nModule=M01; M01.bas\n",
    )
    .expect("write library vbp");

    std::fs::write(
        app_root.join("Main.bas"),
        "Call LibScale.M01.Value()\n",
    )
    .expect("write main module");
    std::fs::write(
        app_root.join("MainApp.vbp"),
        "Type=Exe\nName=\"MainApp\"\nReference=*\\A{11111111-2222-3333-4444-555555555555}#1.0#0#..\\LibScale\\LibScale.vbp#LibScale\nModule=Main; Main.bas\n",
    )
    .expect("write app vbp");

    let loaded =
        oxvba_project::vbp::load_vbp(&app_root.join("MainApp.vbp")).expect("vbp should load");
    assert_eq!(
        loaded.entry_point.as_deref(),
        Some("Main.__OxVbaTopLevelMainline")
    );
    assert_eq!(loaded.manifest.reference_projects.len(), 1);
    assert_eq!(loaded.manifest.reference_projects[0].project_name, "LibScale");

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .expect("vbp project reference graph should execute");

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}
