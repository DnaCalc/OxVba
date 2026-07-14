param(
    [int]$ExcelPid,
    [string]$ExcelIdentityFile,
    [string]$RunId,
    [string]$CaseId,
    [string]$ControlFile,
    [string]$EventsFile,
    [string]$ReadyFile,
    [string]$StopFile,
    [ValidateRange(25, 5000)][int]$PollMilliseconds = 100,
    [ValidateRange(1, 3600)][int]$MaxSeconds = 300,
    [switch]$PolicySelfTest
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "excel-vba-oracle-contract.ps1")

function Assert-GuardianPolicy {
    param(
        [Parameter(Mandatory = $true)][string]$ExpectedKind,
        [Parameter(Mandatory = $true)][string]$ExpectedDisposition,
        [Parameter(Mandatory = $true)]$Actual
    )
    if ($Actual.kind -ne $ExpectedKind -or $Actual.disposition -ne $ExpectedDisposition) {
        throw "excel-vba-oracle-guardian: expected $ExpectedKind/$ExpectedDisposition, got $($Actual.kind)/$($Actual.disposition)"
    }
}

if ($PolicySelfTest) {
    Assert-GuardianPolicy "compile-error" "capture-then-dismiss" (Get-ExcelOracleDialogPolicy -Phase compile -WindowTitle "Microsoft Visual Basic for Applications" -Texts @("Compile error: Sub or Function not defined") -Buttons @("OK"))
    Assert-GuardianPolicy "runtime-error" "capture-then-dismiss" (Get-ExcelOracleDialogPolicy -Phase run -Texts @("Run-time error '13': Type mismatch") -Buttons @("End", "Debug", "Help"))
    Assert-GuardianPolicy "security-or-trust" "block-no-dismiss" (Get-ExcelOracleDialogPolicy -Phase compile -Texts @("Macros in this project are disabled") -Buttons @("Enable Content"))
    Assert-GuardianPolicy "ambiguous-macro-failure" "capture-then-dismiss" (Get-ExcelOracleDialogPolicy -Phase run -Texts @("Cannot run the macro. The macro may not be available or all macros may be disabled.") -Buttons @("OK"))
    Assert-GuardianPolicy "unrecognized-modal" "block-no-dismiss" (Get-ExcelOracleDialogPolicy -Phase run -Texts @("Unexpected prompt") -Buttons @("Yes", "No"))
    Assert-GuardianPolicy "unrecognized-modal" "block-no-dismiss" (Get-ExcelOracleDialogPolicy -Phase run -Texts @("Compile error: Sub or Function not defined") -Buttons @("OK"))
    Assert-GuardianPolicy "unrecognized-modal" "block-no-dismiss" (Get-ExcelOracleDialogPolicy -Phase compile -Texts @("Run-time error '13': Type mismatch") -Buttons @("End"))
    Write-Output "excel-vba-oracle-guardian: policy self-test passed"
    exit 0
}

foreach ($required in @{
        ExcelPid = $ExcelPid
        ExcelIdentityFile = $ExcelIdentityFile
        RunId = $RunId
        CaseId = $CaseId
        ControlFile = $ControlFile
        EventsFile = $EventsFile
        ReadyFile = $ReadyFile
        StopFile = $StopFile
    }.GetEnumerator()) {
    if ($required.Key -eq "ExcelPid") {
        if ([int]$required.Value -le 0) { throw "excel-vba-oracle-guardian: ExcelPid is required" }
    }
    elseif ([string]::IsNullOrWhiteSpace([string]$required.Value)) {
        throw "excel-vba-oracle-guardian: $($required.Key) is required"
    }
}

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

if (-not (Test-Path -LiteralPath $ExcelIdentityFile)) {
    throw "excel-vba-oracle-guardian: Excel identity file does not exist"
}
$excelIdentity = Get-Content -Raw -LiteralPath $ExcelIdentityFile | ConvertFrom-Json
$initialExcelProcess = Get-Process -Id $ExcelPid -ErrorAction Stop
if (-not (Test-ExcelOracleProcessIdentity -Record $excelIdentity -Process $initialExcelProcess -ExpectedProcessName "EXCEL" -RunId $RunId)) {
    throw "excel-vba-oracle-guardian: Excel PID/start/name/executable identity did not validate"
}

