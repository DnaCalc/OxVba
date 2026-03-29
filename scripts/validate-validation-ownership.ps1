$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $required = @(
        "docs/validation/VALIDATION_RESET_AUDIT_2026-03-29.md",
        "docs/validation/VALIDATION_CANONICAL_OWNERSHIP_MAP_2026-03-29.md",
        "docs/validation/CONFORMANCE_TOPIC_MATRIX_MAP_2026-03-29.csv",
        "docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv",
        "docs/validation/COM_EXTERNAL_INTEGRATION_VALIDATION_MATRIX_V1.csv",
        "docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv",
        "docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv",
        "docs/validation/VALIDATION_DERIVED_SUMMARY_LATEST.md"
    )

    foreach ($path in $required) {
        if (-not (Test-Path $path)) {
            throw "validate-validation-ownership: missing required artifact $path"
        }
    }

    $ownership = Get-Content "docs/validation/VALIDATION_CANONICAL_OWNERSHIP_MAP_2026-03-29.md" -Raw
    foreach ($needle in @(
        "docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv",
        "docs/validation/COM_EXTERNAL_INTEGRATION_VALIDATION_MATRIX_V1.csv",
        "docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv",
        "docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv",
        "docs/validation/CONFORMANCE_TOPIC_MATRIX_MAP_2026-03-29.csv"
    )) {
        if (-not $ownership.Contains($needle)) {
            throw "validate-validation-ownership: ownership map missing reference to $needle"
        }
    }

    $audit = Get-Content "docs/validation/VALIDATION_RESET_AUDIT_2026-03-29.md" -Raw
    foreach ($needle in @(
        "docs/evidence/language/COVERAGE_INDEX.csv",
        "docs/evidence/SPEC_CHECKLIST.md",
        "docs/evidence/conformance/CONFORMANCE_CHECK_TOPICS.csv",
        "docs/INITIAL_SCOPE_STATUS_2026-03-24.md"
    )) {
        if (-not $audit.Contains($needle)) {
            throw "validate-validation-ownership: audit ledger missing tracked artifact $needle"
        }
    }

    Write-Host "validate-validation-ownership: ok"
}
finally {
    Pop-Location
}
