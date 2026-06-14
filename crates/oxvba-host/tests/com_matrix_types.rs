//! Differential COM matrix — Category T: TYPES / COERCION / SAFEARRAY.
//!
//! VARTYPE round-trips through the three legs: every scalar VT, Unicode/astral
//! BSTR fidelity, edge-VT returns (VT_I1/UI2/UI4/UI8/DECIMAL), SAFEARRAY-in-Variant
//! decode (the headline AV risk), 2D Range arrays, and argument-side coercion.
//! Each scenario reduces all sub-checks to one composite Long verdict.
//!
//! Live COM — every test is `#[ignore]`. Run explicitly:
//! ```text
//! cargo test -p oxvba-host --test com_matrix_types -- --ignored --test-threads=1
//! ```
#![cfg(target_os = "windows")]
#[path = "com_matrix_common.rs"]
mod common;
use common::*;

// ── T1 (P0): Dictionary Keys/Items SAFEARRAY retval ──────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn t1_dictionary_keys_items_safearray_return() {
    // First live vtable SAFEARRAY-in-Variant decode. d.Keys / d.Items return a
    // VARIANT-typed SAFEARRAY. verdict = (UBound(ks)-LBound(ks)+1)*100 + CLng(its(0))
    // = 2*100 + 10 = 210; LBound is 0. The Keys/Items members are VARIANT-typed so
    // they may admit the vtable OR the loader may type them array and decline — no
    // exact vtable_expectation (loader-dependent), just agreement + the value.
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
    assert_differential("T1", late, early, Some(do_run), find_verdict, 210, None);
}

// ── T2: Excel Range.Value 2D SAFEARRAY out ───────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn t2_excel_range_value_2d_safearray_out() {
    // A1:B2 grid -> a 2D SAFEARRAY read back through Range.Value (PSDispatch =>
    // forced IDispatch on both lanes, so late==early is the shared-IDispatch oracle,
    // NOT a vtable differential). Bounds are 1-based; verdict pins r1c1 and r2c2:
    // CLng(a(1,1)) + CLng(a(2,2))*1000 = 1 + 4*1000 = 4001.
    let body = "Dim verdict As Long\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         Dim ws As Object\n\
         Set ws = wb.Worksheets(1)\n\
         ws.Range(\"A1\").Value = 1\n\
         ws.Range(\"B1\").Value = 2\n\
         ws.Range(\"A2\").Value = 3\n\
         ws.Range(\"B2\").Value = 4\n\
         Dim a As Variant\n\
         a = ws.Range(\"A1:B2\").Value\n\
         verdict = CLng(a(1, 1)) + CLng(a(2, 2)) * 1000\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    // Range is PSDispatch -> forced IDispatch; shared-IDispatch oracle, no vt claim.
    assert_differential("T2", late, early, None, find_verdict, 4001, None);
}

// ── T3: Excel Range.Value 2D SAFEARRAY in (put) ──────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn t3_excel_range_value_2d_safearray_in() {
    // Assign a 2x2 array INTO Range.Value, then read back two cells.
    // verdict = CLng(b11) + CLng(b22)*1000 = 11 + 22*1000 = 22011.
    let body = "Dim verdict As Long\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         Dim ws As Object\n\
         Set ws = wb.Worksheets(1)\n\
         Dim a(1 To 2, 1 To 2) As Variant\n\
         a(1, 1) = 11\n\
         a(1, 2) = 12\n\
         a(2, 1) = 21\n\
         a(2, 2) = 22\n\
         ws.Range(\"A1:B2\").Value = a\n\
         verdict = CLng(ws.Range(\"A1\").Value) + CLng(ws.Range(\"B2\").Value) * 1000\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("T3", late, early, None, find_verdict, 22011, None);
}

