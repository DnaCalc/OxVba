param(
    [switch]$Check
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $matrixFiles = @(
        "docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv",
        "docs/validation/COM_EXTERNAL_INTEGRATION_VALIDATION_MATRIX_V1.csv",
        "docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv",
        "docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv"
    )

    $allRows = @()
    foreach ($path in $matrixFiles) {
        $rows = Import-Csv $path
        foreach ($row in $rows) {
            $row | Add-Member -NotePropertyName matrix_file -NotePropertyValue $path
            $allRows += $row
        }
    }

    $summary = @()
    $summary += "# Validation Derived Summary"
    $summary += ""
    $summary += "Generated from:"
    foreach ($path in $matrixFiles) {
        $summary += ("- ``{0}``" -f $path)
    }
    $summary += ""
    $summary += "## Totals"
    $summary += ""
    $summary += "| Domain | Rows | In Progress | Implemented Subset | Implemented Full | Verified | Planned |"
    $summary += "|---|---|---|---|---|---|---|"

    foreach ($domain in @("language", "com_external", "project_hosting", "language_services_formalization")) {
        $domainRows = @($allRows | Where-Object { $_.domain -eq $domain })
        $summary += "| $domain | $($domainRows.Count) | $((@($domainRows | Where-Object { $_.truth_state -eq 'in-progress' })).Count) | $((@($domainRows | Where-Object { $_.truth_state -eq 'implemented-subset' })).Count) | $((@($domainRows | Where-Object { $_.truth_state -eq 'implemented-full' })).Count) | $((@($domainRows | Where-Object { $_.truth_state -eq 'verified' })).Count) | $((@($domainRows | Where-Object { $_.truth_state -eq 'planned' })).Count) |"
    }

    $summary += ""
    $summary += "## Open Focus"
    $summary += ""
    $summary += "| Feature ID | Domain | Feature | Truth State | Matrix |"
    $summary += "|---|---|---|---|---|"
    foreach ($row in @($allRows | Where-Object { $_.truth_state -in @('in-progress', 'planned') })) {
        $summary += "| $($row.feature_id) | $($row.domain) | $($row.feature_name) | $($row.truth_state) | $($row.matrix_file) |"
    }

    $rendered = $summary -join "`n"
    $outPath = "docs/validation/VALIDATION_DERIVED_SUMMARY_LATEST.md"

    if ($Check) {
        if (-not (Test-Path $outPath)) {
            throw "generate-validation-derived-summaries: missing $outPath"
        }
        $existing = Get-Content $outPath -Raw
        $normalizedExisting = ($existing -replace "`r`n", "`n").TrimEnd()
        $normalizedRendered = ($rendered -replace "`r`n", "`n").TrimEnd()
        if ($normalizedExisting -ne $normalizedRendered) {
            throw "generate-validation-derived-summaries: summary drift detected. regenerate $outPath"
        }
        Write-Host "generate-validation-derived-summaries: ok (checked)"
    }
    else {
        Set-Content -Path $outPath -Value $rendered
        Write-Host "generate-validation-derived-summaries: wrote $outPath"
    }
}
finally {
    Pop-Location
}
