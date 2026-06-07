//! End-to-end clean path from on-disk `.basproj` files: a two-project workspace
//! (App → Lib) loaded with `oxvba_project::load_project_closure`, then run on the
//! new pipeline via `Engine::execute_project_closure_with_variant_snapshot`
//! (`bind_projects` → `linearize` → `oxvba_vm2::Vm::link` → run). Proves the host
//! runs a cross-project workspace straight from disk.

use std::path::{Path, PathBuf};

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
        .map(|(file, _)| format!("    <Module Include=\"{file}\" />\n"))
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
