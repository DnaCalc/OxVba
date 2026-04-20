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

    $scriptHost = if (Get-Command pwsh -ErrorAction SilentlyContinue) {
        "pwsh"
    }
    else {
        "powershell"
    }

    Push-Location $WorktreePath
    try {
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
                $command = @(
                    (Join-Path $worktree "scripts/run-conformance.ps1"),
                    "-Backend", "vm",
                    "-ResultsPath", $artifact
                )
                foreach ($pattern in $ConformanceIncludePattern) {
                    $command += @("-IncludePattern", $pattern)
                }
                Invoke-LoggedCommand -WorktreePath $worktree -Command $command -LogPath $log
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
                $command = @(
                    (Join-Path $worktree "scripts/run-conformance.ps1"),
                    "-Backend", "jit",
                    "-ResultsPath", $artifact
                )
                foreach ($pattern in $ConformanceIncludePattern) {
                    $command += @("-IncludePattern", $pattern)
                }
                Invoke-LoggedCommand -WorktreePath $worktree -Command $command -LogPath $log
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
    )

    $selectedLanes = @($laneDefs)
    if ($IncludeLane -and $IncludeLane.Count -gt 0) {
        $selectedLanes = @($selectedLanes | Where-Object { $_.id -in $IncludeLane })
    }
    if ($ExcludeLane -and $ExcludeLane.Count -gt 0) {
        $selectedLanes = @($selectedLanes | Where-Object { $_.id -notin $ExcludeLane })
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
