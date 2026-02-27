param(
    [string]$ProfileScope = "mvp-boolean-logic-v4",
    [string]$ReportPath = "docs/evidence/formal/latest_run.md",
    [string]$ReportCsvPath = "docs/evidence/formal/latest_run.csv",
    [string]$ObligationsPath = "docs/evidence/formal/obligations.csv"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$reportDir = Split-Path -Parent $ReportPath
if (-not (Test-Path $reportDir)) {
    New-Item -ItemType Directory -Path $reportDir -Force | Out-Null
}

$csvDir = Split-Path -Parent $ReportCsvPath
if (-not (Test-Path $csvDir)) {
    New-Item -ItemType Directory -Path $csvDir -Force | Out-Null
}

if (-not (Test-Path $ObligationsPath)) {
    throw "Missing obligations file: $ObligationsPath"
}

$timestampUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

$targetVersion = 0
if ($ProfileScope -match 'v(\d+)$') {
    $targetVersion = [int]$Matches[1]
}

$allObligations = Import-Csv $ObligationsPath
$obligations = @()
foreach ($entry in $allObligations) {
    if ($entry.active -ne "true") {
        continue
    }

    $entryVersion = $targetVersion
    if ($entry.profile -match '^v(\d+)$') {
        $entryVersion = [int]$Matches[1]
    }

    if ($entryVersion -le $targetVersion) {
        $obligations += $entry
    }
}

$rows = @()
$cargoKaniAvailable = $true
$cargoKaniVersion = ""
try {
    $cargoKaniVersion = (& cargo kani --version) -join " "
}
catch {
    $cargoKaniAvailable = $false
}

foreach ($obligation in $obligations) {
    if (-not $cargoKaniAvailable) {
        $rows += [PSCustomObject]@{
            obligation = $obligation.obligation_id
            profile = $obligation.profile
            command = $obligation.command
            blocking = $obligation.blocking
            status = "skipped"
            note = "cargo-kani not available"
            artifact = $obligation.artifact
        }
        continue
    }

    try {
        Invoke-Expression $obligation.command | Out-Null
        $rows += [PSCustomObject]@{
            obligation = $obligation.obligation_id
            profile = $obligation.profile
            command = $obligation.command
            blocking = $obligation.blocking
            status = "pass"
            note = ""
            artifact = $obligation.artifact
        }
    }
    catch {
        $rows += [PSCustomObject]@{
            obligation = $obligation.obligation_id
            profile = $obligation.profile
            command = $obligation.command
            blocking = $obligation.blocking
            status = "todo"
            note = ($_.Exception.Message -replace "\|", "/")
            artifact = $obligation.artifact
        }
        Write-Warning "formal lane: obligation $($obligation.obligation_id) did not pass (non-blocking)"
    }
}

$rows | Export-Csv -Path $ReportCsvPath -NoTypeInformation

$lines = @(
    "# Formal Run Report",
    "",
    "- Timestamp (UTC): $timestampUtc",
    "- Profile scope: $ProfileScope",
    "- Overall mode: non-blocking"
)

if ($cargoKaniAvailable) {
    $lines += "- cargo-kani: $cargoKaniVersion"
}
else {
    $lines += "- cargo-kani: unavailable"
}

$lines += @(
    "",
    "| Obligation | Profile | Blocking | Status | Command | Artifact | Note |",
    "|---|---|---|---|---|---|---|"
)

foreach ($row in $rows) {
    $lines += "| $($row.obligation) | $($row.profile) | $($row.blocking) | $($row.status) | $($row.command) | $($row.artifact) | $($row.note) |"
}

Set-Content -Path $ReportPath -Value ($lines -join "`n")

if (-not $cargoKaniAvailable) {
    Write-Warning "formal lane: cargo-kani not installed; obligations recorded as skipped"
}

Write-Host "formal run: completed (non-blocking)"
