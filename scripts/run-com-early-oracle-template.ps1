param(
    [string]$OutputDir = "docs/evidence/conformance/oracle_templates/com_early"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

$timestamp = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
$csvPath = Join-Path $OutputDir ("COM_EARLY_ORACLE_TEMPLATE_{0}.csv" -f $timestamp)
$mdPath = Join-Path $OutputDir ("COM_EARLY_ORACLE_TEMPLATE_{0}.md" -f $timestamp)
$latestCsv = Join-Path $OutputDir "COM_EARLY_ORACLE_TEMPLATE_LATEST.csv"
$latestMd = Join-Path $OutputDir "COM_EARLY_ORACLE_TEMPLATE_LATEST.md"

@(
    [PSCustomObject]@{ topic_id = "CCT-046"; scenario = "As New early-bound class instantiation"; expected_vba = ""; observed_oxvba = ""; status = "pending" },
    [PSCustomObject]@{ topic_id = "CCT-047"; scenario = "Dual-interface fallback dispatch-vs-vtable"; expected_vba = ""; observed_oxvba = ""; status = "pending" },
    [PSCustomObject]@{ topic_id = "CCT-048"; scenario = "Typelib version selection and broken-reference behavior"; expected_vba = ""; observed_oxvba = ""; status = "pending" }
) | Export-Csv -Path $csvPath -NoTypeInformation
Copy-Item -Path $csvPath -Destination $latestCsv -Force

$lines = @(
    "# COM Early Oracle Template",
    "",
    "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
    "- Status: scaffold",
    "- CSV: $csvPath",
    "",
    "Use this template to record side-by-side VBA host vs OxVba behavior for deferred oracle topics CCT-046..CCT-048."
)
Set-Content -Path $mdPath -Value ($lines -join "`n")
Copy-Item -Path $mdPath -Destination $latestMd -Force

Write-Host "com-early oracle template generated: $latestCsv"
