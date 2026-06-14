//! Differential COM matrix — Category C: INDEXED PROPERTIES / COLLECTIONS / FOR EACH.
//!
//! Indexed access and enumeration across the three legs: numeric-vs-string keys,
//! Keys/Items SAFEARRAY, `For Each` (IEnumVARIANT) in-proc and OOP, 0-based vs
//! 1-based indexing, object-valued enumeration, mid-iteration abandon, and the
//! canonical DAO Edit/Update/MoveNext/EOF loop.
//!
//! Live COM — every test is `#[ignore]`. Run explicitly:
//! ```text
//! cargo test -p oxvba-host --test com_matrix_collections -- --ignored --test-threads=1
//! ```
#![cfg(target_os = "windows")]
#[path = "com_matrix_common.rs"]
mod common;
use common::*;

// ── C1: Dictionary num-key vs str-key (no coercion) ──────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn c1_dictionary_numeric_vs_string_key() {
    // Variant index VT must not coerce 7 <-> "7". d.Add "7",100 ; d.Add 7,200 ;
    // verdict = d("7") + d(7) = 300 (distinct keys).
    let body = "Dim verdict As Long\n\
         d.Add \"7\", 100\n\
         d.Add 7, 200\n\
         verdict = d(\"7\") + d(7)\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    // 2 Add + 2 default-member gets = 4 eligible vtable calls.
    // RED (flagged gap — see properties P1's note): the 2 Adds vtable-dispatch
    // (unambiguous memid), but the 2 default-member gets are `Item`, whose
    // get/put/putref SHARE a memid, so the memid-only typelib-bind spec lookup declines
    // them to IDispatch (correct value — distinct numeric/string keys still sum to 300).
    // Currently vtable=2. Closing it needs invoke-kind-aware spec selection (structural).
    assert_differential("C1", late, early, Some(do_run), find_verdict, 300, Some(4));
}

// ── C2: Dictionary Keys()/Items() SAFEARRAY (VARIANT-typed) ──────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn c2_dictionary_keys_items_safearray() {
    // ks = d.Keys ; its = d.Items ; verdict = (UBound(ks)-LBound(ks)+1)*100 +
    // CLng(its(0)) = 2*100 + 10 = 210. LBound is 0. Loader-dependent transport
    // (Variant-typed admits, array-typed declines) — assert agreement + value.
    let body = "Dim verdict As Long\n\
         d.Add \"a\", 10\n\
         d.Add \"b\", 20\n\
         Dim ks As Variant\n\
         Dim its As Variant\n\
         ks = d.Keys\n\
         its = d.Items\n\
         verdict = (UBound(ks) - LBound(ks) + 1) * 100 + CLng(its(0))\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    assert_differential("C2", late, early, Some(do_run), find_verdict, 210, None);
}

// ── C3 (P0): Dictionary For Each (IEnumVARIANT) order/count ──────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn c3_dictionary_for_each_order_count() {
    // For Each over keys; total += d(k); count. For Each is IDispatch on BOTH lanes
    // (enumeration has no vtable slot), so late==early is the shared-IDispatch oracle.
    // verdict = total*10 + count = 7*10 + 3 = 73 (values 1+2+4 over 3 keys).
    let body = "Dim verdict As Long\n\
         d.Add \"a\", 1\n\
         d.Add \"b\", 2\n\
         d.Add \"c\", 4\n\
         Dim total As Long\n\
         Dim count As Long\n\
         total = 0\n\
         count = 0\n\
         Dim k As Variant\n\
         For Each k In d.Keys\n\
         total = total + d(k)\n\
         count = count + 1\n\
         Next k\n\
         verdict = total * 10 + count\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    // For Each => IDispatch enumeration on both lanes; shared oracle, no vt claim.
    assert_differential("C3", late, early, None, find_verdict, 73, None);
}

// ── C4: Dictionary For Each empty ────────────────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn c4_dictionary_for_each_empty() {
    // S_FALSE on first Next; zero-iteration boundary. count 0 and reaches `after`.
    // verdict = count*10 + afterReached = 0*10 + 1 = 1.
    let body = "Dim verdict As Long\n\
         Dim count As Long\n\
         count = 0\n\
         Dim k As Variant\n\
         For Each k In d.Keys\n\
         count = count + 1\n\
         Next k\n\
         verdict = count * 10 + 1\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    assert_differential("C4", late, early, None, find_verdict, 1, None);
}

