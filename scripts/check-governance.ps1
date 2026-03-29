$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    Write-Host "[governance] docs-check"
    & "$PSScriptRoot/docs-check.ps1"

    Write-Host "[governance] gate-sync"
    & "$PSScriptRoot/validate-gate-sync.ps1"

    Write-Host "[governance] active-ladder-sync"
    & "$PSScriptRoot/validate-active-ladder-sync.ps1"

    Write-Host "[governance] divergences"
    & "$PSScriptRoot/validate-divergences.ps1"

    Write-Host "[governance] deferred-oracle-gates"
    & "$PSScriptRoot/validate-deferred-oracle-gates.ps1"

    Write-Host "[governance] pmr-followup-sync"
    & "$PSScriptRoot/validate-pmr-followup-sync.ps1"

    Write-Host "[governance] project-integration-catalog"
    & "$PSScriptRoot/validate-project-integration-catalog.ps1"

    Write-Host "[governance] pmr-event-snippets"
    & "$PSScriptRoot/generate-pmr-event-diagnostic-snippets.ps1" -Check

    Write-Host "[governance] pmr-event-diagnostic-sync"
    & "$PSScriptRoot/validate-pmr-event-diagnostic-sync.ps1"

    Write-Host "[governance] validation-ownership"
    & "$PSScriptRoot/validate-validation-ownership.ps1"

    Write-Host "[governance] workset-rollout"
    & "$PSScriptRoot/validate-workset-rollout.ps1"

    Write-Host "[governance] closure-taxonomy"
    & "$PSScriptRoot/validate-closure-taxonomy.ps1"

    Write-Host "[governance] bead-traceability"
    & "$PSScriptRoot/validate-bead-traceability.ps1"

    Write-Host "[governance] validation-derived-summary"
    & "$PSScriptRoot/generate-validation-derived-summaries.ps1" -Check

    Write-Host "[governance] complete"
}
finally {
    Pop-Location
}
