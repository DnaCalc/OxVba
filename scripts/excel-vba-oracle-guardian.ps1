param(
    [int]$ExcelPid,
    [string]$RunId,
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
    Write-Output "excel-vba-oracle-guardian: policy self-test passed"
    exit 0
}

foreach ($required in @{
        ExcelPid = $ExcelPid
        RunId = $RunId
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

function Get-OwnedTopLevelWindows {
    $pidCondition = [Windows.Automation.PropertyCondition]::new(
        [Windows.Automation.AutomationElement]::ProcessIdProperty,
        $ExcelPid
    )
    $windowCondition = [Windows.Automation.PropertyCondition]::new(
        [Windows.Automation.AutomationElement]::ControlTypeProperty,
        [Windows.Automation.ControlType]::Window
    )
    $condition = [Windows.Automation.AndCondition]::new($pidCondition, $windowCondition)
    return @([Windows.Automation.AutomationElement]::RootElement.FindAll(
        [Windows.Automation.TreeScope]::Children,
        $condition
    ))
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
        $name = [string]$element.Current.Name
        if (-not [string]::IsNullOrWhiteSpace($name)) { $values.Add($name.Trim()) }
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
        foreach ($document in @($window.FindAll([Windows.Automation.TreeScope]::Descendants, $documentCondition))) {
            try {
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
            catch {
                # Not every UIA document exposes TextPattern; keep looking in the owned process.
            }
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

$readyParent = Split-Path -Parent $ReadyFile
if ($readyParent) { New-Item -ItemType Directory -Force -Path $readyParent | Out-Null }
$eventParent = Split-Path -Parent $EventsFile
if ($eventParent) { New-Item -ItemType Directory -Force -Path $eventParent | Out-Null }

$ready = [ordered]@{
    schema = "oxvba.excel-vba-oracle-guardian-ready.v1"
    run_id = $RunId
    excel_pid = $ExcelPid
    guardian_pid = $PID
    ready_utc = [DateTime]::UtcNow.ToString("o")
}
$ready | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $ReadyFile -Encoding utf8NoBOM

$seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$deadline = [DateTime]::UtcNow.AddSeconds($MaxSeconds)
while ([DateTime]::UtcNow -lt $deadline -and -not (Test-Path -LiteralPath $StopFile)) {
    if (-not (Get-Process -Id $ExcelPid -ErrorAction SilentlyContinue)) { break }
    $control = $null
    if (Test-Path -LiteralPath $ControlFile) {
        try { $control = Get-Content -Raw -LiteralPath $ControlFile | ConvertFrom-Json }
        catch { $control = $null }
    }
    if ($null -eq $control -or [string]::IsNullOrWhiteSpace([string]$control.operation_id)) {
        Start-Sleep -Milliseconds $PollMilliseconds
        continue
    }

    foreach ($window in @(Get-OwnedTopLevelWindows)) {
        $isModal = $false
        try {
            $windowPattern = $window.GetCurrentPattern([Windows.Automation.WindowPattern]::Pattern)
            $isModal = [bool]$windowPattern.Current.IsModal
        }
        catch { }
        $className = [string]$window.Current.ClassName
        if (-not $isModal -and $className -ne "#32770") { continue }

        $texts = @(Get-ElementStrings -Root $window -ControlType ([Windows.Automation.ControlType]::Text))
        $buttons = @(Get-ElementStrings -Root $window -ControlType ([Windows.Automation.ControlType]::Button))
        $policy = Get-ExcelOracleDialogPolicy -Phase ([string]$control.phase) -WindowTitle ([string]$window.Current.Name) -Texts $texts -Buttons $buttons
        $key = "$($control.operation_id)|$(Get-WindowIdentity $window)|$($policy.kind)|$($texts -join '|')"
        if (-not $seen.Add($key)) { continue }

        $vbeSelection = Get-VbeSelection
        $event = [ordered]@{
            schema = "oxvba.excel-vba-oracle-dialog-event.v1"
            run_id = $RunId
            operation_id = [string]$control.operation_id
            phase = [string]$control.phase
            excel_pid = $ExcelPid
            observed_process_id = [int]$window.Current.ProcessId
            observed_utc = [DateTime]::UtcNow.ToString("o")
            window_title = [string]$window.Current.Name
            window_class = $className
            window_handle = Get-WindowIdentity $window
            is_modal = $isModal
            dialog_text = $texts
            visible_buttons = $buttons
            selected_token = $vbeSelection.selected_token
            expanded_line = $vbeSelection.expanded_line
            classification = [string]$policy.kind
            disposition = [string]$policy.disposition
            dismissed_button = $null
        }
        if ($policy.disposition -eq "capture-then-dismiss" -and [bool]$control.allow_dismiss) {
            $event.dismissed_button = Invoke-OwnedDialogButton -Window $window -PreferredButtons @($policy.preferred_buttons)
        }
        ($event | ConvertTo-Json -Compress -Depth 8) | Add-Content -LiteralPath $EventsFile -Encoding utf8NoBOM
    }
    Start-Sleep -Milliseconds $PollMilliseconds
}
