$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "excel-vba-oracle-contract.ps1")
. (Join-Path $PSScriptRoot "excel-vba-oracle-job.ps1")
. (Join-Path $PSScriptRoot "excel-vba-oracle-bootstrap.ps1")

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

function Copy-TestJsonObject {
    param([Parameter(Mandatory = $true)]$Value)
    return ($Value | ConvertTo-Json -Depth 20 | ConvertFrom-Json -DateKind String)
}

function Get-TestSelectedCaseDescriptors {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$CaseIds)
    $catalog = @(New-ExcelOracleSelectedCaseDescriptors -Cases @(Get-ExcelOracleHarnessCases))
    $selected = [Collections.Generic.List[object]]::new()
    foreach ($id in $CaseIds) {
        $matches = @($catalog | Where-Object { [string]$_.id -ceq $id })
        if ($matches.Count -ne 1) { throw "test-excel-vba-oracle: unknown case descriptor '$id'" }
        $selected.Add($matches[0])
    }
    return @($selected)
}

function New-TestGuardianOperationEvents {
    param(
        [Parameter(Mandatory = $true)][string]$CaseId,
        [Parameter(Mandatory = $true)][ValidateSet("compile", "run")][string]$Phase,
        [Parameter(Mandatory = $true)][int]$ExcelPid,
        [Parameter(Mandatory = $true)][ValidateSet("none", "compile-error", "ambiguous-macro-failure", "runtime-error")][string]$DialogKind,
        [AllowNull()][string]$SelectedToken = $null,
        [AllowNull()][string]$ExpandedLine = $null
    )
    $base = if ($Phase -eq "compile") { 10 } else { 20 }
    $operationId = "$CaseId-$Phase"
    $events = [Collections.Generic.List[object]]::new()
    $events.Add([pscustomobject][ordered]@{
        schema = "oxvba.excel-vba-oracle-operation-state.v1"; event_type = "operation-armed"; run_id = "run-post"
        case_id = $CaseId; operation_id = $operationId; phase = $Phase; control_sequence = 1; event_sequence = $base + 1; observed_utc = "2026-07-14T00:00:01Z"
    })
    $events.Add([pscustomobject][ordered]@{
        schema = "oxvba.excel-vba-oracle-operation-state.v1"; event_type = "guardian-heartbeat"; run_id = "run-post"
        case_id = $CaseId; operation_id = $operationId; phase = $Phase; control_sequence = 1; event_sequence = $base + 2; observed_utc = "2026-07-14T00:00:02Z"
    })
    $observationId = "$operationId-observation"
    if ($DialogKind -eq "none") {
        $events.Add([pscustomobject][ordered]@{
            schema = "oxvba.excel-vba-oracle-window-observation.v1"; event_type = "ignored-top-level-window"; run_id = "run-post"
            observation_id = $observationId; case_id = $CaseId; operation_id = $operationId; control_sequence = 1; event_sequence = $base + 3
            phase = $Phase; excel_pid = $ExcelPid; observed_process_id = $ExcelPid; observed_utc = "2026-07-14T00:00:03Z"; capture_completed_utc = "2026-07-14T00:00:04Z"
            window_handle = "0x100"; classification = "unrecognized-modal"; disposition = "block-no-dismiss"; considered_dialog = $false; is_modal = $false
        })
    }
    else {
        $observation = [pscustomobject][ordered]@{
            schema = "oxvba.excel-vba-oracle-window-observation.v1"; event_type = "dialog-observation"; run_id = "run-post"
            observation_id = $observationId; case_id = $CaseId; operation_id = $operationId; control_sequence = 1; event_sequence = $base + 3
            phase = $Phase; excel_pid = $ExcelPid; observed_process_id = $ExcelPid; observed_utc = "2026-07-14T00:00:03Z"; capture_completed_utc = "2026-07-14T00:00:04Z"
            window_handle = "0x100"; classification = $DialogKind; disposition = "capture-then-dismiss"; considered_dialog = $true; is_modal = $true
            dialog_text = @("owned $DialogKind dialog"); selected_token = $SelectedToken; expanded_line = $ExpandedLine
        }
        $events.Add($observation)
        $events.Add([pscustomobject][ordered]@{
            schema = "oxvba.excel-vba-oracle-dismissal-result.v1"; event_type = "dismissal-result"; run_id = "run-post"
            observation_id = $observationId; case_id = $CaseId; operation_id = $operationId; control_sequence = 1; event_sequence = $base + 4
            phase = $Phase; excel_pid = $ExcelPid; window_handle = "0x100"; attempted_utc = "2026-07-14T00:00:05Z"
            requested_buttons = @("OK"); succeeded = $true; dismissed_button = "OK"
        })
    }
    return @($events)
}

function New-TestPostCleanupCase {
    param(
        [Parameter(Mandatory = $true)][string]$Id,
        [bool]$Passed = $true,
        [bool]$OwnershipRecorded = $true,
        [AllowNull()]$OwnedPid = 7001,
        [AllowNull()]$ObservedPid = 7001,
        [AllowNull()][string]$CompileStatus = $null,
        [AllowNull()][string]$RunStatus = $null,
        [AllowNull()]$TransportError = $null
    )
    $descriptor = @(Get-TestSelectedCaseDescriptors -CaseIds @($Id))[0]
    if ([string]::IsNullOrEmpty($CompileStatus)) { $CompileStatus = if (-not $Passed -and $OwnershipRecorded) { "no-dialog-unverified" } else { [string]$descriptor.expected_compile_status } }
    if ([string]::IsNullOrEmpty($RunStatus)) { $RunStatus = if (-not $Passed -and $OwnershipRecorded) { "not-run" } else { [string]$descriptor.expected_run_status } }
    $hasExecutionEvidence = $OwnershipRecorded -and $CompileStatus -ne "harness-error"
    $compileDialogKind = if ([string]$descriptor.evidence_contract -eq "compile-error-token-line-dismissal-v1") { "compile-error" } else { "none" }
    $compileEvents = if ($hasExecutionEvidence) { @(New-TestGuardianOperationEvents -CaseId $Id -Phase compile -ExcelPid ([int]$OwnedPid) -DialogKind $compileDialogKind -SelectedToken $descriptor.expected_selected_token -ExpandedLine $descriptor.expected_expanded_line) } else { @() }
    $runDialogKind = switch ([string]$descriptor.evidence_contract) {
        "clean-compile-ambiguous-macro-dismissal-v1" { "ambiguous-macro-failure"; break }
        "clean-compile-runtime-error-dismissal-v1" { "runtime-error"; break }
        default { "none"; break }
    }
    $runEvents = if ($hasExecutionEvidence -and $RunStatus -ne "not-run") { @(New-TestGuardianOperationEvents -CaseId $Id -Phase run -ExcelPid ([int]$OwnedPid) -DialogKind $runDialogKind) } else { @() }
    $compileHealthy = $hasExecutionEvidence
    $runHealthy = $hasExecutionEvidence -and $RunStatus -ne "not-run"
    $compileErrorComplete = $hasExecutionEvidence -and $compileDialogKind -eq "compile-error"
    $ambiguousComplete = $hasExecutionEvidence -and $runDialogKind -eq "ambiguous-macro-failure"
    $runtimeErrorComplete = $hasExecutionEvidence -and $runDialogKind -eq "runtime-error"
    $authoritativeEvidencePassed = switch ([string]$descriptor.evidence_contract) {
        "compile-error-token-line-dismissal-v1" { $compileHealthy -and $compileErrorComplete; break }
        "clean-compile-ambiguous-macro-dismissal-v1" { $compileHealthy -and $runHealthy -and $ambiguousComplete; break }
        "clean-compile-runtime-error-dismissal-v1" { $compileHealthy -and $runHealthy -and $runtimeErrorComplete; break }
        default { $compileHealthy -and $runHealthy; break }
    }
    $bootstrap = if ($OwnershipRecorded) {
        [ordered]@{
            schema = "oxvba.excel-vba-oracle-bootstrap-workbook.v1"
            path = "C:\fixture\oracle-bootstrap.xlsx"
            sha256 = "sha256:$('a' * 64)"
            sha256_after = "sha256:$('a' * 64)"
            package_parts = @("[Content_Types].xml", "_rels/.rels", "xl/workbook.xml", "xl/_rels/workbook.xml.rels", "xl/worksheets/sheet1.xml")
            macro_free = $true
        }
    } else { $null }
    $document = [pscustomobject][ordered]@{
        schema = "oxvba.excel-vba-oracle-case-result.v1"
        id = $Id
        purpose = $descriptor.purpose
        passed = $Passed
        owned_excel_pid = if ($OwnershipRecorded) { $OwnedPid } else { $null }
        observed_excel_pid = $ObservedPid
        excel_ownership_recorded = $OwnershipRecorded
        selected_case_descriptor_sha256 = $descriptor.descriptor_sha256
        module_name = $descriptor.module_name
        module_path = "C:\fixture\OracleSelfTest.bas"
        module_sha256 = $descriptor.module_sha256
        case_diagnostic_only = [bool]$descriptor.diagnostic_only
        evidence_contract = $descriptor.evidence_contract
        compile_status = $CompileStatus
        expected_compile_status = $descriptor.expected_compile_status
        compile_command = if ($hasExecutionEvidence) { [ordered]@{ schema = "oxvba.excel-vba-oracle-compile-command.v1"; id = 578; caption = "Compile VBAProject"; enabled_before = $true; enabled_after = $CompileStatus -eq "no-dialog-unverified" } } else { $null }
        compile_execution = if ($hasExecutionEvidence) { [ordered]@{ schema = "oxvba.excel-vba-oracle-compile-execution.v1"; return_value = $null; exception = $null } } else { $null }
        compile_context = if ($hasExecutionEvidence) { [ordered]@{
            schema = "oxvba.excel-vba-oracle-compile-context.v1"; injected_project_name = "VBAProject"; injected_project_file_name = "C:\fixture\oracle-bootstrap.xlsx"
            injected_module_name = $descriptor.module_name; selection_before_execute = $null; injected_source = $descriptor.module_source
            injected_source_sha256 = $descriptor.module_sha256; selected_source_sha256 = $descriptor.module_sha256
            authority_before_execute = [ordered]@{ schema = "oxvba.excel-vba-oracle-compile-authority-snapshot.v1"; stage = "immediately-before-execute"; captured_utc = "2026-07-14T00:00:01Z"; active_project_is_injected_project = $true; active_module_is_injected_module = $true; active_code_pane_is_injected_code_pane = $true; active_project_name = "VBAProject"; active_module_name = $descriptor.module_name; injected_source_sha256 = $descriptor.module_sha256; expected_source_sha256 = $descriptor.module_sha256 }
            authority_after_execute = [ordered]@{ schema = "oxvba.excel-vba-oracle-compile-authority-snapshot.v1"; stage = "immediately-after-execute"; captured_utc = "2026-07-14T00:00:02Z"; active_project_is_injected_project = $true; active_module_is_injected_module = $true; active_code_pane_is_injected_code_pane = $true; active_project_name = "VBAProject"; active_module_name = $descriptor.module_name; injected_source_sha256 = $descriptor.module_sha256; expected_source_sha256 = $descriptor.module_sha256 }
            selection_after_execute_diagnostic_only = $null
        } } else { $null }
        post_dismiss_selection_diagnostic_only = $null
        compile_dialogs = @($compileEvents | Where-Object { [string]$_.event_type -eq "dialog-observation" })
        compile_window_observations = @($compileEvents)
        run_procedure = $descriptor.run_procedure
        run_status = $RunStatus
        expected_run_status = $descriptor.expected_run_status
        run_value = if (-not $runHealthy) { $null } elseif ($null -ne $descriptor.expected_runtime_err) {
            [ordered]@{ number = [int]$descriptor.expected_runtime_err.number; source = [string]$descriptor.expected_runtime_err.source; description = [string]$descriptor.expected_runtime_err.description; help_file = [string]$descriptor.expected_runtime_err.help_file; help_context = [int]$descriptor.expected_runtime_err.help_context; erl = [int]$descriptor.expected_runtime_err.erl } | ConvertTo-Json -Compress
        } elseif ([string]$descriptor.id -eq "ambiguous-macro-failure") { "oracle-ambiguous-entry-observed:Cannot run the macro" }
          else { $descriptor.expected_value }
        runtime_err = if ($runHealthy -and $null -ne $descriptor.expected_runtime_err) { [ordered]@{
            schema = "oxvba.excel-vba-oracle-runtime-err.v1"; number = [int]$descriptor.expected_runtime_err.number; source = [string]$descriptor.expected_runtime_err.source
            description = [string]$descriptor.expected_runtime_err.description; help_file = [string]$descriptor.expected_runtime_err.help_file
            help_context = [int]$descriptor.expected_runtime_err.help_context; erl = [int]$descriptor.expected_runtime_err.erl
        } } else { $null }
        macro_failure_disposition = if (-not $runHealthy) { $null }
            elseif ([string]$descriptor.id -eq "ambiguous-macro-failure") { "missing-macro" }
            elseif ([string]$descriptor.id -eq "runtime-unhandled-modal") { "non-macro-runtime-failure" }
            else { $null }
        runtime_measurement = if ($hasExecutionEvidence) { [ordered]@{
            schema = "oxvba.excel-vba-oracle-runtime-measurement.v1"; measured_utc = "2026-07-14T00:00:05Z"; access_vbom = $true
            invocation_entry = $descriptor.run_procedure; invocation_entry_exists = $null -ne $descriptor.run_procedure
            macro_probe_target = $descriptor.macro_probe_target; macro_probe_target_exists = $null -ne $descriptor.macro_probe_target -and $null -ne $descriptor.run_procedure -and [string]$descriptor.macro_probe_target -ceq [string]$descriptor.run_procedure; automation_security = 1
            macros_configured_for_automation = $true; invocation_entry_observed = $runHealthy
            invocation_observation = if (-not $runHealthy) { $null }
                elseif ([string]$descriptor.id -eq "ambiguous-macro-failure") { "case-specific-return-sentinel" }
                elseif ([string]$descriptor.id -eq "runtime-unhandled-modal") { "owned-runtime-error-modal" }
                else { "qualified-entry-returned" }
            macros_runnable_entry = $runHealthy
        } } else { $null }
        transport_error = $TransportError
        run_dialogs = @($runEvents)
        evidence_status = if ($hasExecutionEvidence) { [ordered]@{
            schema = "oxvba.excel-vba-oracle-evidence-status.v1"; guardian_healthy_before_cleanup = $true
            compile_operation_healthy = $compileHealthy; run_operation_healthy = $runHealthy; compile_error_modal_complete = $compileErrorComplete
            ambiguous_macro_modal_and_dismissal_complete = $ambiguousComplete; runtime_error_modal_and_dismissal_complete = $runtimeErrorComplete
            authoritative_evidence_passed = $authoritativeEvidencePassed
        } } else { $null }
        cleanup_status = if ($OwnershipRecorded) { "owned-process-zero" } else { "not-run" }
        cleanup_authority_errors = @()
        bootstrap_workbook = $bootstrap
        defect_declaration = $null
    }
    return $document
}

