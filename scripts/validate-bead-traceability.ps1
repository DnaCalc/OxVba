$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $tracePath = "docs/validation/MATRIX_BEAD_TRACEABILITY_2026-03-29.csv"
    if (-not (Test-Path $tracePath)) {
        throw "validate-bead-traceability: missing $tracePath"
    }
    if (-not (Test-Path ".beads/issues.jsonl")) {
        throw "validate-bead-traceability: missing .beads/issues.jsonl"
    }

    $rows = Import-Csv $tracePath
    $issues = Get-Content ".beads/issues.jsonl" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | ForEach-Object { $_ | ConvertFrom-Json }
    $issueIds = @($issues | ForEach-Object { $_.id })

    foreach ($row in $rows) {
        if ($issueIds -notcontains $row.bead_id) {
            throw "validate-bead-traceability: bead id '$($row.bead_id)' does not exist"
        }
        if ($issueIds -notcontains $row.parent_epic) {
            throw "validate-bead-traceability: parent epic '$($row.parent_epic)' does not exist"
        }

        $matrixFiles = @($row.matrix_file -split ';' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
        foreach ($matrixFile in $matrixFiles) {
            if (-not (Test-Path $matrixFile)) {
                throw "validate-bead-traceability: missing matrix file '$matrixFile' referenced by bead '$($row.bead_id)'"
            }
        }
    }

    Write-Host "validate-bead-traceability: ok (rows=$($rows.Count))"
}
finally {
    Pop-Location
}
