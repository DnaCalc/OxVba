//! FU#4 fast diagnostic (brief Excel launch, NO workbook ops): exercises the two
//! runtime metadata-resolution paths that decide whether a `CreateObject`'d
//! Excel.Application binding gets the `event_specs` a `WithEvents` subscription
//! needs.
//!
//! Part 1 — the registry path `resolve_typelib_identity_from_prog_id` — is
//! EXPECTED to fail for Excel: its `HKCR\CLSID\{clsid}` key carries no `\TypeLib`
//! subkey, so there is no ProgID→typelib link. This is the root cause of the
//! empty `event_specs`.
//!
//! Part 2 — the dispatch-recovery path `build_metadata_blob_from_dispatch` — must
//! succeed: it walks the live object's own `GetTypeInfo`→`GetContainingTypeLib`
//! and must surface NewWorkbook (token 1565) so events can subscribe.
//!
//! Run explicitly (launches Excel for a few seconds, then releases it):
//!   cargo test -p oxvba-com --test excel_event_metadata_probe -- --ignored --nocapture
#![cfg(target_os = "windows")]

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(reserved: *mut core::ffi::c_void, coinit: u32) -> i32;
    fn CoUninitialize();
}
const COINIT_APARTMENTTHREADED: u32 = 0x2;

#[test]
#[ignore = "live COM (briefly launches Excel); run explicitly"]
fn probe_excel_application_event_metadata() {
    use oxvba_com::windows_client::activate_dispatch_by_prog_id;
    use oxvba_com::windows_typelib_loader::{
        build_metadata_blob_from_dispatch, resolve_typelib_identity_from_prog_id,
    };

    // ── Part 1: registry ProgID→typelib path (expected to FAIL for Excel) ──
    match resolve_typelib_identity_from_prog_id("Excel.Application") {
        Ok(identity) => eprintln!(
            "[PROBE] registry path OK: requested_coclass={:?} libid={:?}",
            identity.requested_coclass, identity.libid
        ),
        Err(e) => eprintln!("[PROBE] registry path FAILED (expected for Excel): {e}"),
    }

    // ── Part 2: dispatch-recovery path (must surface NewWorkbook 1565) ──
    // SAFETY: pairs with CoUninitialize below; the test owns this thread's apartment.
    let hr = unsafe { CoInitializeEx(core::ptr::null_mut(), COINIT_APARTMENTTHREADED) };
    assert!(hr >= 0, "CoInitializeEx failed: 0x{:08X}", hr as u32);

    let dispatch =
        activate_dispatch_by_prog_id("Excel.Application").expect("activate Excel.Application");

    // SAFETY: `dispatch` is the live IDispatch just activated, owned here.
    let blob = unsafe { build_metadata_blob_from_dispatch(dispatch, "Excel.Application") };

    // Release the Excel IDispatch (its only reference) before asserting, so a
    // panic does not strand the COM apartment / leak the launched instance.
    // SAFETY: `dispatch` carries the single retained reference from activation.
    unsafe {
        let vtbl = (*dispatch).vtbl;
        ((*vtbl).unknown.release)(dispatch.cast());
    }
    // SAFETY: balances the CoInitializeEx above on this thread.
    unsafe { CoUninitialize() };

    let blob = blob.expect("recover Excel.Application metadata from live dispatch");
    let has_1565 = blob.events.iter().any(|e| e.token == 1565);
    eprintln!(
        "[PROBE] dispatch path: members={} events={} has_NewWorkbook_1565={has_1565} coclass={:?}",
        blob.members.len(),
        blob.events.len(),
        blob.identity.requested_coclass
    );
    for e in blob
        .events
        .iter()
        .filter(|e| e.name.eq_ignore_ascii_case("NewWorkbook") || e.token == 1565)
    {
        eprintln!(
            "[PROBE]   event name={:?} token={} arity={} path={:?} iid={:?}",
            e.name, e.token, e.callback_arity, e.dispatch_path, e.connection_point_iid
        );
    }
    assert!(
        has_1565,
        "dispatch-recovered Excel metadata must expose NewWorkbook (token 1565); events={}",
        blob.events.len()
    );
}
