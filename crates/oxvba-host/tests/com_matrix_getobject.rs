//! Differential COM matrix — GetObject activation.
//!
//! `GetObject` modes against live COM. The new-instance mode (`GetObject("", progid)`)
//! is the reliably-testable one — `Scripting.Dictionary` is always present — so it is the
//! committed live check. The running-instance (`GetObject(, progid)`) and file-bind
//! (`GetObject(pathname)`) modes are environment-dependent (they need a pre-running app or
//! a bindable file) and are verified manually.
//!
//! Live COM — every test is `#[ignore]`. Run explicitly:
//! ```text
//! cargo test -p oxvba-host --test com_matrix_getobject -- --ignored --test-threads=1
//! ```
#![cfg(target_os = "windows")]
#[path = "com_matrix_common.rs"]
mod common;
use common::*;

// ── G1: GetObject("", progid) — new instance (== CreateObject) ───────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn getobject_empty_path_creates_a_new_instance() {
    // `GetObject("", "Scripting.Dictionary")` returns a NEW Dictionary — the zero-length
    // pathname mode, identical to `CreateObject`. Add one key, read `Count` → 1, proving
    // the object is live and dispatches.
    let src = "Public verdict As Long\n\
         Sub Main()\n\
         Dim d As Object\n\
         Set d = GetObject(\"\", \"Scripting.Dictionary\")\n\
         d.Add \"k\", 1\n\
         verdict = d.Count\n\
         End Sub\n";
    let snap = run_clean(src).expect("GetObject new-instance run");
    assert_eq!(find_verdict(&snap), Some(1), "snapshot: {snap:?}");
}

// ── G2: GetObject(, progid) — running instance via the ROT ───────────────────

#[test]
#[ignore = "live COM; needs a running Excel instance; run explicitly"]
fn getobject_omitted_path_binds_running_excel() {
    // `GetObject(, "Excel.Application")` binds the currently-running Excel registered in the
    // Running Object Table and reads a trivial property. Requires a user-visible Excel to be
    // running (GetActiveObject only sees ROT-registered instances), so this is a manual
    // check — when none is running it errors with HRESULT 0x800401E3 (MK_E_UNAVAILABLE),
    // which currently reaches VBA as Err.Number 5 (the `hal-errors-flattened-to-5` gap),
    // not the canonical 429.
    let src = "Public verdict As String\n\
         Sub Main()\n\
         Dim app As Object\n\
         Set app = GetObject(, \"Excel.Application\")\n\
         verdict = app.Version\n\
         End Sub\n";
    let snap = run_clean(src).expect("GetObject running-instance run");
    assert!(
        snap.iter()
            .any(|v| v.as_bstr().map(|b| !b.as_str().is_empty()).unwrap_or(false)),
        "expected a non-empty Excel Version string: {snap:?}"
    );
}
