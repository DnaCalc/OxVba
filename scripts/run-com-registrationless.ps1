param(
    [switch]$NoCapture,
    [string]$EvidenceDir = "docs/evidence/conformance/com",
    [string]$RunId = "",
    [switch]$NoThrow
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if ([string]::IsNullOrWhiteSpace($RunId)) {
        $RunId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    }
    if (-not (Test-Path $EvidenceDir)) {
        New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
    }

    $cmd = @("test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "--", "--test-threads=1")
    if (-not $NoCapture) {
        $cmd += "--nocapture"
    }
    $cmdText = "cargo " + ($cmd -join " ")
    $logPath = Join-Path $EvidenceDir ("COM_LANE_L2B_LOG_{0}.txt" -f $RunId)
    $reportPath = Join-Path $EvidenceDir ("COM_LANE_L2B_RUN_{0}.md" -f $RunId)
    $latestCsvPath = Join-Path $EvidenceDir "COM_LANE_L2B_LATEST.csv"

    $startedUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    Write-Host "[oxvba] COM lane L2b (registrationless controlled server)"
    Write-Host "[oxvba] command: $cmdText"
    $null = & cargo @cmd 2>&1 | Tee-Object -FilePath $logPath
    $exitCode = $LASTEXITCODE
    $finishedUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $status = if ($exitCode -eq 0) { "pass" } else { "fail" }

    $result = [PSCustomObject]@{
        run_id = $RunId
        lane_id = "L2b"
        lane = "registrationless-controlled"
        profile = "windows-headless"
        prog_id = "OxVba.TestDispatch"
        status = $status
        exit_code = $exitCode
        started_utc = $startedUtc
        finished_utc = $finishedUtc
        command = $cmdText
        log = $logPath
        report = $reportPath
    }

    $report = @(
        "# COM Lane L2b Run",
        "",
        "- Run ID: $RunId",
        "- Lane: L2b registrationless controlled server",
        "- Status: $status",
        "- Exit code: $exitCode",
        "- Started UTC: $startedUtc",
        "- Finished UTC: $finishedUtc",
        "- Command: $cmdText",
        "- ProgID: OxVba.TestDispatch",
        "- Log: $logPath"
    )
    Set-Content -Path $reportPath -Value ($report -join "`n")
    $result | Export-Csv -Path $latestCsvPath -NoTypeInformation

    if ($exitCode -ne 0 -and -not $NoThrow) {
        throw "registrationless COM lane failed (exit=$exitCode)"
    }
    return $result
}
finally {
    Pop-Location
}
