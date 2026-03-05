param(
    [string]$OutputDir = "docs/evidence/conformance/oracle_templates/com_early",
    [string]$RunId = "",
    [switch]$NoArtifacts,
    [switch]$NoLatest
)

$ErrorActionPreference = "Stop"

Push-Location (Join-Path $PSScriptRoot "..")
try {
    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-early-oracle-template" -RequestedRunId $RunId
    $resolvedNoLatest = $NoLatest -or $NoArtifacts
    if ($NoArtifacts) {
        $OutputDir = New-NoArtifactEvidenceDir -Scope "com-early-oracle-template" -RunId $resolvedRunId
    }

if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

$csvPath = Join-Path $OutputDir ("COM_EARLY_ORACLE_TEMPLATE_{0}.csv" -f $resolvedRunId)
$mdPath = Join-Path $OutputDir ("COM_EARLY_ORACLE_TEMPLATE_{0}.md" -f $resolvedRunId)
$latestCsv = Join-Path $OutputDir "COM_EARLY_ORACLE_TEMPLATE_LATEST.csv"
$latestMd = Join-Path $OutputDir "COM_EARLY_ORACLE_TEMPLATE_LATEST.md"

@(
    [PSCustomObject]@{ topic_id = "CCT-046"; scenario = "As New early-bound class instantiation"; expected_vba = ""; observed_oxvba = ""; status = "pending" },
    [PSCustomObject]@{ topic_id = "CCT-047"; scenario = "Dual-interface fallback dispatch-vs-vtable"; expected_vba = ""; observed_oxvba = ""; status = "pending" },
    [PSCustomObject]@{ topic_id = "CCT-048"; scenario = "Typelib version selection and broken-reference behavior"; expected_vba = ""; observed_oxvba = ""; status = "pending" }
) | Export-Csv -Path $csvPath -NoTypeInformation
if (-not $resolvedNoLatest) {
    Copy-Item -Path $csvPath -Destination $latestCsv -Force
}

$lines = @(
    "# COM Early Oracle Template",
    "",
    "- Run ID: $resolvedRunId",
    "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
    "- Status: scaffold",
    "- Latest pointers updated: $((-not $resolvedNoLatest).ToString().ToLowerInvariant())",
    "- CSV: $csvPath",
    "",
    "Use this template to record side-by-side VBA host vs OxVba behavior for deferred oracle topics CCT-046..CCT-048."
)
Set-Content -Path $mdPath -Value ($lines -join "`n")
if (-not $resolvedNoLatest) {
    Copy-Item -Path $mdPath -Destination $latestMd -Force
}

Write-Host "com-early oracle template generated: $csvPath"
}
finally {
    Pop-Location
}
