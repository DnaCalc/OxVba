<#
  diag_read_dialog.ps1 — inject a probes file (runner-style: strip Attribute, CRLF,
  append per-probe wrappers), then Run the first wrapper while a background runspace
  watches for the modal "Microsoft Visual Basic" compile/run-error dialog, reads its
  text + buttons, and dismisses it (so the blocked Run returns). Prints the dialog text
  = the actual compile error. Based on the UI Automation technique from govert's gist.
#>
param([string]$ProbesFile = "$PSScriptRoot/probes_min.bas")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes

Get-Process EXCEL -EA SilentlyContinue | Stop-Process -Force -EA SilentlyContinue
$resi = 'HKCU:\Software\Microsoft\Office\16.0\Excel\Resiliency'
foreach ($s in 'DisabledItems', 'StartupItems', 'DocumentRecovery') { $p = Join-Path $resi $s; if (Test-Path $p) { Remove-Item $p -Recurse -Force -EA SilentlyContinue } }
Set-ItemProperty 'HKCU:\Software\Microsoft\Office\16.0\Excel\Security' -Name VBAWarnings -Value 1 -Type DWord

$src = Get-Content -Raw -LiteralPath $ProbesFile
$src = ($src -split "`r?`n" | Where-Object { $_ -notmatch '^\s*Attribute\s+VB_Name\b' }) -join "`n"
$src = $src -replace "`r`n", "`n" -replace "`n", "`r`n"
$names = @([regex]::Matches($src, '(?im)^\s*(?:Public\s+|Private\s+)?Function\s+(PROBE_\w+)\s*\(') | ForEach-Object { $_.Groups[1].Value })
$wrappers = $names | ForEach-Object { "Function __w_$_() As String`r`n    __w_$_ = $_()`r`nEnd Function" }
$src = $src + "`r`n" + ($wrappers -join "`r`n")
$first = $names[0]

$xl = New-Object -ComObject Excel.Application
$xl.Visible = $true
$xl.DisplayAlerts = $false
$wb = $xl.Workbooks.Add()
$c = $wb.VBProject.VBComponents.Add(1)
$c.CodeModule.AddFromString($src)

# Background watcher: find + read + dismiss the VB modal dialog.
$rs = [runspacefactory]::CreateRunspace(); $rs.Open()
$w = [powershell]::Create(); $w.Runspace = $rs
[void]$w.AddScript({
    Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
    $deadline = (Get-Date).AddSeconds(25)
    while ((Get-Date) -lt $deadline) {
      Start-Sleep -Milliseconds 300
      try {
        $root = [System.Windows.Automation.AutomationElement]::RootElement
        $wins = $root.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
        foreach ($win in $wins) {
          $nm = $win.Current.Name
          if ($nm -like '*Visual Basic*') {
            $tc = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Text)
            $texts = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, $tc)
            $msg = ($texts | ForEach-Object { $_.Current.Name }) -join ' || '
            $bc = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Button)
            $btns = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, $bc)
            $bnames = ($btns | ForEach-Object { $_.Current.Name }) -join ','
            $ok = $btns | Where-Object { $_.Current.Name -in @('OK', 'End', 'Cancel') } | Select-Object -First 1
            if ($ok) { try { $ok.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() } catch {} }
            return "WIN=[$nm] BTNS=[$bnames] MSG=[$msg]"
          }
        }
      } catch {}
    }
    return "NO_DIALOG_FOUND"
  })
$h = $w.BeginInvoke()
Start-Sleep -Milliseconds 400
$runRes = try { [string]$xl.Run("__w_$first") } catch { "RUNERR:" + $_.Exception.Message }
$dialog = $w.EndInvoke($h) -join "`n"
"FIRST_PROBE=$first"
"RUN_RESULT=$runRes"
"DIALOG=$dialog"
try { $wb.Close($false) } catch {}
try { $xl.Quit() } catch {}
[void][Runtime.InteropServices.Marshal]::ReleaseComObject($xl)
Get-Process EXCEL -EA SilentlyContinue | Stop-Process -Force -EA SilentlyContinue
