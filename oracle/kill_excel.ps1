<#
  kill_excel.ps1 — hard recovery for a wedged oracle run.

  Forcibly terminates every Excel process (and any orphaned pwsh COM host that is
  blocked on a modal dialog). Use this when run_oracle.ps1 times out: a VBA probe
  hit a modal "Run-time error" dialog the harness could not suppress, so the COM
  call is blocked indefinitely. Killing the process is the reliable backstop.
#>
$killed = @()
foreach ($name in 'EXCEL') {
  Get-Process -Name $name -ErrorAction SilentlyContinue | ForEach-Object {
    $killed += "$($_.ProcessName)($($_.Id))"
    try { Stop-Process -Id $_.Id -Force -ErrorAction Stop } catch {}
  }
}
if ($killed.Count -eq 0) { "no Excel processes running" } else { "killed: " + ($killed -join ', ') }
