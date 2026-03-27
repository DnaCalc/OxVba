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
