//! Live COM reference tests driving real Office (Excel, Access, DAO) and the
//! `OxVba.TestEventServer` coclass through the clean stack (bind → linearize → vm2 →
//! HAL COM bridge → real CoCreateInstance / IDispatch::Invoke), in both late-bound
//! (`CreateObject`, by-name dispatch) and early-bound (typelib reference, dispatch
//! by dispid) forms, including a COM-source `WithEvents` sink.
//!
//! Coverage:
//! - **Late-bound** (no typelib): Excel Range round-trip, DAO recordset, Access
//!   activation — `CreateObject` + by-name `LateDispatch`.
//! - **Early-bound** (typelib reference → typed receiver → `EarlyCom{dispid}`):
//!   Excel `Application`/`Range` round-trip, and `OxVba.TestEventServer.Ping() = 42`.
//! - **WithEvents** (COM source): a `WithEvents x As OxVba.TestEventServer` sink whose
//!   `x_OnValueChanged` handler fires through the live connection-point dispatch.
//!
//! These launch real Office applications / activate registered coclasses, so every
//! test is `#[ignore]` — normal `cargo test` skips them. Run the lane explicitly:
//!
//! ```text
//! cargo test -p oxvba-host --test com_office_integration -- --ignored --test-threads=1
//! ```
//!
//! Each Office test runs invisibly (`Visible = False`, `DisplayAlerts = False`) and
//! quits in the guest script; a missing component (class/typelib not registered) is
//! treated as an environment skip, not a failure. The early-bound tests reference a
//! typelib by LIBID and skip when that typelib is not registered. Windows-only.
#![cfg(target_os = "windows")]

use std::path::PathBuf;

use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ProjectReference;

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

/// Run a single-module VBA source that carries typelib `references` through the
/// clean Engine under the interactive-dev policy, so a typed receiver resolves to
/// early-bound COM dispatch (dispatch by dispid). Same snapshot as [`run_clean`].
fn run_clean_with_references(
    source: &str,
    references: Vec<ProjectReference>,
) -> Result<Vec<Variant>, String> {
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .execute_source_with_references_and_snapshot(source, references)
        .map_err(|d| format!("{:?}: {}", d.phase(), d.message()))
}

/// A typelib reference identified by its LIBID GUID (the way the early-bound tests
/// point at a registered typelib). `request_from` maps `guid` → `libid_hint`, and
/// the live resolver loads it via `LoadRegTypeLib`.
fn typelib_ref_by_libid(name: &str, guid: &str, major: u16, minor: u16) -> ProjectReference {
    ProjectReference::TypeLibrary {
        name: name.to_string(),
        guid: Some(guid.to_string()),
        version_major: Some(major),
        version_minor: Some(minor),
        lcid: None,
        import_lib: None,
    }
}

