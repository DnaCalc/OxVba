//! Filesystem statement intrinsics (`MkDir`/`RmDir`) end to end through the
//! clean stack under the interactive-dev policy (real host filesystem). `Kill`
//! is covered by the native-declare/SQLiteForExcel paths; these guard the
//! directory statements ChibiPDF and similar real-world code use.
#![cfg(target_os = "windows")]

use std::path::PathBuf;

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig};

fn run_clean(source: &str) -> Result<(), String> {
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .execute_source_with_variant_snapshot_clean(source)
        .map(|_| ())
        .map_err(|d| format!("{:?}: {}", d.phase(), d.message()))
}

fn unique_temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("oxvba_{tag}_{}", std::process::id()))
}

fn vba_literal(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('"', "\"\"")
}

#[test]
fn mkdir_creates_a_real_directory() {
    let dir = unique_temp_dir("mkdir");
    let _ = std::fs::remove_dir_all(&dir);
    let source = format!("Sub Main()\n    MkDir \"{}\"\nEnd Sub\n", vba_literal(&dir));
    let result = run_clean(&source);
    let exists = dir.is_dir();
    let _ = std::fs::remove_dir_all(&dir); // teardown regardless of outcome
    result.expect("MkDir should succeed");
    assert!(exists, "MkDir did not create {}", dir.display());
}

#[test]
fn rmdir_removes_an_empty_directory() {
    let dir = unique_temp_dir("rmdir");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir(&dir).expect("seed dir");
    let source = format!("Sub Main()\n    RmDir \"{}\"\nEnd Sub\n", vba_literal(&dir));
    let result = run_clean(&source);
    let still_exists = dir.exists();
    let _ = std::fs::remove_dir_all(&dir);
    result.expect("RmDir should succeed");
    assert!(!still_exists, "RmDir did not remove {}", dir.display());
}
