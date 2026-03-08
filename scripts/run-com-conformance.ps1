param(
    [switch]$IncludeRegisteredLane,
    [switch]$IncludeRegisteredEventLane,
    [string]$RegisteredProgId = "Scripting.Dictionary",
    [string[]]$RegisteredProgIds = @(),
    [string]$RegisteredEventProgId = "OxVba.TestDispatch",
    [int]$RegisteredEventToken = 1,
    [int]$RegisteredEventTriggerMember = 3,
    [int]$RegisteredEventTriggerArg = 77,
    [switch]$ForceRegisteredTestDispatch,
    [switch]$NoCapture,
    [string]$EvidenceDir = "docs/evidence/conformance/com",
    [string]$RunId = "",
    [switch]$NoArtifacts,
    [switch]$NoLatest
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-conformance" -RequestedRunId $RunId
    $resolvedNoLatest = $NoLatest -or $NoArtifacts
    if ($NoArtifacts) {
        $EvidenceDir = New-NoArtifactEvidenceDir -Scope "com-conformance" -RunId $resolvedRunId
    }

    if (-not (Test-Path $EvidenceDir)) {
        New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
    }

    $results = @()

    Write-Host "[oxvba] COM conformance orchestrator (run_id=$resolvedRunId)"
    $registrationlessArgs = @{
        EvidenceDir = $EvidenceDir
        RunId = $resolvedRunId
        NoThrow = $true
        NoLatest = $resolvedNoLatest
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
                RunId = $resolvedRunId
                NoThrow = $true
                NoLatest = $resolvedNoLatest
            }
            if ($NoCapture) {
                $registeredArgs["NoCapture"] = $true
            }
            $results += & (Join-Path $PSScriptRoot "run-com-registered.ps1") @registeredArgs
        }
    } else {
        Write-Host "[oxvba] registered lane skipped (use -IncludeRegisteredLane to enable)"
    }

    if ($IncludeRegisteredEventLane) {
        $registeredEventArgs = @{
            ProgId = $RegisteredEventProgId
            EventToken = $RegisteredEventToken
            TriggerMember = $RegisteredEventTriggerMember
            TriggerArg = $RegisteredEventTriggerArg
            EvidenceDir = $EvidenceDir
            RunId = $resolvedRunId
            NoThrow = $true
            NoLatest = $resolvedNoLatest
        }
        if ($ForceRegisteredTestDispatch) {
            $registeredEventArgs["ForceRegisteredTestDispatch"] = $true
        }
        if ($NoCapture) {
            $registeredEventArgs["NoCapture"] = $true
        }
        $results += & (Join-Path $PSScriptRoot "run-com-registered-events.ps1") @registeredEventArgs
    } else {
        Write-Host "[oxvba] registered event lane skipped (use -IncludeRegisteredEventLane to enable)"
    }

    $summaryPath = Join-Path $EvidenceDir ("COM_CONFORMANCE_RUN_{0}.md" -f $resolvedRunId)
    $summaryLatestPath = Join-Path $EvidenceDir "COM_CONFORMANCE_LATEST.md"
    $csvPath = Join-Path $EvidenceDir ("COM_CONFORMANCE_RUN_{0}.csv" -f $resolvedRunId)
    $csvLatestPath = Join-Path $EvidenceDir "COM_CONFORMANCE_LATEST.csv"

    $results | Export-Csv -Path $csvPath -NoTypeInformation
    if (-not $resolvedNoLatest) {
        Copy-Item -Path $csvPath -Destination $csvLatestPath -Force
    }

    $failed = @($results | Where-Object { $_.status -ne "pass" })
    $report = @(
        "# COM Conformance Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Status: $(if ($failed.Count -eq 0) { 'pass' } else { 'fail' })",
        "- Registrationless lane: required",
        "- Registered lane: $(if ($IncludeRegisteredLane) { 'included' } else { 'skipped' })",
        "- Registered event lane: $(if ($IncludeRegisteredEventLane) { 'included' } else { 'skipped' })",
        "- Latest pointers updated: $((-not $resolvedNoLatest).ToString().ToLowerInvariant())",
        "- Results CSV: $csvPath",
        "",
        "| Lane | Profile | ProgID | Status | Exit | Log |",
        "|---|---|---|---|---:|---|"
    )
    foreach ($row in $results) {
        $report += "| $($row.lane_id) | $($row.profile) | $($row.prog_id) | $($row.status) | $($row.exit_code) | $($row.log) |"
    }
    Set-Content -Path $summaryPath -Value ($report -join "`n")
    if (-not $resolvedNoLatest) {
        Copy-Item -Path $summaryPath -Destination $summaryLatestPath -Force
    }

    Write-Host "[oxvba] COM conformance summary: $(if ($failed.Count -eq 0) { 'pass' } else { 'fail' })"
    if ($failed.Count -ne 0) {
        throw "COM conformance run failed for $($failed.Count) lane(s)"
    }
}
finally {
    Pop-Location
}
