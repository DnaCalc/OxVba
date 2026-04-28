param(
    [string]$StagingManifest = "",
    [string]$ExpectedCsv = "examples/xll/scalar_addin/expected.csv",
    [string]$OutputRoot = "target/xll-host-validation/excel-worksheet",
    [string]$RunId = "",
    [switch]$AllowUnavailable,
    [switch]$DisableDialogGuardian,
    [int]$DialogGuardianPollMs = 250,
    [int]$DialogGuardianMaxSeconds = 300
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if ([string]::IsNullOrWhiteSpace($RunId)) {
        $RunId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    }
    if ([string]::IsNullOrWhiteSpace($StagingManifest)) {
        $latestManifest = Get-ChildItem -Path "target/xll-host-validation/scalar_addin" -Filter "manifest.json" -Recurse -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTimeUtc -Descending |
            Select-Object -First 1
        if ($null -eq $latestManifest) {
            throw "No staged XLL manifest found. Run scripts/stage-xll-scalar-addin.ps1 first."
        }
        $StagingManifest = $latestManifest.FullName
    }

    $manifestPath = (Resolve-Path -LiteralPath $StagingManifest).Path
    $expectedPath = (Resolve-Path -LiteralPath $ExpectedCsv).Path
    $staging = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $artifactPath = [string]$staging.artifact_path
    if (-not (Test-Path -LiteralPath $artifactPath)) {
        throw "Staged XLL artifact is missing: $artifactPath"
    }

    $runDir = Join-Path $OutputRoot $RunId
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null
    $resultPath = Join-Path $runDir "worksheet_result.json"
    $resultsCsvPath = Join-Path $runDir "worksheet_results.csv"
    $workbookPath = Join-Path $runDir "worksheet_smoke.xlsx"
    $guardianLog = Join-Path $runDir "excel_dialog_guardian.log"
    $guardianStop = Join-Path $runDir "excel_dialog_guardian.stop"
    $xllTracePath = Join-Path $runDir "xll_trace.log"

    Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class OxVbaXllWorksheetWin32User32Pid {
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@

    function Get-WindowProcessId {
        param([int]$Hwnd)
        [uint32]$windowPid = 0
        [void][OxVbaXllWorksheetWin32User32Pid]::GetWindowThreadProcessId([IntPtr]::new($Hwnd), [ref]$windowPid)
        [int]$windowPid
    }

    function Test-ObservedValue {
        param($Observed, [string]$CellText, [string]$Expected)
        $trimmed = $Expected.Trim()
        if ($trimmed.Equals("TRUE", [System.StringComparison]::OrdinalIgnoreCase) -or
            $trimmed.Equals("FALSE", [System.StringComparison]::OrdinalIgnoreCase)) {
            $expectedBool = $trimmed.Equals("TRUE", [System.StringComparison]::OrdinalIgnoreCase)
            if ($Observed -is [bool]) {
                return ([bool]$Observed) -eq $expectedBool
            }
            return $CellText.Equals($trimmed, [System.StringComparison]::OrdinalIgnoreCase)
        }
        $number = 0.0
        if ([double]::TryParse(
                $trimmed,
                [System.Globalization.NumberStyles]::Float,
                [System.Globalization.CultureInfo]::InvariantCulture,
                [ref]$number
            )) {
            try {
                $observedNumber = [double]$Observed
                return [Math]::Abs($observedNumber - $number) -lt 0.000000001
            } catch {
                return $false
            }
        }
        [string]$Observed -eq $trimmed
    }

    function Write-Result {
        param([hashtable]$Result)
        $Result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $resultPath -Encoding utf8
    }

    $expectedRows = Import-Csv -LiteralPath $expectedPath
    $startedAt = (Get-Date).ToUniversalTime().ToString("o")
    $result = [ordered]@{
        run_id = $RunId
        started_at = $startedAt
        ended_at = $null
        status = "not_run"
        staging_manifest = $manifestPath
        expected_csv = $expectedPath
        artifact_path = (Resolve-Path -LiteralPath $artifactPath).Path
        artifact_bytes = (Get-Item -LiteralPath $artifactPath).Length
        excel_version = $null
        excel_build = $null
        excel_operating_system = $null
        excel_path = $null
        excel_pid = $null
        register_xll_return = $null
        workbook_path = $workbookPath
        results_csv = $resultsCsvPath
        xll_trace = $xllTracePath
        xll_trace_exists = $false
        guardian_log = $guardianLog
        passed = 0
        failed = 0
        error = $null
    }

    $excel = $null
    $workbook = $null
    $worksheet = $null
    $guardianProc = $null
    $oldTraceEnv = [Environment]::GetEnvironmentVariable("OXVBA_XLL_TRACE", "Process")
    $rows = New-Object System.Collections.Generic.List[object]
    try {
        [Environment]::SetEnvironmentVariable("OXVBA_XLL_TRACE", (Join-Path (Get-Location) $xllTracePath), "Process")
        try {
            $excel = New-Object -ComObject Excel.Application
        } catch {
            $result.status = "excel_unavailable"
            $result.error = $_.Exception.Message
            $result.ended_at = (Get-Date).ToUniversalTime().ToString("o")
            Write-Result -Result $result
            if ($AllowUnavailable) {
                Write-Host "excel unavailable; result: $resultPath"
                return
            }
            throw
        }

        $excel.Visible = $false
        $excel.DisplayAlerts = $false
        $result.excel_version = [string]$excel.Version
        $result.excel_build = [string]$excel.Build
        $result.excel_operating_system = [string]$excel.OperatingSystem
        $result.excel_path = [string]$excel.Path
        $result.excel_pid = Get-WindowProcessId -Hwnd ([int]$excel.Hwnd)

        if (-not $DisableDialogGuardian) {
            $guardianProc = Start-Process -FilePath "powershell" -WindowStyle Hidden -PassThru -ArgumentList @(
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                (Join-Path $PSScriptRoot "excel-dialog-guardian.ps1"),
                "-ExcelPid",
                ([string]$result.excel_pid),
                "-StopFile",
                $guardianStop,
                "-LogFile",
                $guardianLog,
                "-PollMs",
                ([string]$DialogGuardianPollMs),
                "-MaxSeconds",
                ([string]$DialogGuardianMaxSeconds)
            )
        }

        $registered = [bool]$excel.RegisterXLL($artifactPath)
        $result.register_xll_return = $registered
        if (-not $registered) {
            throw "Excel RegisterXLL returned False"
        }

        $workbook = $excel.Workbooks.Add()
        $worksheet = $workbook.Worksheets.Item(1)
        $rowIndex = 1
        foreach ($expected in $expectedRows) {
            $cell = $worksheet.Cells.Item($rowIndex, 1)
            $cell.Formula = [string]$expected.formula
            $rowIndex += 1
        }
        $excel.CalculateFullRebuild()

        $rowIndex = 1
        foreach ($expected in $expectedRows) {
            $cell = $worksheet.Cells.Item($rowIndex, 1)
            $observed = $cell.Value2
            $cellText = [string]$cell.Text
            $match = Test-ObservedValue -Observed $observed -CellText $cellText -Expected ([string]$expected.expected)
            if ($match) {
                $result.passed += 1
            } else {
                $result.failed += 1
            }
            $rows.Add([pscustomobject]@{
                    function = [string]$expected.function
                    formula = [string]$expected.formula
                    expected = [string]$expected.expected
                    observed_value = [string]$observed
                    observed_text = $cellText
                    match = if ($match) { "true" } else { "false" }
                }) | Out-Null
            $rowIndex += 1
        }

        $rows | Export-Csv -LiteralPath $resultsCsvPath -NoTypeInformation
        $workbook.SaveAs((Join-Path (Get-Location) $workbookPath), 51)
        $result.status = if ($result.failed -eq 0) { "passed" } else { "failed" }
        if ($result.failed -ne 0) {
            throw "Worksheet invocation mismatches: $($result.failed)"
        }
    } catch {
        if ($result.status -eq "not_run") {
            $result.status = "failed"
        }
        $result.error = $_.Exception.Message
        throw
    } finally {
        $result.ended_at = (Get-Date).ToUniversalTime().ToString("o")
        $result.xll_trace_exists = Test-Path -LiteralPath $xllTracePath
        Write-Result -Result $result
        [Environment]::SetEnvironmentVariable("OXVBA_XLL_TRACE", $oldTraceEnv, "Process")

        if ($workbook -ne $null) {
            try { $workbook.Close($false) | Out-Null } catch {}
            try { [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($workbook) } catch {}
        }
        if ($worksheet -ne $null) {
            try { [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($worksheet) } catch {}
        }
        if ($excel -ne $null) {
            try { $excel.Quit() | Out-Null } catch {}
            try { [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel) } catch {}
        }
        if ($guardianProc -ne $null) {
            try { New-Item -ItemType File -Force -Path $guardianStop | Out-Null } catch {}
            try {
                Wait-Process -Id $guardianProc.Id -Timeout 5 -ErrorAction SilentlyContinue
            } catch {}
            if (-not $guardianProc.HasExited) {
                try { Stop-Process -Id $guardianProc.Id -Force -ErrorAction SilentlyContinue } catch {}
            }
        }
    }

    Write-Host "xll excel worksheet smoke: $($result.status) passed=$($result.passed) failed=$($result.failed)"
    Write-Host "result: $resultPath"
}
finally {
    Pop-Location
}
