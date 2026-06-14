//! Differential COM matrix — Category P: PROPERTIES.
//!
//! Property get/put/let across the three legs with exact transport counts:
//! distinct get/put slots, indexed property put, default-member equivalence,
//! propputref, OOP Bool/String puts, `[propget,lcid]` sweeps, and the riskiest
//! `[propget,lcid]`-with-a-real-arg seam (P13/International).
//!
//! Live COM — every test is `#[ignore]`. Run explicitly:
//! ```text
//! cargo test -p oxvba-host --test com_matrix_properties -- --ignored --test-threads=1
//! ```
#![cfg(target_os = "windows")]
#[path = "com_matrix_common.rs"]
mod common;
use common::*;

// ── P1: Dictionary CompareMode get + let (read-before != after) ──────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn p1_dictionary_compare_mode_get_let() {
    // CompareMode defaults to 0 (binary). Set it to 2 (database compare; != default)
    // on a still-empty dictionary, read it back. verdict = the read-back value = 2.
    // Round-trip must change the value (read-before != after). Eligible: put + get;
    // plus the read-before get = 3 vtable calls.
    let body = "Dim verdict As Long\n\
         Dim before As Long\n\
         before = d.CompareMode\n\
         d.CompareMode = 2\n\
         verdict = d.CompareMode\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    // get(before) + put + get(verdict) = 3 vtable calls. GREEN since the FU#1
    // invoke-kind-aware spec selection: `binding.member_specs` is keyed by
    // `(memid, invoke_kind)`, so `CompareMode`'s get + put FUNCDESCs (which share a
    // memid) each carry their own enriched slot. Closing P1 also required two real
    // marshalling fixes the differential surfaced: (1) an enum retval with
    // `hreftype == 0` must resolve to `Long` (it was mis-typed `Variant`, so the
    // 4-byte enum the getter wrote decoded as 0); (2) a `[propput]` value param is
    // unnamed in the typelib, so the name list is padded to the type count.
    assert_differential("P1", late, early, Some(do_run), find_verdict, 2, Some(3));
}

// ── P2: Dictionary Item(key) get + indexed let ───────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn p2_dictionary_item_indexed_get_and_let() {
    // Add k->10, then Item("k")=20 (indexed put), then read Item("k") (=20).
    // verdict = the read-back = 20. Eligible: Add, put_Item, get_Item, get_Count = 4.
    let body = "Dim verdict As Long\n\
         d.Add \"k\", 10\n\
         d.Item(\"k\") = 20\n\
         Dim n As Long\n\
         n = d.Count\n\
         verdict = d.Item(\"k\")\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    // Add + put_Item + get_Count + get_Item = 4 vtable calls. GREEN since FU#1
    // invoke-kind-aware spec selection: `Item`'s get/put/putref each resolve to their
    // own enriched slot via the `(memid, invoke_kind)` key.
    assert_differential("P2", late, early, Some(do_run), find_verdict, 20, Some(4));
}

// ── P3: Dictionary default member d(k) == d.Item(k) ──────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn p3_dictionary_default_member_equals_item() {
    // dispid-0 default member must equal explicit Item across transports.
    // verdict = IIf(d("k") = d.Item("k") And d("k") = 77, 77, 0) = 77.
    let body = "Dim verdict As Long\n\
         d.Add \"k\", 77\n\
         verdict = IIf(d(\"k\") = d.Item(\"k\") And d(\"k\") = 77, 77, 0)\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    // Add + 3 gets (two d("k"), one d.Item("k")) = 4 vtable calls. GREEN since FU#1
    // invoke-kind-aware spec selection: the `Item` gets (default-member and explicit)
    // resolve to the enriched get slot via the `(memid, invoke_kind)` key.
    assert_differential("P3", late, early, Some(do_run), find_verdict, 77, Some(4));
}

