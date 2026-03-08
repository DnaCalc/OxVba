param(
    [string]$ProgId = "OxVba.TestDispatch",
    [int]$EventToken = 1,
    [int]$TriggerMember = 3,
    [int]$TriggerArg = 77,
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
        throw "registered COM event lane is Windows-only"
    }
    if ([string]::IsNullOrWhiteSpace($RunId)) {
        $RunId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    }
    if (-not (Test-Path $EvidenceDir)) {
        New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
    }

    $prevProg = $env:OXVBA_REGISTERED_COM_PROGID
    $prevRequire = $env:OXVBA_REGISTERED_EVENT_REQUIRE_SUCCESS
    $prevEventToken = $env:OXVBA_REGISTERED_EVENT_TOKEN
    $prevTriggerMember = $env:OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER
    $prevTriggerArg = $env:OXVBA_REGISTERED_EVENT_TRIGGER_ARG
    $hadPrevProg = Test-Path Env:OXVBA_REGISTERED_COM_PROGID
    $hadPrevRequire = Test-Path Env:OXVBA_REGISTERED_EVENT_REQUIRE_SUCCESS
    $hadPrevEventToken = Test-Path Env:OXVBA_REGISTERED_EVENT_TOKEN
    $hadPrevTriggerMember = Test-Path Env:OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER
    $hadPrevTriggerArg = Test-Path Env:OXVBA_REGISTERED_EVENT_TRIGGER_ARG
    try {
        $env:OXVBA_REGISTERED_COM_PROGID = $ProgId
        $env:OXVBA_REGISTERED_EVENT_REQUIRE_SUCCESS = "1"
        $env:OXVBA_REGISTERED_EVENT_TOKEN = "$EventToken"
        $env:OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER = "$TriggerMember"
        $env:OXVBA_REGISTERED_EVENT_TRIGGER_ARG = "$TriggerArg"

        $cmd = @(
            "test",
            "-p",
            "oxvba-host",
            "--test",
            "com_client_registered_lane",
            "windows_registered_com_lane::registered_event_callback_success_when_event_capable_server_is_configured",
            "--",
            "--ignored",
            "--exact",
            "--test-threads=1"
        )
        if (-not $NoCapture) {
            $cmd += "--nocapture"
        }

        $cmdText = "cargo " + ($cmd -join " ")
        $safeProg = ($ProgId -replace "[^A-Za-z0-9\.-]", "_")
        $logPath = Join-Path $EvidenceDir ("COM_LANE_L2E_LOG_{0}_{1}.txt" -f $safeProg, $RunId)
        $reportPath = Join-Path $EvidenceDir ("COM_LANE_L2E_RUN_{0}_{1}.md" -f $safeProg, $RunId)
        $latestCsvPath = Join-Path $EvidenceDir ("COM_LANE_L2E_LATEST_{0}.csv" -f $safeProg)

        $startedUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        Write-Host "[oxvba] COM lane L2E (registered event-capable server)"
        Write-Host "[oxvba] registered ProgID: $ProgId"
        Write-Host "[oxvba] event token/member/arg: $EventToken/$TriggerMember/$TriggerArg"
        Write-Host "[oxvba] command: $cmdText"
        $null = & cargo @cmd 2>&1 | Tee-Object -FilePath $logPath
        $exitCode = $LASTEXITCODE
        $finishedUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        $status = if ($exitCode -eq 0) { "pass" } else { "fail" }

        $result = [PSCustomObject]@{
            run_id = $RunId
            lane_id = "L2E"
            lane = "registered-event-callback"
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
            "# COM Lane L2E Run",
            "",
            "- Run ID: $RunId",
            "- Lane: L2E registered event callback",
            "- Status: $status",
            "- Exit code: $exitCode",
            "- Started UTC: $startedUtc",
            "- Finished UTC: $finishedUtc",
            "- Command: $cmdText",
            "- ProgID: $ProgId",
            "- Event token: $EventToken",
            "- Trigger member: $TriggerMember",
            "- Trigger arg: $TriggerArg",
            "- Log: $logPath"
        )
        Set-Content -Path $reportPath -Value ($report -join "`n")
        if (-not $NoLatest) {
            $result | Export-Csv -Path $latestCsvPath -NoTypeInformation
        }

        if ($exitCode -ne 0 -and -not $NoThrow) {
            throw "registered COM event lane failed (exit=$exitCode)"
        }
        return $result
    }
    finally {
        if ($hadPrevProg) {
            $env:OXVBA_REGISTERED_COM_PROGID = $prevProg
        } else {
            Remove-Item Env:OXVBA_REGISTERED_COM_PROGID -ErrorAction SilentlyContinue
        }
        if ($hadPrevRequire) {
            $env:OXVBA_REGISTERED_EVENT_REQUIRE_SUCCESS = $prevRequire
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_REQUIRE_SUCCESS -ErrorAction SilentlyContinue
        }
        if ($hadPrevEventToken) {
            $env:OXVBA_REGISTERED_EVENT_TOKEN = $prevEventToken
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_TOKEN -ErrorAction SilentlyContinue
        }
        if ($hadPrevTriggerMember) {
            $env:OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER = $prevTriggerMember
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER -ErrorAction SilentlyContinue
        }
        if ($hadPrevTriggerArg) {
            $env:OXVBA_REGISTERED_EVENT_TRIGGER_ARG = $prevTriggerArg
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_TRIGGER_ARG -ErrorAction SilentlyContinue
        }
    }
}
finally {
    Pop-Location
}
