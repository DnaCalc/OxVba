param(
    [string]$OutputRoot = "docs/evidence/conformance/oracle_captures",
    [string]$RunId = "",
    [switch]$NoArtifacts,
    [int]$ProbeTimeoutSeconds = 15
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not $IsWindows) {
        throw "COM TestEventServer unqualified broken-later oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    . "$PSScriptRoot/lib-com-testeventserver-alt-project.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-testeventserver-unqualified-broken-later-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "com-testeventserver-unqualified-broken-later-oracle" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "com_testeventserver_unqualified_broken_later_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    $generatedRoot = Join-Path $workspaceRoot "temp\generated\com_testeventserver_unqualified_broken_later\$resolvedRunId"
    $altProjectRoot = Join-Path $generatedRoot "OxVba.TestEventServerAlt"
    New-AltTestEventServerProject -WorkspaceRoot $workspaceRoot -DestinationRoot $altProjectRoot

    & (Join-Path $workspaceRoot "tools/OxVba.TestEventServer/register.ps1") -Configuration Debug -Scope CurrentUser
    & (Join-Path $altProjectRoot "register.ps1") -Configuration Debug -Scope CurrentUser

    $baseTypeLibPath = (Resolve-Path "tools/OxVba.TestEventServer/bin/Debug/net48/OxVba.TestEventServer.tlb").Path
    $altTypeLibPath = (Resolve-Path (Join-Path $altProjectRoot "bin/Debug/net48/OxVba.TestEventServerAlt.tlb")).Path

    $rows = New-Object System.Collections.Generic.List[object]
    $vbaDialogHandlerScriptPath = (Resolve-Path (Join-Path $PSScriptRoot "excel-vbe-dialog-handler.ps1")).Path
    $probeScriptPath = Join-Path $runDir "_unqualified_broken_later_probe.ps1"
    $probeScript = @'
param(
    [string]$FirstTypeLibPath,
    [string]$SecondTypeLibPath,
    [string]$StatePath,
    [string]$VbaDialogHandlerScriptPath,
    [string]$VbaDialogHandlerLogPath,
    [int]$RunTimeoutSeconds = 15
)

$ErrorActionPreference = "Stop"

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class ProbeWin32Pid {
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@

function Get-WindowProcessId {
    param([int]$Hwnd)
    [uint32]$windowPid = 0
    [void][ProbeWin32Pid]::GetWindowThreadProcessId([IntPtr]::new($Hwnd), [ref]$windowPid)
    [int]$windowPid
}

$root = Join-Path $env:TEMP ("oxvba_unqualified_broken_later_" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $root | Out-Null
$firstCopy = Join-Path $root ([System.IO.Path]::GetFileName($FirstTypeLibPath))
$secondCopy = Join-Path $root ([System.IO.Path]::GetFileName($SecondTypeLibPath))
$workbookPath = Join-Path $root "probe.xlsm"
$vbaDialogHandlerStop = Join-Path $root "_vba_dialog_handler.stop"
$deadlineFile = Join-Path $root "_run_deadline.txt"
Copy-Item $FirstTypeLibPath $firstCopy -Force
Copy-Item $SecondTypeLibPath $secondCopy -Force
$code = "Public Function RunProbe()`n    Dim obj As TestEventServer`n    Set obj = New TestEventServer`n    RunProbe = obj.Ping()`nEnd Function`n"

$excel = $null
$wb = $null
$reopened = $null
$vbaDialogHandler = $null
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excelPid = Get-WindowProcessId -Hwnd ([int]$excel.Hwnd)
    if ($excelPid -gt 0 -and (Test-Path $VbaDialogHandlerScriptPath)) {
        if (Test-Path $VbaDialogHandlerLogPath) {
            Remove-Item -Force -Path $VbaDialogHandlerLogPath
        }
        $vbaDialogHandler = Start-Process `
            -FilePath (Get-Command pwsh).Source `
            -ArgumentList @(
                "-NoProfile",
                "-NonInteractive",
                "-File",
                $VbaDialogHandlerScriptPath,
                $excelPid,
                $vbaDialogHandlerStop,
                $VbaDialogHandlerLogPath,
                $deadlineFile,
                200
            ) `
            -PassThru `
            -WindowStyle Hidden
    }

    $wb = $excel.Workbooks.Add()
    [void]$wb.VBProject.References.AddFromFile($firstCopy)
    [void]$wb.VBProject.References.AddFromFile($secondCopy)
    $mod = $wb.VBProject.VBComponents.Add(1)
    $mod.Name = "MainModule"
    [void]$mod.CodeModule.AddFromString($code)
    $wb.SaveAs($workbookPath, 52)
    $wb.Close($false)
    [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
    $wb = $null

    Rename-Item $secondCopy ($secondCopy + ".missing")

    $reopened = $excel.Workbooks.Open($workbookPath)
    $refs = @(
        $reopened.VBProject.References |
            Where-Object {
                $_.Guid -in @(
                    "{E2A30001-0001-0001-0001-000000000001}",
                    "{E2A30001-0001-0001-0001-000000000101}"
                )
            } |
            ForEach-Object { "name={0};guid={1};broken={2}" -f $_.Name, $_.Guid, [string]$_.IsBroken }
    )
    @{ stage = "reopened"; refs = $refs } | ConvertTo-Json -Compress | Set-Content -Path $StatePath

    try {
        [DateTime]::UtcNow.AddSeconds($RunTimeoutSeconds).Ticks | Set-Content -Path $deadlineFile
        $result = [string]$excel.Run("RunProbe")
        @{ stage = "completed"; refs = $refs; run = $result; handler_log = $VbaDialogHandlerLogPath } | ConvertTo-Json -Compress | Set-Content -Path $StatePath
    } catch {
        @{ stage = "run_error"; refs = $refs; run_error = $_.Exception.Message; handler_log = $VbaDialogHandlerLogPath } | ConvertTo-Json -Compress | Set-Content -Path $StatePath
    } finally {
        if (Test-Path $deadlineFile) {
            Remove-Item -Force -Path $deadlineFile
        }
    }
} finally {
    Set-Content -Path $vbaDialogHandlerStop -Value "stop" -Encoding UTF8
    if ($vbaDialogHandler -ne $null) {
        $null = $vbaDialogHandler.WaitForExit(2000)
        if (-not $vbaDialogHandler.HasExited) {
            Stop-Process -Id $vbaDialogHandler.Id -Force -ErrorAction SilentlyContinue
        }
    }
    if ($reopened -ne $null) {
        $reopened.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($reopened)
    }
    if ($wb -ne $null) {
        $wb.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
    }
    if ($excel -ne $null) {
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    }
}
'@
    Set-Content -Path $probeScriptPath -Value $probeScript -Encoding UTF8

    function Add-Row {
        param(
            [string]$CaseId,
            [string]$Scenario,
            [string]$VbaStatus,
            [string]$VbaObserved,
            [string]$OxVbaStatus,
            [string]$OxVbaObserved,
            [string]$Match,
            [string]$Notes
        )

        $rows.Add([PSCustomObject]@{
                topic_id       = "CCT-043"
                case_id        = $CaseId
                scenario       = $Scenario
                vba_status     = $VbaStatus
                vba_observed   = $VbaObserved
                oxvba_status   = $OxVbaStatus
                oxvba_observed = $OxVbaObserved
                match          = $Match
                notes          = $Notes
            }) | Out-Null
    }

    function Get-HandlerLogMetadata {
        param([string]$Path)

        $classification = ""
        $observed = "unknown"
        $waitDeadline = (Get-Date).AddSeconds(2)
        while ((Get-Date) -lt $waitDeadline) {
            if ((Test-Path $Path) -and (Get-Item $Path).Length -gt 0) {
                break
            }
            Start-Sleep -Milliseconds 100
        }
        if (Test-Path $Path) {
            $lines = Get-Content $Path
            $signalLines = @(
                $lines | Where-Object {
                    $_ -match "observed window=" -or
                    $_ -match "deadline exceeded"
                }
            )
            if ($signalLines.Count -gt 0) {
                $observed = "true"
                $classification = "ui-blocked-or-compile-failure"
            } elseif ($lines.Count -gt 0) {
                $observed = "false"
            }
        }

        @{
            observed = $observed
            classification = $classification
        }
    }

    function Invoke-UnqualifiedBrokenLaterProbe {
        param(
            [string]$CaseId,
            [string]$FirstTypeLibPath,
            [string]$SecondTypeLibPath
        )

        $baselineExcelPids = @(Get-Process EXCEL -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
        $statePath = Join-Path $runDir ($CaseId + ".vba-state.json")
        $stdoutPath = Join-Path $runDir ($CaseId + ".probe.stdout.txt")
        $stderrPath = Join-Path $runDir ($CaseId + ".probe.stderr.txt")
        $handlerLogPath = Join-Path $runDir ($CaseId + ".vba-dialog-handler.log")
        if (Test-Path $statePath) {
            Remove-Item -Force -Path $statePath
        }
        if (Test-Path $handlerLogPath) {
            Remove-Item -Force -Path $handlerLogPath
        }
        $probeProcess = Start-Process `
            -FilePath (Get-Command pwsh).Source `
            -ArgumentList @(
                "-NoProfile",
                "-NonInteractive",
                "-File",
                $probeScriptPath,
                $FirstTypeLibPath,
                $SecondTypeLibPath,
                $statePath,
                $vbaDialogHandlerScriptPath,
                $handlerLogPath,
                $ProbeTimeoutSeconds
            ) `
            -RedirectStandardOutput $stdoutPath `
            -RedirectStandardError $stderrPath `
            -PassThru `
            -WindowStyle Hidden

        $completed = $probeProcess.WaitForExit($ProbeTimeoutSeconds * 1000)
        $state = $null
        if (Test-Path $statePath) {
            $state = Get-Content $statePath -Raw | ConvertFrom-Json
        }
        if (-not $completed) {
            Stop-Process -Id $probeProcess.Id -Force -ErrorAction SilentlyContinue
            $newExcelPids = @(Get-Process EXCEL -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
            $orphanedPids = $newExcelPids | Where-Object { $_ -notin $baselineExcelPids }
            foreach ($orphanedPid in $orphanedPids) {
                Stop-Process -Id $orphanedPid -Force -ErrorAction SilentlyContinue
            }
            $handlerLogMeta = Get-HandlerLogMetadata -Path $handlerLogPath
            if ($state -and $state.stage -eq "completed") {
                return @{
                    status          = "ok"
                    observed        = [string]$state.run
                    refs            = ($state.refs -join "|")
                    stage           = [string]$state.stage
                    modal_observed  = if ($handlerLogMeta.observed -ne "unknown") { $handlerLogMeta.observed } else { "false" }
                    probe_exit_code = "cleanup-timeout"
                    handler_log     = $handlerLogPath
                    handler_signal  = "cleanup-timeout-after-completed-state"
                }
            }
            return @{
                status          = "timeout"
                observed        = "execution-did-not-return-within-${ProbeTimeoutSeconds}s"
                refs            = if ($state) { ($state.refs -join "|") } else { "" }
                stage           = if ($state) { [string]$state.stage } else { "" }
                modal_observed  = $handlerLogMeta.observed
                probe_exit_code = ""
                handler_log     = $handlerLogPath
                handler_signal  = $handlerLogMeta.classification
            }
        }

        $probeExitCode = [string]$probeProcess.ExitCode
        $handlerLogMeta = Get-HandlerLogMetadata -Path $handlerLogPath
        if ($null -eq $state) {
            return @{
                status          = "error"
                observed        = "no-state-captured(exit=$probeExitCode)"
                refs            = ""
                stage           = ""
                modal_observed  = "unknown"
                probe_exit_code = $probeExitCode
                handler_log     = $handlerLogPath
                handler_signal  = $handlerLogMeta.classification
            }
        }
        if ($state.stage -eq "completed") {
            return @{
                status          = "ok"
                observed        = [string]$state.run
                refs            = ($state.refs -join "|")
                stage           = [string]$state.stage
                modal_observed  = if ($handlerLogMeta.observed -ne "unknown") { $handlerLogMeta.observed } else { "false" }
                probe_exit_code = $probeExitCode
                handler_log     = $handlerLogPath
                handler_signal  = $handlerLogMeta.classification
            }
        }
        return @{
            status          = "error"
            observed        = [string]$state.run_error
            refs            = ($state.refs -join "|")
            stage           = [string]$state.stage
            modal_observed  = if ($handlerLogMeta.observed -ne "unknown") { $handlerLogMeta.observed } else { "false" }
            probe_exit_code = $probeExitCode
            handler_log     = $handlerLogPath
            handler_signal  = $handlerLogMeta.classification
        }
    }

    $cases = @(
        @{
            case_id = "CCT-043-TES-BROKEN-LATER-001"
            scenario = "Saved workbook with base valid then alt broken after save; unqualified TestEventServer still selects first valid base reference"
            first = $baseTypeLibPath
            second = $altTypeLibPath
            command = @(
                "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end",
                "early_bound_loaded_basproj_valid_base_then_broken_alt_prefers_base_for_unqualified_testeventserver",
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            )
            expected_vba = "42"
            expected_prog_id = "OxVba.TestEventServer"
        }
        @{
            case_id = "CCT-043-TES-BROKEN-LATER-002"
            scenario = "Saved workbook with alt valid then base broken after save; unqualified TestEventServer still selects first valid alt reference"
            first = $altTypeLibPath
            second = $baseTypeLibPath
            command = @(
                "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end",
                "early_bound_loaded_basproj_valid_alt_then_broken_base_prefers_alt_for_unqualified_testeventserver",
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            )
            expected_vba = "84"
            expected_prog_id = "OxVba.TestEventServerAlt"
        }
    )

    foreach ($case in $cases) {
        $probe = Invoke-UnqualifiedBrokenLaterProbe `
            -CaseId $case.case_id `
            -FirstTypeLibPath $case.first `
            -SecondTypeLibPath $case.second
        $logPath = Join-Path $runDir ($case.case_id + ".log.txt")
        $cmdText = "cargo " + ($case.command -join " ")
        $null = & cargo @($case.command) 2>&1 | Tee-Object -FilePath $logPath
        $exitCode = $LASTEXITCODE
        $oxStatus = if ($exitCode -eq 0) { "ok" } else { "error" }
        $oxObserved = if ($exitCode -eq 0) {
            "compile-selected-progid=$($case.expected_prog_id)"
        } else {
            "lane-failed(exit=$exitCode)"
        }
        $match = if (
            $probe.status -eq "ok" `
                -and $exitCode -eq 0 `
                -and $probe.observed -eq $case.expected_vba
        ) { "true" } else { "false" }
        Add-Row `
            -CaseId $case.case_id `
            -Scenario $case.scenario `
            -VbaStatus $probe.status `
            -VbaObserved $probe.observed `
            -OxVbaStatus $oxStatus `
            -OxVbaObserved $oxObserved `
            -Match $match `
            -Notes (
                "Excel stage=" + $probe.stage +
                "; refs=" + $probe.refs +
                "; modal_observed=" + $probe.modal_observed +
                "; handler_signal=" + $probe.handler_signal +
                "; handler_log=" + $probe.handler_log +
                "; probe_exit_code=" + $probe.probe_exit_code +
                "; OxVba anchor command=" + $cmdText +
                "; log=" + $logPath
            )
    }

    $csvPath = Join-Path $runDir "results.csv"
    $summaryPath = Join-Path $runDir "summary.md"
    $rows | Export-Csv -Path $csvPath -NoTypeInformation

    $summary = @(
        "# COM TestEventServer Unqualified Broken-Later Oracle Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Base TypeLib: $baseTypeLibPath",
        "- Alt TypeLib: $altTypeLibPath",
        "- Probe timeout seconds: $ProbeTimeoutSeconds",
        "- Output CSV: $csvPath",
        "- Excel popup handling note: this runner uses a harness-side VBE dialog helper to keep hidden Excel automation bounded. Popup handling is treated as automation hygiene and coarse failure classification, not user-facing parity.",
        "",
        "- Total cases: $($rows.Count)",
        "- Match count: $(($rows | Where-Object { $_.match -eq 'true' }).Count)",
        "- Mismatch count: $(($rows | Where-Object { $_.match -ne 'true' }).Count)",
        "",
        "## Case Results",
        "| Topic | Case | VBA | OxVba | Match | Notes |",
        "|---|---|---|---|---|---|"
    )
    foreach ($row in $rows) {
        $summary += "| $($row.topic_id) | $($row.case_id) | $($row.vba_status): $($row.vba_observed) | $($row.oxvba_status): $($row.oxvba_observed) | $($row.match) | $($row.notes) |"
    }
    Set-Content -Path $summaryPath -Value ($summary -join [Environment]::NewLine)

    Write-Host "com-testeventserver-unqualified-broken-later-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "csv=$csvPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
