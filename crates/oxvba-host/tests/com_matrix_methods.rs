//! Differential COM matrix — Category M: METHODS.
//!
//! Method calls across the three differential legs (late `As Object`, early
//! `PreferVtable`, early `DispatchOnly`) with exact transport-count assertions.
//! Covers no-arg void slots, multi-arg marshalling order, named-vs-positional
//! dispatch, omitted/interior-optional synthesis, object-as-`[in]`-interface args,
//! and the gold-standard exact-`vt=1` proof (M7).
//!
//! Live COM — every test is `#[ignore]`. Run explicitly:
//! ```text
//! cargo test -p oxvba-host --test com_matrix_methods -- --ignored --test-threads=1
//! ```
#![cfg(target_os = "windows")]
#[path = "com_matrix_common.rs"]
mod common;
use common::*;

// ── M1: Dictionary RemoveAll (void) + Count ──────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn m1_dictionary_remove_all_void_then_count() {
    // Add x2 (Count 2), RemoveAll (void), Count (0). Verdict = c1*10 + c2 = 20.
    // Eligible vtable members in the early run: 2x Add, Count, RemoveAll, Count = 5.
    let body = "Dim verdict As Long\n\
         d.Add \"a\", 1\n\
         d.Add \"b\", 2\n\
         Dim c1 As Long\n\
         c1 = d.Count\n\
         d.RemoveAll\n\
         Dim c2 As Long\n\
         c2 = d.Count\n\
         verdict = c1 * 10 + c2\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    assert_differential(
        "M1",
        late,
        early,
        Some(do_run),
        find_verdict,
        20,
        Some(5), // 2x Add + 2x Count + RemoveAll
    );
}

// ── M2: Dictionary Add (2 args) + Exists (Bool) ──────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn m2_dictionary_add_two_args_then_exists() {
    // Add "k",1 ; b1 = Exists("k") (True), b2 = Exists("z") (False).
    // verdict = IIf(b1,1,0) + IIf(b2,0,2) = 1. Eligible: Add + 2x Exists = 3.
    let body = "Dim verdict As Long\n\
         d.Add \"k\", 1\n\
         Dim b1 As Boolean\n\
         Dim b2 As Boolean\n\
         b1 = d.Exists(\"k\")\n\
         b2 = d.Exists(\"z\")\n\
         verdict = IIf(b1, 1, 0) + IIf(b2, 0, 2)\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    assert_differential("M2", late, early, Some(do_run), find_verdict, 1, Some(3));
}

// ── M3 (P0): Dictionary Add named-args vs positional ─────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn m3_dictionary_add_named_vs_positional() {
    // d1.Add "k",100 (positional => VTABLE) ; d2.Add Item:=100, Key:="k" (named =>
    // forced IDispatch, windows_invoke.rs:1955). verdict = IIf(d1("k")=100 And
    // d2("k")=100, 100, 0) = 100. The named call must NOT secretly vtable-dispatch
    // with a GetIDsOfNames reorder bug: exact vt counts only the positional-eligible
    // members. Positional-eligible: d1.Add, d1("k") get, d2("k") get = 3 (the named
    // d2.Add goes IDispatch). The default-member d(k) gets ARE vtable-eligible.
    let early_src = "Public verdict As Long\n\
         Sub Main()\n\
         Dim d1 As Scripting.Dictionary\n\
         Dim d2 As Scripting.Dictionary\n\
         Set d1 = CreateObject(\"Scripting.Dictionary\")\n\
         Set d2 = CreateObject(\"Scripting.Dictionary\")\n\
         d1.Add \"k\", 100\n\
         d2.Add Item:=100, Key:=\"k\"\n\
         verdict = IIf(d1(\"k\") = 100 And d2(\"k\") = 100, 100, 0)\n\
         End Sub\n";
    let late_src = "Public verdict As Long\n\
         Sub Main()\n\
         Dim d1 As Object\n\
         Dim d2 As Object\n\
         Set d1 = CreateObject(\"Scripting.Dictionary\")\n\
         Set d2 = CreateObject(\"Scripting.Dictionary\")\n\
         d1.Add \"k\", 100\n\
         d2.Add Item:=100, Key:=\"k\"\n\
         verdict = IIf(d1(\"k\") = 100 And d2(\"k\") = 100, 100, 0)\n\
         End Sub\n";
    let late = run_clean(late_src);
    let early = run_clean_with_references_prefer_vtable(early_src, scripting_ref());
    // The named d2.Add is forced-IDispatch; d1.Add + d1("k") + d2("k") = 3 vtable.
    assert_differential("M3", late, early, None, find_verdict, 100, Some(3));
}

