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
        throw "COM TestEventServer mixed broken-reference oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-testeventserver-mixed-broken-reference-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "com-testeventserver-mixed-broken-reference-oracle" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "com_testeventserver_mixed_broken_reference_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    $baseTypeLibPath = (Resolve-Path "tools/OxVba.TestEventServer/bin/Debug/net48/OxVba.TestEventServer.tlb").Path
    $altTypeLibPath = (Get-ChildItem -Path "temp\generated\com_testeventserver_reference_order" -Recurse -Filter "OxVba.TestEventServerAlt.tlb" |
        Sort-Object `
            @{ Expression = "LastWriteTimeUtc"; Descending = $true }, `
            @{ Expression = "FullName"; Descending = $false } |
        Select-Object -First 1 -ExpandProperty FullName)
    if (-not $altTypeLibPath) {
        throw "alt TestEventServer typelib not found under temp\\generated\\com_testeventserver_reference_order"
    }

    $rows = New-Object System.Collections.Generic.List[object]
    $vbaDialogHandlerScriptPath = Join-Path $runDir "_vba_dialog_handler.ps1"
    $vbaDialogHandlerScript = @'
param([int]$ExcelPid, $StopFile, $LogFile, $DeadlineFile, [int]$PollMs = 200)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class VbeWin32 {
    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool PostMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
    public const uint BM_CLICK = 0x00F5;
    public const uint WM_CLOSE = 0x0010;
}
"@

function Write-Log {
    param([string]$Message)
    Add-Content -Path $LogFile -Value "$(Get-Date -Format o) $Message" -Encoding UTF8
}

function Get-DescendantNamesByControlType {
    param(
        [System.Windows.Automation.AutomationElement]$Root,
        [System.Windows.Automation.ControlType]$ControlType
    )

    $names = @()
    try {
        $cond = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            $ControlType
        )
        $items = $Root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond)
        for ($i = 0; $i -lt $items.Count; $i++) {
            $name = [string]$items.Item($i).Current.Name
            if (-not [string]::IsNullOrWhiteSpace($name)) {
                $names += $name.Trim()
            }
        }
    } catch {
        # Stale UIA trees are expected; caller retries on next poll.
    }
    $names
}

function Try-ClickDescendantButton {
    param(
        [System.Windows.Automation.AutomationElement]$Root,
        [string[]]$PreferredNames
    )

    function Invoke-AutomationElement {
        param([System.Windows.Automation.AutomationElement]$Element)

        try {
            $pattern = $Element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
            if ($pattern -ne $null) {
                $pattern.Invoke()
                return $true
            }
        } catch {
            # Fall through to legacy/default-action patterns.
        }

        try {
            $pattern = $Element.GetCurrentPattern([System.Windows.Automation.LegacyIAccessiblePattern]::Pattern)
            if ($pattern -ne $null) {
                $pattern.DoDefaultAction()
                return $true
            }
        } catch {
            # Fall through to hwnd click.
        }

        try {
            $buttonHwnd = [IntPtr]::new($Element.Current.NativeWindowHandle)
            if ($buttonHwnd -ne [IntPtr]::Zero) {
                [void][VbeWin32]::PostMessage($buttonHwnd, [VbeWin32]::BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero)
                return $true
            }
        } catch {
            # No invokable pattern or handle.
        }

        $false
    }

    $btnCond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Button
    )
    $buttons = $Root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $btnCond)
    foreach ($preferredName in $PreferredNames) {
        for ($i = 0; $i -lt $buttons.Count; $i++) {
            $button = $buttons.Item($i)
            $buttonName = ""
            try {
                $buttonName = [string]$button.Current.Name
            } catch {
                continue
            }
            if ($buttonName -ne $preferredName) {
                continue
            }
            if (Invoke-AutomationElement -Element $button) {
                Write-Log "clicked button '$buttonName'"
                return $true
            }
        }
    }
    $false
}

Write-Log "start excel_pid=$ExcelPid poll_ms=$PollMs stop_file=$StopFile"
$root = [System.Windows.Automation.AutomationElement]::RootElement

