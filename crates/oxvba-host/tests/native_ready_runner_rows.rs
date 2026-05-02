use std::collections::BTreeMap;

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_host::{
    NATIVE_READY_RUNNER_SCHEMA_HEADER, NativeReadyRunnerConfig, emit_native_ready_vm_jit_csv,
    produce_native_ready_vm_jit_rows,
};

fn manifest_from_source(source: &str) -> ProjectManifest {
    ProjectManifest {
        project_name: "NativeReadyRunnerSmoke".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![
            module_unit_from_source("MainModule", ModuleKind::Procedural, source)
                .expect("module parses"),
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    }
}

fn runner_config() -> NativeReadyRunnerConfig {
    NativeReadyRunnerConfig::correctness(
        "nr-recovery-smoke-001",
        "2026-05-02T00:00:00Z",
        "NR-RUNNER-001",
        "Native-ready runner smoke",
        "crates/oxvba-host/tests/native_ready_runner_rows.rs",
    )
}

#[test]
fn native_ready_vm_jit_runner_produces_schema_rows() {
    let manifest = manifest_from_source(
        r#"
Option Explicit
Public Sub Main()
    Dim x As Long
    x = 40 + 2
End Sub
"#,
    );

    let rows = produce_native_ready_vm_jit_rows(&manifest, &runner_config())
        .expect("runner rows should be produced");
    assert_eq!(rows.len(), 2);

    let vm = rows.iter().find(|row| row.backend == "vm").expect("vm row");
    assert_eq!(vm.artifact_kind, "none");
    assert_eq!(vm.artifact_size_bytes, 0);
    assert_eq!(vm.exit_status, 0);
    assert!(!vm.fallback_used);
    assert_eq!(vm.fallback_reason, "not-applicable");
    assert_eq!(vm.result_kind, "variant-snapshot");
    assert!(vm.result_digest.starts_with("fnv1a64:"));

    let jit = rows
        .iter()
        .find(|row| row.backend == "jit")
        .expect("jit row");
    assert_eq!(jit.exit_status, 0);
    assert!(jit.fallback_used);
    assert_eq!(jit.fallback_reason, "project-visible-snapshot-vm-fallback");
    assert!(jit.result_digest.starts_with("fnv1a64:"));
    assert_eq!(jit.result_digest, vm.result_digest);
}

#[test]
fn native_ready_runner_csv_uses_locked_header_and_data_rows() {
    let manifest = manifest_from_source(
        r#"
Option Explicit
Public Sub Main()
    Dim total As Long
    total = 7 * 6
End Sub
"#,
    );

    let csv =
        emit_native_ready_vm_jit_csv(&manifest, &runner_config()).expect("csv should be emitted");
    let mut lines = csv.lines();
    assert_eq!(lines.next(), Some(NATIVE_READY_RUNNER_SCHEMA_HEADER));

    let row_lines: Vec<&str> = lines.collect();
    assert_eq!(row_lines.len(), 2, "csv={csv}");
    assert!(
        row_lines.iter().any(|line| line.contains(",vm,")),
        "csv={csv}"
    );
    assert!(
        row_lines.iter().any(|line| line.contains(",jit,")),
        "csv={csv}"
    );
    assert!(
        row_lines
            .iter()
            .any(|line| line.contains(",true,project-visible-snapshot-vm-fallback,")),
        "csv={csv}"
    );
}
