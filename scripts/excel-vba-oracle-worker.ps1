param(
    [Parameter(Mandatory = $true)][string]$RunId,
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [Parameter(Mandatory = $true)][string]$OwnershipFile,
    [Parameter(Mandatory = $true)][string]$HelperOwnershipFile,
    [Parameter(Mandatory = $true)][string]$ContainmentReadyFile,
    [Parameter(Mandatory = $true)][string]$ContainmentToken,
    [ValidateRange(5, 600)][int]$CaseTimeoutSeconds = 90,
    [string]$DiagnosticCaseId = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "excel-vba-oracle-contract.ps1")
. (Join-Path $PSScriptRoot "excel-vba-oracle-job.ps1")

if (-not ([System.Management.Automation.PSTypeName]'ExcelOracleNativeMethods').Type) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class ExcelOracleNativeMethods
{
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

    [DllImport("oleacc.dll")]
    private static extern int AccessibleObjectFromWindow(
        IntPtr hwnd,
        uint objectId,
        ref Guid interfaceId,
        [MarshalAs(UnmanagedType.Interface)] out object nativeObject);

    public static object GetNativeObjectFromWindow(IntPtr hwnd)
    {
        const uint OBJID_NATIVEOM = 0xFFFFFFF0;
        Guid dispatch = new Guid("00020400-0000-0000-C000-000000000046");
        object nativeObject;
        int hr = AccessibleObjectFromWindow(hwnd, OBJID_NATIVEOM, ref dispatch, out nativeObject);
        if (hr < 0) Marshal.ThrowExceptionForHR(hr);
        return nativeObject;
    }
}
'@
}

function Wait-ContainmentAuthority {
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $ContainmentReadyFile) {
            try {
                $ready = Get-Content -Raw -LiteralPath $ContainmentReadyFile | ConvertFrom-Json
                if ([string]$ready.schema -ne "oxvba.excel-vba-oracle-containment-ready.v1" -or
                    [string]$ready.run_id -ne $RunId -or
                    [string]$ready.containment_token -ne $ContainmentToken -or
                    [int]$ready.worker_pid -ne $PID -or
                    $ready.worker_job_membership_verified -isnot [bool] -or -not [bool]$ready.worker_job_membership_verified) {
                    throw "containment ready identity mismatch"
                }
                return $ready
            }
            catch { throw "excel-vba-oracle-worker: invalid containment authority: $($_.Exception.Message)" }
        }
        Start-Sleep -Milliseconds 25
    }
    throw "excel-vba-oracle-worker: containment authority was not published before mutation"
}

function Get-ExcelExecutablePath {
    foreach ($keyPath in @(
        "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\excel.exe",
        "HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\excel.exe"
    )) {
        if (-not (Test-Path -LiteralPath $keyPath)) { continue }
        $candidate = [string](Get-Item -LiteralPath $keyPath).GetValue("")
        if (-not [string]::IsNullOrWhiteSpace($candidate) -and (Test-Path -LiteralPath $candidate)) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }
    $command = Get-Command excel.exe -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($command -and (Test-Path -LiteralPath $command.Source)) { return (Resolve-Path -LiteralPath $command.Source).Path }
    throw "excel-vba-oracle-worker: Excel executable was not found"
}

function Start-OwnedExcelApplication {
    param([Parameter(Mandatory = $true)][string]$ExcelExecutable)
    $process = Start-Process -FilePath $ExcelExecutable -ArgumentList "/x" -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    $nativeWindow = $null
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($process.HasExited) { throw "excel-vba-oracle-worker: directly launched Excel exited before automation attachment" }
        $process.Refresh()
        $hwnd = $process.MainWindowHandle
        if ($hwnd -ne [IntPtr]::Zero) {
            try {
                $nativeWindow = [ExcelOracleNativeMethods]::GetNativeObjectFromWindow($hwnd)
                $application = $nativeWindow.Application
                if ($null -ne $application) {
                    return [pscustomobject]@{ process = $process; application = $application; native_window = $nativeWindow }
                }
            }
            catch { }
        }
        Start-Sleep -Milliseconds 100
    }
    throw "excel-vba-oracle-worker: directly launched Excel did not expose OBJID_NATIVEOM"
}

function Get-ExcelProcessIds {
    return @(Get-Process -Name EXCEL -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
}

function Get-ExcelPidFromApplication {
    param([Parameter(Mandatory = $true)]$Application)
    $processId = [uint32]0
    [void][ExcelOracleNativeMethods]::GetWindowThreadProcessId([IntPtr][int64]$Application.Hwnd, [ref]$processId)
    if ($processId -eq 0) { throw "excel-vba-oracle-worker: Excel Hwnd did not resolve to a process" }
    return [int]$processId
}

function Add-OwnershipRecord {
    param(
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][int[]]$BeforePids,
        [Parameter(Mandatory = $true)][string]$CaseId
    )
    $record = [ordered]@{
        schema = "oxvba.excel-vba-oracle-owned-process.v1"
        run_id = $RunId
        case_id = $CaseId
        pid = $Process.Id
        process_name = [string]$Process.ProcessName
        process_start_utc = $Process.StartTime.ToUniversalTime().ToString("o")
        executable_path = [string]$Process.Path
        before_excel_pids = @($BeforePids)
        ownership = "owned-new-instance"
        acquired_utc = [DateTime]::UtcNow.ToString("o")
    }
    ($record | ConvertTo-Json -Compress -Depth 5) | Add-Content -LiteralPath $OwnershipFile -Encoding utf8NoBOM
    return [pscustomobject]$record
}