// ── M4: FSO BuildPath (2 str) + FileExists (Bool) ────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn m4_fso_build_path_and_file_exists() {
    // Build a path from two strings; FileExists on a real temp file (True) and a
    // ghost (False). verdict = IIf(exists1,1,0) + IIf(exists2,0,2) = 1.
    let db = TempDbPath::new("m4_fso");
    // Write a real file (per leg) so FileExists is True in every differential leg.
    db.write_all_legs(b"x");
    let lit = db.as_vba_literal();
    let body = format!(
        "Dim verdict As Long\n\
         Dim p As String\n\
         p = fso.BuildPath(\"C:\\\\tmp\", \"a.txt\")\n\
         Dim e1 As Boolean\n\
         Dim e2 As Boolean\n\
         e1 = fso.FileExists(\"{lit}\")\n\
         e2 = fso.FileExists(\"{lit}.ghost\")\n\
         verdict = IIf(e1, 1, 0) + IIf(e2, 0, 2)\n"
    );
    let late = run_clean(&fso_late(&body));
    let early = run_clean_with_references_prefer_vtable(&fso_early(&body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&fso_early(&body), scripting_ref());
    // BuildPath + 2x FileExists = 3 eligible vtable calls.
    assert_differential("M4", late, early, Some(do_run), find_verdict, 1, Some(3));
}

// ── M5: Excel Workbooks.Add omitted-optional vs supplied ─────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn m5_excel_workbooks_add_omitted_vs_supplied() {
    // wb1 = Workbooks.Add (omitted optional) ; wb2 = Workbooks.Add(-4167) (xlWorksheet
    // template arg supplied). Both must yield a workbook with >= 1 worksheet.
    // verdict = IIf(c1 >= 1 And c2 >= 1, 1, 0) = 1. Out-of-process; no exact-vt claim.
    let body = "Dim verdict As Long\n\
         Dim wb1 As Object\n\
         Dim wb2 As Object\n\
         Set wb1 = app.Workbooks.Add\n\
         Set wb2 = app.Workbooks.Add(-4167)\n\
         Dim c1 As Long\n\
         Dim c2 As Long\n\
         c1 = wb1.Worksheets.Count\n\
         c2 = wb2.Worksheets.Count\n\
         verdict = IIf(c1 >= 1 And c2 >= 1, 1, 0)\n";
    let late = run_clean(&excel_late(body, true));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body, true), excel_ref());
    assert_differential("M5", late, early, None, find_verdict, 1, None);
}

// ── M6: Excel Worksheets.Add interior omission (forced IDispatch) ─────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn m6_excel_worksheets_add_interior_gap_forced_idispatch() {
    // Worksheets.Add(, , 2) — an INTERIOR omitted positional. An interior gap forces
    // IDispatch on BOTH lanes (windows_invoke.rs:1964), so late==early here is the
    // SHARED-IDispatch oracle, NOT a vtable differential — no vtable_expectation.
    // verdict = (sheets_after - sheets_before) = 2.
    let body = "Dim verdict As Long\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         Dim before As Long\n\
         before = wb.Worksheets.Count\n\
         wb.Worksheets.Add , , 2\n\
         verdict = wb.Worksheets.Count - before\n";
    let late = run_clean(&excel_late(body, true));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body, true), excel_ref());
    // Forced IDispatch on the interior-gap Add: late==early is the shared-IDispatch
    // oracle, NOT a vtable differential.
    assert_differential("M6", late, early, None, find_verdict, 2, None);
}

// ── M7 (P0): TestEventServer SumPair(40,2) → 42, exact vt=1 ──────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn m7_test_event_server_sum_pair_exact_vtable_one() {
    // THE gold-standard exact-count proof: a single 2-int->Long call over a fully
    // controlled in-process dual, no Add noise. If vtable_count != 1 the fast path is
    // silently falling back. Validates the entire transport-counting harness.
    let early_src = "Public verdict As Long\n\
         Sub Main()\n\
         Dim s As New OxVba.TestEventServer\n\
         verdict = s.SumPair(40, 2)\n\
         End Sub\n";
    let late_src = "Public verdict As Long\n\
         Sub Main()\n\
         Dim s As Object\n\
         Set s = CreateObject(\"OxVba.TestEventServer\")\n\
         verdict = s.SumPair(40, 2)\n\
         End Sub\n";
    let late = run_clean(late_src);
    let early = run_clean_with_references_prefer_vtable(early_src, tes_ref());
    let do_run = run_clean_with_references_dispatch_only(early_src, tes_ref());
    // Exactly ONE eligible member call (SumPair); `New` is activation, not a slot call.
    assert_differential("M7", late, early, Some(do_run), find_verdict, 42, Some(1));
}

