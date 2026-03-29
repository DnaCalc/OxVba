use oxvba_compiler::{OxBundle, compile_project};
use oxvba_host::{Engine, HostConfig};
use oxvba_project::load_basproj_from_str;
use oxvba_runtime::RuntimeValue;

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

fn load_project(main_source: &str) -> oxvba_project::LoadedProject {
    let temp_root = unique_temp_dir("oxvba_loaded_project_session_duplication");
    std::fs::create_dir_all(&temp_root).expect("create temp project root");
    std::fs::write(temp_root.join("Main.bas"), main_source).expect("write main module");

    let loaded = load_basproj_from_str(
        "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>LoadedProjectSessionDuplication</ProjectName>
    <EntryPoint>Main.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"Main.bas\" />
  </ItemGroup>
</Project>
",
        &temp_root,
    )
    .expect("load project");

    std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
    loaded
}

#[test]
fn compile_and_prepare_session_does_not_run_loaded_entry_shim() {
    let loaded = load_project(concat!(
        "Attribute VB_Name = \"Main\"\n",
        "Public runs As Long\n",
        "Public Function Main() As Long\n",
        "runs = runs + 1\n",
        "Main = runs\n",
        "End Function\n"
    ));

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let mut session = engine
        .compile_and_prepare_session(&loaded.manifest)
        .expect("session prep should succeed");
    let result = engine
        .invoke_procedure(&mut session, "Main", "Main", &[])
        .expect("manual invoke should succeed");

    assert_eq!(result, RuntimeValue::I32(1));
}

#[test]
fn compile_and_prepare_session_from_bundle_does_not_run_loaded_entry_shim() {
    let loaded = load_project(concat!(
        "Attribute VB_Name = \"Main\"\n",
        "Public runs As Long\n",
        "Public Function Main() As Long\n",
        "runs = runs + 1\n",
        "Main = runs\n",
        "End Function\n"
    ));
    let compiled = compile_project(&loaded.manifest).expect("project should compile");
    let bundle = OxBundle::from_compiled_project(&compiled, &loaded.manifest.project_name);

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let mut session = engine
        .compile_and_prepare_session_from_bundle(&bundle)
        .expect("bundle session prep should succeed");
    let result = engine
        .invoke_procedure(&mut session, "Main", "Main", &[])
        .expect("manual invoke should succeed");

    assert_eq!(result, RuntimeValue::I32(1));
}
