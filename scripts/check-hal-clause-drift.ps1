param(
    [string]$MarkdownPath = "docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md",
    [string]$CsvPath = "docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.csv"
)

$ErrorActionPreference = "Stop"

$mdResolved = Resolve-Path $MarkdownPath
$csvResolved = Resolve-Path $CsvPath

$csvIds = Import-Csv $csvResolved |
    ForEach-Object { $_.clause_id } |
    Where-Object { $_ -and $_.Trim().Length -gt 0 } |
    Sort-Object -Unique

$mdText = Get-Content -Raw $mdResolved
$mdIds = [regex]::Matches($mdText, '`(HAL-[A-Z]+-[0-9]{3})`') |
    ForEach-Object { $_.Groups[1].Value } |
    Sort-Object -Unique

$missingInCsv = $mdIds | Where-Object { $_ -notin $csvIds }
$missingInMd = $csvIds | Where-Object { $_ -notin $mdIds }

if ($missingInCsv.Count -gt 0 -or $missingInMd.Count -gt 0) {
    Write-Error "HAL clause catalog drift detected. Missing in CSV: [$($missingInCsv -join ', ')]. Missing in Markdown: [$($missingInMd -join ', ')]."
    exit 1
}

Write-Host "[hal] clause catalog drift check passed ($($csvIds.Count) clause IDs)"
