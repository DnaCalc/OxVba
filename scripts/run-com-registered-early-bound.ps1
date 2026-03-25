param(
    [switch]$NoCapture,
    [string]$EvidenceDir = "docs/evidence/conformance/com",
    [string]$RunId = "",
    [switch]$NoThrow,
    [switch]$NoLatest
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not $IsWindows) {
        throw "registered COM early-bound lane is Windows-only"
    }
    if ([string]::IsNullOrWhiteSpace($RunId)) {
        $RunId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    }
    if (-not (Test-Path $EvidenceDir)) {
        New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
    }

    $progId = "Scripting.Dictionary"
    $cmd = @(
        "test",
        "-p",
        "oxvba-host",
        "--test",
        "com_early_project_end_to_end",
        "early_bound_project_executes_registered_scripting_dictionary_member_subset",
        "--",
        "--ignored",
        "--exact",
        "--test-threads=1"
    )
    if (-not $NoCapture) {
        $cmd += "--nocapture"
    }
    $cmdText = "cargo " + ($cmd -join " ")
    $safeProg = ($progId -replace "[^A-Za-z0-9\.-]", "_")
    $logPath = Join-Path $EvidenceDir ("COM_LANE_L2B_LOG_{0}_{1}.txt" -f $safeProg, $RunId)
    $reportPath = Join-Path $EvidenceDir ("COM_LANE_L2B_RUN_{0}_{1}.md" -f $safeProg, $RunId)
    $latestCsvPath = Join-Path $EvidenceDir ("COM_LANE_L2B_LATEST_{0}.csv" -f $safeProg)

    $startedUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    Write-Host "[oxvba] COM lane L2B (registered early-bound supported subset)"
    Write-Host "[oxvba] ProgID: $progId"
    Write-Host "[oxvba] command: $cmdText"
    $null = & cargo @cmd 2>&1 | Tee-Object -FilePath $logPath
    $exitCode = $LASTEXITCODE
    $finishedUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $status = if ($exitCode -eq 0) { "pass" } else { "fail" }

    $result = [PSCustomObject]@{
        run_id = $RunId
        lane_id = "L2B"
        lane = "registered-early-bound"
        profile = "windows-headless"
        prog_id = $progId
        status = $status
        exit_code = $exitCode
        started_utc = $startedUtc
        finished_utc = $finishedUtc
        command = $cmdText
        log = $logPath
        report = $reportPath
    }

    $report = @(
        "# COM Lane L2B Run",
        "",
        "- Run ID: $RunId",
        "- Lane: L2B registered early-bound supported subset",
        "- Status: $status",
        "- Exit code: $exitCode",
        "- Started UTC: $startedUtc",
        "- Finished UTC: $finishedUtc",
        "- Command: $cmdText",
        "- ProgID: $progId",
        "- Scenario: As New Scripting.Dictionary plus Add / Exists / Count",
        "- Log: $logPath"
    )
    Set-Content -Path $reportPath -Value ($report -join "`n")
    if (-not $NoLatest) {
        $result | Export-Csv -Path $latestCsvPath -NoTypeInformation
    }

    if ($exitCode -ne 0 -and -not $NoThrow) {
        throw "registered COM early-bound lane failed (exit=$exitCode)"
    }
    return $result
}
finally {
    Pop-Location
}
