param(
    [switch]$Fast,
    [switch]$Conformance,
    [switch]$Matrix,
    [switch]$Formal,
    [switch]$SkipPathStability,
    [switch]$NoArtifacts,
    [string]$RunId = ""
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$previousRunId = ""
$hadPreviousRunId = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "meta-check" -RequestedRunId $RunId
    $previousRunId = [string]$env:OXVBA_RUN_ID
    $hadPreviousRunId = Test-Path Env:OXVBA_RUN_ID
    $env:OXVBA_RUN_ID = $resolvedRunId

    Write-Host "[oxvba] run-id: $resolvedRunId"
    if ($NoArtifacts) {
        Write-Host "[oxvba] no-artifacts mode: enabled (writes redirected to temp/no-artifacts)"
    }

    Write-Host "[oxvba] governance"
    & "$PSScriptRoot/check-governance.ps1"

    Write-Host "[oxvba] profile-artifact-scope"
    & "$PSScriptRoot/validate-profile-artifact-scope.ps1" -Mode working

    Write-Host "[oxvba] language-coverage"
    & "$PSScriptRoot/validate-language-coverage.ps1"

    Write-Host "[oxvba] coverage-notes"
    & "$PSScriptRoot/validate-coverage-notes.ps1"

    Write-Host "[oxvba] intrinsic-surface"
    & "$PSScriptRoot/validate-intrinsic-surface.ps1"

    Write-Host "[oxvba] hal-clause-drift"
    & "$PSScriptRoot/check-hal-clause-drift.ps1"

    Write-Host "[oxvba] pmr-clause-drift"
    & "$PSScriptRoot/check-pmr-clause-drift.ps1"

    Write-Host "[oxvba] kani-obligation-policy"
    & "$PSScriptRoot/validate-kani-obligation-policy.ps1"

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
        Write-Host "[oxvba] conformance (com-early)"
        $comEarlyArgs = @{
            IncludeFormalLane = $true
            RunId = $resolvedRunId
        }
        if ($NoArtifacts) {
            $comEarlyArgs["NoArtifacts"] = $true
        }
        & "$PSScriptRoot/run-com-early-conformance.ps1" @comEarlyArgs
    }

    if ($Matrix) {
        Write-Host "[oxvba] matrix"
        $matrixArgs = @{
            RunId = $resolvedRunId
        }
        if ($NoArtifacts) {
            $matrixArgs["NoArtifacts"] = $true
        }
        & "$PSScriptRoot/run-matrix.ps1" @matrixArgs
    }

    if ($Formal) {
        Write-Host "[oxvba] formal (non-blocking)"
        $formalArgs = @{
            RunId = $resolvedRunId
            Quiet = $true
        }
        if ($NoArtifacts) {
            $formalArgs["NoArtifacts"] = $true
        }
        & "$PSScriptRoot/run-formal.ps1" @formalArgs
    }

    Write-Host "[oxvba] meta check complete"
}
finally {
    if ($hadPreviousRunId) {
        $env:OXVBA_RUN_ID = $previousRunId
    }
    else {
        Remove-Item Env:OXVBA_RUN_ID -ErrorAction SilentlyContinue
    }
    Pop-Location
}