function Add-HelperOwnershipRecord {
    param(
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)][string]$CaseId
    )
    $record = [ordered]@{
        schema = "oxvba.excel-vba-oracle-owned-helper.v1"
        run_id = $RunId
        case_id = $CaseId
        role = "guardian"
        pid = $Process.Id
        process_name = [string]$Process.ProcessName
        process_start_utc = $Process.StartTime.ToUniversalTime().ToString("o")
        executable_path = [string]$Process.Path
        ownership = "owned-helper-process"
        acquired_utc = [DateTime]::UtcNow.ToString("o")
    }
    ($record | ConvertTo-Json -Compress -Depth 5) | Add-Content -LiteralPath $HelperOwnershipFile -Encoding utf8NoBOM
    return [pscustomobject]$record
}

function Set-GuardianControl {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$CaseId,
        [Parameter(Mandatory = $true)][string]$OperationId,
        [Parameter(Mandatory = $true)][ValidateSet("compile", "run", "cleanup")][string]$Phase,
        [Parameter(Mandatory = $true)][bool]$AllowDismiss
    )
    $script:GuardianControlSequence++
    $temporary = "$Path.$PID.tmp"
    [ordered]@{
        schema = "oxvba.excel-vba-oracle-guardian-control.v2"
        run_id = $RunId
        case_id = $CaseId
        operation_id = $OperationId
        sequence = $script:GuardianControlSequence
        phase = $Phase
        allow_dismiss = $AllowDismiss
        written_utc = [DateTime]::UtcNow.ToString("o")
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $temporary -Encoding utf8NoBOM
    Move-Item -Force -LiteralPath $temporary -Destination $Path
    return [pscustomobject]@{ case_id = $CaseId; operation_id = $OperationId; phase = $Phase; sequence = $script:GuardianControlSequence }
}

function Get-GuardianEvents {
    param(
        [Parameter(Mandatory = $true)][string]$EventsFile,
        [string]$OperationId
    )
    $lines = if (Test-Path -LiteralPath $EventsFile) { @(Get-Content -LiteralPath $EventsFile) } else { @() }
    $ledger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines $lines -RunId $RunId -ExpectedCaseIds $script:SelectedCaseIds
    if (@($ledger.errors).Count -gt 0) {
        throw "excel-vba-oracle-worker: invalid guardian event ledger: $($ledger.errors -join '; ')"
    }
    return @($ledger.records | Where-Object {
        [string]::IsNullOrWhiteSpace($OperationId) -or [string]$_.operation_id -eq $OperationId
    })
}

function Test-GuardianOperationHealthy {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events)
    $armed = @($Events | Where-Object { [string]$_.event_type -eq "operation-armed" })
    $heartbeats = @($Events | Where-Object { [string]$_.event_type -eq "guardian-heartbeat" })
    if ($armed.Count -ne 1 -or @($heartbeats | Where-Object { [long]$_.event_sequence -gt [long]$armed[0].event_sequence }).Count -eq 0) { return $false }
    $observations = @($Events | Where-Object { [string]$_.event_type -in @("dialog-observation", "ignored-top-level-window") })
    if ($observations.Count -eq 0) { return $false }
    $unsafe = @($observations | Where-Object {
        [string]$_.event_type -eq "dialog-observation" -and [string]$_.classification -in @("security-or-trust", "unrecognized-modal")
    })
    return $unsafe.Count -eq 0
}

function Test-LinkedSuccessfulDismissal {
    param(
        [Parameter(Mandatory = $true)]$Observation,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events
    )
    return @($Events | Where-Object {
        [string]$_.event_type -eq "dismissal-result" -and
        [string]$_.observation_id -eq [string]$Observation.observation_id -and
        [string]$_.operation_id -eq [string]$Observation.operation_id -and
        [bool]$_.succeeded -and
        -not [string]::IsNullOrWhiteSpace([string]$_.dismissed_button)
    }).Count -eq 1
}

function Test-CompileErrorEvidence {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events,
        [Parameter(Mandatory = $true)][string]$InjectedSource,
        [Parameter(Mandatory = $true)][string]$ExpectedToken,
        [Parameter(Mandatory = $true)][string]$ExpectedLine
    )
    $sourceLines = @($InjectedSource -split "`r?`n" | ForEach-Object { $_.Trim() })
    $dialogs = @($Events | Where-Object { [string]$_.event_type -eq "dialog-observation" })
    if ($dialogs.Count -eq 0 -or @($dialogs | Where-Object { [string]$_.classification -ne "compile-error" }).Count -gt 0) { return $false }
    $candidates = @($dialogs)
    foreach ($observation in $candidates) {
        $text = @($observation.dialog_text) -join " / "
        if (-not [string]::IsNullOrWhiteSpace($text) -and
            [string]$observation.selected_token -ceq $ExpectedToken -and
            [string]$observation.expanded_line.Trim() -ceq $ExpectedLine -and
            [string]$observation.expanded_line.Trim() -in $sourceLines -and
            (Test-LinkedSuccessfulDismissal -Observation $observation -Events $Events)) {
            return $true
        }
    }
    return $false
}

function Test-RuntimeErrorEvidence {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events)
    $dialogs = @($Events | Where-Object { [string]$_.event_type -eq "dialog-observation" })
    return $dialogs.Count -eq 1 -and [string]$dialogs[0].classification -eq "runtime-error" -and
        -not [string]::IsNullOrWhiteSpace((@($dialogs[0].dialog_text) -join " / ")) -and
        (Test-LinkedSuccessfulDismissal -Observation $dialogs[0] -Events $Events)
}

