<#
.SYNOPSIS
    v620 readiness dashboard — aggregates all gate, conformance, formal, and divergence statuses.

.DESCRIPTION
    Wires together existing gate/conformance/formal scripts into a single terminal report
    for v620 gate evaluation. Produces a summary of:
    - CCT topic statuses (48 topics)
    - ODG oracle gate statuses (46 gates)
    - DG formal deferred gate statuses
    - Active divergence records
    - Miri coverage summary
    - Test suite baseline

.PARAMETER Fast
    Skip cargo test (assumes recent passing run).

.EXAMPLE
    ./scripts/v620-readiness-report.ps1
    ./scripts/v620-readiness-report.ps1 -Fast
#>
[CmdletBinding()]
param(
    [switch]$Fast
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " OxVba v620 Readiness Report" -ForegroundColor Cyan
Write-Host " Generated: $(Get-Date -Format 'yyyy-MM-ddTHH:mm:ssZ')" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# ── 1. Oracle Gate Status (ODG) ──
Write-Host "--- Oracle Gates (ODG) ---" -ForegroundColor Yellow
$odgPath = Join-Path $root "docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv"
if (Test-Path $odgPath) {
    $odgRows = Import-Csv $odgPath
    $odgTotal = $odgRows.Count
    $odgOpen = ($odgRows | Where-Object { $_.status -eq 'open' }).Count
    $odgClosed = ($odgRows | Where-Object { $_.status -eq 'closed' }).Count
    $odgDeferred = $odgTotal - $odgOpen - $odgClosed
    Write-Host "  Total:    $odgTotal"
    Write-Host "  Closed:   $odgClosed" -ForegroundColor Green
    Write-Host "  Open:     $odgOpen" -ForegroundColor $(if ($odgOpen -gt 0) { 'Red' } else { 'Green' })
    Write-Host "  Deferred: $odgDeferred" -ForegroundColor $(if ($odgDeferred -gt 0) { 'Yellow' } else { 'Green' })

    # List open gates
    $openGates = $odgRows | Where-Object { $_.status -eq 'open' }
    if ($openGates) {
        Write-Host ""
        Write-Host "  Open gates:" -ForegroundColor Red
        foreach ($g in $openGates) {
            Write-Host "    $($g.gate_id): $($g.domain) — $($g.unblock_condition)"
        }
    }
} else {
    Write-Host "  [MISSING] $odgPath" -ForegroundColor Red
}
Write-Host ""

# ── 2. Formal Deferred Gates (DG) ──
Write-Host "--- Formal Deferred Gates (DG) ---" -ForegroundColor Yellow
$dgPath = Join-Path $root "docs/evidence/formal/DEFERRED_GATES.md"
if (Test-Path $dgPath) {
    $dgContent = Get-Content $dgPath -Raw
    $dgFolded = ([regex]::Matches($dgContent, '\| dg-folded \|')).Count
    $dgPass = ([regex]::Matches($dgContent, '\| dg-pass \|')).Count
    $dgFail = ([regex]::Matches($dgContent, '\| dg-fail \|')).Count
    $dgRunning = ([regex]::Matches($dgContent, '\| dg-running \|')).Count
    $dgDeferred = ([regex]::Matches($dgContent, '\| dg-deferred \|')).Count
    $dgNotStarted = ([regex]::Matches($dgContent, '\| dg-not-started \|')).Count
    $dgTotal = $dgFolded + $dgPass + $dgFail + $dgRunning + $dgDeferred + $dgNotStarted

    Write-Host "  Total:       $dgTotal"
    Write-Host "  Folded:      $dgFolded" -ForegroundColor Green
    Write-Host "  Pass:        $dgPass" -ForegroundColor Green
    Write-Host "  Fail:        $dgFail" -ForegroundColor $(if ($dgFail -gt 0) { 'Red' } else { 'Green' })
    Write-Host "  Running:     $dgRunning" -ForegroundColor $(if ($dgRunning -gt 0) { 'Yellow' } else { 'Green' })
    Write-Host "  Deferred:    $dgDeferred" -ForegroundColor $(if ($dgDeferred -gt 0) { 'Yellow' } else { 'Green' })
    Write-Host "  Not Started: $dgNotStarted" -ForegroundColor $(if ($dgNotStarted -gt 0) { 'Yellow' } else { 'Green' })
} else {
    Write-Host "  [MISSING] $dgPath" -ForegroundColor Red
}
Write-Host ""

# ── 3. Divergence Records ──
Write-Host "--- Active Divergences ---" -ForegroundColor Yellow
$divDir = Join-Path $root "docs/evidence/divergences"
if (Test-Path $divDir) {
    $divFiles = Get-ChildItem -Path $divDir -Filter "DIV-*.md" -ErrorAction SilentlyContinue
    $divCount = $divFiles.Count
    Write-Host "  Active divergence records: $divCount"
    foreach ($df in $divFiles) {
        Write-Host "    $($df.BaseName)"
    }
} else {
    Write-Host "  [NO DIR] $divDir"
}
Write-Host ""

# ── 4. Conformance Test Corpus ──
Write-Host "--- Conformance Tests ---" -ForegroundColor Yellow
$basDir = Join-Path $root "conformance/tests"
if (Test-Path $basDir) {
    $basCount = (Get-ChildItem -Path $basDir -Filter "*.bas").Count
    Write-Host "  .bas files: $basCount"
}
$intDir = Join-Path $root "conformance/integration/projects"
if (Test-Path $intDir) {
    $intCount = (Get-ChildItem -Path $intDir -Directory).Count
    Write-Host "  Integration fixtures: $intCount"
}
Write-Host ""

# ── 5. Test Suite Baseline ──
Write-Host "--- Test Suite ---" -ForegroundColor Yellow
if (-not $Fast) {
    Write-Host "  Running cargo test --workspace..." -ForegroundColor Gray
    $testOutput = & cargo test --workspace 2>&1 | Select-String "^test result:"
    foreach ($line in $testOutput) {
        Write-Host "  $line"
    }
} else {
    Write-Host "  [SKIPPED — use without -Fast to run tests]" -ForegroundColor Gray
}
Write-Host ""

# ── 6. Miri Coverage ──
Write-Host "--- Miri Coverage ---" -ForegroundColor Yellow
Write-Host "  Covered crates: oxvba-runtime (full), oxvba-jit (full), oxvba-vm (semantics+broadword), oxvba-com (miri_variant_mock)"
Write-Host "  Script: scripts/run-miri.sh"
Write-Host ""

# ── 7. CI Lanes ──
Write-Host "--- CI Lanes ---" -ForegroundColor Yellow
$ciPath = Join-Path $root ".github/workflows/ci.yml"
if (Test-Path $ciPath) {
    $ciContent = Get-Content $ciPath -Raw
    $jobCount = ([regex]::Matches($ciContent, '^\s+\w[\w-]*:', [System.Text.RegularExpressions.RegexOptions]::Multiline)).Count
    Write-Host "  CI jobs defined: ~$jobCount"
    if ($ciContent -match 'ubuntu-latest') {
        Write-Host "  Linux lane: present" -ForegroundColor Green
    } else {
        Write-Host "  Linux lane: missing" -ForegroundColor Red
    }
} else {
    Write-Host "  [MISSING] $ciPath" -ForegroundColor Red
}
Write-Host ""

# ── Summary ──
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Summary" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

$blockers = @()
if ($odgOpen -and $odgOpen -gt 0) { $blockers += "$odgOpen open ODGs" }
if ($dgFail -and $dgFail -gt 0) { $blockers += "$dgFail failing DGs" }
if ($dgRunning -and $dgRunning -gt 0) { $blockers += "$dgRunning running DGs" }

if ($blockers.Count -eq 0) {
    Write-Host "  No critical blockers detected." -ForegroundColor Green
} else {
    Write-Host "  Blockers: $($blockers -join ', ')" -ForegroundColor Red
}

Write-Host ""
Write-Host "Report complete." -ForegroundColor Cyan