// ── M8: DAO Execute (omitted opt) + OpenRecordset (scalar-ret-but-object) ────

#[test]
#[ignore = "live COM; run explicitly"]
fn m8_dao_execute_and_open_recordset_return_trust() {
    // The existing D3 script, made differential. CreateDatabase + 2x Execute go
    // VTABLE (vt=3); OpenRecordset is scalar-declared-but-object so the return-trust
    // guard declines it to IDispatch. A regression would decode the object as a Long.
    // verdict = got = 7.
    let db = TempDbPath::new("m8_dao");
    let lit = db.as_vba_literal();
    let body = format!(
        "Dim verdict As Long\n\
         Set db = eng.CreateDatabase(\"{lit}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         db.Execute \"CREATE TABLE T (N LONG)\"\n\
         db.Execute \"INSERT INTO T (N) VALUES (7)\"\n\
         Dim rs As Object\n\
         Set rs = db.OpenRecordset(\"SELECT N FROM T\")\n\
         verdict = rs.Fields(0).Value\n\
         rs.Close\n\
         db.Close\n"
    );
    let late = run_clean(&dao_late(&body, false));
    let early = run_clean_with_references_prefer_vtable(&dao_early(&body, true), dao_ref());
    // CreateDatabase + 2x Execute via vtable; OpenRecordset + Fields/Value mix —
    // the bridge declines the scalar-returning OpenRecordset so the exact total is
    // loader-sensitive; assert agreement + value, not an exact vtable count.
    assert_differential("M8", late, early, None, find_verdict, 7, None);
}

// ── M9: DAO LoginTimeout put round-trip (differential) ───────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn m9_dao_login_timeout_put_round_trip() {
    // read-before (default, != 45), set 45, read-after. verdict = IIf(before <> 45
    // And after = 45, 45, 0) = 45. Value != default (30) and != other tests.
    let body = "Dim verdict As Long\n\
         Dim before As Long\n\
         before = eng.LoginTimeout\n\
         eng.LoginTimeout = 45\n\
         Dim after As Long\n\
         after = eng.LoginTimeout\n\
         verdict = IIf(before <> 45 And after = 45, 45, 0)\n";
    let late = run_clean(&dao_late(body, false));
    let early = run_clean_with_references_prefer_vtable(&dao_early(body, true), dao_ref());
    let do_run = run_clean_with_references_dispatch_only(&dao_early(body, true), dao_ref());
    // get + put + get = 3 eligible vtable calls on the in-process DBEngine.
    assert_differential("M9", late, early, Some(do_run), find_verdict, 45, Some(3));
}

// ── M10 (P0 GAP): DAO Fields.Append fld (object-as-[in]-interface arg) ────────

#[test]
#[ignore = "live COM; run explicitly"]
fn m10_dao_fields_append_object_in_arg() {
    // GAP 1: marshal_inbound_param P::Object passes IDispatch with NO QI to the
    // declared param IID (windows_vtable.rs:437); ACE expects Field* -> AV if the QI
    // is missing. Late (VT_DISPATCH) works; an early divergence indicts the missing
    // QI. verdict = db.TableDefs("T2").Fields.Count = 1.
    let db = TempDbPath::new("m10_dao");
    let lit = db.as_vba_literal();
    let body = format!(
        "Dim verdict As Long\n\
         Set db = eng.CreateDatabase(\"{lit}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         Dim td As Object\n\
         Set td = db.CreateTableDef(\"T2\")\n\
         Dim fld As Object\n\
         Set fld = td.CreateField(\"N\", 4)\n\
         td.Fields.Append fld\n\
         db.TableDefs.Append td\n\
         verdict = db.TableDefs(\"T2\").Fields.Count\n\
         db.Close\n"
    );
    let late = run_clean(&dao_late(&body, false));
    let early = run_clean_with_references_prefer_vtable(&dao_early(&body, true), dao_ref());
    // The P::Object Append arm is the focus; the exact vtable total over this whole
    // create-chain is loader-sensitive, so assert agreement + the absolute value.
    assert_differential("M10", late, early, None, find_verdict, 1, None);
}

