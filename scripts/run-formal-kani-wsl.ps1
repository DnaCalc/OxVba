param(
    [string]$ProfileScope = "mvp-perf-shape-v26",
    [string]$ReportPath = "docs/evidence/formal/latest_run.md",
    [string]$ReportCsvPath = "docs/evidence/formal/latest_run.csv",
    [string]$ObligationsPath = "docs/evidence/formal/obligations.csv"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    & "$PSScriptRoot/run-formal.ps1" `
        -ProfileScope $ProfileScope `
        -ReportPath $ReportPath `
        -ReportCsvPath $ReportCsvPath `
        -ObligationsPath $ObligationsPath `
        -RequireKani `
        -UseWslKani
}
finally {
    Pop-Location
}
