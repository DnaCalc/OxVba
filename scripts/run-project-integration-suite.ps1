param(
    [string]$CasePattern = "",
    [string]$EvidenceDir = "docs/evidence/conformance/project_integration"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    & (Join-Path $PSScriptRoot "lint-integration-fixtures.ps1")
    if ($LASTEXITCODE -ne 0) {
        throw "integration fixture lint failed"
    }

    $catalogPath = "conformance/integration/catalog.psv"
    if (-not (Test-Path $catalogPath)) {
        throw "Missing integration catalog: $catalogPath"
    }

    if (-not (Test-Path $EvidenceDir)) {
        New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
    }

    if ([string]::IsNullOrWhiteSpace($CasePattern)) {
        Remove-Item Env:OXVBA_PROJECT_INTEGRATION_FILTER -ErrorAction SilentlyContinue
    } else {
        $env:OXVBA_PROJECT_INTEGRATION_FILTER = $CasePattern
    }

    $timestamp = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    $logPath = Join-Path $EvidenceDir ("PROJECT_INTEGRATION_SUITE_LOG_{0}.txt" -f $timestamp)

    $lines = Get-Content $catalogPath | Where-Object {
        $trimmed = $_.Trim()
        $trimmed -ne "" -and -not $trimmed.StartsWith("#")
    }
    if ($lines.Count -lt 2) {
        throw "Integration catalog has no data rows"
    }

    $rows = @()
    for ($i = 1; $i -lt $lines.Count; $i++) {
        $parts = $lines[$i].Split('|')
        if ($parts.Count -ne 18) {
            throw "Catalog row $($i + 1) has $($parts.Count) columns (expected 18)"
        }
        $rows += [PSCustomObject]@{
            case_id = $parts[0].Trim()
            status = $parts[3].Trim().ToLowerInvariant()
            deferred_gate = $parts[14].Trim()
            topic_refs = $parts[15].Trim()
        }
    }

    $activeRows = $rows | Where-Object { $_.status -eq "active" -or $_.status -eq "active-limit" }
    $deferredRows = $rows | Where-Object { $_.status -eq "deferred" -or $_.status -eq "planned" }

    $cmd = @("test", "-p", "oxvba-host", "--test", "project_integration_suite", "--", "--nocapture")
    $cmdText = "cargo " + ($cmd -join " ")
    $startUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

    Write-Host "[oxvba] project integration suite"
    Write-Host "[oxvba] command: $cmdText"
    if (-not [string]::IsNullOrWhiteSpace($CasePattern)) {
        Write-Host "[oxvba] filter: $CasePattern"
    }

    & cargo @cmd 2>&1 | Tee-Object -FilePath $logPath
    $exitCode = $LASTEXITCODE

    $finishUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $status = if ($exitCode -eq 0) { "pass" } else { "fail" }

    $reportPath = Join-Path $EvidenceDir ("PROJECT_INTEGRATION_SUITE_RUN_{0}.md" -f $timestamp)
    $latestPath = Join-Path $EvidenceDir "PROJECT_INTEGRATION_SUITE_LATEST.md"
    $csvPath = Join-Path $EvidenceDir "PROJECT_INTEGRATION_SUITE_LATEST.csv"
    $caseFilterText = if ([string]::IsNullOrWhiteSpace($CasePattern)) { "<none>" } else { $CasePattern }

    $report = @(
        "# Project Integration Suite Run",
        "",
        "- Started UTC: $startUtc",
        "- Finished UTC: $finishUtc",
        "- Status: $status",
        "- Exit code: $exitCode",
        "- Command: $cmdText",
        "- Case filter: $caseFilterText",
        "- Active cases in catalog: $($activeRows.Count)",
        "- Deferred/planned cases in catalog: $($deferredRows.Count)",
        "- Log: $logPath",
        "",
        "## Deferred Tracking Snapshot",
        "",
        "| Case | Deferred Gate | Topics |",
        "|---|---|---|"
    )

    foreach ($row in $deferredRows) {
        $gate = if ([string]::IsNullOrWhiteSpace($row.deferred_gate)) { "-" } else { $row.deferred_gate }
        $topics = if ([string]::IsNullOrWhiteSpace($row.topic_refs)) { "-" } else { $row.topic_refs }
        $report += "| $($row.case_id) | $gate | $topics |"
    }

    Set-Content -Path $reportPath -Value ($report -join "`n")
    Copy-Item -Path $reportPath -Destination $latestPath -Force

    [PSCustomObject]@{
        timestamp_utc = $finishUtc
        status = $status
        exit_code = $exitCode
        active_cases = $activeRows.Count
        deferred_cases = $deferredRows.Count
        report = $reportPath
        log = $logPath
    } | Export-Csv -Path $csvPath -NoTypeInformation

    Write-Host "project integration suite: $status (active=$($activeRows.Count), deferred=$($deferredRows.Count))"

    if ($exitCode -ne 0) {
        throw "project integration suite failed (exit=$exitCode)"
    }
}
finally {
    Pop-Location
}