function New-TestPostCleanupResults {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Cases,
        [string]$RunId = "run-post",
        [int]$WorkerPid = 43210,
        [string]$ContainmentToken = "11111111-2222-3333-4444-555555555555",
        [bool]$DiagnosticOnly = $false,
        [AllowNull()]$AggregatePassed = $null,
        [AllowNull()][string[]]$SelectedCaseIds = $null
    )
    $passed = if ($null -eq $AggregatePassed) { @($Cases | Where-Object { -not [bool]$_.passed }).Count -eq 0 } else { [bool]$AggregatePassed }
    if ($null -eq $SelectedCaseIds) { $SelectedCaseIds = @($Cases | ForEach-Object { [string]$_.id }) }
    $descriptors = @(Get-TestSelectedCaseDescriptors -CaseIds $SelectedCaseIds)
    $document = [pscustomobject][ordered]@{
        schema = "oxvba.excel-vba-oracle-results.v1"
        run_id = $RunId
        generated_utc = "2026-07-14T00:00:03Z"
        worker_pid = $WorkerPid
        containment_token = $ContainmentToken
        containment_authority = [pscustomobject][ordered]@{
            schema = "oxvba.excel-vba-oracle-containment-ready.v1"
            run_id = $RunId
            containment_token = $ContainmentToken
            worker_pid = $WorkerPid
            worker_process_start_utc = "2026-07-14T00:00:01Z"
            worker_executable_path = "C:\Program Files\PowerShell\7\pwsh.exe"
            worker_job_membership_verified = $true
            published_utc = "2026-07-14T00:00:02Z"
        }
        selected_case_descriptor_digest = Get-ExcelOracleSelectedCaseDescriptorSequenceDigest -Descriptors $descriptors
        diagnostic_only = $DiagnosticOnly
        cases = @($Cases)
        passed = $passed
    }
    return ($document | ConvertTo-Json -Depth 20 | ConvertFrom-Json -DateKind String)
}

function New-TestPostCleanupLedger {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$CaseIds,
        [int]$FirstPid = 7001,
        [switch]$Guardian
    )
    $records = [Collections.Generic.List[object]]::new()
    for ($index = 0; $index -lt $CaseIds.Count; $index++) {
        $records.Add([pscustomobject]@{ case_id = $CaseIds[$index]; pid = if ($Guardian) { 8001 + $index } else { $FirstPid + $index } })
    }
    return [pscustomobject]@{ records = @($records); errors = @() }
}

function Invoke-TestPostCleanupResolution {
    param(
        [Parameter(Mandatory = $true)][AllowNull()]$Results,
        [Parameter(Mandatory = $true)]$ExcelLedger,
        [Parameter(Mandatory = $true)]$HelperLedger,
        [Parameter(Mandatory = $true)][string[]]$ExpectedCaseIds,
        [int]$WorkerExitCode = 0,
        [bool]$DiagnosticOnly = $false,
        [int]$WorkerPid = 43210,
        [string]$WorkerStartUtc = "2026-07-14T00:00:01Z",
        [string]$WorkerExecutablePath = "C:\Program Files\PowerShell\7\pwsh.exe",
        [string]$ContainmentToken = "11111111-2222-3333-4444-555555555555",
        [bool]$WorkerQuiesced = $true,
        [bool]$WorkerTimedOut = $false,
        [AllowNull()][object[]]$SelectedCaseDescriptors = $null
    )
    if ($null -eq $SelectedCaseDescriptors) { $SelectedCaseDescriptors = @(Get-TestSelectedCaseDescriptors -CaseIds $ExpectedCaseIds) }
    return Resolve-ExcelOraclePostCleanupResult -Results $Results -ExcelLedger $ExcelLedger -HelperLedger $HelperLedger `
        -SelectedCaseDescriptors $SelectedCaseDescriptors -RunId "run-post" -ExpectedWorkerPid $WorkerPid -ExpectedWorkerStartUtc $WorkerStartUtc `
        -ExpectedWorkerExecutablePath $WorkerExecutablePath -ExpectedContainmentToken $ContainmentToken `
        -ExpectedDiagnosticOnly $DiagnosticOnly -WorkerExitCode $WorkerExitCode -WorkerQuiesced $WorkerQuiesced -WorkerTimedOut $WorkerTimedOut
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
    return $body -match 'Invoke-ExcelOracleRetainedProcessTermination' -and
        $body -notmatch 'Get-Process|\.Kill\(\)|Stop-Process'
}

function Test-WorkerEvidenceGatedAcceptanceShape {
    param([Parameter(Mandatory = $true)][string]$WorkerSource, [Parameter(Mandatory = $true)][string]$ContractSource)
    return $WorkerSource -match '\$passed\s*=\s*\$behaviorPassed\s+-and\s+\$guardianHealthy\s+-and\s+\$authoritativeEvidencePassed' -and
        $ContractSource -match 'function Test-CompileErrorEvidence' -and
        $ContractSource -match 'function Test-AmbiguousMacroEvidence' -and
        $ContractSource -match 'function Test-LinkedSuccessfulDismissal' -and
        $ContractSource -match 'passed value disagrees with derived behavior/evidence'
}

function Test-WorkerExactPidAttachmentShape {
    param([Parameter(Mandatory = $true)][string]$Source)
    return $Source -match 'EnumWindows\(' -and
        $Source -match 'EnumChildWindows\(' -and
        $Source -match 'GetWindowThreadProcessId' -and
        $Source -match 'topLevelProcessId != expectedProcessId' -and
        $Source -match 'childProcessId == expectedProcessId' -and
        $Source -match 'const int WindowLimit = 512' -and
        $Source -match 'Truncated = truncated' -and
        $Source -match 'Succeeded = completed && !truncated' -and
        $Source -match 'Test-ExcelOracleWindowEnumerationAuthority' -and
        $Source -match 'Select-Object -First 128' -and
        $Source -match 'foreach \(\$window in \$ownedWindows\)' -and
        $Source -match 'TryGetNativeObjectFromWindow\(\[IntPtr\]\[int64\]\$window\.Hwnd' -and
        $Source -match 'Resolve-ExcelOracleAttachmentCandidate' -and
        $Source -match '\[int\]\$ApplicationPid -ne \$ExpectedProcessId' -and
        $Source -match '\[string\]\$Candidate\.ClassName -cne "EXCEL7"' -and
        $Source -match 'Write-ExcelAttachmentDiagnostic' -and
        $Source -match 'observation_limit = 256' -and
        $Source -match 'blocked-owned-window' -and
        $Source -match 'ProcessStartInfo' -and
        $Source -match 'ArgumentList\.Add\("/x"\)' -and
        $Source -match 'ArgumentList\.Add\(\[string\]\$BootstrapWorkbook\.path\)' -and
        $Source -match 'ArgumentList\.Count -ne 2' -and
        $Source -match 'ArgumentList -contains "/n"' -and
        $Source -match 'oracle-bootstrap\.xlsx' -and
        $Source -match 'attached Excel workbook does not match the controlled bootstrap path' -and
        $Source -notmatch 'MainWindowHandle|GetActiveObject|New-Object\s+-ComObject|Workbooks\.Add\(|ArgumentList\.Add\("/n"\)'
}

function Test-RunnerEmptyLedgerShape {
    param([Parameter(Mandatory = $true)][string]$Source)
    $match = [regex]::Match($Source, '(?s)function Read-OwnershipLedger\s*\{(?<body>.*?)\r?\n\}\r?\n\r?\nfunction Stop-RecordedOwnedResources')
    if (-not $match.Success) { return $false }
    $body = $match.Groups['body'].Value
    return $body -match '\[string\[\]\]\$lines = \[string\[\]\]::new\(0\)' -and
        $body -match '-Lines \(\[string\[\]\]\$lines\)'
}

function Test-RetainedHandleAuthorityShape {
    param([Parameter(Mandatory = $true)][string]$Source)
    $match = [regex]::Match($Source, '(?s)function Invoke-ExcelOracleRetainedProcessTermination\s*\{(?<body>.*?)\r?\n\}')
    if (-not $match.Success) { return $false }
    $body = $match.Groups['body'].Value
    return @([regex]::Matches($body, '\[ExcelOracleRetainedProcess\]::Open')).Count -eq 1 -and
        $body -match 'Get-ExcelOracleRetainedProcessIdentityState.+-RetainedProcess \$retained' -and
        $body -match '\$retained\.TerminateAndWait' -and
        $body -notmatch 'Get-Process|GetProcessById|\.Kill\('
}

function Test-CompileSnapshotBorrowedAliasShape {
    param([Parameter(Mandatory = $true)][string]$Source)
    $match = [regex]::Match($Source, '(?s)function Get-CompileAuthoritySnapshot\s*\{(?<body>.*?)\r?\n\}\r?\n\r?\nfunction Test-VbomProcedureExists')
    if (-not $match.Success) { return $false }
    $body = $match.Groups['body'].Value
    return $body -match 'ActiveVBProject' -and $body -match 'ActiveCodePane' -and
        $body -notmatch 'Release-ComObject|FinalReleaseComObject'
}

function Test-JobContainsPreLedgerChild {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [ValidateSet("Terminate", "Dispose")][string]$CloseMode = "Terminate"
    )
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
        Assert-True ($job.ContainsProcess($worker.Handle)) "$Label membership must be proven before simulated mutation authority"
        New-Item -ItemType File -Force -Path $readyFile | Out-Null
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        while ([DateTime]::UtcNow -lt $deadline -and -not (Test-Path -LiteralPath $childPidFile)) { Start-Sleep -Milliseconds 20 }
        Assert-True (Test-Path -LiteralPath $childPidFile) "$Label contained child must start before simulated ledger write"
        $childPid = [int](Get-Content -Raw -LiteralPath $childPidFile)
        $childProcess = Get-Process -Id $childPid -ErrorAction Stop
        if ($CloseMode -eq "Terminate") { $job.Terminate() } else { $job.Dispose() }
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

function Test-JobKillsOnAbruptSupervisorDeath {
    $directory = Join-Path ([IO.Path]::GetTempPath()) "oxvba-oracle-job-abrupt-$([Guid]::NewGuid().ToString('N'))"
    [void][IO.Directory]::CreateDirectory($directory)
    $childPidFile = Join-Path $directory "child.pid"
    $jobScript = (Join-Path $PSScriptRoot "excel-vba-oracle-job.ps1").Replace("'", "''")
    $payload = @"
. '$jobScript'
`$job = [ExcelOracleJob]::new('OxVbaOracleAbrupt-$([Guid]::NewGuid().ToString('N'))')
`$child = Start-Process -FilePath (Join-Path `$PSHOME 'pwsh.exe') -ArgumentList @('-NoLogo','-NoProfile','-Command','Start-Sleep -Seconds 120') -PassThru -WindowStyle Hidden
`$job.AssignProcess(`$child.Handle)
if (-not `$job.ContainsProcess(`$child.Handle)) { throw 'membership failed' }
Set-Content -LiteralPath '$($childPidFile.Replace("'", "''"))' -Value `$child.Id -Encoding ascii
Start-Sleep -Seconds 120
"@
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($payload))
    $supervisor = $null
    $child = $null
    try {
        $supervisor = Start-Process -FilePath (Join-Path $PSHOME "pwsh.exe") -ArgumentList @("-NoLogo", "-NoProfile", "-EncodedCommand", $encoded) -PassThru -WindowStyle Hidden
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        while ([DateTime]::UtcNow -lt $deadline -and -not (Test-Path -LiteralPath $childPidFile)) { Start-Sleep -Milliseconds 20 }
        Assert-True (Test-Path -LiteralPath $childPidFile) "abrupt supervisor test must publish contained child PID"
        $child = Get-Process -Id ([int](Get-Content -Raw -LiteralPath $childPidFile)) -ErrorAction Stop
        $supervisor.Kill()
        [void]$supervisor.WaitForExit(10000)
        [void]$child.WaitForExit(10000)
        Assert-True $child.HasExited "kill-on-close Job must terminate the child after abrupt supervisor death"
    }
    finally {
        if ($supervisor -and -not $supervisor.HasExited) { try { $supervisor.Kill() } catch { } }
        if ($child -and -not $child.HasExited) { try { $child.Kill() } catch { } }
        Remove-Item -LiteralPath $directory -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-RetainedHandleTerminationAuthority {
    $child = Start-Process -FilePath (Join-Path $PSHOME "pwsh.exe") -ArgumentList @("-NoLogo", "-NoProfile", "-Command", "Start-Sleep -Seconds 120") -PassThru -WindowStyle Hidden
    try {
        $record = [pscustomobject]@{
            run_id = "retained-test"; pid = $child.Id; process_name = [string]$child.ProcessName
            process_start_utc = $child.StartTime.ToUniversalTime().ToString("o"); executable_path = [string]$child.Path
        }
        $conflict = $record | Select-Object *
        $conflict.executable_path = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetFileName($child.Path))
        $rejected = Invoke-ExcelOracleRetainedProcessTermination -Record $conflict -ExpectedProcessName $child.ProcessName -RunId "retained-test"
        Assert-Equal "same-instance-conflict" $rejected.state "adversarial same-PID/path mutation must be rejected on retained handle"
        Assert-True (-not $child.HasExited) "identity conflict must not terminate the process"
        $terminated = Invoke-ExcelOracleRetainedProcessTermination -Record $record -ExpectedProcessName $child.ProcessName -RunId "retained-test"
        Assert-Equal "exact" $terminated.state "exact retained identity state"
        Assert-True ([bool]$terminated.terminated -and $child.WaitForExit(5000)) "exact retained handle must terminate and wait for the same process object"
    }
    finally { if (-not $child.HasExited) { try { $child.Kill() } catch { } } }
}

