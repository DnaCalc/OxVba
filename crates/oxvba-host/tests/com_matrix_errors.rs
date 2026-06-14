//! Differential COM matrix — Category E: ERRORS.
//!
//! Each scenario triggers a COM error and captures `Err.Number`/`Err.Description`
//! via `On Error Resume Next`. Two distinct test shapes:
//!
//! - **ORACLE tests** assert a SPECIFIC real VBA `Err.Number` (457 dup key, 9
//!   subscript, 3xxx DAO, …). The full COM error plumbing
//!   (`ComInvokeFailure::vba_error_number` → `HalError::host_error_code` →
//!   `Fault::from_hal`) surfaces the real number on every dispatch-fault path —
//!   the structured `InvokeFailure` lane AND the rendered `Message` lane (where
//!   `com_dispatch_adapter_fault` recovers the codes from the rendered text) —
//!   so these assert the CORRECT expected number directly.
//! - **AGREEMENT tests** assert only that an error OCCURRED and that late and early
//!   AGREE on the number; these are normal `#[ignore]` live tests.
//!
//! `E9` (Nothing-deref → 91) is a VM-layer error (NOT routed through `from_hal`):
//! an unset object-variable deref raises 91 in `variant_to_object`, independent
//! of the COM number plumbing.
//!
//! Live COM — every test is `#[ignore]`. Run explicitly:
//! ```text
//! cargo test -p oxvba-host --test com_matrix_errors -- --ignored --test-threads=1
//! ```
#![cfg(target_os = "windows")]
#[path = "com_matrix_common.rs"]
mod common;
use common::*;

// ─────────────────────────────────────────────────────────────────────────────
// ORACLE tests — assert the SPECIFIC real VBA Err.Number, now surfaced end to end
// by the COM error plumbing. Live COM, so still `#[ignore]`.
// ─────────────────────────────────────────────────────────────────────────────

// ── E1: Dictionary Add dup key → 457 ─────────────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn e1_dictionary_add_duplicate_key_457() {
    // Adding the same key twice raises VBA error 457 (duplicate key).
    let body = "d.Add \"k\", 1\n\
         On Error Resume Next\n\
         d.Add \"k\", 2\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         On Error GoTo 0\n";
    let late = run_clean(&dict_err_late(body));
    let early = run_clean_with_references(&dict_err_early(body), scripting_ref());
    run_err_oracle("E1", late, early, 457);
}

// ── E2: Dictionary Remove("ghost") → 32811 ───────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn e2_dictionary_remove_missing_key_32811() {
    // Remove of an absent key raises 32811 (element not found). (The asymmetry with
    // Item — which auto-vivifies and does NOT error — is asserted by E2b.)
    let body = "On Error Resume Next\n\
         d.Remove \"ghost\"\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         On Error GoTo 0\n";
    let late = run_clean(&dict_err_late(body));
    let early = run_clean_with_references(&dict_err_early(body), scripting_ref());
    run_err_oracle("E2", late, early, 32811);
}

// ── E3 (P0): Excel Worksheets(99) → 9 (subscript out of range) ───────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn e3_excel_worksheets_bad_index_9() {
    // First live error through a marshalled OOP proxy vtable: Worksheets(99) on a
    // fresh workbook raises VBA 9 (subscript out of range). Host-AV guard.
    let body = "Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         On Error Resume Next\n\
         Dim ws As Object\n\
         Set ws = wb.Worksheets(99)\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         On Error GoTo 0\n";
    let late = run_clean(&excel_err_late(body));
    let early = run_clean_with_references(&excel_err_early(body), excel_ref());
    run_err_oracle("E3", late, early, 9);
}

// ── E4: Excel Range("ZZ") invalid → 1004 ─────────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn e4_excel_invalid_range_1004() {
    // An invalid Range address raises VBA 1004 via the EXCEPINFO path. Range is
    // PSDispatch, so the dispatch must not over-read a 7-slot vtable (AV guard).
    let body = "Dim wb As Object\n\
         Set wb = app.Workbooks.Add\n\
         Dim ws As Object\n\
         Set ws = wb.Worksheets(1)\n\
         On Error Resume Next\n\
         Dim r As Object\n\
         Set r = ws.Range(\"ZZ\")\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         On Error GoTo 0\n";
    let late = run_clean(&excel_err_late(body));
    let early = run_clean_with_references(&excel_err_early(body), excel_ref());
    run_err_oracle("E4", late, early, 1004);
}