function Test-AmbiguousMacroEvidence {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events)
    $dialogs = @($Events | Where-Object { [string]$_.event_type -eq "dialog-observation" })
    if ($dialogs.Count -eq 0 -or @($dialogs | Where-Object { [string]$_.classification -ne "ambiguous-macro-failure" }).Count -gt 0) { return $false }
    $candidates = @($dialogs)
    foreach ($observation in $candidates) {
        $text = @($observation.dialog_text) -join " / "
        if (-not [string]::IsNullOrWhiteSpace($text) -and (Test-LinkedSuccessfulDismissal -Observation $observation -Events $Events)) {
            return $true
        }
    }
    return $false
}

function Test-NoDialogObservations {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events)
    return @($Events | Where-Object { [string]$_.event_type -eq "dialog-observation" }).Count -eq 0
}

function Wait-GuardianReady {
    param(
        [Parameter(Mandatory = $true)][string]$ReadyFile,
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process
    )
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $ReadyFile) {
            try {
                $ready = Get-Content -Raw -LiteralPath $ReadyFile | ConvertFrom-Json
                if ([string]$ready.schema -ne "oxvba.excel-vba-oracle-guardian-ready.v1" -or [int]$ready.guardian_pid -ne $Process.Id) {
                    throw "guardian ready schema/PID mismatch"
                }
                if (-not (Test-ExcelOracleProcessIdentity -Record $ready -Process $Process -ExpectedProcessName $Process.ProcessName -RunId $RunId)) {
                    throw "guardian ready PID/start/name/executable identity mismatch"
                }
                return $ready
            }
            catch {
                throw "excel-vba-oracle-worker: invalid guardian ready record: $($_.Exception.Message)"
            }
        }
        if ($Process.HasExited) { throw "excel-vba-oracle-worker: guardian exited before becoming ready (exit $($Process.ExitCode))" }
        Start-Sleep -Milliseconds 50
    }
    throw "excel-vba-oracle-worker: guardian did not become ready"
}

function Assert-GuardianLive {
    param(
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)]$ReadyRecord,
        [Parameter(Mandatory = $true)][string]$Phase
    )
    if ($Process.HasExited -or -not (Test-ExcelOracleProcessIdentity -Record $ReadyRecord -Process $Process -ExpectedProcessName $Process.ProcessName -RunId $RunId)) {
        throw "excel-vba-oracle-worker: guardian is not live with its exact identity immediately before $Phase"
    }
}

function Wait-GuardianEventFlush {
    param(
        [Parameter(Mandatory = $true)][string]$EventsFile,
        [Parameter(Mandatory = $true)][string]$OperationId,
        [Parameter(Mandatory = $true)]$ArmRecord,
        [Parameter(Mandatory = $true)][DateTime]$InvocationCompletedUtc
    )
    $deadline = [DateTime]::UtcNow.AddMilliseconds(2500)
    do {
        $events = @(Get-GuardianEvents -EventsFile $EventsFile -OperationId $OperationId)
        if (Test-ExcelOracleGuardianOperationCoverage -Events $events -OperationId $OperationId -ControlSequence ([long]$ArmRecord.control_sequence) -InvocationCompletedUtc $InvocationCompletedUtc) { return $events }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "excel-vba-oracle-worker: guardian heartbeat did not span operation '$OperationId' through evidence flush"
}

function Wait-GuardianOperationArmed {
    param(
        [Parameter(Mandatory = $true)][string]$EventsFile,
        [Parameter(Mandatory = $true)]$Control,
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)]$ReadyRecord
    )
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        Assert-GuardianLive -Process $Process -ReadyRecord $ReadyRecord -Phase "operation arm acknowledgement"
        $events = @(Get-GuardianEvents -EventsFile $EventsFile -OperationId $Control.operation_id)
        $armed = @($events | Where-Object {
            [string]$_.event_type -eq "operation-armed" -and
            [string]$_.case_id -eq [string]$Control.case_id -and
            [string]$_.phase -eq [string]$Control.phase -and
            [long]$_.control_sequence -eq [long]$Control.sequence
        })
        if ($armed.Count -eq 1) { return $armed[0] }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "excel-vba-oracle-worker: guardian did not acknowledge operation '$($Control.operation_id)'"
}

function Get-VbeCompileControl {
    param([Parameter(Mandatory = $true)]$Vbe)
    try {
        $found = $Vbe.CommandBars.FindControl($null, 578, $null, $null, $true)
        if ($null -ne $found) { return $found }
    }
    catch { }

    foreach ($bar in @($Vbe.CommandBars)) {
        foreach ($control in @($bar.Controls)) {
            try {
                if ([int]$control.Id -eq 578) { return $control }
                foreach ($child in @($control.Controls)) {
                    if ([int]$child.Id -eq 578) { return $child }
                }
            }
            catch { }
        }
    }
    return $null
}

function Get-VbeSelectionFromCom {
    param([Parameter(Mandatory = $true)]$Vbe)
    try {
        $pane = $Vbe.ActiveCodePane
        if ($null -eq $pane) { return $null }
        $startLine = 0
        $startColumn = 0
        $endLine = 0
        $endColumn = 0
        $pane.GetSelection([ref]$startLine, [ref]$startColumn, [ref]$endLine, [ref]$endColumn)
        $line = [string]$pane.CodeModule.Lines($startLine, 1)
        $token = $null
        if ($endLine -eq $startLine -and $endColumn -gt $startColumn -and $startColumn -gt 0) {
            $offset = $startColumn - 1
            $length = [Math]::Min($line.Length - $offset, $endColumn - $startColumn)
            if ($offset -ge 0 -and $length -gt 0) { $token = $line.Substring($offset, $length) }
        }
        return [pscustomobject]@{
            selected_token = if ($token) { $token.Trim() } else { $null }
            expanded_line = $line.Trim("`r", "`n")
        }
    }
    catch { return $null }
}

