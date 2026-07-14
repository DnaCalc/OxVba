param(
    [ValidateSet("HarnessSelfTest")][string]$Suite = "HarnessSelfTest",
    [Parameter(Mandatory = $true)][string]$EnvironmentId,
    [switch]$NoMatrixUpdate,
    [switch]$PlanOnly,
    [string]$RunId = ("excel_vba_oracle_{0}" -f [Guid]::NewGuid().ToString("N")),
    [string]$OutputRoot = "artifacts/windows-x64/excel-vba-oracle",
    [ValidateRange(30, 1800)][int]$TimeoutSeconds = 600,
    [string]$DiagnosticCaseId = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "excel-vba-oracle-contract.ps1")
. (Join-Path $PSScriptRoot "excel-vba-oracle-job.ps1")
. (Join-Path $PSScriptRoot "excel-vba-oracle-bootstrap.ps1")

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

$cases = @(Get-ExcelOracleHarnessCases | Where-Object { -not [bool]$_.diagnostic_only })
if (-not [string]::IsNullOrWhiteSpace($DiagnosticCaseId)) {
    $cases = @(Get-ExcelOracleHarnessCases | Where-Object { $_.id -eq $DiagnosticCaseId })
    if ($cases.Count -ne 1) { throw "run-excel-vba-oracle: unknown diagnostic case '$DiagnosticCaseId'" }
}
if (@($cases.id | Select-Object -Unique).Count -ne $cases.Count -or @($cases | Where-Object { [string]::IsNullOrWhiteSpace([string]$_.id) }).Count -gt 0) {
    throw "run-excel-vba-oracle: selected case identities must be nonempty and unique"
}
$selectedCaseDescriptors = @(New-ExcelOracleSelectedCaseDescriptors -Cases $cases)
if ($selectedCaseDescriptors.Count -ne $cases.Count -or
    @($selectedCaseDescriptors | Where-Object { -not (Test-ExcelOracleSelectedCaseDescriptor -Descriptor $_) }).Count -gt 0) {
    throw "run-excel-vba-oracle: selected case descriptor sealing failed"
}
$selectedCaseDescriptorEnvelope = New-ExcelOracleSelectedCaseDescriptorEnvelope -Descriptors $selectedCaseDescriptors
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
    ownership_policy = "assign the waiting worker to a kill-on-close job before publishing mutation authority; launch Excel directly inside that containment; record PID+process-start+name+executable for Excel and guardian processes; validate classified identity before fallback cleanup"
    modal_policy = "start PID-scoped UIA guardian before command-ID-578 compile and runtime invocation; capture first; never auto-enable security/trust prompts"
    compile_policy = "VBE Debug -> Compile VBAProject command ID 578; Application.Run is never a compile check"
    cases = @($selectedCaseDescriptors | ForEach-Object {
        [ordered]@{
            id = $_.id
            expected_compile_status = $_.expected_compile_status
            expected_run_status = $_.expected_run_status
            module_sha256 = $_.module_sha256
            descriptor_sha256 = $_.descriptor_sha256
        }
    })
    selected_case_count = $cases.Count
    selected_case_ids = @($cases | ForEach-Object { [string]$_.id })
    selected_case_descriptor_digest = [string]$selectedCaseDescriptorEnvelope.aggregate_sha256
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
    [string[]]$lines = [string[]]::new(0)
    if (Test-Path -LiteralPath $Path) { $lines = [string[]]@(Get-Content -LiteralPath $Path) }
    return ConvertFrom-ExcelOracleOwnershipLedger -Lines ([string[]]$lines) -Kind $Kind -RunId $RunId -BaselineExcelPids $BaselineExcelPids -ExpectedCaseIds @($cases.id)
}

