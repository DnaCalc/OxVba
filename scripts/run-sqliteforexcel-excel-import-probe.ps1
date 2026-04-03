param(
    [string]$SqliteModulePath = "C:\Work\SqliteForExcel\Source\SQLite3VBAModules\Sqlite3_64.bas",
    [string]$DemoModulePath = "C:\Work\SqliteForExcel\Source\SQLite3VBAModules\Sqlite3Demo_64.bas"
)

$ErrorActionPreference = "Stop"

$reportPath = Join-Path $env:TEMP "sqliteforexcel_excel_import_probe_report.txt"
$tempDb = Join-Path $env:TEMP "TestSqlite3ForExcel.db3"

if (Test-Path $reportPath) {
    Remove-Item $reportPath -Force
}
if (Test-Path $tempDb) {
    Remove-Item $tempDb -Force
}

$excel = $null
$wb = $null
$module = $null
$sheet = $null

try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excel.AutomationSecurity = 1

    $wb = $excel.Workbooks.Add()
    $sheet = $wb.Worksheets.Item(1)
    $sheet.Name = "Probe"

    $vbProject = $wb.VBProject
    $null = $vbProject.VBComponents.Import($SqliteModulePath)
    $null = $vbProject.VBComponents.Import($DemoModulePath)

    $module = $vbProject.VBComponents.Add(1)
    $module.Name = "OxVbaImportProbe"

    $vba = @"
Option Explicit

Private Sub ProbeLog(ByVal message As String)
    Dim ff As Integer
    ff = FreeFile
    Open "$reportPath" For Append As #ff
    Print #ff, message
    Close #ff
End Sub

Public Sub OxVbaImportProbeRun()
    Dim initReturn As Long

    On Error GoTo Fail
    ProbeLog "START"
    ProbeLog "CALL SQLite3Initialize"
    initReturn = SQLite3Initialize(ThisWorkbook.Path + "\x64")
    ProbeLog "INIT_RETURN=" & CStr(initReturn)
    If initReturn <> SQLITE_INIT_OK Then
        ProbeLog "INIT_FAIL_ERR=" & CStr(Err.LastDllError)
        Exit Sub
    End If

    ProbeLog "CALL TestVersion"
    Call Sqlite3Demo.TestVersion
    ProbeLog "DONE TestVersion"

    ProbeLog "CALL TestOpenClose"
    Call Sqlite3Demo.TestOpenClose
    ProbeLog "DONE TestOpenClose"

    ProbeLog "RUN_OK"
    Exit Sub

Fail:
    ProbeLog "RUN_FAIL"
    ProbeLog CStr(Err.Number)
    ProbeLog Err.Description
End Sub
"@

    $module.CodeModule.AddFromString($vba)
    $excel.Run("OxVbaImportProbeRun")

    if (Test-Path $reportPath) {
        Get-Content $reportPath
    } else {
        Write-Output "REPORT_MISSING"
    }
} finally {
    if ($wb -ne $null) {
        try {
            $wb.Close($false) | Out-Null
        } catch {
        }
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb) | Out-Null
    }
    if ($sheet -ne $null) {
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($sheet) | Out-Null
    }
    if ($module -ne $null) {
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($module) | Out-Null
    }
    if ($excel -ne $null) {
        try {
            $excel.Quit() | Out-Null
        } catch {
        }
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel) | Out-Null
    }
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
}