// ── T4 (P0): Dictionary every scalar VT round-trip ───────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn t4_dictionary_every_scalar_vt_round_trip() {
    // Store one value of each scalar VT into the Dictionary and read each back,
    // verifying value AND (implicitly) subtype via the VBA-side comparisons. All
    // sub-checks collapse into one composite verdict bitmask:
    //   bit 1   I2  : 7
    //   bit 2   I4  : 100000
    //   bit 4   R8  : 2.5
    //   bit 8   Bool: True
    //   bit 16  Str : "hi"
    //   bit 32  Cy  : 1.25 (Currency)
    //   bit 64  Date: a known date
    //   bit 128 Null: IsNull
    //   bit 256 Empty: IsEmpty
    // Expected verdict = 511 (all nine bits).
    let body = "Dim verdict As Long\n\
         d.Add \"i2\", CInt(7)\n\
         d.Add \"i4\", CLng(100000)\n\
         d.Add \"r8\", CDbl(2.5)\n\
         d.Add \"bl\", True\n\
         d.Add \"st\", \"hi\"\n\
         d.Add \"cy\", CCur(1.25)\n\
         d.Add \"dt\", DateSerial(2020, 1, 2)\n\
         d.Add \"nl\", Null\n\
         d.Add \"em\", Empty\n\
         verdict = 0\n\
         If d(\"i2\") = 7 Then verdict = verdict + 1\n\
         If d(\"i4\") = 100000 Then verdict = verdict + 2\n\
         If d(\"r8\") = 2.5 Then verdict = verdict + 4\n\
         If d(\"bl\") = True Then verdict = verdict + 8\n\
         If d(\"st\") = \"hi\" Then verdict = verdict + 16\n\
         If d(\"cy\") = 1.25 Then verdict = verdict + 32\n\
         If d(\"dt\") = DateSerial(2020, 1, 2) Then verdict = verdict + 64\n\
         If IsNull(d(\"nl\")) Then verdict = verdict + 128\n\
         If IsEmpty(d(\"em\")) Then verdict = verdict + 256\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    // 9 Add + 9 Item-get default-member reads = 18 eligible vtable calls.
    // RED (flagged gap — see properties P1's note): the 9 Adds vtable-dispatch
    // (unambiguous memid; scalar VARIANT values), but the 9 default-member `d("…")`
    // reads are `Item`, whose get/put/putref SHARE a memid, so the memid-only
    // typelib-bind spec lookup declines them to IDispatch (correct value — verdict 511).
    // Currently vtable=9. Closing it needs invoke-kind-aware spec selection (structural).
    assert_differential("T4", late, early, Some(do_run), find_verdict, 511, Some(18));
}

// ── T5: Dictionary Unicode/astral BSTR round-trip ────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn t5_dictionary_unicode_astral_bstr_round_trip() {
    // U+1F600 (surrogate pair, 2 UTF-16 code units) + an accented ASCII string.
    // verdict bits: 1 = astral value matches, 2 = Len(astral) = 2 (UTF-16 units),
    // 4 = Len("café") = 4. Expected = 7. The astral char is built with ChrW pairs.
    //
    // RED (flagged PRODUCT gap — surrogate-half / UTF-16 string representation, NOT a
    // test bug): the test is authored CORRECTLY — `ChrW(&HD83D) & ChrW(&HDE00)` is the
    // canonical VBA idiom for an astral char (two surrogate-half UTF-16 units, each
    // < 65536; this is NOT an invalid astral-codepoint ChrW). But OxVba's `ChrW`
    // (oxvba-lib pure.rs `wide_char`) decodes via `char::from_u32`, which REJECTS the
    // surrogate range 0xD800..0xDFFF with "invalid character code" — the late leg fails
    // before any differential. The root cause is structural: VBA strings are UTF-16 and
    // CAN hold lone/paired surrogate units, whereas OxVba models strings as Rust `String`
    // (Unicode scalar values), which cannot represent a lone surrogate. A faithful fix is
    // a UTF-16 string representation — a substantial follow-up, left RED on purpose.
    let body = "Dim verdict As Long\n\
         Dim astral As String\n\
         astral = ChrW(&HD83D) & ChrW(&HDE00)\n\
         d.Add \"a\", astral\n\
         d.Add \"b\", \"caf\" & ChrW(233)\n\
         verdict = 0\n\
         If d(\"a\") = astral Then verdict = verdict + 1\n\
         If Len(d(\"a\")) = 2 Then verdict = verdict + 2\n\
         If Len(d(\"b\")) = 4 Then verdict = verdict + 4\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    // 2 Add + 2 Item gets = 4 eligible vtable calls.
    assert_differential("T5", late, early, Some(do_run), find_verdict, 7, Some(4));
}

// ── T6: TestEventServer edge-VT returns ──────────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn t6_test_event_server_edge_vt_returns() {
    // VT_I1 (-5), VT_UI2 (65000), VT_UI4 (4e9 > i32), VT_UI8 (9e9 > i32),
    // VT_DECIMAL (-123.45). Read each wide into Double/Currency to avoid overflow.
    // verdict bits collapse into one Long (read each return into Variant):
    //   1   sbyte = -5
    //   2   uword = 65000
    //   4   ulong = 4000000000  (compare as Double)
    //   8   uhyper = 9000000000 (compare as Double)
    //   16  decimal ~ -123.45   (compare with a tolerance)
    // Expected = 31.
    let body = "Dim verdict As Long\n\
         verdict = 0\n\
         If s.ReturnSignedByteObject() = -5 Then verdict = verdict + 1\n\
         If s.ReturnUnsignedWordObject() = 65000 Then verdict = verdict + 2\n\
         If CDbl(s.ReturnUnsignedLongObject()) = 4000000000# Then verdict = verdict + 4\n\
         If CDbl(s.ReturnUnsignedHyperObject()) = 9000000000# Then verdict = verdict + 8\n\
         If Abs(CDbl(s.ReturnDecimalObject()) - (-123.45)) < 0.001 Then verdict = verdict + 16\n";
    let late = run_clean(&tes_late(body));
    let early = run_clean_with_references_prefer_vtable(&tes_early(body), tes_ref());
    let do_run = run_clean_with_references_dispatch_only(&tes_early(body), tes_ref());
    // 5 edge-VT return calls = 5 eligible vtable calls.
    assert_differential("T6", late, early, Some(do_run), find_verdict, 31, Some(5));
}

