param(
    [ValidateSet("HarnessSelfTest")][string]$Suite = "HarnessSelfTest",
    [Parameter(Mandatory = $true)][string]$EnvironmentId,
    [switch]$NoMatrixUpdate,
    [switch]$PlanOnly,
    [string]$RunId = ("excel_vba_oracle_{0:yyyyMMddTHHmmssZ}" -f [DateTime]::UtcNow),
    [string]$OutputRoot = "artifacts/windows-x64/excel-vba-oracle",
    [ValidateRange(30, 1800)][int]$TimeoutSeconds = 600,
    [string]$DiagnosticCaseId = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "excel-vba-oracle-contract.ps1")

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$environmentPath = Join-Path $repoRoot "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv"
$environmentRows = @(Import-Csv -LiteralPath $environmentPath)
$matches = @($environmentRows | Where-Object { [string]$_.environment_id -eq $EnvironmentId })
if ($matches.Count -ne 1) {
    throw "run-excel-vba-oracle: expected exactly one environment '$EnvironmentId', found $($matches.Count)"
}
$environment = $matches[0]
if ([string]$environment.profile -ne "windows-x64" -or [string]$environment.target_arch -ne "x64" -or [string]$environment.office_bitness -ne "64") {
    throw "run-excel-vba-oracle: environment '$EnvironmentId' is not a Windows x64 / 64-bit Excel oracle environment"
}
if ([string]$environment.role -eq "dev-oracle" -and -not $NoMatrixUpdate) {
    throw "run-excel-vba-oracle: dev-oracle environment '$EnvironmentId' requires -NoMatrixUpdate"
}
if ([string]$environment.evidence_state -ne "characterized-noncertifying") {
    throw "run-excel-vba-oracle: environment '$EnvironmentId' is '$($environment.evidence_state)' and is not runnable by this development/oracle supervisor"
}

$cases = @(Get-ExcelOracleHarnessCases)
if (-not [string]::IsNullOrWhiteSpace($DiagnosticCaseId)) {
    $cases = @($cases | Where-Object { $_.id -eq $DiagnosticCaseId })
    if ($cases.Count -ne 1) { throw "run-excel-vba-oracle: unknown diagnostic case '$DiagnosticCaseId'" }
}
$plan = [ordered]@{
    schema = "oxvba.excel-vba-oracle-plan.v1"
    suite = $Suite
    run_id = $RunId
    environment_id = $EnvironmentId
    environment_role = [string]$environment.role
    evidence_state = [string]$environment.evidence_state
    certifying = $false
    matrix_update = $false
    release_credit = $false
    capability_credit = $false
    diagnostic_only = -not [string]::IsNullOrWhiteSpace($DiagnosticCaseId)
    ownership_policy = "record PID+process-start+name+executable for new Excel HWND and guardian processes; validate the complete identity before fallback cleanup; never touch baseline unrecorded or reused PIDs"
    modal_policy = "start PID-scoped UIA guardian before command-ID-578 compile and runtime invocation; capture first; never auto-enable security/trust prompts"
    compile_policy = "VBE Debug -> Compile VBAProject command ID 578; Application.Run is never a compile check"
    cases = @($cases | ForEach-Object {
        [ordered]@{
            id = $_.id
            expected_compile_status = $_.expected_compile_status
            expected_run_status = $_.expected_run_status
            module_sha256 = Get-ExcelOracleSha256 -Text $_.module_source
        }
    })
}
if ($PlanOnly) {
    Write-Output ($plan | ConvertTo-Json -Depth 8)
    exit 0
}

if (-not $IsWindows) { throw "run-excel-vba-oracle: live execution requires Windows" }
if (-not [Environment]::Is64BitOperatingSystem -or -not [Environment]::Is64BitProcess) {
    throw "run-excel-vba-oracle: live execution requires a 64-bit Windows host process"
}

function Get-ExcelProcessIds {
    return @(Get-Process -Name EXCEL -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
}

function Read-OwnershipLedger {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][ValidateSet("excel", "guardian")][string]$Kind,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][int[]]$BaselineExcelPids
    )
    $lines = if (Test-Path -LiteralPath $Path) { @(Get-Content -LiteralPath $Path) } else { @() }
    return ConvertFrom-ExcelOracleOwnershipLedger -Lines $lines -Kind $Kind -RunId $RunId -BaselineExcelPids $BaselineExcelPids
}

