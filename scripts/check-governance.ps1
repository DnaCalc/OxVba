$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    Write-Host "[governance] line-endings"
    & "$PSScriptRoot/validate-line-endings.ps1"

    Write-Host "[governance] line-ending-mutations"
    & "$PSScriptRoot/test-line-endings.ps1"

    Write-Host "[governance] linux-ci-environment"
    & "$PSScriptRoot/validate-linux-ci-environment.ps1"

    Write-Host "[governance] linux-ci-environment-mutations"
    & "$PSScriptRoot/test-linux-ci-environment.ps1"

    Write-Host "[governance] docs-check"
    & "$PSScriptRoot/docs-check.ps1"

    Write-Host "[governance] active-program-sync"
    & "$PSScriptRoot/validate-active-program-sync.ps1"

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

    Write-Host "[governance] windows-x64-control-surfaces"
    & "$PSScriptRoot/validate-windows-x64-control-surfaces.ps1"

    Write-Host "[governance] windows-current-stack-residuals"
    & "$PSScriptRoot/validate-windows-current-stack-residuals.ps1"

    Write-Host "[governance] windows-current-stack-residual-mutations"
    & "$PSScriptRoot/test-windows-current-stack-residuals.ps1"

    Write-Host "[governance] windows-x64-fixture-manifest-sync"
    & "$PSScriptRoot/sync-windows-fixture-manifest.ps1" -Check

    Write-Host "[governance] windows-x64-fixture-manifest"
    & "$PSScriptRoot/validate-windows-fixture-manifest.ps1"

    Write-Host "[governance] contract-clause-disposition"
    & "$PSScriptRoot/validate-contract-clause-disposition.ps1"

    Write-Host "[governance] environment-manifest"
    & "$PSScriptRoot/validate-environment-manifest.ps1"

    Write-Host "[governance] legacy-migration"
    & "$PSScriptRoot/validate-ideal-legacy-migration.ps1"

    Write-Host "[governance] closure-taxonomy"
    & "$PSScriptRoot/validate-closure-taxonomy.ps1"

    Write-Host "[governance] bead-traceability"
    & "$PSScriptRoot/validate-bead-traceability.ps1"

    Write-Host "[governance] workset-rollout"
    & "$PSScriptRoot/validate-workset-rollout.ps1"

    Write-Host "[governance] ideal-validator-negative-cases"
    & "$PSScriptRoot/test-ideal-program-validator-negative-cases.ps1"

    Write-Host "[governance] validation-derived-summary"
    & "$PSScriptRoot/generate-validation-derived-summaries.ps1" -Check

    Write-Host "[governance] complete"
}
finally {
    Pop-Location
}
