param(
    [string]$ProgId = "OxVba.TestDispatch",
    [int]$EventToken = 1,
    [int]$TriggerMember = 3,
    [int]$TriggerArg = 77,
    [int]$ExpectedArgCount = 1,
    [string]$EventPath = "",
    [string]$ConnectionPointIid = "",
    [Nullable[int]]$DispatchMember = $null,
    [bool]$TriggerRequiresArg = $true,
    [string]$TriggerInvokeKind = "",
    [switch]$ForceRegisteredTestDispatch,
    [switch]$EnableTrace,
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

    $progIdNormalized = $ProgId.Trim().ToLowerInvariant()
    if ($progIdNormalized -eq "excel.application") {
        if (-not $PSBoundParameters.ContainsKey("EventToken")) {
            $EventToken = 10
        }
        if (-not $PSBoundParameters.ContainsKey("TriggerMember")) {
            $TriggerMember = 10
        }
        if (-not $PSBoundParameters.ContainsKey("TriggerArg")) {
            $TriggerArg = 0
        }
        if (-not $PSBoundParameters.ContainsKey("ExpectedArgCount")) {
            $ExpectedArgCount = 0
        }
    }

    $prevProg = $env:OXVBA_REGISTERED_COM_PROGID
    $prevRequire = $env:OXVBA_REGISTERED_EVENT_REQUIRE_SUCCESS
    $prevEventToken = $env:OXVBA_REGISTERED_EVENT_TOKEN
    $prevTriggerMember = $env:OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER
    $prevTriggerArg = $env:OXVBA_REGISTERED_EVENT_TRIGGER_ARG
    $prevExpectedArgCount = $env:OXVBA_REGISTERED_EVENT_EXPECTED_ARGC
    $prevEventPath = $env:OXVBA_REGISTERED_EVENT_PATH
    $prevConnectionPointIid = $env:OXVBA_REGISTERED_EVENT_CONNECTION_POINT_IID
    $prevDispatchMember = $env:OXVBA_REGISTERED_EVENT_DISPATCH_MEMBER
    $prevTriggerRequiresArg = $env:OXVBA_REGISTERED_EVENT_TRIGGER_REQUIRES_ARG
    $prevTriggerInvokeKind = $env:OXVBA_REGISTERED_EVENT_TRIGGER_INVOKE_KIND
    $prevForceRegistered = $env:OXVBA_COM_FORCE_REGISTERED_TESTDISPATCH
    $prevTrace = $env:OXVBA_COM_EVENT_TRACE
    $hadPrevProg = Test-Path Env:OXVBA_REGISTERED_COM_PROGID
    $hadPrevRequire = Test-Path Env:OXVBA_REGISTERED_EVENT_REQUIRE_SUCCESS
    $hadPrevEventToken = Test-Path Env:OXVBA_REGISTERED_EVENT_TOKEN
    $hadPrevTriggerMember = Test-Path Env:OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER
    $hadPrevTriggerArg = Test-Path Env:OXVBA_REGISTERED_EVENT_TRIGGER_ARG
    $hadPrevExpectedArgCount = Test-Path Env:OXVBA_REGISTERED_EVENT_EXPECTED_ARGC
    $hadPrevEventPath = Test-Path Env:OXVBA_REGISTERED_EVENT_PATH
    $hadPrevConnectionPointIid = Test-Path Env:OXVBA_REGISTERED_EVENT_CONNECTION_POINT_IID
    $hadPrevDispatchMember = Test-Path Env:OXVBA_REGISTERED_EVENT_DISPATCH_MEMBER
    $hadPrevTriggerRequiresArg = Test-Path Env:OXVBA_REGISTERED_EVENT_TRIGGER_REQUIRES_ARG
    $hadPrevTriggerInvokeKind = Test-Path Env:OXVBA_REGISTERED_EVENT_TRIGGER_INVOKE_KIND
    $hadPrevForceRegistered = Test-Path Env:OXVBA_COM_FORCE_REGISTERED_TESTDISPATCH
    $hadPrevTrace = Test-Path Env:OXVBA_COM_EVENT_TRACE
    try {
        $env:OXVBA_REGISTERED_COM_PROGID = $ProgId
        $env:OXVBA_REGISTERED_EVENT_REQUIRE_SUCCESS = "1"
        $env:OXVBA_REGISTERED_EVENT_TOKEN = "$EventToken"
        $env:OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER = "$TriggerMember"
        $env:OXVBA_REGISTERED_EVENT_TRIGGER_ARG = "$TriggerArg"
        $env:OXVBA_REGISTERED_EVENT_EXPECTED_ARGC = "$ExpectedArgCount"
        $triggerRequiresArgValue = "0"
        if ($TriggerRequiresArg) {
            $triggerRequiresArgValue = "1"
        }
        $env:OXVBA_REGISTERED_EVENT_TRIGGER_REQUIRES_ARG = $triggerRequiresArgValue
        if (-not [string]::IsNullOrWhiteSpace($EventPath)) {
            $env:OXVBA_REGISTERED_EVENT_PATH = $EventPath
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_PATH -ErrorAction SilentlyContinue
        }
        if (-not [string]::IsNullOrWhiteSpace($ConnectionPointIid)) {
            $env:OXVBA_REGISTERED_EVENT_CONNECTION_POINT_IID = $ConnectionPointIid
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_CONNECTION_POINT_IID -ErrorAction SilentlyContinue
        }
        if ($DispatchMember.HasValue) {
            $env:OXVBA_REGISTERED_EVENT_DISPATCH_MEMBER = "$($DispatchMember.Value)"
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_DISPATCH_MEMBER -ErrorAction SilentlyContinue
        }
        if (-not [string]::IsNullOrWhiteSpace($TriggerInvokeKind)) {
            $env:OXVBA_REGISTERED_EVENT_TRIGGER_INVOKE_KIND = $TriggerInvokeKind
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_TRIGGER_INVOKE_KIND -ErrorAction SilentlyContinue
        }
        if ($ForceRegisteredTestDispatch) {
            $env:OXVBA_COM_FORCE_REGISTERED_TESTDISPATCH = "1"
        }
        if ($EnableTrace) {
            $env:OXVBA_COM_EVENT_TRACE = "1"
        }

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
        Write-Host "[oxvba] expected callback arg count: $ExpectedArgCount"
        Write-Host "[oxvba] event path: $(if ([string]::IsNullOrWhiteSpace($EventPath)) { '<default>' } else { $EventPath })"
        Write-Host "[oxvba] connection-point IID: $(if ([string]::IsNullOrWhiteSpace($ConnectionPointIid)) { '<default>' } else { $ConnectionPointIid })"
        Write-Host "[oxvba] dispatch-member override: $(if ($DispatchMember.HasValue) { $DispatchMember.Value } else { '<default>' })"
        Write-Host "[oxvba] trigger requires arg: $($TriggerRequiresArg.ToString().ToLowerInvariant())"
        Write-Host "[oxvba] trigger invoke kind: $(if ([string]::IsNullOrWhiteSpace($TriggerInvokeKind)) { '<default>' } else { $TriggerInvokeKind })"
        Write-Host "[oxvba] force registered test dispatch: $($ForceRegisteredTestDispatch.ToString().ToLowerInvariant())"
        Write-Host "[oxvba] com event trace: $($EnableTrace.ToString().ToLowerInvariant())"
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
            "- Expected callback arg count: $ExpectedArgCount",
            "- Event path override: $(if ([string]::IsNullOrWhiteSpace($EventPath)) { '<default>' } else { $EventPath })",
            "- Connection-point IID override: $(if ([string]::IsNullOrWhiteSpace($ConnectionPointIid)) { '<default>' } else { $ConnectionPointIid })",
            "- Dispatch-member override: $(if ($DispatchMember.HasValue) { $DispatchMember.Value } else { '<default>' })",
            "- Trigger requires arg: $($TriggerRequiresArg.ToString().ToLowerInvariant())",
            "- Trigger invoke kind override: $(if ([string]::IsNullOrWhiteSpace($TriggerInvokeKind)) { '<default>' } else { $TriggerInvokeKind })",
            "- COM event trace: $($EnableTrace.ToString().ToLowerInvariant())",
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
        if ($hadPrevExpectedArgCount) {
            $env:OXVBA_REGISTERED_EVENT_EXPECTED_ARGC = $prevExpectedArgCount
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_EXPECTED_ARGC -ErrorAction SilentlyContinue
        }
        if ($hadPrevEventPath) {
            $env:OXVBA_REGISTERED_EVENT_PATH = $prevEventPath
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_PATH -ErrorAction SilentlyContinue
        }
        if ($hadPrevConnectionPointIid) {
            $env:OXVBA_REGISTERED_EVENT_CONNECTION_POINT_IID = $prevConnectionPointIid
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_CONNECTION_POINT_IID -ErrorAction SilentlyContinue
        }
        if ($hadPrevDispatchMember) {
            $env:OXVBA_REGISTERED_EVENT_DISPATCH_MEMBER = $prevDispatchMember
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_DISPATCH_MEMBER -ErrorAction SilentlyContinue
        }
        if ($hadPrevTriggerRequiresArg) {
            $env:OXVBA_REGISTERED_EVENT_TRIGGER_REQUIRES_ARG = $prevTriggerRequiresArg
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_TRIGGER_REQUIRES_ARG -ErrorAction SilentlyContinue
        }
        if ($hadPrevTriggerInvokeKind) {
            $env:OXVBA_REGISTERED_EVENT_TRIGGER_INVOKE_KIND = $prevTriggerInvokeKind
        } else {
            Remove-Item Env:OXVBA_REGISTERED_EVENT_TRIGGER_INVOKE_KIND -ErrorAction SilentlyContinue
        }
        if ($hadPrevForceRegistered) {
            $env:OXVBA_COM_FORCE_REGISTERED_TESTDISPATCH = $prevForceRegistered
        } else {
            Remove-Item Env:OXVBA_COM_FORCE_REGISTERED_TESTDISPATCH -ErrorAction SilentlyContinue
        }
        if ($hadPrevTrace) {
            $env:OXVBA_COM_EVENT_TRACE = $prevTrace
        } else {
            Remove-Item Env:OXVBA_COM_EVENT_TRACE -ErrorAction SilentlyContinue
        }
    }
}
finally {
    Pop-Location
}
