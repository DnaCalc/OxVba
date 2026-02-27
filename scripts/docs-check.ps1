$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $required = @(
        "CHARTER.md",
        "OPERATIONS.md",
        "MACH1000_PLAN.md",
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
