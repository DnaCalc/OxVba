param(
    [string]$BaselineRef = "pre-value-model-migration-2026-04-20",
    [string]$CandidateRef = "HEAD",
    [string]$OutputRoot = "docs/evidence/value_model_migration",
    [string]$RunId = "",
    [string[]]$IncludeLane = @(),
    [string[]]$ExcludeLane = @(),
    [string[]]$ConformanceIncludePattern = @(),
    [switch]$KeepWorktrees
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

function Normalize-SelectorList {
    param(
        [string[]]$Values
    )

    $normalized = @()
    foreach ($value in $Values) {
        if ([string]::IsNullOrWhiteSpace($value)) {
            continue
        }
        foreach ($entry in ($value -split ",")) {
            $trimmed = $entry.Trim()
            if (-not [string]::IsNullOrWhiteSpace($trimmed)) {
                $normalized += $trimmed
            }
        }
    }
    return @($normalized)
}

function Assert-PathUnderRoot {
    param(
        [string]$CandidatePath,
        [string]$RootPath
    )

    $resolvedRoot = [System.IO.Path]::GetFullPath($RootPath)
    $resolvedCandidate = [System.IO.Path]::GetFullPath($CandidatePath)
    if (-not $resolvedCandidate.StartsWith($resolvedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "run-value-model-correctness: path '$resolvedCandidate' escapes root '$resolvedRoot'"
    }
}

function Resolve-GitRef {
    param(
        [string]$RepoRoot,
        [string]$Ref
    )

    $resolved = (& git -C $RepoRoot rev-parse $Ref 2>$null | Select-Object -First 1).Trim()
    if ([string]::IsNullOrWhiteSpace($resolved)) {
        throw "run-value-model-correctness: unable to resolve git ref '$Ref'"
    }
    return $resolved
}

function Ensure-DetachedWorktree {
    param(
        [string]$RepoRoot,
        [string]$WorktreePath,
        [string]$Ref
    )

    Assert-PathUnderRoot -CandidatePath $WorktreePath -RootPath (Join-Path $RepoRoot "temp")
    if (Test-Path $WorktreePath) {
        & git -C $RepoRoot worktree remove --force $WorktreePath | Out-Null
    }
    $parent = Split-Path -Parent $WorktreePath
    if (-not (Test-Path $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    & git -C $RepoRoot worktree add --force --detach $WorktreePath $Ref | Out-Null
}

function Remove-DetachedWorktree {
    param(
        [string]$RepoRoot,
        [string]$WorktreePath
    )

    if (Test-Path $WorktreePath) {
        Assert-PathUnderRoot -CandidatePath $WorktreePath -RootPath (Join-Path $RepoRoot "temp")
        & git -C $RepoRoot worktree remove --force $WorktreePath | Out-Null
    }
}

function Invoke-LoggedCommand {
    param(
        [string]$WorktreePath,
        [string[]]$Command,
        [string]$LogPath
    )

    $logDir = Split-Path -Parent $LogPath
    if (-not (Test-Path $logDir)) {
        New-Item -ItemType Directory -Path $logDir -Force | Out-Null
    }

    $commandText = $Command -join " "
    $commandName = $Command[0]
    $commandArgs = @($Command | Select-Object -Skip 1)
    Set-Content -Path $LogPath -Value @(
        "# Value Model Correctness Lane Log",
        "",
        "- Worktree: $WorktreePath",
        "- Command: $commandText",
        ""
    )

    Push-Location $WorktreePath
    try {
        $scriptHost = if (Get-Command pwsh -ErrorAction SilentlyContinue) {
            "pwsh"
        }
        else {
            "powershell"
        }

        if ($commandName.EndsWith(".ps1", [System.StringComparison]::OrdinalIgnoreCase)) {
            & $scriptHost -NoProfile -ExecutionPolicy Bypass -File $commandName @commandArgs 2>&1 | Tee-Object -FilePath $LogPath -Append
        }
        else {
            & $commandName @commandArgs 2>&1 | Tee-Object -FilePath $LogPath -Append
        }
        if ($LASTEXITCODE -ne 0) {
            throw "command failed (exit=$LASTEXITCODE): $commandText"
        }
    }
    finally {
        Pop-Location
    }
}

function Invoke-LoggedScript {
    param(
        [string]$WorktreePath,
        [string]$ScriptPath,
        [System.Collections.Specialized.OrderedDictionary]$Parameters,
        [string]$LogPath
    )

    $logDir = Split-Path -Parent $LogPath
    if (-not (Test-Path $logDir)) {
        New-Item -ItemType Directory -Path $logDir -Force | Out-Null
    }

    $commandParts = New-Object System.Collections.Generic.List[string]
    $commandParts.Add($ScriptPath)
    foreach ($entry in $Parameters.GetEnumerator()) {
        $commandParts.Add(("-{0}" -f $entry.Key))
        if ($entry.Value -is [System.Array] -and -not ($entry.Value -is [string])) {
            $commandParts.Add(("@({0})" -f (($entry.Value | ForEach-Object { [string]$_ }) -join ", ")))
        }
        else {
            $commandParts.Add([string]$entry.Value)
        }
    }

    Set-Content -Path $LogPath -Value @(
        "# Value Model Correctness Lane Log",
        "",
        "- Worktree: $WorktreePath",
        "- Command: $($commandParts -join ' ')",
        ""
    )

    Push-Location $WorktreePath
    try {
        & $ScriptPath @Parameters 2>&1 | Tee-Object -FilePath $LogPath -Append
        if ($LASTEXITCODE -ne 0) {
            throw "command failed (exit=$LASTEXITCODE): $($commandParts -join ' ')"
        }
    }
    finally {
        Pop-Location
    }
}

Push-Location (Join-Path $PSScriptRoot "..")
try {
    . "$PSScriptRoot/lib-run-context.ps1"

    $repoRoot = Get-RepoRoot
    $resolvedRunId = Resolve-RunId -Name "value-model-correctness" -RequestedRunId $RunId
    $env:OXVBA_RUN_ID = $resolvedRunId

    $resolvedOutputRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    }
    else {
        Join-Path $repoRoot $OutputRoot
    }

    $runRoot = Join-Path $resolvedOutputRoot (Join-Path "runs" ("value_model_correctness_{0}" -f $resolvedRunId))
    $baselineRoot = Join-Path $runRoot "baseline"
    $candidateRoot = Join-Path $runRoot "candidate"
    $comparisonRoot = Join-Path $runRoot "comparison"
    foreach ($dir in @($runRoot, $baselineRoot, $candidateRoot, $comparisonRoot)) {
        if (-not (Test-Path $dir)) {
            New-Item -ItemType Directory -Path $dir -Force | Out-Null
        }
    }

    $worktreeRoot = Join-Path $repoRoot (Join-Path "temp" (Join-Path "value-model-migration" "worktrees"))
    $baselineWorktree = Join-Path $worktreeRoot ("baseline_{0}" -f $resolvedRunId)
    $candidateWorktree = Join-Path $worktreeRoot ("candidate_{0}" -f $resolvedRunId)

    $baselineCommit = Resolve-GitRef -RepoRoot $repoRoot -Ref $BaselineRef
    $candidateCommit = Resolve-GitRef -RepoRoot $repoRoot -Ref $CandidateRef

    Ensure-DetachedWorktree -RepoRoot $repoRoot -WorktreePath $baselineWorktree -Ref $baselineCommit
    Ensure-DetachedWorktree -RepoRoot $repoRoot -WorktreePath $candidateWorktree -Ref $candidateCommit

    $laneDefs = @(
        @{
            id = "conformance_vm"
            summary_artifact = "correctness/conformance_vm.csv"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $artifact = Join-Path $laneDir "conformance_vm.csv"
                $log = Join-Path $laneDir "conformance_vm.log.txt"
                $scriptPath = Join-Path $worktree "scripts/run-conformance.ps1"
                $parameters = [ordered]@{
                    Backend = "vm"
                    ResultsPath = $artifact
                }
                if ($ConformanceIncludePattern -and $ConformanceIncludePattern.Count -gt 0) {
                    $parameters["IncludePattern"] = @($ConformanceIncludePattern)
                }
                Invoke-LoggedScript -WorktreePath $worktree -ScriptPath $scriptPath -Parameters $parameters -LogPath $log
                return @{ artifact = $artifact; log = $log }
            }
        }
        @{
            id = "conformance_jit"
            summary_artifact = "correctness/conformance_jit.csv"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $artifact = Join-Path $laneDir "conformance_jit.csv"
                $log = Join-Path $laneDir "conformance_jit.log.txt"
                $scriptPath = Join-Path $worktree "scripts/run-conformance.ps1"
                $parameters = [ordered]@{
                    Backend = "jit"
                    ResultsPath = $artifact
                }
                if ($ConformanceIncludePattern -and $ConformanceIncludePattern.Count -gt 0) {
                    $parameters["IncludePattern"] = @($ConformanceIncludePattern)
                }
                Invoke-LoggedScript -WorktreePath $worktree -ScriptPath $scriptPath -Parameters $parameters -LogPath $log
                return @{ artifact = $artifact; log = $log }
            }
        }
        @{
            id = "matrix_gate"
            summary_artifact = "correctness/matrix_latest.csv"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness/matrix"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $artifact = Join-Path $laneDir "matrix_latest.csv"
                $summary = Join-Path $laneDir "gate_report.md"
                $log = Join-Path $laneDir "matrix_gate.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    (Join-Path $worktree "scripts/run-matrix.ps1"),
                    "-RunId", $resolvedRunId,
                    "-OutputCsv", $artifact,
                    "-SummaryPath", $summary
                ) -LogPath $log
                return @{ artifact = $artifact; log = $log }
            }
        }
        @{
            id = "project_integration"
            summary_artifact = "correctness/project_integration/PROJECT_INTEGRATION_SUITE_LATEST.csv"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness/project_integration"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $artifact = Join-Path $laneDir "PROJECT_INTEGRATION_SUITE_LATEST.csv"
                $log = Join-Path $laneDir "project_integration.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    (Join-Path $worktree "scripts/run-project-integration-suite.ps1"),
                    "-EvidenceDir", $laneDir,
                    "-RunId", $resolvedRunId,
                    "-NoLatest"
                ) -LogPath $log
                return @{ artifact = $artifact; log = $log }
            }
        }
        @{
            id = "com_early"
            summary_artifact = "correctness/com_early/COM_EARLY_CONFORMANCE_RUN.csv"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness/com_early"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $artifact = Join-Path $laneDir ("COM_EARLY_CONFORMANCE_RUN_{0}.csv" -f $resolvedRunId)
                $log = Join-Path $laneDir "com_early.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    (Join-Path $worktree "scripts/run-com-early-conformance.ps1"),
                    "-EvidenceDir", $laneDir,
                    "-RunId", $resolvedRunId,
                    "-NoLatest"
                ) -LogPath $log
                return @{ artifact = $artifact; log = $log }
            }
        }
        @{
            id = "pointer_helpers"
            summary_artifact = "correctness/pointer_helpers.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "pointer_helpers.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "pointer_helpers_end_to_end", "--", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "native_string"
            summary_artifact = "correctness/native_string.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "native_string.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "native_declare_string_marshalling_end_to_end", "--", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "dispatch_boundary"
            summary_artifact = "correctness/com_client_end_to_end.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_client_end_to_end.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "--", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "registered_com"
            summary_artifact = "correctness/com_client_registered_lane.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_client_registered_lane.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_registered_lane", "--", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "events_identity"
            summary_artifact = "correctness/com_early_project_end_to_end.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_early_project_end_to_end.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end", "--", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "dispatch_exception_details"
            summary_artifact = "correctness/dispatch_exception_details.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "dispatch_exception_details.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_exception_details_surface_deterministically", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "dispatch_exception_resume_next"
            summary_artifact = "correctness/dispatch_exception_resume_next.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "dispatch_exception_resume_next.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_exception_path_routes_through_on_error_resume_next", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "dispatch_exception_rich_excepinfo"
            summary_artifact = "correctness/dispatch_exception_rich_excepinfo.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "dispatch_exception_rich_excepinfo.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_rich_exception_preserves_full_excepinfo_surface", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "event_callback_handler_body"
            summary_artifact = "correctness/event_callback_handler_body.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "event_callback_handler_body.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end", "early_bound_project_registered_testeventserver_withevents_callback_invokes_handler_body", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "event_callback_value_payload"
            summary_artifact = "correctness/event_callback_value_payload.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "event_callback_value_payload.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end", "early_bound_project_registered_testeventserver_withevents_callback_preserves_value_payload", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "registered_event_callback_identity"
            summary_artifact = "correctness/registered_event_callback_identity.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "registered_event_callback_identity.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_registered_lane", "windows_registered_com_lane::registered_event_callback_success_when_event_capable_server_is_configured", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "pointer_variant_scalar_container"
            summary_artifact = "correctness/pointer_variant_scalar_container.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "pointer_variant_scalar_container.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "pointer_helpers_end_to_end", "windows_pointer_helper_e2e::varptr_variant_scalar_variable_exposes_scalar_variant_container_in_vm_and_jit", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "pointer_variant_decimal_container"
            summary_artifact = "correctness/pointer_variant_decimal_container.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "pointer_variant_decimal_container.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "pointer_helpers_end_to_end", "windows_pointer_helper_e2e::varptr_variant_decimal_variable_exposes_decimal_variant_container_in_vm_and_jit", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "pointer_variant_object_rejected"
            summary_artifact = "correctness/pointer_variant_object_rejected.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "pointer_variant_object_rejected.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "pointer_helpers_end_to_end", "windows_pointer_helper_e2e::varptr_variant_object_container_rejects_explicitly_in_vm_and_jit", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "pointer_variant_array_rejected"
            summary_artifact = "correctness/pointer_variant_array_rejected.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "pointer_variant_array_rejected.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "pointer_helpers_end_to_end", "windows_pointer_helper_e2e::varptr_variant_array_container_rejects_explicitly_in_vm_and_jit", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_scalar_classifier"
            summary_artifact = "correctness/com_variant_scalar_classifier.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_scalar_classifier.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_classifies_scalar_variant_arguments_at_com_boundary", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_numeric_classifier"
            summary_artifact = "correctness/com_variant_numeric_classifier.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_numeric_classifier.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_classifies_float_currency_and_decimal_arguments_at_com_boundary", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_object_classifier"
            summary_artifact = "correctness/com_variant_object_classifier.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_object_classifier.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_classifies_object_arguments_at_com_boundary", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_array_classifier"
            summary_artifact = "correctness/com_variant_array_classifier.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_array_classifier.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_classifies_array_arguments_at_com_boundary", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_nested_object_array_classifier"
            summary_artifact = "correctness/com_variant_nested_object_array_classifier.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_nested_object_array_classifier.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_classifies_object_elements_inside_variant_arrays_at_com_boundary", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_object_results"
            summary_artifact = "correctness/com_variant_object_results.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_object_results.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_accepts_object_variant_results", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_typed_array_results"
            summary_artifact = "correctness/com_variant_typed_array_results.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_typed_array_results.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_accepts_typed_safe_array_variant_results", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_typed_float_array_results"
            summary_artifact = "correctness/com_variant_typed_float_array_results.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_typed_float_array_results.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_accepts_typed_float_safe_array_variant_results", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_typed_currency_array_results"
            summary_artifact = "correctness/com_variant_typed_currency_array_results.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_typed_currency_array_results.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_accepts_typed_currency_safe_array_variant_results", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_typed_decimal_array_results"
            summary_artifact = "correctness/com_variant_typed_decimal_array_results.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_typed_decimal_array_results.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_accepts_typed_decimal_safe_array_variant_results", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_multidim_matrix_results"
            summary_artifact = "correctness/com_variant_multidim_matrix_results.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_multidim_matrix_results.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_multidim_variant_array_results_preserve_two_dimensional_shape", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_plain_unknown_results"
            summary_artifact = "correctness/com_variant_plain_unknown_results.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_plain_unknown_results.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_plain_unknown_results_fail_with_bounded_nondispatch_diagnostic", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_plain_unknown_arrays"
            summary_artifact = "correctness/com_variant_plain_unknown_arrays.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_plain_unknown_arrays.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_plain_unknown_arrays_fail_with_bounded_nondispatch_diagnostic", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_plain_unknown_variant_arrays"
            summary_artifact = "correctness/com_variant_plain_unknown_variant_arrays.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_plain_unknown_variant_arrays.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_plain_unknown_variant_arrays_fail_with_bounded_nondispatch_diagnostic", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_wide_i64_scalar_boundary"
            summary_artifact = "correctness/com_variant_wide_i64_scalar_boundary.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_wide_i64_scalar_boundary.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_wide_i64_scalar_arguments_normalize_to_vt_i8_at_com_boundary", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "com_variant_wide_i64_array_boundary"
            summary_artifact = "correctness/com_variant_wide_i64_array_boundary.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "com_variant_wide_i64_array_boundary.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "com_client_end_to_end", "windows_com_e2e::dispatchinvoke_wide_i64_variant_array_elements_normalize_to_vt_i8_at_com_boundary", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
        @{
            id = "native_string_writeback_array_slot"
            summary_artifact = "correctness/native_string_writeback_array_slot.log.txt"
            invoke = {
                param($worktree, $sideRoot)
                $laneDir = Join-Path $sideRoot "correctness"
                if (-not (Test-Path $laneDir)) { New-Item -ItemType Directory -Path $laneDir -Force | Out-Null }
                $log = Join-Path $laneDir "native_string_writeback_array_slot.log.txt"
                Invoke-LoggedCommand -WorktreePath $worktree -Command @(
                    "cargo", "test", "-p", "oxvba-host", "--test", "native_declare_string_marshalling_end_to_end", "windows_native_declare_string_e2e::widechartomultibyte_varptr_buffer_target_writes_back_array_slot_in_vm_and_jit", "--", "--exact", "--test-threads=1", "--nocapture"
                ) -LogPath $log
                return @{ artifact = $log; log = $log }
            }
        }
    )

    $includeLaneFilter = Normalize-SelectorList -Values $IncludeLane
    $excludeLaneFilter = Normalize-SelectorList -Values $ExcludeLane

    $selectedLanes = @($laneDefs)
    if ($includeLaneFilter -and $includeLaneFilter.Count -gt 0) {
        $selectedLanes = @($selectedLanes | Where-Object { $_.id -in $includeLaneFilter })
    }
    if ($excludeLaneFilter -and $excludeLaneFilter.Count -gt 0) {
        $selectedLanes = @($selectedLanes | Where-Object { $_.id -notin $excludeLaneFilter })
    }
    if ($selectedLanes.Count -eq 0) {
        throw "run-value-model-correctness: no lanes selected"
    }

    $rows = @()
    $sides = @(
        @{ name = "baseline"; ref = $BaselineRef; commit = $baselineCommit; worktree = $baselineWorktree; root = $baselineRoot },
        @{ name = "candidate"; ref = $CandidateRef; commit = $candidateCommit; worktree = $candidateWorktree; root = $candidateRoot }
    )

    foreach ($side in $sides) {
        foreach ($lane in $selectedLanes) {
            $result = & $lane.invoke $side.worktree $side.root
            $rows += [PSCustomObject]@{
                run_id = $resolvedRunId
                side = $side.name
                ref = $side.ref
                commit = $side.commit
                lane_id = $lane.id
                status = "pass"
                artifact_path = $result.artifact
                log_path = $result.log
            }
        }
    }

    $summaryCsv = Join-Path $runRoot "correctness_summary.csv"
    $rows | Export-Csv -Path $summaryCsv -NoTypeInformation

    $comparisonMd = Join-Path $comparisonRoot "correctness_summary.md"
    $lines = @(
        "# Value Model Correctness Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Baseline ref: $BaselineRef",
        "- Baseline commit: $baselineCommit",
        "- Candidate ref: $CandidateRef",
        "- Candidate commit: $candidateCommit",
        "- Lanes: $($selectedLanes.Count)",
        "",
        "| Lane | Baseline artifact | Candidate artifact |",
        "|---|---|---|"
    )
    foreach ($lane in $selectedLanes) {
        $baselineRow = $rows | Where-Object { $_.side -eq "baseline" -and $_.lane_id -eq $lane.id } | Select-Object -First 1
        $candidateRow = $rows | Where-Object { $_.side -eq "candidate" -and $_.lane_id -eq $lane.id } | Select-Object -First 1
        $lines += "| $($lane.id) | $($baselineRow.artifact_path) | $($candidateRow.artifact_path) |"
    }
    Set-Content -Path $comparisonMd -Value ($lines -join "`n")

    Write-Host "value-model correctness run: complete (run_id=$resolvedRunId lanes=$($selectedLanes.Count))"
    Write-Host "value-model correctness run: summary=$summaryCsv"
    Write-Host "value-model correctness run: comparison=$comparisonMd"
}
finally {
    Remove-Item Env:OXVBA_RUN_ID -ErrorAction SilentlyContinue
    if (-not $KeepWorktrees) {
        try {
            if ($repoRoot) {
                Remove-DetachedWorktree -RepoRoot $repoRoot -WorktreePath $baselineWorktree
                Remove-DetachedWorktree -RepoRoot $repoRoot -WorktreePath $candidateWorktree
                & git -C $repoRoot worktree prune | Out-Null
            }
        }
        catch {
            Write-Warning "run-value-model-correctness: failed to clean worktrees: $($_.Exception.Message)"
        }
    }
    Pop-Location
}
