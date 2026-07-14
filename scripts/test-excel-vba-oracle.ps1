$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "excel-vba-oracle-contract.ps1")
. (Join-Path $PSScriptRoot "excel-vba-oracle-job.ps1")

function Assert-True {
    param([Parameter(Mandatory = $true)][bool]$Condition, [Parameter(Mandatory = $true)][string]$Message)
    if (-not $Condition) { throw "test-excel-vba-oracle: $Message" }
}

function Assert-Equal {
    param($Expected, $Actual, [Parameter(Mandatory = $true)][string]$Message)
    if ($Expected -ne $Actual) {
        throw "test-excel-vba-oracle: $Message (expected '$Expected', got '$Actual')"
    }
}

function Test-GuardianOwnedWindowEnumerationShape {
    param([Parameter(Mandatory = $true)][string]$Source)
    $match = [regex]::Match(
        $Source,
        '(?s)function Get-OwnedTopLevelWindows\s*\{(?<body>.*?)\r?\n\}\r?\n\r?\nfunction Get-ElementStrings'
    )
    if (-not $match.Success) { return $false }
    $body = $match.Groups['body'].Value
    return $body -match 'RootElement\.FindAll' -and
        $body -match 'Condition\]::TrueCondition' -and
        $body -match 'ProcessId\s+-eq\s+\$ExcelPid' -and
        $body -notmatch 'ControlTypeProperty|ControlType\]::Window|AndCondition'
}

function Test-GuardianCaptureBeforeDismissShape {
    param([Parameter(Mandatory = $true)][string]$Source)
    $observationAppend = $Source.IndexOf('Add-GuardianEvent -Event $observationEvent')
    $invoke = $Source.IndexOf('$dismissedButton = Invoke-OwnedDialogButton')
    $dismissalAppend = $Source.IndexOf('Add-GuardianEvent -Event $dismissalEvent')
    return $observationAppend -ge 0 -and $invoke -gt $observationAppend -and $dismissalAppend -gt $invoke
}

function Test-RunnerIdentityCheckedCleanupShape {
    param([Parameter(Mandatory = $true)][string]$Source)
    $match = [regex]::Match(
        $Source,
        '(?s)function Stop-RecordedOwnedResources\s*\{(?<body>.*?)\r?\n\}\r?\n\r?\n\$outputBase'
    )
    if (-not $match.Success) { return $false }
    $body = $match.Groups['body'].Value
    return $body -match 'Get-ExcelOracleProcessIdentityState' -and
        $body -match '\.Kill\(\)' -and
        $body -notmatch 'Stop-Process'
}

function Test-WorkerEvidenceGatedAcceptanceShape {
    param([Parameter(Mandatory = $true)][string]$Source)
    return $Source -match '\$passed\s*=\s*\$behaviorPassed\s+-and\s+\$guardianHealthy\s+-and\s+\$authoritativeEvidencePassed' -and
        $Source -match 'Test-CompileErrorEvidence' -and
        $Source -match 'Test-AmbiguousMacroEvidence' -and
        $Source -match 'Test-LinkedSuccessfulDismissal'
}