function Stop-RecordedOwnedResources {
    param(
        [Parameter(Mandatory = $true)][string]$OwnershipPath,
        [Parameter(Mandatory = $true)][string]$HelperOwnershipPath,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][int[]]$BaselineExcelPids
    )
    $excelLedger = Read-OwnershipLedger -Path $OwnershipPath -Kind excel -BaselineExcelPids $BaselineExcelPids
    $helperLedger = Read-OwnershipLedger -Path $HelperOwnershipPath -Kind guardian -BaselineExcelPids $BaselineExcelPids
    $authorityErrors = [Collections.Generic.List[string]]::new()
    foreach ($errorText in @($excelLedger.errors) + @($helperLedger.errors)) { $authorityErrors.Add([string]$errorText) }
    foreach ($record in @($excelLedger.records)) {
        try { $termination = Invoke-ExcelOracleRetainedProcessTermination -Record $record -ExpectedProcessName "EXCEL" -RunId $RunId }
        catch { $authorityErrors.Add("exact Excel identity could not be opened/terminated through one retained handle: $($_.Exception.Message)"); continue }
        if ($termination.state -eq "same-instance-conflict") {
            $authorityErrors.Add("Excel PID $($record.pid) has the recorded start but conflicting name/executable identity")
        }
    }
    foreach ($record in @($helperLedger.records)) {
        try { $termination = Invoke-ExcelOracleRetainedProcessTermination -Record $record -ExpectedProcessName ([string]$record.process_name) -RunId $RunId }
        catch { $authorityErrors.Add("exact guardian identity could not be opened/terminated through one retained handle: $($_.Exception.Message)"); continue }
        if ($termination.state -eq "same-instance-conflict") {
            $authorityErrors.Add("guardian PID $($record.pid) has the recorded start but conflicting name/executable identity")
        }
    }
    return @($authorityErrors)
}

$outputBase = if ([IO.Path]::IsPathRooted($OutputRoot)) { $OutputRoot } else { Join-Path $repoRoot $OutputRoot }
$runClaim = Enter-ExcelOracleRunClaim -OutputBase $outputBase -RunId $RunId
$outputDirectory = [string]$runClaim.output_directory
try {
$plan | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $outputDirectory "plan.json") -Encoding utf8NoBOM