// ── C5: Excel Worksheets(num) vs ("name") + Count + Range mix ────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn c5_excel_worksheets_index_by_num_and_name() {
    // Rename sheet 1 to "Probe", set A1=35 there; byNum reads name "Probe", byStr
    // reads A1 value via the named sheet. verdict bits: 1 = byNum name "Probe",
    // 2 = byStr A1 = 35, 4 = Worksheets.Count >= 1. Expected = 7. Range is PSDispatch.
    let body = "Dim verdict As Long\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         wb.Worksheets(1).Name = \"Probe\"\n\
         wb.Worksheets(\"Probe\").Range(\"A1\").Value = 35\n\
         verdict = 0\n\
         If wb.Worksheets(1).Name = \"Probe\" Then verdict = verdict + 1\n\
         If CLng(wb.Worksheets(\"Probe\").Range(\"A1\").Value) = 35 Then verdict = verdict + 2\n\
         If wb.Worksheets.Count >= 1 Then verdict = verdict + 4\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("C5", late, early, None, find_verdict, 7, None);
}

// ── C6 (P0): Excel For Each ws In app.Worksheets (OOP IEnumVARIANT) ──────────

#[test]
#[ignore = "live COM; run explicitly"]
fn c6_excel_for_each_worksheets_oop_enum() {
    // Hardest enum case: a marshalled enumerator proxy, each element re-QI'd to
    // _Worksheet and .Name read over vtable. Add sheets to reach >= 3, loop, count.
    // verdict bits: 1 = count >= 3, 2 = every element had a non-empty Name (allNamed).
    // Expected = 3.
    let body = "Dim verdict As Long\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         wb.Worksheets.Add\n\
         wb.Worksheets.Add\n\
         wb.Worksheets.Add\n\
         Dim count As Long\n\
         Dim allNamed As Boolean\n\
         count = 0\n\
         allNamed = True\n\
         Dim ws As Object\n\
         For Each ws In wb.Worksheets\n\
         count = count + 1\n\
         If Len(ws.Name) = 0 Then allNamed = False\n\
         Next ws\n\
         verdict = 0\n\
         If count >= 3 Then verdict = verdict + 1\n\
         If allNamed Then verdict = verdict + 2\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("C6", late, early, None, find_verdict, 3, None);
}

// ── C7: Excel Sheets(i) vs Worksheets(i) parity + Workbooks(i) ───────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn c7_excel_sheets_vs_worksheets_parity() {
    // Sheets.Item heterogeneous return vs Worksheets.Item typed return. Rename sheet
    // 1 to "First". verdict bits: 1 = Sheets(1).Name "First", 2 = Worksheets(1).Name
    // "First", 4 = Workbooks(1).Name non-empty. Expected = 7.
    let body = "Dim verdict As Long\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         wb.Worksheets(1).Name = \"First\"\n\
         verdict = 0\n\
         If wb.Sheets(1).Name = \"First\" Then verdict = verdict + 1\n\
         If wb.Worksheets(1).Name = \"First\" Then verdict = verdict + 2\n\
         If Len(app.Workbooks(1).Name) > 0 Then verdict = verdict + 4\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("C7", late, early, None, find_verdict, 7, None);
}

// ── C8: DAO Fields(0) vs Fields("N") + Count (0-based) ───────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn c8_dao_fields_index_by_num_and_name_zero_based() {
    // DAO is 0-based (vs Excel 1-based): the engine passes the index verbatim.
    // verdict bits: 1 = byIdx = 7, 2 = byName = 7, 4 = Fields.Count = 2. Expected = 7.
    let db = TempDbPath::new("c8_dao");
    let lit = db.as_vba_literal();
    let body = format!(
        "Dim verdict As Long\n\
         Set db = eng.CreateDatabase(\"{lit}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         db.Execute \"CREATE TABLE T (N LONG, M LONG)\"\n\
         db.Execute \"INSERT INTO T (N, M) VALUES (7, 8)\"\n\
         Dim rs As Object\n\
         Set rs = db.OpenRecordset(\"SELECT N, M FROM T\")\n\
         verdict = 0\n\
         If rs.Fields(0).Value = 7 Then verdict = verdict + 1\n\
         If rs.Fields(\"N\").Value = 7 Then verdict = verdict + 2\n\
         If rs.Fields.Count = 2 Then verdict = verdict + 4\n\
         rs.Close\n\
         db.Close\n"
    );
    let late = run_clean(&dao_late(&body));
    let early = run_clean_with_references_prefer_vtable(&dao_early(&body), dao_ref());
    let do_run = run_clean_with_references_dispatch_only(&dao_early(&body), dao_ref());
    assert_differential("C8", late, early, Some(do_run), find_verdict, 7, None);
}

