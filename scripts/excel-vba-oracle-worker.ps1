param(
    [Parameter(Mandatory = $true)][string]$RunId,
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [Parameter(Mandatory = $true)][string]$OwnershipFile,
    [Parameter(Mandatory = $true)][string]$HelperOwnershipFile,
    [ValidateRange(5, 600)][int]$CaseTimeoutSeconds = 90,
    [string]$DiagnosticCaseId = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "excel-vba-oracle-contract.ps1")

if (-not ([System.Management.Automation.PSTypeName]'ExcelOracleNativeMethods').Type) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class ExcelOracleNativeMethods
{
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
'@
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
}

function Set-GuardianControl {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$OperationId,
        [Parameter(Mandatory = $true)][ValidateSet("compile", "run", "cleanup")][string]$Phase,
        [Parameter(Mandatory = $true)][bool]$AllowDismiss
    )
    $temporary = "$Path.$PID.tmp"
    [ordered]@{
        schema = "oxvba.excel-vba-oracle-guardian-control.v1"
        run_id = $RunId
        operation_id = $OperationId
        phase = $Phase
        allow_dismiss = $AllowDismiss
        written_utc = [DateTime]::UtcNow.ToString("o")
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $temporary -Encoding utf8NoBOM
    Move-Item -Force -LiteralPath $temporary -Destination $Path
}

function Get-GuardianEvents {
    param(
        [Parameter(Mandatory = $true)][string]$EventsFile,
        [string]$OperationId
    )
    if (-not (Test-Path -LiteralPath $EventsFile)) { return @() }
    $events = [Collections.Generic.List[object]]::new()
    foreach ($line in @(Get-Content -LiteralPath $EventsFile)) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        try {
            $event = $line | ConvertFrom-Json
            if ([string]::IsNullOrWhiteSpace($OperationId) -or [string]$event.operation_id -eq $OperationId) {
                $events.Add($event)
            }
        }
        catch { }
    }
    return @($events)
}

function Wait-GuardianReady {
    param(
        [Parameter(Mandatory = $true)][string]$ReadyFile,
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process
    )
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $ReadyFile) { return }
        if ($Process.HasExited) { throw "excel-vba-oracle-worker: guardian exited before becoming ready (exit $($Process.ExitCode))" }
        Start-Sleep -Milliseconds 50
    }
    throw "excel-vba-oracle-worker: guardian did not become ready"
}

