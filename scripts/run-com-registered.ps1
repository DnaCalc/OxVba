param(
    [string]$ProgId = "Scripting.Dictionary",
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
        throw "registered COM lane is Windows-only"
    }
    if ([string]::IsNullOrWhiteSpace($RunId)) {
        $RunId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    }
    if (-not (Test-Path $EvidenceDir)) {
        New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
    }

    $prevProg = $env:OXVBA_REGISTERED_COM_PROGID
    $hadPrevProg = Test-Path Env:OXVBA_REGISTERED_COM_PROGID
    try {
        $env:OXVBA_REGISTERED_COM_PROGID = $ProgId

        $cmd = @(
            "test",
            "-p",
            "oxvba-host",
            "--test",
            "com_client_registered_lane",
            "--",
            "--ignored",
            "--test-threads=1"
        )
        if (-not $NoCapture) {
            $cmd += "--nocapture"
        }
        $cmdText = "cargo " + ($cmd -join " ")
        $safeProg = ($ProgId -replace "[^A-Za-z0-9\.-]", "_")
        $logPath = Join-Path $EvidenceDir ("COM_LANE_L2_LOG_{0}_{1}.txt" -f $safeProg, $RunId)
        $reportPath = Join-Path $EvidenceDir ("COM_LANE_L2_RUN_{0}_{1}.md" -f $safeProg, $RunId)
        $latestCsvPath = Join-Path $EvidenceDir ("COM_LANE_L2_LATEST_{0}.csv" -f $safeProg)

        $startedUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        Write-Host "[oxvba] COM lane L2 (registered external server)"
        Write-Host "[oxvba] registered ProgID: $ProgId"
        Write-Host "[oxvba] command: $cmdText"
        $null = & cargo @cmd 2>&1 | Tee-Object -FilePath $logPath
        $exitCode = $LASTEXITCODE
        $finishedUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        $status = if ($exitCode -eq 0) { "pass" } else { "fail" }

        $result = [PSCustomObject]@{
            run_id = $RunId
            lane_id = "L2"
            lane = "registered-external"
            profile = "windows-headless"
            prog_id = $ProgId
            status = $status
            exit_code = $exitCode
            started_utc = $startedUtc
            finished_utc = $finishedUtc
            command = $cmdText
            log = $logPath
            report = $reportPath
        }

        $report = @(
            "# COM Lane L2 Run",
            "",
            "- Run ID: $RunId",
            "- Lane: L2 registered external",
            "- Status: $status",
            "- Exit code: $exitCode",
            "- Started UTC: $startedUtc",
            "- Finished UTC: $finishedUtc",
            "- Command: $cmdText",
            "- ProgID: $ProgId",
            "- Log: $logPath"
        )
        Set-Content -Path $reportPath -Value ($report -join "`n")
        if (-not $NoLatest) {
            $result | Export-Csv -Path $latestCsvPath -NoTypeInformation
        }

        if ($exitCode -ne 0 -and -not $NoThrow) {
            throw "registered COM lane failed (exit=$exitCode)"
        }
        return $result
    }
    finally {
        if ($hadPrevProg) {
            $env:OXVBA_REGISTERED_COM_PROGID = $prevProg
        } else {
            Remove-Item Env:OXVBA_REGISTERED_COM_PROGID -ErrorAction SilentlyContinue
        }
    }
}
finally {
    Pop-Location
}
