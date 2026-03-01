$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $coveragePath = "docs/evidence/language/COVERAGE_INDEX.csv"
    $libraryPath = "docs/evidence/runtime/LIBRARY_CHECKLIST.csv"

    foreach ($path in @($coveragePath, $libraryPath)) {
        if (-not (Test-Path $path)) {
            throw "missing coverage artifact: $path"
        }
    }

    $staleTokens = @(
        "TODO remove",
        "obsolete",
        "legacy projection",
        "removed subset"
    )

    $missingEvidence = @()
    $staleMentions = @()

    $coverageRows = Import-Csv $coveragePath
    foreach ($row in $coverageRows) {
        $evidence = [string]$row.evidence
        if (-not [string]::IsNullOrWhiteSpace($evidence)) {
            if (-not (Test-Path $evidence)) {
                $missingEvidence += "coverage:$($row.feature_area):$evidence"
            }
        }

        $notes = [string]$row.notes
        foreach ($token in $staleTokens) {
            if ($notes.IndexOf($token, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
                $staleMentions += "coverage:$($row.feature_area):$token"
            }
        }
    }

    $libraryRows = Import-Csv $libraryPath
    foreach ($row in $libraryRows) {
        $evidence = [string]$row.evidence
        if (-not [string]::IsNullOrWhiteSpace($evidence)) {
            if (-not (Test-Path $evidence)) {
                $missingEvidence += "library:$($row.library_family):$evidence"
            }
        }

        $notes = [string]$row.notes
        foreach ($token in $staleTokens) {
            if ($notes.IndexOf($token, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
                $staleMentions += "library:$($row.library_family):$token"
            }
        }
    }

    if ($missingEvidence.Count -gt 0) {
        throw "missing evidence references: $($missingEvidence -join '; ')"
    }

    if ($staleMentions.Count -gt 0) {
        throw "stale note markers found: $($staleMentions -join '; ')"
    }

    Write-Host "coverage-notes: ok (coverage_rows=$($coverageRows.Count) library_rows=$($libraryRows.Count))"
}
finally {
    Pop-Location
}
