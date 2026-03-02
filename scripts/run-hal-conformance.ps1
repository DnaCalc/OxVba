param(
    [string]$OutputDir = "docs/evidence/hal",
    [switch]$SkipTests
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $repoRoot
try {
    Write-Host "[hal] validating clause catalog drift guard"
    & (Join-Path $PSScriptRoot "check-hal-clause-drift.ps1")

    if (-not $SkipTests) {
        Write-Host "[hal] running crate tests (oxvba-hal)"
        cargo test -p oxvba-hal
    }

    Write-Host "[hal] generating conformance artifacts"
    cargo run -q -p oxvba-hal --bin hal-conformance -- --output-dir $OutputDir
}
finally {
    Pop-Location
}
