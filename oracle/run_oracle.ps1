<#
  run_oracle.ps1 — drive Excel as a VBA oracle.

  Injects a module of `Function PROBE_xxx() As String` probes into a throwaway
  workbook, runs each via Application.Run, and records its returned result string.
  Every probe is self-contained (it captures its own Err state and returns a string),
  so a probe never leaks an UNHANDLED run-time error to the COM boundary — which is
  what would otherwise pop the modal "Run-time error" dialog and wedge automation.

  Recovery: this script cleans up Excel in `finally`. If a probe still wedges Excel
  (an unexpected modal), the *caller* times this script out and runs
  `kill_excel.ps1` — the process kill is the hard backstop.

  Usage: pwsh run_oracle.ps1 [-ProbesFile probes.bas] [-OutFile results/oracle_results.json]
#>
param(
  [string]$ProbesFile = "$PSScriptRoot/probes.bas",
  [string]$OutFile = "$PSScriptRoot/results/oracle_results.json"
)
$ErrorActionPreference = 'Stop'

# --- prep: clean slate, so a prior probe crash can't wedge this run ---
# A VBA crash leaves Excel in post-crash "safe mode" (macros disabled) with a
# Resiliency\DisabledItems / DocumentRecovery state; clear it and kill stray Excel.
try { Get-Process EXCEL -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue } catch {}
try { Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Office\16.0\Excel\Security' -Name VBAWarnings -Value 1 -Type DWord -ErrorAction Stop } catch {}
$resi = 'HKCU:\Software\Microsoft\Office\16.0\Excel\Resiliency'
foreach ($sub in 'DisabledItems', 'StartupItems', 'DocumentRecovery') {
  $p = Join-Path $resi $sub
  if (Test-Path $p) { Remove-Item $p -Recurse -Force -ErrorAction SilentlyContinue }
}

$src = Get-Content -Raw -LiteralPath $ProbesFile
# Strip the exported-file `Attribute VB_Name = ...` header: it is file-import metadata,
# not code, and AddFromString into an already-named module rejects/mis-parses it.
$src = ($src -split "`r?`n" | Where-Object { $_ -notmatch '^\s*Attribute\s+VB_Name\b' }) -join "`n"
# VBA's CodeModule wants CRLF line breaks; normalize any lone LF.
$src = $src -replace "`r`n", "`n" -replace "`n", "`r`n"

# Discover probe names (Function PROBE_xxx). @() forces an array even for one match
# (else a single name is a string and `$names[0]` yields its first character).
$names = @([regex]::Matches($src, '(?im)^\s*(?:Public\s+|Private\s+)?Function\s+(PROBE_\w+)\s*\(') |
  ForEach-Object { $_.Groups[1].Value })

# NOTE: a probe whose error handler does Resume/Resume Next after a CROSS-CALL propagated
# error must NOT be the frame Application.Run invokes directly (the COM top frame) — that
# crashes Excel (a COM-invocation artifact, not real VBA). Such probes wrap the risky
# logic in an inner Private Function so the handler runs one frame down. See probes.bas.

$results = [ordered]@{}
$xl = $null
try {
  $xl = New-Object -ComObject Excel.Application
  $xl.Visible = $false
  $xl.DisplayAlerts = $false
  $xl.EnableEvents = $false
  $wb = $xl.Workbooks.Add()
  $comp = $wb.VBProject.VBComponents.Add(1) # 1 = vbext_ct_StdModule
  $comp.Name = "OracleProbes"
  $comp.CodeModule.AddFromString($src)
  foreach ($n in $names) {
    try {
      $r = $xl.Run($n)
      $results[$n] = [string]$r
    } catch {
      # A COM exception here means the probe leaked an error to the boundary (a probe
      # bug) — record it rather than letting it abort the batch.
      $results[$n] = "RUNERROR: " + $_.Exception.Message
    }
  }
  $wb.Close($false)
} catch {
  $results['__HARNESS__'] = "ERROR: " + $_.Exception.Message
} finally {
  if ($xl) {
    try { $xl.Quit() } catch {}
    [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($xl)
  }
  [GC]::Collect(); [GC]::WaitForPendingFinalizers()
}

$dir = Split-Path -Parent $OutFile
if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
$results | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 -LiteralPath $OutFile
$results.GetEnumerator() | ForEach-Object { "$($_.Key) => $($_.Value)" }