foreach ($fileName in @(
    "excel-vba-oracle-contract.ps1",
    "excel-vba-oracle-bootstrap.ps1",
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
foreach ($productionFile in @("excel-vba-oracle-contract.ps1", "excel-vba-oracle-bootstrap.ps1", "excel-vba-oracle-job.ps1", "excel-vba-oracle-guardian.ps1", "excel-vba-oracle-worker.ps1", "run-excel-vba-oracle.ps1")) {
    $productionSource = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot $productionFile)
    Assert-True ($productionSource -notmatch '\.Kill\(') "$productionFile must not use Process.Kill outside harmless offline test fixtures"
}

$bootstrapDirectory = Join-Path ([IO.Path]::GetTempPath()) "oxvba-oracle-bootstrap-$([Guid]::NewGuid().ToString('N'))"
[void][IO.Directory]::CreateDirectory($bootstrapDirectory)
try {
    $bootstrapA = New-ExcelOracleBootstrapWorkbook -Path (Join-Path $bootstrapDirectory "a.xlsx")
    $bootstrapB = New-ExcelOracleBootstrapWorkbook -Path (Join-Path $bootstrapDirectory "b.xlsx")
    Assert-Equal $bootstrapA.sha256 $bootstrapB.sha256 "controlled OpenXML bootstrap package must be byte deterministic"
    Assert-Equal $true ([bool]$bootstrapA.macro_free) "controlled OpenXML bootstrap must declare macro-free content"
    Assert-True (Test-ExcelOracleBootstrapWorkbook -Descriptor $bootstrapA) "controlled OpenXML bootstrap must pass hash, part, XML, content-type, and OPC relationship closure"
    $archive = [IO.Compression.ZipFile]::OpenRead([string]$bootstrapA.path)
    try {
        $entryNames = @($archive.Entries | ForEach-Object FullName)
        $expectedParts = @("[Content_Types].xml", "_rels/.rels", "xl/workbook.xml", "xl/_rels/workbook.xml.rels", "xl/worksheets/sheet1.xml")
        Assert-Equal ($expectedParts -join ",") ($entryNames -join ",") "controlled OpenXML bootstrap part order and set"
        Assert-Equal 0 @($entryNames | Where-Object { $_ -match '(?i)vbaProject|macrosheet|xl4' }).Count "controlled OpenXML bootstrap must contain no macro parts"
        foreach ($entry in $archive.Entries) {
            $reader = [IO.StreamReader]::new($entry.Open(), [Text.UTF8Encoding]::new($false))
            try { $xml = [xml]$reader.ReadToEnd() }
            finally { $reader.Dispose() }
            Assert-True ($null -ne $xml.DocumentElement) "OpenXML bootstrap part '$($entry.FullName)' must be well-formed XML"
        }
        $contentTypesEntry = $archive.GetEntry("[Content_Types].xml")
        $contentTypesReader = [IO.StreamReader]::new($contentTypesEntry.Open(), [Text.UTF8Encoding]::new($false))
        try { $contentTypesText = $contentTypesReader.ReadToEnd() }
        finally { $contentTypesReader.Dispose() }
        Assert-True ($contentTypesText -match 'spreadsheetml\.sheet\.main\+xml' -and $contentTypesText -notmatch '(?i)macroEnabled') "bootstrap content types must describe an ordinary macro-free .xlsx workbook"
    }
    finally { $archive.Dispose() }

    $missingBootstrap = New-ExcelOracleBootstrapWorkbook -Path (Join-Path $bootstrapDirectory "missing.xlsx")
    Remove-Item -LiteralPath $missingBootstrap.path -Force
    Assert-True (-not (Test-ExcelOracleBootstrapWorkbook -Descriptor $missingBootstrap)) "missing bootstrap package must fail closed"

    $modifiedBootstrap = New-ExcelOracleBootstrapWorkbook -Path (Join-Path $bootstrapDirectory "modified.xlsx")
    [IO.File]::AppendAllText([string]$modifiedBootstrap.path, "modified", [Text.UTF8Encoding]::new($false))
    Assert-True (-not (Test-ExcelOracleBootstrapWorkbook -Descriptor $modifiedBootstrap)) "modified bootstrap package must fail its recorded hash"

    $brokenRelationship = New-ExcelOracleBootstrapWorkbook -Path (Join-Path $bootstrapDirectory "broken-relationship.xlsx")
    $updateArchive = [IO.Compression.ZipFile]::Open([string]$brokenRelationship.path, [IO.Compression.ZipArchiveMode]::Update)
    try {
        $relationshipEntry = $updateArchive.GetEntry("xl/_rels/workbook.xml.rels")
        $relationshipStream = $relationshipEntry.Open()
        try {
            $relationshipStream.SetLength(0)
            $brokenBytes = [Text.UTF8Encoding]::new($false).GetBytes('<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/missing.xml"/></Relationships>')
            $relationshipStream.Write($brokenBytes, 0, $brokenBytes.Length)
        }
        finally { $relationshipStream.Dispose() }
    }
    finally { $updateArchive.Dispose() }
    $brokenRelationship.sha256 = "sha256:$((Get-FileHash -LiteralPath $brokenRelationship.path -Algorithm SHA256).Hash.ToLowerInvariant())"
    Assert-True (-not (Test-ExcelOracleBootstrapWorkbook -Descriptor $brokenRelationship)) "hash-consistent OPC relationship with a missing target must fail closure"
}
finally { Remove-Item -LiteralPath $bootstrapDirectory -Recurse -Force -ErrorAction SilentlyContinue }

Test-JobContainsPreLedgerChild -Label "excel-before-ledger"
Test-JobContainsPreLedgerChild -Label "guardian-before-ledger"
Test-JobContainsPreLedgerChild -Label "dispose-only" -CloseMode Dispose
Test-JobKillsOnAbruptSupervisorDeath
Test-RetainedHandleTerminationAuthority
$jobSource = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot "excel-vba-oracle-job.ps1")
Assert-True (Test-RetainedHandleAuthorityShape -Source $jobSource) "fallback identity query, termination, and wait must share one retained SafeProcessHandle"
$reopenedPidMutation = $jobSource.Replace('$retained.TerminateAndWait($TimeoutMilliseconds)', '[Diagnostics.Process]::GetProcessById([int]$Record.pid).Kill()')
Assert-True (-not (Test-RetainedHandleAuthorityShape -Source $reopenedPidMutation)) "mutation: reopening a PID between identity query and termination must be rejected"
$injectedStartFailure = $false
try {
    [void](Start-ExcelOracleContainedProcess -JobName "OxVbaOracleInjectedStartFailure-$([Guid]::NewGuid().ToString('N'))" -RunId "run-injected-start-failure" -StartProcess { throw "injected start failure" })
}
catch { $injectedStartFailure = $_.Exception.Message -match "contained process start failed deterministically: injected start failure" }
Assert-True $injectedStartFailure "Job creation and process start must share one deterministic cleanup scope under injected start failure"
Assert-True ($jobSource -match '(?s)function Start-ExcelOracleContainedProcess.+?\$job = \[ExcelOracleJob\]::new.+?\$process = & \$StartProcess.+?catch.+?\$job\.Terminate\(\).+?finally \{ \$job\.Dispose\(\) \}') "contained start failure scope must terminate/dispose its Job before surfacing failure"
$script:assignmentFailureChild = $null
$assignmentFailure = $false
try {
    [void](Start-ExcelOracleContainedProcess -JobName "OxVbaOracleAssignmentFailure-$([Guid]::NewGuid().ToString('N'))" -RunId "run-assignment-failure" `
        -StartProcess { $script:assignmentFailureChild = Start-Process -FilePath (Join-Path $PSHOME "pwsh.exe") -ArgumentList @("-NoLogo", "-NoProfile", "-Command", "Start-Sleep -Seconds 120") -PassThru -WindowStyle Hidden; $script:assignmentFailureChild } `
        -AssignProcess { param($Job, $Process) throw "injected assignment failure" })
}
catch { $assignmentFailure = $_.Exception.Message -match "injected assignment failure" }
Assert-True ($assignmentFailure -and $null -ne $script:assignmentFailureChild -and $script:assignmentFailureChild.WaitForExit(5000) -and
    $null -eq (Get-Process -Id $script:assignmentFailureChild.Id -ErrorAction SilentlyContinue)) "injected Job assignment failure must terminate the real waiting child with zero residue"
$script:assignmentFailureChild.Dispose()
$script:membershipFailureChild = $null
$membershipFailure = $false
try {
    [void](Start-ExcelOracleContainedProcess -JobName "OxVbaOracleMembershipFailure-$([Guid]::NewGuid().ToString('N'))" -RunId "run-membership-failure" `
        -StartProcess { $script:membershipFailureChild = Start-Process -FilePath (Join-Path $PSHOME "pwsh.exe") -ArgumentList @("-NoLogo", "-NoProfile", "-Command", "Start-Sleep -Seconds 120") -PassThru -WindowStyle Hidden; $script:membershipFailureChild } `
        -TestMembership { param($Job, $Process) $false })
}
catch { $membershipFailure = $_.Exception.Message -match "not a member of the kill-on-close Job" }
Assert-True ($membershipFailure -and $null -ne $script:membershipFailureChild -and $script:membershipFailureChild.WaitForExit(5000) -and
    $null -eq (Get-Process -Id $script:membershipFailureChild.Id -ErrorAction SilentlyContinue)) "injected Job membership failure must terminate the real assigned waiting child with zero residue"
$script:membershipFailureChild.Dispose()

$completeIds = @("success", "compile-failure")
$completeCases = @(
    (New-TestPostCleanupCase -Id $completeIds[0] -OwnedPid 7001 -ObservedPid 7001),
    (New-TestPostCleanupCase -Id $completeIds[1] -OwnedPid 7002 -ObservedPid 7002)
)
$completeResults = New-TestPostCleanupResults -Cases $completeCases
$completeExcelLedger = New-TestPostCleanupLedger -CaseIds $completeIds -FirstPid 7001
$completeHelperLedger = New-TestPostCleanupLedger -CaseIds $completeIds -Guardian
$completeResolution = Invoke-TestPostCleanupResolution -Results $completeResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds
Assert-True ([bool]$completeResolution.valid -and [string]$completeResolution.disposition -eq "complete-success") "post-cleanup validator complete success envelope: $(@($completeResolution.errors) -join '; ')"
$failedCompleteCases = @(
    (New-TestPostCleanupCase -Id $completeIds[0] -Passed $false -OwnedPid 7001 -ObservedPid 7001 -TransportError "behavior mismatch"),
    (New-TestPostCleanupCase -Id $completeIds[1] -OwnedPid 7002 -ObservedPid 7002)
)
$failedCompleteResults = New-TestPostCleanupResults -Cases $failedCompleteCases -AggregatePassed $false
$failedCompleteResolution = Invoke-TestPostCleanupResolution -Results $failedCompleteResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds -WorkerExitCode 1
Assert-True ([bool]$failedCompleteResolution.valid -and [string]$failedCompleteResolution.disposition -eq "complete-case-failure") "fully owned failed cases must surface only through the failed aggregate exit envelope"

$fiveSelectedIds = @("success", "compile-failure", "ambiguous-macro-failure", "intrinsic-shadow", "runtime-full-err")
$fiveCompleteCases = [Collections.Generic.List[object]]::new()
for ($index = 0; $index -lt $fiveSelectedIds.Count; $index++) {
    $fiveCompleteCases.Add((New-TestPostCleanupCase -Id $fiveSelectedIds[$index] -OwnedPid (7101 + $index) -ObservedPid (7101 + $index)))
}
$fiveCompleteResults = New-TestPostCleanupResults -Cases @($fiveCompleteCases) -SelectedCaseIds $fiveSelectedIds
$fiveCompleteExcelLedger = New-TestPostCleanupLedger -CaseIds $fiveSelectedIds -FirstPid 7101
$fiveCompleteHelperLedger = New-TestPostCleanupLedger -CaseIds $fiveSelectedIds -Guardian
$fiveCompleteResolution = Invoke-TestPostCleanupResolution -Results $fiveCompleteResults -ExcelLedger $fiveCompleteExcelLedger -HelperLedger $fiveCompleteHelperLedger -ExpectedCaseIds $fiveSelectedIds
Assert-True ([bool]$fiveCompleteResolution.valid -and [string]$fiveCompleteResolution.disposition -eq "complete-success") "all five default descriptors must derive their compile/run outcomes from exact nested evidence"