function Test-ComObjectIdentity {
    param($Left, $Right)
    if ($null -eq $Left -or $null -eq $Right) { return $false }
    $leftIdentity = [IntPtr]::Zero
    $rightIdentity = [IntPtr]::Zero
    try {
        $leftIdentity = [Runtime.InteropServices.Marshal]::GetIUnknownForObject($Left)
        $rightIdentity = [Runtime.InteropServices.Marshal]::GetIUnknownForObject($Right)
        return $leftIdentity -eq $rightIdentity
    }
    finally {
        if ($leftIdentity -ne [IntPtr]::Zero) { [void][Runtime.InteropServices.Marshal]::Release($leftIdentity) }
        if ($rightIdentity -ne [IntPtr]::Zero) { [void][Runtime.InteropServices.Marshal]::Release($rightIdentity) }
    }
}

function Get-ProjectFileName {
    param($Project)
    try { return [string]$Project.FileName }
    catch { return $null }
}

function Get-CompileAuthoritySnapshot {
    param(
        [Parameter(Mandatory = $true)]$Vbe,
        [Parameter(Mandatory = $true)]$Project,
        [Parameter(Mandatory = $true)]$Component,
        [Parameter(Mandatory = $true)][string]$ExpectedSourceSha256,
        [Parameter(Mandatory = $true)][string]$Stage
    )
    $activeProject = $null
    $activePane = $null
    $activeModule = $null
    $injectedPane = $null
    $activeProject = $Vbe.ActiveVBProject
    $activePane = $Vbe.ActiveCodePane
    $activeModule = if ($activePane) { $activePane.CodeModule } else { $null }
    $injectedPane = $Component.CodeModule.CodePane
    $source = [string]$Component.CodeModule.Lines(1, $Component.CodeModule.CountOfLines)
    $snapshot = [pscustomobject]@{
        stage = $Stage
        captured_utc = [DateTime]::UtcNow.ToString("o")
        active_project_is_injected_project = Test-ComObjectIdentity -Left $activeProject -Right $Project
        active_module_is_injected_module = if ($activeModule) { Test-ComObjectIdentity -Left $activeModule -Right $Component.CodeModule } else { $false }
        active_code_pane_is_injected_code_pane = if ($activePane -and $injectedPane) { Test-ComObjectIdentity -Left $activePane -Right $injectedPane } else { $false }
        active_project_name = if ($activeProject) { [string]$activeProject.Name } else { $null }
        active_module_name = if ($activeModule) { [string]$activeModule.Parent.Name } else { $null }
        injected_source_sha256 = Get-ExcelOracleSha256 -Text $source
        expected_source_sha256 = $ExpectedSourceSha256
    }
    if (-not $snapshot.active_project_is_injected_project -or -not $snapshot.active_module_is_injected_module -or
        -not $snapshot.active_code_pane_is_injected_code_pane -or $snapshot.injected_source_sha256 -cne $ExpectedSourceSha256) {
        throw "compile authority mismatch at $Stage"
    }
    # Active project/module/pane values are borrowed aliases and may share RCWs
    # with caller-owned objects. Never FinalRelease them inside this snapshot.
    return $snapshot
}

function Test-VbomProcedureExists {
    param(
        [Parameter(Mandatory = $true)]$Project,
        [Parameter(Mandatory = $true)][string]$QualifiedProcedure
    )
    $parts = $QualifiedProcedure.Split('.', 2)
    if ($parts.Count -ne 2) { return $false }
    try {
        $component = $Project.VBComponents.Item($parts[0])
        $line = [int]$component.CodeModule.ProcStartLine($parts[1], 0)
        return $line -gt 0
    }
    catch { return $false }
}

function Get-VbomRuntimeMeasurement {
    param(
        [Parameter(Mandatory = $true)]$Project,
        [Parameter(Mandatory = $true)]$Excel,
        [AllowNull()][string]$InvocationEntry,
        [AllowNull()][string]$MacroProbeTarget,
        [Parameter(Mandatory = $true)][string]$CompileStatus
    )
    $accessVbom = $false
    $invocationExists = $false
    $probeExists = $false
    try {
        [void]$Project.VBComponents.Count
        $accessVbom = $true
        if (-not [string]::IsNullOrWhiteSpace($InvocationEntry)) { $invocationExists = Test-VbomProcedureExists -Project $Project -QualifiedProcedure $InvocationEntry }
        if (-not [string]::IsNullOrWhiteSpace($MacroProbeTarget)) { $probeExists = Test-VbomProcedureExists -Project $Project -QualifiedProcedure $MacroProbeTarget }
    }
    catch { $accessVbom = $false }
    $automationSecurity = [int]$Excel.AutomationSecurity
    return [pscustomobject]@{
        measured_utc = [DateTime]::UtcNow.ToString("o")
        access_vbom = $accessVbom
        invocation_entry = $InvocationEntry
        invocation_entry_exists = $invocationExists
        macro_probe_target = $MacroProbeTarget
        macro_probe_target_exists = $probeExists
        automation_security = $automationSecurity
        macros_configured_for_automation = $automationSecurity -eq 1
        invocation_entry_observed = $false
        invocation_observation = $null
        macros_runnable_entry = $false
    }
}