/// True if an error denotes an absent/unregistered typelib (the early-bound
/// counterpart of [`is_component_absent`]): the reference never resolved, so the
/// typed receiver had no COM members and the member call failed to bind/dispatch.
fn is_typelib_absent(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("loadregtypelib")
        || lower.contains("type library")
        || lower.contains("typelib")
        || lower.contains("0x8002801d") // TYPE_E_LIBNOTREGISTERED
        || lower.contains("8002801d")
        || is_component_absent(err)
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

#[test]
#[ignore = "requires the registered OxVba.TestEventServer COM coclass; run explicitly with --ignored"]
fn com_source_withevents_handler_fires_through_live_dispatch_sink() {
    // End-to-end live validation of the COM-source `WithEvents` path: a sink with
    // a `WithEvents x As OxVba.TestEventServer` field + an `x_OnValueChanged`
    // handler subscribes to the real coclass's dispinterface event source. Calling
    // `FireValueChanged 42` makes the server raise `OnValueChanged(42)`, which the
    // connection-point sink delivers back into the guest handler (the inbound
    // event pump runs at statement boundaries), landing 42 in a guest global.
    //
    // This exercises the binder's COM-source EventRoute emission (Fix A), the live
    // loader's dispinterface→Dispatch-path classification (Fix B), and the
    // connection-point Advise/IDispatch-sink runtime together. The headless lane
    // proves the route + classification in isolation; this proves the live wiring.
    let source = "Public got As Long\n\
         Private WithEvents x As OxVba.TestEventServer\n\
         Sub Main()\n\
         Set x = New OxVba.TestEventServer\n\
         x.FireValueChanged 42\n\
         End Sub\n\
         Private Sub x_OnValueChanged(ByVal value As Long)\n\
         got = value\n\
         End Sub\n";
    match run_clean(source) {
        Ok(snap) => {
            assert!(
                snap.iter().any(|v| v.as_i32() == Some(42)),
                "expected the OnValueChanged(42) callback to reach x_OnValueChanged in {snap:?}"
            );
        }
        Err(err) if is_component_absent(&err) => {
            eprintln!("SKIP: OxVba.TestEventServer not registered: {err}");
        }
        Err(err) => panic!("COM-source WithEvents live dispatch failed: {err}"),
    }
}

// ── Early-bound lane (typelib reference → typed receiver → EarlyCom{dispid}) ──

#[test]
#[ignore = "launches real Excel; run explicitly with --ignored"]
fn excel_early_bound_range_value_round_trips() {
    // Early-bound twin of `excel_late_bound_range_value_round_trips`: a reference to
    // the Excel typelib (LIBID {00020813-0000-0000-C000-000000000046}) makes the
    // typed receivers (`Excel.Application`, `Excel.Workbook`, `Excel.Worksheet`,
    // `Excel.Range`) resolve their members to dispids, so every member call lowers to
    // `EarlyCom{dispid}` and dispatches via real IDispatch::Invoke by dispid. Sets
    // `Range("A1").Value` then reads it back, round-tripping a Double through Excel.
    let references = vec![typelib_ref_by_libid(
        "Excel",
        "{00020813-0000-0000-C000-000000000046}",
        1,
        9,
    )];
    let source = "Public result As Double\n\
         Sub Main()\n\
         Dim app As Excel.Application\n\
         Set app = CreateObject(\"Excel.Application\")\n\
         app.Visible = False\n\
         app.DisplayAlerts = False\n\
         Dim wb As Excel.Workbook\n\
         Set wb = app.Workbooks.Add\n\
         Dim ws As Excel.Worksheet\n\
         Set ws = wb.Worksheets(1)\n\
         ws.Range(\"A1\").Value = 42.5\n\
         result = ws.Range(\"A1\").Value\n\
         wb.Close False\n\
         app.Quit\n\
         End Sub\n";
    match run_clean_with_references(source, references) {
        Ok(snap) => {
            assert!(
                snap.iter().any(|v| v.as_f64() == Some(42.5)),
                "expected the early-bound Excel Range round-trip 42.5 in {snap:?}"
            );
        }
        Err(err) if is_typelib_absent(&err) => {
            eprintln!("SKIP: Excel typelib / Excel.Application not registered: {err}");
        }
        Err(err) => panic!("Excel early-bound round-trip failed: {err}"),
    }
}

#[test]
#[ignore = "requires the registered OxVba.TestEventServer COM coclass + typelib; run explicitly with --ignored"]
fn test_event_server_early_bound_ping_returns_42() {
    // Early-bound activation of the OxVba.TestEventServer coclass: a reference to its
    // typelib (LIBID {E2A30001-0001-0001-0001-000000000001}) makes `Dim s As New
    // OxVba.TestEventServer` resolve the coclass (so `New` lowers to its ProgID
    // activation) and types `s` so `s.Ping()` lowers to `EarlyCom{dispid = 104}`,
    // dispatching by dispid through real IDispatch::Invoke. The server's `Ping`
    // returns 42 (tools/OxVba.TestEventServer/TestEventServer.cs).
    let references = vec![typelib_ref_by_libid(
        "OxVba_TestEventServer",
        "{E2A30001-0001-0001-0001-000000000001}",
        1,
        0,
    )];
    let source = "Public got As Long\n\
         Sub Main()\n\
         Dim s As New OxVba.TestEventServer\n\
         got = s.Ping()\n\
         End Sub\n";
    match run_clean_with_references(source, references) {
        Ok(snap) => {
            assert!(
                snap.iter().any(|v| v.as_i32() == Some(42)),
                "expected the early-bound OxVba.TestEventServer.Ping() = 42 in {snap:?}"
            );
        }
        Err(err) if is_typelib_absent(&err) => {
            eprintln!("SKIP: OxVba.TestEventServer typelib/coclass not registered: {err}");
        }
        Err(err) => panic!("OxVba.TestEventServer early-bound Ping failed: {err}"),
    }
}