# Permanent contradictions from fresh-eyes review: no worker-authored outcome
# label or aggregate Boolean may repair incompatible retained evidence.
$compileModalWithExceptionResults = Copy-TestJsonObject -Value $fiveCompleteResults
$compileModalWithExceptionResults.cases[1].compile_execution.exception = [pscustomobject][ordered]@{
    schema = "oxvba.excel-vba-oracle-compile-exception.v1"; message = "well-formed Execute exception"
    hresult = "0x80004005"; type = "System.Runtime.InteropServices.COMException"
}
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $compileModalWithExceptionResults -ExcelLedger $fiveCompleteExcelLedger -HelperLedger $fiveCompleteHelperLedger -ExpectedCaseIds $fiveSelectedIds).valid) "compile modal plus non-null Execute exception must derive harness-error before comparing the compile-error label"
$falseGuardianHealthResults = Copy-TestJsonObject -Value $fiveCompleteResults
$falseGuardianHealthResults.cases[0].evidence_status.guardian_healthy_before_cleanup = $false
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $falseGuardianHealthResults -ExcelLedger $fiveCompleteExcelLedger -HelperLedger $fiveCompleteHelperLedger -ExpectedCaseIds $fiveSelectedIds).valid) "a successful case must independently require guardian_healthy_before_cleanup=true"
$compileNotRunWithRunLedgerResults = Copy-TestJsonObject -Value $fiveCompleteResults
$compileNotRunWithRunLedgerResults.cases[1].run_dialogs = @(New-TestGuardianOperationEvents -CaseId "compile-failure" -Phase run -ExcelPid 7102 -DialogKind none)
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $compileNotRunWithRunLedgerResults -ExcelLedger $fiveCompleteExcelLedger -HelperLedger $fiveCompleteHelperLedger -ExpectedCaseIds $fiveSelectedIds).valid) "compile-not-run label must not suppress a contradictory healthy run-operation ledger"
$attackerRuntimeErrValueResults = Copy-TestJsonObject -Value $fiveCompleteResults
$attackerRuntimeErrValueResults.cases[4].run_value = "attacker text"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $attackerRuntimeErrValueResults -ExcelLedger $fiveCompleteExcelLedger -HelperLedger $fiveCompleteHelperLedger -ExpectedCaseIds $fiveSelectedIds).valid) "runtime-full-err must bind the returned exact Err JSON to the separately parsed runtime_err"
$ambiguousWithUnrelatedErrResults = Copy-TestJsonObject -Value $fiveCompleteResults
$ambiguousWithUnrelatedErrResults.cases[2].runtime_err = [pscustomobject][ordered]@{
    schema = "oxvba.excel-vba-oracle-runtime-err.v1"; number = 5; source = "attacker"; description = "unrelated"
    help_file = ""; help_context = 0; erl = 0
}
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $ambiguousWithUnrelatedErrResults -ExcelLedger $fiveCompleteExcelLedger -HelperLedger $fiveCompleteHelperLedger -ExpectedCaseIds $fiveSelectedIds).valid) "ambiguous macro outcome must exclude any unrelated runtime_err payload"
$ambiguousWithoutRuntimeMeasurementResults = Copy-TestJsonObject -Value $fiveCompleteResults
$ambiguousWithoutRuntimeMeasurementResults.cases[2].runtime_measurement = $null
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $ambiguousWithoutRuntimeMeasurementResults -ExcelLedger $fiveCompleteExcelLedger -HelperLedger $fiveCompleteHelperLedger -ExpectedCaseIds $fiveSelectedIds).valid) "missing ambiguous runtime measurement must fail closed without dereferencing worker-authored labels"
$successWithRuntimeErrResults = Copy-TestJsonObject -Value $fiveCompleteResults
$successWithRuntimeErrResults.cases[0].runtime_err = Copy-TestJsonObject -Value $ambiguousWithUnrelatedErrResults.cases[2].runtime_err
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $successWithRuntimeErrResults -ExcelLedger $fiveCompleteExcelLedger -HelperLedger $fiveCompleteHelperLedger -ExpectedCaseIds $fiveSelectedIds).valid) "return-value success must exclude runtime Err evidence"
$successWithRuntimeModalResults = Copy-TestJsonObject -Value $fiveCompleteResults
$successWithRuntimeModalResults.cases[0].run_dialogs = @(New-TestGuardianOperationEvents -CaseId "success" -Phase run -ExcelPid 7101 -DialogKind runtime-error)
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $successWithRuntimeModalResults -ExcelLedger $fiveCompleteExcelLedger -HelperLedger $fiveCompleteHelperLedger -ExpectedCaseIds $fiveSelectedIds).valid) "return-value success must exclude runtime modal evidence"
$wrongInvocationObservationResults = Copy-TestJsonObject -Value $fiveCompleteResults
$wrongInvocationObservationResults.cases[0].runtime_measurement.invocation_observation = "attacker-observation"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $wrongInvocationObservationResults -ExcelLedger $fiveCompleteExcelLedger -HelperLedger $fiveCompleteHelperLedger -ExpectedCaseIds $fiveSelectedIds).valid) "runtime outcome derivation must require the exact case-specific invocation observation"

$runtimeDiagnosticIds = @("runtime-unhandled-modal")
$runtimeDiagnosticCase = New-TestPostCleanupCase -Id $runtimeDiagnosticIds[0] -OwnedPid 7199 -ObservedPid 7199
$runtimeDiagnosticResults = New-TestPostCleanupResults -Cases @($runtimeDiagnosticCase) -SelectedCaseIds $runtimeDiagnosticIds -DiagnosticOnly $true
$runtimeDiagnosticResolution = Invoke-TestPostCleanupResolution -Results $runtimeDiagnosticResults -ExcelLedger (New-TestPostCleanupLedger -CaseIds $runtimeDiagnosticIds -FirstPid 7199) `
    -HelperLedger (New-TestPostCleanupLedger -CaseIds $runtimeDiagnosticIds -Guardian) -ExpectedCaseIds $runtimeDiagnosticIds -DiagnosticOnly $true
Assert-True ([bool]$runtimeDiagnosticResolution.valid -and [string]$runtimeDiagnosticResolution.disposition -eq "complete-success") "bounded unhandled-runtime diagnostic must derive its modal outcome from exact nested evidence"
$runtimeDiagnosticWithErrResults = Copy-TestJsonObject -Value $runtimeDiagnosticResults
$runtimeDiagnosticWithErrResults.cases[0].runtime_err = Copy-TestJsonObject -Value $ambiguousWithUnrelatedErrResults.cases[2].runtime_err
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $runtimeDiagnosticWithErrResults -ExcelLedger (New-TestPostCleanupLedger -CaseIds $runtimeDiagnosticIds -FirstPid 7199) `
    -HelperLedger (New-TestPostCleanupLedger -CaseIds $runtimeDiagnosticIds -Guardian) -ExpectedCaseIds $runtimeDiagnosticIds -DiagnosticOnly $true).valid) "runtime diagnostic modal shape must exclude a runtime_err payload"
