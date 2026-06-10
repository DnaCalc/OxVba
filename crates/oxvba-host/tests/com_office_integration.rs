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

use std::path::PathBuf;

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

/// A unique temp path for a test database, removed up front and on teardown so
/// the run is repeatable. `tag` keeps concurrent lanes from colliding.
struct TempDbPath(PathBuf);

impl TempDbPath {
    fn new(tag: &str) -> Self {
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("oxvba_{tag}_{pid}.accdb"));
        let _ = std::fs::remove_file(&path);
        Self(path)
    }

    fn as_vba_literal(&self) -> String {
        // VBA string literal: backslashes are fine; only `"` needs doubling.
        self.0.to_string_lossy().replace('"', "\"\"")
    }
}

impl Drop for TempDbPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
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

#[test]
#[ignore = "launches the real ACE DAO engine; run explicitly with --ignored"]
fn dao_late_bound_create_table_insert_query() {
    // Late-bound DAO via the Access Database Engine (ACE): create a database,
    // a table, insert a row, and read it back through a recordset — a real
    // data round-trip with no Access UI. Exercises method calls with args
    // (CreateDatabase, Execute, OpenRecordset) and indexed Fields(0).
    let db = TempDbPath::new("dao_smoke");
    let source = format!(
        "Public got As Long\n\
         Sub Main()\n\
         Dim eng As Object\n\
         Set eng = CreateObject(\"DAO.DBEngine.120\")\n\
         Dim db As Object\n\
         Set db = eng.CreateDatabase(\"{path}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         db.Execute \"CREATE TABLE T (N LONG)\"\n\
         db.Execute \"INSERT INTO T (N) VALUES (7)\"\n\
         Dim rs As Object\n\
         Set rs = db.OpenRecordset(\"SELECT N FROM T\")\n\
         got = rs.Fields(0).Value\n\
         rs.Close\n\
         db.Close\n\
         End Sub\n",
        path = db.as_vba_literal()
    );
    match run_clean(&source) {
        Ok(snap) => {
            assert!(
                snap.iter().any(|v| v.as_i32() == Some(7)),
                "expected the DAO recordset value 7 in {snap:?}"
            );
        }
        Err(err) if is_component_absent(&err) => {
            eprintln!("SKIP: DAO.DBEngine.120 (ACE) not registered: {err}");
        }
        Err(err) => panic!("DAO late-bound round-trip failed: {err}"),
    }
}

#[test]
#[ignore = "launches real Access; run explicitly with --ignored"]
fn access_late_bound_application_activation() {
    // Late-bound Access.Application activation: read a root property and quit.
    // Proves activation + property-get against the heavyweight Access app.
    let source = "Public build As Long\n\
         Sub Main()\n\
         Dim app As Object\n\
         Set app = CreateObject(\"Access.Application\")\n\
         app.Visible = False\n\
         build = app.Build\n\
         app.Quit\n\
         End Sub\n";
    match run_clean(source) {
        Ok(snap) => {
            assert!(
                snap.iter().any(|v| v.as_i32().is_some_and(|n| n > 0)),
                "expected a positive Access.Build in {snap:?}"
            );
        }
        Err(err) if is_component_absent(&err) => {
            eprintln!("SKIP: Access.Application not registered: {err}");
        }
        Err(err) => panic!("Access late-bound activation failed: {err}"),
    }
}
