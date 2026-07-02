//! M3-8 — late-bound COM dispatch on **vm3**, differentialed against vm2's late-bound leg.
//!
//! The `com_matrix_*` category files run their three legs (late / early-PreferVtable /
//! early-DispatchOnly) on the **vm2** backend. This file adds the missing axis: vm3's
//! late-bound COM dispatch (`ComCallLate` → `host.com().dispatch_invoke_dynamic_variant_with_writebacks`)
//! must produce the SAME observable as vm2's late-bound leg — value (axis 1), the rich
//! VBA `Err.Number` recovered from the HRESULT/EXCEPINFO (axes 2 & 6, *not* flattened to 5),
//! and the COM transport (axis 5: vm3 and vm2 must drive the host through the SAME transport
//! mix — a dual interface rides the vtable even late-bound under `PreferVtable`, so the oracle
//! is count-parity between the VMs, not a fixed transport).
//!
//! Early-bound (`ComCallEarly`, typed vtable, M3-9) is exercised by the `vm3_early_*` tests
//! at the bottom: a typed receiver + typelib reference under `PreferVtable`, differentialed
//! against vm2's early-bound leg on both value and the `(vtable,idispatch)` transport counts.
//!
//! Uses `Scripting.Dictionary`, registered on every Windows install, so these need no Excel/
//! DAO/typelib. Windows-only; `#[ignore]` (live COM) with an env-skip on an absent component.
#![cfg(target_os = "windows")]
#[path = "com_matrix_common.rs"]
mod common;
use common::*;

/// A value scenario: `Verdict` (the first `Long`) encodes all sub-checks so [`find_verdict`]
/// cannot pick the wrong cell.
fn dict_value_src() -> String {
    "Public Verdict As Long\n\
     Sub Main()\n\
     Dim d As Object\n\
     Set d = CreateObject(\"Scripting.Dictionary\")\n\
     d.Add \"a\", 10\n\
     d.Add \"b\", 20\n\
     If d.Exists(\"a\") And Not d.Exists(\"zzz\") Then\n\
     Verdict = d.Count * 1000 + d.Item(\"a\") + d.Item(\"b\")\n\
     Else\n\
     Verdict = -1\n\
     End If\n\
     End Sub\n"
        .to_string()
}

/// An error scenario: `errNum` (the first `Long`) captures `Err.Number`.
fn dict_err_src(body: &str) -> String {
    format!(
        "Public errNum As Long\n\
         Public errDesc As String\n\
         Sub Main()\n\
         Dim d As Object\n\
         Set d = CreateObject(\"Scripting.Dictionary\")\n\
         {body}\
         End Sub\n"
    )
}

/// Run a source on both backends' late-bound leg; `None` (skip) if the component is absent.
fn both_late(source: &str) -> Option<(Vec<Variant>, Vec<Variant>)> {
    let vm2 = match run_clean(source) {
        Ok(s) => s,
        Err(e) if is_component_absent(&e) => {
            eprintln!("SKIP: {e}");
            return None;
        }
        Err(e) => panic!("vm2 late failed: {e}"),
    };
    let vm3 = match run_clean_vm3(source) {
        Ok(s) => s,
        Err(e) if is_component_absent(&e) => {
            eprintln!("SKIP: {e}");
            return None;
        }
        Err(e) => panic!("vm3 late failed: {e}"),
    };
    Some((vm2, vm3))
}

// ── V1: value parity (axis 1) — Count / Item / Exists ────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn vm3_dictionary_value_matches_vm2() {
    let src = dict_value_src();
    let Some((vm2, vm3)) = both_late(&src) else {
        return;
    };
    assert_eq!(
        find_verdict(&vm2),
        find_verdict(&vm3),
        "vm3 vs vm2 late-bound Dictionary value divergence"
    );
    // Absolute oracle: 2 keys → Count*1000 + 10 + 20 = 2030 (catches late==vm3-but-both-wrong).
    assert_eq!(
        find_verdict(&vm3),
        Some(2030),
        "Dictionary verdict {:?} != expected 2030",
        find_verdict(&vm3)
    );
}

// ── V2: error number parity (axes 2 & 6) — dup-key Add → 457, rich, not 5 ─────

#[test]
#[ignore = "live COM; run explicitly"]
fn vm3_dictionary_dup_key_457_matches_vm2() {
    let src = dict_err_src(
        "d.Add \"k\", 1\n\
         On Error Resume Next\n\
         d.Add \"k\", 2\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         On Error GoTo 0\n",
    );
    let Some((vm2, vm3)) = both_late(&src) else {
        return;
    };
    let (n2, _, _) = extract_err(&vm2);
    let (n3, _, _) = extract_err(&vm3);
    assert_eq!(
        n2, n3,
        "Err.Number vm2 {n2:?} != vm3 {n3:?} (late-bound COM error divergence)"
    );
    // The rich VBA number must thread end to end through vm3's `Fault::from_hal`
    // (host_error_code). A `Some(5)` would mean a COM fault path flattened the HRESULT.
    assert_eq!(
        n3,
        Some(457),
        "duplicate-key Add must surface VBA 457 on vm3 (got {n3:?}); a Some(5) means the \
         rich HRESULT->Err number was lost"
    );
}