// ── C9: DAO For Each fld In rs.Fields ────────────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn c9_dao_for_each_fields() {
    // In-process object-valued enumeration; QI each element to DAO.Field.
    // verdict bits: 1 = count = 2, 2 = first field name "N". Expected = 3.
    let db = TempDbPath::new("c9_dao");
    let lit = db.as_vba_literal();
    let body = format!(
        "Dim verdict As Long\n\
         Set db = eng.CreateDatabase(\"{lit}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         db.Execute \"CREATE TABLE T (N LONG, M LONG)\"\n\
         db.Execute \"INSERT INTO T (N, M) VALUES (7, 8)\"\n\
         Dim rs As Object\n\
         Set rs = db.OpenRecordset(\"SELECT N, M FROM T\")\n\
         Dim count As Long\n\
         Dim firstName As String\n\
         count = 0\n\
         Dim fld As Object\n\
         For Each fld In rs.Fields\n\
         If count = 0 Then firstName = fld.Name\n\
         count = count + 1\n\
         Next fld\n\
         verdict = 0\n\
         If count = 2 Then verdict = verdict + 1\n\
         If firstName = \"N\" Then verdict = verdict + 2\n\
         rs.Close\n\
         db.Close\n"
    );
    let late = run_clean(&dao_late(&body));
    let early = run_clean_with_references_prefer_vtable(&dao_early(&body), dao_ref());
    assert_differential("C9", late, early, None, find_verdict, 3, None);
}

// ── C10 (GAP): Excel For Each early Exit For (enumerator abandon) ────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn c10_excel_for_each_exit_for_abandon() {
    // GAP 3: a live IEnumVARIANT released mid-iteration (the abandon path). Add a
    // couple of sheets, loop, Exit For on the second element. verdict = partialCount = 2.
    let body = "Dim verdict As Long\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         wb.Worksheets.Add\n\
         wb.Worksheets.Add\n\
         Dim count As Long\n\
         count = 0\n\
         Dim ws As Object\n\
         For Each ws In wb.Worksheets\n\
         count = count + 1\n\
         If count = 2 Then Exit For\n\
         Next ws\n\
         verdict = count\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("C10", late, early, None, find_verdict, 2, None);
}

// ── C11 (P0 GAP): DAO Edit/Update/MoveNext/EOF loop ──────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn c11_dao_edit_update_movenext_eof_loop() {
    // GAP 12: highest-traffic real DAO path. VARIANT_BOOL EOF in a tight loop (wrong
    // -1/0 decode -> infinite loop or premature exit) plus void state-effect methods.
    // Insert 3 rows (1,2,3), loop incrementing each by 10, then sum. verdict bits:
    // 1 = iterations = 3, 2 = sum = (11+12+13) = 36. Encode: verdict = iterations*100
    // + sum = 3*100 + 36 = 336.
    let db = TempDbPath::new("c11_dao");
    let lit = db.as_vba_literal();
    let body = format!(
        "Dim verdict As Long\n\
         Set db = eng.CreateDatabase(\"{lit}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         db.Execute \"CREATE TABLE T (N LONG)\"\n\
         db.Execute \"INSERT INTO T (N) VALUES (1)\"\n\
         db.Execute \"INSERT INTO T (N) VALUES (2)\"\n\
         db.Execute \"INSERT INTO T (N) VALUES (3)\"\n\
         Dim rs As Object\n\
         Set rs = db.OpenRecordset(\"SELECT N FROM T\", 2)\n\
         Dim iterations As Long\n\
         iterations = 0\n\
         Do Until rs.EOF\n\
         rs.Edit\n\
         rs.Fields(\"N\").Value = rs.Fields(\"N\").Value + 10\n\
         rs.Update\n\
         iterations = iterations + 1\n\
         rs.MoveNext\n\
         Loop\n\
         Dim total As Long\n\
         total = 0\n\
         Dim rs2 As Object\n\
         Set rs2 = db.OpenRecordset(\"SELECT N FROM T\")\n\
         Do Until rs2.EOF\n\
         total = total + rs2.Fields(\"N\").Value\n\
         rs2.MoveNext\n\
         Loop\n\
         verdict = iterations * 100 + total\n\
         rs.Close\n\
         rs2.Close\n\
         db.Close\n"
    );
    let late = run_clean(&dao_late(&body));
    let early = run_clean_with_references_prefer_vtable(&dao_early(&body), dao_ref());
    assert_differential("C11", late, early, None, find_verdict, 336, None);
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