// ── P4: Dictionary Count (ro) + Exists (Bool) ────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn p4_dictionary_count_and_exists() {
    // Add x2; verdict = Count*10 + IIf(Exists("a"),1,0)*100 + IIf(Exists("z"),1,0)
    //                 = 2*10 + 100 + 0 = 120.
    let body = "Dim verdict As Long\n\
         d.Add \"a\", 1\n\
         d.Add \"b\", 2\n\
         verdict = d.Count * 10 + IIf(d.Exists(\"a\"), 1, 0) * 100 + IIf(d.Exists(\"z\"), 1, 0)\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    // 2 Add + Count + 2 Exists = 5 eligible vtable calls.
    assert_differential("P4", late, early, Some(do_run), find_verdict, 120, Some(5));
}

// ── P5: Dictionary Key(old)=new (write-only indexed put) ─────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn p5_dictionary_key_rename_write_only_put() {
    // Put-only FUNCDESC with no paired get; BSTR index + BSTR value order.
    // d.Add "old",55 ; d.Key("old")="new" ;
    // verdict = IIf(Not d.Exists("old") And d.Exists("new") And d("new")=55, 55, 0).
    let body = "Dim verdict As Long\n\
         d.Add \"old\", 55\n\
         d.Key(\"old\") = \"new\"\n\
         verdict = IIf(Not d.Exists(\"old\") And d.Exists(\"new\") And d(\"new\") = 55, 55, 0)\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    // Add + put_Key + 2 Exists + get_Item = 5 vtable calls. GREEN since FU#1
    // invoke-kind-aware spec selection: write-only `put_Key` and default-member
    // `Item` get each resolve to their own enriched slot via the `(memid, invoke_kind)`
    // key (`Key`'s unnamed propput value param is name-padded so the arity check passes).
    assert_differential("P5", late, early, Some(do_run), find_verdict, 55, Some(5));
}

// ── P6: Dictionary Set d.Item(k)=obj (propputref, forced IDispatch) ──────────

#[test]
#[ignore = "live COM; run explicitly"]
fn p6_dictionary_set_item_propputref() {
    // propputref defers to IDispatch (windows_invoke.rs:1976), so late==early here is
    // the SHARED-IDispatch oracle, NOT a vtable differential — no vtable_expectation.
    // Set outer.Item("s")=inner ; verdict = IIf(outer.Item("s") Is inner, 1, 0) = 1.
    let body = "Dim verdict As Long\n\
         Dim inner As Object\n\
         Set inner = CreateObject(\"Scripting.Dictionary\")\n\
         Set d.Item(\"s\") = inner\n\
         verdict = IIf(d.Item(\"s\") Is inner, 1, 0)\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    // propputref => forced IDispatch; shared-IDispatch oracle, no vtable claim.
    assert_differential("P6", late, early, None, find_verdict, 1, None);
}

// ── P7: Excel Application.Visible/DisplayAlerts Bool get+put ─────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn p7_excel_visible_display_alerts_bool() {
    // Set both False, read back. verdict = IIf(vis=False And alerts=False, 1, 0) = 1.
    // OOP VARIANT_BOOL get/put. (The setup already sets these False; we re-read.)
    let body = "Dim verdict As Long\n\
         Dim vis As Boolean\n\
         Dim alerts As Boolean\n\
         vis = app.Visible\n\
         alerts = app.DisplayAlerts\n\
         verdict = IIf(vis = False And alerts = False, 1, 0)\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("P7", late, early, None, find_verdict, 1, None);
}

// ── P8: Excel Application.Caption String get+put ─────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn p8_excel_caption_string_get_put() {
    // Put a known caption then read it back (read-before != after). OOP BSTR put+get.
    // This is a STRING verdict, so the test asserts via assert_str on both lanes
    // directly rather than a Long verdict.
    let body = "Dim cap As String\n\
         app.Caption = \"OxVbaP\"\n\
         cap = app.Caption\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_string_differential("P8", late, early, "OxVbaP");
}

