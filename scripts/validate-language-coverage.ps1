$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $path = "docs/evidence/language/COVERAGE_INDEX.csv"
    if (-not (Test-Path $path)) {
        throw "Missing language coverage index: $path"
    }

    $rows = Import-Csv $path
    if (-not $rows -or $rows.Count -eq 0) {
        throw "Language coverage index is empty: $path"
    }

    $required = @("feature_area", "construct", "status", "phase_scope", "evidence", "notes")
    $header = $rows[0].PSObject.Properties.Name
    foreach ($name in $required) {
        if (-not ($header -contains $name)) {
            throw "Language coverage index missing required column '$name'"
        }
    }

    $allowedStatus = @("implemented", "partial", "planned", "derived-summary")
    $seen = @{}
    foreach ($row in $rows) {
        if ([string]::IsNullOrWhiteSpace($row.feature_area) -or [string]::IsNullOrWhiteSpace($row.construct)) {
            throw "Language coverage row has empty feature_area/construct"
        }
        if (-not ($allowedStatus -contains $row.status)) {
            throw "Language coverage row has unsupported status '$($row.status)' for $($row.feature_area)/$($row.construct)"
        }
        if ([string]::IsNullOrWhiteSpace($row.phase_scope)) {
            throw "Language coverage row missing phase_scope for $($row.feature_area)/$($row.construct)"
        }

        $key = "$($row.feature_area)|$($row.construct)"
        if ($seen.ContainsKey($key)) {
            throw "Duplicate language coverage row: $key"
        }
        $seen[$key] = $true
    }

    Write-Host "language-coverage: ok ($($rows.Count) rows)"
}
finally {
    Pop-Location
}
