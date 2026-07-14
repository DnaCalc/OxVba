$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "excel-vba-oracle-contract.ps1")

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

foreach ($fileName in @(
    "excel-vba-oracle-contract.ps1",
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

$cases = @(Get-ExcelOracleHarnessCases)
Assert-Equal 5 $cases.Count "self-test case count"
Assert-Equal "success,compile-failure,ambiguous-macro-failure,intrinsic-shadow,runtime-full-err" ($cases.id -join ",") "self-test case identities"
Assert-Equal 3 @($cases | Where-Object expected_compile_status -eq "ok").Count "clean-compile case count"
Assert-Equal 2 @($cases | Where-Object expected_compile_status -eq "compile-error").Count "compile-failure case count"
Assert-True ($cases[1].module_source -match "MissingOracleSymbol") "compile-failure source must contain the missing call target"
Assert-True ($cases[3].module_source -match "ByVal Fix As Double") "intrinsic-shadow source must retain the shadowing declaration"
Assert-True ($cases[3].module_source -match "Fix\(Fix\)") "intrinsic-shadow source must call through the shadowed name"
Assert-True ($cases[4].module_source -match '(?m)^100 Err\.Raise') "runtime case must carry an Erl source label"

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

$ownedRecord = [pscustomobject]@{ run_id = "run-a"; ownership = "owned-new-instance"; pid = 303 }
Assert-True (Test-ExcelOracleOwnedProcessRecord -Record $ownedRecord -BaselineExcelPids @(101, 202) -RunId "run-a") "new recorded Excel PID must be recognized as owned"
Assert-True (-not (Test-ExcelOracleOwnedProcessRecord -Record $ownedRecord -BaselineExcelPids @(101, 303) -RunId "run-a")) "baseline Excel PID must never be recognized as owned"
Assert-True (-not (Test-ExcelOracleOwnedProcessRecord -Record $ownedRecord -BaselineExcelPids @() -RunId "other-run")) "foreign run record must never be recognized as owned"

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

$workerSource = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot "excel-vba-oracle-worker.ps1")
$compileIndex = $workerSource.IndexOf('$compileControl.Execute()')
$runIndex = $workerSource.IndexOf('$runValue = $excel.Run($qualifiedName)')
Assert-True ($compileIndex -ge 0 -and $runIndex -gt $compileIndex) "forced VBE compile must precede Application.Run"
Assert-True ($workerSource -match "Wait-GuardianReady" -and $workerSource.IndexOf('Wait-GuardianReady') -lt $compileIndex) "guardian readiness must precede forced compile"
Assert-True ($workerSource -match "module_sha256") "case evidence must seal module source"

Write-Output "test-excel-vba-oracle: PASS"
