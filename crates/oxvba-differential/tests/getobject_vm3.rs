//! vm3 `GetObject` — deterministic (non-native) binding + error-path coverage.
//!
//! The live activation modes (running-instance via `GetActiveObject`, file bind via
//! `CoGetObject`) are covered by the `#[ignore]` live tests in
//! `oxvba-host/tests/com_matrix_getobject.rs`. This file pins the DETERMINISTIC behaviour
//! the standard differential harness exercises (`deterministic_runtime` → `native_com_enabled`
//! is always false): every `GetObject` shape binds and routes, the running-instance and
//! file-bind modes decline without native COM (raising — never panicking/AV-ing), the invalid
//! shapes raise the same way regardless of profile, and the empty-string mode delegates to
//! `CreateObject`. Reaching each assertion is itself the no-panic guard.
//!
//! The raised `Err.Number` is the live-verified one (no longer flattened to 5): the
//! running-instance shape is 429, the file-bind shape is 432.

use oxvba_differential::{Executor, RunOutcome, run};

fn run_main(body: &str) -> RunOutcome {
    let source = format!("Sub Main()\n    Dim x As Object\n{body}\nEnd Sub\n");
    run(Executor::Vm3, &source)
}

#[test]
fn getobject_running_instance_declines_without_native_com() {
    // `GetObject(, "Excel.Application")` (omitted pathname) has no headless equivalent.
    let outcome = run_main("    Set x = GetObject(, \"Excel.Application\")");
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.result.is_err(),
        "running-instance GetObject should raise without native COM, got {:?}",
        outcome.result
    );
    assert_eq!(
        outcome.err.number, 429,
        "running-instance GetObject → 429; err={:?}",
        outcome.err
    );
}

#[test]
fn getobject_file_bind_declines_without_native_com() {
    // `GetObject("<path>")` (non-empty pathname) has no headless equivalent.
    let outcome = run_main("    Set x = GetObject(\"C:\\does\\not\\exist.doc\")");
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.result.is_err(),
        "file-bind GetObject should raise without native COM, got {:?}",
        outcome.result
    );
    assert_eq!(
        outcome.err.number, 432,
        "file-bind GetObject → 432; err={:?}",
        outcome.err
    );
}

#[test]
fn getobject_without_pathname_or_class_is_invalid() {
    // `GetObject("")` with no class is invalid in EVERY mode — the rejection happens before
    // the native gate, so it is identical on native and deterministic profiles.
    let outcome = run_main("    Set x = GetObject(\"\")");
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.result.is_err(),
        "GetObject with neither pathname nor class should raise, got {:?}",
        outcome.result
    );
}

#[test]
fn getobject_empty_pathname_delegates_to_createobject() {
    // `GetObject("", progid)` routes to `CreateObject(progid)`; in deterministic mode that
    // yields the synthetic projection object (no raise), proving the empty-string mode is
    // wired through the same activation path.
    let outcome = run_main("    Set x = GetObject(\"\", \"Scripting.Dictionary\")");
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.result.is_ok(),
        "empty-string GetObject should delegate to CreateObject, got {:?}",
        outcome.result
    );
}
