$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $required = @(
        "CHARTER.md",
        "OPERATIONS.md",
        "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md",
        "docs/ARCHITECTURE.md",
        "docs/AUTORUN_STATE.md",
        "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
        "docs/validation/IDEAL_LEGACY_BEAD_MIGRATION_V1.csv",
        "README.md",
        "Cargo.toml"
    )

    foreach ($file in $required) {
        if (-not (Test-Path $file)) {
            throw "Missing required file: $file"
        }
    }

    Write-Host "docs-check: required docs are present"
}
finally {
    Pop-Location
}
