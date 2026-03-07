$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $manifestPath = "docs/evidence/diagnostics/PMR_EVENT_DIAGNOSTICS_V1.csv"
    if (-not (Test-Path $manifestPath)) {
        throw "missing PMR event diagnostic manifest: $manifestPath"
    }

    $rows = Import-Csv $manifestPath
    if ($rows.Count -eq 0) {
        throw "PMR event diagnostic manifest is empty: $manifestPath"
    }

    $canonicalIds = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
    $legacyIds = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
    foreach ($row in $rows) {
        $diagId = ([string]$row.diag_id).Trim()
        if ([string]::IsNullOrWhiteSpace($diagId)) {
            throw "manifest contains empty diag_id"
        }
        if (-not $canonicalIds.Add($diagId)) {
            throw "manifest contains duplicate diag_id: $diagId"
        }
        $legacyRaw = ([string]$row.legacy_ids).Trim()
        if (-not [string]::IsNullOrWhiteSpace($legacyRaw)) {
            foreach ($legacyId in ($legacyRaw -split ';')) {
                $trimmed = $legacyId.Trim()
                if (-not [string]::IsNullOrWhiteSpace($trimmed)) {
                    $legacyIds.Add($trimmed) | Out-Null
                }
            }
        }
    }

    $activeFiles = @(
        "docs/DIAGNOSTIC_TAXONOMY.md",
        "docs/spec/PROJECT_MODULE_REFERENCE_CONFORMANCE_V1.md",
        "docs/spec/PROJECT_MODULE_REFERENCE_HAL_INTEGRATION_V1.md",
        "docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL_V2.md",
        "docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv",
        "docs/evidence/conformance/PROJECT_INTEGRATION_DEFERRED_UNCERTAINTIES_V1.md",
        "docs/evidence/divergences/DIV-0003.md",
        "docs/evidence/divergences/DIV-0004.md",
        "docs/evidence/language/PMR_CLASS_COM_ALIGNMENT_A1_A5_2026-03-03.md",
        "conformance/integration/catalog.psv"
    )

    $historicalRoots = @(
        "docs/evidence/conformance/oracle_captures/"
    )

    $fileText = @{}
    foreach ($path in $activeFiles) {
        if (-not (Test-Path $path)) {
            throw "missing active PMR/event surface file: $path"
        }
        $fileText[$path] = Get-Content $path -Raw
    }

    foreach ($diagId in ($canonicalIds | Sort-Object)) {
        $found = $false
        foreach ($path in $activeFiles) {
            if ($fileText[$path].Contains($diagId)) {
                $found = $true
                break
            }
        }
        if (-not $found) {
            throw "canonical PMR/event diagnostic id is not referenced by active surfaces: $diagId"
        }
    }

    foreach ($legacyId in ($legacyIds | Sort-Object)) {
        $hits = @()
        foreach ($path in $activeFiles) {
            if ($fileText[$path].Contains($legacyId)) {
                $hits += $path
            }
        }
        if ($hits.Count -gt 0) {
            $joined = ($hits -join ", ")
            throw "legacy PMR/event diagnostic id leaked into active surfaces ($legacyId): $joined"
        }
    }

    Write-Host "pmr-event-diagnostic-sync: ok (active surfaces clean; historical roots excluded by default: $($historicalRoots -join ', '))"
}
finally {
    Pop-Location
}
