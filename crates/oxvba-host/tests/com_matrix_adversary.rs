//! Differential COM matrix — Category A: ADVERSARY BUG-HUNT.
//!
//! The highest-bug-yield seams, each with the transport-delta assertion that
//! distinguishes "dangerous path attempted-and-survived" from "correctly declined":
//! SAFEARRAY-return admission, interior-`[propget,lcid]` arg ordering, PSOA-verdict
//! leaking onto a PSDispatch child, exact-arity return-trust holes, `[propput,lcid]`
//! ordering, per-call QI/pin leaks (in-proc + OOP orphan), and the binder-path gaps
//! (`Missing` optional, `CallByName`, enum-constant import, implicit default member).
//!
//! A12 (the interior-lcid gate UNIT guard + SAFEARRAY-decline) is NOT a live test
//! and is NOT expressible here — it constructs an `oxvba-com`-internal
//! `ComMemberSpec` and calls the crate-private `vtable_gate_admits` /
//! `is_v1_vtable_vartype`, neither of which is reachable from an `oxvba-host`
//! integration test. It therefore lives in `oxvba-com`'s `gate_tests` module (runs
//! in plain `cargo test`, can't rot). `a12_*` below documents that pointer.
//!
//! Live COM — every live test is `#[ignore]`. Run explicitly:
//! ```text
//! cargo test -p oxvba-host --test com_matrix_adversary -- --ignored --test-threads=1
//! ```
#![cfg(target_os = "windows")]
#[path = "com_matrix_common.rs"]
mod common;
use common::*;

// ── A1 (P0): SAFEARRAY return wrongly admitted → AV ──────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn a1_safearray_return_admit_or_decline() {
    // Most likely hard AV: a Variant-typed member returning a SAFEARRAY runs
    // safe_array_to_com_value on the vtable for the first time live. TestEventServer
    // ReturnLongArray = {4,5,6}. verdict = CLng(arr(LBound)) + CLng(arr(UBound))*1000
    // = 4 + 6*1000 = 6004. Pin the loader's typing via agreement (no exact vt claim).
    let body = "Dim verdict As Long\n\
         Dim arr As Variant\n\
         arr = s.ReturnLongArray()\n\
         verdict = CLng(arr(LBound(arr))) + CLng(arr(UBound(arr))) * 1000\n";
    let late = run_clean(&tes_late(body));
    let early = run_clean_with_references_prefer_vtable(&tes_early(body), tes_ref());
    let do_run = run_clean_with_references_dispatch_only(&tes_early(body), tes_ref());
    assert_differential("A1", late, early, Some(do_run), find_verdict, 6004, None);
}

// ── A2 (P0): [propget,lcid] + real arg ordering (= P13 International) ─────────

#[test]
#[ignore = "live COM; run explicitly"]
fn a2_excel_international_interior_lcid_arg_ordering() {
    // Interior-lcid: the gate inspects only the [supplied..] tail, never the prefix
    // (windows_invoke.rs). International(idx) carries a hidden [lcid] AND a real arg.
    // verdict bits: 1 = country code > 0, 2 = decimal-sep length 1. Expected = 3.
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
    assert_differential("A2", late, early, None, find_verdict, 3, None);
}

// ── A3: PSOA verdict leaking onto a PSDispatch child ─────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn a3_psoa_verdict_not_inherited_by_psdispatch_range() {
    // ws.Range("A1").Value after Worksheets(1) (vtable) returns: a Range bound from a
    // vtable interface retval must re-probe its OWN IID, not inherit the parent PSOA
    // verdict -> else AV. verdict = CLng(value*10) = 425 (value 42.5).
    let body = "Dim verdict As Long\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         Dim ws As Object\n\
         Set ws = wb.Worksheets(1)\n\
         ws.Range(\"A1\").Value = 42.5\n\
         verdict = CLng(ws.Range(\"A1\").Value * 10)\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("A3", late, early, None, find_verdict, 425, None);
}

// ── A4: exact-arity scalar-ret-but-object trap ───────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn a4_dao_open_recordset_exact_arity_return_trust() {
    // DAO OpenRecordset(sql, 4) (2 args supplied = near-exact) — the return-trust
    // guard must still decline the scalar-declared-but-object return; a regression
    // decodes the object as a Long. verdict = rs.Fields(0).Value = 7 with no AV on
    // .Fields. (dbOpenSnapshot = 4.)
    let db = TempDbPath::new("a4_dao");
    let lit = db.as_vba_literal();
    let body = format!(
        "Dim verdict As Long\n\
         Set db = eng.CreateDatabase(\"{lit}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         db.Execute \"CREATE TABLE T (N LONG)\"\n\
         db.Execute \"INSERT INTO T (N) VALUES (7)\"\n\
         Dim rs As Object\n\
         Set rs = db.OpenRecordset(\"SELECT N FROM T\", 4)\n\
         verdict = rs.Fields(0).Value\n\
         rs.Close\n\
         db.Close\n"
    );
    let late = run_clean(&dao_late(&body));
    let early = run_clean_with_references_prefer_vtable(&dao_early(&body), dao_ref());
    assert_differential("A4", late, early, None, find_verdict, 7, None);
}

