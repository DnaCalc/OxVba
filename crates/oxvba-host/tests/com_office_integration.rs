//! Live COM reference tests driving real Office (Excel, Access, DAO) through the
//! clean stack (bind → linearize → vm2 → HAL COM bridge → real CoCreateInstance),
//! in both late-bound (`CreateObject`) and early-bound (typelib) forms, including
//! `WithEvents` sinks.
//!
//! These launch real Office applications, so every test is `#[ignore]` — normal
//! `cargo test` skips them. Run the lane explicitly:
//!
//! ```text
//! cargo test -p oxvba-host --test com_office_integration -- --ignored --test-threads=1
//! ```
//!
//! Each test runs Office invisibly (`Visible = False`, `DisplayAlerts = False`)
//! and quits it in the guest script; a missing component (class not registered)
//! is treated as an environment skip, not a failure. Windows-only.
#![cfg(target_os = "windows")]

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig};
use oxvba_runtime::Variant;

/// Run a single-module VBA source through the clean Engine under the
/// interactive-dev policy (which permits real COM activation), returning the
/// snapshot (module globals followed by `Main`'s locals).
fn run_clean(source: &str) -> Result<Vec<Variant>, String> {
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .execute_source_with_variant_snapshot_clean(source)
        .map_err(|d| format!("{:?}: {}", d.phase(), d.message()))
}

/// Whether an error denotes an absent COM component (class not registered /
/// invalid class string) rather than a real failure — an environment skip.
fn is_component_absent(err: &str) -> bool {
    err.contains("8004_0154")
        || err.contains("80040154")
        || err.contains("8004_01F3")
        || err.contains("800401F3")
        || err.to_ascii_lowercase().contains("not registered")
        || err.to_ascii_lowercase().contains("invalid class string")
}

#[test]
#[ignore = "launches real Excel; run explicitly with --ignored"]
fn excel_late_bound_range_value_round_trips() {
    // Late-bound: CreateObject → Workbooks.Add → Worksheets(1) indexed get →
    // Range("A1") method call → Value property put then get. Round-trips a
    // Double through real Excel and back into a guest global.
    let source = "Public result As Double\n\
         Sub Main()\n\
         Dim app As Object\n\
         Set app = CreateObject(\"Excel.Application\")\n\
         app.Visible = False\n\
         app.DisplayAlerts = False\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         Dim ws As Object\n\
         Set ws = wb.Worksheets(1)\n\
         ws.Range(\"A1\").Value = 42.5\n\
         result = ws.Range(\"A1\").Value\n\
         wb.Close False\n\
         app.Quit\n\
         End Sub\n";
    match run_clean(source) {
        Ok(snap) => {
            assert!(
                snap.iter().any(|v| v.as_f64() == Some(42.5)),
                "expected the Excel Range round-trip 42.5 in {snap:?}"
            );
        }
        Err(err) if is_component_absent(&err) => {
            eprintln!("SKIP: Excel.Application not registered: {err}");
        }
        Err(err) => panic!("Excel late-bound round-trip failed: {err}"),
    }
}