$runtimeDiagnosticWithValueResults = Copy-TestJsonObject -Value $runtimeDiagnosticResults
$runtimeDiagnosticWithValueResults.cases[0].run_value = "attacker value"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $runtimeDiagnosticWithValueResults -ExcelLedger (New-TestPostCleanupLedger -CaseIds $runtimeDiagnosticIds -FirstPid 7199) `
    -HelperLedger (New-TestPostCleanupLedger -CaseIds $runtimeDiagnosticIds -Guardian) -ExpectedCaseIds $runtimeDiagnosticIds -DiagnosticOnly $true).valid) "runtime diagnostic modal shape must exclude a return value"
$preOwnershipCase = New-TestPostCleanupCase -Id $fiveSelectedIds[0] -Passed $false -OwnershipRecorded $false -OwnedPid $null -ObservedPid 9123 -CompileStatus "harness-error" -RunStatus "not-run" -TransportError "durable ownership write failed"
$preOwnershipResults = New-TestPostCleanupResults -Cases @($preOwnershipCase) -AggregatePassed $false -SelectedCaseIds $fiveSelectedIds
$emptyLedger = New-TestPostCleanupLedger -CaseIds @()
$preOwnershipResolution = Invoke-TestPostCleanupResolution -Results $preOwnershipResults -ExcelLedger $emptyLedger -HelperLedger $emptyLedger -ExpectedCaseIds $fiveSelectedIds -WorkerExitCode 1
Assert-True ([bool]$preOwnershipResolution.valid -and [string]$preOwnershipResolution.disposition -eq "pre-ownership-transport" -and [string]$preOwnershipResolution.transport_error -eq "durable ownership write failed") "five-case early stop must surface exactly the first pre-ownership transport after empty-ledger cleanup"
Assert-True (Test-ExcelOracleShouldStopAfterCase -CaseResult $preOwnershipCase) "ownership-write failure with an observed PID but no durable record must stop relaunches"
$jobDeferredPreOwnershipResults = Copy-TestJsonObject -Value $preOwnershipResults
$jobDeferredPreOwnershipResults.cases[0].cleanup_status = "job-contained-preownership"
Assert-True ([bool](Invoke-TestPostCleanupResolution -Results $jobDeferredPreOwnershipResults -ExcelLedger $emptyLedger -HelperLedger $emptyLedger -ExpectedCaseIds $fiveSelectedIds -WorkerExitCode 1).valid) "ownership-write failure may surface only after its exact process was deferred to and removed by the supervisor Job"
$durablyOwnedHarnessFailure = New-TestPostCleanupCase -Id "success" -Passed $false -OwnershipRecorded $true -OwnedPid 7001 -ObservedPid 7001 -CompileStatus "harness-error" -RunStatus "not-run" -TransportError "later failure"
Assert-True (-not (Test-ExcelOracleShouldStopAfterCase -CaseResult $durablyOwnedHarnessFailure)) "an observed PID must not substitute for or erase the durable ownership-record boundary"

$wrongFirstResults = Copy-TestJsonObject -Value $preOwnershipResults
$wrongFirstResults.cases[0].id = $fiveSelectedIds[1]
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $wrongFirstResults -ExcelLedger $emptyLedger -HelperLedger $emptyLedger -ExpectedCaseIds $fiveSelectedIds -WorkerExitCode 1).valid) "special transport from a non-first selected case must fail"
$extraEarlyResults = New-TestPostCleanupResults -Cases @($preOwnershipCase, (New-TestPostCleanupCase -Id $fiveSelectedIds[1] -Passed $false -OwnershipRecorded $false -OwnedPid $null -ObservedPid $null -CompileStatus "harness-error" -RunStatus "not-run" -TransportError "extra")) -AggregatePassed $false -SelectedCaseIds $fiveSelectedIds
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $extraEarlyResults -ExcelLedger $emptyLedger -HelperLedger $emptyLedger -ExpectedCaseIds $fiveSelectedIds -WorkerExitCode 1).valid) "special transport must contain exactly one first-case result"
$unexpectedExcelLedger = New-TestPostCleanupLedger -CaseIds @($fiveSelectedIds[0]) -FirstPid 9123
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $preOwnershipResults -ExcelLedger $unexpectedExcelLedger -HelperLedger $emptyLedger -ExpectedCaseIds $fiveSelectedIds -WorkerExitCode 1).valid) "special transport with a nonempty Excel ledger must fail"
$unexpectedHelperLedger = New-TestPostCleanupLedger -CaseIds @($fiveSelectedIds[0]) -Guardian
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $preOwnershipResults -ExcelLedger $emptyLedger -HelperLedger $unexpectedHelperLedger -ExpectedCaseIds $fiveSelectedIds -WorkerExitCode 1).valid) "special transport with a nonempty guardian ledger must fail"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $preOwnershipResults -ExcelLedger $emptyLedger -HelperLedger $emptyLedger -ExpectedCaseIds $fiveSelectedIds -WorkerExitCode 0).valid) "special transport must require the failed worker exit envelope"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $null -ExcelLedger $emptyLedger -HelperLedger $emptyLedger -ExpectedCaseIds $fiveSelectedIds -WorkerExitCode 1).valid) "worker exit failure alone must never bypass result/ledger authority"

$wrongWorkerResults = Copy-TestJsonObject -Value $completeResults
$wrongWorkerResults.worker_pid = 99999
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $wrongWorkerResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "foreign results worker PID must fail"
$wrongWorkerStartResults = Copy-TestJsonObject -Value $completeResults
$wrongWorkerStartResults.containment_authority.worker_process_start_utc = "2026-07-14T00:00:00Z"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $wrongWorkerStartResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "plausible but foreign worker start time must not replace the retained supervisor Process identity"
$wrongWorkerPathResults = Copy-TestJsonObject -Value $completeResults
$wrongWorkerPathResults.containment_authority.worker_executable_path = "C:\Program Files\PowerShell\7-preview\pwsh.exe"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $wrongWorkerPathResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "plausible but foreign worker executable path must not replace the retained supervisor Process identity"
$publishedBeforeWorkerStartResults = Copy-TestJsonObject -Value $completeResults
$publishedBeforeWorkerStartResults.containment_authority.published_utc = "2026-07-14T00:00:00Z"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $publishedBeforeWorkerStartResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "containment publication must not predate the exact worker start"
$resultsBeforeContainmentResults = Copy-TestJsonObject -Value $completeResults
$resultsBeforeContainmentResults.generated_utc = "2026-07-14T00:00:01Z"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $resultsBeforeContainmentResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "results generation must not predate containment publication"
$missingWorkerFieldResults = Copy-TestJsonObject -Value $completeResults
$missingWorkerFieldResults.PSObject.Properties.Remove("worker_pid")
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $missingWorkerFieldResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "missing results authority field must return invalid without throwing"
$wrongTokenResults = Copy-TestJsonObject -Value $completeResults
$wrongTokenResults.containment_token = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $wrongTokenResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "foreign results containment token must fail"
$stringDiagnosticResults = Copy-TestJsonObject -Value $completeResults
$stringDiagnosticResults.diagnostic_only = "false"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $stringDiagnosticResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "string diagnostic_only impostor must fail"
$wrongCaseSchemaResults = Copy-TestJsonObject -Value $completeResults
$wrongCaseSchemaResults.cases[0].schema = "attacker.v1"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $wrongCaseSchemaResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "wrong case-result schema must fail"
$missingCaseFieldResults = Copy-TestJsonObject -Value $completeResults
$missingCaseFieldResults.cases[0].PSObject.Properties.Remove("bootstrap_workbook")
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $missingCaseFieldResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "missing case-result field must return invalid without throwing"
$stringCasePassedResults = Copy-TestJsonObject -Value $completeResults
$stringCasePassedResults.cases[0].passed = "true"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $stringCasePassedResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "string case passed impostor must fail"
$wrongOrderResults = New-TestPostCleanupResults -Cases @($completeCases[1], $completeCases[0])
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $wrongOrderResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "case-result order drift must fail"
$wrongAggregateResults = New-TestPostCleanupResults -Cases $completeCases -AggregatePassed $false
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $wrongAggregateResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds -WorkerExitCode 1).valid) "aggregate/case disagreement must fail"
$wrongLedgerOrder = New-TestPostCleanupLedger -CaseIds @($completeIds[1], $completeIds[0]) -FirstPid 7001
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $completeResults -ExcelLedger $wrongLedgerOrder -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "ownership ledger order drift must fail"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $completeResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds -WorkerExitCode 1).valid) "nonzero worker exit must not bypass a complete-success envelope"
$missingBootstrapResults = Copy-TestJsonObject -Value $completeResults
$missingBootstrapResults.cases[0].bootstrap_workbook = $null
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $missingBootstrapResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "complete result with missing bootstrap authority must fail"
$modifiedBootstrapResults = Copy-TestJsonObject -Value $completeResults
$modifiedBootstrapResults.cases[0].bootstrap_workbook.sha256_after = "sha256:$('c' * 64)"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $modifiedBootstrapResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "complete result with modified bootstrap bytes must fail"
$observedPidMismatchResults = Copy-TestJsonObject -Value $completeResults
$observedPidMismatchResults.cases[0].observed_excel_pid = 7999
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $observedPidMismatchResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "observed Excel PID must equal both the durable owned PID and exact ledger PID"
$passedStatusMismatchResults = Copy-TestJsonObject -Value $completeResults
$passedStatusMismatchResults.cases[0].compile_status = "compile-error"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $passedStatusMismatchResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "worker passed=true cannot override a compile-status mismatch"
$malformedCompileCommandResults = Copy-TestJsonObject -Value $completeResults
$malformedCompileCommandResults.cases[0].compile_command.enabled_before = "true"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $malformedCompileCommandResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "malformed compile_command Boolean must fail closed"
$malformedEvidenceStatusResults = Copy-TestJsonObject -Value $completeResults
$malformedEvidenceStatusResults.cases[0].evidence_status.schema = "attacker.evidence.v1"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $malformedEvidenceStatusResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "malformed evidence_status schema must fail closed"
$contradictoryCompileEvidenceResults = Copy-TestJsonObject -Value $completeResults
$contradictoryCompileEvidenceResults.cases[0].compile_command.enabled_after = $true
$contradictoryCompileEvidenceResults.cases[0].compile_execution.exception = [pscustomobject][ordered]@{ schema = "oxvba.excel-vba-oracle-compile-exception.v1"; message = "forced exception"; hresult = "0x80004005"; type = "System.Runtime.InteropServices.COMException" }
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $contradictoryCompileEvidenceResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "passed=true compile status must not override enabled-after plus forced-exception evidence"
$attackerRuntimeMeasurementResults = Copy-TestJsonObject -Value $completeResults
$attackerRuntimeMeasurementResults.cases[0].runtime_measurement.invocation_entry = "OracleSelfTest.Attacker"
$attackerRuntimeMeasurementResults.cases[0].runtime_measurement.invocation_entry_exists = $false
$attackerRuntimeMeasurementResults.cases[0].runtime_measurement.invocation_entry_observed = $false
$attackerRuntimeMeasurementResults.cases[0].runtime_measurement.macros_runnable_entry = $false
$attackerRuntimeMeasurementResults.cases[0].runtime_measurement.macros_configured_for_automation = $false
$attackerRuntimeMeasurementResults.cases[0].runtime_measurement.invocation_observation = $null
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $attackerRuntimeMeasurementResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "passed=true run status must not override attacker entry and false observation/configuration evidence"
$wrongDescriptorHashResults = Copy-TestJsonObject -Value $completeResults
$wrongDescriptorHashResults.cases[0].selected_case_descriptor_sha256 = "sha256:$('d' * 64)"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $wrongDescriptorHashResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "wrong selected descriptor hash echo must fail"
$wrongProcedureResults = Copy-TestJsonObject -Value $completeResults
$wrongProcedureResults.cases[0].run_procedure = "OracleSelfTest.Attacker"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $wrongProcedureResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "wrong descriptor-bound run procedure must fail"
$wrongExpectedStatusResults = Copy-TestJsonObject -Value $completeResults
$wrongExpectedStatusResults.cases[0].expected_compile_status = "compile-error"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $wrongExpectedStatusResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "wrong descriptor-bound expected status must fail"
$wrongModuleIdentityResults = Copy-TestJsonObject -Value $completeResults
$wrongModuleIdentityResults.cases[0].module_sha256 = "sha256:$('e' * 64)"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $wrongModuleIdentityResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds).valid) "wrong descriptor-bound module identity must fail"
$tamperedSelectedDescriptors = @(Get-TestSelectedCaseDescriptors -CaseIds $completeIds | ForEach-Object { Copy-TestJsonObject -Value $_ })
$tamperedSelectedDescriptors[0].run_procedure = "OracleSelfTest.Attacker"
Assert-True (-not [bool](Invoke-TestPostCleanupResolution -Results $completeResults -ExcelLedger $completeExcelLedger -HelperLedger $completeHelperLedger -ExpectedCaseIds $completeIds -SelectedCaseDescriptors $tamperedSelectedDescriptors).valid) "tampered immutable descriptor payload must fail its seal before result adjudication"
$descriptorTransportDirectory = Join-Path $env:TEMP ("oxvba-oracle-descriptor-transport-{0}" -f [Guid]::NewGuid().ToString("N"))
[void][IO.Directory]::CreateDirectory($descriptorTransportDirectory)
try {
    $transportDescriptors = @(Get-TestSelectedCaseDescriptors -CaseIds $completeIds)
    $transportEnvelope = New-ExcelOracleSelectedCaseDescriptorEnvelope -Descriptors $transportDescriptors
    $transportPath = Join-Path $descriptorTransportDirectory "selected-case-descriptors.json"
    $transportEnvelope | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $transportPath -Encoding utf8NoBOM
    $mutatedTransport = Get-Content -Raw -LiteralPath $transportPath | ConvertFrom-Json -DateKind String
    $firstTransportDescriptor = $mutatedTransport.descriptors[0]
    $mutatedTransport.descriptors[0] = $mutatedTransport.descriptors[1]
    $mutatedTransport.descriptors[1] = $firstTransportDescriptor
    $mutatedTransport | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $transportPath -Encoding utf8NoBOM
    $workerStdout = Join-Path $descriptorTransportDirectory "worker.stdout.txt"
    $workerStderr = Join-Path $descriptorTransportDirectory "worker.stderr.txt"
    $excelBeforeDescriptorMutation = @(Get-Process -Name EXCEL -ErrorAction SilentlyContinue).Count
    $mutatedWorker = Start-Process -FilePath (Join-Path $PSHOME "pwsh.exe") -ArgumentList @(
        "-NoLogo", "-NoProfile", "-NonInteractive", "-File", (Join-Path $PSScriptRoot "excel-vba-oracle-worker.ps1"),
        "-RunId", "run-mutated-descriptor", "-OutputDirectory", (Join-Path $descriptorTransportDirectory "output"),
        "-OwnershipFile", (Join-Path $descriptorTransportDirectory "owned.jsonl"), "-HelperOwnershipFile", (Join-Path $descriptorTransportDirectory "helpers.jsonl"),
        "-ContainmentReadyFile", (Join-Path $descriptorTransportDirectory "never-published.json"), "-ContainmentToken", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
        "-SelectedCaseDescriptorFile", $transportPath, "-SelectedCaseDescriptorDigest", [string]$transportEnvelope.aggregate_sha256,
        "-CaseTimeoutSeconds", "5"
    ) -PassThru -WindowStyle Hidden -RedirectStandardOutput $workerStdout -RedirectStandardError $workerStderr
    Assert-True ($mutatedWorker.WaitForExit(10000)) "mutated descriptor worker must fail before waiting for containment or launching Excel"
    $mutatedWorker.Refresh()
    $mutatedWorkerError = if (Test-Path -LiteralPath $workerStderr) { Get-Content -Raw -LiteralPath $workerStderr } else { "" }
    Assert-True ($mutatedWorker.ExitCode -ne 0 -and $mutatedWorkerError -match "selected descriptor sequence changed" -and
        @(Get-Process -Name EXCEL -ErrorAction SilentlyContinue).Count -eq $excelBeforeDescriptorMutation) "descriptor-order mutation after supervisor sealing must fail its aggregate digest at worker consumption before any Excel launch"
    $mutatedWorker.Dispose()
}
finally { Remove-Item -LiteralPath $descriptorTransportDirectory -Recurse -Force -ErrorAction SilentlyContinue }

$completeWindowEnumeration = [pscustomobject]@{ Windows = @([pscustomobject]@{ ProcessId = $PID }); Truncated = $false; Limit = 512; Succeeded = $true; ErrorCode = 0 }
Assert-True (Test-ExcelOracleWindowEnumerationAuthority -Enumeration $completeWindowEnumeration -ExpectedProcessId $PID) "complete exact-PID window enumeration authority"
$truncatedWindowEnumeration = Copy-TestJsonObject -Value $completeWindowEnumeration
$truncatedWindowEnumeration.Truncated = $true
$truncatedWindowEnumeration.Succeeded = $false
Assert-True (-not (Test-ExcelOracleWindowEnumerationAuthority -Enumeration $truncatedWindowEnumeration -ExpectedProcessId $PID)) "truncated window enumeration must fail closed"
$foreignWindowEnumeration = Copy-TestJsonObject -Value $completeWindowEnumeration
$foreignWindowEnumeration.Windows[0].ProcessId = $PID + 1
Assert-True (-not (Test-ExcelOracleWindowEnumerationAuthority -Enumeration $foreignWindowEnumeration -ExpectedProcessId $PID)) "foreign-PID candidate in an enumeration must fail closed"

$excel7Candidate = [pscustomobject]@{ Hwnd = "0x101"; ProcessId = $PID; ClassName = "EXCEL7"; IsTopLevel = $false; Visible = $true }
$attachmentEnumeration = [pscustomobject]@{ Windows = @($excel7Candidate); Truncated = $false; Limit = 512; Succeeded = $true; ErrorCode = 0 }
$exactAttachment = Resolve-ExcelOracleAttachmentCandidate -Enumeration $attachmentEnumeration -ExpectedProcessId $PID -Candidate $excel7Candidate -HResult 0 -NativeObjectPresent $true -ApplicationPresent $true -ApplicationPid $PID
Assert-True ([bool]$exactAttachment.attached -and [string]$exactAttachment.disposition -eq "attached-exact-process-excel7") "pure attachment adjudicator must accept only exact EXCEL7 + Application.Hwnd PID authority"
$truncatedAttachment = Resolve-ExcelOracleAttachmentCandidate -Enumeration $truncatedWindowEnumeration -ExpectedProcessId $PID -Candidate $excel7Candidate -HResult 0 -NativeObjectPresent $true -ApplicationPresent $true -ApplicationPid $PID
Assert-True (-not [bool]$truncatedAttachment.attached -and [string]$truncatedAttachment.disposition -eq "window-enumeration-invalid") "pure attachment adjudicator must reject truncated enumeration"
$foreignCandidate = Copy-TestJsonObject -Value $excel7Candidate
$foreignCandidate.ProcessId = $PID + 1
$foreignCandidateAttachment = Resolve-ExcelOracleAttachmentCandidate -Enumeration $attachmentEnumeration -ExpectedProcessId $PID -Candidate $foreignCandidate -HResult 0 -NativeObjectPresent $true -ApplicationPresent $true -ApplicationPid $PID
Assert-True (-not [bool]$foreignCandidateAttachment.attached) "pure attachment adjudicator must reject a foreign-PID candidate"
$nonExcel7Candidate = Copy-TestJsonObject -Value $excel7Candidate
$nonExcel7Candidate.ClassName = "XLMAIN"
$nonExcel7Enumeration = [pscustomobject]@{ Windows = @($nonExcel7Candidate); Truncated = $false; Limit = 512; Succeeded = $true; ErrorCode = 0 }
$nonExcel7Attachment = Resolve-ExcelOracleAttachmentCandidate -Enumeration $nonExcel7Enumeration -ExpectedProcessId $PID -Candidate $nonExcel7Candidate -HResult 0 -NativeObjectPresent $true -ApplicationPresent $true -ApplicationPid $PID
Assert-True (-not [bool]$nonExcel7Attachment.attached -and [string]$nonExcel7Attachment.disposition -eq "non-excel7-candidate") "pure attachment adjudicator must reject a non-EXCEL7 native object"
$blockingWindow = [pscustomobject]@{ Hwnd = "0x202"; ProcessId = $PID; ClassName = "NUIDialog"; IsTopLevel = $true; Visible = $true }
$blockingEnumeration = [pscustomobject]@{ Windows = @($excel7Candidate, $blockingWindow); Truncated = $false; Limit = 512; Succeeded = $true; ErrorCode = 0 }
$blockedAttachment = Resolve-ExcelOracleAttachmentCandidate -Enumeration $blockingEnumeration -ExpectedProcessId $PID -Candidate $excel7Candidate -HResult 0 -NativeObjectPresent $true -ApplicationPresent $true -ApplicationPid $PID
Assert-True (-not [bool]$blockedAttachment.attached -and [string]$blockedAttachment.disposition -eq "blocked-owned-window") "pure attachment adjudicator must reject a visible owned startup/modal blocker"
$wrongApplicationAttachment = Resolve-ExcelOracleAttachmentCandidate -Enumeration $attachmentEnumeration -ExpectedProcessId $PID -Candidate $excel7Candidate -HResult 0 -NativeObjectPresent $true -ApplicationPresent $true -ApplicationPid ($PID + 1)
Assert-True (-not [bool]$wrongApplicationAttachment.attached -and [string]$wrongApplicationAttachment.disposition -eq "application-pid-mismatch") "pure attachment adjudicator must reject returned Application.Hwnd PID mismatch"

$startInfo = New-ExcelOracleProcessStartInfo -ExcelExecutable "C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE" -BootstrapWorkbook ([pscustomobject]@{ path = "C:\fixture with spaces\oracle-bootstrap.xlsx" })
Assert-True ([string]$startInfo.FileName -ceq "C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE" -and
    -not [bool]$startInfo.UseShellExecute -and $startInfo.ArgumentList.Count -eq 2 -and
    [string]$startInfo.ArgumentList[0] -ceq "/x" -and [string]$startInfo.ArgumentList[1] -ceq "C:\fixture with spaces\oracle-bootstrap.xlsx" -and
    $startInfo.ArgumentList -notcontains "/n") "pure ProcessStartInfo construction must preserve exact ['/x', bootstrap-path] argv with no /n"

$cases = @(Get-ExcelOracleHarnessCases)
Assert-Equal 6 $cases.Count "declared harness and bounded diagnostic case count"
Assert-Equal "success,compile-failure,ambiguous-macro-failure,intrinsic-shadow,runtime-full-err,runtime-unhandled-modal" ($cases.id -join ",") "case identities"
Assert-Equal 5 @($cases | Where-Object { -not [bool]$_.diagnostic_only }).Count "default self-test case count"
Assert-Equal 4 @($cases | Where-Object expected_compile_status -eq "ok").Count "clean-compile case count"
Assert-Equal 2 @($cases | Where-Object expected_compile_status -eq "compile-error").Count "compile-failure case count"
Assert-True ($cases[1].module_source -match "MissingOracleSymbol") "compile-failure source must contain the missing call target"
Assert-True ($cases[3].module_source -match "ByVal Fix As Double") "intrinsic-shadow source must retain the shadowing declaration"
Assert-True ($cases[3].module_source -match "Fix\(Fix\)") "intrinsic-shadow source must call through the shadowed name"
Assert-True ($cases[4].module_source -match '(?m)^100 Err\.Raise') "runtime case must carry an Erl source label"
Assert-True ($cases[2].module_source -match 'Application\.Run "OracleSelfTest\.MissingMacro"' -and $cases[2].module_source -match 'MsgBox capturedDescription') "ambiguous case must surface the real generic Application.Run failure through an owned modal"
Assert-Equal "OracleSelfTest.RunProbe" $cases[2].run_procedure "ambiguous case must invoke the existing harness entry after clean compile"
Assert-True ($cases[2].module_source -match 'oracle-ambiguous-entry-observed:' -and $cases[2].invocation_observation_prefix -eq 'oracle-ambiguous-entry-observed:') "ambiguous case must emit a case-bound observed-entry sentinel"
Assert-True ($cases[5].module_source -match 'Err\.Raise 13' -and [bool]$cases[5].diagnostic_only) "unhandled runtime modal must have a real live diagnostic fixture"
Assert-Equal 6 @($cases.id | Select-Object -Unique).Count "case identities must be unique"

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
Assert-Equal "unrecognized-modal" (Get-ExcelOracleDialogPolicy -Phase run -Texts @("Compile error: Sub or Function not defined") -Buttons @("OK")).kind "compile dialog must not be recognized in run phase"
Assert-Equal "unrecognized-modal" (Get-ExcelOracleDialogPolicy -Phase compile -Texts @("Run-time error '13': Type mismatch") -Buttons @("End")).kind "runtime dialog must not be recognized in compile phase"

$controlJson = [ordered]@{
    schema = "oxvba.excel-vba-oracle-guardian-control.v2"; run_id = "run-a"; case_id = "success"; operation_id = "success-compile"
    sequence = 1; phase = "compile"; allow_dismiss = $true; written_utc = "2026-07-14T00:00:00Z"
} | ConvertTo-Json -Compress
Assert-Equal 0 @((ConvertFrom-ExcelOracleGuardianControl -Json $controlJson -RunId "run-a").errors).Count "valid strict guardian control"
Assert-True (@((ConvertFrom-ExcelOracleGuardianControl -Json $controlJson.Replace('"allow_dismiss":true', '"allow_dismiss":"false"') -RunId "run-a").errors).Count -gt 0) "string Boolean guardian control must fail closed"
Assert-True (@((ConvertFrom-ExcelOracleGuardianControl -Json $controlJson.Replace('"allow_dismiss":true', '"allow_dismiss":1') -RunId "run-a").errors).Count -gt 0) "numeric Boolean guardian control must fail closed"
Assert-True (@((ConvertFrom-ExcelOracleGuardianControl -Json $controlJson.Replace('"phase":"compile"', '"phase":"run"').Replace('"run_id":"run-a"', '"run_id":"other"') -RunId "run-a").errors).Count -gt 0) "foreign run control must fail closed"

$claimRoot = Join-Path ([IO.Path]::GetTempPath()) "oxvba-oracle-claims-$([Guid]::NewGuid().ToString('N'))"
$claimOne = $null
$claimOther = $null
try {
    $claimOne = Enter-ExcelOracleRunClaim -OutputBase $claimRoot -RunId "same-run"
    $sameRejected = $false
    try { [void](Enter-ExcelOracleRunClaim -OutputBase $claimRoot -RunId "same-run") } catch { $sameRejected = $_.Exception.Message -match "atomic run claim" }
    Assert-True $sameRejected "concurrent same-RunId claim must be rejected while first claim is held"
    $claimOther = Enter-ExcelOracleRunClaim -OutputBase $claimRoot -RunId "isolated-run"
    Assert-True ([string]$claimOne.output_directory -cne [string]$claimOther.output_directory) "different RunIds must receive isolated directories"
}
finally {
    if ($claimOne) { Exit-ExcelOracleRunClaim -Claim $claimOne }
    if ($claimOther) { Exit-ExcelOracleRunClaim -Claim $claimOther }
    Remove-Item -LiteralPath $claimRoot -Recurse -Force -ErrorAction SilentlyContinue
}

$claimCleanupRoot = Join-Path ([IO.Path]::GetTempPath()) "oxvba-oracle-claim-cleanup-$([Guid]::NewGuid().ToString('N'))"
$deletionFailureClaim = $null
try {
    $deletionFailureClaim = Enter-ExcelOracleRunClaim -OutputBase $claimCleanupRoot -RunId "delete-failure"
    $combinedFailureObserved = $false
    try {
        Exit-ExcelOracleRunClaim -Claim $deletionFailureClaim -PrimaryFailure ([InvalidOperationException]::new("injected primary failure")) `
            -RemoveClaim { param($Path) throw "injected deletion failure for $Path" }
    }
    catch {
        $combinedFailureObserved = $_.Exception.Message -match "injected primary failure" -and $_.Exception.Message -match "injected deletion failure"
    }
    Assert-True $combinedFailureObserved "claim deletion failure must surface without erasing the primary failure context"
    Assert-True (Test-Path -LiteralPath $deletionFailureClaim.claim_path) "injected deletion failure must leave the exact marker visible for fail-closed diagnosis"
    Remove-Item -LiteralPath $deletionFailureClaim.claim_path -Force -ErrorAction Stop

    $staleMarkerClaim = Enter-ExcelOracleRunClaim -OutputBase $claimCleanupRoot -RunId "stale-marker"
    $staleMarkerRejected = $false
    try { Exit-ExcelOracleRunClaim -Claim $staleMarkerClaim -RemoveClaim { param($Path) } }
    catch { $staleMarkerRejected = $_.Exception.Message -match "claim marker remains after deletion attempt" }
    Assert-True ($staleMarkerRejected -and (Test-Path -LiteralPath $staleMarkerClaim.claim_path)) "a no-op deletion must be detected by exact post-cleanup marker absence verification"
    Remove-Item -LiteralPath $staleMarkerClaim.claim_path -Force -ErrorAction Stop
}
finally {
    if ($deletionFailureClaim -and $deletionFailureClaim.stream) { $deletionFailureClaim.stream.Dispose() }
    Remove-Item -LiteralPath $claimCleanupRoot -Recurse -Force -ErrorAction SilentlyContinue
}

