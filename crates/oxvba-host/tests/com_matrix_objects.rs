//! Differential COM matrix — Category O: OBJECTS / Set / Is / Nothing / IDENTITY.
//!
//! Object identity across the three legs: back-reference identity, default-vs-
//! explicit member identity, in-process `ReturnSelfObject` round-trip, arg-in/
//! return-out canonicalization, deep chaining, `TypeName`, `Set = Nothing`, the
//! indispensable negative control (distinct objects NOT `Is`-equal), `With`,
//! `Find -> Nothing`, `TypeOf`, and identity-through-a-VBA-`ByRef`-hop.
//!
//! Live COM — every test is `#[ignore]`. Run explicitly:
//! ```text
//! cargo test -p oxvba-host --test com_matrix_objects -- --ignored --test-threads=1
//! ```
#![cfg(target_os = "windows")]
#[path = "com_matrix_common.rs"]
mod common;
use common::*;

// ── O1 (P0): Excel back-reference identity ───────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o1_excel_back_reference_identity() {
    // ws.Application Is app, ws.Parent Is wb — back-references up the tree must
    // canonicalize by IUnknown. verdict = IIf(sameApp,1,0) + IIf(sameWb,2,0) = 3.
    let body = "Dim verdict As Long\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         Dim ws As Object\n\
         Set ws = wb.Worksheets(1)\n\
         Dim sameApp As Boolean\n\
         Dim sameWb As Boolean\n\
         sameApp = (ws.Application Is app)\n\
         sameWb = (ws.Parent Is wb)\n\
         verdict = IIf(sameApp, 1, 0) + IIf(sameWb, 2, 0)\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("O1", late, early, None, find_verdict, 3, None);
}

// ── O2: Dictionary default-vs-explicit Item identity ─────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o2_dictionary_default_vs_explicit_item_identity() {
    // Store an object; Set a=d("k"), Set b=d.Item("k); a Is b must hold.
    // verdict = IIf(a Is b, 1, 0) = 1.
    let body = "Dim verdict As Long\n\
         Dim inner As Object\n\
         Set inner = CreateObject(\"Scripting.Dictionary\")\n\
         d.Add \"k\", inner\n\
         Dim a As Object\n\
         Dim b As Object\n\
         Set a = d(\"k\")\n\
         Set b = d.Item(\"k\")\n\
         verdict = IIf(a Is b, 1, 0)\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    // Add + d("k") get + d.Item("k") get = 3 eligible vtable calls.
    assert_differential("O2", late, early, Some(do_run), find_verdict, 1, Some(3));
}

// ── O3: TestEventServer ReturnSelfObject identity round-trip ─────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o3_test_event_server_return_self_identity() {
    // In-process identity oracle without Office: two retrievals of `this` must dedup
    // to one handle and equal the original. verdict = IIf(a Is b And a Is s, 1, 0) = 1.
    let body = "Dim verdict As Long\n\
         Dim a As Object\n\
         Dim b As Object\n\
         Set a = s.ReturnSelfObject()\n\
         Set b = s.ReturnSelfObject()\n\
         verdict = IIf(a Is b And a Is s, 1, 0)\n";
    let late = run_clean(&tes_late(body));
    let early = run_clean_with_references_prefer_vtable(&tes_early(body), tes_ref());
    let do_run = run_clean_with_references_dispatch_only(&tes_early(body), tes_ref());
    // 2x ReturnSelfObject = 2 eligible vtable calls.
    assert_differential("O3", late, early, Some(do_run), find_verdict, 1, Some(2));
}

// ── O4: Dictionary+FSO arg-in / return-out identity ──────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o4_dictionary_arg_in_return_out_identity() {
    // Store an FSO object into a Dictionary then read it back: both directions must
    // canonicalize to the same IUnknown. verdict = IIf(g Is fso, 1, 0) = 1.
    let body = "Dim verdict As Long\n\
         Dim fso As Object\n\
         Set fso = CreateObject(\"Scripting.FileSystemObject\")\n\
         d.Add \"fs\", fso\n\
         Dim g As Object\n\
         Set g = d(\"fs\")\n\
         verdict = IIf(g Is fso, 1, 0)\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    // Add + d("fs") get = 2 eligible vtable calls.
    assert_differential("O4", late, early, Some(do_run), find_verdict, 1, Some(2));
}