function Stop-RecordedOwnedResources {
    param(
        [Parameter(Mandatory = $true)][string]$OwnershipPath,
        [Parameter(Mandatory = $true)][string]$HelperOwnershipPath,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][int[]]$BaselineExcelPids
    )
    $excelLedger = Read-OwnershipLedger -Path $OwnershipPath -Kind excel -BaselineExcelPids $BaselineExcelPids
    $helperLedger = Read-OwnershipLedger -Path $HelperOwnershipPath -Kind guardian -BaselineExcelPids $BaselineExcelPids
    foreach ($record in @($excelLedger.records)) {
        $process = Get-Process -Id ([int]$record.pid) -ErrorAction SilentlyContinue
        if ($process -and (Test-ExcelOracleProcessIdentity -Record $record -Process $process -ExpectedProcessName "EXCEL" -RunId $RunId)) {
            try { $process.Kill(); [void]$process.WaitForExit(5000) } catch { }
        }
    }
    foreach ($record in @($helperLedger.records)) {
        $process = Get-Process -Id ([int]$record.pid) -ErrorAction SilentlyContinue
        if ($process -and (Test-ExcelOracleProcessIdentity -Record $record -Process $process -ExpectedProcessName ([string]$record.process_name) -RunId $RunId)) {
            try { $process.Kill(); [void]$process.WaitForExit(5000) } catch { }
        }
    }
    return @($excelLedger.errors) + @($helperLedger.errors)
}

$outputBase = if ([IO.Path]::IsPathRooted($OutputRoot)) { $OutputRoot } else { Join-Path $repoRoot $OutputRoot }
$outputDirectory = Join-Path $outputBase $RunId
if (Test-Path -LiteralPath $outputDirectory) {
    throw "run-excel-vba-oracle: run directory already exists; refusing stale ready/control/event state: $outputDirectory"
}
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$plan | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $outputDirectory "plan.json") -Encoding utf8NoBOM

$ownershipFile = Join-Path $outputDirectory "owned-processes.jsonl"
$helperOwnershipFile = Join-Path $outputDirectory "owned-helper-processes.jsonl"
$workerStdout = Join-Path $outputDirectory "worker.stdout.txt"
$workerStderr = Join-Path $outputDirectory "worker.stderr.txt"
$baselineExcelPids = @(Get-ExcelProcessIds)
$workerArguments = @(
    "-NoLogo", "-NoProfile", "-NonInteractive", "-STA", "-File", (Join-Path $PSScriptRoot "excel-vba-oracle-worker.ps1"),
    "-RunId", $RunId,
    "-OutputDirectory", $outputDirectory,
    "-OwnershipFile", $ownershipFile,
    "-HelperOwnershipFile", $helperOwnershipFile,
    "-CaseTimeoutSeconds", [string][Math]::Min(120, $TimeoutSeconds)
)
if (-not [string]::IsNullOrWhiteSpace($DiagnosticCaseId)) {
    $workerArguments += @("-DiagnosticCaseId", $DiagnosticCaseId)
}
$startedUtc = [DateTime]::UtcNow
$worker = Start-Process -FilePath (Join-Path $PSHOME "pwsh.exe") -ArgumentList $workerArguments -PassThru -WindowStyle Hidden -RedirectStandardOutput $workerStdout -RedirectStandardError $workerStderr
$timedOut = $false
$workerFailure = $null
$cleanupAuthorityErrors = @()
$workerQuiesced = $false
try {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while (-not $worker.HasExited -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 100
    }
    if (-not $worker.HasExited) {
        $timedOut = $true
        try { $worker.Kill() }
        catch { $workerFailure = "worker termination failed: $($_.Exception.Message)" }
        $workerQuiesced = $worker.WaitForExit(10000)
        $workerFailure = "run-excel-vba-oracle: worker timed out after $TimeoutSeconds seconds"
    }
    else {
        $workerQuiesced = $true
    }
    if ($workerQuiesced -and $worker.ExitCode -ne 0) {
        $stderrText = if (Test-Path -LiteralPath $workerStderr) { Get-Content -Raw -LiteralPath $workerStderr } else { "" }
        $workerFailure = "run-excel-vba-oracle: worker failed with exit code $($worker.ExitCode): $stderrText"
    }
}
finally {
    if ($workerQuiesced) {
        $cleanupAuthorityErrors = @(Stop-RecordedOwnedResources -OwnershipPath $ownershipFile -HelperOwnershipPath $helperOwnershipFile -BaselineExcelPids $baselineExcelPids)
    }
    else {
        $cleanupAuthorityErrors = @("exact worker process did not quiesce; ownership ledgers remain mutable and cleanup is unsafe")
    }
}
if ($cleanupAuthorityErrors.Count -gt 0) {
    throw "run-excel-vba-oracle: cleanup authority is uncertain: $($cleanupAuthorityErrors -join '; ')$(if ($workerFailure) { "; primary failure: $workerFailure" })"
}
$excelLedger = Read-OwnershipLedger -Path $ownershipFile -Kind excel -BaselineExcelPids $baselineExcelPids
$helperLedger = Read-OwnershipLedger -Path $helperOwnershipFile -Kind guardian -BaselineExcelPids $baselineExcelPids
if (@($excelLedger.errors).Count -gt 0 -or @($helperLedger.errors).Count -gt 0) {
    throw "run-excel-vba-oracle: residual audit authority is uncertain: $(@($excelLedger.errors) + @($helperLedger.errors) -join '; ')"
}
$remainingOwned = [Collections.Generic.List[int]]::new()
foreach ($record in @($excelLedger.records)) {
    $process = Get-Process -Id ([int]$record.pid) -ErrorAction SilentlyContinue
    if ($process -and (Test-ExcelOracleProcessIdentity -Record $record -Process $process -ExpectedProcessName "EXCEL" -RunId $RunId)) {
        $remainingOwned.Add([int]$record.pid)
    }
}
if ($remainingOwned.Count -ne 0) { throw "run-excel-vba-oracle: owned Excel PIDs remain: $($remainingOwned -join ', ')" }

