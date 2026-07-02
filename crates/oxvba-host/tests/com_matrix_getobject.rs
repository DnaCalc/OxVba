//! Differential COM matrix - GetObject activation.
//!
//! `GetObject` modes against live COM. The tests keep their own live dependencies: the
//! running-instance row creates an owned Excel.Application before binding through the ROT,
//! and the file-bind row creates a temporary workbook before calling `GetObject(pathname)`.
//!
//! Live COM — every test is `#[ignore]`. Run explicitly:
//! ```text
//! cargo test -p oxvba-host --test com_matrix_getobject -- --ignored --test-threads=1
//! ```
#![cfg(target_os = "windows")]
#[path = "com_matrix_common.rs"]
mod common;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use common::*;

struct TempWorkbookPath(PathBuf);

impl TempWorkbookPath {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "oxvba_{name}_{}_{}.xlsx",
            std::process::id(),
            stamp
        )))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempWorkbookPath {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn run_vm3_live(case: &str, source: &str) -> Option<Vec<Variant>> {
    match run_clean_vm3(source) {
        Ok(snapshot) => Some(snapshot),
        Err(err) if is_component_absent(&err) || is_typelib_absent(&err) => {
            eprintln!("SKIP {case}: {err}");
            None
        }
        Err(err) => panic!("{case} vm3 failed: {err}"),
    }
}

fn first_nonempty_string(snapshot: &[Variant]) -> Option<String> {
    snapshot
        .iter()
        .find_map(|value| value.as_bstr().map(|bstr| bstr.as_str()))
        .filter(|text| !text.is_empty())
}

fn assert_non_error_string(case: &str, snapshot: &[Variant]) {
    let text = first_nonempty_string(snapshot)
        .unwrap_or_else(|| panic!("{case}: expected a non-empty String in {snapshot:?}"));
    assert!(
        !text.starts_with("ERR:"),
        "{case}: VBA scenario captured {text:?} in {snapshot:?}"
    );
}

fn vba_string_literal(value: &Path) -> String {
    value.to_string_lossy().replace('"', "\"\"")
}

fn create_excel_workbook(path: &Path, cell_value: &str) -> Result<(), String> {
    fn quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "''"))
    }

    let command = format!(
        r#"
$ErrorActionPreference = "Stop"
$WorkbookPath = {path}
$CellValue = {cell_value}
$excel = New-Object -ComObject Excel.Application
$excel.Visible = $false
$excel.DisplayAlerts = $false
$wb = $null
try {{
    $wb = $excel.Workbooks.Add()
    $wb.Worksheets.Item(1).Range("A1").Value2 = $CellValue
    $wb.SaveAs($WorkbookPath, 51)
}}
finally {{
    if ($wb -ne $null) {{
        $wb.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
    }}
    $excel.Quit()
    [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
}}
"#,
        path = quote(&path.to_string_lossy()),
        cell_value = quote(cell_value)
    );

    let output = Command::new("pwsh")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &command,
        ])
        .output()
        .map_err(|err| format!("launch pwsh: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

// -- G1: GetObject("", progid) - new instance (== CreateObject) ----------------

#[test]
#[ignore = "live COM; run explicitly"]
fn getobject_empty_path_creates_a_new_instance_vm3() {
    // `GetObject("", "Scripting.Dictionary")` returns a new Dictionary: the zero-length
    // pathname mode is identical to `CreateObject`. Add one key, read `Count` -> 1,
    // proving the object is live and dispatches on vm3.
    let src = "Public verdict As Long\n\
         Sub Main()\n\
         Dim d As Object\n\
         Set d = GetObject(\"\", \"Scripting.Dictionary\")\n\
         d.Add \"k\", 1\n\
         verdict = d.Count\n\
         End Sub\n";
    assert_vm3_verdict(
        "GetObject empty pathname creates a new instance",
        run_clean_vm3(src),
        find_verdict,
        1,
    );
}

// -- G2: GetObject(, progid) - running instance via the ROT --------------------

#[test]
#[ignore = "live COM; creates an owned Excel instance; run explicitly"]
fn getobject_omitted_path_binds_owned_running_excel_vm3() {
    // `GetObject(, "Excel.Application")` binds the currently-running Excel registered in the
    // Running Object Table and reads a trivial property. The owned instance is cleaned up
    // inside the VBA scenario; an ERR: verdict is a real failure, not an environment skip.
    let src = "Public verdict As String\n\
         Sub Main()\n\
         Dim owner As Object\n\
         Dim wb As Object\n\
         Dim app As Object\n\
         On Error GoTo Fail\n\
         Set owner = CreateObject(\"Excel.Application\")\n\
         owner.Visible = False\n\
         owner.DisplayAlerts = False\n\
         Set wb = owner.Workbooks.Add\n\
         Set app = GetObject(, \"Excel.Application\")\n\
         verdict = app.Version\n\
Clean:\n\
         On Error Resume Next\n\
         If Not (wb Is Nothing) Then wb.Close False\n\
         If Not (owner Is Nothing) Then owner.Quit\n\
         Exit Sub\n\
Fail:\n\
         verdict = \"ERR:\" & CStr(Err.Number) & \":\" & Err.Description\n\
         Resume Clean\n\
         End Sub\n";
    if let Some(snapshot) = run_vm3_live("GetObject omitted pathname binds running Excel", src) {
        assert_non_error_string("GetObject omitted pathname binds running Excel", &snapshot);
    }
}

// -- G3: GetObject(pathname) - bind a workbook file through CoGetObject ---------

#[test]
#[ignore = "live COM; creates a temporary Excel workbook; run explicitly"]
fn getobject_path_binds_excel_workbook_file_vm3() {
    // `GetObject(pathname)` binds the object named by a workbook file. The workbook
    // fixture is prepared outside OxVBA so the scenario's observable path is the
    // file-moniker bind itself.
    let path = TempWorkbookPath::new("getobject_file_bind");
    let cell_value = "vm3-getobject-file-bind";
    if let Err(err) = create_excel_workbook(path.path(), cell_value) {
        if is_component_absent(&err) {
            eprintln!("SKIP GetObject workbook file bind: {err}");
            return;
        }
        panic!("prepare Excel workbook for GetObject file bind failed: {err}");
    }

    let path_literal = vba_string_literal(path.path());
    let src = format!(
        "Public verdict As String\n\
         Sub Main()\n\
         Dim wb As Object\n\
         Dim app As Object\n\
         On Error GoTo Fail\n\
         Set wb = GetObject(\"{path_literal}\")\n\
         Set app = wb.Application\n\
         app.DisplayAlerts = False\n\
         verdict = CStr(wb.Worksheets(1).Range(\"A1\").Value)\n\
Clean:\n\
         On Error Resume Next\n\
         If Not (wb Is Nothing) Then wb.Close False\n\
         If Not (app Is Nothing) Then app.Quit\n\
         Exit Sub\n\
Fail:\n\
         verdict = \"ERR:\" & CStr(Err.Number) & \":\" & Err.Description\n\
         Resume Clean\n\
         End Sub\n"
    );

    if let Some(snapshot) = run_vm3_live("GetObject workbook file bind", &src) {
        assert_str(
            &snapshot,
            cell_value,
            "GetObject workbook file bind should read the saved cell",
        );
    }
}