// ── M11: TestEventServer IsSelf(obj) object-as-[in] arg + identity ───────────

#[test]
#[ignore = "live COM; run explicitly"]
fn m11_test_event_server_is_self_object_in_arg() {
    // GAP 1/14 in-process oracle without Office: Set self = s.ReturnSelfObject() then
    // s.IsSelf(self) is ReferenceEquals(this, value) -> proves the SAME pointer
    // round-tripped through the [in] object arg AND identity survived.
    // verdict = IIf(s.IsSelf(self), 1, 0) = 1.
    let early_src = "Public verdict As Long\n\
         Sub Main()\n\
         Dim s As New OxVba.TestEventServer\n\
         Dim selfRef As Object\n\
         Set selfRef = s.ReturnSelfObject()\n\
         verdict = IIf(s.IsSelf(selfRef), 1, 0)\n\
         End Sub\n";
    let late_src = "Public verdict As Long\n\
         Sub Main()\n\
         Dim s As Object\n\
         Set s = CreateObject(\"OxVba.TestEventServer\")\n\
         Dim selfRef As Object\n\
         Set selfRef = s.ReturnSelfObject()\n\
         verdict = IIf(s.IsSelf(selfRef), 1, 0)\n\
         End Sub\n";
    let late = run_clean(late_src);
    let early = run_clean_with_references_prefer_vtable(early_src, tes_ref());
    let do_run = run_clean_with_references_dispatch_only(early_src, tes_ref());
    // ReturnSelfObject + IsSelf = 2 eligible vtable calls.
    assert_differential("M11", late, early, Some(do_run), find_verdict, 1, Some(2));
}

// ── M12 (doc-gap): ByRef-out method ──────────────────────────────────────────

#[test]
#[ignore = "DOCUMENTED GAP: no ByRef-out member on the registered surface"]
fn m12_byref_out_method_documented_gap() {
    // TestEventServer.cs confirms NO ByRef-out member exists. To express this
    // scenario the server needs `Sub Increment(ByRef n As Long)` (write-back through
    // a [in,out] interface param). Until then this is a discoverable, non-silent gap.
    eprintln!(
        "M12 GAP: no ByRef-out COM member registered. Add \
         `Sub Increment(ByRef n As Long)` to OxVba.TestEventServer to express this."
    );
}

// ── M13 (doc-gap): ParamArray method ─────────────────────────────────────────

#[test]
#[ignore = "DOCUMENTED GAP: no ParamArray/vararg member on the registered surface"]
fn m13_param_array_method_documented_gap() {
    // No vararg member exists. To express this scenario the server needs
    // `Function Sum(ParamArray nums()) As Long`. Discoverable, non-silent gap.
    eprintln!(
        "M13 GAP: no ParamArray COM member registered. Add \
         `Function Sum(ParamArray nums()) As Long` to OxVba.TestEventServer."
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Source builders — wrap a scenario `body` in a Main that activates the receiver
// late (`As Object` + CreateObject) or early (typed). The verdict global is always
// declared first inside `body` so `find_verdict` is unambiguous.
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

fn fso_late(body: &str) -> String {
    format!(
        "Sub Main()\n\
         Dim fso As Object\n\
         Set fso = CreateObject(\"Scripting.FileSystemObject\")\n\
         {body}\
         End Sub\n"
    )
}

fn fso_early(body: &str) -> String {
    format!(
        "Sub Main()\n\
         Dim fso As Scripting.FileSystemObject\n\
         Set fso = CreateObject(\"Scripting.FileSystemObject\")\n\
         {body}\
         End Sub\n"
    )
}

fn dao_late(body: &str, _typed: bool) -> String {
    format!(
        "Sub Main()\n\
         Dim eng As Object\n\
         Dim db As Object\n\
         Set eng = CreateObject(\"DAO.DBEngine.120\")\n\
         {body}\
         End Sub\n"
    )
}

fn dao_early(body: &str, _typed: bool) -> String {
    format!(
        "Sub Main()\n\
         Dim eng As DAO.DBEngine\n\
         Dim db As DAO.Database\n\
         Set eng = CreateObject(\"DAO.DBEngine.120\")\n\
         {body}\
         End Sub\n"
    )
}

fn excel_late(body: &str, _setup: bool) -> String {
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

fn excel_early(body: &str, _setup: bool) -> String {
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