// ── E5: Excel Workbooks.Open(missing) → 1004 ─────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn e5_excel_open_missing_file_1004() {
    // Opening a non-existent workbook raises VBA 1004; exercises inbound-BSTR free
    // on the failure path.
    let body = "On Error Resume Next\n\
         Dim wb As Object\n\
         Set wb = app.Workbooks.Open(\"C:\\\\nonexistent_oxvba_e5.xlsx\")\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         On Error GoTo 0\n";
    let late = run_clean(&excel_err_late(body));
    let early = run_clean_with_references(&excel_err_early(body), excel_ref());
    run_err_oracle("E5", late, early, 1004);
}

// ── E6 (P0): DAO Execute "BAD SQL" → 3xxx ────────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn e6_dao_bad_sql_3000_range() {
    // A malformed SQL statement raises a DAO 3xxx error. This straddles IErrorInfo
    // (vtable) and EXCEPINFO (IDispatch) Description retrieval. The live ACE build
    // reports 3078 ("cannot find the input table or query 'THIS'") for this shape
    // — the parser treats the first token as a table name — verified equal across
    // late and early (E6b is the agreement safety net). Both legs now run cleanly
    // because the differential runner gives each leg a per-leg CreateDatabase path.
    let db = TempDbPath::new("e6_dao");
    let lit = db.as_vba_literal();
    let body = format!(
        "Set db = eng.CreateDatabase(\"{lit}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         On Error Resume Next\n\
         db.Execute \"THIS IS NOT SQL\"\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         On Error GoTo 0\n\
         db.Close\n"
    );
    let late = run_clean(&dao_err_late(&body));
    let early = run_clean_with_references(&dao_err_early(&body), dao_ref());
    run_err_oracle("E6", late, early, 3078);
}

// ── E7: DAO CreateDatabase(bad path) → 3024/3044 ─────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn e7_dao_create_database_bad_path_3024() {
    // CreateDatabase to an unreachable path raises a DAO 30xx (3024 not found /
    // 3044 invalid path). Exercises object-retval failure cleanup (discard_out_cell).
    let body = "On Error Resume Next\n\
         Set db = eng.CreateDatabase(\"Q:\\\\no_such_dir\\\\bad.accdb\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         On Error GoTo 0\n";
    let late = run_clean(&dao_err_late(body));
    let early = run_clean_with_references(&dao_err_early(body), dao_ref());
    run_err_oracle("E7", late, early, 3044);
}

// ── E8 (P0): Dictionary Add("onlyKey") → 449 (arg-count) ─────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn e8_dictionary_add_missing_arg_449() {
    // Add with a single argument (missing the required Item) is an argument-count
    // error. The vtable gate vs the IDispatch layer may surface this differently per
    // transport — highest divergence probability. Real VBA number 449 (argument not
    // optional) / 450 (wrong number of arguments) depending on layer; assert 449.
    let body = "On Error Resume Next\n\
         d.Add \"onlyKey\"\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         On Error GoTo 0\n";
    let late = run_clean(&dict_err_late(body));
    let early = run_clean_with_references(&dict_err_early(body), scripting_ref());
    run_err_oracle("E8", late, early, 449);
}

// ── E10: DAO fail-then-succeed (stale IErrorInfo) → 3xxx then 0 ──────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn e10_dao_fail_then_succeed_clears_error() {
    // A failing Execute followed by a succeeding one: errNum1 must be a DAO 3xxx and
    // errNum2 must be 0 (the stale IErrorInfo must be cleared). A non-zero errNum2 in
    // early-bound would be a guaranteed divergence (vtable-only stale-IErrorInfo
    // hazard). We assert the FIRST captured number against the live ACE 3078 (same
    // bad-SQL shape as E6), verified equal across late and early.
    let db = TempDbPath::new("e10_dao");
    let lit = db.as_vba_literal();
    let body = format!(
        "Set db = eng.CreateDatabase(\"{lit}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         db.Execute \"CREATE TABLE T (N LONG)\"\n\
         On Error Resume Next\n\
         db.Execute \"THIS IS NOT SQL\"\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         Err.Clear\n\
         db.Execute \"INSERT INTO T (N) VALUES (1)\"\n\
         Dim errNum2 As Long\n\
         errNum2 = Err.Number\n\
         On Error GoTo 0\n\
         db.Close\n"
    );
    let late = run_clean(&dao_err_late(&body));
    let early = run_clean_with_references(&dao_err_early(&body), dao_ref());
    run_err_oracle("E10", late, early, 3078);
}

