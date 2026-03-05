param(
    [switch]$IncludeRegisteredLane,
    [string]$RegisteredProgId = "Scripting.Dictionary",
    [string[]]$RegisteredProgIds = @(),
    [switch]$NoCapture,
    [string]$EvidenceDir = "docs/evidence/conformance/com"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not (Test-Path $EvidenceDir)) {
        New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
    }

    $runId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    $results = @()

    Write-Host "[oxvba] COM conformance orchestrator (run_id=$runId)"
    $registrationlessArgs = @{
        EvidenceDir = $EvidenceDir
        RunId = $runId
        NoThrow = $true
    }
    if ($NoCapture) {
        $registrationlessArgs["NoCapture"] = $true
    }
    $results += & (Join-Path $PSScriptRoot "run-com-registrationless.ps1") @registrationlessArgs

    $targets = @()
    if ($RegisteredProgIds.Count -gt 0) {
        $targets = $RegisteredProgIds
    } else {
        $targets = @($RegisteredProgId)
    }

    if ($IncludeRegisteredLane) {
        foreach ($target in $targets) {
            $registeredArgs = @{
                ProgId = $target
                EvidenceDir = $EvidenceDir
                RunId = $runId
                NoThrow = $true
            }
            if ($NoCapture) {
                $registeredArgs["NoCapture"] = $true
            }
            $results += & (Join-Path $PSScriptRoot "run-com-registered.ps1") @registeredArgs
        }
    } else {
        Write-Host "[oxvba] registered lane skipped (use -IncludeRegisteredLane to enable)"
    }

    $summaryPath = Join-Path $EvidenceDir ("COM_CONFORMANCE_RUN_{0}.md" -f $runId)
    $summaryLatestPath = Join-Path $EvidenceDir "COM_CONFORMANCE_LATEST.md"
    $csvPath = Join-Path $EvidenceDir ("COM_CONFORMANCE_RUN_{0}.csv" -f $runId)
    $csvLatestPath = Join-Path $EvidenceDir "COM_CONFORMANCE_LATEST.csv"

    $results | Export-Csv -Path $csvPath -NoTypeInformation
    Copy-Item -Path $csvPath -Destination $csvLatestPath -Force

    $failed = @($results | Where-Object { $_.status -ne "pass" })
    $report = @(
        "# COM Conformance Run",
        "",
        "- Run ID: $runId",
        "- Status: $(if ($failed.Count -eq 0) { 'pass' } else { 'fail' })",
        "- Registrationless lane: required",
        "- Registered lane: $(if ($IncludeRegisteredLane) { 'included' } else { 'skipped' })",
        "- Results CSV: $csvPath",
        "",
        "| Lane | Profile | ProgID | Status | Exit | Log |",
        "|---|---|---|---|---:|---|"
    )
    foreach ($row in $results) {
        $report += "| $($row.lane_id) | $($row.profile) | $($row.prog_id) | $($row.status) | $($row.exit_code) | $($row.log) |"
    }
    Set-Content -Path $summaryPath -Value ($report -join "`n")
    Copy-Item -Path $summaryPath -Destination $summaryLatestPath -Force

    Write-Host "[oxvba] COM conformance summary: $(if ($failed.Count -eq 0) { 'pass' } else { 'fail' })"
    if ($failed.Count -ne 0) {
        throw "COM conformance run failed for $($failed.Count) lane(s)"
    }
}
finally {
    Pop-Location
}