$failureRoot = Join-Path ([IO.Path]::GetTempPath()) "oxvba-oracle-claim-failure-$([Guid]::NewGuid().ToString('N'))"
$failedClaim = $null
$failedClaimPath = $null
$failedOutputDirectory = $null
try {
    try {
        $failedClaim = Enter-ExcelOracleRunClaim -OutputBase $failureRoot -RunId "failed-run"
        $failedClaimPath = [string]$failedClaim.claim_path
        $failedOutputDirectory = [string]$failedClaim.output_directory
        Set-Content -LiteralPath (Join-Path $failedOutputDirectory "failure-evidence.txt") -Value "preserve" -Encoding utf8NoBOM
        throw "forced post-claim failure"
    }
    catch {
        Assert-True ($_.Exception.Message -match "forced post-claim failure") "forced post-claim failure must reach cleanup"
    }
    finally {
        if ($failedClaim) { Exit-ExcelOracleRunClaim -Claim $failedClaim }
    }
    Assert-True (-not (Test-Path -LiteralPath $failedClaimPath)) "failed run must release and remove only its exact held claim"
    Assert-True (Test-Path -LiteralPath (Join-Path $failedOutputDirectory "failure-evidence.txt")) "failed run evidence directory must remain intact"
    $staleRejected = $false
    try { [void](Enter-ExcelOracleRunClaim -OutputBase $failureRoot -RunId "failed-run") } catch { $staleRejected = $_.Exception.Message -match "run directory already exists" }
    Assert-True $staleRejected "released claim must not allow reuse of a stale failed run directory"
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $failureRoot ".failed-run.run-claim"))) "stale-directory rejection must not strand a replacement claim"
    $isolatedClaim = Enter-ExcelOracleRunClaim -OutputBase $failureRoot -RunId "other-run"
    try { Assert-True (Test-Path -LiteralPath $isolatedClaim.output_directory) "different RunId must remain available after another run fails" }
    finally { Exit-ExcelOracleRunClaim -Claim $isolatedClaim }
}
finally { Remove-Item -LiteralPath $failureRoot -Recurse -Force -ErrorAction SilentlyContinue }

Assert-Equal "compile-failure" (Get-ExcelOracleMacroFailureDisposition -Message "Cannot run the macro" -CompileStatus "compile-error" -AccessVbom $true -RunnableEntryObserved $true -TargetExists $true) "generic macro error after compile failure"
Assert-Equal "missing-macro" (Get-ExcelOracleMacroFailureDisposition -Message "Cannot run the macro. The macro may not be available." -CompileStatus "ok" -AccessVbom $true -RunnableEntryObserved $true -TargetExists $false) "missing macro adjudication after an observed runnable entry"
Assert-Equal "suspected-compile-failure" (Get-ExcelOracleMacroFailureDisposition -Message "Cannot run the macro. All macros may be disabled." -CompileStatus "ok" -AccessVbom $true -RunnableEntryObserved $true -TargetExists $true) "generic macro failure with present target after an observed runnable entry"
Assert-Equal "macros-runnable-entry-unobserved" (Get-ExcelOracleMacroFailureDisposition -Message "Cannot run the macro" -CompileStatus "ok" -AccessVbom $true -RunnableEntryObserved $false -TargetExists $true) "low security plus an existing procedure without observed entry must remain unresolved"