function Release-ComObject {
    param($Value)
    if ($null -ne $Value -and [Runtime.InteropServices.Marshal]::IsComObject($Value)) {
        try { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($Value) }
        catch { }
    }
}

function Invoke-HarnessCase {
    param([Parameter(Mandatory = $true)]$Case)

    $caseDirectory = Join-Path $OutputDirectory $Case.id
    New-Item -ItemType Directory -Force -Path $caseDirectory | Out-Null
    $modulePath = Join-Path $caseDirectory "$($Case.module_name).bas"
    Set-Content -LiteralPath $modulePath -Value $Case.module_source -Encoding utf8NoBOM

    $beforePids = @(Get-ExcelProcessIds)
    $excel = $null
    $excelNativeWindow = $null
    $workbook = $null
    $project = $null
    $component = $null
    $compileControl = $null
    $guardian = $null
    $guardianOwnershipRecord = $null
    $ownedExcelProcess = $null
    $excelOwnershipRecord = $null
    $excelPid = $null
    $controlFile = Join-Path $caseDirectory "guardian-control.json"
    $eventsFile = Join-Path $caseDirectory "guardian-events.jsonl"
    $readyFile = Join-Path $caseDirectory "guardian-ready.json"
    $stopFile = Join-Path $caseDirectory "guardian-stop"
    $excelIdentityFile = Join-Path $caseDirectory "excel-process-identity.json"
    $guardianStdout = Join-Path $caseDirectory "guardian.stdout.txt"
    $guardianStderr = Join-Path $caseDirectory "guardian.stderr.txt"
    $compileEvents = @()
    $compileDialogs = @()
    $compileCommand = $null
    $compileExecution = $null
    $compileContext = $null
    $postDismissSelectionDiagnostic = $null
    $runtimeMeasurement = $null
    $runEvents = @()
    $compileStatus = "not-run"
    $runStatus = "not-run"
    $runValue = $null
    $runtimeErr = $null
    $errorMessage = $null
    $macroDisposition = $null
    $passed = $false
    $guardianHealthy = $false
    $evidenceStatus = $null
    $cleanupStatus = "not-run"
    $cleanupAuthorityErrors = [Collections.Generic.List[string]]::new()

    try {
        $launchedExcel = Start-OwnedExcelApplication -ExcelExecutable $script:ExcelExecutablePath
        $ownedExcelProcess = $launchedExcel.process
        $excel = $launchedExcel.application
        $excelNativeWindow = $launchedExcel.native_window
        $excel.Visible = $true
        $excel.DisplayAlerts = $false
        $excel.AutomationSecurity = 1
        $excelPid = Get-ExcelPidFromApplication -Application $excel
        if ($excelPid -ne $ownedExcelProcess.Id) {
            throw "excel-vba-oracle-worker: OBJID_NATIVEOM application PID does not match the directly launched contained Excel process"
        }
        if ($excelPid -in $beforePids) {
            throw "excel-vba-oracle-worker: Excel PID $excelPid existed before this case; refusing ownership"
        }
        $excelOwnershipRecord = Add-OwnershipRecord -Process $ownedExcelProcess -BeforePids $beforePids -CaseId $Case.id
        $excelOwnershipRecord | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $excelIdentityFile -Encoding utf8NoBOM

        $guardianArguments = @(
            "-NoLogo", "-NoProfile", "-NonInteractive", "-STA", "-File", (Join-Path $PSScriptRoot "excel-vba-oracle-guardian.ps1"),
            "-ExcelPid", [string]$excelPid,
            "-ExcelIdentityFile", $excelIdentityFile,
            "-RunId", $RunId,
            "-ControlFile", $controlFile,
            "-EventsFile", $eventsFile,
            "-ReadyFile", $readyFile,
            "-StopFile", $stopFile,
            "-MaxSeconds", [string]$CaseTimeoutSeconds
        )
        $guardian = Start-Process -FilePath (Join-Path $PSHOME "pwsh.exe") -ArgumentList $guardianArguments -PassThru -WindowStyle Hidden -RedirectStandardOutput $guardianStdout -RedirectStandardError $guardianStderr
        $guardianOwnershipRecord = Add-HelperOwnershipRecord -Process $guardian -CaseId $Case.id
        $guardianReady = Wait-GuardianReady -ReadyFile $readyFile -Process $guardian

        $workbook = $excel.Workbooks.Add()
        $workbook.Activate()
        $project = $workbook.VBProject
        $component = $project.VBComponents.Add(1)
        $component.Name = $Case.module_name
        [void]$component.CodeModule.AddFromString($Case.module_source)
        $excel.VBE.MainWindow.Visible = $true
        $component.CodeModule.CodePane.Show()
        try { $excel.VBE.MainWindow.SetFocus() } catch { }
        Start-Sleep -Milliseconds 250

        $injectedSource = [string]$component.CodeModule.Lines(1, $component.CodeModule.CountOfLines)
        $injectedSourceSha256 = Get-ExcelOracleSha256 -Text $injectedSource
        $selectedSourceSha256 = Get-ExcelOracleSha256 -Text ([string]$Case.module_source)
        if ($injectedSourceSha256 -cne $selectedSourceSha256) {
            throw "excel-vba-oracle-worker: injected module source does not match the selected case before compile authority"
        }
        $compileContext = [pscustomobject]@{
            injected_project_name = [string]$project.Name
            injected_project_file_name = Get-ProjectFileName -Project $project
            injected_module_name = [string]$component.Name
            selection_before_execute = Get-VbeSelectionFromCom -Vbe $excel.VBE
            injected_source = $injectedSource
            injected_source_sha256 = $injectedSourceSha256
            selected_source_sha256 = $selectedSourceSha256
            authority_before_execute = $null
            authority_after_execute = $null
        }

        $compileControl = Get-VbeCompileControl -Vbe $excel.VBE
        if ($null -eq $compileControl) { throw "excel-vba-oracle-worker: VBE compile command ID 578 was not found" }
        $compileCommand = [ordered]@{
            id = [int]$compileControl.Id
            caption = [string]$compileControl.Caption
            enabled_before = [bool]$compileControl.Enabled
            enabled_after = $null
        }
        if (-not $compileCommand.enabled_before) { throw "excel-vba-oracle-worker: VBE compile command ID 578 is disabled for the active code pane" }
        $compileOperation = "$($Case.id)-compile"
        $compileControlRecord = Set-GuardianControl -Path $controlFile -CaseId $Case.id -OperationId $compileOperation -Phase compile -AllowDismiss $true
        $compileArm = Wait-GuardianOperationArmed -EventsFile $eventsFile -Control $compileControlRecord -Process $guardian -ReadyRecord $guardianReady
        Assert-GuardianLive -Process $guardian -ReadyRecord $guardianReady -Phase "forced VBE compile after control publication"
        $compileContext.authority_before_execute = Get-CompileAuthoritySnapshot -Vbe $excel.VBE -Project $project -Component $component -ExpectedSourceSha256 $selectedSourceSha256 -Stage "immediately-before-execute"
        $executeException = $null
        $executeReturn = $null
        try { $executeReturn = $compileControl.Execute() }
        catch {
            $executeException = [pscustomobject]@{
                message = $_.Exception.Message
                hresult = "0x$($_.Exception.HResult.ToString('x8'))"
                type = $_.Exception.GetType().FullName
            }
        }
        $compileInvocationCompletedUtc = [DateTime]::UtcNow
        $compileContext.authority_after_execute = Get-CompileAuthoritySnapshot -Vbe $excel.VBE -Project $project -Component $component -ExpectedSourceSha256 $selectedSourceSha256 -Stage "immediately-after-execute"
        $compileEvents = @(Wait-GuardianEventFlush -EventsFile $eventsFile -OperationId $compileOperation -ArmRecord $compileArm -InvocationCompletedUtc $compileInvocationCompletedUtc)
        $compileDialogs = @($compileEvents | Where-Object { [string]$_.event_type -eq "dialog-observation" })
        # Post-dismiss COM state is diagnostic only. It can never repair or satisfy
        # the immutable guardian observation that authorized dismissal.
        $postDismissSelectionDiagnostic = Get-VbeSelectionFromCom -Vbe $excel.VBE
        $compileContext | Add-Member -NotePropertyName selection_after_execute_diagnostic_only -NotePropertyValue $postDismissSelectionDiagnostic
        $compileCommand.enabled_after = [bool]$compileControl.Enabled
        $compileExecution = [pscustomobject]@{
            return_value = if ($null -eq $executeReturn) { $null } else { [string]$executeReturn }
            exception = $executeException
        }
        $compileKinds = @($compileDialogs | Select-Object -ExpandProperty classification)
        if ($compileKinds -contains "security-or-trust" -or $compileKinds -contains "unrecognized-modal") {
            throw "excel-vba-oracle-worker: compile was blocked by a security/trust or unrecognized owned modal"
        }
        if ($compileKinds -contains "compile-error") { $compileStatus = "compile-error" }
        elseif ($executeException) { $compileStatus = "harness-error" }
        elseif (-not [bool]$compileCommand.enabled_after) { $compileStatus = "ok" }
        else { $compileStatus = "no-dialog-unverified" }

        $runtimeMeasurement = Get-VbomRuntimeMeasurement -Project $project -Excel $excel -InvocationEntry ([string]$Case.run_procedure) -MacroProbeTarget ([string]$Case.macro_probe_target) -CompileStatus $compileStatus
        if ($compileStatus -eq "ok" -and -not [string]::IsNullOrWhiteSpace([string]$Case.run_procedure)) {
            $runOperation = "$($Case.id)-run"
            $runControlRecord = Set-GuardianControl -Path $controlFile -CaseId $Case.id -OperationId $runOperation -Phase run -AllowDismiss $true
            $runArm = Wait-GuardianOperationArmed -EventsFile $eventsFile -Control $runControlRecord -Process $guardian -ReadyRecord $guardianReady
            Assert-GuardianLive -Process $guardian -ReadyRecord $guardianReady -Phase "runtime invocation after control publication"
            try {
                $qualifiedName = "'$($workbook.Name)'!$($Case.run_procedure)"
                $runValue = $excel.Run($qualifiedName)
                $observationPrefix = [string]$Case.invocation_observation_prefix
                if (-not [string]::IsNullOrWhiteSpace($observationPrefix)) {
                    if ([string]$runValue -like "$observationPrefix*") {
                        $runtimeMeasurement.invocation_entry_observed = $true
                        $runtimeMeasurement.invocation_observation = "case-specific-return-sentinel"
                        $runtimeMeasurement.macros_runnable_entry = $true
                    }
                }
                else {
                    $runtimeMeasurement.invocation_entry_observed = $true
                    $runtimeMeasurement.invocation_observation = "qualified-entry-returned"
                    $runtimeMeasurement.macros_runnable_entry = $true
                }
                if ($Case.id -eq "runtime-full-err") {
                    $runtimeErr = ConvertFrom-ExcelOracleRuntimeErr -Json ([string]$runValue)
                    $runStatus = "runtime-err-captured"
                }
                else {
                    $runStatus = "ok"
                }
            }
            catch {
                $errorMessage = $_.Exception.Message
                $macroDisposition = Get-ExcelOracleMacroFailureDisposition `
                    -Message $errorMessage `
                    -CompileStatus $compileStatus `
                    -AccessVbom ([bool]$runtimeMeasurement.access_vbom) `
                    -RunnableEntryObserved ([bool]$runtimeMeasurement.invocation_entry_observed) `
                    -TargetExists ([bool]$runtimeMeasurement.macro_probe_target_exists)
                $runStatus = $macroDisposition
            }
            $runInvocationCompletedUtc = [DateTime]::UtcNow
            $runEvents = @(Wait-GuardianEventFlush -EventsFile $eventsFile -OperationId $runOperation -ArmRecord $runArm -InvocationCompletedUtc $runInvocationCompletedUtc)
            if ($Case.id -eq "ambiguous-macro-failure" -and $runStatus -eq "ok") {
                $observationPrefix = [string]$Case.invocation_observation_prefix
                if (-not [bool]$runtimeMeasurement.invocation_entry_observed -or -not ([string]$runValue).StartsWith($observationPrefix, [StringComparison]::Ordinal)) {
                    throw "excel-vba-oracle-worker: ambiguous macro probe did not return its entry-observation sentinel"
                }
                $errorMessage = ([string]$runValue).Substring($observationPrefix.Length)
                $macroDisposition = Get-ExcelOracleMacroFailureDisposition `
                    -Message $errorMessage `
                    -CompileStatus $compileStatus `
                    -AccessVbom ([bool]$runtimeMeasurement.access_vbom) `
                    -RunnableEntryObserved ([bool]$runtimeMeasurement.invocation_entry_observed) `
                    -TargetExists ([bool]$runtimeMeasurement.macro_probe_target_exists)
                $runStatus = $macroDisposition
            }
            if ($Case.id -eq "runtime-unhandled-modal" -and (Test-RuntimeErrorEvidence -Events $runEvents)) {
                $runtimeMeasurement.invocation_entry_observed = $true
                $runtimeMeasurement.invocation_observation = "owned-runtime-error-modal"
                $runtimeMeasurement.macros_runnable_entry = $true
                $runStatus = "runtime-error-modal"
            }
        }

        $guardianHealthy = $guardian -and -not $guardian.HasExited
        $compileOperationHealthy = Test-GuardianOperationHealthy -Events $compileEvents
        $runOperationHealthy = if ($runStatus -eq "not-run") { $false } else { Test-GuardianOperationHealthy -Events $runEvents }
        $compileErrorEvidence = if ($Case.expected_compile_status -eq "compile-error") {
            Test-CompileErrorEvidence -Events $compileEvents -InjectedSource $injectedSource -ExpectedToken ([string]$Case.expected_selected_token) -ExpectedLine ([string]$Case.expected_expanded_line)
        } else { $false }
        $ambiguousMacroEvidence = Test-AmbiguousMacroEvidence -Events $runEvents
        $runtimeErrorEvidence = Test-RuntimeErrorEvidence -Events $runEvents
        $behaviorPassed = $compileStatus -eq $Case.expected_compile_status -and $runStatus -eq $Case.expected_run_status
        if ($behaviorPassed -and $Case.expected_value) { $behaviorPassed = [string]$runValue -eq [string]$Case.expected_value }
        if ($behaviorPassed -and $Case.id -eq "runtime-full-err") {
            $expectedErr = Get-ExcelOracleExpectedRuntimeErr
            foreach ($field in @("number", "source", "description", "help_file", "help_context", "erl")) {
                if ($runtimeErr.$field -ne $expectedErr.$field) { $behaviorPassed = $false }
            }
        }
        $authoritativeEvidencePassed = switch ($Case.id) {
            { $_ -in @("compile-failure", "intrinsic-shadow") } { $compileOperationHealthy -and $compileErrorEvidence; break }
            "ambiguous-macro-failure" { $compileOperationHealthy -and (Test-NoDialogObservations -Events $compileEvents) -and $runOperationHealthy -and $ambiguousMacroEvidence; break }
            "runtime-unhandled-modal" { $compileOperationHealthy -and (Test-NoDialogObservations -Events $compileEvents) -and $runOperationHealthy -and $runtimeErrorEvidence; break }
            default { $compileOperationHealthy -and (Test-NoDialogObservations -Events $compileEvents) -and $runOperationHealthy -and (Test-NoDialogObservations -Events $runEvents); break }
        }
        $passed = $behaviorPassed -and $guardianHealthy -and $authoritativeEvidencePassed
        $evidenceStatus = [pscustomobject]@{
            guardian_healthy_before_cleanup = $guardianHealthy
            compile_operation_healthy = $compileOperationHealthy
            run_operation_healthy = $runOperationHealthy
            compile_error_modal_complete = $compileErrorEvidence
            ambiguous_macro_modal_and_dismissal_complete = $ambiguousMacroEvidence
            runtime_error_modal_and_dismissal_complete = $runtimeErrorEvidence
            authoritative_evidence_passed = $authoritativeEvidencePassed
        }
    }
    catch {
        $errorMessage = $_.Exception.Message
        if ($compileStatus -eq "not-run") { $compileStatus = "harness-error" }
    }
    finally {
        if ($guardian) {
            New-Item -ItemType File -Force -Path $stopFile | Out-Null
            if (-not $guardian.WaitForExit(3000)) {
                if ($guardianOwnershipRecord) {
                    $guardianTermination = Invoke-ExcelOracleRetainedProcessTermination -Record $guardianOwnershipRecord -ExpectedProcessName ([string]$guardianOwnershipRecord.process_name) -RunId $RunId
                    if ([string]$guardianTermination.state -eq "same-instance-conflict") { $cleanupAuthorityErrors.Add("guardian retained identity conflict") }
                }
                else { $cleanupAuthorityErrors.Add("guardian cleanup lacks its written ownership record") }
            }
        }
        if ($workbook) {
            try { $workbook.Close($false) } catch { }
        }
        if ($excel) {
            try { $excel.Quit() } catch { }
        }
        Release-ComObject $compileControl
        Release-ComObject $component
        Release-ComObject $project
        Release-ComObject $workbook
        Release-ComObject $excel
        Release-ComObject $excelNativeWindow
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
        if ($ownedExcelProcess) {
            if (-not $ownedExcelProcess.WaitForExit(2000)) {
                if ($excelOwnershipRecord) {
                    $excelTermination = Invoke-ExcelOracleRetainedProcessTermination -Record $excelOwnershipRecord -ExpectedProcessName "EXCEL" -RunId $RunId
                    if ([string]$excelTermination.state -eq "same-instance-conflict") { $cleanupAuthorityErrors.Add("Excel retained identity conflict") }
                }
                else { $cleanupAuthorityErrors.Add("Excel cleanup lacks its written ownership record") }
            }
            $cleanupStatus = if ($ownedExcelProcess.HasExited) { "owned-process-zero" } else { "owned-process-remains" }
        }
        if ($cleanupAuthorityErrors.Count -gt 0) {
            $passed = $false
            $cleanupStatus = "cleanup-authority-error"
            $cleanupText = $cleanupAuthorityErrors -join "; "
            $errorMessage = if ([string]::IsNullOrWhiteSpace($errorMessage)) { $cleanupText } else { "$errorMessage; $cleanupText" }
        }
    }

    return [pscustomobject]@{
        schema = "oxvba.excel-vba-oracle-case-result.v1"
        id = $Case.id
        purpose = $Case.purpose
        passed = $passed
        owned_excel_pid = $excelPid
        module_path = $modulePath
        module_sha256 = Get-ExcelOracleSha256 -Text $Case.module_source
        compile_status = $compileStatus
        expected_compile_status = $Case.expected_compile_status
        compile_command = $compileCommand
        compile_execution = $compileExecution
        compile_context = $compileContext
        post_dismiss_selection_diagnostic_only = $postDismissSelectionDiagnostic
        compile_dialogs = @($compileDialogs)
        compile_window_observations = @($compileEvents)
        run_procedure = $Case.run_procedure
        run_status = $runStatus
        expected_run_status = $Case.expected_run_status
        run_value = if ($null -eq $runValue) { $null } else { [string]$runValue }
        runtime_err = $runtimeErr
        macro_failure_disposition = $macroDisposition
        runtime_measurement = $runtimeMeasurement
        transport_error = $errorMessage
        run_dialogs = @($runEvents)
        evidence_status = $evidenceStatus
        cleanup_status = $cleanupStatus
        cleanup_authority_errors = @($cleanupAuthorityErrors)
        defect_declaration = if ($Case.id -eq "intrinsic-shadow") { "Public Function Shadowed(ByVal Fix As Double) As Double" } else { $null }
    }
}

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$ownershipParent = Split-Path -Parent $OwnershipFile
if ($ownershipParent) { New-Item -ItemType Directory -Force -Path $ownershipParent | Out-Null }
$helperOwnershipParent = Split-Path -Parent $HelperOwnershipFile
if ($helperOwnershipParent) { New-Item -ItemType Directory -Force -Path $helperOwnershipParent | Out-Null }