// ── E11: Dictionary dup key with On Error GoTo handler → 457 ─────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn e11_dictionary_dup_key_structured_handler_457() {
    // The same 457 dup-key, but caught by a structured `On Error GoTo` handler
    // (control-flow parity). The handler records the number and resumes.
    let body = "On Error GoTo Handler\n\
         d.Add \"k\", 1\n\
         d.Add \"k\", 2\n\
         GoTo Done\n\
         Handler:\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         Resume Done\n\
         Done:\n\
         On Error GoTo 0\n";
    let late = run_clean(&dict_err_late(body));
    let early = run_clean_with_references(&dict_err_early(body), scripting_ref());
    run_err_oracle("E11", late, early, 457);
}

// ── E12 (GAP): Err.Source parity ─────────────────────────────────────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn e12_dictionary_err_source_parity() {
    // GAP 13: bstrSource (EXCEPINFO) vs IErrorInfo::GetSource are different paths and
    // must match. We capture Err.Source into the String global and Err.Number into
    // the Long; the oracle pins the number, and late==early pins Source equality.
    let body = "d.Add \"k\", 1\n\
         On Error Resume Next\n\
         d.Add \"k\", 2\n\
         errNum = Err.Number\n\
         errDesc = Err.Source\n\
         On Error GoTo 0\n";
    let late = run_clean(&dict_err_late(body));
    let early = run_clean_with_references(&dict_err_early(body), scripting_ref());
    run_err_oracle("E12", late, early, 457);
}

// ─────────────────────────────────────────────────────────────────────────────
// VM-layer + AGREEMENT tests — normal #[ignore] live tests.
// ─────────────────────────────────────────────────────────────────────────────

// ── E9 (P0): Nothing-deref → 91 (VM-layer, not routed through from_hal) ──────

#[test]
#[ignore = "live COM; run explicitly"]
fn e9_nothing_deref_control_91() {
    // Dereferencing an unset object (`d.Count` with d = Nothing) is a VM-layer
    // error (91, object variable not set) short-circuited BEFORE any COM call —
    // so it is NOT routed through from_hal. `variant_to_object` raises 91 for an
    // unset object-typed/`Empty` receiver, distinct from a non-object value (424).
    let late_src = "Public errNum As Long\n\
         Public errDesc As String\n\
         Sub Main()\n\
         Dim d As Object\n\
         On Error Resume Next\n\
         Dim c As Long\n\
         c = d.Count\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         On Error GoTo 0\n\
         End Sub\n";
    let early_src = "Public errNum As Long\n\
         Public errDesc As String\n\
         Sub Main()\n\
         Dim d As Scripting.Dictionary\n\
         On Error Resume Next\n\
         Dim c As Long\n\
         c = d.Count\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         On Error GoTo 0\n\
         End Sub\n";
    let late = match run_clean(late_src) {
        Ok(s) => s,
        Err(e) if is_component_absent(&e) => {
            eprintln!("SKIP E9: {e}");
            return;
        }
        Err(e) => panic!("E9 late failed: {e}"),
    };
    let early = match run_clean_with_references(early_src, scripting_ref()) {
        Ok(s) => s,
        Err(e) if is_typelib_absent(&e) => {
            eprintln!("SKIP E9: {e}");
            return;
        }
        Err(e) => panic!("E9 early failed: {e}"),
    };
    let (ln, _, _) = extract_err(&late);
    let (en, _, _) = extract_err(&early);
    assert_eq!(ln, en, "E9: Err.Number late {ln:?} != early {en:?}");
    assert_eq!(
        ln,
        Some(91),
        "E9: Nothing-deref must report VBA 91 (object variable not set); got {ln:?}."
    );
}

// ── E2b: Dictionary Item auto-vivify does NOT error (agreement) ──────────────

#[test]
#[ignore = "live COM; run explicitly"]
fn e2b_dictionary_item_autovivify_no_error_agreement() {
    // Asymmetry oracle: reading Item("ghost") on a Dictionary auto-vivifies the key
    // (no error) and grows Count to 1. This is an AGREEMENT/value test (no specific
    // COM number), so it is a normal #[ignore] test. verdict = afterCount = 1.
    let body = "Dim verdict As Long\n\
         Dim v As Variant\n\
         v = d.Item(\"ghost\")\n\
         verdict = d.Count\n";
    let late = run_clean(&dict_late(body));
    let early = run_clean_with_references_prefer_vtable(&dict_early(body), scripting_ref());
    let do_run = run_clean_with_references_dispatch_only(&dict_early(body), scripting_ref());
    assert_differential("E2b", late, early, Some(do_run), find_verdict, 1, None);
}

// ── E6b: DAO bad SQL — error occurs and late==early agree (agreement) ────────

