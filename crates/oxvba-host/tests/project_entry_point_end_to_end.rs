use oxvba_host::{Engine, HostConfig};
use oxvba_runtime::Variant;

#[test]
fn loaded_basproj_configured_entry_point_bypasses_default_main_startup_path() {
    let unique = format!(
        "oxvba_host_entrypoint_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    );
    let temp_root = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&temp_root).expect("create temp project root");

    let main_path = temp_root.join("MainModule.bas");
    let startup_path = temp_root.join("StartupModule.bas");
    std::fs::write(
        &main_path,
        "Public Sub Main()\n\
         Dim x\n\
         x = 1 / 0\n\
         End Sub\n",
    )
    .expect("write main module");
    std::fs::write(&startup_path, "Public Sub Run()\nEnd Sub\n").expect("write startup module");

    let basproj_path = temp_root.join("ProjectA.basproj");
    std::fs::write(
        &basproj_path,
        "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ProjectA</ProjectName>
    <EntryPoint>StartupModule.Run</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"MainModule.bas\" />
    <Module Include=\"StartupModule.bas\" />
  </ItemGroup>
</Project>
",
    )
    .expect("write basproj");

    let loaded = oxvba_project::load_basproj(&basproj_path).expect("basproj should load");
    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: Some(loaded.default_root_object.clone()),
    });
    let snapshot = engine
        .execute_project_with_variant_snapshot_phased(&loaded.manifest)
        .expect("configured entry point should bypass failing default Main");
    assert_eq!(
        snapshot,
        vec![Variant::empty()],
        "configured entry point snapshot should reflect the entry procedure slot view"
    );

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}

#[test]
fn loaded_basproj_unique_sub_main_fallback_bypasses_first_module_order_bias() {
    let unique = format!(
        "oxvba_host_unique_main_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix epoch")
            .as_nanos()
    );
    let temp_root = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&temp_root).expect("create temp project root");

    let helper_path = temp_root.join("HelperModule.bas");
    let main_path = temp_root.join("MainModule.bas");
    std::fs::write(
        &helper_path,
        "Public Sub Warmup()\n\
         Dim x\n\
         x = 1 / 0\n\
         End Sub\n",
    )
    .expect("write helper module");
    std::fs::write(
        &main_path,
        "Public Sub Main()\n\
         Dim x\n\
         x = 5\n\
         End Sub\n",
    )
    .expect("write main module");

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
    <Module Include=\"HelperModule.bas\" />
    <Module Include=\"MainModule.bas\" />
  </ItemGroup>
</Project>
",
    )
    .expect("write basproj");

    let loaded = oxvba_project::load_basproj(&basproj_path).expect("basproj should load");
    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: Some(loaded.default_root_object.clone()),
    });
    let snapshot = engine
        .execute_project_with_variant_snapshot_phased(&loaded.manifest)
        .expect("unique Sub Main fallback should bypass first-module Warmup");
    assert_eq!(
        snapshot,
        vec![Variant::from_i32(5)],
        "unique Sub Main fallback should reflect the selected Main procedure slot view"
    );

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
}
