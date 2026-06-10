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
fn filecopy_and_filelen_round_trip() {
    // FileCopy duplicates a file; FileLen reports the copy's byte size; CurDir
    // returns a non-empty working directory. (Snapshot order: globals.)
    let dir = unique_temp_dir("fileops");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let src = dir.join("src.bin");
    let dst = dir.join("dst.bin");
    std::fs::write(&src, b"abcde").expect("seed src"); // 5 bytes
    let source = format!(
        "Public n As Long\nPublic cwd As String\n\
         Sub Main()\n    FileCopy \"{src}\", \"{dst}\"\n    n = FileLen(\"{dst}\")\n    cwd = CurDir()\nEnd Sub\n",
        src = vba_literal(&src),
        dst = vba_literal(&dst),
    );
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    let copied = dst.is_file();
    let _ = std::fs::remove_dir_all(&dir);
    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    assert!(copied, "FileCopy did not create {}", dst.display());
    assert_eq!(snap[0].as_i32(), Some(5), "FileLen should be 5: {snap:?}");
    assert!(
        snap[1].as_bstr().is_some_and(|s| !s.as_str().is_empty()),
        "CurDir should be non-empty: {snap:?}"
    );
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

fn clear_readonly(path: &std::path::Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        // Windows-only test teardown: clearing the read-only bit so the temp file
        // can be deleted. The cross-platform caveat the lint warns about (Unix
        // mode bits) does not apply on this `#![cfg(target_os = "windows")]` lane.
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = std::fs::set_permissions(path, perms);
    }
}

#[test]
fn getattr_and_setattr_round_trip() {
    // GetAttr reports a directory's vbDirectory bit; SetAttr toggles vbReadOnly on
    // a file and GetAttr reflects it; SetAttr vbNormal clears it. (Snapshot order:
    // globals in declaration order.)
    let dir = unique_temp_dir("attrs");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("a.txt");
    std::fs::write(&file, b"x").expect("seed file");
    let source = format!(
        "Public dirAttr As Long\nPublic roAttr As Long\nPublic clearedAttr As Long\n\
         Sub Main()\n\
         \u{20}   dirAttr = GetAttr(\"{dir}\")\n\
         \u{20}   SetAttr \"{file}\", vbReadOnly\n\
         \u{20}   roAttr = GetAttr(\"{file}\")\n\
         \u{20}   SetAttr \"{file}\", vbNormal\n\
         \u{20}   clearedAttr = GetAttr(\"{file}\")\n\
         End Sub\n",
        dir = vba_literal(&dir),
        file = vba_literal(&file),
    );
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    clear_readonly(&file); // ensure teardown can delete it
    let _ = std::fs::remove_dir_all(&dir);
    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    assert_eq!(
        snap[0].as_i32().map(|v| v & 16),
        Some(16),
        "directory should carry vbDirectory: {snap:?}"
    );
    assert_eq!(
        snap[1].as_i32().map(|v| v & 1),
        Some(1),
        "file should be read-only after SetAttr vbReadOnly: {snap:?}"
    );
    assert_eq!(
        snap[2].as_i32().map(|v| v & 1),
        Some(0),
        "read-only should be cleared after SetAttr vbNormal: {snap:?}"
    );
}

#[test]
fn filedatetime_reads_a_files_modification_time() {
    // A just-written file's FileDateTime must sit within a minute of Now() — both
    // are built from the same UTC serial model, so this holds regardless of the
    // test machine's time zone, without asserting an absolute timestamp.
    let dir = unique_temp_dir("fdt");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("seed dir");
    let file = dir.join("recent.txt");
    std::fs::write(&file, b"x").expect("seed file");
    let source = format!(
        "Public gap As Double\n\
         Sub Main()\n    gap = Abs(Now() - FileDateTime(\"{file}\"))\nEnd Sub\n",
        file = vba_literal(&file),
    );
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let snap = engine.execute_source_with_variant_snapshot_clean(&source);
    let _ = std::fs::remove_dir_all(&dir);
    let snap = snap.unwrap_or_else(|d| panic!("{:?}: {}", d.phase(), d.message()));
    let gap_days = snap[0].as_f64().expect("gap is a Double");
    // One minute = 1/1440 of a day; a freshly written file is well within that.
    assert!(
        gap_days < 1.0 / 1440.0,
        "FileDateTime should be ~Now for a just-written file, gap was {gap_days} days"
    );
}

#[test]
fn chdrive_runs_against_the_current_drive() {
    // ChDrive selects a drive's current directory. Drive a path on the current
    // drive and confirm it executes; save/restore the process CWD so sibling
    // tests (which all use absolute temp paths) are unaffected.
    let saved = std::env::current_dir().expect("cwd");
    let drive_letter = saved
        .to_string_lossy()
        .chars()
        .next()
        .expect("a drive letter")
        .to_string();
    let source = format!("Sub Main()\n    ChDrive \"{drive_letter}\"\nEnd Sub\n");
    let result = run_clean(&source);
    let _ = std::env::set_current_dir(&saved);
    result.expect("ChDrive on the current drive should succeed");
}
