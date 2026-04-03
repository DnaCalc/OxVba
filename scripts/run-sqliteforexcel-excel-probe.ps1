param(
    [string]$WorkbookPath = "C:\Work\SqliteForExcel\Distribution\SQLiteForExcel_64.xlsm",
    [switch]$Staged
)

$ErrorActionPreference = "Stop"

$reportPath = Join-Path $env:TEMP "sqliteforexcel_excel_probe_report.txt"
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

try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excel.AutomationSecurity = 1

    $wb = $excel.Workbooks.Open($WorkbookPath)
    $module = $wb.VBProject.VBComponents.Add(1)
    $module.Name = "OxVbaProbeModule"

    if ($Staged) {
        $vba = @"
Option Explicit

Private Sub ProbeLog(ByVal message As String)
    Dim ff As Integer
    ff = FreeFile
    Open "$reportPath" For Append As #ff
    Print #ff, message
    Close #ff
End Sub

Public Sub OxVbaProbeRun()
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

    ProbeLog "CALL TestOpenCloseV2"
    Call Sqlite3Demo.TestOpenCloseV2
    ProbeLog "DONE TestOpenCloseV2"

    ProbeLog "CALL TestError"
    Call Sqlite3Demo.TestError
    ProbeLog "DONE TestError"

    ProbeLog "CALL TestInsert"
    Call Sqlite3Demo.TestInsert
    ProbeLog "DONE TestInsert"

    ProbeLog "CALL TestSelect"
    Call Sqlite3Demo.TestSelect
    ProbeLog "DONE TestSelect"

    ProbeLog "CALL TestBinding"
    Call Sqlite3Demo.TestBinding
    ProbeLog "DONE TestBinding"

    ProbeLog "CALL TestDates"
    Call Sqlite3Demo.TestDates
    ProbeLog "DONE TestDates"

    ProbeLog "CALL TestStrings"
    Call Sqlite3Demo.TestStrings
    ProbeLog "DONE TestStrings"

    ProbeLog "CALL TestBackup"
    Call Sqlite3Demo.TestBackup
    ProbeLog "DONE TestBackup"

    ProbeLog "CALL TestBlob"
    Call Sqlite3Demo.TestBlob
    ProbeLog "DONE TestBlob"

    ProbeLog "CALL TestWriteReadOnly"
    Call Sqlite3Demo.TestWriteReadOnly
    ProbeLog "DONE TestWriteReadOnly"

    ProbeLog "CALL SQLite3Free"
    Call SQLite3Free
    ProbeLog "DONE SQLite3Free"
    ProbeLog "RUN_OK"
    Exit Sub

Fail:
    ProbeLog "RUN_FAIL"
    ProbeLog CStr(Err.Number)
    ProbeLog Err.Description
End Sub
"@
    } else {
        $vba = @"
Option Explicit

Public Sub OxVbaProbeRun()
    Dim ff As Integer

    ff = FreeFile
    On Error GoTo Fail

    Open "$reportPath" For Output As #ff
    Print #ff, "START"
    Close #ff

    Call Sqlite3Demo.AllTests

    ff = FreeFile
    Open "$reportPath" For Append As #ff
    Print #ff, "RUN_OK"
    Close #ff
    Exit Sub

Fail:
    On Error Resume Next
    ff = FreeFile
    Open "$reportPath" For Append As #ff
    Print #ff, "RUN_FAIL"
    Print #ff, CStr(Err.Number)
    Print #ff, Err.Description
    Close #ff
End Sub
"@
    }

    $module.CodeModule.AddFromString($vba)
    $excel.Run("OxVbaProbeRun")

    if (Test-Path $reportPath) {
        Get-Content $reportPath
    } else {
        Write-Output "REPORT_MISSING"
    }

    if (Test-Path $tempDb) {
        Get-Item $tempDb | Select-Object FullName, Length, LastWriteTime
    } else {
        Write-Output "TEMP_DB_MISSING"
    }
} finally {
    if ($wb -ne $null) {
        try {
            $wb.Close($false) | Out-Null
        } catch {
        }
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb) | Out-Null
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