// ── A5: [propput,lcid] value/lcid ordering ───────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn a5_dao_login_timeout_propput_ordering() {
    // LoginTimeout (value-only put) never tested ordering with a hidden [lcid]; this
    // is a clean property-put round-trip (read-before != after). The DBEngine put is
    // the closest reliable [propput] surface. verdict = IIf(before <> 33 And
    // after = 33, 33, 0) = 33.
    let body = "Dim verdict As Long\n\
         Dim before As Long\n\
         before = eng.LoginTimeout\n\
         eng.LoginTimeout = 33\n\
         Dim after As Long\n\
         after = eng.LoginTimeout\n\
         verdict = IIf(before <> 33 And after = 33, 33, 0)\n";
    let late = run_clean(&dao_late(body));
    let early = run_clean_with_references_prefer_vtable(&dao_early(body), dao_ref());
    let do_run = run_clean_with_references_dispatch_only(&dao_early(body), dao_ref());
    // get + put + get = 3 eligible vtable calls on the in-process DBEngine.
    assert_differential("A5", late, early, Some(do_run), find_verdict, 33, Some(3));
}

// ── A6: per-call QI/pin leak, in-process (exact vtable==1000) ─────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn a6_per_call_qi_pin_leak_in_process() {
    // For i=1 To 1000: t = d.Count. Per-call QI release + libffi pin free. The loop
    // makes EXACTLY 1000 eligible vtable calls; if a pin or QI leaks the run still
    // completes but the count is the proof the path was taken 1000x. verdict = the
    // last Count = 1 (one key added before the loop).
    let body = "Dim verdict As Long\n\
         d.Add \"k\", 1\n\
         Dim i As Long\n\
         Dim t As Long\n\
         For i = 1 To 1000\n\
         t = d.Count\n\
         Next i\n\
         verdict = t\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    // 1 Add + 1000 Count gets = 1001 eligible vtable calls.
    assert_differential("A6", late, early, Some(do_run), find_verdict, 1, Some(1001));
}

// ── A7: per-call QI leak, OOP + orphan process ───────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn a7_per_call_qi_leak_out_of_process_orphan() {
    // For i=1 To 200: b = app.Build. A proxy reference leak -> zombie EXCEL.EXE. The
    // value must match across the loop; the process-baseline check is performed by
    // the live-run script (kill stray EXCEL before/after). verdict = Build > 0 -> 1.
    let body = "Dim verdict As Long\n\
         Dim i As Long\n\
         Dim b As Long\n\
         For i = 1 To 200\n\
         b = app.Build\n\
         Next i\n\
         verdict = IIf(b > 0, 1, 0)\n";
    let late = run_clean(&excel_late(body));
    let early = run_clean_with_references_prefer_vtable(&excel_early(body), excel_ref());
    assert_differential("A7", late, early, None, find_verdict, 1, None);
}

// ── A8 (GAP): supplied-but-Missing Variant optional ──────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn a8_supplied_missing_variant_optional() {
    // GAP 4: Missing (VT_ERROR 0x80020004) vs synthesized-absent must agree. A VBA
    // Optional param left unsupplied forwards as Missing into Worksheets.Add Before.
    // verdict = (sheets_after - sheets_before) = 1 via the omitted-and-Missing form.
    let early_src = "Public verdict As Long\n\
         Sub Main()\n\
         Dim app As Excel.Application\n\
         Set app = CreateObject(\"Excel.Application\")\n\
         app.Visible = False\n\
         app.DisplayAlerts = False\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         verdict = AddOne(wb)\n\
         app.Quit\n\
         End Sub\n\
         Function AddOne(ByVal wb As Object) As Long\n\
         Dim before As Long\n\
         before = wb.Worksheets.Count\n\
         AddSheet wb\n\
         AddOne = wb.Worksheets.Count - before\n\
         End Function\n\
         Sub AddSheet(ByVal wb As Object, Optional ByVal x As Variant)\n\
         wb.Worksheets.Add Before:=x\n\
         End Sub\n";
    let late_src = early_src.replace("Dim app As Excel.Application", "Dim app As Object");
    let late = run_clean(&late_src);
    let early = run_clean_with_references_prefer_vtable(early_src, excel_ref());
    assert_differential("A8", late, early, None, find_verdict, 1, None);
}