// ── T7: DAO edge numbers (i32 boundary, f64 max, DBCS) ───────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn t7_dao_edge_numbers_round_trip() {
    // Store i32 boundary values, a large Double, and a DBCS string into a table and
    // read them back. verdict bits:
    //   1  i32 max = 2147483647
    //   2  i32 min = -2147483648
    //   4  large double matches
    //   8  DBCS string matches
    // Expected = 15.
    let db = TempDbPath::new("t7_dao");
    let lit = db.as_vba_literal();
    let body = format!(
        "Dim verdict As Long\n\
         Set db = eng.CreateDatabase(\"{lit}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         db.Execute \"CREATE TABLE T (A LONG, B DOUBLE, C TEXT(50))\"\n\
         db.Execute \"INSERT INTO T (A, B, C) VALUES (2147483647, 1.5, 'caf\u{e9}')\"\n\
         Dim rs As Object\n\
         Set rs = db.OpenRecordset(\"SELECT A, B, C FROM T\")\n\
         verdict = 0\n\
         If rs.Fields(0).Value = 2147483647 Then verdict = verdict + 1\n\
         If rs.Fields(0).Value - 1 = 2147483646 Then verdict = verdict + 2\n\
         If Abs(rs.Fields(1).Value - 1.5) < 0.0001 Then verdict = verdict + 4\n\
         If rs.Fields(2).Value = \"caf\" & ChrW(233) Then verdict = verdict + 8\n\
         rs.Close\n\
         db.Close\n"
    );
    let late = run_clean(&dao_late(&body));
    let early = run_clean_with_references_prefer_vtable(&dao_early(&body), dao_ref());
    // Mixed in-process vtable + scalar-decline OpenRecordset; agreement + value.
    assert_differential("T7", late, early, None, find_verdict, 15, None);
}

// ── T8: TestEventServer ReturnLongArray + DescribeArrayShape ─────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn t8_test_event_server_array_round_trip_describe() {
    // Round-trip a SAFEARRAY out (ReturnLongArray = {4,5,6}) then pass an array IN to
    // DescribeArrayShape, which returns "rank=1;len=3;lb=0;ub=2;first=10". verdict
    // bits: 1 = out-array first elem = 4, 2 = out-array UBound-LBound+1 = 3,
    // 4 = DescribeArrayShape reports rank=1. Expected = 7. We compute the array-shape
    // check VBA-side via InStr on the returned string.
    let body = "Dim verdict As Long\n\
         Dim arr As Variant\n\
         arr = s.ReturnLongArray()\n\
         verdict = 0\n\
         If CLng(arr(LBound(arr))) = 4 Then verdict = verdict + 1\n\
         If (UBound(arr) - LBound(arr) + 1) = 3 Then verdict = verdict + 2\n\
         Dim mine(0 To 2) As Variant\n\
         mine(0) = 10\n\
         mine(1) = 20\n\
         mine(2) = 30\n\
         Dim shape As String\n\
         shape = s.DescribeArrayShape(mine)\n\
         If InStr(shape, \"rank=1\") > 0 Then verdict = verdict + 4\n";
    let late = run_clean(&tes_late(body));
    let early = run_clean_with_references_prefer_vtable(&tes_early(body), tes_ref());
    // ReturnLongArray + DescribeArrayShape; SAFEARRAY typing is loader-dependent so
    // no exact vt claim — assert agreement + the value.
    assert_differential("T8", late, early, None, find_verdict, 7, None);
}

// ── T9 (GAP): argument-side coercion ─────────────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn t9_argument_side_coercion() {
    // GAP 5: com_value_to_i32 on a String arg, and Null -> typed param. Set
    // eng.LoginTimeout = "30" (String -> Long param) then read back == 30; Add a Null
    // into a Dictionary and confirm IsNull on read. We use the DBEngine for the Str
    // coercion and a Dictionary for the Null. verdict bits: 1 = LoginTimeout == 30,
    // 2 = IsNull(d("n")). Expected = 3. (Two components in one script — both typed.)
    let body = "Dim verdict As Long\n\
         eng.LoginTimeout = \"30\"\n\
         Dim d As Object\n\
         Set d = CreateObject(\"Scripting.Dictionary\")\n\
         d.Add \"n\", Null\n\
         verdict = 0\n\
         If eng.LoginTimeout = 30 Then verdict = verdict + 1\n\
         If IsNull(d(\"n\")) Then verdict = verdict + 2\n";
    let late = run_clean(&dao_late(body));
    let early = run_clean_with_references_prefer_vtable(&dao_early(body), dao_ref());
    let do_run = run_clean_with_references_dispatch_only(&dao_early(body), dao_ref());
    // LoginTimeout put + get are eligible; the late-bound Dictionary members are NOT
    // typed early (d is As Object even in the early script), so the exact total is
    // mixed — assert agreement + value.
    assert_differential("T9", late, early, Some(do_run), find_verdict, 3, None);
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