while ($true) {
    if (Test-Path $StopFile) {
        Write-Log "stop file observed"
        break
    }
    if (-not (Get-Process -Id $ExcelPid -ErrorAction SilentlyContinue)) {
        Write-Log "excel exited"
        break
    }

    if ($DeadlineFile -and (Test-Path $DeadlineFile)) {
        try {
            $deadlineTicks = [long](Get-Content $DeadlineFile -Raw).Trim()
            if ([DateTime]::UtcNow.Ticks -gt $deadlineTicks) {
                Write-Log "deadline exceeded; killing excel pid=$ExcelPid"
                Stop-Process -Id $ExcelPid -Force -ErrorAction SilentlyContinue
                break
            }
        } catch {
            Write-Log "deadline read error: $($_.Exception.Message)"
        }
    }

    try {
        $pidCond = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
            $ExcelPid
        )
        $winCond = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            [System.Windows.Automation.ControlType]::Window
        )
        $combined = New-Object System.Windows.Automation.AndCondition($pidCond, $winCond)
        $windows = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $combined)
        for ($i = 0; $i -lt $windows.Count; $i++) {
            $window = $windows.Item($i)
            $title = ""
            try {
                $title = [string]$window.Current.Name
            } catch {
                continue
            }
            if ($title -notmatch "Microsoft Visual Basic") {
                continue
            }

            $textNames = @(Get-DescendantNamesByControlType -Root $window -ControlType ([System.Windows.Automation.ControlType]::Text))
            $buttonNames = @(Get-DescendantNamesByControlType -Root $window -ControlType ([System.Windows.Automation.ControlType]::Button))
            Write-Log "observed window='$title' texts='$($textNames -join "|")' buttons='$($buttonNames -join "|")'"

            $clickedAction = $false
            if ($buttonNames.Count -gt 0) {
                $clickedAction = Try-ClickDescendantButton -Root $window -PreferredNames @("Close", "OK", "End")
                if ($clickedAction) {
                    Start-Sleep -Milliseconds 500
                }
            }

            $trueCond = [System.Windows.Automation.Condition]::TrueCondition
            $children = $window.FindAll([System.Windows.Automation.TreeScope]::Children, $trueCond)
            for ($j = 0; $j -lt $children.Count; $j++) {
                $child = $children.Item($j)
                $childName = ""
                try {
                    $childName = [string]$child.Current.Name
                } catch {
                    continue
                }
                if ($childName -ne "Microsoft Visual Basic for Applications") {
                    continue
                }

                $btnCond = New-Object System.Windows.Automation.PropertyCondition(
                    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
                    [System.Windows.Automation.ControlType]::Button
                )
                $buttons = $child.FindAll([System.Windows.Automation.TreeScope]::Children, $btnCond)
                for ($k = 0; $k -lt $buttons.Count; $k++) {
                    $button = $buttons.Item($k)
                    $buttonName = ""
                    try {
                        $buttonName = [string]$button.Current.Name
                    } catch {
                        continue
                    }
                    if ($buttonName -ne "OK") {
                        continue
                    }
                    $buttonHwnd = [IntPtr]::new($button.Current.NativeWindowHandle)
                    [void][VbeWin32]::PostMessage($buttonHwnd, [VbeWin32]::BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero)
                    Write-Log "clicked compile-error OK: window='$title'"
                    $clickedAction = $true
                    break
                }
                break
            }

            if ($clickedAction) {
                Start-Sleep -Milliseconds 500
            }

            if ($clickedAction -or $title -match "\[break\]" -or $title -match "\(Code\)") {
                $vbeHwnd = [IntPtr]::new($window.Current.NativeWindowHandle)
                if ($vbeHwnd -ne [IntPtr]::Zero) {
                    [void][VbeWin32]::PostMessage($vbeHwnd, [VbeWin32]::WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
                    Write-Log "sent WM_CLOSE to VBE: window='$title'"
                }
            }
        }
    } catch {
        Write-Log "poll error: $($_.Exception.Message)"
    }

    Start-Sleep -Milliseconds $PollMs
}

Write-Log "exit"
'@
    Set-Content -Path $vbaDialogHandlerScriptPath -Value $vbaDialogHandlerScript -Encoding UTF8
    $probeScriptPath = Join-Path $runDir "_mixed_broken_reference_probe.ps1"
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