// ── P9: Excel Worksheet.Name String put on a chained child ───────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn p9_excel_worksheet_name_put_on_child() {
    // Put Name on a chained child (Workbooks.Add -> Worksheets(1)). IID threading
    // through the object chain. String verdict "PropSheet".
    let body = "Dim nm As String\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         Dim ws As Object\n\
         Set ws = wb.Worksheets(1)\n\
         ws.Name = \"PropSheet\"\n\
         nm = ws.Name\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_string_differential("P9", late, early, "PropSheet");
}

// ── P10: Excel Workbook.Saved Bool put-with-effect + Range fallback ──────────

#[test]
#[ignore = "live COM; run explicitly"]
fn p10_excel_workbook_saved_put_effect_with_range() {
    // Edit A1 (Saved -> False), set Saved=True, read. verdict = IIf(afterEdit=False
    // And afterSet=True, 1, 0) = 1. Range (PSDispatch) is exercised in the same run.
    let body = "Dim verdict As Long\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         Dim ws As Object\n\
         Set ws = wb.Worksheets(1)\n\
         ws.Range(\"A1\").Value = 1\n\
         Dim afterEdit As Boolean\n\
         afterEdit = wb.Saved\n\
         wb.Saved = True\n\
         Dim afterSet As Boolean\n\
         afterSet = wb.Saved\n\
         verdict = IIf(afterEdit = False And afterSet = True, 1, 0)\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("P10", late, early, None, find_verdict, 1, None);
}

// ── P11: Excel 5x [propget,lcid] sweep (Version/OS/Path/Name/Build) ──────────

#[test]
#[ignore = "live COM; run explicitly"]
fn p11_excel_propget_lcid_sweep() {
    // Read 5 [propget,lcid] members; per-field early==late; all non-empty, Build>0.
    // verdict bits: 1=Version non-empty, 2=OperatingSystem non-empty, 4=Path
    // non-empty, 8=Name non-empty, 16=Build>0. Expected = 31.
    let body = "Dim verdict As Long\n\
         verdict = 0\n\
         If Len(app.Version) > 0 Then verdict = verdict + 1\n\
         If Len(app.OperatingSystem) > 0 Then verdict = verdict + 2\n\
         If Len(app.Path) > 0 Then verdict = verdict + 4\n\
         If Len(app.Name) > 0 Then verdict = verdict + 8\n\
         If app.Build > 0 Then verdict = verdict + 16\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("P11", late, early, None, find_verdict, 31, None);
}

// ── P12: Excel Version(String) + Build(Long) type-pin ────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn p12_excel_version_build_type_pin() {
    // Result-decode TYPE agreement, not just value: Version must decode String,
    // Build must decode Long (and not Double). This asserts on the typed globals
    // directly across both lanes via a dedicated type-pin helper.
    let body = "Dim ver As String\n\
         Dim bld As Long\n\
         ver = app.Version\n\
         bld = app.Build\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_version_build_type_pin("P12", late, early);
}

// ── P13 (P0): Excel Application.International(idx) [propget,lcid] + real arg ──

#[test]
#[ignore = "live COM; run explicitly"]
fn p13_excel_international_lcid_with_real_arg() {
    // THE riskiest seam: a [propget,lcid] member with a REAL leading arg. xlCountryCode
    // = International(1) (numeric), xlDecimalSeparator = International(3) (1-char str).
    // The MIDL emission order determines whether lcid is trailing; if declined to
    // IDispatch the value must still match. verdict bits: 1 = country code numeric > 0,
    // 2 = decimal-separator string length = 1. Expected = 3.
    let body = "Dim verdict As Long\n\
         verdict = 0\n\
         Dim cc As Variant\n\
         Dim ds As Variant\n\
         cc = app.International(1)\n\
         ds = app.International(3)\n\
         If CLng(cc) > 0 Then verdict = verdict + 1\n\
         If Len(CStr(ds)) = 1 Then verdict = verdict + 2\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    // Loader-dependent transport (lcid+arg+retval ordering); assert agreement + value.
    assert_differential("P13", late, early, None, find_verdict, 3, None);
}

