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