$root = Join-Path $env:TEMP ("oxvba_mixed_broken_ref_" + [guid]::NewGuid().ToString("N"))
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

    Rename-Item $firstCopy ($firstCopy + ".missing")

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
                topic_id        = "CCT-043"
                case_id         = $CaseId
                scenario        = $Scenario
                vba_status      = $VbaStatus
                vba_observed    = $VbaObserved
                oxvba_status    = $OxVbaStatus
                oxvba_observed  = $OxVbaObserved
                match           = $Match
                notes           = $Notes
            }) | Out-Null
    }

    function Invoke-MixedBrokenReferenceProbe {
        param(
            [string]$CaseId,
            [string]$FirstTypeLibPath,
            [string]$SecondTypeLibPath
        )

        function Get-HandlerLogMetadata {
            param([string]$Path)

            $summary = ""
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
                        $_ -match "clicked compile-error OK" -or
                        $_ -match "clicked button " -or
                        $_ -match "sent WM_CLOSE to VBE" -or
                        $_ -match "deadline exceeded"
                    }
                )
                if ($signalLines.Count -gt 0) {
                    $observed = "true"
                    $summary = ($signalLines -join " || ")
                } elseif ($lines.Count -gt 0) {
                    $observed = "false"
                }
            }

            @{
                observed = $observed
                summary = $summary
            }
        }

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
            $orphanedWindowTitles = @(
                Get-Process -Id $orphanedPids -ErrorAction SilentlyContinue |
                    Where-Object { -not [string]::IsNullOrWhiteSpace($_.MainWindowTitle) } |
                    ForEach-Object { $_.MainWindowTitle }
            )
            foreach ($orphanedPid in $orphanedPids) {
                Stop-Process -Id $orphanedPid -Force -ErrorAction SilentlyContinue
            }
            $handlerLogMeta = Get-HandlerLogMetadata -Path $handlerLogPath
            return @{
                status         = "timeout"
                observed       = "execution-did-not-return-within-${ProbeTimeoutSeconds}s"
                refs           = if ($state) { ($state.refs -join "|") } else { "" }
                stage          = if ($state) { [string]$state.stage } else { "" }
                modal_observed = if ($handlerLogMeta.observed -ne "unknown") { $handlerLogMeta.observed } elseif ($state -and $state.stage -eq "reopened") { "possible" } else { "unknown" }
                probe_exit_code = ""
                window_titles  = ($orphanedWindowTitles -join "|")
                handler_log    = $handlerLogPath
                handler_signal = $handlerLogMeta.summary
            }
        }

        $probeExitCode = [string]$probeProcess.ExitCode
        $handlerLogMeta = Get-HandlerLogMetadata -Path $handlerLogPath
        if ($null -eq $state) {
            return @{
                status         = "error"
                observed       = "no-state-captured(exit=$probeExitCode)"
                refs           = ""
                stage          = ""
                modal_observed = "unknown"
                probe_exit_code = $probeExitCode
                window_titles  = ""
                handler_log    = $handlerLogPath
                handler_signal = $handlerLogMeta.summary
            }
        }
        if ($state.stage -eq "completed") {
            return @{
                status         = "ok"
                observed       = [string]$state.run
                refs           = ($state.refs -join "|")
                stage          = [string]$state.stage
                modal_observed = if ($handlerLogMeta.observed -ne "unknown") { $handlerLogMeta.observed } else { "false" }
                probe_exit_code = $probeExitCode
                window_titles  = ""
                handler_log    = $handlerLogPath
                handler_signal = $handlerLogMeta.summary
            }
        }
        return @{
            status         = "error"
            observed       = [string]$state.run_error
            refs           = ($state.refs -join "|")
            stage          = [string]$state.stage
            modal_observed = if ($handlerLogMeta.observed -ne "unknown") { $handlerLogMeta.observed } else { "false" }
            probe_exit_code = $probeExitCode
            window_titles  = ""
            handler_log    = $handlerLogPath
            handler_signal = $handlerLogMeta.summary
        }
    }

    $cases = @(
        @{
            case_id = "CCT-043-TES-MIXED-001"
            scenario = "Saved workbook with base then alt references; first typelib removed before reopen"
            first = $baseTypeLibPath
            second = $altTypeLibPath
            command = @(
                "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end",
                "early_bound_loaded_basproj_mixed_broken_base_then_valid_alt_reports_unresolved_importlib",
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            )
            expected_ox = "PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"
        }
        @{
            case_id = "CCT-043-TES-MIXED-002"
            scenario = "Saved workbook with alt then base references; first typelib removed before reopen"
            first = $altTypeLibPath
            second = $baseTypeLibPath
            command = @(
                "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end",
                "early_bound_loaded_basproj_mixed_broken_alt_then_valid_base_reports_unresolved_importlib",
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            )
            expected_ox = "PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"
        }
    )

    foreach ($case in $cases) {
        $probe = Invoke-MixedBrokenReferenceProbe -CaseId $case.case_id -FirstTypeLibPath $case.first -SecondTypeLibPath $case.second
        $logPath = Join-Path $runDir ($case.case_id + ".log.txt")
        $cmdText = "cargo " + ($case.command -join " ")
        $null = & cargo @($case.command) 2>&1 | Tee-Object -FilePath $logPath
        $exitCode = $LASTEXITCODE
        $oxStatus = if ($exitCode -eq 0) { "ok" } else { "error" }
        $oxObserved = if ($exitCode -eq 0) { $case.expected_ox } else { "lane-failed(exit=$exitCode)" }
        $match = if (
            $probe.status -eq "ok" `
                -and $exitCode -eq 0 `
                -and $probe.observed -eq $case.expected_ox
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
                "; window_titles=" + $probe.window_titles +
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
        "# COM TestEventServer Mixed Broken Reference Oracle Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Base TypeLib: $baseTypeLibPath",
        "- Alt TypeLib: $altTypeLibPath",
        "- Probe timeout seconds: $ProbeTimeoutSeconds",
        "- Output CSV: $csvPath",
        "- Modal inspection note: this runner now starts a VBE/UIAutomation handler per Excel probe, records observed Microsoft Visual Basic windows/buttons/text, attempts to click compile-error OK when present, and closes the VBE code window to turn blocked UI state into a bounded `Application.Run` result where possible.",
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
    Set-Content -Path $summaryPath -Value ($summary -join "`n")

    Write-Host "com-testeventserver-mixed-broken-reference-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "results=$csvPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
