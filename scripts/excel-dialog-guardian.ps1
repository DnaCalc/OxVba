param(
    [Parameter(Mandatory = $true)]
    [int]$ExcelPid,
    [Parameter(Mandatory = $true)]
    [string]$StopFile,
    [Parameter(Mandatory = $true)]
    [string]$LogFile,
    [int]$PollMs = 250,
    [int]$MaxSeconds = 1800
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

function Write-GuardianLog {
    param([string]$Message)
    $timestamp = (Get-Date).ToString("yyyy-MM-ddTHH:mm:ss.fffK")
    Add-Content -Path $LogFile -Value "$timestamp $Message" -Encoding UTF8
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
            $name = $items.Item($i).Current.Name
            if (-not [string]::IsNullOrWhiteSpace($name)) {
                $names += $name.Trim()
            }
        }
    } catch {
        # Ignore stale/invalid UIA trees; caller retries on next poll.
    }
    $names
}

function Invoke-AutomationElement {
    param([System.Windows.Automation.AutomationElement]$Element)

    try {
        $pattern = $Element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        if ($pattern -ne $null) {
            $pattern.Invoke()
            return $true
        }
    } catch {
        # Fall through to legacy pattern.
    }

    try {
        $pattern = $Element.GetCurrentPattern([System.Windows.Automation.LegacyIAccessiblePattern]::Pattern)
        if ($pattern -ne $null) {
            $pattern.DoDefaultAction()
            return $true
        }
    } catch {
        # No invokable pattern.
    }

    $false
}

function Try-HandleSecurityPrompt {
    param([System.Windows.Automation.AutomationElement]$Window)

    $windowName = ""
    try {
        $windowName = [string]$Window.Current.Name
    } catch {
        return $false
    }

    $textNames = Get-DescendantNamesByControlType -Root $Window -ControlType ([System.Windows.Automation.ControlType]::Text)
    $buttonNames = Get-DescendantNamesByControlType -Root $Window -ControlType ([System.Windows.Automation.ControlType]::Button)
    $signal = ("$windowName " + ($textNames -join " ") + " " + ($buttonNames -join " ")).ToLowerInvariant()

    $securityKeywords = @(
        "macro",
        "security",
        "add-in",
        "addin",
        "enable content",
        "enable macros",
        "trusted",
        "trust",
        "publisher",
        "have been disabled",
        "content has been disabled"
    )
    $isSecurityPrompt = $false
    foreach ($keyword in $securityKeywords) {
        if ($signal.Contains($keyword)) {
            $isSecurityPrompt = $true
            break
        }
    }
    if (-not $isSecurityPrompt) {
        return $false
    }

    $cond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Button
    )
    $buttons = $Window.FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond)
    if ($buttons.Count -eq 0) {
        return $false
    }

    $preferredPatterns = @(
        '^enable macros$',
        '^enable content$',
        '^enable this add-?in$',
        '^enable$',
        '^allow$',
        '^open$',
        '^run$',
        '^yes$',
        '^ok$'
    )

    for ($p = 0; $p -lt $preferredPatterns.Count; $p++) {
        $regex = [regex]::new($preferredPatterns[$p], [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
        for ($i = 0; $i -lt $buttons.Count; $i++) {
            $button = $buttons.Item($i)
            $buttonName = [string]$button.Current.Name
            if ([string]::IsNullOrWhiteSpace($buttonName)) {
                continue
            }
            if (-not $regex.IsMatch($buttonName.Trim())) {
                continue
            }
            if (Invoke-AutomationElement -Element $button) {
                Write-GuardianLog "handled dialog window='$windowName' button='$buttonName'"
                return $true
            }
        }
    }

    Write-GuardianLog "detected security-like dialog but no preferred action button found; window='$windowName' buttons='$($buttonNames -join "|")'"
    $false
}

Write-GuardianLog "start excel_pid=$ExcelPid poll_ms=$PollMs max_seconds=$MaxSeconds stop_file=$StopFile"

$deadline = (Get-Date).AddSeconds($MaxSeconds)
$root = [System.Windows.Automation.AutomationElement]::RootElement

while ((Get-Date) -lt $deadline) {
    if (Test-Path $StopFile) {
        Write-GuardianLog "stop file observed; exiting"
        break
    }

    if (-not (Get-Process -Id $ExcelPid -ErrorAction SilentlyContinue)) {
        Write-GuardianLog "excel process exited; guardian stopping"
        break
    }

    try {
        $pidCond = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
            $ExcelPid
        )
        $windowCond = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            [System.Windows.Automation.ControlType]::Window
        )
        $combined = New-Object System.Windows.Automation.AndCondition($pidCond, $windowCond)
        $windows = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $combined)
        for ($i = 0; $i -lt $windows.Count; $i++) {
            $window = $windows.Item($i)
            [void](Try-HandleSecurityPrompt -Window $window)
        }
    } catch {
        Write-GuardianLog "poll error: $($_.Exception.Message)"
    }

    Start-Sleep -Milliseconds $PollMs
}

Write-GuardianLog "exit"
