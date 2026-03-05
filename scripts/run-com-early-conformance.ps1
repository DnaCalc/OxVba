param(
    [switch]$NoCapture,
    [switch]$IncludeFormalLane,
    [string]$EvidenceDir = "docs/evidence/conformance/com_early"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not (Test-Path $EvidenceDir)) {
        New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
    }
    $laneDir = Join-Path $EvidenceDir "lanes"
    if (-not (Test-Path $laneDir)) {
        New-Item -ItemType Directory -Path $laneDir -Force | Out-Null
    }

    $runId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    $lanes = @("e0", "e1", "e2", "e3", "e4", "e5")
    if ($IncludeFormalLane) {
        $lanes += "e6"
    }

    $rows = @()
    foreach ($lane in $lanes) {
        $args = @{
            EvidenceDir = $laneDir
            RunId = $runId
            NoThrow = $true
        }
        if ($NoCapture) {
            $args["NoCapture"] = $true
        }
        $rows += & (Join-Path $PSScriptRoot ("run-com-early-lane-{0}.ps1" -f $lane)) @args
    }

    $csvPath = Join-Path $EvidenceDir ("COM_EARLY_CONFORMANCE_RUN_{0}.csv" -f $runId)
    $mdPath = Join-Path $EvidenceDir ("COM_EARLY_CONFORMANCE_RUN_{0}.md" -f $runId)
    $latestCsv = Join-Path $EvidenceDir "COM_EARLY_CONFORMANCE_LATEST.csv"
    $latestMd = Join-Path $EvidenceDir "COM_EARLY_CONFORMANCE_LATEST.md"

    $rows | Export-Csv -Path $csvPath -NoTypeInformation
    Copy-Item -Path $csvPath -Destination $latestCsv -Force

    $failed = @($rows | Where-Object { $_.status -eq "fail" })
    $deferred = @($rows | Where-Object { $_.status -eq "deferred" })
    $status = if ($failed.Count -eq 0) { "pass" } else { "fail" }

    $lines = @(
        "# COM Early Conformance Run",
        "",
        "- Run ID: $runId",
        "- Status: $status",
        "- Failed rows: $($failed.Count)",
        "- Deferred rows: $($deferred.Count)",
        "- Included lanes: $($lanes -join ', ')",
        "",
        "| Lane | Test | Status | Clause IDs | Evidence |",
        "|---|---|---|---|---|"
    )
    foreach ($row in $rows) {
        $lines += "| $($row.lane_id) | $($row.test_id) | $($row.status) | $($row.clause_ids) | $($row.evidence_path) |"
    }
    Set-Content -Path $mdPath -Value ($lines -join "`n")
    Copy-Item -Path $mdPath -Destination $latestMd -Force

    if ($failed.Count -gt 0) {
        throw "COM early conformance failed for $($failed.Count) rows"
    }
}
finally {
    Pop-Location
}