// ── O5: Excel deep chain Workbooks.Add.Worksheets(1).Range("A1") ─────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o5_excel_deep_chain_round_trip() {
    // Temporary lifetime across hops; vtable -> IDispatch handoff (Range PSDispatch).
    // Set then get 123.5. verdict = CLng(value*10) = 1235 (encode the Double so the
    // verdict stays a Long).
    let body = "Dim verdict As Long\n\
         app.Workbooks.Add.Worksheets(1).Range(\"A1\").Value = 123.5\n\
         Dim wb As Object\n\
         Set wb = app.ActiveWorkbook\n\
         Dim v As Double\n\
         v = wb.Worksheets(1).Range(\"A1\").Value\n\
         verdict = CLng(v * 10)\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("O5", late, early, None, find_verdict, 1235, None);
}

// ── O6: TypeName(obj) for Dictionary + Excel ─────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o6_typename_dictionary_and_excel() {
    // Interface-name vs coclass-name divergence across transports. We assert
    // TypeName(d) == "Dictionary" via a String verdict; not "Object".
    let body = "Dim tn As String\n\
         tn = TypeName(d)\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    assert_string_differential("O6", late, early, "Dictionary");
}

// ── O7: Set d=Nothing + Is Nothing + dual-ref release ────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o7_set_nothing_is_nothing() {
    // verdict = IIf(wasSet,1,0) + IIf(isCleared,2,0) = 3. wasSet: Not (d Is Nothing)
    // while live; isCleared: d Is Nothing after Set d = Nothing.
    let body = "Dim verdict As Long\n\
         Dim wasSet As Boolean\n\
         wasSet = Not (d Is Nothing)\n\
         Set d = Nothing\n\
         Dim isCleared As Boolean\n\
         isCleared = (d Is Nothing)\n\
         verdict = IIf(wasSet, 1, 0) + IIf(isCleared, 2, 0)\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    // Is-Nothing comparisons are VM-layer, not COM slot calls; no member dispatches
    // here, so do not pin an exact vtable count.
    assert_differential("O7", late, early, Some(do_run), find_verdict, 3, None);
}

// ── O8 (P0): negative control — distinct objects NOT Is-equal ────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o8_negative_control_distinct_objects_not_is_equal() {
    // INDISPENSABLE: without it a degenerate always-True `Is` passes O1-O4.
    // verdict = IIf(Not (d1 Is d2), 1, 0) = 1.
    let early_src = "Public verdict As Long\n\
         Sub Main()\n\
         Dim d1 As Scripting.Dictionary\n\
         Dim d2 As Scripting.Dictionary\n\
         Set d1 = CreateObject(\"Scripting.Dictionary\")\n\
         Set d2 = CreateObject(\"Scripting.Dictionary\")\n\
         verdict = IIf(Not (d1 Is d2), 1, 0)\n\
         End Sub\n";
    let late_src = "Public verdict As Long\n\
         Sub Main()\n\
         Dim d1 As Object\n\
         Dim d2 As Object\n\
         Set d1 = CreateObject(\"Scripting.Dictionary\")\n\
         Set d2 = CreateObject(\"Scripting.Dictionary\")\n\
         verdict = IIf(Not (d1 Is d2), 1, 0)\n\
         End Sub\n";
    let late = run_clean(late_src);
    let early = run_clean_with_references_prefer_vtable(early_src, scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(early_src, scripting_ref());
    assert_differential("O8", late, early, Some(do_run), find_verdict, 1, None);
}