function Test-JobContainsPreLedgerChild {
    param([Parameter(Mandatory = $true)][string]$Label)
    $directory = Join-Path ([IO.Path]::GetTempPath()) "oxvba-oracle-job-$Label-$([Guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    $readyFile = Join-Path $directory "ready"
    $childPidFile = Join-Path $directory "child.pid"
    $env:OXVBA_ORACLE_JOB_TEST_READY = $readyFile
    $env:OXVBA_ORACLE_JOB_TEST_CHILD_PID = $childPidFile
    $payload = @'
while (-not (Test-Path -LiteralPath $env:OXVBA_ORACLE_JOB_TEST_READY)) { Start-Sleep -Milliseconds 10 }
$child = Start-Process -FilePath (Join-Path $PSHOME "pwsh.exe") -ArgumentList @("-NoLogo", "-NoProfile", "-Command", "Start-Sleep -Seconds 120") -PassThru
Set-Content -LiteralPath $env:OXVBA_ORACLE_JOB_TEST_CHILD_PID -Value $child.Id -Encoding ascii
Start-Sleep -Seconds 120
'@
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($payload))
    $job = [ExcelOracleJob]::new("OxVbaOracleTest-$Label-$([Guid]::NewGuid().ToString('N'))")
    $worker = $null
    $childProcess = $null
    try {
        $worker = Start-Process -FilePath (Join-Path $PSHOME "pwsh.exe") -ArgumentList @("-NoLogo", "-NoProfile", "-EncodedCommand", $encoded) -PassThru -WindowStyle Hidden
        $job.AssignProcess($worker.Handle)
        New-Item -ItemType File -Force -Path $readyFile | Out-Null
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        while ([DateTime]::UtcNow -lt $deadline -and -not (Test-Path -LiteralPath $childPidFile)) { Start-Sleep -Milliseconds 20 }
        Assert-True (Test-Path -LiteralPath $childPidFile) "$Label contained child must start before simulated ledger write"
        $childPid = [int](Get-Content -Raw -LiteralPath $childPidFile)
        $childProcess = Get-Process -Id $childPid -ErrorAction Stop
        $job.Terminate()
        [void]$worker.WaitForExit(10000)
        [void]$childProcess.WaitForExit(10000)
        Assert-True $worker.HasExited "$Label worker must be terminated by its job"
        Assert-True $childProcess.HasExited "$Label unrecorded child must be terminated by its job"
    }
    finally {
        if ($worker -and -not $worker.HasExited) { try { $worker.Kill() } catch { } }
        if ($childProcess -and -not $childProcess.HasExited) { try { $childProcess.Kill() } catch { } }
        $job.Dispose()
        Remove-Item -LiteralPath $directory -Recurse -Force -ErrorAction SilentlyContinue
        Remove-Item Env:\OXVBA_ORACLE_JOB_TEST_READY -ErrorAction SilentlyContinue
        Remove-Item Env:\OXVBA_ORACLE_JOB_TEST_CHILD_PID -ErrorAction SilentlyContinue
    }
}

foreach ($fileName in @(
    "excel-vba-oracle-contract.ps1",
    "excel-vba-oracle-job.ps1",
    "excel-vba-oracle-guardian.ps1",
    "excel-vba-oracle-worker.ps1",
    "run-excel-vba-oracle.ps1",
    "test-excel-vba-oracle.ps1"
)) {
    $tokens = $null
    $parseErrors = $null
    [void][Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $fileName), [ref]$tokens, [ref]$parseErrors)
    Assert-Equal 0 @($parseErrors).Count "$fileName must parse"
}

Test-JobContainsPreLedgerChild -Label "excel-before-ledger"
Test-JobContainsPreLedgerChild -Label "guardian-before-ledger"

$cases = @(Get-ExcelOracleHarnessCases)
Assert-Equal 5 $cases.Count "self-test case count"
Assert-Equal "success,compile-failure,ambiguous-macro-failure,intrinsic-shadow,runtime-full-err" ($cases.id -join ",") "self-test case identities"
Assert-Equal 3 @($cases | Where-Object expected_compile_status -eq "ok").Count "clean-compile case count"
Assert-Equal 2 @($cases | Where-Object expected_compile_status -eq "compile-error").Count "compile-failure case count"
Assert-True ($cases[1].module_source -match "MissingOracleSymbol") "compile-failure source must contain the missing call target"
Assert-True ($cases[3].module_source -match "ByVal Fix As Double") "intrinsic-shadow source must retain the shadowing declaration"
Assert-True ($cases[3].module_source -match "Fix\(Fix\)") "intrinsic-shadow source must call through the shadowed name"
Assert-True ($cases[4].module_source -match '(?m)^100 Err\.Raise') "runtime case must carry an Erl source label"
Assert-True ($cases[2].module_source -match 'Application\.Run "OracleSelfTest\.MissingMacro"' -and $cases[2].module_source -match 'MsgBox capturedDescription') "ambiguous case must surface the real generic Application.Run failure through an owned modal"
Assert-Equal "OracleSelfTest.RunProbe" $cases[2].run_procedure "ambiguous case must invoke the existing harness entry after clean compile"

