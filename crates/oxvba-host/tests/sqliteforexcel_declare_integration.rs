//! Real-world acceptance test: the SQLiteForExcel VBA project driving the real
//! `sqlite3.dll` through `Declare Lib` on the clean stack (closure → bind →
//! linearize → vm2 → HAL native FFI). The bounded demo opens an in-memory
//! database, runs statements, and closes — completing only if native Declare
//! execution (lane routing, ByVal/ByRef marshalling, StrPtr/VarPtr buffers +
//! write-back, RtlMoveMemory) all work end to end. Windows-only; uses the
//! fixtures + x64 sqlite3.dll vendored under `.external/sqliteforexcel`.
#![cfg(target_os = "windows")]

use std::path::{Path, PathBuf};

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn bounded_demo_basproj() -> PathBuf {
    workspace_root().join(
        ".external/sqliteforexcel/fixtures/Demo64NormalizedBounded/SQLiteForExcelDemo64NormalizedBounded.basproj",
    )
}

/// Load the bounded demo's project-reference closure and run it on the requested
/// backend under the interactive-dev policy (which enables native dynamic linking).
///
/// The fixture's `ThisWorkbook.Path` is a workspace-relative directory
/// (`.external/sqliteforexcel/upstream/Distribution`) — in real Excel it would be the
/// workbook's absolute folder. `SQLite3Initialize` `LoadLibrary`s
/// `ThisWorkbook.Path + "\x64\SQLite3.dll"`, so the run must happen with the
/// workspace root as the current directory for that relative path to resolve.
fn run_bounded_demo(jit_requested: bool) -> Result<(), String> {
    std::env::set_current_dir(workspace_root())
        .map_err(|err| format!("could not set cwd to the workspace root: {err}"))?;
    let closure = oxvba_project::load_project_closure_with_entry(&bounded_demo_basproj(), None)
        .map_err(|err| format!("closure load failed: {err}"))?;
    let mut engine = Engine::new(if jit_requested {
        HostConfig::jit()
    } else {
        HostConfig::vm3()
    });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .execute_project_closure_with_variant_snapshot(&closure)
        .map(|_| ())
        .map_err(|err| err.to_string())
}

#[test]
fn bounded_demo_completes_on_vm_via_native_sqlite() {
    run_bounded_demo(false).expect(
        "SQLiteForExcel bounded demo should complete on the VM backend via native sqlite3.dll",
    );
}

#[test]
fn bounded_demo_rejects_jit_without_falling_back() {
    let err = run_bounded_demo(true)
        .expect_err("JIT execution is not implemented; it must not silently fall back to the VM");
    assert!(
        err.contains("JIT execution"),
        "unexpected diagnostic: {err}"
    );
}