// ─────────────────────────────────────────────────────────────────────────────
// String-verdict and type-pin differential helpers (P8/P9/P12 carry a String or a
// type assertion rather than a single Long verdict).
// ─────────────────────────────────────────────────────────────────────────────

/// late == early on a String value, with env-skip and the no-AV guard.
fn assert_string_differential(case: &str, late: LateRun, early: EarlyRun, expected: &str) {
    let late = match late {
        Ok(s) => s,
        Err(e) if is_component_absent(&e) => {
            eprintln!("SKIP {case}: {e}");
            return;
        }
        Err(e) => panic!("{case} late failed: {e}"),
    };
    let (early_snap, _) = match early {
        Ok(x) => x,
        Err(e) if is_typelib_absent(&e) => {
            eprintln!("SKIP {case}: {e}");
            return;
        }
        Err(e) => panic!("{case} early failed: {e}"),
    };
    let pick = |snap: &[Variant]| -> Option<String> {
        snap.iter()
            .find_map(|v| v.as_bstr().map(|b| b.as_str()))
            .filter(|s| !s.is_empty())
    };
    assert_eq!(
        pick(&late),
        pick(&early_snap),
        "{case}: String value late vs early divergence (BUG)"
    );
    assert_str(&early_snap, expected, case);
}

/// P12 type-pin: Version decodes as a non-empty String, Build decodes as a Long
/// (strict as_i32) and NOT as a Double — across both lanes.
fn assert_version_build_type_pin(case: &str, late: LateRun, early: EarlyRun) {
    let late = match late {
        Ok(s) => s,
        Err(e) if is_component_absent(&e) => {
            eprintln!("SKIP {case}: {e}");
            return;
        }
        Err(e) => panic!("{case} late failed: {e}"),
    };
    let (early_snap, _) = match early {
        Ok(x) => x,
        Err(e) if is_typelib_absent(&e) => {
            eprintln!("SKIP {case}: {e}");
            return;
        }
        Err(e) => panic!("{case} early failed: {e}"),
    };
    for (label, snap) in [("late", &late), ("early", &early_snap)] {
        assert!(
            snap.iter()
                .any(|v| v.as_bstr().is_some_and(|b| !b.as_str().is_empty())),
            "{case} ({label}): Version must decode as a non-empty String in {snap:?}"
        );
        assert!(
            snap.iter().any(|v| v.as_i32().is_some_and(|n| n > 0)),
            "{case} ({label}): Build must decode as a Long > 0 in {snap:?}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Source builders.
// ─────────────────────────────────────────────────────────────────────────────

fn dict_late(body: &str) -> String {
    format!(
        "Sub Main()\n\
         Dim d As Object\n\
         Set d = CreateObject(\"Scripting.Dictionary\")\n\
         {body}\
         End Sub\n"
    )
}

fn dict_early(body: &str) -> String {
    format!(
        "Sub Main()\n\
         Dim d As Scripting.Dictionary\n\
         Set d = CreateObject(\"Scripting.Dictionary\")\n\
         {body}\
         End Sub\n"
    )
}

fn excel_late(body: &str) -> String {
    format!(
        "Sub Main()\n\
         Dim app As Object\n\
         Set app = CreateObject(\"Excel.Application\")\n\
         app.Visible = False\n\
         app.DisplayAlerts = False\n\
         {body}\
         app.Quit\n\
         End Sub\n"
    )
}

fn excel_early(body: &str) -> String {
    format!(
        "Sub Main()\n\
         Dim app As Excel.Application\n\
         Set app = CreateObject(\"Excel.Application\")\n\
         app.Visible = False\n\
         app.DisplayAlerts = False\n\
         {body}\
         app.Quit\n\
         End Sub\n"
    )
}