function Get-OwnedTopLevelWindows {
    # Office/VBE top-level HWNDs are not consistently projected as UIA
    # ControlType.Window. Enumerate every desktop child and then apply the hard
    # PID boundary, matching the proven oracle probes in this repository.
    $desktopChildren = [Windows.Automation.AutomationElement]::RootElement.FindAll(
        [Windows.Automation.TreeScope]::Children,
        [Windows.Automation.Condition]::TrueCondition
    )
    $owned = [Collections.Generic.List[Windows.Automation.AutomationElement]]::new()
    foreach ($window in $desktopChildren) {
        try {
            if ([int]$window.Current.ProcessId -eq $ExcelPid) { $owned.Add($window) }
        }
        catch {
            # Desktop UIA children can disappear between enumeration and read.
        }
    }
    return @($owned)
}

function Get-ElementStrings {
    param(
        [Parameter(Mandatory = $true)][Windows.Automation.AutomationElement]$Root,
        [Parameter(Mandatory = $true)][Windows.Automation.ControlType]$ControlType
    )
    $condition = [Windows.Automation.PropertyCondition]::new(
        [Windows.Automation.AutomationElement]::ControlTypeProperty,
        $ControlType
    )
    $values = [Collections.Generic.List[string]]::new()
    foreach ($element in @($Root.FindAll([Windows.Automation.TreeScope]::Descendants, $condition))) {
        try {
            $name = [string]$element.Current.Name
            if (-not [string]::IsNullOrWhiteSpace($name)) { $values.Add($name.Trim()) }
        }
        catch {
            # A stale descendant must not stop the guardian.
        }
    }
    return @($values | Select-Object -Unique)
}

function Get-VbeSelection {
    $selection = [ordered]@{ selected_token = $null; expanded_line = $null }
    $documentCondition = [Windows.Automation.OrCondition]::new(
        [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Document),
        [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Edit)
    )
    foreach ($window in @(Get-OwnedTopLevelWindows)) {
        try {
            if ([string]$window.Current.Name -notlike "Microsoft Visual Basic for Applications*") { continue }
            foreach ($document in @($window.FindAll([Windows.Automation.TreeScope]::Descendants, $documentCondition))) {
                $pattern = $document.GetCurrentPattern([Windows.Automation.TextPattern]::Pattern)
                $ranges = @($pattern.GetSelection())
                if ($ranges.Count -eq 0) { continue }
                $token = [string]$ranges[0].GetText(-1)
                $lineRange = $ranges[0].Clone()
                $lineRange.ExpandToEnclosingUnit([Windows.Automation.TextUnit]::Line)
                $line = [string]$lineRange.GetText(-1)
                if (-not [string]::IsNullOrWhiteSpace($token) -or -not [string]::IsNullOrWhiteSpace($line)) {
                    $selection.selected_token = $token.Trim("`r", "`n")
                    $selection.expanded_line = $line.Trim("`r", "`n")
                    return [pscustomobject]$selection
                }
            }
        }
        catch {
            # Stale windows and documents without TextPattern are nonfatal.
        }
    }
    return [pscustomobject]$selection
}

function Get-WindowIdentity {
    param([Parameter(Mandatory = $true)][Windows.Automation.AutomationElement]$Window)
    try { return [string]$Window.Current.NativeWindowHandle }
    catch { return "$($Window.Current.Name):$($Window.Current.AutomationId)" }
}

