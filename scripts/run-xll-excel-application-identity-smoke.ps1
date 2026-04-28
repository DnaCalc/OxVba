param(
    [string]$StagingManifest = "",
    [string]$OutputRoot = "target/xll-host-validation/excel-application-identity",
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
        $latestManifest = Get-ChildItem -Path "target/xll-host-validation/application_addin" -Filter "manifest.json" -Recurse -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTimeUtc -Descending |
            Select-Object -First 1
        if ($null -eq $latestManifest) {
            throw "No staged Application XLL manifest found. Run scripts/stage-xll-application-addin.ps1 first."
        }
        $StagingManifest = $latestManifest.FullName
    }

    $manifestPath = (Resolve-Path -LiteralPath $StagingManifest).Path
    $staging = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $artifactPath = [string]$staging.artifact_path
    if (-not (Test-Path -LiteralPath $artifactPath)) {
        throw "Staged XLL artifact is missing: $artifactPath"
    }
    $artifactPath = (Resolve-Path -LiteralPath $artifactPath).Path

    $runDir = Join-Path $OutputRoot $RunId
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null
    $resultPath = Join-Path $runDir "identity_result.json"
    $workbookPath = Join-Path $runDir "identity_smoke.xlsx"
    $xllTracePath = Join-Path $runDir "xll_trace.log"
    $hostGuardianLog = Join-Path $runDir "host_excel_dialog_guardian.log"
    $hostGuardianStop = Join-Path $runDir "host_excel_dialog_guardian.stop"
    $decoyGuardianLog = Join-Path $runDir "decoy_excel_dialog_guardian.log"
    $decoyGuardianStop = Join-Path $runDir "decoy_excel_dialog_guardian.stop"

    Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class OxVbaXllApplicationIdentityWin32User32Pid {
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@

    function Get-WindowProcessId {
        param([int]$Hwnd)
        [uint32]$windowPid = 0
        [void][OxVbaXllApplicationIdentityWin32User32Pid]::GetWindowThreadProcessId([IntPtr]::new($Hwnd), [ref]$windowPid)
        [int]$windowPid
    }

    function Start-ExcelDialogGuardian {
        param(
            [int]$ExcelPid,
            [string]$StopFile,
            [string]$LogFile
        )
        if ($DisableDialogGuardian) {
            return $null
        }
        Start-Process -FilePath "powershell" -WindowStyle Hidden -PassThru -ArgumentList @(
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            (Join-Path $PSScriptRoot "excel-dialog-guardian.ps1"),
            "-ExcelPid",
            ([string]$ExcelPid),
            "-StopFile",
            $StopFile,
            "-LogFile",
            $LogFile,
            "-PollMs",
            ([string]$DialogGuardianPollMs),
            "-MaxSeconds",
            ([string]$DialogGuardianMaxSeconds)
        )
    }

    function Stop-ExcelDialogGuardian {
        param(
            $Process,
            [string]$StopFile
        )
        if ($null -eq $Process) {
            return
        }
        try { New-Item -ItemType File -Force -Path $StopFile | Out-Null } catch {}
        try { Wait-Process -Id $Process.Id -Timeout 5 -ErrorAction SilentlyContinue } catch {}
        if (-not $Process.HasExited) {
            try { Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue } catch {}
        }
    }

    function Release-ComObjectQuietly {
        param($ComObject)
        if ($null -ne $ComObject) {
            try { [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($ComObject) } catch {}
        }
    }

    function Write-Result {
        param([hashtable]$Result)
        $Result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $resultPath -Encoding utf8
    }

    $startedAt = (Get-Date).ToUniversalTime().ToString("o")
    $result = [ordered]@{
        run_id = $RunId
        started_at = $startedAt
        ended_at = $null
        status = "not_run"
        staging_manifest = $manifestPath
        artifact_path = $artifactPath
        artifact_bytes = (Get-Item -LiteralPath $artifactPath).Length
        workbook_path = $workbookPath
        xll_trace = $xllTracePath
        xll_trace_exists = $false
        host_excel_version = $null
        host_excel_hwnd = $null
        host_excel_pid = $null
        decoy_excel_version = $null
        decoy_excel_hwnd = $null
        decoy_excel_pid = $null
        register_xll_return = $null
        observed_version = $null
        observed_hwnd = $null
        observed_hwnd_text = $null
        trace_host_identity_seen = $false
        trace_decoy_identity_seen_as_host = $false
        trace_matching_host_candidate_seen = $false
        trace_decoy_candidate_seen = $false
        passed = 0
        failed = 0
        error = $null
    }

    $hostExcel = $null
    $decoyExcel = $null
    $workbook = $null
    $decoyWorkbook = $null
    $worksheet = $null
    $hostGuardianProc = $null
    $decoyGuardianProc = $null
    $oldTraceEnv = [Environment]::GetEnvironmentVariable("OXVBA_XLL_TRACE", "Process")

    try {
        [Environment]::SetEnvironmentVariable("OXVBA_XLL_TRACE", (Join-Path (Get-Location) $xllTracePath), "Process")
        try {
            $hostExcel = New-Object -ComObject Excel.Application
            $decoyExcel = New-Object -ComObject Excel.Application
        } catch {
            $result.status = "excel_unavailable"
            $result.error = $_.Exception.Message
            if ($AllowUnavailable) {
                return
            }
            throw
        }

        $hostExcel.Visible = $false
        $hostExcel.DisplayAlerts = $false
        $decoyExcel.Visible = $false
        $decoyExcel.DisplayAlerts = $false

        $result.host_excel_version = [string]$hostExcel.Version
        $result.host_excel_hwnd = [int]$hostExcel.Hwnd
        $result.host_excel_pid = Get-WindowProcessId -Hwnd $result.host_excel_hwnd
        $result.decoy_excel_version = [string]$decoyExcel.Version
        $result.decoy_excel_hwnd = [int]$decoyExcel.Hwnd
        $result.decoy_excel_pid = Get-WindowProcessId -Hwnd $result.decoy_excel_hwnd

        if (($result.host_excel_pid -eq $result.decoy_excel_pid) -or ($result.host_excel_hwnd -eq $result.decoy_excel_hwnd)) {
            $result.status = "multi_instance_unavailable"
            $result.error = "Excel COM automation did not provide distinct host and decoy instances."
            if ($AllowUnavailable) {
                return
            }
            throw $result.error
        }

        $hostGuardianProc = Start-ExcelDialogGuardian -ExcelPid $result.host_excel_pid -StopFile $hostGuardianStop -LogFile $hostGuardianLog
        $decoyGuardianProc = Start-ExcelDialogGuardian -ExcelPid $result.decoy_excel_pid -StopFile $decoyGuardianStop -LogFile $decoyGuardianLog

        $decoyWorkbook = $decoyExcel.Workbooks.Add()
        $registered = [bool]$hostExcel.RegisterXLL($artifactPath)
        $result.register_xll_return = $registered
        if (-not $registered) {
            throw "Host Excel RegisterXLL returned False"
        }

        $workbook = $hostExcel.Workbooks.Add()
        $worksheet = $workbook.Worksheets.Item(1)
        $worksheet.Cells.Item(1, 1).Formula = "=ExcelVersion()"
        $worksheet.Cells.Item(2, 1).Formula = "=ExcelHwnd()"
        $hostExcel.CalculateFullRebuild()

        $versionValue = $worksheet.Cells.Item(1, 1).Value2
        $hwndValue = $worksheet.Cells.Item(2, 1).Value2
        $hwndText = [string]$worksheet.Cells.Item(2, 1).Text
        $result.observed_version = [string]$versionValue
        $result.observed_hwnd = [string]$hwndValue
        $result.observed_hwnd_text = $hwndText

        if ([string]$versionValue -eq $result.host_excel_version) {
            $result.passed += 1
        } else {
            $result.failed += 1
        }

        $observedHwndNumber = [int][double]$hwndValue
        if ($observedHwndNumber -eq $result.host_excel_hwnd) {
            $result.passed += 1
        } else {
            $result.failed += 1
        }
        if ($observedHwndNumber -ne $result.decoy_excel_hwnd) {
            $result.passed += 1
        } else {
            $result.failed += 1
        }

        $workbook.SaveAs((Join-Path (Get-Location) $workbookPath), 51)

        if (Test-Path -LiteralPath $xllTracePath) {
            $trace = Get-Content -LiteralPath $xllTracePath -Raw
            $hostIdentityText = "Excel.Application host identity hwnd=$($result.host_excel_hwnd) pid=$($result.host_excel_pid)"
            $decoyIdentityText = "Excel.Application host identity hwnd=$($result.decoy_excel_hwnd) pid=$($result.decoy_excel_pid)"
            $hostCandidatePattern = "Excel\.Application ROT candidate hwnd=$($result.host_excel_hwnd) pid=$($result.host_excel_pid) match=true"
            $decoyCandidatePattern = "Excel\.Application ROT candidate hwnd=$($result.decoy_excel_hwnd) pid=$($result.decoy_excel_pid) match=false"
            $result.trace_host_identity_seen = $trace.Contains($hostIdentityText)
            $result.trace_decoy_identity_seen_as_host = $trace.Contains($decoyIdentityText)
            $result.trace_matching_host_candidate_seen = [regex]::IsMatch($trace, $hostCandidatePattern)
            $result.trace_decoy_candidate_seen = [regex]::IsMatch($trace, $decoyCandidatePattern)

            if ($result.trace_host_identity_seen) {
                $result.passed += 1
            } else {
                $result.failed += 1
            }
            if (-not $result.trace_decoy_identity_seen_as_host) {
                $result.passed += 1
            } else {
                $result.failed += 1
            }
            if ($result.trace_matching_host_candidate_seen) {
                $result.passed += 1
            } else {
                $result.failed += 1
            }
            if ($result.trace_decoy_candidate_seen) {
                $result.passed += 1
            } else {
                $result.failed += 1
            }
        } else {
            $result.failed += 4
        }

        $result.status = if ($result.failed -eq 0) { "passed" } else { "failed" }
        if ($result.failed -ne 0) {
            throw "XLL host identity mismatches: $($result.failed)"
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
        }
        if ($decoyWorkbook -ne $null) {
            try { $decoyWorkbook.Close($false) | Out-Null } catch {}
        }
        Release-ComObjectQuietly -ComObject $worksheet
        Release-ComObjectQuietly -ComObject $workbook
        Release-ComObjectQuietly -ComObject $decoyWorkbook
        if ($hostExcel -ne $null) {
            try { $hostExcel.Quit() | Out-Null } catch {}
        }
        if ($decoyExcel -ne $null) {
            try { $decoyExcel.Quit() | Out-Null } catch {}
        }
        Release-ComObjectQuietly -ComObject $hostExcel
        Release-ComObjectQuietly -ComObject $decoyExcel
        Stop-ExcelDialogGuardian -Process $hostGuardianProc -StopFile $hostGuardianStop
        Stop-ExcelDialogGuardian -Process $decoyGuardianProc -StopFile $decoyGuardianStop
    }

    Write-Host "xll excel application identity smoke: $($result.status) passed=$($result.passed) failed=$($result.failed)"
    Write-Host "host hwnd=$($result.host_excel_hwnd) pid=$($result.host_excel_pid)"
    Write-Host "decoy hwnd=$($result.decoy_excel_hwnd) pid=$($result.decoy_excel_pid)"
    Write-Host "observed hwnd=$($result.observed_hwnd)"
    Write-Host "result: $resultPath"
}
finally {
    Pop-Location
}