$intrinsics = @(Get-ExcelOracleIntrinsicShadowNames)
Assert-Equal 10 $intrinsics.Count "intrinsic-shadow catalog count"
foreach ($name in @("Fix", "Date", "Time", "Name", "Error", "Left", "Right", "Len", "Val", "Format")) {
    Assert-True (Test-ExcelOracleIntrinsicShadowName -Name $name) "intrinsic-shadow catalog must include $name"
}
Assert-True (-not (Test-ExcelOracleIntrinsicShadowName -Name "NotAnIntrinsic")) "intrinsic-shadow catalog must reject unrelated names"

$compilePolicy = Get-ExcelOracleDialogPolicy -Phase compile -Texts @("Compile error: Expected array or user-defined type") -Buttons @("OK")
Assert-Equal "compile-error" $compilePolicy.kind "compile dialog classification"
Assert-Equal "capture-then-dismiss" $compilePolicy.disposition "compile dialog disposition"
$runtimePolicy = Get-ExcelOracleDialogPolicy -Phase run -Texts @("Run-time error '13': Type mismatch") -Buttons @("End")
Assert-Equal "runtime-error" $runtimePolicy.kind "runtime dialog classification"
$securityPolicy = Get-ExcelOracleDialogPolicy -Phase compile -Texts @("Macros in this project are disabled") -Buttons @("Enable Content")
Assert-Equal "block-no-dismiss" $securityPolicy.disposition "security prompts must not be dismissed"
$ambiguousPolicy = Get-ExcelOracleDialogPolicy -Phase run -Texts @("Cannot run the macro. The macro may not be available or all macros may be disabled.") -Buttons @("OK")
Assert-Equal "ambiguous-macro-failure" $ambiguousPolicy.kind "generic macro failure remains ambiguous at dialog capture"
Assert-Equal "capture-then-dismiss" $ambiguousPolicy.disposition "owned generic macro dialog may be dismissed after capture without adjudicating its cause"
$unknownPolicy = Get-ExcelOracleDialogPolicy -Phase run -Texts @("Do the surprising thing?") -Buttons @("Yes")
Assert-Equal "block-no-dismiss" $unknownPolicy.disposition "unrecognized prompts must not be dismissed"

Assert-Equal "compile-failure" (Get-ExcelOracleMacroFailureDisposition -Message "Cannot run the macro" -CompileStatus "compile-error" -AccessVbom $true -MacrosEnabled $true -TargetExists $true) "generic macro error after compile failure"
Assert-Equal "missing-macro" (Get-ExcelOracleMacroFailureDisposition -Message "Cannot run the macro. The macro may not be available." -CompileStatus "ok" -AccessVbom $true -MacrosEnabled $true -TargetExists $false) "missing macro adjudication"
Assert-Equal "suspected-compile-failure" (Get-ExcelOracleMacroFailureDisposition -Message "Cannot run the macro. All macros may be disabled." -CompileStatus "ok" -AccessVbom $true -MacrosEnabled $true -TargetExists $true) "generic macro failure with present target"
Assert-Equal "macros-disabled-or-policy" (Get-ExcelOracleMacroFailureDisposition -Message "Cannot run the macro" -CompileStatus "ok" -AccessVbom $true -MacrosEnabled $false -TargetExists $true) "disabled macro adjudication"

$expectedErr = Get-ExcelOracleExpectedRuntimeErr
$errJson = '{"number":513,"source":"OracleRuntimeSource","description":"oracle-runtime-error","help_file":"oracle-help.chm","help_context":42,"erl":100}'
$parsedErr = ConvertFrom-ExcelOracleRuntimeErr -Json $errJson
foreach ($field in @("number", "source", "description", "help_file", "help_context", "erl")) {
    Assert-Equal $expectedErr.$field $parsedErr.$field "complete runtime Err field $field"
}
$missingErrRejected = $false
try { [void](ConvertFrom-ExcelOracleRuntimeErr -Json '{"number":513}') }
catch { $missingErrRejected = $_.Exception.Message -match "missing" }
Assert-True $missingErrRejected "incomplete runtime Err payload must fail closed"

