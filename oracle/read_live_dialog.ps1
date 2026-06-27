<#
  read_live_dialog.ps1 — read (and optionally dismiss) a currently-open VBE
  compile/syntax error dialog via UI Automation. Reports the message text, the dialog
  buttons, and the highlighted failing line from the code pane (gist technique).
#>
param([switch]$Dismiss)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$root = [System.Windows.Automation.AutomationElement]::RootElement
$wins = $root.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
$found = $false
foreach ($win in $wins) {
  $nm = $win.Current.Name
  if ($nm -notlike '*Visual Basic*' -and $nm -notlike '*Microsoft Excel*') { continue }
  "WINDOW=[$nm]"
  # message text
  $tc = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Text)
  $texts = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, $tc)
  $msg = ($texts | ForEach-Object { $_.Current.Name } | Where-Object { $_ -and $_.Trim() }) -join ' || '
  if ($msg) { "  MSG=[$msg]"; $found = $true }
  # buttons
  $bc = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Button)
  $btns = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, $bc)
  $bnames = ($btns | ForEach-Object { $_.Current.Name }) -join ','
  if ($bnames) { "  BUTTONS=[$bnames]" }
  # highlighted failing line from a code pane (Document control)
  try {
    $dc = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Document)
    $docs = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, $dc)
    foreach ($doc in $docs) {
      $tp = $doc.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
      $sel = $tp.GetSelection()[0].Clone()
      $sel.ExpandToEnclosingUnit([System.Windows.Automation.TextUnit]::Line) | Out-Null
      $line = $sel.GetText(2000)
      if ($line -and $line.Trim()) { "  FAILING_LINE=[" + $line.Trim() + "]" }
    }
  } catch {}
  if ($Dismiss) {
    $ok = $btns | Where-Object { $_.Current.Name -in @('OK', 'End', 'Cancel') } | Select-Object -First 1
    if ($ok) { try { $ok.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke(); "  DISMISSED via $($ok.Current.Name)" } catch { "  dismiss failed: $($_.Exception.Message)" } }
  }
}
if (-not $found) { "no Visual Basic / Excel dialog window found" }