function Wait-GuardianEventFlush {
    param(
        [Parameter(Mandatory = $true)][string]$EventsFile,
        [Parameter(Mandatory = $true)][string]$OperationId
    )
    $deadline = [DateTime]::UtcNow.AddMilliseconds(2500)
    do { Start-Sleep -Milliseconds 50 } while ([DateTime]::UtcNow -lt $deadline)
    return @(Get-GuardianEvents -EventsFile $EventsFile -OperationId $OperationId)
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
    $workbook = $null
    $project = $null
    $component = $null
    $compileControl = $null
    $guardian = $null
    $ownedExcelProcess = $null
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
    $runEvents = @()
    $compileStatus = "not-run"
    $runStatus = "not-run"
    $runValue = $null
    $runtimeErr = $null
    $errorMessage = $null
    $macroDisposition = $null
    $passed = $false
    $cleanupStatus = "not-run"

    try {
        $excel = New-Object -ComObject Excel.Application
        $excel.Visible = $true
        $excel.DisplayAlerts = $false
        $excel.AutomationSecurity = 1
        $excelPid = Get-ExcelPidFromApplication -Application $excel
        if ($excelPid -in $beforePids) {
            throw "excel-vba-oracle-worker: Excel PID $excelPid existed before this case; refusing ownership"
        }
        $ownedExcelProcess = Get-Process -Id $excelPid -ErrorAction Stop
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
        Add-HelperOwnershipRecord -Process $guardian -CaseId $Case.id
        Wait-GuardianReady -ReadyFile $readyFile -Process $guardian

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

        $activeProject = $excel.VBE.ActiveVBProject
        $activePane = $excel.VBE.ActiveCodePane
        $activeModule = if ($activePane) { $activePane.CodeModule } else { $null }
        $injectedSource = [string]$component.CodeModule.Lines(1, $component.CodeModule.CountOfLines)
        $compileContext = [pscustomobject]@{
            injected_project_name = [string]$project.Name
            injected_project_file_name = Get-ProjectFileName -Project $project
            active_project_name = if ($activeProject) { [string]$activeProject.Name } else { $null }
            active_project_file_name = if ($activeProject) { Get-ProjectFileName -Project $activeProject } else { $null }
            active_project_is_injected_project = Test-ComObjectIdentity -Left $activeProject -Right $project
            injected_module_name = [string]$component.Name
            active_module_name = if ($activeModule) { [string]$activeModule.Parent.Name } else { $null }
            active_module_is_injected_module = if ($activeModule) { Test-ComObjectIdentity -Left $activeModule -Right $component.CodeModule } else { $false }
            selection_before_execute = Get-VbeSelectionFromCom -Vbe $excel.VBE
            injected_source = $injectedSource
            injected_source_sha256 = Get-ExcelOracleSha256 -Text $injectedSource
        }
        Release-ComObject $activeModule
        Release-ComObject $activePane
        Release-ComObject $activeProject

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
        Set-GuardianControl -Path $controlFile -OperationId $compileOperation -Phase compile -AllowDismiss $true
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
        $compileEvents = @(Wait-GuardianEventFlush -EventsFile $eventsFile -OperationId $compileOperation)
        $compileDialogs = @($compileEvents | Where-Object { [string]$_.event_type -eq "dialog-observation" })
        $comSelection = Get-VbeSelectionFromCom -Vbe $excel.VBE
        $compileContext | Add-Member -NotePropertyName selection_after_execute -NotePropertyValue $comSelection
        if ($comSelection) {
            foreach ($event in $compileDialogs) {
                if ([string]::IsNullOrWhiteSpace([string]$event.selected_token)) { $event.selected_token = $comSelection.selected_token }
                if ([string]::IsNullOrWhiteSpace([string]$event.expanded_line)) { $event.expanded_line = $comSelection.expanded_line }
            }
        }
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

        if ($compileStatus -eq "ok" -and -not [string]::IsNullOrWhiteSpace([string]$Case.run_procedure)) {
            $runOperation = "$($Case.id)-run"
            Set-GuardianControl -Path $controlFile -OperationId $runOperation -Phase run -AllowDismiss $true
            try {
                $qualifiedName = "'$($workbook.Name)'!$($Case.run_procedure)"
                $runValue = $excel.Run($qualifiedName)
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
                    -AccessVbom $true `
                    -MacrosEnabled $true `
                    -TargetExists ([bool]$Case.target_exists)
                $runStatus = $macroDisposition
            }
            $runEvents = @(Wait-GuardianEventFlush -EventsFile $eventsFile -OperationId $runOperation)
        }

        $passed = $compileStatus -eq $Case.expected_compile_status -and $runStatus -eq $Case.expected_run_status
        if ($passed -and $Case.expected_value) { $passed = [string]$runValue -eq [string]$Case.expected_value }
        if ($passed -and $Case.id -eq "runtime-full-err") {
            $expectedErr = Get-ExcelOracleExpectedRuntimeErr
            foreach ($field in @("number", "source", "description", "help_file", "help_context", "erl")) {
                if ($runtimeErr.$field -ne $expectedErr.$field) { $passed = $false }
            }
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
                try { $guardian.Kill() } catch { }
                [void]$guardian.WaitForExit(5000)
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
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
        if ($ownedExcelProcess) {
            if (-not $ownedExcelProcess.WaitForExit(2000)) {
                try { $ownedExcelProcess.Kill() } catch { }
                [void]$ownedExcelProcess.WaitForExit(5000)
            }
            $cleanupStatus = if ($ownedExcelProcess.HasExited) { "owned-process-zero" } else { "owned-process-remains" }
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
        compile_dialogs = @($compileDialogs)
        compile_window_observations = @($compileEvents)
        run_procedure = $Case.run_procedure
        run_status = $runStatus
        expected_run_status = $Case.expected_run_status
        run_value = if ($null -eq $runValue) { $null } else { [string]$runValue }
        runtime_err = $runtimeErr
        macro_failure_disposition = $macroDisposition
        transport_error = $errorMessage
        run_dialogs = @($runEvents)
        cleanup_status = $cleanupStatus
        defect_declaration = if ($Case.id -eq "intrinsic-shadow") { "Public Function Shadowed(ByVal Fix As Double) As Double" } else { $null }
    }
}

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$ownershipParent = Split-Path -Parent $OwnershipFile
if ($ownershipParent) { New-Item -ItemType Directory -Force -Path $ownershipParent | Out-Null }
$helperOwnershipParent = Split-Path -Parent $HelperOwnershipFile
if ($helperOwnershipParent) { New-Item -ItemType Directory -Force -Path $helperOwnershipParent | Out-Null }

$selectedCases = @(Get-ExcelOracleHarnessCases)
if (-not [string]::IsNullOrWhiteSpace($DiagnosticCaseId)) {
    $selectedCases = @($selectedCases | Where-Object { $_.id -eq $DiagnosticCaseId })
    if ($selectedCases.Count -ne 1) { throw "excel-vba-oracle-worker: unknown diagnostic case '$DiagnosticCaseId'" }
}
$results = [Collections.Generic.List[object]]::new()
foreach ($case in $selectedCases) {
    $results.Add((Invoke-HarnessCase -Case $case))
}

$document = [ordered]@{
    schema = "oxvba.excel-vba-oracle-results.v1"
    run_id = $RunId
    generated_utc = [DateTime]::UtcNow.ToString("o")
    worker_pid = $PID
    diagnostic_only = -not [string]::IsNullOrWhiteSpace($DiagnosticCaseId)
    cases = @($results)
    passed = @($results | Where-Object { -not $_.passed }).Count -eq 0
}
$document | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath (Join-Path $OutputDirectory "results.json") -Encoding utf8NoBOM
if (-not $document.passed -and -not $document.diagnostic_only) { exit 1 }
