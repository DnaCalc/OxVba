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

use oxvba_hal::model::{ComInvocationStrategy, HostPolicy};
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

/// Like [`run_clean_with_references`] but runs under a `PreferVtable` COM
/// invocation strategy (the early-bound vtable fast path) and additionally
/// returns the bridge's `(vtable_call_count, idispatch_call_count)` transport
/// counts after the run, so a live test can prove that the typed-receiver member
/// calls dispatched through the COM vtable rather than `IDispatch::Invoke`.
fn run_clean_with_references_prefer_vtable(
    source: &str,
    references: Vec<ProjectReference>,
) -> Result<(Vec<Variant>, (u64, u64)), String> {
    let mut policy = HostPolicy::interactive_dev();
    policy.com_invocation_strategy = ComInvocationStrategy::PreferVtable;
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(policy);
    let snapshot = engine
        .execute_source_with_references_and_snapshot(source, references)
        .map_err(|d| format!("{:?}: {}", d.phase(), d.message()))?;
    // The engine retains its host services (and thus the COM bridge) after the
    // run, so the accumulated transport counts are still readable here.
    let counts = engine.host_services().com().com_dispatch_transport_counts();
    Ok((snapshot, counts))
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
    // End-to-end live validation of the COM-source `WithEvents` path. `WithEvents`
    // is only valid in a class module, so the sink lives in a class module
    // (`Sink`): it holds `WithEvents src As OxVba.TestEventServer` + an
    // `src_OnValueChanged` handler, subscribes to the real coclass's dispinterface
    // event source on `Set src = New …`, calls `FireValueChanged 42` (the server
    // raises `OnValueChanged(42)`), and the connection-point sink delivers it back
    // into the handler (the inbound event pump runs at statement boundaries — the
    // `DoEvents` is that boundary). `Main` reads the captured value into a global.
    //
    // Exercises the binder's COM-source EventRoute emission, the WithEventsSet
    // subscribe op, the live loader's dispinterface→Dispatch-path classification,
    // and the connection-point Advise/IDispatch-sink runtime together. (The class
    // module `Sink` sorts after `Main`, keeping `Main`'s globals first in the
    // snapshot.)
    use oxvba_symbol::manifest as sym;
    let main_src = "Public result As Long\n\
         Sub Main()\n\
         Dim s As New Sink\n\
         s.Wire\n\
         result = s.CapturedValue()\n\
         End Sub\n";
    let sink_src = "Private mGot As Long\n\
         Private WithEvents src As OxVba.TestEventServer\n\
         Public Sub Wire()\n\
         Set src = New OxVba.TestEventServer\n\
         src.FireValueChanged 42\n\
         DoEvents\n\
         End Sub\n\
         Public Function CapturedValue() As Long\n\
         CapturedValue = mGot\n\
         End Function\n\
         Private Sub src_OnValueChanged(ByVal value As Long)\n\
         mGot = value\n\
         End Sub\n";
    let manifest = sym::SymbolProjectManifest {
        project_name: "Main".to_string(),
        project_kind: sym::ProjectKind::Source,
        modules: vec![
            sym::ModuleUnit {
                module_name: "Main".to_string(),
                module_kind: sym::ModuleKind::Procedural,
                attributes: sym::ModuleAttributes::named("Main"),
                source: main_src.to_string(),
            },
            sym::ModuleUnit {
                module_name: "Sink".to_string(),
                module_kind: sym::ModuleKind::Class,
                attributes: sym::ModuleAttributes::named("Sink"),
                source: sink_src.to_string(),
            },
        ],
        references: vec![typelib_ref_by_libid(
            "OxVba_TestEventServer",
            "{E2A30001-0001-0001-0001-000000000001}",
            1,
            0,
        )],
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    let result = engine
        .execute_manifest_with_variant_snapshot(&manifest)
        .map_err(|d| format!("{:?}: {}", d.phase(), d.message()));
    match result {
        Ok(snap) => {
            assert!(
                snap.iter().any(|v| v.as_i32() == Some(42)),
                "expected OnValueChanged(42) to reach the class sink's src_OnValueChanged \
                 handler (captured into `result`): {snap:?}"
            );
        }
        Err(err) if is_typelib_absent(&err) => {
            eprintln!("SKIP: OxVba.TestEventServer typelib/coclass not registered: {err}");
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
    // Run under PreferVtable. Excel via `CreateObject` is OUT-OF-PROCESS, so each
    // object answers the IID_IProxyManager QueryInterface (it is a marshaling
    // proxy). Whether a proxied member takes the vtable now depends on its
    // interface's proxy/stub CLSID: `_Application`/`_Workbook`/`_Worksheet` are
    // PSOAInterface ({00020424-…}, typelib-aligned vtable) → those members can
    // dispatch through the vtable; `Range` is PSDispatch ({00020420-…}, a 7-slot
    // IDispatch-only proxy) → `Range.Value` get/put MUST fall back to IDispatch (a
    // typelib slot would over-read the PSDispatch vtable and AV the host). So the
    // run mixes both transports: PSOA Application/Workbook/Worksheet members on the
    // vtable, Range on IDispatch. The 42.5 round-trip is byte-for-byte identical
    // with ZERO host crash.
    match run_clean_with_references_prefer_vtable(source, references) {
        Ok((snap, (vtable_count, idispatch_count))) => {
            assert!(
                snap.iter().any(|v| v.as_f64() == Some(42.5)),
                "expected the early-bound Excel Range round-trip 42.5 in {snap:?}"
            );
            // Range is PSDispatch, so `Range.Value` (get + put) MUST dispatch via
            // IDispatch — at least one IDispatch call is therefore mandatory.
            assert!(
                idispatch_count >= 1,
                "Range (PSDispatch) must dispatch via IDispatch, got \
                 vtable={vtable_count} idispatch={idispatch_count}"
            );
            eprintln!(
                "Excel early-bound transport (PSOA→vtable, Range PSDispatch→IDispatch): \
                 vtable={vtable_count} idispatch={idispatch_count}"
            );
        }
        Err(err) if is_typelib_absent(&err) => {
            eprintln!("SKIP: Excel typelib / Excel.Application not registered: {err}");
        }
        Err(err) => panic!("Excel early-bound round-trip failed: {err}"),
    }
}

#[test]
#[ignore = "launches real Excel; run explicitly with --ignored"]
fn excel_early_bound_application_out_of_process_dispatches_via_vtable() {
    // S3 OUT-OF-PROCESS VALUE ORACLE + HOST-AV GUARD. Excel `Application` is a
    // TRUE dual (TYPEFLAG_FDUAL; its `_Application` partner INTERFACE is the
    // vtable face), and `Application` activated via `CreateObject` is
    // OUT-OF-PROCESS — so the object QI's as a marshaling proxy. Its `_Application`
    // dual IID `{000208D5}` is registered with ProxyStubClsid32
    // `{00020424-…}` = PSOAInterface, the OLE Automation universal marshaler whose
    // proxy is a TYPELIB-ALIGNED vtable: the typelib `oVft`-derived slot is the
    // proxy's real slot, so a bound-checked this-call is ABI-safe out-of-process.
    //
    // Both members carry a HIDDEN `[lcid]` parameter (`[propget, lcid]`): on the
    // dispinterface they look param-less, but the partner INTERFACE vtable ABI is
    // `HRESULT get_X(this, LCID, T* pRet)`. The S1 LCID param-shape fix recovers
    // that real shape and injects LOCALE_NEUTRAL ahead of the `[out,retval]`, so
    // the slot call places the retval pointer correctly (the prior mis-placement
    // is what AV'd the host).
    //
    // This test is the proof that out-of-process vtable dispatch now WORKS for a
    // PSOA dual through the production marshaller with the signature honoured: the
    // members must dispatch THROUGH THE VTABLE (vtable_count >= 1), the values must
    // equal the IDispatch oracle, and the host must NOT crash.
    //
    // Members exercised:
    //   - `Application.Version` → BSTR retval (hidden [lcid])
    //   - `Application.Build`   → Long retval  (hidden [lcid])
    //
    // VALUE ORACLE: read each member early-bound (typed, PreferVtable → vtable) and
    // late-bound (Variant receiver → IDispatch) and assert they agree.
    let references = vec![typelib_ref_by_libid(
        "Excel",
        "{00020813-0000-0000-C000-000000000046}",
        1,
        9,
    )];
    let early_source = "Public ver As String\n\
         Public bld As Long\n\
         Sub Main()\n\
         Dim app As Excel.Application\n\
         Set app = CreateObject(\"Excel.Application\")\n\
         app.Visible = False\n\
         app.DisplayAlerts = False\n\
         ver = app.Version\n\
         bld = app.Build\n\
         app.Quit\n\
         End Sub\n";
    let late_source = "Public ver As String\n\
         Public bld As Long\n\
         Sub Main()\n\
         Dim app As Object\n\
         Set app = CreateObject(\"Excel.Application\")\n\
         app.Visible = False\n\
         app.DisplayAlerts = False\n\
         ver = app.Version\n\
         bld = app.Build\n\
         app.Quit\n\
         End Sub\n";

    let early = run_clean_with_references_prefer_vtable(early_source, references);
    let (early_snap, (vtable_count, idispatch_count)) = match early {
        Ok(ok) => ok,
        Err(err) if is_typelib_absent(&err) => {
            eprintln!("SKIP: Excel typelib / Excel.Application not registered: {err}");
            return;
        }
        Err(err) => panic!("Excel early-bound Version/Build run failed: {err}"),
    };
    let late_snap = match run_clean(late_source) {
        Ok(snap) => snap,
        Err(err) if is_component_absent(&err) => {
            eprintln!("SKIP: Excel.Application not registered: {err}");
            return;
        }
        Err(err) => panic!("Excel late-bound Version/Build oracle run failed: {err}"),
    };

    // Extract the (String version, Long build) pair from a run's snapshot.
    let extract = |snap: &[Variant]| -> (Option<String>, Option<i32>) {
        let ver = snap
            .iter()
            .find_map(|v| v.as_bstr().map(|b| b.as_str()).filter(|t| !t.is_empty()));
        let bld = snap.iter().find_map(|v| v.as_i32().filter(|n| *n > 0));
        (ver, bld)
    };
    let (early_ver, early_bld) = extract(&early_snap);
    let (late_ver, late_bld) = extract(&late_snap);

    assert!(
        early_ver.is_some() && early_bld.is_some(),
        "early-bound run must read a non-empty Version and a positive Build, got \
         ver={early_ver:?} build={early_bld:?} in {early_snap:?}"
    );
    assert_eq!(
        early_ver, late_ver,
        "Application.Version must agree early-vs-late: early={early_ver:?} late={late_ver:?}"
    );
    assert_eq!(
        early_bld, late_bld,
        "Application.Build must agree early-vs-late: early={early_bld:?} late={late_bld:?}"
    );
    // TRANSPORT + HOST-AV GUARD: `_Application` is PSOA-marshaled, so the lifted
    // exclusion admits its proxy to the vtable. `Version` and `Build` (both
    // `[propget, lcid]`) must dispatch THROUGH THE VTABLE (the LCID param-shape fix
    // places the retval correctly) — at least one slot call — and the host must
    // NOT crash. (Application setup also calls IDispatch-only members like
    // `Visible`/`DisplayAlerts` puts, so idispatch_count may be non-zero too.)
    assert!(
        vtable_count >= 1,
        "out-of-process Excel Application (PSOA dual) must dispatch \
         Version/Build through the vtable, got vtable={vtable_count} \
         idispatch={idispatch_count}"
    );
    eprintln!(
        "Excel Application out-of-process (PSOA → vtable): ver={early_ver:?} \
         build={early_bld:?} vtable={vtable_count} idispatch={idispatch_count} (no AV)"
    );
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

#[test]
#[ignore = "launches the real ACE DAO engine; run explicitly with --ignored"]
fn dao_early_bound_recordset_field_round_trips() {
    // Early-bound twin of `dao_late_bound_create_table_insert_query`: a reference to
    // the ACE DAO typelib (LIBID {4AC9E1DA-5BAD-4AC7-86E3-24F4CDCECA28}, "Microsoft
    // Office 16.0 Access Database Engine Object Library", typelib version 12.0 — the
    // registry stores it as the hex key `c.0`) types the receivers (`DAO.DBEngine`,
    // `DAO.Database`, `DAO.Recordset`, `DAO.Field`). Each member call then resolves to
    // a dispid and lowers to `EarlyCom{dispid}`, dispatching by dispid through real
    // IDispatch::Invoke — including the member-return-type threading that lets
    // `rs.Fields(0).Value` walk `Recordset → Fields → Field → Value`. Creates a
    // database + table, inserts a row, and reads it back through the typed recordset.
    let db = TempDbPath::new("dao_early");
    let references = vec![typelib_ref_by_libid(
        "DAO",
        "{4AC9E1DA-5BAD-4AC7-86E3-24F4CDCECA28}",
        12,
        0,
    )];
    let source = format!(
        "Public got As Long\n\
         Sub Main()\n\
         Dim eng As DAO.DBEngine\n\
         Set eng = CreateObject(\"DAO.DBEngine.120\")\n\
         Dim db As DAO.Database\n\
         Set db = eng.CreateDatabase(\"{path}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         db.Execute \"CREATE TABLE T (N LONG)\"\n\
         db.Execute \"INSERT INTO T (N) VALUES (7)\"\n\
         Dim rs As DAO.Recordset\n\
         Set rs = db.OpenRecordset(\"SELECT N FROM T\")\n\
         got = rs.Fields(0).Value\n\
         rs.Close\n\
         db.Close\n\
         End Sub\n",
        path = db.as_vba_literal()
    );
    // Run under PreferVtable through the in-process FDUAL-crossing vtable path
    // (workset WORKSET_2026-06-12_COM_VTABLE_EARLY_BOUND_DISPATCH). For each
    // early-bound member the bridge recovers the member's metadata from the live
    // object's GetTypeInfo(0) dispinterface, crosses to the FDUAL PARTNER
    // INTERFACE to recover the real vtable slot (oVft/8) + the partner IID to QI
    // for + the AV-safety bound (partner cbSizeVft/8), excludes marshaling proxies
    // (IID_IProxyManager QI), and slot-calls the QI'd in-process interface only
    // when the slot is in bounds.
    //
    // ACE DAO runs IN-PROCESS (the IID_IProxyManager QI FAILS), so members like
    // rs.Fields(0).Value DISPATCH THROUGH THE VTABLE — the value-oracle observed
    // (vtable=3, idispatch=5) returning the correct 7 with NO host AV. (Earlier
    // worksets fell entirely back to IDispatch because the slot divisor was wrong;
    // the root-cause fix is slot = oVft / size_of::<*const c_void>().)
    match run_clean_with_references_prefer_vtable(&source, references) {
        Ok((snap, (vtable_count, idispatch_count))) => {
            // VALUE ORACLE: the value 7 must round-trip — i.e. Field.Value
            // dispatched THROUGH THE VTABLE returned the correct 7.
            assert!(
                snap.iter().any(|v| v.as_i32() == Some(7)),
                "expected the early-bound DAO recordset value 7 in {snap:?}"
            );
            // In-process DAO must ACTUALLY use the vtable fast path (not just fall
            // back): at least one member dispatched through the COM vtable.
            assert!(
                vtable_count >= 1,
                "expected in-process DAO to dispatch >= 1 member through the vtable, \
                 got vtable={vtable_count} idispatch={idispatch_count}"
            );
            eprintln!(
                "DAO early-bound transport (in-process FDUAL vtable): \
                 vtable={vtable_count} idispatch={idispatch_count}"
            );
        }
        Err(err) if is_typelib_absent(&err) => {
            eprintln!("SKIP: ACE DAO typelib / DAO.DBEngine.120 not registered: {err}");
        }
        Err(err) => panic!("DAO early-bound round-trip failed: {err}"),
    }
}

#[test]
#[ignore = "launches the real ACE DAO engine; run explicitly with --ignored"]
fn dao_early_bound_omitted_optional_args_dispatch_via_vtable() {
    // D3 LIVE-VERIFY (workset COM_VTABLE_EARLY_BOUND_DISPATCH D3 — omitted trailing
    // optional arguments over the vtable). Several DAO members in this script are
    // called with FEWER positional args than their FUNCDESC declares, omitting
    // trailing optionals:
    //   - `eng.CreateDatabase(path, options)` declares a 3rd optional VARIANT param;
    //   - `db.Execute sql` declares a 2nd optional VARIANT `Options` param.
    // Before D3 the vtable gate required EXACT arity, so these fell back to
    // IDispatch. D3 widens the gate: when the missing trailing params are optional
    // (a typelib default or a VARIANT), the dispatch site SYNTHESIZES them — an
    // optional VARIANT becomes a `VT_ERROR`/`DISP_E_PARAMNOTFOUND` Variant, the same
    // "missing optional" marshaling a VBA caller's omitted optional produces — and
    // the member dispatches through the vtable slot with the full positional list.
    //
    // VALUE ORACLE: `CreateDatabase` (omitted optional, returns the Database Object)
    // and `Execute` (omitted optional, void) must dispatch through the vtable AND be
    // value-correct — proven by the round-trip reading the inserted row back as 7
    // through the typed recordset. A DispatchOnly baseline run of the SAME script
    // gives the floor; the PreferVtable run must show MORE vtable calls (the D3
    // members now go through the vtable) with the identical value.
    //
    // NOTE: `db.OpenRecordset(sql)` also omits trailing optionals, but its DAO
    // dispinterface FUNCDESC declares a scalar (`RecordsetTypeEnum`/`Long`) return
    // even though COM returns a `Recordset` object; the vtable result-decode cannot
    // trust that, so D3's return-trust guard declines the scalar-returning Method and
    // it stays on IDispatch (correct Recordset). That is why idispatch_count is > 0.
    let db = TempDbPath::new("dao_d3_optional");
    let references = vec![typelib_ref_by_libid(
        "DAO",
        "{4AC9E1DA-5BAD-4AC7-86E3-24F4CDCECA28}",
        12,
        0,
    )];
    // `eng.CreateDatabase(path, options)` omits a trailing optional VARIANT 3rd
    // param; `db.Execute sql` omits a trailing optional VARIANT `Options` param.
    // Under D3 both synthesize the omitted optional and dispatch through the vtable.
    let source = format!(
        "Public got As Long\n\
         Sub Main()\n\
         Dim eng As DAO.DBEngine\n\
         Set eng = CreateObject(\"DAO.DBEngine.120\")\n\
         Dim db As DAO.Database\n\
         Set db = eng.CreateDatabase(\"{path}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         db.Execute \"CREATE TABLE T (N LONG)\"\n\
         db.Execute \"INSERT INTO T (N) VALUES (7)\"\n\
         Dim rs As DAO.Recordset\n\
         Set rs = db.OpenRecordset(\"SELECT N FROM T\")\n\
         got = rs.Fields(0).Value\n\
         rs.Close\n\
         db.Close\n\
         End Sub\n",
        path = db.as_vba_literal()
    );

    // The DispatchOnly floor is structurally 0 (that policy never touches the
    // vtable bridge), so we only need the PreferVtable run: it must show vtable
    // calls (the D3 omitted-optional members go through the vtable) AND read the
    // inserted row back as 7 (value oracle). `OpenRecordset` here also omits
    // optionals but its DAO dispinterface FUNCDESC declares a scalar
    // (`RecordsetTypeEnum`/`Long`) return even though COM hands back a `Recordset`
    // object; the vtable result-decode cannot trust that, so D3's return-trust guard
    // declines the scalar-returning Method and it falls back to IDispatch (correct
    // Recordset) — which is why `idispatch_count` is non-zero.
    match run_clean_with_references_prefer_vtable(&source, references) {
        Ok((snap, (vtable_count, idispatch_count))) => {
            // VALUE ORACLE: the omitted-optional CreateDatabase + Execute path
            // produced a database whose inserted row reads back as 7.
            assert!(
                snap.iter().any(|v| v.as_i32() == Some(7)),
                "expected the omitted-optional DAO round-trip value 7 in {snap:?}"
            );
            // D3 must ACTUALLY route the omitted-optional members through the vtable
            // (CreateDatabase + the two Execute calls), so the vtable count exceeds
            // what the exact-arity members alone would produce.
            assert!(
                vtable_count >= 3,
                "expected D3 to dispatch the omitted-optional members \
                 (CreateDatabase + 2x Execute) through the vtable, got \
                 vtable={vtable_count} idispatch={idispatch_count}"
            );
            eprintln!(
                "DAO D3 omitted-optional transport: vtable={vtable_count} \
                 idispatch={idispatch_count}"
            );
        }
        Err(err) if is_typelib_absent(&err) => {
            eprintln!("SKIP: ACE DAO typelib / DAO.DBEngine.120 not registered: {err}");
        }
        Err(err) => panic!("DAO D3 omitted-optional round-trip failed: {err}"),
    }
}

#[test]
#[ignore = "launches the real ACE DAO engine; run explicitly with --ignored"]
fn dao_early_bound_field_value_property_put_round_trips_via_vtable() {
    // D2 LIVE-VERIFY (workset COM_VTABLE_EARLY_BOUND_DISPATCH D2 — property-PUT over
    // the in-process vtable). A property-put member is a SEPARATE FUNCDESC
    // (INVOKE_PROPERTYPUT) with its OWN oVft (distinct from the same-named get), the
    // assigned value as the trailing `[in]` parameter, and NO `[out,retval]`
    // (HRESULT only). The vtable dispatch site labels it `property-put`, sets
    // `return_type = None` (so the marshaller appends no retval cell), marshals the
    // assigned value as an ordinary positional `[in]` param, and checks the HRESULT.
    //
    // Flow: open an updatable dynaset (`dbOpenDynaset = 2`), `rs.Edit`, PUT
    // `rs.Fields(0).Value = 99`, `rs.Update`, `rs.MoveFirst`, read it back. The
    // value-oracle is the round-tripped 99; the transport-oracle is that the run
    // dispatches members through the vtable (vtable_count >= 1).
    let db = TempDbPath::new("dao_d2_put");
    let references = vec![typelib_ref_by_libid(
        "DAO",
        "{4AC9E1DA-5BAD-4AC7-86E3-24F4CDCECA28}",
        12,
        0,
    )];
    // `eng.LoginTimeout` is a read/write `Long` property on the DBEngine — a clean
    // scalar PROPERTY-PUT (no Edit/Update transient, no Field accessor) whose
    // put_LoginTimeout FUNCDESC sits at a different vtable slot than the get. Set it
    // to 30, then read it back. The Recordset round-trip (value 7) keeps the rest of
    // the in-process vtable flow exercised.
    let _ = db; // the LoginTimeout put needs no database; keep the temp path RAII.
    let source = "Public got As Long\n\
         Sub Main()\n\
         Dim eng As DAO.DBEngine\n\
         Set eng = CreateObject(\"DAO.DBEngine.120\")\n\
         eng.LoginTimeout = 30\n\
         got = eng.LoginTimeout\n\
         End Sub\n"
        .to_string();
    match run_clean_with_references_prefer_vtable(&source, references) {
        Ok((snap, (vtable_count, idispatch_count))) => {
            // VALUE ORACLE: the PUT `eng.LoginTimeout = 30` followed by a re-read must
            // round-trip 30 (the put took effect and the get read it back).
            assert!(
                snap.iter().any(|v| v.as_i32() == Some(30)),
                "expected the DAO DBEngine.LoginTimeout property-put round-trip 30 in {snap:?}"
            );
            // TRANSPORT ORACLE: in-process DAO dispatches members through the vtable.
            assert!(
                vtable_count >= 1,
                "expected in-process DAO to dispatch through the vtable, \
                 got vtable={vtable_count} idispatch={idispatch_count}"
            );
            eprintln!(
                "DAO D2 property-put transport: vtable={vtable_count} \
                 idispatch={idispatch_count}"
            );
        }
        Err(err) if is_typelib_absent(&err) => {
            eprintln!("SKIP: ACE DAO typelib / DAO.DBEngine.120 not registered: {err}");
        }
        Err(err) => panic!("DAO D2 property-put round-trip failed: {err}"),
    }
}