$expectedErr = Get-ExcelOracleExpectedRuntimeErr
$errJson = '{"number":513,"source":"OracleRuntimeSource","description":"oracle-runtime-error","help_file":"oracle-help.chm","help_context":42,"erl":100}'
$parsedErr = ConvertFrom-ExcelOracleRuntimeErr -Json $errJson
foreach ($field in @("number", "source", "description", "help_file", "help_context", "erl")) {
    Assert-Equal $expectedErr.$field $parsedErr.$field "complete runtime Err field $field"
}
$missingErrRejected = $false
try { [void](ConvertFrom-ExcelOracleRuntimeErr -Json '{"number":513}') }
catch { $missingErrRejected = $_.Exception.Message -match "invalid exact field/type shape" }
Assert-True $missingErrRejected "incomplete runtime Err payload must fail closed"

$ownedRecord = [pscustomobject]@{
    schema = "oxvba.excel-vba-oracle-owned-process.v1"
    run_id = "run-a"
    case_id = "success"
    ownership = "owned-new-instance"
    pid = 303
    process_name = "EXCEL"
    process_start_utc = "2026-07-14T00:00:00.0000000Z"
    executable_path = "C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE"
    acquired_utc = "2026-07-14T00:00:01.0000000Z"
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
    case_id = "success"
    ownership = "owned-helper-process"
    role = "guardian"
    pid = $selfProcess.Id
    process_name = [string]$selfProcess.ProcessName
    process_start_utc = $selfProcess.StartTime.ToUniversalTime().ToString("o")
    executable_path = [string]$selfProcess.Path
    acquired_utc = [DateTime]::UtcNow.ToString("o")
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
$emptyExcelLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines ([string[]]::new(0)) -Kind excel -RunId "run-a" -BaselineExcelPids ([int[]]::new(0))
Assert-Equal 0 @($emptyExcelLedger.records).Count "explicit empty Excel ownership ledger record count"
Assert-Equal 0 @($emptyExcelLedger.errors).Count "explicit empty Excel ownership ledger error count"
$malformedExcelLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines @($validExcelLedgerLine, '{not-json') -Kind excel -RunId "run-a" -BaselineExcelPids @(101, 202)
Assert-Equal 1 @($malformedExcelLedger.errors).Count "mutation: malformed nonempty ownership JSON must make authority uncertain"
$nullExcelLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines @('null') -Kind excel -RunId "run-a" -BaselineExcelPids @(101, 202)
Assert-Equal 1 @($nullExcelLedger.errors).Count "mutation: null ownership JSON must make authority uncertain"
$wrongSchemaExcelLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines @($validExcelLedgerLine.Replace('owned-process.v1', 'attacker.v1')) -Kind excel -RunId "run-a" -BaselineExcelPids @(101, 202)
Assert-Equal 1 @($wrongSchemaExcelLedger.errors).Count "mutation: wrong ownership schema must make authority uncertain"
$wrongExcelLeafLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines @($validExcelLedgerLine.Replace('EXCEL.EXE', 'NOTEPAD.EXE')) -Kind excel -RunId "run-a" -BaselineExcelPids @(101, 202)
Assert-Equal 1 @($wrongExcelLeafLedger.errors).Count "mutation: Excel ownership executable leaf must be EXCEL.EXE"
$duplicateExcelLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines @($validExcelLedgerLine, $validExcelLedgerLine) -Kind excel -RunId "run-a" -BaselineExcelPids @(101, 202)
Assert-True (@($duplicateExcelLedger.errors).Count -gt 0) "mutation: duplicate ownership identity/case must make authority uncertain"
Assert-True (Test-ExcelOracleLedgerCaseBinding -Records @($validExcelLedger.records) -ExpectedCaseIds @("success")) "ownership ledger must bind exactly to selected cases"
Assert-True (-not (Test-ExcelOracleLedgerCaseBinding -Records @($validExcelLedger.records) -ExpectedCaseIds @("success", "compile-failure"))) "missing selected case must fail ledger binding"
$wrongCaseLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines @($validExcelLedgerLine) -Kind excel -RunId "run-a" -BaselineExcelPids @(101, 202) -ExpectedCaseIds @("compile-failure")
Assert-True (@($wrongCaseLedger.errors).Count -gt 0) "unselected ownership case must fail closed"
$stringPidLedger = ConvertFrom-ExcelOracleOwnershipLedger -Lines @($validExcelLedgerLine.Replace('"pid":303', '"pid":"303"')) -Kind excel -RunId "run-a" -BaselineExcelPids @(101, 202)
Assert-True (@($stringPidLedger.errors).Count -gt 0) "string ownership PID must fail closed"

$observation = [ordered]@{
    schema = "oxvba.excel-vba-oracle-window-observation.v1"; event_type = "dialog-observation"; observation_id = "obs-1"; run_id = "run-a"
    case_id = "success"; operation_id = "compile"; control_sequence = 1; event_sequence = 1; phase = "compile"; excel_pid = 303; observed_process_id = 303; observed_utc = "2026-07-14T00:00:01Z"; capture_completed_utc = "2026-07-14T00:00:01.100Z"
    window_handle = "123"; classification = "compile-error"; disposition = "capture-then-dismiss"; considered_dialog = $true; is_modal = $true
    dialog_text = @("Compile error", "Sub or Function not defined"); selected_token = "MissingOracleSymbol"; expanded_line = "RunProbe = MissingOracleSymbol(1)"
}
$dismissal = [ordered]@{
    schema = "oxvba.excel-vba-oracle-dismissal-result.v1"; event_type = "dismissal-result"; observation_id = "obs-1"; run_id = "run-a"
    case_id = "success"; operation_id = "compile"; control_sequence = 1; event_sequence = 2; phase = "compile"; excel_pid = 303; window_handle = "123"; attempted_utc = "2026-07-14T00:00:02Z"; requested_buttons = @("OK"); succeeded = $true; dismissed_button = "OK"
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
Assert-True (@($duplicateObservationLedger.errors).Count -gt 0) "mutation: duplicate guardian observation must fail capture authority"
$stringBooleanObservationLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine.Replace('"considered_dialog":true', '"considered_dialog":"false"')) -RunId "run-a"
Assert-Equal 1 @($stringBooleanObservationLedger.errors).Count "mutation: string considered_dialog impostor must fail capture authority"
$numericBooleanObservationLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine.Replace('"considered_dialog":true', '"considered_dialog":1')) -RunId "run-a"
Assert-Equal 1 @($numericBooleanObservationLedger.errors).Count "mutation: numeric considered_dialog impostor must fail capture authority"
$stringBooleanDismissalLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine, $dismissalLine.Replace('"succeeded":true', '"succeeded":"false"')) -RunId "run-a"
Assert-Equal 1 @($stringBooleanDismissalLedger.errors).Count "mutation: string succeeded impostor must fail capture authority"
$numericBooleanDismissalLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine, $dismissalLine.Replace('"succeeded":true', '"succeeded":1')) -RunId "run-a"
Assert-Equal 1 @($numericBooleanDismissalLedger.errors).Count "mutation: numeric succeeded impostor must fail capture authority"
$wrongDismissedButtonLedger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine, $dismissalLine.Replace('"dismissed_button":"OK"', '"dismissed_button":"Cancel"')) -RunId "run-a"
Assert-True (@($wrongDismissedButtonLedger.errors).Count -gt 0) "mutation: dismissed button not in requested set must fail capture authority"
$crossPhaseObservation = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine.Replace('"phase":"compile"', '"phase":"run"')) -RunId "run-a"
Assert-True (@($crossPhaseObservation.errors).Count -gt 0) "mutation: compile classification in run phase must fail capture authority"
$missingTokenObservation = ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($observationLine.Replace('"selected_token":"MissingOracleSymbol"', '"selected_token":""')) -RunId "run-a"
Assert-True (@($missingTokenObservation.errors).Count -gt 0) "mutation: incomplete pre-dismiss compile selection must fail authority"

$arm = [pscustomobject]@{ event_type = "operation-armed"; operation_id = "op"; control_sequence = 1; event_sequence = 1; observed_utc = "2026-07-14T00:00:01Z" }
$earlyHeartbeat = [pscustomobject]@{ event_type = "guardian-heartbeat"; operation_id = "op"; control_sequence = 1; event_sequence = 2; observed_utc = "2026-07-14T00:00:02Z" }
Assert-True (-not (Test-ExcelOracleGuardianOperationCoverage -Events @($arm, $earlyHeartbeat) -OperationId "op" -ControlSequence 1 -InvocationCompletedUtc ([DateTime]"2026-07-14T00:00:03Z"))) "ready plus benign heartbeat followed by hang must not cover invocation"
$lateHeartbeat = [pscustomobject]@{ event_type = "guardian-heartbeat"; operation_id = "op"; control_sequence = 1; event_sequence = 3; observed_utc = "2026-07-14T00:00:04Z" }
Assert-True (Test-ExcelOracleGuardianOperationCoverage -Events @($arm, $earlyHeartbeat, $lateHeartbeat) -OperationId "op" -ControlSequence 1 -InvocationCompletedUtc ([DateTime]"2026-07-14T00:00:03Z")) "post-invocation heartbeat must close operation coverage"
$wrongLifecycleSchema = [ordered]@{ schema = "attacker.v1"; event_type = "guardian-heartbeat"; run_id = "run-a"; case_id = "success"; operation_id = "op"; phase = "compile"; control_sequence = 1; event_sequence = 1; observed_utc = "2026-07-14T00:00:04Z" } | ConvertTo-Json -Compress
Assert-True (@((ConvertFrom-ExcelOracleGuardianEventLedger -Lines @($wrongLifecycleSchema) -RunId "run-a").errors).Count -gt 0) "lifecycle event with wrong schema must fail closed"

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
$runtimeModalPlan = ((& $runnerPath -Suite HarnessSelfTest -EnvironmentId win-x64-dev-oracle-2026-07 -NoMatrixUpdate -PlanOnly -DiagnosticCaseId runtime-unhandled-modal -RunId offline-runtime-modal-test) -join "`n") | ConvertFrom-Json
Assert-Equal 1 @($runtimeModalPlan.cases).Count "unhandled runtime modal diagnostic plan count"
Assert-Equal "runtime-unhandled-modal" $runtimeModalPlan.cases[0].id "unhandled runtime modal diagnostic plan identity"

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
Assert-True ($guardianSource -match 'ConvertFrom-ExcelOracleGuardianControl' -and $guardianSource -match 'invalid-control' -and $guardianSource -match 'never arms an operation') "invalid controls must be durably reported and never authorize dismissal"
Assert-True ($guardianSource -match 'operation-armed' -and $guardianSource -match 'guardian-heartbeat') "guardian must acknowledge and heartbeat each operation"

$workerSource = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot "excel-vba-oracle-worker.ps1")
$contractSource = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot "excel-vba-oracle-contract.ps1")
$bootstrapSource = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot "excel-vba-oracle-bootstrap.ps1")
$attachmentSource = "$workerSource`n$contractSource`n$bootstrapSource"
$nativeSourceMatch = [regex]::Match($workerSource, "(?s)Add-Type @'\r?\n(?<code>.*?)\r?\n'@")
Assert-True $nativeSourceMatch.Success "worker native attachment source must be extractable"
if (-not ([Management.Automation.PSTypeName]'ExcelOracleNativeMethods').Type) {
    Add-Type -TypeDefinition $nativeSourceMatch.Groups['code'].Value
}
$ownedTestEnumeration = [ExcelOracleNativeMethods]::EnumerateOwnedWindows([uint32]$PID)
Assert-True (Test-ExcelOracleWindowEnumerationAuthority -Enumeration $ownedTestEnumeration -ExpectedProcessId $PID) "native enumeration must complete without truncation and return only exact-PID windows"
Assert-True (Test-WorkerExactPidAttachmentShape -Source $attachmentSource) "worker attachment must enumerate bounded exact-PID top-level/descendant windows, verify returned Application.Hwnd PID, and write bounded diagnostics"
$pidFilterMutation = $attachmentSource.Replace('topLevelProcessId != expectedProcessId', 'topLevelProcessId == expectedProcessId')
Assert-True (-not (Test-WorkerExactPidAttachmentShape -Source $pidFilterMutation)) "mutation: inverted top-level PID ownership filter must be rejected"
$unboundedWindowMutation = $attachmentSource.Replace('Select-Object -First 128', 'Select-Object')
Assert-True (-not (Test-WorkerExactPidAttachmentShape -Source $unboundedWindowMutation)) "mutation: unbounded attachment candidate enumeration must be rejected"
$unverifiedApplicationMutation = $attachmentSource.Replace('[int]$ApplicationPid -ne $ExpectedProcessId', '$false')
Assert-True (-not (Test-WorkerExactPidAttachmentShape -Source $unverifiedApplicationMutation)) "mutation: accepting an OBJID_NATIVEOM result without exact Application.Hwnd PID verification must be rejected"
$ambiguousLaunchMutation = $attachmentSource.Replace('$startInfo.ArgumentList.Add([string]$BootstrapWorkbook.path)', '$startInfo.Arguments = "/x $($BootstrapWorkbook.path)"')
Assert-True (-not (Test-WorkerExactPidAttachmentShape -Source $ambiguousLaunchMutation)) "mutation: string-concatenated bootstrap launch arguments must be rejected"
$threeArgumentMutation = $attachmentSource.Replace('$startInfo.ArgumentList.Add([string]$BootstrapWorkbook.path)', '$startInfo.ArgumentList.Add([string]$BootstrapWorkbook.path); $startInfo.ArgumentList.Add("/n")').Replace('$startInfo.ArgumentList.Count -ne 2', '$startInfo.ArgumentList.Count -ne 3')
Assert-True (-not (Test-WorkerExactPidAttachmentShape -Source $threeArgumentMutation)) "mutation: undocumented /n or any third Excel argv must be rejected"
$compileIndex = $workerSource.IndexOf('$compileControl.Execute()')
$runIndex = $workerSource.IndexOf('$runValue = $excel.Run($qualifiedName)')
Assert-True ($compileIndex -ge 0 -and $runIndex -gt $compileIndex) "forced VBE compile must precede Application.Run"
$compileExceptionDecision = $workerSource.IndexOf('if ($executeException) { $compileStatus = "harness-error" }')
$compileModalDecision = $workerSource.IndexOf('elseif ($compileKinds -contains "compile-error") { $compileStatus = "compile-error" }')
Assert-True ($compileExceptionDecision -gt $compileIndex -and $compileModalDecision -gt $compileExceptionDecision) "worker compile classification must give any Execute exception precedence over modal classification"
Assert-True ($workerSource -match '\$runOperationHealthy = Test-GuardianOperationHealthy -Events \$runEvents' -and
    $workerSource -notmatch '\$runOperationHealthy = if \(\$runStatus') "worker operation health must derive from the run ledger without consulting run_status"