$ownedRecord = [pscustomobject]@{
    schema = "oxvba.excel-vba-oracle-owned-process.v1"
    run_id = "run-a"
    ownership = "owned-new-instance"
    pid = 303
    process_name = "EXCEL"
    process_start_utc = "2026-07-14T00:00:00.0000000Z"
    executable_path = "C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE"
}
Assert-True (Test-ExcelOracleOwnedProcessRecord -Record $ownedRecord -BaselineExcelPids @(101, 202) -RunId "run-a") "new recorded Excel PID must be recognized as owned"
Assert-True (-not (Test-ExcelOracleOwnedProcessRecord -Record $ownedRecord -BaselineExcelPids @(101, 303) -RunId "run-a")) "baseline Excel PID must never be recognized as owned"
Assert-True (-not (Test-ExcelOracleOwnedProcessRecord -Record $ownedRecord -BaselineExcelPids @() -RunId "other-run")) "foreign run record must never be recognized as owned"
$pidOnlyRecord = [pscustomobject]@{ run_id = "run-a"; ownership = "owned-new-instance"; pid = 303; process_name = "EXCEL" }
Assert-True (-not (Test-ExcelOracleOwnedProcessRecord -Record $pidOnlyRecord -BaselineExcelPids @() -RunId "run-a")) "mutation: PID/name-only ownership records must fail closed"

$selfProcess = Get-Process -Id $PID
$selfRecord = [pscustomobject]@{
    run_id = "run-self"
    pid = $selfProcess.Id
    process_name = [string]$selfProcess.ProcessName
    process_start_utc = $selfProcess.StartTime.ToUniversalTime().ToString("o")
    executable_path = [string]$selfProcess.Path
}
Assert-True (Test-ExcelOracleProcessIdentity -Record $selfRecord -Process $selfProcess -ExpectedProcessName $selfProcess.ProcessName -RunId "run-self") "exact PID/start/name/path process identity must match"
Assert-Equal "missing" (Get-ExcelOracleProcessIdentityState -Record $selfRecord -Process $null -ExpectedProcessName $selfProcess.ProcessName -RunId "run-self") "missing process identity state"
$wrongStartRecord = $selfRecord | Select-Object *
$wrongStartRecord.process_start_utc = $selfProcess.StartTime.ToUniversalTime().AddTicks(1).ToString("o")
Assert-True (-not (Test-ExcelOracleProcessIdentity -Record $wrongStartRecord -Process $selfProcess -ExpectedProcessName $selfProcess.ProcessName -RunId "run-self")) "mutation: reused PID with a different start time must fail closed"
Assert-Equal "pid-reused" (Get-ExcelOracleProcessIdentityState -Record $wrongStartRecord -Process $selfProcess -ExpectedProcessName $selfProcess.ProcessName -RunId "run-self") "different start time must classify as PID reuse"
$wrongPathRecord = $selfRecord | Select-Object *
$wrongPathRecord.executable_path = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetFileName($selfProcess.Path))
Assert-True (-not (Test-ExcelOracleProcessIdentity -Record $wrongPathRecord -Process $selfProcess -ExpectedProcessName $selfProcess.ProcessName -RunId "run-self")) "mutation: matching PID/start with different executable must fail closed"
Assert-Equal "same-instance-conflict" (Get-ExcelOracleProcessIdentityState -Record $wrongPathRecord -Process $selfProcess -ExpectedProcessName $selfProcess.ProcessName -RunId "run-self") "same PID/start with conflicting path must not be treated as gone/reused"
$helperRecord = [pscustomobject]@{
    schema = "oxvba.excel-vba-oracle-owned-helper.v1"
    run_id = "run-self"
    ownership = "owned-helper-process"
    role = "guardian"
    pid = $selfProcess.Id
    process_name = [string]$selfProcess.ProcessName
    process_start_utc = $selfProcess.StartTime.ToUniversalTime().ToString("o")
    executable_path = [string]$selfProcess.Path
}
Assert-True (Test-ExcelOracleHelperProcessRecord -Record $helperRecord -RunId "run-self") "complete guardian ownership record must pass structural validation"
$pidOnlyHelperRecord = [pscustomobject]@{ run_id = "run-self"; ownership = "owned-helper-process"; role = "guardian"; pid = $selfProcess.Id; process_name = $selfProcess.ProcessName }
Assert-True (-not (Test-ExcelOracleHelperProcessRecord -Record $pidOnlyHelperRecord -RunId "run-self")) "mutation: PID/name-only guardian ownership must fail closed"
$wrongHelperLeafRecord = $helperRecord | Select-Object *
$wrongHelperLeafRecord.executable_path = Join-Path ([IO.Path]::GetDirectoryName($selfProcess.Path)) "not-the-declared-helper.exe"
Assert-True (-not (Test-ExcelOracleHelperProcessRecord -Record $wrongHelperLeafRecord -RunId "run-self")) "mutation: guardian executable leaf must match its declared process name"

