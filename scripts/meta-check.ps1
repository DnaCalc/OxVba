param(
    [switch]$Fast,
    [switch]$Conformance,
    [switch]$Matrix
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Write-Host "[oxvba] docs-check"
& "$PSScriptRoot/docs-check.ps1"

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

Write-Host "[oxvba] meta check complete"