$remainingHelpers = [Collections.Generic.List[int]]::new()
foreach ($record in @($helperLedger.records)) {
    $process = Get-Process -Id ([int]$record.pid) -ErrorAction SilentlyContinue
    if ($process -and (Test-ExcelOracleProcessIdentity -Record $record -Process $process -ExpectedProcessName ([string]$record.process_name) -RunId $RunId)) {
        $remainingHelpers.Add([int]$record.pid)
    }
}
if ($remainingHelpers.Count -ne 0) { throw "run-excel-vba-oracle: owned guardian PIDs remain: $($remainingHelpers -join ', ')" }
if ($workerFailure) { throw $workerFailure }

$resultsPath = Join-Path $outputDirectory "results.json"
if (-not (Test-Path -LiteralPath $resultsPath)) { throw "run-excel-vba-oracle: worker did not produce results.json" }
$results = Get-Content -Raw -LiteralPath $resultsPath | ConvertFrom-Json
if ([string]::IsNullOrWhiteSpace($DiagnosticCaseId) -and (@($results.cases).Count -ne 5 -or -not [bool]$results.passed)) {
    throw "run-excel-vba-oracle: harness self-test did not produce five passing cases"
}
if (-not [string]::IsNullOrWhiteSpace($DiagnosticCaseId) -and @($results.cases).Count -ne 1) {
    throw "run-excel-vba-oracle: targeted diagnostic did not produce exactly one case"
}

$completedUtc = [DateTime]::UtcNow
$transcript = [ordered]@{
    schema = "oxvba.excel-vba-oracle-transcript.v1"
    run_id = $RunId
    environment_id = $EnvironmentId
    started_utc = $startedUtc.ToString("o")
    completed_utc = $completedUtc.ToString("o")
    duration_seconds = [Math]::Round(($completedUtc - $startedUtc).TotalSeconds, 3)
    supervisor_pid = $PID
    worker_pid = $worker.Id
    baseline_excel_pids = $baselineExcelPids
    timeout = $timedOut
    no_matrix_update = [bool]$NoMatrixUpdate
    certifying = $false
    diagnostic_only = -not [string]::IsNullOrWhiteSpace($DiagnosticCaseId)
    passed = [bool]$results.passed
    case_count = @($results.cases).Count
}
$transcript | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $outputDirectory "transcript.json") -Encoding utf8NoBOM

$caseLines = @($results.cases | ForEach-Object {
    "| $($_.id) | $($_.compile_status) | $($_.run_status) | $($_.passed) |"
})
$displayResult = if ([string]::IsNullOrWhiteSpace($DiagnosticCaseId)) { "PASS ($(@($results.cases).Count)/5 cases)" } else { "DIAGNOSTIC CAPTURED (case ``$DiagnosticCaseId``; expectation pass=$([bool]$results.passed))" }
$summary = @"
# Excel/VBA Oracle Harness Self-Test

- Run: ``$RunId``
- Environment: ``$EnvironmentId``
- Result: **$displayResult**
- Authority: development/oracle characterization only; noncertifying
- Credit: no canonical matrix, release, certification, or capability credit
- Ownership: newly created Excel HWNDs and guardian processes were sealed by PID, process start, name, and executable; fallback cleanup required the complete identity
- Compile authority: VBE Debug -> Compile VBAProject command ID 578; runtime invocation was never used as a compile check
- Modal safety: the PID-scoped UIA guardian was ready before each compile and runtime invocation

| Case | Compile | Runtime | Pass |
|---|---|---|---|
$($caseLines -join "`n")

Raw evidence: ``results.json``, ``transcript.json``, ``owned-processes.jsonl``, and per-case guardian/module artifacts in this run directory.
"@
Set-Content -LiteralPath (Join-Path $outputDirectory "summary.md") -Value $summary -Encoding utf8NoBOM

if ([string]::IsNullOrWhiteSpace($DiagnosticCaseId)) {
    Write-Output "excel-vba-oracle: PASS 5/5 (development/oracle, noncertifying, no matrix update)"
}
else {
    Write-Output "excel-vba-oracle: DIAGNOSTIC $DiagnosticCaseId captured (development/oracle, noncertifying, no matrix update)"
}
Write-Output "excel-vba-oracle: $outputDirectory"