$validExcelLedgerLine = $ownedRecord | ConvertTo-Json -Compress
$validExcelLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines @($validExcelLedgerLine) -Kind excel -RunId "run-a" -BaselineExcelPids @(101, 202)
Assert-Equal 1 @($validExcelLedger.records).Count "valid Excel ownership ledger record count"
Assert-Equal 0 @($validExcelLedger.errors).Count "valid Excel ownership ledger error count"
$malformedExcelLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines @($validExcelLedgerLine, '{not-json') -Kind excel -RunId "run-a" -BaselineExcelPids @(101, 202)
Assert-Equal 1 @($malformedExcelLedger.errors).Count "mutation: malformed nonempty ownership JSON must make authority uncertain"
$nullExcelLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines @('null') -Kind excel -RunId "run-a" -BaselineExcelPids @(101, 202)
Assert-Equal 1 @($nullExcelLedger.errors).Count "mutation: null ownership JSON must make authority uncertain"
$wrongSchemaExcelLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines @($validExcelLedgerLine.Replace('owned-process.v1', 'attacker.v1')) -Kind excel -RunId "run-a" -BaselineExcelPids @(101, 202)
Assert-Equal 1 @($wrongSchemaExcelLedger.errors).Count "mutation: wrong ownership schema must make authority uncertain"
$wrongExcelLeafLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines @($validExcelLedgerLine.Replace('EXCEL.EXE', 'NOTEPAD.EXE')) -Kind excel -RunId "run-a" -BaselineExcelPids @(101, 202)
Assert-Equal 1 @($wrongExcelLeafLedger.errors).Count "mutation: Excel ownership executable leaf must be EXCEL.EXE"
$duplicateExcelLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines @($validExcelLedgerLine, $validExcelLedgerLine) -Kind excel -RunId "run-a" -BaselineExcelPids @(101, 202)
Assert-Equal 1 @($duplicateExcelLedger.errors).Count "mutation: duplicate ownership identity must make authority uncertain"