// ── V3: error number parity — Remove("ghost") → 32811 ────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn vm3_dictionary_remove_missing_32811_matches_vm2() {
    let src = dict_err_src(
        "On Error Resume Next\n\
         d.Remove \"ghost\"\n\
         errNum = Err.Number\n\
         On Error GoTo 0\n",
    );
    let Some((vm2, vm3)) = both_late(&src) else {
        return;
    };
    let (n2, _, _) = extract_err(&vm2);
    let (n3, _, _) = extract_err(&vm3);
    assert_eq!(n2, n3, "Err.Number vm2 {n2:?} != vm3 {n3:?}");
    assert_eq!(
        n3,
        Some(32811),
        "Remove of an absent key must surface VBA 32811 on vm3 (got {n3:?})"
    );
}

// ── V4: transport oracle (axis 5) — vm3 and vm2 drive the SAME transport mix ──

#[test]
#[ignore = "live COM; run explicitly"]
fn vm3_late_dictionary_transport_matches_vm2() {
    let src = dict_value_src();
    let (vm2_snap, vm2_counts) = match run_clean_with_counts(&src) {
        Ok(x) => x,
        Err(e) if is_component_absent(&e) => {
            eprintln!("SKIP: {e}");
            return;
        }
        Err(e) => panic!("vm2 late failed: {e}"),
    };
    let (vm3_snap, vm3_counts) = match run_clean_vm3_with_counts(&src) {
        Ok(x) => x,
        Err(e) if is_component_absent(&e) => {
            eprintln!("SKIP: {e}");
            return;
        }
        Err(e) => panic!("vm3 late failed: {e}"),
    };
    assert_eq!(
        find_verdict(&vm2_snap),
        find_verdict(&vm3_snap),
        "value divergence before transport check"
    );
    // Axis 5: vm3 must drive the host through the IDENTICAL transport mix vm2 does — same
    // vtable and idispatch counts. A divergence here is a silent transport difference between
    // the two VMs (the bug this oracle exists to catch). The absolute mix is the host's call
    // (a dual interface rides the vtable even late-bound under PreferVtable).
    assert_eq!(
        vm2_counts, vm3_counts,
        "vm3 vs vm2 COM transport-count divergence (vtable,idispatch): vm2={vm2_counts:?} vm3={vm3_counts:?}"
    );
    eprintln!("V4 transport mix (vtable,idispatch): vm2={vm2_counts:?} vm3={vm3_counts:?}");
}

// ── M3-9: early-bound (ComCallEarly, typed vtable) ───────────────────────────

/// The same value scenario as [`dict_value_src`] but with a TYPED receiver, so the binder
/// lowers the member calls to early-bound `ComCallEarly` (descriptor-typed dispid dispatch).
fn dict_value_early_src() -> String {
    "Public Verdict As Long\n\
     Sub Main()\n\
     Dim d As Scripting.Dictionary\n\
     Set d = CreateObject(\"Scripting.Dictionary\")\n\
     d.Add \"a\", 10\n\
     d.Add \"b\", 20\n\
     If d.Exists(\"a\") And Not d.Exists(\"zzz\") Then\n\
     Verdict = d.Count * 1000 + d.Item(\"a\") + d.Item(\"b\")\n\
     Else\n\
     Verdict = -1\n\
     End If\n\
     End Sub\n"
        .to_string()
}

#[test]
#[ignore = "live COM; run explicitly"]
fn vm3_early_dictionary_value_and_transport_match_vm2() {
    let src = dict_value_early_src();
    let (vm2_snap, vm2_counts) =
        match run_clean_with_references_prefer_vtable(&src, scripting_ref()) {
            Ok(x) => x,
            Err(e) if is_typelib_absent(&e) => {
                eprintln!("SKIP: {e}");
                return;
            }
            Err(e) => panic!("vm2 early failed: {e}"),
        };
    let (vm3_snap, vm3_counts) =
        match run_clean_vm3_with_references_prefer_vtable(&src, scripting_ref()) {
            Ok(x) => x,
            Err(e) if is_typelib_absent(&e) => {
                eprintln!("SKIP: {e}");
                return;
            }
            Err(e) => panic!("vm3 early failed: {e}"),
        };
    // Value parity + absolute oracle (axis 1).
    assert_eq!(
        find_verdict(&vm2_snap),
        find_verdict(&vm3_snap),
        "vm3 vs vm2 early-bound Dictionary value divergence"
    );
    assert_eq!(
        find_verdict(&vm3_snap),
        Some(2030),
        "early-bound Dictionary verdict {:?} != 2030",
        find_verdict(&vm3_snap)
    );
    // Transport parity (axis 5): vm3's ComCallEarly must drive the host through the IDENTICAL
    // transport mix vm2's early-bound (EarlyCom → DispatchIdNamed) does.
    assert_eq!(
        vm2_counts, vm3_counts,
        "vm3 vs vm2 early-bound transport-count divergence: vm2={vm2_counts:?} vm3={vm3_counts:?}"
    );
    // And the vtable path actually ran (not a silent IDispatch fallback for every member).
    assert!(
        vm3_counts.0 > 0,
        "early-bound PreferVtable must slot-call the vtable at least once (got vtable={}, idispatch={})",
        vm3_counts.0,
        vm3_counts.1
    );
    eprintln!("early transport (vtable,idispatch): vm2={vm2_counts:?} vm3={vm3_counts:?}");
}