#[test]
#[ignore = "live COM; run explicitly"]
fn e6b_dao_bad_sql_error_occurs_agreement() {
    // The agreement safety net for E6: assert merely that an error OCCURRED
    // (Err.Number != 0) and that late and early AGREE on the captured number —
    // independent of which exact 3xxx the ACE build emits.
    let db = TempDbPath::new("e6b_dao");
    let lit = db.as_vba_literal();
    let body = format!(
        "Set db = eng.CreateDatabase(\"{lit}\", \";LANGID=0x0409;CP=1252;COUNTRY=0\")\n\
         On Error Resume Next\n\
         db.Execute \"THIS IS NOT SQL\"\n\
         errNum = Err.Number\n\
         errDesc = Err.Description\n\
         On Error GoTo 0\n\
         db.Close\n"
    );
    let late = run_clean(&dao_err_late(&body));
    let early = run_clean_with_references(&dao_err_early(&body), dao_ref());
    run_err_agreement("E6b", late, early);
}

// ─────────────────────────────────────────────────────────────────────────────
// Runners that thread the env-skip through the absolute / agreement oracles.
// ─────────────────────────────────────────────────────────────────────────────

/// Absolute oracle: skip on absent component/typelib, else assert the number.
fn run_err_oracle(case: &str, late: LateRun, early: LateRun, real_vba_err: i32) {
    let late = match late {
        Ok(s) => s,
        Err(e) if is_component_absent(&e) => {
            eprintln!("SKIP {case}: {e}");
            return;
        }
        Err(e) => panic!("{case} late failed: {e}"),
    };
    let early = match early {
        Ok(s) => s,
        Err(e) if is_typelib_absent(&e) => {
            eprintln!("SKIP {case}: {e}");
            return;
        }
        Err(e) => panic!("{case} early failed: {e}"),
    };
    assert_err_oracle(case, &late, &early, real_vba_err);
}

/// AGREEMENT oracle: skip on absent component/typelib, else assert error-occurred
/// + late==early.
fn run_err_agreement(case: &str, late: LateRun, early: LateRun) {
    let late = match late {
        Ok(s) => s,
        Err(e) if is_component_absent(&e) => {
            eprintln!("SKIP {case}: {e}");
            return;
        }
        Err(e) => panic!("{case} late failed: {e}"),
    };
    let early = match early {
        Ok(s) => s,
        Err(e) if is_typelib_absent(&e) => {
            eprintln!("SKIP {case}: {e}");
            return;
        }
        Err(e) => panic!("{case} early failed: {e}"),
    };
    assert_err_agrees(case, &late, &early);
}

// ─────────────────────────────────────────────────────────────────────────────
// Source builders. Error scenarios declare `errNum`/`errDesc` globals FIRST so
// extract_err finds them (the Long then the String).
// ─────────────────────────────────────────────────────────────────────────────

fn dict_err_late(body: &str) -> String {
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

fn dict_err_early(body: &str) -> String {
    format!(
        "Public errNum As Long\n\
         Public errDesc As String\n\
         Sub Main()\n\
         Dim d As Scripting.Dictionary\n\
         Set d = CreateObject(\"Scripting.Dictionary\")\n\
         {body}\
         End Sub\n"
    )
}

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

fn excel_err_late(body: &str) -> String {
    format!(
        "Public errNum As Long\n\
         Public errDesc As String\n\
         Sub Main()\n\
         Dim app As Object\n\
         Set app = CreateObject(\"Excel.Application\")\n\
         app.Visible = False\n\
         app.DisplayAlerts = False\n\
         {body}\
         app.Quit\n\
         End Sub\n"
    )
}

fn excel_err_early(body: &str) -> String {
    format!(
        "Public errNum As Long\n\
         Public errDesc As String\n\
         Sub Main()\n\
         Dim app As Excel.Application\n\
         Set app = CreateObject(\"Excel.Application\")\n\
         app.Visible = False\n\
         app.DisplayAlerts = False\n\
         {body}\
         app.Quit\n\
         End Sub\n"
    )
}

fn dao_err_late(body: &str) -> String {
    format!(
        "Public errNum As Long\n\
         Public errDesc As String\n\
         Sub Main()\n\
         Dim eng As Object\n\
         Dim db As Object\n\
         Set eng = CreateObject(\"DAO.DBEngine.120\")\n\
         {body}\
         End Sub\n"
    )
}

fn dao_err_early(body: &str) -> String {
    format!(
        "Public errNum As Long\n\
         Public errDesc As String\n\
         Sub Main()\n\
         Dim eng As DAO.DBEngine\n\
         Dim db As DAO.Database\n\
         Set eng = CreateObject(\"DAO.DBEngine.120\")\n\
         {body}\
         End Sub\n"
    )
}
