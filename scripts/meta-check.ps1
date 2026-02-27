param(
    [switch]$Fast,
    [switch]$Conformance,
    [switch]$Matrix,
    [switch]$Formal,
    [switch]$SkipPathStability
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    Write-Host "[oxvba] docs-check"
    & "$PSScriptRoot/docs-check.ps1"

    Write-Host "[oxvba] divergence-structure"
    & "$PSScriptRoot/validate-divergences.ps1"

    Write-Host "[oxvba] language-coverage"
    & "$PSScriptRoot/validate-language-coverage.ps1"

    if (-not $SkipPathStability) {
        Write-Host "[oxvba] path-stability"
        & "$PSScriptRoot/test-path-stability.ps1"
    }

    Write-Host "[oxvba] cargo fmt --check"
    cargo fmt --all --check

    Write-Host "[oxvba] cargo clippy"
    cargo clippy --workspace --all-targets -- -D warnings

    Write-Host "[oxvba] cargo test"
    cargo test --workspace

    if (-not $Fast) {
        Write-Host "[oxvba] cargo check"
        cargo check --workspace
    }

    if ($Conformance) {
        Write-Host "[oxvba] conformance"
        & "$PSScriptRoot/run-conformance.ps1"
    }

    if ($Matrix) {
        Write-Host "[oxvba] matrix"
        & "$PSScriptRoot/run-matrix.ps1"
    }

    if ($Formal) {
        Write-Host "[oxvba] formal (non-blocking)"
        & "$PSScriptRoot/run-formal.ps1"
    }

    Write-Host "[oxvba] meta check complete"
}
finally {
    Pop-Location
}