$containmentAuthority = Wait-ContainmentAuthority
$script:ExcelExecutablePath = Get-ExcelExecutablePath

$selectedCases = @(Get-ExcelOracleHarnessCases | Where-Object { -not [bool]$_.diagnostic_only })
if (-not [string]::IsNullOrWhiteSpace($DiagnosticCaseId)) {
    $selectedCases = @(Get-ExcelOracleHarnessCases | Where-Object { $_.id -eq $DiagnosticCaseId })
    if ($selectedCases.Count -ne 1) { throw "excel-vba-oracle-worker: unknown diagnostic case '$DiagnosticCaseId'" }
}
if (@($selectedCases.id | Select-Object -Unique).Count -ne $selectedCases.Count -or @($selectedCases | Where-Object { [string]::IsNullOrWhiteSpace([string]$_.id) }).Count -gt 0) {
    throw "excel-vba-oracle-worker: selected case identities must be nonempty and unique"
}
$script:SelectedCaseIds = @($selectedCases | ForEach-Object { [string]$_.id })
$script:GuardianControlSequence = 0L
$results = [Collections.Generic.List[object]]::new()
foreach ($case in $selectedCases) {
    $results.Add((Invoke-HarnessCase -Case $case))
}

$document = [ordered]@{
    schema = "oxvba.excel-vba-oracle-results.v1"
    run_id = $RunId
    generated_utc = [DateTime]::UtcNow.ToString("o")
    worker_pid = $PID
    containment_token = $ContainmentToken
    containment_authority = $containmentAuthority
    diagnostic_only = -not [string]::IsNullOrWhiteSpace($DiagnosticCaseId)
    cases = @($results)
    passed = @($results | Where-Object { -not $_.passed }).Count -eq 0
}
$document | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath (Join-Path $OutputDirectory "results.json") -Encoding utf8NoBOM
if (-not $document.passed -and -not $document.diagnostic_only) { exit 1 }
