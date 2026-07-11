param(
    [switch]$Refresh
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if ($Refresh) {
        & "$PSScriptRoot/generate-validation-derived-summaries.ps1"
    }

    & "$PSScriptRoot/validate-active-program-sync.ps1"
    & "$PSScriptRoot/validate-validation-ownership.ps1"
    & "$PSScriptRoot/validate-contract-clause-disposition.ps1"
    & "$PSScriptRoot/validate-environment-manifest.ps1"
    & "$PSScriptRoot/validate-ideal-legacy-migration.ps1"
    & "$PSScriptRoot/validate-closure-taxonomy.ps1"
    & "$PSScriptRoot/validate-bead-traceability.ps1"
    & "$PSScriptRoot/validate-workset-rollout.ps1"
    & "$PSScriptRoot/generate-validation-derived-summaries.ps1" -Check

    Write-Host "run-truth-reconciliation: ok"
}
finally {
    Pop-Location
}
