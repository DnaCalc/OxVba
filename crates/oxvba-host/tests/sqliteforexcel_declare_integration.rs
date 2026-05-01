use std::path::{Path, PathBuf};

use oxvba_compiler::{
    ModuleKind, ProjectKind, ProjectManifest, ProjectReference, ReferenceKind, compile_project,
    module_unit_from_source,
};
use oxvba_hal::model::HostPolicy;
use oxvba_host::engine::DiagnosticPhase;
use oxvba_host::{Engine, HostConfig, compat::RuntimeValueCompatEngineExt};
use oxvba_project::load_basproj;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn sqlite_fixture_root() -> PathBuf {
    workspace_root().join(".external/sqliteforexcel/fixtures")
}

fn sqlite_bounded_demo_basproj() -> PathBuf {
    sqlite_fixture_root()
        .join("Demo64NormalizedBounded/SQLiteForExcelDemo64NormalizedBounded.basproj")
}

#[cfg(target_os = "windows")]
#[test]
fn sqliteforexcel_core64_normalized_basproj_moves_past_compile_frontiers_to_runtime_loadlibrary_boundary_in_vm()
 {
    let basproj_path =
        sqlite_fixture_root().join("Core64Normalized/SQLiteForExcelCore64Normalized.basproj");
    let loaded = load_basproj(&basproj_path).expect("sqlite core fixture should load");

    let mut engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let err = engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .expect_err("sqlite core fixture should now reach the runtime LoadLibrary boundary");
    assert_eq!(err.phase(), DiagnosticPhase::Runtime);
    assert!(
        err.message()
            .to_ascii_lowercase()
            .contains("loadlibraryw failed"),
        "unexpected runtime diagnostic: {}",
        err.message()
    );
}

#[test]
fn sqliteforexcel_host_environment_reference_loads_expected_predeclared_path_property() {
    let basproj_path = sqlite_fixture_root().join("HostEnvironment/HostEnvironment.basproj");
    let loaded = load_basproj(&basproj_path).expect("host environment fixture should load");
    let loaded_names = loaded
        .manifest
        .modules
        .iter()
        .map(|module| format!("{}:{:?}", module.module_name, module.module_kind))
        .collect::<Vec<_>>();
    let workbook = loaded
        .manifest
        .modules
        .iter()
        .find(|module| module.module_name.eq_ignore_ascii_case("ThisWorkbook"))
        .unwrap_or_else(|| panic!("predeclared workbook module should load; got {loaded_names:?}"));
    assert!(
        matches!(
            workbook.module_kind,
            ModuleKind::Class | ModuleKind::Document
        ),
        "unexpected workbook module kind {:?} in {:?}",
        workbook.module_kind,
        loaded_names
    );
    assert!(
        workbook.attributes.vb_predeclared_id,
        "ThisWorkbook should remain predeclared"
    );
    assert!(
        workbook
            .source
            .contains("Public Property Get Path() As String"),
        "expected Path property getter in loaded workbook source: {}",
        workbook.source
    );
    assert!(
        workbook
            .source
            .contains("Path = \".external\\sqliteforexcel\\upstream\\Distribution\""),
        "expected stable literal path in loaded workbook source: {}",
        workbook.source
    );
}

#[test]
fn sqliteforexcel_sqlite3_module_source_direct_compile_moves_past_pointer_and_redim_boundaries() {
    let core_path =
        sqlite_fixture_root().join("Core64Normalized/SQLiteForExcelCore64Normalized.basproj");
    let host_path = sqlite_fixture_root().join("HostEnvironment/HostEnvironment.basproj");
    let core = load_basproj(&core_path).expect("sqlite core fixture should load");
    let host = load_basproj(&host_path).expect("host environment fixture should load");

    let loaded_names = core
        .manifest
        .modules
        .iter()
        .map(|module| format!("{}:{:?}", module.module_name, module.module_kind))
        .collect::<Vec<_>>();
    let sqlite3_module = core
        .manifest
        .modules
        .iter()
        .find(|module| module.module_name.eq_ignore_ascii_case("Sqlite3"))
        .unwrap_or_else(|| panic!("sqlite3 module should load; got {loaded_names:?}"))
        .clone();
    let main_module = module_unit_from_source(
        "MainModule",
        ModuleKind::Procedural,
        "Attribute VB_Name = \"MainModule\"\nPublic Sub Main()\nEnd Sub",
    )
    .expect("main module parses");

    let manifest = ProjectManifest {
        project_name: "SQLiteCoreDirectProbe".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main_module, sqlite3_module],
        references: vec![ProjectReference {
            referenced_project_name: "HostEnvironment".to_string(),
            reference_kind: ReferenceKind::Project,
        }],
        reference_projects: vec![oxvba_compiler::ReferencedProjectManifest {
            project_name: "HostEnvironment".to_string(),
            modules: host.manifest.modules.clone(),
        }],
        conditional_constants: core.manifest.conditional_constants.clone(),
    };

    compile_project(&manifest)
        .expect("direct compile should now succeed for the normalized core sqlite module");
}

#[cfg(target_os = "windows")]
#[test]
fn sqliteforexcel_demo64_normalized_basproj_now_reaches_runtime_boundary_after_compile_frontiers() {
    let basproj_path =
        sqlite_fixture_root().join("Demo64Normalized/SQLiteForExcelDemo64Normalized.basproj");
    let loaded = load_basproj(&basproj_path).expect("sqlite demo fixture should load");

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .expect_err("sqlite demo fixture should currently fail at runtime");
    assert_eq!(err.phase(), DiagnosticPhase::Runtime);
}

#[test]
fn sqliteforexcel_demo_module_sources_direct_compile_now_succeeds_after_compile_frontiers() {
    let demo_path =
        sqlite_fixture_root().join("Demo64Normalized/SQLiteForExcelDemo64Normalized.basproj");
    let demo = load_basproj(&demo_path).expect("sqlite demo fixture should load");

    compile_project(&demo.manifest)
        .expect("direct compile should now succeed for the normalized sqlite demo manifest");
}

#[cfg(target_os = "windows")]
#[test]
fn sqliteforexcel_bounded_normalized_demo_completes_in_vm_and_jit() {
    let basproj_path = sqlite_bounded_demo_basproj();
    let loaded = load_basproj(&basproj_path).expect("bounded sqlite demo fixture should load");

    for enable_jit in [false, true] {
        let mut engine = Engine::new(HostConfig {
            enable_jit,
            root_object_name: None,
        });
        engine.set_host_policy(HostPolicy::interactive_dev());
        engine
            .execute_project_with_snapshot_phased(&loaded.manifest)
            .unwrap_or_else(|err| {
                panic!(
                    "bounded sqlite demo fixture should complete for enable_jit={enable_jit}: {}",
                    err
                )
            });
    }
}
