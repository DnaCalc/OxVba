param(
    [string]$StagingManifest = "",
    [string]$OutputRoot = "target/xll-host-validation/excel-load",
    [string]$RunId = "",
    [ValidateSet("RegisterXLL", "AddIns")]
    [string]$LoadMethod = "RegisterXLL",
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
    $staging = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $artifactPath = [string]$staging.artifact_path
    if (-not (Test-Path -LiteralPath $artifactPath)) {
        throw "Staged XLL artifact is missing: $artifactPath"
    }

    $runDir = Join-Path $OutputRoot $RunId
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null
    $resultPath = Join-Path $runDir "excel_load_result.json"
    $guardianLog = Join-Path $runDir "excel_dialog_guardian.log"
    $guardianStop = Join-Path $runDir "excel_dialog_guardian.stop"
    $xllTracePath = Join-Path $runDir "xll_trace.log"

    Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class OxVbaXllWin32User32Pid {
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@

    function Get-WindowProcessId {
        param([int]$Hwnd)
        [uint32]$windowPid = 0
        [void][OxVbaXllWin32User32Pid]::GetWindowThreadProcessId([IntPtr]::new($Hwnd), [ref]$windowPid)
        [int]$windowPid
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
        artifact_path = (Resolve-Path -LiteralPath $artifactPath).Path
        artifact_bytes = (Get-Item -LiteralPath $artifactPath).Length
        excel_version = $null
        excel_build = $null
        excel_operating_system = $null
        excel_path = $null
        excel_pid = $null
        load_method = $LoadMethod
        register_xll_return = $null
        addin_full_name = $null
        addin_installed_after_load = $null
        addin_installed_after_unload = $null
        error = $null
        guardian_log = $guardianLog
        xll_trace = $xllTracePath
        xll_trace_exists = $false
    }

    $excel = $null
    $addin = $null
    $guardianProc = $null
    $oldTraceEnv = [Environment]::GetEnvironmentVariable("OXVBA_XLL_TRACE", "Process")
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

        if ($LoadMethod -eq "RegisterXLL") {
            $registered = [bool]$excel.RegisterXLL($artifactPath)
            $result.register_xll_return = $registered
            if (-not $registered) {
                throw "Excel RegisterXLL returned False"
            }
            $result.status = "registered_and_excel_quit"
        } else {
            $addin = $excel.AddIns.Add($artifactPath, $false)
            $addin.Installed = $true
            $result.addin_full_name = [string]$addin.FullName
            $result.addin_installed_after_load = [bool]$addin.Installed

            $addin.Installed = $false
            $result.addin_installed_after_unload = [bool]$addin.Installed
            $result.status = "loaded_and_unloaded"
        }
    } catch {
        $result.status = "failed"
        $result.error = $_.Exception.Message
        throw
    } finally {
        $result.ended_at = (Get-Date).ToUniversalTime().ToString("o")
        $result.xll_trace_exists = Test-Path -LiteralPath $xllTracePath
        Write-Result -Result $result
        [Environment]::SetEnvironmentVariable("OXVBA_XLL_TRACE", $oldTraceEnv, "Process")

        if ($addin -ne $null) {
            try { [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($addin) } catch {}
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

    Write-Host "xll excel load smoke: $($result.status)"
    Write-Host "result: $resultPath"
}
finally {
    Pop-Location
}
