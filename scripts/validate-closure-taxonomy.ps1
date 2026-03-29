$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $matrixFiles = @(
        "docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv",
        "docs/validation/COM_EXTERNAL_INTEGRATION_VALIDATION_MATRIX_V1.csv",
        "docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv",
        "docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv"
    )

    $allowed = @("planned", "in-progress", "implemented-subset", "implemented-full", "verified", "archived")

    foreach ($path in $matrixFiles) {
        $rows = Import-Csv $path
        foreach ($row in $rows) {
            if (-not $allowed.Contains($row.truth_state)) {
                throw "validate-closure-taxonomy: $path has invalid truth_state '$($row.truth_state)' for row '$($row.feature_id)'"
            }
            if ($row.truth_state -eq "implemented") {
                throw "validate-closure-taxonomy: $path uses forbidden bare truth_state 'implemented' for row '$($row.feature_id)'"
            }
        }
    }

    Write-Host "validate-closure-taxonomy: ok"
}
finally {
    Pop-Location
}