function Invoke-OwnedDialogButton {
    param(
        [Parameter(Mandatory = $true)][Windows.Automation.AutomationElement]$Window,
        [Parameter(Mandatory = $true)][string[]]$PreferredButtons
    )
    $buttonCondition = [Windows.Automation.PropertyCondition]::new(
        [Windows.Automation.AutomationElement]::ControlTypeProperty,
        [Windows.Automation.ControlType]::Button
    )
    $buttons = @($Window.FindAll([Windows.Automation.TreeScope]::Descendants, $buttonCondition))
    foreach ($preferred in $PreferredButtons) {
        $button = @($buttons | Where-Object { [string]$_.Current.Name -eq $preferred } | Select-Object -First 1)
        if ($button.Count -eq 0) { continue }
        try {
            $invoke = $button[0].GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)
            $invoke.Invoke()
            return $preferred
        }
        catch {
            return $null
        }
    }
    return $null
}

function Add-GuardianEvent {
    param([Parameter(Mandatory = $true)]$Event)
    $script:eventSequence++
    $Event.event_sequence = $script:eventSequence
    $bytes = [Text.UTF8Encoding]::new($false).GetBytes("$(($Event | ConvertTo-Json -Compress -Depth 8))`n")
    $stream = [IO.FileStream]::new(
        $EventsFile,
        [IO.FileMode]::Append,
        [IO.FileAccess]::Write,
        [IO.FileShare]::Read,
        4096,
        [IO.FileOptions]::WriteThrough
    )
    try {
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
    }
    finally { $stream.Dispose() }
}

$readyParent = Split-Path -Parent $ReadyFile
if ($readyParent) { New-Item -ItemType Directory -Force -Path $readyParent | Out-Null }
$eventParent = Split-Path -Parent $EventsFile
if ($eventParent) { New-Item -ItemType Directory -Force -Path $eventParent | Out-Null }

$selfProcess = Get-Process -Id $PID
$ready = [ordered]@{
    schema = "oxvba.excel-vba-oracle-guardian-ready.v1"
    run_id = $RunId
    excel_pid = $ExcelPid
    excel_process_start_utc = [string]$excelIdentity.process_start_utc
    excel_executable_path = [string]$excelIdentity.executable_path
    guardian_pid = $PID
    pid = $PID
    process_name = [string]$selfProcess.ProcessName
    process_start_utc = $selfProcess.StartTime.ToUniversalTime().ToString("o")
    executable_path = [string]$selfProcess.Path
    ready_utc = [DateTime]::UtcNow.ToString("o")
}
$ready | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $ReadyFile -Encoding utf8NoBOM

$seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$invalidControls = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$armedControls = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$script:eventSequence = 0L
$lastControlSequence = 0L
$deadline = [DateTime]::UtcNow.AddSeconds($MaxSeconds)
while ([DateTime]::UtcNow -lt $deadline -and -not (Test-Path -LiteralPath $StopFile)) {
    $currentExcelProcess = Get-Process -Id $ExcelPid -ErrorAction SilentlyContinue
    if (-not $currentExcelProcess -or -not (Test-ExcelOracleProcessIdentity -Record $excelIdentity -Process $currentExcelProcess -ExpectedProcessName "EXCEL" -RunId $RunId)) { break }
    $control = $null
    if (Test-Path -LiteralPath $ControlFile) {
        $controlJson = Get-Content -Raw -LiteralPath $ControlFile
        $controlHash = Get-ExcelOracleSha256 -Text $controlJson
        $parsedControl = ConvertFrom-ExcelOracleGuardianControl -Json $controlJson -RunId $RunId
        if (@($parsedControl.errors).Count -gt 0) {
            if ($invalidControls.Add($controlHash)) {
                Add-GuardianEvent -Event ([ordered]@{
                    schema = "oxvba.excel-vba-oracle-control-status.v1"
                    event_type = "invalid-control"
                    run_id = $RunId
                    event_sequence = 0L
                    observed_utc = [DateTime]::UtcNow.ToString("o")
                    control_sha256 = $controlHash
                    errors = @($parsedControl.errors)
                    valid = $false
                })
            }
            # An invalid control never arms an operation and can never authorize a dismissal.
            Start-Sleep -Milliseconds $PollMilliseconds
            continue
        }
        $control = $parsedControl.control
    }
    if ($null -eq $control -or [string]::IsNullOrWhiteSpace([string]$control.operation_id)) {
        Start-Sleep -Milliseconds $PollMilliseconds
        continue
    }
    $controlKey = "$([string]$control.case_id)|$([string]$control.operation_id)|$([long]$control.sequence)"
    if ([long]$control.sequence -lt $lastControlSequence) {
        if ($invalidControls.Add("stale:$controlKey")) {
            Add-GuardianEvent -Event ([ordered]@{
                schema = "oxvba.excel-vba-oracle-control-status.v1"; event_type = "invalid-control"; run_id = $RunId
                event_sequence = 0L; observed_utc = [DateTime]::UtcNow.ToString("o"); control_sha256 = Get-ExcelOracleSha256 -Text ($control | ConvertTo-Json -Compress)
                errors = @("control sequence regressed"); valid = $false
            })
        }
        Start-Sleep -Milliseconds $PollMilliseconds
        continue
    }
    if ($armedControls.Add($controlKey)) {
        if ([long]$control.sequence -le $lastControlSequence) {
            Add-GuardianEvent -Event ([ordered]@{
                schema = "oxvba.excel-vba-oracle-control-status.v1"; event_type = "invalid-control"; run_id = $RunId
                event_sequence = 0L; observed_utc = [DateTime]::UtcNow.ToString("o"); control_sha256 = Get-ExcelOracleSha256 -Text ($control | ConvertTo-Json -Compress)
                errors = @("control sequence was reused"); valid = $false
            })
            Start-Sleep -Milliseconds $PollMilliseconds
            continue
        }
        $lastControlSequence = [long]$control.sequence
        Add-GuardianEvent -Event ([ordered]@{
            schema = "oxvba.excel-vba-oracle-operation-state.v1"
            event_type = "operation-armed"
            run_id = $RunId
            case_id = [string]$control.case_id
            operation_id = [string]$control.operation_id
            phase = [string]$control.phase
            control_sequence = [long]$control.sequence
            event_sequence = 0L
            observed_utc = [DateTime]::UtcNow.ToString("o")
        })
    }
    Add-GuardianEvent -Event ([ordered]@{
        schema = "oxvba.excel-vba-oracle-operation-state.v1"
        event_type = "guardian-heartbeat"
        run_id = $RunId
        case_id = [string]$control.case_id
        operation_id = [string]$control.operation_id
        phase = [string]$control.phase
        control_sequence = [long]$control.sequence
        event_sequence = 0L
        observed_utc = [DateTime]::UtcNow.ToString("o")
    })

    foreach ($window in @(Get-OwnedTopLevelWindows)) {
        try {
            $isModal = $false
            try {
                $windowPattern = $window.GetCurrentPattern([Windows.Automation.WindowPattern]::Pattern)
                $isModal = [bool]$windowPattern.Current.IsModal
            }
            catch { }
            $className = [string]$window.Current.ClassName
            $windowTitle = [string]$window.Current.Name
            $windowHandle = Get-WindowIdentity $window
            $observedProcessId = [int]$window.Current.ProcessId
            $texts = @(Get-ElementStrings -Root $window -ControlType ([Windows.Automation.ControlType]::Text))
            $buttons = @(Get-ElementStrings -Root $window -ControlType ([Windows.Automation.ControlType]::Button))
            $policy = Get-ExcelOracleDialogPolicy -Phase ([string]$control.phase) -WindowTitle $windowTitle -Texts $texts -Buttons $buttons
            # VBE compile dialogs are not consistently exposed as IsModal/#32770 on
            # all Office builds. Recognized dialog text is authoritative. Unknown
            # non-modal application windows are recorded for the bounded audit but
            # ignored for policy/action purposes.
            $consideredDialog = $policy.kind -ne "unrecognized-modal" -or $isModal -or $className -eq "#32770"
            $key = "$($control.operation_id)|$windowHandle|$($policy.kind)|$($texts -join '|')"
            if (-not $seen.Add($key)) { continue }

            $vbeSelection = if ($consideredDialog) { Get-VbeSelection } else { [pscustomobject]@{ selected_token = $null; expanded_line = $null } }
            $observationId = [Guid]::NewGuid().ToString("D")
            $observationEvent = [ordered]@{
                schema = "oxvba.excel-vba-oracle-window-observation.v1"
                event_type = if ($consideredDialog) { "dialog-observation" } else { "ignored-top-level-window" }
                observation_id = $observationId
                run_id = $RunId
                case_id = [string]$control.case_id
                operation_id = [string]$control.operation_id
                control_sequence = [long]$control.sequence
                event_sequence = 0L
                phase = [string]$control.phase
                excel_pid = $ExcelPid
                observed_process_id = $observedProcessId
                observed_utc = [DateTime]::UtcNow.ToString("o")
                capture_completed_utc = [DateTime]::UtcNow.ToString("o")
                window_title = $windowTitle
                window_class = $className
                window_handle = $windowHandle
                is_modal = $isModal
                considered_dialog = $consideredDialog
                dialog_text = $texts
                visible_buttons = $buttons
                selected_token = $vbeSelection.selected_token
                expanded_line = $vbeSelection.expanded_line
                classification = [string]$policy.kind
                disposition = [string]$policy.disposition
            }
            # The immutable observation must be durable before any owned UI is
            # changed. A later crash can therefore never dismiss without capture.
            Add-GuardianEvent -Event $observationEvent

            if ($consideredDialog -and $policy.disposition -eq "capture-then-dismiss" -and [bool]$control.allow_dismiss) {
                $dismissedButton = Invoke-OwnedDialogButton -Window $window -PreferredButtons @($policy.preferred_buttons)
                $dismissalEvent = [ordered]@{
                    schema = "oxvba.excel-vba-oracle-dismissal-result.v1"
                    event_type = "dismissal-result"
                    observation_id = $observationId
                    run_id = $RunId
                    case_id = [string]$control.case_id
                    operation_id = [string]$control.operation_id
                    control_sequence = [long]$control.sequence
                    event_sequence = 0L
                    phase = [string]$control.phase
                    excel_pid = $ExcelPid
                    window_handle = $windowHandle
                    attempted_utc = [DateTime]::UtcNow.ToString("o")
                    requested_buttons = @($policy.preferred_buttons)
                    dismissed_button = $dismissedButton
                    succeeded = -not [string]::IsNullOrWhiteSpace([string]$dismissedButton)
                }
                Add-GuardianEvent -Event $dismissalEvent
            }
        }
        catch {
            # Stale top-level UIA children are expected and nonfatal per element.
        }
    }
    Start-Sleep -Milliseconds $PollMilliseconds
}