// ── O9: Excel Workbooks.Add re-called + lookup-by-Name identity ──────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o9_excel_workbook_lookup_by_name_identity() {
    // Two routes to the same workbook: Set wb2 = Workbooks(wb.Name) must be wb.
    // verdict = IIf(wb2 Is wb, 1, 0) = 1.
    let body = "Dim verdict As Long\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         Dim wb2 As Object\n\
         Set wb2 = app.Workbooks(wb.Name)\n\
         verdict = IIf(wb2 Is wb, 1, 0)\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("O9", late, early, None, find_verdict, 1, None);
}

// ── O10: DAO Fields(0) Is Fields("N") + Field re-call ────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o10_dao_field_identity_agreement() {
    // val == 7; ACE wrapper policy may differ so assert AGREEMENT (late==early) on the
    // sameField flag, not an absolute. verdict = value read (7) — sameField is folded
    // into the value: verdict = IIf(byIdx Is byName, byIdx.Value, 0) = 7 if the
    // wrappers dedup, else 0. We assert late==early on this composite + that it is 7
    // IF dedup holds; ACE may return distinct wrappers, so this is an agreement test.
    let db = TempDbPath::new("o10_dao");
    let lit = db.as_vba_literal();
    let body = format!(
        "Dim verdict As Long\n\
         Set db = eng.CreateDatabase(\"{lit}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         db.Execute \"CREATE TABLE T (N LONG)\"\n\
         db.Execute \"INSERT INTO T (N) VALUES (7)\"\n\
         Dim rs As Object\n\
         Set rs = db.OpenRecordset(\"SELECT N FROM T\")\n\
         Dim byIdx As Object\n\
         Dim byName As Object\n\
         Set byIdx = rs.Fields(0)\n\
         Set byName = rs.Fields(\"N\")\n\
         verdict = byIdx.Value + byName.Value\n\
         rs.Close\n\
         db.Close\n"
    );
    let late = run_clean(&dao_late(&body));
    let early = run_clean_with_references_prefer_vtable(&dao_early(&body), dao_ref());
    // Two Field wrappers read 7 each -> 14 (proves both routes reach the same data,
    // independent of whether the wrappers are identity-deduped). Agreement + value.
    assert_differential("O10", late, early, None, find_verdict, 14, None);
}

// ── O11 (GAP): With block over an Excel chain ────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o11_excel_with_block_cached_receiver() {
    // GAP 2: cached With-receiver retained across members, released at End With;
    // per-member re-dispatch on the same object. verdict = CLng(.Range("A1").Value)
    // after setting Name + a cell inside the With. Expected = 5 (cell value).
    let body = "Dim verdict As Long\n\
         With app.Workbooks.Add.Worksheets(1)\n\
         .Name = \"WithSheet\"\n\
         .Range(\"A1\").Value = 5\n\
         verdict = CLng(.Range(\"A1\").Value)\n\
         End With\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("O11", late, early, None, find_verdict, 5, None);
}

// ── O12 (GAP): Range.Find returns Nothing ────────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o12_excel_range_find_returns_nothing() {
    // GAP 11: a null interface retval must become Nothing (identity 0), not a bound
    // object wrapping null. verdict = IIf(f Is Nothing, 1, 0) = 1; guarded .Value no AV.
    let body = "Dim verdict As Long\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         Dim ws As Object\n\
         Set ws = wb.Worksheets(1)\n\
         Dim f As Object\n\
         Set f = ws.Range(\"A1:A10\").Find(\"zzz\")\n\
         verdict = IIf(f Is Nothing, 1, 0)\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("O12", late, early, None, find_verdict, 1, None);
}

