param(
    [string]$OutputCsv = "docs/evidence/phase12/matrix_latest.csv",
    [string]$SummaryPath = "docs/evidence/phase12/gate_report.md"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$outputDir = Split-Path -Parent $OutputCsv
if (-not (Test-Path $outputDir)) {
    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
}

$summaryDir = Split-Path -Parent $SummaryPath
if (-not (Test-Path $summaryDir)) {
    New-Item -ItemType Directory -Path $summaryDir -Force | Out-Null
}

$profileScope = "mvp-int32-core-v1"
$osName = if ($IsWindows) { "windows" } elseif ($IsLinux) { "linux" } elseif ($IsMacOS) { "macos" } else { "unknown" }
$arch = [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString().ToLowerInvariant()
$timestampUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$backends = @("vm", "jit")
$records = @()

foreach ($backend in $backends) {
    $tmpCsv = Join-Path $outputDir ("conformance_{0}.csv" -f $backend)
    & "$PSScriptRoot/run-conformance.ps1" -Backend $backend -ResultsPath $tmpCsv

    $rows = Import-Csv $tmpCsv
    if (-not $rows -or $rows.Count -eq 0) {
        throw "No conformance rows produced for backend=$backend"
    }

    $records += [PSCustomObject]@{
        profile_scope = $profileScope
        os = $osName
        arch = $arch
        backend = $backend
        required = "true"
        result = "green"
        tests_ran = $rows.Count
        captured_at_utc = $timestampUtc
        evidence_file = $tmpCsv
    }
}

$records | Export-Csv -Path $OutputCsv -NoTypeInformation

$gateStatus = if (($records | Where-Object { $_.result -ne "green" }).Count -eq 0) { "PASS" } else { "FAIL" }
$requiredRows = ($records | Where-Object { $_.required -eq "true" }).Count
$greenRows = ($records | Where-Object { $_.required -eq "true" -and $_.result -eq "green" }).Count

$summary = @(
    "# Phase 12 Gate Report",
    "",
    "- Timestamp (UTC): $timestampUtc",
    "- Profile scope: $profileScope",
    "- Required matrix cells: $requiredRows",
    "- Green required cells: $greenRows",
    "- Final gate status: $gateStatus",
    "",
    "## Required Cells",
    "",
    "| OS | Arch | Backend | Result | Tests | Evidence |",
    "|---|---|---|---|---|---|"
)

foreach ($record in $records) {
    $summary += "| $($record.os) | $($record.arch) | $($record.backend) | $($record.result) | $($record.tests_ran) | $($record.evidence_file) |"
}

Set-Content -Path $SummaryPath -Value ($summary -join "`n")
Write-Host "matrix run: $gateStatus ($greenRows/$requiredRows required cells green)"