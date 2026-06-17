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

#[test]
#[ignore = "live registered Excel typelib; run explicitly"]
fn resolve_excel_irtdserver_interface_metadata() {
    let request = oxvba_com::TypeLibResolveRequest {
        reference_name: "Excel".to_string(),
        requested_coclass: None,
        importlib_hint: None,
        libid_hint: Some("00020813-0000-0000-C000-000000000046".to_string()),
        major_version_hint: Some(1),
        minor_version_hint: Some(9),
        lcid_hint: Some(0),
    };

    let metadata = oxvba_com::resolve_typelib_interface_metadata(&request, "IRtdServer")
        .expect("resolve Excel IRtdServer interface metadata");
    assert_eq!(metadata.name, "IRtdServer");
    assert_eq!(
        metadata.iid.map(|iid| format!(
            "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            iid.data1,
            iid.data2,
            iid.data3,
            iid.data4[0],
            iid.data4[1],
            iid.data4[2],
            iid.data4[3],
            iid.data4[4],
            iid.data4[5],
            iid.data4[6],
            iid.data4[7]
        )),
        Some("EC0E6191-DB51-11D3-8F3E-00C04F3651B8".to_string())
    );
    assert!(
        metadata
            .members
            .iter()
            .any(|member| member.name == "ServerStart" && member.vtable_slot == Some(7))
    );
    assert!(
        metadata
            .members
            .iter()
            .any(|member| member.name == "RefreshData" && member.vtable_slot == Some(9))
    );
    let server_start = metadata
        .members
        .iter()
        .find(|member| member.name == "ServerStart")
        .expect("ServerStart metadata");
    assert!(
        server_start
            .parameter_iids
            .first()
            .and_then(|iid| *iid)
            .is_some()
    );
    let connect_data = metadata
        .members
        .iter()
        .find(|member| member.name == "ConnectData")
        .expect("ConnectData metadata");
    assert_eq!(
        connect_data.parameter_wire_types.get(1),
        Some(&oxvba_com::TypeLibWireType::SafeArrayVariant)
    );
    let refresh_data = metadata
        .members
        .iter()
        .find(|member| member.name == "RefreshData")
        .expect("RefreshData metadata");
    assert_eq!(
        refresh_data.return_wire_type.as_ref(),
        Some(&oxvba_com::TypeLibWireType::ByRefSafeArrayVariant)
    );
}

#[test]
#[ignore = "live registered Add-In Designer typelib; run explicitly"]
fn resolve_addin_designer_idtextensibility2_interface_metadata() {
    let request = oxvba_com::TypeLibResolveRequest {
        reference_name: "AddInDesignerObjects".to_string(),
        requested_coclass: None,
        importlib_hint: None,
        libid_hint: Some("AC0714F2-3D04-11D1-AE7D-00A0C90F26F4".to_string()),
        major_version_hint: Some(1),
        minor_version_hint: Some(0),
        lcid_hint: Some(0),
    };

    let metadata = oxvba_com::resolve_typelib_interface_metadata(&request, "IDTExtensibility2")
        .or_else(|| oxvba_com::resolve_typelib_interface_metadata(&request, "_IDTExtensibility2"))
        .expect("resolve Add-In Designer IDTExtensibility2 interface metadata");
    let on_connection = metadata
        .members
        .iter()
        .find(|member| member.name == "OnConnection")
        .expect("OnConnection metadata");
    assert_eq!(on_connection.vtable_slot, Some(7));
    assert!(matches!(
        on_connection.parameter_wire_types.as_slice(),
        [
            oxvba_com::TypeLibWireType::InterfacePointer { .. },
            oxvba_com::TypeLibWireType::InterfacePointer { .. },
            oxvba_com::TypeLibWireType::InterfacePointer { .. },
            oxvba_com::TypeLibWireType::SafeArrayVariant,
        ]
    ));
    let on_disconnection = metadata
        .members
        .iter()
        .find(|member| member.name == "OnDisconnection")
        .expect("OnDisconnection metadata");
    assert_eq!(on_disconnection.vtable_slot, Some(8));
    assert!(matches!(
        on_disconnection.parameter_wire_types.as_slice(),
        [
            oxvba_com::TypeLibWireType::InterfacePointer { .. }
                | oxvba_com::TypeLibWireType::Automation(oxvba_com::TypeLibParamType::Object),
            oxvba_com::TypeLibWireType::SafeArrayVariant,
        ]
    ));
}

#[test]
#[ignore = "live registered Office typelib; run explicitly"]
fn resolve_office_iribbonextensibility_interface_metadata() {
    let request = oxvba_com::TypeLibResolveRequest {
        reference_name: "Office".to_string(),
        requested_coclass: None,
        importlib_hint: None,
        libid_hint: Some("2DF8D04C-5BFA-101B-BDE5-00AA0044DE52".to_string()),
        major_version_hint: Some(2),
        minor_version_hint: Some(8),
        lcid_hint: Some(0),
    };

    let metadata = oxvba_com::resolve_typelib_interface_metadata(&request, "IRibbonExtensibility")
        .expect("resolve Office IRibbonExtensibility interface metadata");
    assert_eq!(metadata.name, "IRibbonExtensibility");
    assert_eq!(
        metadata.iid.map(|iid| format!(
            "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            iid.data1,
            iid.data2,
            iid.data3,
            iid.data4[0],
            iid.data4[1],
            iid.data4[2],
            iid.data4[3],
            iid.data4[4],
            iid.data4[5],
            iid.data4[6],
            iid.data4[7]
        )),
        Some("000C0396-0000-0000-C000-000000000046".to_string())
    );
    let get_custom_ui = metadata
        .members
        .iter()
        .find(|member| member.name == "GetCustomUI")
        .expect("GetCustomUI metadata");
    assert_eq!(get_custom_ui.vtable_slot, Some(7));
    assert_eq!(
        get_custom_ui.parameter_wire_types.as_slice(),
        [oxvba_com::TypeLibWireType::Automation(
            oxvba_com::TypeLibParamType::String
        )]
    );
    assert_eq!(
        get_custom_ui.return_wire_type.as_ref(),
        Some(&oxvba_com::TypeLibWireType::Automation(
            oxvba_com::TypeLibParamType::String
        ))
    );
}