// ── A9 (GAP): CallByName forced late dispatch ────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn a9_callbyname_forced_late_dispatch() {
    // GAP 6: CallByName must use IDispatch even on an early-bound receiver, and equal
    // the direct call. verdict = IIf(CallByName(d,"Item",VbGet,"k") = d.Item("k") And
    // d.Item("k") = 99, 99, 0) = 99.
    let body = "Dim verdict As Long\n\
         d.Add \"k\", 99\n\
         Dim viaName As Variant\n\
         viaName = CallByName(d, \"Item\", VbGet, \"k\")\n\
         verdict = IIf(viaName = d.Item(\"k\") And d.Item(\"k\") = 99, 99, 0)\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    // CallByName forces IDispatch; assert agreement + value (no exact vt claim).
    assert_differential("A9", late, early, None, find_verdict, 99, None);
}

// ── A10 (GAP): typelib enum-constant resolution ──────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn a10_enum_constant_resolution() {
    // GAP 8: d.CompareMode = vbTextCompare (named VBA constant = 1) vs = 1 (magic)
    // must agree. (vbTextCompare is a VBA.Constants member, available without the
    // Scripting enum import, so this exercises the named-constant binder path.)
    // verdict = IIf(byConst = byMagic And byConst = 1, 1, 0) = 1.
    let body = "Dim verdict As Long\n\
         d.CompareMode = vbTextCompare\n\
         Dim byConst As Long\n\
         byConst = d.CompareMode\n\
         Dim d2 As Object\n\
         Set d2 = CreateObject(\"Scripting.Dictionary\")\n\
         d2.CompareMode = 1\n\
         Dim byMagic As Long\n\
         byMagic = d2.CompareMode\n\
         verdict = IIf(byConst = byMagic And byConst = 1, 1, 0)\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    assert_differential("A10", late, early, None, find_verdict, 1, None);
}

// ── A11 (GAP): implicit default-member in an expression ──────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn a11_implicit_default_member_in_expression() {
    // GAP 9: implicit default extraction is a distinct binder path. d("a")+d("b") in
    // arithmetic, and "x=" & d("a") in string concat, must both work.
    // verdict bits: 1 = d("a")+d("b") = 30, 2 = ("x=" & d("a")) = "x=10". Expected 3.
    let body = "Dim verdict As Long\n\
         d.Add \"a\", 10\n\
         d.Add \"b\", 20\n\
         verdict = 0\n\
         If d(\"a\") + d(\"b\") = 30 Then verdict = verdict + 1\n\
         If (\"x=\" & d(\"a\")) = \"x=10\" Then verdict = verdict + 2\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    // 2 Add + 3 default-member gets (two in the sum, one in the concat) = 5.
    // RED (flagged gap — see properties P1's note): the 2 Adds vtable-dispatch
    // (unambiguous memid), but the 3 default-member `d("…")` gets are `Item`, whose
    // get/put/putref SHARE a memid, so the memid-only typelib-bind spec lookup declines
    // them to IDispatch (correct value — verdict 3). Currently vtable=2. Closing it needs
    // invoke-kind-aware spec selection (a structural follow-up).
    assert_differential("A11", late, early, Some(do_run), find_verdict, 3, Some(5));
}

// ── A12 (doc-pointer): interior-lcid gate UNIT guard lives in oxvba-com ──────

#[test]
#[ignore = "doc-pointer: the A12 unit guard lives in oxvba-com gate_tests (plain cargo test)"]
fn a12_interior_lcid_gate_unit_guard_pointer() {
    // The plan's A12 is a UNIT guard (NOT live COM): construct a ComMemberSpec with
    // parameter_optional_defaults = [Lcid, Required] (lcid-PREFIX, must DECLINE) vs
    // [Required, Lcid] (lcid-TRAILING, must ADMIT + synthesize), plus
    // is_v1_vtable_vartype(<array>) == false. Those touch crate-private
    // oxvba-com internals (vtable_gate_admits, is_v1_vtable_vartype, ComMemberSpec)
    // unreachable from an oxvba-host integration test, so the guard is added to
    // `crates/oxvba-com/src/windows_invoke.rs` `mod gate_tests` where it runs in
    // plain `cargo test -p oxvba-com` and cannot rot. This stub records that pointer.
    eprintln!(
        "A12 guard: see oxvba-com windows_invoke.rs gate_tests \
         (gate_rejects_lcid_prefix_shape / gate_admits_lcid_trailing_shape)."
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