Assert-True ($workerSource -match "Wait-GuardianReady" -and $workerSource.IndexOf('Wait-GuardianReady') -lt $compileIndex) "guardian readiness must precede forced compile"
Assert-True ($workerSource -match "module_sha256") "case evidence must seal module source"
Assert-True ($workerSource -match "Get-VbeSelectionFromCom" -and $workerSource -match "diagnostic only" -and $workerSource -notmatch "vbe-com-post-dialog-fallback") "post-dismiss COM selection must remain diagnostic-only and never repair authority"
Assert-True ($workerSource -notmatch '\$event\.(selected_token|expanded_line)\s*=' -and $workerSource -match '-ExpectedToken \(\[string\]\$Descriptor\.expected_selected_token\).+-ExpectedLine') "only exact immutable pre-dismiss token/line evidence may satisfy compile acceptance"
Assert-True ($workerSource -match "CodePane.Show\(\)" -and $workerSource -match "compile command ID 578 is disabled") "worker must activate the code pane and reject a disabled compile command"
Assert-True ($workerSource -match 'no-dialog-unverified') "absence of a captured dialog must remain fail-closed"
Assert-True ($workerSource -match 'owned-helper-process' -and $workerSource -match 'process_start_utc' -and $workerSource -match 'executable_path') "worker must record complete Excel and guardian identities"
Assert-True ($workerSource -notmatch 'Stop-Process') "worker cleanup must retain exact Process objects instead of PID-only termination"
Assert-True ($workerSource -match 'Invoke-ExcelOracleRetainedProcessTermination.+guardianOwnershipRecord' -and $workerSource -match 'Invoke-ExcelOracleRetainedProcessTermination.+excelOwnershipRecord' -and $workerSource -match 'cleanup-authority-error') "worker guardian/Excel fallback cleanup must use exact written records and fail closed on identity conflict"
Assert-True (Test-WorkerEvidenceGatedAcceptanceShape -WorkerSource $workerSource -ContractSource $contractSource) "case acceptance must be gated by healthy guardian and authoritative modal evidence"
$statusOnlyAcceptanceMutation = $workerSource.Replace('$passed = $behaviorPassed -and $guardianHealthy -and $authoritativeEvidencePassed', '$passed = $behaviorPassed')
Assert-True (-not (Test-WorkerEvidenceGatedAcceptanceShape -WorkerSource $statusOnlyAcceptanceMutation -ContractSource $contractSource)) "mutation: status-only case acceptance must be rejected"
Assert-True ($workerSource -match 'invalid guardian event ledger' -and $workerSource -notmatch 'catch \{ \}\s*\r?\n\s*return @\(\$events\)') "guardian event parsing must fail closed"
Assert-True ($workerSource -match 'Assert-GuardianLive.+forced VBE compile' -and $workerSource -match 'Assert-GuardianLive.+runtime invocation') "guardian exact liveness must be checked immediately before compile and runtime"
Assert-True ($workerSource.IndexOf('immediately-before-execute') -lt $compileIndex -and $workerSource.IndexOf('immediately-after-execute') -gt $compileIndex) "compile Execute must be enclosed by exact active project/module/source/code-pane authority snapshots"
Assert-True (Test-CompileSnapshotBorrowedAliasShape -Source $workerSource) "compile authority snapshot must not FinalRelease borrowed project/module/code-pane RCW aliases"
Assert-True ($workerSource -match 'injectedSourceSha256 -cne \$selectedSourceSha256' -and $workerSource -match 'ExpectedSourceSha256 \$selectedSourceSha256') "compile source authority must be anchored to the selected case text, not an earlier read of mutable module text"
Assert-True ($workerSource -match 'Wait-GuardianOperationArmed' -and $workerSource -match 'GuardianOperationCoverage') "each operation must require an arm acknowledgement and a post-invocation heartbeat"
Assert-True ($workerSource -match 'Get-VbomRuntimeMeasurement' -and $workerSource -match 'macro_probe_target_exists' -and $workerSource -match 'invocation_entry_observed' -and $workerSource -match 'case-specific-return-sentinel') "macro adjudication must use measured VBOM target plus an observed runnable-entry sentinel"
Assert-True ($workerSource -notmatch '-MacrosEnabled' -and $workerSource -match '-RunnableEntryObserved \(\[bool\]\$runtimeMeasurement\.invocation_entry_observed\)') "configured low AutomationSecurity must not substitute for observed macro entry"
Assert-True ($workerSource -match 'runtime-unhandled-modal' -and $workerSource -match 'Test-RuntimeErrorEvidence') "live worker must implement the unhandled runtime modal diagnostic"
Assert-True ($workerSource -notmatch 'New-Object\s+-ComObject\s+Excel\.Application' -and $workerSource -match 'Start-OwnedExcelApplication') "Excel must be directly launched inside prepared job containment, not activated through an uncontained COM launch"
Assert-True ($workerSource -match 'Test-ExcelOracleShouldStopAfterCase -CaseResult \$caseResult' -and $workerSource -match 'Do not multiply' -and $workerSource -match 'excel_ownership_recorded = \$null -ne \$excelOwnershipRecord') "a pre-ownership attachment/bootstrap failure must stop additional owned Excel launches by durable ownership-record state"
Assert-True ($workerSource -match 'job-contained-preownership' -and $workerSource -match 'defer it to Job termination') "ownership-write failure must defer the one exact contained Excel process to the supervisor Job without authorizing another launch"
$descriptorReadIndex = $workerSource.IndexOf('$descriptorEnvelope = Read-ExcelOracleSelectedCaseDescriptorEnvelope')
$containmentWaitIndex = $workerSource.IndexOf('$containmentAuthority = Wait-ContainmentAuthority')
Assert-True ($descriptorReadIndex -ge 0 -and $descriptorReadIndex -lt $containmentWaitIndex -and
    $containmentWaitIndex -lt $workerSource.IndexOf('$script:ExcelExecutablePath = Get-ExcelExecutablePath') -and
    $workerSource -notmatch 'Get-ExcelOracleHarnessCases|\$Case\.' -and $workerSource -match 'Invoke-HarnessCase -Descriptor \$descriptor') "worker must validate only the supervisor descriptor sequence before containment, then wait for containment before any Excel resolution or case mutation"
$compileControlPublish = $workerSource.IndexOf('Set-GuardianControl -Path $controlFile -CaseId $Descriptor.id -OperationId $compileOperation')
$compileLiveCheck = $workerSource.IndexOf('Assert-GuardianLive -Process $guardian -ReadyRecord $guardianReady -Phase "forced VBE compile after control publication"')
$runControlPublish = $workerSource.IndexOf('Set-GuardianControl -Path $controlFile -CaseId $Descriptor.id -OperationId $runOperation')
$runLiveCheck = $workerSource.IndexOf('Assert-GuardianLive -Process $guardian -ReadyRecord $guardianReady -Phase "runtime invocation after control publication"')
Assert-True ($compileControlPublish -ge 0 -and $compileLiveCheck -gt $compileControlPublish -and $runControlPublish -ge 0 -and $runLiveCheck -gt $runControlPublish) "control publication must precede the immediate guardian liveness check for both phases"

$runnerSource = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot "run-excel-vba-oracle.ps1")
Assert-True (Test-RunnerEmptyLedgerShape -Source $runnerSource) "missing ownership ledger paths must bind to explicit empty arrays during pre-ledger cleanup"
$nullLedgerMutation = $runnerSource.Replace('[string[]]$lines = [string[]]::new(0)', '$lines = if (Test-Path -LiteralPath $Path) { @(Get-Content -LiteralPath $Path) } else { @() }')
Assert-True (-not (Test-RunnerEmptyLedgerShape -Source $nullLedgerMutation)) "mutation: pipeline-collapsed missing ledger arrays must be rejected"
Assert-True ($runnerSource -match 'Resolve-ExcelOraclePostCleanupResult' -and $runnerSource -match 'first selected case failed before durable ownership after owned Job cleanup' -and $runnerSource -notmatch 'if \(\$workerFailure\) \{ throw \$workerFailure \}') "runner must use the pure post-cleanup authority result and never let process exit alone bypass ledger binding"
Assert-True ($runnerSource -match 'New-ExcelOracleSelectedCaseDescriptorEnvelope' -and
    $runnerSource.IndexOf('$selectedCaseDescriptorEnvelope | ConvertTo-Json') -lt $runnerSource.IndexOf('$containedWorker = Start-ExcelOracleContainedProcess') -and
    $runnerSource -match '"-SelectedCaseDescriptorFile", \$selectedCaseDescriptorFile' -and
    $runnerSource -match '"-SelectedCaseDescriptorDigest", \[string\]\$selectedCaseDescriptorEnvelope\.aggregate_sha256' -and
    $runnerSource -match '-SelectedCaseDescriptors \$selectedCaseDescriptors') "supervisor must serialize and pass its exact sealed descriptor sequence/digest, then reuse the same descriptors for post-cleanup authority"
Assert-True ($runnerSource -match 'Test-ExcelOracleBootstrapWorkbook -Descriptor \$caseResult\.bootstrap_workbook' -and $runnerSource -match 'selected oracle case expectations failed after owned cleanup') "runner must validate persisted bootstrap bytes before surfacing complete success or case failure"
Assert-True (Test-RunnerIdentityCheckedCleanupShape -Source $runnerSource) "supervisor fallback cleanup must query, terminate, and wait through one retained native handle"
$pidOnlyCleanupMutation = $runnerSource.Replace('Invoke-ExcelOracleRetainedProcessTermination', 'Invoke-PidOnlyTermination')
Assert-True (-not (Test-RunnerIdentityCheckedCleanupShape -Source $pidOnlyCleanupMutation)) "mutation: PID-only fallback cleanup must be rejected"
Assert-True ($runnerSource -match 'Enter-ExcelOracleRunClaim' -and $runnerSource -notmatch 'New-Item -ItemType Directory -Force -Path \$outputDirectory') "runner must hold an atomic CreateNew run claim without Force directory creation"
$claimIndex = $runnerSource.IndexOf('$runClaim = Enter-ExcelOracleRunClaim')
$postClaimTryIndex = $runnerSource.IndexOf('try {', $claimIndex)
Assert-True ($claimIndex -ge 0 -and $postClaimTryIndex -gt $claimIndex -and $postClaimTryIndex -lt $runnerSource.IndexOf('$plan | ConvertTo-Json', $claimIndex) -and $runnerSource.LastIndexOf('Exit-ExcelOracleRunClaim -Claim $runClaim') -gt $runnerSource.IndexOf('Set-Content -LiteralPath (Join-Path $outputDirectory "summary.md")')) "every post-claim runner path must release the exact held claim through top-level finally"
Assert-True ($runnerSource -match 'Exit-ExcelOracleRunClaim -Claim \$runClaim -PrimaryFailure \$primaryRunFailure') "top-level claim cleanup must preserve primary failure context while surfacing cleanup failure"
Assert-True ($runnerSource -match '\$RunId = \("excel_vba_oracle_\{0\}" -f \[Guid\]::NewGuid') "default RunId must include a GUID rather than timestamp-only uniqueness"
Assert-True ($runnerSource -match '\$worker\.WaitForExit\(10000\)' -and $runnerSource.IndexOf('$worker.WaitForExit(10000)') -lt $runnerSource.LastIndexOf('Stop-RecordedOwnedResources')) "timeout cleanup must wait for the exact worker before reading ledgers"
Assert-True ($runnerSource.IndexOf('$containedWorker = Start-ExcelOracleContainedProcess') -ge 0 -and $runnerSource.IndexOf('$containedWorker = Start-ExcelOracleContainedProcess') -lt $runnerSource.IndexOf('oxvba.excel-vba-oracle-containment-ready.v1') -and
    $jobSource -match '& \$AssignProcess \$job \$process') "supervisor must assign the waiting worker to the job before publishing mutation authority"
Assert-True ($jobSource -match '& \$TestMembership \$job \$process' -and $runnerSource.IndexOf('$containedWorker = Start-ExcelOracleContainedProcess') -lt $runnerSource.IndexOf('oxvba.excel-vba-oracle-containment-ready.v1')) "supervisor must prove Job membership before publishing mutation authority"
Assert-True ($runnerSource -match 'same-instance-conflict' -and $runnerSource -match 'identity conflict') "supervisor must fail cleanup and residual audits on same-instance identity conflicts"
Assert-True ($runnerSource -match 'worker timed out.+\$terminationFailure') "timeout evidence must preserve worker termination failure detail"

Write-Output "test-excel-vba-oracle: PASS"