$observation = [ordered]@{
    schema = "oxvba.excel-vba-oracle-window-observation.v1"; event_type = "dialog-observation"; observation_id = "obs-1"; run_id = "run-a"
    operation_id = "compile"; phase = "compile"; excel_pid = 303; observed_process_id = 303; observed_utc = "2026-07-14T00:00:01Z"
    window_handle = "123"; classification = "compile-error"; disposition = "capture-then-dismiss"; considered_dialog = $true; is_modal = $true
    dialog_text = @("Compile error", "Sub or Function not defined"); selected_token = "MissingOracleSymbol"; expanded_line = "RunProbe = MissingOracleSymbol(1)"
}
$dismissal = [ordered]@{
    schema = "oxvba.excel-vba-oracle-dismissal-result.v1"; event_type = "dismissal-result"; observation_id = "obs-1"; run_id = "run-a"
    operation_id = "compile"; excel_pid = 303; window_handle = "123"; attempted_utc = "2026-07-14T00:00:02Z"; requested_buttons = @("OK"); succeeded = $true; dismissed_button = "OK"
}
$observationLine = $observation | ConvertTo-Json -Compress
$dismissalLine = $dismissal | ConvertTo-Json -Compress
$validGuardianLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine, $dismissalLine) -RunId "run-a"
Assert-Equal 2 @($validGuardianLedger.records).Count "valid guardian event ledger record count"
Assert-Equal 0 @($validGuardianLedger.errors).Count "valid guardian event ledger error count"
$malformedGuardianLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine, '{not-json') -RunId "run-a"
Assert-Equal 1 @($malformedGuardianLedger.errors).Count "mutation: malformed guardian JSON must fail capture authority"
$nullGuardianLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @('null') -RunId "run-a"
Assert-Equal 1 @($nullGuardianLedger.errors).Count "mutation: null guardian JSON must fail capture authority"
$orphanDismissalLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($dismissalLine) -RunId "run-a"
Assert-Equal 1 @($orphanDismissalLedger.errors).Count "mutation: dismissal without a prior observation must fail capture authority"
$duplicateObservationLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine, $observationLine) -RunId "run-a"
Assert-Equal 1 @($duplicateObservationLedger.errors).Count "mutation: duplicate guardian observation must fail capture authority"
$stringBooleanObservationLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine.Replace('"considered_dialog":true', '"considered_dialog":"false"')) -RunId "run-a"
Assert-Equal 1 @($stringBooleanObservationLedger.errors).Count "mutation: string considered_dialog impostor must fail capture authority"
$numericBooleanObservationLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine.Replace('"considered_dialog":true', '"considered_dialog":1')) -RunId "run-a"
Assert-Equal 1 @($numericBooleanObservationLedger.errors).Count "mutation: numeric considered_dialog impostor must fail capture authority"
$stringBooleanDismissalLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine, $dismissalLine.Replace('"succeeded":true', '"succeeded":"false"')) -RunId "run-a"
Assert-Equal 1 @($stringBooleanDismissalLedger.errors).Count "mutation: string succeeded impostor must fail capture authority"
$numericBooleanDismissalLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine, $dismissalLine.Replace('"succeeded":true', '"succeeded":1')) -RunId "run-a"
Assert-Equal 1 @($numericBooleanDismissalLedger.errors).Count "mutation: numeric succeeded impostor must fail capture authority"

$guardianOutput = & (Join-Path $PSScriptRoot "excel-vba-oracle-guardian.ps1") -PolicySelfTest
Assert-True (($guardianOutput -join "`n") -match "passed") "guardian policy self-test"

$runnerPath = Join-Path $PSScriptRoot "run-excel-vba-oracle.ps1"
$planJson = & $runnerPath -Suite HarnessSelfTest -EnvironmentId win-x64-dev-oracle-2026-07 -NoMatrixUpdate -PlanOnly -RunId offline-contract-test
$plan = ($planJson -join "`n") | ConvertFrom-Json
Assert-Equal "oxvba.excel-vba-oracle-plan.v1" $plan.schema "plan schema"
Assert-Equal 5 @($plan.cases).Count "plan case count"
Assert-Equal $false ([bool]$plan.certifying) "dev/oracle plan cannot certify"
Assert-Equal $false ([bool]$plan.matrix_update) "dev/oracle plan cannot update matrices"
Assert-Equal $false ([bool]$plan.release_credit) "dev/oracle plan cannot claim release credit"
Assert-Equal $false ([bool]$plan.capability_credit) "dev/oracle plan cannot claim capability credit"
Assert-True ([string]$plan.ownership_policy -match "kill-on-close job" -and [string]$plan.ownership_policy -match "process-start") "plan must require prepared job containment plus complete process identity"
Assert-True ([string]$plan.compile_policy -match "command ID 578") "plan must require forced VBE compile command ID 578"
Assert-True ([string]$plan.modal_policy -match "guardian before") "plan must start the guardian before invocation"

$missingNoMatrixRejected = $false
try { [void](& $runnerPath -Suite HarnessSelfTest -EnvironmentId win-x64-dev-oracle-2026-07 -PlanOnly -RunId offline-contract-test) }
catch { $missingNoMatrixRejected = $_.Exception.Message -match "requires -NoMatrixUpdate" }
Assert-True $missingNoMatrixRejected "dev/oracle runs without -NoMatrixUpdate must fail before Excel starts"

