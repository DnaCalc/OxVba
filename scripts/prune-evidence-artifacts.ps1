param(
    [int]$KeepCount = 5,
    [string[]]$Roots = @(
        "docs/evidence/conformance/com_early",
        "docs/evidence/conformance/com",
        "docs/evidence/conformance/project_integration",
        "docs/evidence/conformance/oracle_templates/com_early",
        "docs/evidence/perf/com_early"
    ),
    [switch]$WhatIf
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

if ($KeepCount -lt 1) {
    throw "prune-evidence-artifacts: KeepCount must be >= 1"
}

$timestampRegex = [regex]'(?<ts>\d{8}T\d{6}Z)'
$totalDeleted = 0
$totalGroups = 0

Push-Location (Join-Path $PSScriptRoot "..")
try {
    foreach ($root in $Roots) {
        if (-not (Test-Path $root)) {
            continue
        }

        $candidates = Get-ChildItem -Path $root -File -Recurse |
            Where-Object { $timestampRegex.IsMatch($_.Name) }

        $groupMap = @{}
        foreach ($file in $candidates) {
            $normalized = $file.FullName.Replace('\', '/')
            $match = $timestampRegex.Match($file.Name)
            if (-not $match.Success) {
                continue
            }
            $timestamp = $match.Groups["ts"].Value
            $keyPath = $normalized.Replace($timestamp, "{ts}")
            if (-not $groupMap.ContainsKey($keyPath)) {
                $groupMap[$keyPath] = @()
            }
            $groupMap[$keyPath] += [PSCustomObject]@{
                path = $file.FullName
                timestamp = $timestamp
            }
        }

        foreach ($entry in $groupMap.GetEnumerator()) {
            $totalGroups += 1
            $sorted = @($entry.Value | Sort-Object timestamp -Descending)
            $toDelete = @()
            if ($sorted.Count -gt $KeepCount) {
                $toDelete = $sorted[$KeepCount..($sorted.Count - 1)]
            }

            foreach ($item in $toDelete) {
                if ($WhatIf) {
                    Write-Host "prune-evidence-artifacts: would delete $($item.path)"
                }
                else {
                    Remove-Item -Path $item.path -Force
                    Write-Host "prune-evidence-artifacts: deleted $($item.path)"
                }
                $totalDeleted += 1
            }
        }
    }

    Write-Host "prune-evidence-artifacts: complete (groups=$totalGroups deleted=$totalDeleted keep=$KeepCount what_if=$($WhatIf.IsPresent))"
}
finally {
    Pop-Location
}
