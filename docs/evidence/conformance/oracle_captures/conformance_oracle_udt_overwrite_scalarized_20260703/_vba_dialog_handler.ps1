param([int]$ExcelPid, $StopFile, $LogFile, $DeadlineFile, [int]$PollMs = 200)
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class VbeWin32 {
    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool PostMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
    public const uint BM_CLICK  = 0x00F5;
    public const uint WM_CLOSE  = 0x0010;
}
"@

function Write-Log { param([string]$Msg)
    Add-Content -Path $LogFile -Value "$(Get-Date -Format o) $Msg" -Encoding UTF8
}

Write-Log "start excel_pid=$ExcelPid poll_ms=$PollMs stop_file=$StopFile"
$root = [System.Windows.Automation.AutomationElement]::RootElement

while ($true) {
    if (Test-Path $StopFile) { Write-Log "stop file observed"; break }
    if (-not (Get-Process -Id $ExcelPid -ErrorAction SilentlyContinue)) { Write-Log "excel exited"; break }

    # Deadline watchdog: kill Excel if a test exceeds its timeout.
    # The oracle writes the deadline (UTC ticks) to $DeadlineFile before
    # each Application.Run call and deletes it after the call returns.
    if ($DeadlineFile -and (Test-Path $DeadlineFile)) {
        try {
            $deadlineTicks = [long](Get-Content $DeadlineFile -Raw).Trim()
            if ([DateTime]::UtcNow.Ticks -gt $deadlineTicks) {
                Write-Log "deadline exceeded — killing excel pid=$ExcelPid"
                Stop-Process -Id $ExcelPid -Force -ErrorAction SilentlyContinue
                break
            }
        } catch {}
    }

    try {
        $pidCond  = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ExcelPid)
        $winCond  = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            [System.Windows.Automation.ControlType]::Window)
        $combined = New-Object System.Windows.Automation.AndCondition($pidCond, $winCond)
        $windows  = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $combined)

        for ($i = 0; $i -lt $windows.Count; $i++) {
            $w = $windows.Item($i)
            $title = ""
            try { $title = [string]$w.Current.Name } catch { continue }
            $sig = $title.ToLowerInvariant()

            if ($sig -notmatch 'microsoft visual basic') { continue }

            # Phase 1: find and click OK on the compile error child dialog
            # (child window named "Microsoft Visual Basic for Applications"
            #  with OK/Help buttons).
            $trueCond = [System.Windows.Automation.Condition]::TrueCondition
            $children = $w.FindAll([System.Windows.Automation.TreeScope]::Children, $trueCond)
            $clickedOK = $false
            for ($j = 0; $j -lt $children.Count; $j++) {
                $c = $children.Item($j)
                $cn = ""; try { $cn = $c.Current.Name } catch {}
                if ($cn -ne "Microsoft Visual Basic for Applications") { continue }

                $btnCond = New-Object System.Windows.Automation.PropertyCondition(
                    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
                    [System.Windows.Automation.ControlType]::Button)
                $btns = $c.FindAll([System.Windows.Automation.TreeScope]::Children, $btnCond)
                for ($k = 0; $k -lt $btns.Count; $k++) {
                    $btn = $btns.Item($k)
                    $bname = ""; try { $bname = $btn.Current.Name } catch {}
                    if ($bname -eq "OK") {
                        $bHwnd = [IntPtr]::new($btn.Current.NativeWindowHandle)
                        [void][VbeWin32]::PostMessage($bHwnd, [VbeWin32]::BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero)
                        Write-Log "clicked compile-error OK: window='$title'"
                        $clickedOK = $true
                        break
                    }
                }
                break
            }

            if ($clickedOK) {
                Start-Sleep -Milliseconds 500
            }

            # Phase 2: if VBE is now in break mode, close it via WM_CLOSE.
            # This terminates the VBA execution and makes Application.Run
            # return a COM error to the oracle runner.
            if ($sig -match '\[break\]' -or $clickedOK) {
                $vbeHwnd = [IntPtr]::new($w.Current.NativeWindowHandle)
                if ($vbeHwnd -ne [IntPtr]::Zero) {
                    [void][VbeWin32]::PostMessage($vbeHwnd, [VbeWin32]::WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
                    Write-Log "sent WM_CLOSE to VBE: window='$title'"
                }
            }
        }
    } catch {}

    Start-Sleep -Milliseconds $PollMs
}
Write-Log "exit"
