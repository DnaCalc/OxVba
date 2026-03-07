$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $worksetPath = "docs/worksets/WORKSET_2026-03-03_PMR_FOLLOWUP_QUEUE_FROM_OBSERVATIONS.md"
    $divReadmePath = "docs/evidence/divergences/README.md"
    $foldbackPath = "docs/evidence/conformance/PMR_PROJECT_MODEL_ORACLE_FOLDBACK_2026-03-03.md"
    $oracleGatesPath = "docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv"
    $toleranceMatrixPath = "docs/evidence/conformance/PMR_HOST_IMPORT_TOLERANCE_MATRIX_V1.md"

    foreach ($path in @($worksetPath, $divReadmePath, $foldbackPath, $oracleGatesPath, $toleranceMatrixPath)) {
        if (-not (Test-Path $path)) {
            throw "missing PMR follow-up sync artifact: $path"
        }
    }

    $worksetText = Get-Content $worksetPath -Raw
    foreach ($needle in @(
        "PMR-FUP-004",
        "PMR-FUP-006",
        "DIV-0003",
        "DIV-0004",
        "CCT-040",
        "CCT-041"
    )) {
        if (-not $worksetText.Contains($needle)) {
            throw "PMR follow-up workset missing required linkage token '$needle'"
        }
    }

    $divReadmeText = Get-Content $divReadmePath -Raw
    foreach ($divId in @("DIV-0003", "DIV-0004")) {
        if (-not $divReadmeText.Contains($divId)) {
            throw "divergence index missing PMR divergence '$divId'"
        }
    }

    $foldbackText = Get-Content $foldbackPath -Raw
    foreach ($needle in @("ODG-038", "ODG-039", "DIV-0003", "DIV-0004")) {
        if (-not $foldbackText.Contains($needle)) {
            throw "PMR oracle foldback evidence missing '$needle'"
        }
    }

    $gates = Import-Csv $oracleGatesPath
    $odg038 = $gates | Where-Object { $_.gate_id -eq "ODG-038" } | Select-Object -First 1
    if ($null -eq $odg038) {
        throw "deferred oracle gates missing ODG-038 row"
    }
    if (([string]$odg038.topic_id).Trim() -ne "CCT-040") {
        throw "ODG-038 topic mismatch: expected CCT-040"
    }
    $odg038Status = ([string]$odg038.status).Trim()
    if ($odg038Status -eq "open") {
        if (([string]$odg038.owner_phase).Trim() -ne "events-story-completion") {
            throw "ODG-038 open-state owner_phase must be events-story-completion"
        }
        if (([string]$odg038.foldback_required).Trim().ToLowerInvariant() -ne "true") {
            throw "ODG-038 open-state must set foldback_required=true"
        }
        if ([string]::IsNullOrWhiteSpace(([string]$odg038.foldback_steps))) {
            throw "ODG-038 open-state must include foldback_steps"
        }
    }
    elseif ($odg038Status -ne "closed") {
        throw "ODG-038 status must be open or closed"
    }
    if (-not ([string]$odg038.notes).Contains("DIV-0003")) {
        throw "ODG-038 notes must reference DIV-0003"
    }

    $odg039 = $gates | Where-Object { $_.gate_id -eq "ODG-039" } | Select-Object -First 1
    if ($null -eq $odg039) {
        throw "deferred oracle gates missing ODG-039 row"
    }
    if (([string]$odg039.topic_id).Trim() -ne "CCT-041") {
        throw "ODG-039 topic mismatch: expected CCT-041"
    }
    $odg039Status = ([string]$odg039.status).Trim()
    if ($odg039Status -eq "open") {
        if (([string]$odg039.owner_phase).Trim() -ne "events-story-completion") {
            throw "ODG-039 open-state owner_phase must be events-story-completion"
        }
        if (([string]$odg039.foldback_required).Trim().ToLowerInvariant() -ne "true") {
            throw "ODG-039 open-state must set foldback_required=true"
        }
        if ([string]::IsNullOrWhiteSpace(([string]$odg039.foldback_steps))) {
            throw "ODG-039 open-state must include foldback_steps"
        }
    }
    elseif ($odg039Status -ne "closed") {
        throw "ODG-039 status must be open or closed"
    }
    if (-not ([string]$odg039.notes).Contains("DIV-0004")) {
        throw "ODG-039 notes must reference DIV-0004"
    }

    $matrixText = Get-Content $toleranceMatrixPath -Raw
    foreach ($needle in @(
        "PMR-TOL-001",
        "PMR-TOL-006",
        "module_unit_tolerates_unknown_header_attributes",
        "module_unit_rejects_malformed_attribute_line"
    )) {
        if (-not $matrixText.Contains($needle)) {
            throw "PMR tolerance matrix missing required anchor '$needle'"
        }
    }

    Write-Host "pmr-followup-sync: ok (ODG-038/039 + DIV-0003/0004 + FUP-004/006 linked; open/closed lifecycle accepted)"
}
finally {
    Pop-Location
}