$controlledStopObserved = Test-Path -LiteralPath $StopFile
$finalExcelProcess = Get-Process -Id $ExcelPid -ErrorAction SilentlyContinue
$excelIdentityLiveAtStop = $false
if ($finalExcelProcess) {
    try { $excelIdentityLiveAtStop = Test-ExcelOracleProcessIdentity -Record $excelIdentity -Process $finalExcelProcess -ExpectedProcessName "EXCEL" -RunId $RunId }
    catch { $excelIdentityLiveAtStop = $false }
}
$exitReason = if ($controlledStopObserved -and $excelIdentityLiveAtStop) { "controlled-stop" }
    elseif (-not $excelIdentityLiveAtStop) { "excel-identity-lost" }
    else { "deadline" }
Add-GuardianEvent -Event ([ordered]@{
    schema = "oxvba.excel-vba-oracle-final-state.v1"
    event_type = "guardian-stopped"
    run_id = $RunId
    case_id = $CaseId
    event_sequence = 0L
    observed_utc = [DateTime]::UtcNow.ToString("o")
    guardian_pid = $PID
    process_name = [string]$selfProcess.ProcessName
    process_start_utc = $selfProcess.StartTime.ToUniversalTime().ToString("o")
    executable_path = [string]$selfProcess.Path
    controlled_stop_observed = [bool]$controlledStopObserved
    excel_identity_live_at_stop = [bool]$excelIdentityLiveAtStop
    exit_reason = $exitReason
})
if ($exitReason -ne "controlled-stop") { exit 2 }
