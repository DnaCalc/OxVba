use std::path::{Path, PathBuf};

use oxvba_compiler::{
    ModuleKind, ProjectKind, ProjectManifest, ProjectReference, ReferenceKind, compile_project,
    module_unit_from_source,
};
use oxvba_host::engine::DiagnosticPhase;
use oxvba_host::{Engine, HostConfig};
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

#[cfg(target_os = "windows")]
#[test]
fn sqliteforexcel_core64_normalized_basproj_reports_current_thisworkbook_path_compile_failure() {
    let basproj_path =
        sqlite_fixture_root().join("Core64Normalized/SQLiteForExcelCore64Normalized.basproj");
    let loaded = load_basproj(&basproj_path).expect("sqlite core fixture should load");

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .expect_err("sqlite core fixture should currently fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("use of undeclared variable: thisworkbook_path"),
        "unexpected compile diagnostic: {}",
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
        matches!(workbook.module_kind, ModuleKind::Class | ModuleKind::Document),
        "unexpected workbook module kind {:?} in {:?}",
        workbook.module_kind,
        loaded_names
    );
    assert!(
        workbook.attributes.vb_predeclared_id,
        "ThisWorkbook should remain predeclared"
    );
    assert!(
        workbook.source.contains("Public Property Get Path() As String"),
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
fn sqliteforexcel_sqlite3_module_source_direct_compile_reproduces_thisworkbook_path_failure() {
    let core_path = sqlite_fixture_root().join("Core64Normalized/SQLiteForExcelCore64Normalized.basproj");
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

    let err = compile_project(&manifest)
        .expect_err("direct compile should currently reproduce sqlite fixture failure");
    let rendered = err.to_string();
    assert!(
        rendered.contains("use of undeclared variable: thisworkbook_path"),
        "unexpected direct compile diagnostic: {rendered}"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn sqliteforexcel_demo64_normalized_basproj_reports_current_sqlite3open_duplicate_failure() {
    let basproj_path =
        sqlite_fixture_root().join("Demo64Normalized/SQLiteForExcelDemo64Normalized.basproj");
    let loaded = load_basproj(&basproj_path).expect("sqlite demo fixture should load");

    let engine = Engine::new(HostConfig {
        enable_jit: false,
        root_object_name: None,
    });
    let err = engine
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .expect_err("sqlite demo fixture should currently fail at compile-time");
    assert_eq!(err.phase(), DiagnosticPhase::CompileTime);
    assert!(
        err.message()
            .contains("PMR-E-NAME-QUALIFICATION-REQUIRED")
            && err.message().to_ascii_lowercase().contains("sqlite3open"),
        "unexpected compile diagnostic: {}",
        err.message()
    );
}

#[test]
fn sqliteforexcel_demo_module_sources_direct_compile_reproduce_sqlite3open_duplicate_failure() {
    let demo_path = sqlite_fixture_root().join("Demo64Normalized/SQLiteForExcelDemo64Normalized.basproj");
    let demo = load_basproj(&demo_path).expect("sqlite demo fixture should load");

    let err = compile_project(&demo.manifest)
        .expect_err("direct compile should currently reproduce sqlite demo duplicate-name failure");
    let rendered = err.to_string();
    assert!(
        rendered.contains("PMR-E-NAME-QUALIFICATION-REQUIRED")
            && rendered.to_ascii_lowercase().contains("sqlite3open"),
        "unexpected direct compile diagnostic: {rendered}"
    );
}