$ownershipFile = Join-Path $outputDirectory "owned-processes.jsonl"
$helperOwnershipFile = Join-Path $outputDirectory "owned-helper-processes.jsonl"
$selectedCaseDescriptorFile = Join-Path $outputDirectory "selected-case-descriptors.json"
$selectedCaseDescriptorEnvelope | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $selectedCaseDescriptorFile -Encoding utf8NoBOM
$containmentReadyFile = Join-Path $outputDirectory "containment-ready.json"
$containmentToken = [Guid]::NewGuid().ToString("D")
$workerStdout = Join-Path $outputDirectory "worker.stdout.txt"
$workerStderr = Join-Path $outputDirectory "worker.stderr.txt"
$baselineExcelPids = @(Get-ExcelProcessIds)
$workerArguments = @(
    "-NoLogo", "-NoProfile", "-NonInteractive", "-STA", "-File", (Join-Path $PSScriptRoot "excel-vba-oracle-worker.ps1"),
    "-RunId", $RunId,
    "-OutputDirectory", $outputDirectory,
    "-OwnershipFile", $ownershipFile,
    "-HelperOwnershipFile", $helperOwnershipFile,
    "-ContainmentReadyFile", $containmentReadyFile,
    "-ContainmentToken", $containmentToken,
    "-SelectedCaseDescriptorFile", $selectedCaseDescriptorFile,
    "-SelectedCaseDescriptorDigest", [string]$selectedCaseDescriptorEnvelope.aggregate_sha256,
    "-CaseTimeoutSeconds", [string][Math]::Min(120, $TimeoutSeconds)
)
$startedUtc = [DateTime]::UtcNow
$containedWorker = Start-ExcelOracleContainedProcess -JobName "OxVbaExcelOracle-$PID-$containmentToken" -RunId $RunId -StartProcess {
    Start-Process -FilePath (Join-Path $PSHOME "pwsh.exe") -ArgumentList $workerArguments -PassThru -WindowStyle Hidden -RedirectStandardOutput $workerStdout -RedirectStandardError $workerStderr
}
$job = $containedWorker.job
$worker = $containedWorker.process
try {
    [ordered]@{
        schema = "oxvba.excel-vba-oracle-containment-ready.v1"
        run_id = $RunId
        containment_token = $containmentToken
        worker_pid = $worker.Id
        worker_process_start_utc = $worker.StartTime.ToUniversalTime().ToString("o")
        worker_executable_path = [string]$worker.Path
        worker_job_membership_verified = $true
        published_utc = [DateTime]::UtcNow.ToString("o")
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $containmentReadyFile -Encoding utf8NoBOM
}
catch {
    $containmentError = $_.Exception.Message
    try {
        $workerIdentity = [pscustomobject]@{
            run_id = $RunId; pid = $worker.Id; process_name = [string]$worker.ProcessName
            process_start_utc = $worker.StartTime.ToUniversalTime().ToString("o"); executable_path = [string]$worker.Path
        }
        [void](Invoke-ExcelOracleRetainedProcessTermination -Record $workerIdentity -ExpectedProcessName $worker.ProcessName -RunId $RunId -TimeoutMilliseconds 10000)
    }
    catch { $containmentError = "$containmentError; retained worker cleanup failed: $($_.Exception.Message)" }
    $job.Dispose()
    throw "run-excel-vba-oracle: worker containment could not be established before mutation authority: $containmentError"
}
$timedOut = $false
$workerFailure = $null
$cleanupAuthorityErrors = @()
$workerQuiesced = $false
$terminationFailure = $null
try {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while (-not $worker.HasExited -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 100
    }
    if (-not $worker.HasExited) {
        $timedOut = $true
        try { $job.Terminate() }
        catch { $terminationFailure = "worker Job termination failed: $($_.Exception.Message)" }
        $workerQuiesced = $worker.WaitForExit(10000)
        $workerFailure = "run-excel-vba-oracle: worker timed out after $TimeoutSeconds seconds$(if ($terminationFailure) { "; $terminationFailure" })"
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
    try { $job.Terminate() }
    catch { $cleanupAuthorityErrors += "job termination failed: $($_.Exception.Message)" }
    finally { $job.Dispose() }
    if (-not $workerQuiesced) { $workerQuiesced = $worker.WaitForExit(10000) }
    if ($workerQuiesced) {
        $cleanupAuthorityErrors += @(Stop-RecordedOwnedResources -OwnershipPath $ownershipFile -HelperOwnershipPath $helperOwnershipFile -BaselineExcelPids $baselineExcelPids)
    }
    else {
        $cleanupAuthorityErrors = @("exact worker process did not quiesce; ownership ledgers remain mutable and cleanup is unsafe")
    }
}
if ($cleanupAuthorityErrors.Count -gt 0) {
    throw "run-excel-vba-oracle: cleanup authority is uncertain: $($cleanupAuthorityErrors -join '; ')$(if ($workerFailure) { "; primary failure: $workerFailure" })"
}
$resultsPath = Join-Path $outputDirectory "results.json"
$results = $null
$resultsParseError = $null
if (Test-Path -LiteralPath $resultsPath) {
    try { $results = Get-Content -Raw -LiteralPath $resultsPath | ConvertFrom-Json -DateKind String }
    catch { $resultsParseError = $_.Exception.Message }
}
$excelLedger = Read-OwnershipLedger -Path $ownershipFile -Kind excel -BaselineExcelPids $baselineExcelPids
$helperLedger = Read-OwnershipLedger -Path $helperOwnershipFile -Kind guardian -BaselineExcelPids $baselineExcelPids
$remainingOwned = [Collections.Generic.List[int]]::new()
foreach ($record in @($excelLedger.records)) {
    $process = Get-Process -Id ([int]$record.pid) -ErrorAction SilentlyContinue
    $identityState = Get-ExcelOracleProcessIdentityState -Record $record -Process $process -ExpectedProcessName "EXCEL" -RunId $RunId
    if ($identityState -eq "exact") {
        $remainingOwned.Add([int]$record.pid)
    }
    elseif ($identityState -eq "same-instance-conflict") { throw "run-excel-vba-oracle: Excel residual identity conflict for PID $($record.pid)" }
}
if ($remainingOwned.Count -ne 0) { throw "run-excel-vba-oracle: owned Excel PIDs remain: $($remainingOwned -join ', ')" }

$remainingHelpers = [Collections.Generic.List[int]]::new()
foreach ($record in @($helperLedger.records)) {
    $process = Get-Process -Id ([int]$record.pid) -ErrorAction SilentlyContinue
    $identityState = Get-ExcelOracleProcessIdentityState -Record $record -Process $process -ExpectedProcessName ([string]$record.process_name) -RunId $RunId
    if ($identityState -eq "exact") {
        $remainingHelpers.Add([int]$record.pid)
    }
    elseif ($identityState -eq "same-instance-conflict") { throw "run-excel-vba-oracle: guardian residual identity conflict for PID $($record.pid)" }
}
if ($remainingHelpers.Count -ne 0) { throw "run-excel-vba-oracle: owned guardian PIDs remain: $($remainingHelpers -join ', ')" }
$workerExitCode = if ($workerQuiesced) { [int]$worker.ExitCode } else { -1 }
$postCleanup = Resolve-ExcelOraclePostCleanupResult `
    -Results $results `
    -ExcelLedger $excelLedger `
    -HelperLedger $helperLedger `
    -SelectedCaseDescriptors $selectedCaseDescriptors `
    -RunId $RunId `
    -ExpectedWorkerPid $worker.Id `
    -ExpectedContainmentToken $containmentToken `
    -ExpectedDiagnosticOnly (-not [string]::IsNullOrWhiteSpace($DiagnosticCaseId)) `
    -WorkerExitCode $workerExitCode `
    -WorkerQuiesced $workerQuiesced `
    -WorkerTimedOut $timedOut
if (-not [bool]$postCleanup.valid) {
    $parseContext = if ($resultsParseError) { "; results parse error: $resultsParseError" } else { "" }
    $workerContext = if ($workerFailure) { "; worker envelope: $workerFailure" } else { "" }
    throw "run-excel-vba-oracle: post-cleanup result/ledger authority is invalid: $($postCleanup.errors -join '; ')$parseContext$workerContext"
}
if ([string]$postCleanup.disposition -eq "pre-ownership-transport") {
    throw "run-excel-vba-oracle: first selected case failed before durable ownership after owned Job cleanup: $($postCleanup.transport_error); evidence '$outputDirectory'"
}
foreach ($caseResult in @($results.cases)) {
    if (-not (Test-ExcelOracleBootstrapWorkbook -Descriptor $caseResult.bootstrap_workbook)) {
        throw "run-excel-vba-oracle: controlled bootstrap workbook is missing, modified, or has invalid OPC relationship closure after worker cleanup for case '$($caseResult.id)'"
    }
}
if ([string]$postCleanup.disposition -eq "complete-case-failure") {
    $failureDetails = @($results.cases | Where-Object { -not [bool]$_.passed } | ForEach-Object {
        "$($_.id): compile=$($_.compile_status) run=$($_.run_status) transport=$($_.transport_error)"
    }) -join "; "
    throw "run-excel-vba-oracle: selected oracle case expectations failed after owned cleanup: $failureDetails; evidence '$outputDirectory'"
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
    containment_token = $containmentToken
    job_terminated_before_residual_audit = $true
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
}
finally {
    # Release only the exact stream/path returned by the successful CreateNew
    # claim. Failed run directories and evidence remain intact and fail closed.
    Exit-ExcelOracleRunClaim -Claim $runClaim
}