$pendingCertRejected = $false
try { [void](& $runnerPath -Suite HarnessSelfTest -EnvironmentId win-x64-cert-vm-pending-v1 -NoMatrixUpdate -PlanOnly -RunId offline-contract-test) }
catch { $pendingCertRejected = $_.Exception.Message -match "planned-blocking" }
Assert-True $pendingCertRejected "pending certification VM must not be runnable"

$guardianSource = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot "excel-vba-oracle-guardian.ps1")
Assert-True ($guardianSource -notmatch 'Stop-Process\s+-Id\s+\$ExcelPid') "guardian must never terminate Excel"
Assert-True ($guardianSource -match "observed_process_id") "guardian events must record the observed UIA process ID"
Assert-True ($guardianSource -match "selected_token" -and $guardianSource -match "expanded_line") "guardian must capture token and expanded line"
Assert-True ($guardianSource -match "Recognized dialog text is authoritative") "guardian must recognize VBE dialogs even when Office omits modal/class metadata"
Assert-True (Test-GuardianOwnedWindowEnumerationShape -Source $guardianSource) "guardian must enumerate all desktop children before applying the hard PID boundary"
$windowPrefilterMutation = $guardianSource.Replace(
    '[Windows.Automation.Condition]::TrueCondition',
    '[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Window)'
)
Assert-True (-not (Test-GuardianOwnedWindowEnumerationShape -Source $windowPrefilterMutation)) "mutation: ControlType.Window root prefilter must be rejected"
Assert-True (Test-GuardianCaptureBeforeDismissShape -Source $guardianSource) "guardian must durably append the immutable observation before invoking a dismiss button and append a linked result afterward"
$dismissBeforeCaptureMutation = $guardianSource.Replace(
    'Add-GuardianEvent -Event $observationEvent',
    'TEMP-CAPTURE-MARKER'
).Replace(
    '$dismissedButton = Invoke-OwnedDialogButton',
    'Add-GuardianEvent -Event $observationEvent'
).Replace(
    'TEMP-CAPTURE-MARKER',
    '$dismissedButton = Invoke-OwnedDialogButton'
)
Assert-True (-not (Test-GuardianCaptureBeforeDismissShape -Source $dismissBeforeCaptureMutation)) "mutation: dismiss-before-capture ordering must be rejected"
Assert-True ($guardianSource -match 'process_start_utc' -and $guardianSource -match 'executable_path') "guardian ready identity must include start time and executable"
Assert-True ($guardianSource -match 'Stale top-level UIA children are expected and nonfatal per element') "stale UIA children must be nonfatal per element"
Assert-True ($guardianSource -match 'Microsoft Visual Basic for Applications\*') "selected-token UIA capture must be scoped to the VBE window"

