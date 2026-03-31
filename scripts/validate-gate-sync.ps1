$ErrorActionPreference = "Stop"

$repoRoot = Join-Path $PSScriptRoot ".."
$agentsPath = Join-Path $repoRoot "AGENTS.md"
$autorunPath = Join-Path $repoRoot "docs/AUTORUN_STATE.md"
$docsReadmePath = Join-Path $repoRoot "docs/README.md"

$agents = Get-Content $agentsPath -Raw
$autorun = Get-Content $autorunPath -Raw
$docsReadme = Get-Content $docsReadmePath -Raw

$agentsGate = [regex]::Match($agents, 'Current required terminal gate:\s*`?(v\d+)`?').Groups[1].Value
$autorunGate = [regex]::Match($autorun, 'Terminal gate:\s*`?(v\d+)`?').Groups[1].Value
$docsGate = [regex]::Match($docsReadme, 'Current target is .* gate `?(v\d+)`?', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase).Groups[1].Value

if ([string]::IsNullOrWhiteSpace($agentsGate)) {
    throw "gate-sync: unable to parse terminal gate from AGENTS.md"
}
if ([string]::IsNullOrWhiteSpace($autorunGate)) {
    throw "gate-sync: unable to parse terminal gate from docs/AUTORUN_STATE.md"
}
if ([string]::IsNullOrWhiteSpace($docsGate)) {
    throw "gate-sync: unable to parse terminal gate from docs/README.md"
}

$unique = @($agentsGate, $autorunGate, $docsGate) | Select-Object -Unique
if ($unique.Count -ne 1) {
    throw "gate-sync mismatch: AGENTS=$agentsGate AUTORUN=$autorunGate docs/README=$docsGate"
}

Write-Host "gate-sync: ok ($agentsGate)"
