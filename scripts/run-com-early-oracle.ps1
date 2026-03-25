param(
    [string]$OutputRoot = "docs/evidence/conformance/oracle_captures",
    [switch]$KeepArtifacts,
    [switch]$DisableDialogGuardian,
    [int]$DialogGuardianPollMs = 250,
    [int]$DialogGuardianMaxSeconds = 1800
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not $IsWindows) {
        throw "COM early oracle runner is Windows-only"
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $stamp = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "com_early_oracle_$stamp"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null
    $artifactDir = Join-Path $runDir "artifacts"
    New-Item -ItemType Directory -Force -Path $artifactDir | Out-Null
    $dialogGuardianLog = Join-Path $runDir "excel_dialog_guardian.log"
    $dialogGuardianStop = Join-Path $runDir "excel_dialog_guardian.stop"
    $dialogGuardianProc = $null

    Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win32User32Pid {
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@

    function Get-WindowProcessId {
        param([int]$Hwnd)
        [uint32]$windowPid = 0
        [void][Win32User32Pid]::GetWindowThreadProcessId([IntPtr]::new($Hwnd), [ref]$windowPid)
        [int]$windowPid
    }

    function Invoke-ExcelScriptingDictionaryProbe {
        $excel = New-Object -ComObject Excel.Application
        $excel.Visible = $false
        $excel.DisplayAlerts = $false
        $excelVersion = $excel.Version
        $excelPid = Get-WindowProcessId -Hwnd ([int]$excel.Hwnd)
        if (-not $DisableDialogGuardian -and $excelPid -gt 0) {
            if (Test-Path $dialogGuardianStop) {
                Remove-Item -Force $dialogGuardianStop
            }
            $guardianScript = Join-Path $PSScriptRoot "excel-dialog-guardian.ps1"
            $guardianArgs = @(
                "-NoLogo",
                "-NoProfile",
                "-ExecutionPolicy", "Bypass",
                "-File", $guardianScript,
                "-ExcelPid", "$excelPid",
                "-StopFile", $dialogGuardianStop,
                "-LogFile", $dialogGuardianLog,
                "-PollMs", "$DialogGuardianPollMs",
                "-MaxSeconds", "$DialogGuardianMaxSeconds"
            )
            $dialogGuardianProc = Start-Process -FilePath "pwsh" -ArgumentList $guardianArgs -WindowStyle Hidden -PassThru
            Write-Host "excel-dialog-guardian: started (pid=$($dialogGuardianProc.Id), excel_pid=$excelPid)"
        }

        $wb = $null
        try {
            $wb = $excel.Workbooks.Add()
            [void]$wb.VBProject.References.AddFromGuid("{420B2830-E718-11CF-893D-00A0C9054228}", 1, 0)
            $module = $wb.VBProject.VBComponents.Add(1)
            $module.Name = "OracleModule"
            $code = @"
Public Function RunProbe()
    Dim obj As New Scripting.Dictionary
    Call obj.Add("a", 1)
    RunProbe = CStr(obj.Exists("a")) & "," & CStr(obj.Count)
End Function
"@
            $null = $module.CodeModule.AddFromString($code)
            $observed = [string]$excel.Run("RunProbe")
            @{
                status = "ok"
                observed = $observed
                excel_version = $excelVersion
                excel_pid = $excelPid
            }
        } catch {
            @{
                status = "error"
                observed = $_.Exception.Message
                excel_version = $excelVersion
                excel_pid = $excelPid
            }
        } finally {
            if ($wb -ne $null) {
                $wb.Close($false)
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
            }
            if ($dialogGuardianProc -ne $null) {
                New-Item -ItemType File -Path $dialogGuardianStop -Force | Out-Null
                Start-Sleep -Milliseconds 500
                if (-not $dialogGuardianProc.HasExited) {
                    Stop-Process -Id $dialogGuardianProc.Id -Force -ErrorAction SilentlyContinue
                }
                if (Test-Path $dialogGuardianStop) {
                    Remove-Item -Force $dialogGuardianStop -ErrorAction SilentlyContinue
                }
            }
            $excel.Quit()
            [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
        }
    }

    $oxLane = & "$PSScriptRoot/run-com-registered-early-bound.ps1" -RunId $stamp -NoLatest
    $oxStatus = if ($oxLane.status -eq "pass") { "ok" } else { "error" }
    $oxObserved = if ($oxStatus -eq "ok") { "True,1" } else { "lane-failed(exit=$($oxLane.exit_code))" }
    $oxNotes = "OxVba anchor: com_early_project_end_to_end::early_bound_project_executes_registered_scripting_dictionary_member_subset"

    $excelProbe = Invoke-ExcelScriptingDictionaryProbe
    $match = ($excelProbe.status -eq $oxStatus -and $excelProbe.observed -eq $oxObserved)

    $rows = @(
        [PSCustomObject]@{
            topic_id = "CCT-046"
            case_id = "CCT-046-A"
            scenario = "As New Scripting.Dictionary plus Add / Exists / Count"
            vba_status = $excelProbe.status
            vba_observed = $excelProbe.observed
            oxvba_status = $oxStatus
            oxvba_observed = $oxObserved
            match = if ($match) { "true" } else { "false" }
            notes = "$oxNotes; OxVba report: $($oxLane.report); OxVba log: $($oxLane.log)"
        }
    )

    $csvPath = Join-Path $runDir "results.csv"
    $rows | Export-Csv -Path $csvPath -NoTypeInformation

    $summaryPath = Join-Path $runDir "summary.md"
    $md = @()
    $md += "# COM Early Oracle Run"
    $md += ""
    $md += "- Timestamp (UTC): $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))"
    $md += "- Excel version: $($excelProbe.excel_version)"
    $md += "- Excel process id: $($excelProbe.excel_pid)"
    $md += "- Dialog guardian enabled: $([string](-not $DisableDialogGuardian))"
    if (-not $DisableDialogGuardian) {
        $md += "- Dialog guardian log: $dialogGuardianLog"
    }
    $md += "- Output CSV: $csvPath"
    $md += "- OxVba lane report: $($oxLane.report)"
    $md += "- OxVba lane log: $($oxLane.log)"
    $md += "- Total cases: 1"
    $md += "- Match count: $((@($rows | Where-Object { $_.match -eq 'true' })).Count)"
    $md += "- Mismatch count: $((@($rows | Where-Object { $_.match -ne 'true' })).Count)"
    $md += ""
    $md += "## Case Results"
    $md += "| Topic | Case | VBA | OxVba | Match | Notes |"
    $md += "|---|---|---|---|---|---|"
    foreach ($row in $rows) {
        $vbaCell = "$($row.vba_status): $($row.vba_observed)"
        $oxCell = "$($row.oxvba_status): $($row.oxvba_observed)"
        $md += "| $($row.topic_id) | $($row.case_id) | $vbaCell | $oxCell | $($row.match) | $($row.notes) |"
    }
    Set-Content -Path $summaryPath -Value ($md -join [Environment]::NewLine)

    Write-Host "com-early-oracle-run: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "csv=$csvPath"
    Write-Host "summary=$summaryPath"
} finally {
    if (-not $KeepArtifacts -and (Test-Path $artifactDir)) {
        Remove-Item -Recurse -Force -Path $artifactDir
    }
    Pop-Location
}