$workerSource = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot "excel-vba-oracle-worker.ps1")
$compileIndex = $workerSource.IndexOf('$compileControl.Execute()')
$runIndex = $workerSource.IndexOf('$runValue = $excel.Run($qualifiedName)')
Assert-True ($compileIndex -ge 0 -and $runIndex -gt $compileIndex) "forced VBE compile must precede Application.Run"
Assert-True ($workerSource -match "Wait-GuardianReady" -and $workerSource.IndexOf('Wait-GuardianReady') -lt $compileIndex) "guardian readiness must precede forced compile"
Assert-True ($workerSource -match "module_sha256") "case evidence must seal module source"
Assert-True ($workerSource -match "Get-VbeSelectionFromCom") "worker must preserve the VBE COM selection as a scoped fallback"
Assert-True ($workerSource -match "CodePane.Show\(\)" -and $workerSource -match "compile command ID 578 is disabled") "worker must activate the code pane and reject a disabled compile command"
Assert-True ($workerSource -match 'no-dialog-unverified') "absence of a captured dialog must remain fail-closed"
Assert-True ($workerSource -match 'owned-helper-process' -and $workerSource -match 'process_start_utc' -and $workerSource -match 'executable_path') "worker must record complete Excel and guardian identities"
Assert-True ($workerSource -notmatch 'Stop-Process') "worker cleanup must retain exact Process objects instead of PID-only termination"
Assert-True (Test-WorkerEvidenceGatedAcceptanceShape -Source $workerSource) "case acceptance must be gated by healthy guardian and authoritative modal evidence"
$statusOnlyAcceptanceMutation = $workerSource.Replace('$passed = $behaviorPassed -and $guardianHealthy -and $authoritativeEvidencePassed', '$passed = $behaviorPassed')
Assert-True (-not (Test-WorkerEvidenceGatedAcceptanceShape -Source $statusOnlyAcceptanceMutation)) "mutation: status-only case acceptance must be rejected"
Assert-True ($workerSource -match 'invalid guardian event ledger' -and $workerSource -notmatch 'catch \{ \}\s*\r?\n\s*return @\(\$events\)') "guardian event parsing must fail closed"
Assert-True ($workerSource -match 'Assert-GuardianLive.+forced VBE compile' -and $workerSource -match 'Assert-GuardianLive.+runtime invocation') "guardian exact liveness must be checked immediately before compile and runtime"
Assert-True ($workerSource -notmatch 'New-Object\s+-ComObject\s+Excel\.Application' -and $workerSource -match 'Start-OwnedExcelApplication') "Excel must be directly launched inside prepared job containment, not activated through an uncontained COM launch"
Assert-True ($workerSource.IndexOf('$containmentAuthority = Wait-ContainmentAuthority') -lt $workerSource.IndexOf('$selectedCases = @(Get-ExcelOracleHarnessCases)')) "worker must wait for containment authority before any case mutation"
$compileControlPublish = $workerSource.IndexOf('Set-GuardianControl -Path $controlFile -OperationId $compileOperation')
$compileLiveCheck = $workerSource.IndexOf('Assert-GuardianLive -Process $guardian -ReadyRecord $guardianReady -Phase "forced VBE compile after control publication"')
$runControlPublish = $workerSource.IndexOf('Set-GuardianControl -Path $controlFile -OperationId $runOperation')
$runLiveCheck = $workerSource.IndexOf('Assert-GuardianLive -Process $guardian -ReadyRecord $guardianReady -Phase "runtime invocation after control publication"')
Assert-True ($compileControlPublish -ge 0 -and $compileLiveCheck -gt $compileControlPublish -and $runControlPublish -ge 0 -and $runLiveCheck -gt $runControlPublish) "control publication must precede the immediate guardian liveness check for both phases"

$runnerSource = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot "run-excel-vba-oracle.ps1")
Assert-True (Test-RunnerIdentityCheckedCleanupShape -Source $runnerSource) "supervisor fallback cleanup must check PID/start/name/path identity before Process.Kill"
$pidOnlyCleanupMutation = $runnerSource.Replace('Get-ExcelOracleProcessIdentityState', 'Get-PidOnlyIdentityState')
Assert-True (-not (Test-RunnerIdentityCheckedCleanupShape -Source $pidOnlyCleanupMutation)) "mutation: PID-only fallback cleanup must be rejected"
Assert-True ($runnerSource -match 'run directory already exists; refusing stale ready/control/event state') "runner must reject reused run directories"
Assert-True ($runnerSource -match '\$worker\.WaitForExit\(10000\)' -and $runnerSource.IndexOf('$worker.WaitForExit(10000)') -lt $runnerSource.LastIndexOf('Stop-RecordedOwnedResources')) "timeout cleanup must wait for the exact worker before reading ledgers"
Assert-True ($runnerSource.IndexOf('$job.AssignProcess($worker.Handle)') -lt $runnerSource.IndexOf('oxvba.excel-vba-oracle-containment-ready.v1')) "supervisor must assign the waiting worker to the job before publishing mutation authority"
Assert-True ($runnerSource -match 'same-instance-conflict' -and $runnerSource -match 'identity conflict') "supervisor must fail cleanup and residual audits on same-instance identity conflicts"
Assert-True ($runnerSource -match 'worker timed out.+\$terminationFailure') "timeout evidence must preserve worker termination failure detail"

Write-Output "test-excel-vba-oracle: PASS"
