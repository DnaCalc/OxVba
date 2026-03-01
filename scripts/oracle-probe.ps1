param(
    [string[]]$TestPath = @(),
    [string]$OutputCsvPath = "docs/evidence/conformance/oracle_probe_queue.csv",
    [string]$HostTag = "excel-vba"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not $TestPath -or $TestPath.Count -eq 0) {
        $TestPath = Get-ChildItem -Path "conformance/tests" -Filter *.bas |
            Sort-Object Name |
            Select-Object -ExpandProperty FullName
    }

    $rows = @()
    $index = 1
    $timestampUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

    foreach ($path in $TestPath) {
        if (-not (Test-Path $path)) {
            continue
        }

        $name = Split-Path -Leaf $path
        $rows += [PSCustomObject]@{
            probe_id = ("OPR-{0:D4}" -f $index)
            test_file = $name
            host = $HostTag
            status = "pending"
            observed_slots = ""
            notes = "scaffold only; deferred oracle capture"
            captured_at_utc = ""
            generated_at_utc = $timestampUtc
        }
        $index += 1
    }

    $dir = Split-Path -Parent $OutputCsvPath
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }

    $rows | Export-Csv -Path $OutputCsvPath -NoTypeInformation
    Write-Host "oracle probe scaffold: rows=$($rows.Count) output=$OutputCsvPath"
}
finally {
    Pop-Location
}