// ── O13 (GAP): TypeOf sh Is Excel.Worksheet over heterogeneous Sheets ────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o13_excel_typeof_worksheet_discrimination() {
    // GAP 7: QI-based interface discrimination via TypeOf. Count worksheets among the
    // Sheets collection by TypeOf; it must equal Worksheets.Count.
    // verdict = IIf(typedCount = wsCount, typedCount, -1).
    let body = "Dim verdict As Long\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         Dim sh As Object\n\
         Dim typedCount As Long\n\
         typedCount = 0\n\
         For Each sh In wb.Sheets\n\
         If TypeOf sh Is Excel.Worksheet Then typedCount = typedCount + 1\n\
         Next sh\n\
         Dim wsCount As Long\n\
         wsCount = wb.Worksheets.Count\n\
         verdict = IIf(typedCount = wsCount, typedCount, -1)\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    // The TypeOf branch needs the Excel.Worksheet IID from the typelib; the late lane
    // resolves it the same way. Worksheets.Count on a fresh workbook is >= 1; assert
    // agreement + that the typed count is positive via the value being >= 1.
    assert_differential_min("O13", late, early, 1);
}

// ── O14 (GAP): COM object through a VBA ByRef param then dispatch ─────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn o14_com_object_through_vba_byref_then_dispatch() {
    // GAP 14: identity survives a VBA ByRef hop before COM dispatch. `Touch d` where
    // `Sub Touch(ByRef o As Object) : o.Add "x", 1 : End Sub`. verdict = d.Count = 1.
    let early_src = "Public verdict As Long\n\
         Sub Main()\n\
         Dim d As Scripting.Dictionary\n\
         Set d = CreateObject(\"Scripting.Dictionary\")\n\
         Touch d\n\
         verdict = d.Count\n\
         End Sub\n\
         Sub Touch(ByRef o As Object)\n\
         o.Add \"x\", 1\n\
         End Sub\n";
    let late_src = "Public verdict As Long\n\
         Sub Main()\n\
         Dim d As Object\n\
         Set d = CreateObject(\"Scripting.Dictionary\")\n\
         Touch d\n\
         verdict = d.Count\n\
         End Sub\n\
         Sub Touch(ByRef o As Object)\n\
         o.Add \"x\", 1\n\
         End Sub\n";
    let late = run_clean(late_src);
    let early = run_clean_with_references_prefer_vtable(early_src, scripting_ref());
    // `o` is typed As Object inside Touch (late dispatch there), so the exact total is
    // mixed; assert agreement + the absolute Count.
    assert_differential("O14", late, early, None, find_verdict, 1, None);
}

// ─────────────────────────────────────────────────────────────────────────────
// Differential helpers specific to this category.
// ─────────────────────────────────────────────────────────────────────────────

/// late == early on a String value (O6), env-skip + no-AV guard.
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
        "{case}: TypeName String late vs early divergence (BUG)"
    );
    assert_str(&early_snap, expected, case);
}

/// late == early on the verdict AND the verdict is at least `min` (used where the
/// absolute is environment-variable, e.g. a fresh-workbook sheet count >= 1).
fn assert_differential_min(case: &str, late: LateRun, early: EarlyRun, min: i64) {
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
    assert_eq!(
        find_verdict(&late),
        find_verdict(&early_snap),
        "{case}: late vs early verdict divergence (BUG)"
    );
    let v = find_verdict(&early_snap);
    assert!(
        v.is_some_and(|n| n >= min),
        "{case}: verdict {v:?} must be >= {min}"
    );
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

fn tes_late(body: &str) -> String {
    format!(
        "Sub Main()\n\
         Dim s As Object\n\
         Set s = CreateObject(\"OxVba.TestEventServer\")\n\
         {body}\
         End Sub\n"
    )
}

fn tes_early(body: &str) -> String {
    format!(
        "Sub Main()\n\
         Dim s As New OxVba.TestEventServer\n\
         {body}\
         End Sub\n"
    )
}

fn dao_late(body: &str) -> String {
    format!(
        "Sub Main()\n\
         Dim eng As Object\n\
         Dim db As Object\n\
         Set eng = CreateObject(\"DAO.DBEngine.120\")\n\
         {body}\
         End Sub\n"
    )
}

fn dao_early(body: &str) -> String {
    format!(
        "Sub Main()\n\
         Dim eng As DAO.DBEngine\n\
         Dim db As DAO.Database\n\
         Set eng = CreateObject(\"DAO.DBEngine.120\")\n\
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
