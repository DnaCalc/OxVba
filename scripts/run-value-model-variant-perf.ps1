param(
    [string]$BaselineRef = "pre-value-model-migration-2026-04-20",
    [string]$CandidateRef = "HEAD",
    [string]$OutputRoot = "docs/evidence/value_model_migration",
    [string]$RunId = "",
    [int]$Iterations = 1,
    [string[]]$IncludeWorkload = @(),
    [string[]]$ExcludeWorkload = @(),
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
        throw "run-value-model-variant-perf: path '$resolvedCandidate' escapes root '$resolvedRoot'"
    }
}

function Resolve-GitRef {
    param(
        [string]$RepoRoot,
        [string]$Ref
    )

    $resolved = (& git -C $RepoRoot rev-parse $Ref 2>$null | Select-Object -First 1).Trim()
    if ([string]::IsNullOrWhiteSpace($resolved)) {
        throw "run-value-model-variant-perf: unable to resolve git ref '$Ref'"
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
        try {
            & git -C $RepoRoot worktree remove --force $WorktreePath | Out-Null
        }
        catch {
            Write-Warning "run-value-model-variant-perf: failed to remove worktree '$WorktreePath': $($_.Exception.Message)"
        }
    }
}

function Get-VariantPerfWorkloads {
    return @(
        @{
            id = "scalar_classifier"
            category = "scalar"
            description = "Classify scalar Variant arguments across the outward COM boundary"
            command = @(
                "cargo", "test", "-q", "-p", "oxvba-host", "--test", "com_client_end_to_end",
                "windows_com_e2e::dispatchinvoke_classifies_scalar_variant_arguments_at_com_boundary",
                "--", "--exact", "--test-threads=1", "--nocapture"
            )
        }
        @{
            id = "numeric_classifier"
            category = "numeric"
            description = "Classify float, currency, and decimal Variant arguments"
            command = @(
                "cargo", "test", "-q", "-p", "oxvba-host", "--test", "com_client_end_to_end",
                "windows_com_e2e::dispatchinvoke_classifies_float_currency_and_decimal_arguments_at_com_boundary",
                "--", "--exact", "--test-threads=1", "--nocapture"
            )
        }
        @{
            id = "typed_array_results"
            category = "typed_array"
            description = "Roundtrip typed SAFEARRAY Variant results through the host COM boundary"
            command = @(
                "cargo", "test", "-q", "-p", "oxvba-host", "--test", "com_client_end_to_end",
                "windows_com_e2e::dispatchinvoke_accepts_typed_safe_array_variant_results",
                "--", "--exact", "--test-threads=1", "--nocapture"
            )
        }
        @{
            id = "typed_decimal_array_results"
            category = "typed_array"
            description = "Roundtrip typed decimal SAFEARRAY Variant results"
            command = @(
                "cargo", "test", "-q", "-p", "oxvba-host", "--test", "com_client_end_to_end",
                "windows_com_e2e::dispatchinvoke_accepts_typed_decimal_safe_array_variant_results",
                "--", "--exact", "--test-threads=1", "--nocapture"
            )
        }
        @{
            id = "object_results"
            category = "object"
            description = "Rebind object-valued VT_DISPATCH and VT_UNKNOWN results"
            command = @(
                "cargo", "test", "-q", "-p", "oxvba-host", "--test", "com_client_end_to_end",
                "windows_com_e2e::dispatchinvoke_accepts_object_variant_results",
                "--", "--exact", "--test-threads=1", "--nocapture"
            )
        }
        @{
            id = "wide_i64_array_boundary"
            category = "wide_i64"
            description = "Normalize wide integer Variant-array elements to VT_I8 at the COM boundary"
            command = @(
                "cargo", "test", "-q", "-p", "oxvba-host", "--test", "com_client_end_to_end",
                "windows_com_e2e::dispatchinvoke_wide_i64_variant_array_elements_normalize_to_vt_i8_at_com_boundary",
                "--", "--exact", "--test-threads=1", "--nocapture"
            )
        }
        @{
            id = "variant_matrix_results"
            category = "matrix"
            description = "Materialize multidimensional Variant matrix results"
            command = @(
                "cargo", "test", "-q", "-p", "oxvba-host", "--test", "com_client_end_to_end",
                "windows_com_e2e::dispatchinvoke_multidim_variant_array_results_preserve_two_dimensional_shape",
                "--", "--exact", "--test-threads=1", "--nocapture"
            )
        }
    )
}

function Write-WorkloadManifest {
    param(
        [array]$Workloads,
        [string]$ManifestPath
    )

    $manifestDir = Split-Path -Parent $ManifestPath
    if (-not (Test-Path $manifestDir)) {
        New-Item -ItemType Directory -Path $manifestDir -Force | Out-Null
    }

    $rows = foreach ($workload in $Workloads) {
        [PSCustomObject]@{
            workload_id = $workload.id
            category = $workload.category
            description = $workload.description
            command = ($workload.command -join " ")
        }
    }
    $rows | Export-Csv -Path $ManifestPath -NoTypeInformation
}

function Invoke-PerfCommand {
    param(
        [string]$WorkingDirectory,
        [string[]]$Command,
        [string]$LogPath,
        [string]$CargoTargetDir
    )

    $logDir = Split-Path -Parent $LogPath
    if (-not (Test-Path $logDir)) {
        New-Item -ItemType Directory -Path $logDir -Force | Out-Null
    }

    $commandText = $Command -join " "
    $commandName = $Command[0]
    $commandArgs = @($Command | Select-Object -Skip 1)

    Push-Location $WorkingDirectory
    $previousTargetDir = $env:CARGO_TARGET_DIR
    try {
        $env:CARGO_TARGET_DIR = $CargoTargetDir
        $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
        & $commandName @commandArgs 2>&1 | Tee-Object -FilePath $LogPath -Append | Out-Null
        $exitCode = $LASTEXITCODE
        $stopwatch.Stop()
        if ($exitCode -ne 0) {
            throw "command failed (exit=$exitCode): $commandText"
        }
        return [Math]::Round($stopwatch.Elapsed.TotalMilliseconds, 2)
    }
    finally {
        if ($null -eq $previousTargetDir) {
            Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:CARGO_TARGET_DIR = $previousTargetDir
        }
        Pop-Location
    }
}

Push-Location (Join-Path $PSScriptRoot "..")
try {
    . "$PSScriptRoot/lib-run-context.ps1"

    if ($Iterations -lt 1) {
        throw "run-value-model-variant-perf: -Iterations must be at least 1"
    }

    $repoRoot = Get-RepoRoot
    $resolvedRunId = Resolve-RunId -Name "value-model-variant-perf" -RequestedRunId $RunId
    $env:OXVBA_RUN_ID = $resolvedRunId

    $resolvedOutputRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    }
    else {
        Join-Path $repoRoot $OutputRoot
    }

    $runRoot = Join-Path $resolvedOutputRoot (Join-Path "runs" ("value_model_variant_perf_{0}" -f $resolvedRunId))
    $baselineRoot = Join-Path $runRoot "baseline"
    $candidateRoot = Join-Path $runRoot "candidate"
    $comparisonRoot = Join-Path $runRoot "comparison"
    $generatedRoot = Join-Path $runRoot "generated"
    foreach ($dir in @($runRoot, $baselineRoot, $candidateRoot, $comparisonRoot, $generatedRoot)) {
        if (-not (Test-Path $dir)) {
            New-Item -ItemType Directory -Path $dir -Force | Out-Null
        }
    }

    $includeWorkloadFilter = Normalize-SelectorList -Values $IncludeWorkload
    $excludeWorkloadFilter = Normalize-SelectorList -Values $ExcludeWorkload

    $workloads = @(Get-VariantPerfWorkloads)
    if ($includeWorkloadFilter -and $includeWorkloadFilter.Count -gt 0) {
        $workloads = @($workloads | Where-Object { $_.id -in $includeWorkloadFilter })
    }
    if ($excludeWorkloadFilter -and $excludeWorkloadFilter.Count -gt 0) {
        $workloads = @($workloads | Where-Object { $_.id -notin $excludeWorkloadFilter })
    }
    if ($workloads.Count -eq 0) {
        throw "run-value-model-variant-perf: no workloads selected"
    }

    $manifestPath = Join-Path $generatedRoot "workload_manifest.csv"
    Write-WorkloadManifest -Workloads $workloads -ManifestPath $manifestPath

    $worktreeRoot = Join-Path $repoRoot (Join-Path "temp" (Join-Path "value-model-migration" "worktrees"))
    $targetRoot = Join-Path $repoRoot (Join-Path "temp" (Join-Path "value-model-migration" (Join-Path "target" $resolvedRunId)))
    $baselineWorktree = Join-Path $worktreeRoot ("baseline_{0}" -f $resolvedRunId)
    $candidateWorktree = Join-Path $worktreeRoot ("candidate_{0}" -f $resolvedRunId)

    $baselineCommit = Resolve-GitRef -RepoRoot $repoRoot -Ref $BaselineRef
    $candidateCommit = Resolve-GitRef -RepoRoot $repoRoot -Ref $CandidateRef

    Ensure-DetachedWorktree -RepoRoot $repoRoot -WorktreePath $baselineWorktree -Ref $baselineCommit
    Ensure-DetachedWorktree -RepoRoot $repoRoot -WorktreePath $candidateWorktree -Ref $candidateCommit

    $rows = @()
    $sides = @(
        @{ name = "baseline"; ref = $BaselineRef; commit = $baselineCommit; worktree = $baselineWorktree; root = $baselineRoot },
        @{ name = "candidate"; ref = $CandidateRef; commit = $candidateCommit; worktree = $candidateWorktree; root = $candidateRoot }
    )

    foreach ($side in $sides) {
        $perfDir = Join-Path $side.root "perf"
        $logDir = Join-Path $perfDir "logs"
        if (-not (Test-Path $logDir)) {
            New-Item -ItemType Directory -Path $logDir -Force | Out-Null
        }
        $sideTargetDir = Join-Path $targetRoot $side.name
        if (-not (Test-Path $sideTargetDir)) {
            New-Item -ItemType Directory -Path $sideTargetDir -Force | Out-Null
        }

        $warmupLog = Join-Path $logDir "warmup.log.txt"
        Set-Content -Path $warmupLog -Value @(
            "# Value Model Variant Perf Warmup",
            "",
            "- Run ID: $resolvedRunId",
            "- Side: $($side.name)",
            "- Ref: $($side.ref)",
            "- Commit: $($side.commit)",
            ""
        )
        Invoke-PerfCommand -WorkingDirectory $side.worktree -Command @(
            "cargo", "test", "-q", "-p", "oxvba-host", "--test", "com_client_end_to_end", "--no-run"
        ) -LogPath $warmupLog -CargoTargetDir $sideTargetDir | Out-Null

        foreach ($workload in $workloads) {
            $durations = New-Object System.Collections.Generic.List[double]
            $logPath = Join-Path $logDir ("{0}.log.txt" -f $workload.id)
            Set-Content -Path $logPath -Value @(
                "# Value Model Variant Perf Lane Log",
                "",
                "- Run ID: $resolvedRunId",
                "- Side: $($side.name)",
                "- Ref: $($side.ref)",
                "- Commit: $($side.commit)",
                "- Workload: $($workload.id)",
                "- Iterations: $Iterations",
                "- Command: $($workload.command -join ' ')",
                ""
            )

            for ($iteration = 1; $iteration -le $Iterations; $iteration++) {
                Add-Content -Path $logPath -Value ("## Iteration {0}" -f $iteration)
                $elapsedMs = Invoke-PerfCommand -WorkingDirectory $side.worktree -Command $workload.command -LogPath $logPath -CargoTargetDir $sideTargetDir
                Add-Content -Path $logPath -Value ("elapsed_ms={0}" -f $elapsedMs)
                Add-Content -Path $logPath -Value ""
                $durations.Add($elapsedMs)
            }

            $averageMs = [Math]::Round((($durations | Measure-Object -Average).Average), 2)
            $minMs = [Math]::Round((($durations | Measure-Object -Minimum).Minimum), 2)
            $maxMs = [Math]::Round((($durations | Measure-Object -Maximum).Maximum), 2)
            $sideSummaryPath = Join-Path $perfDir "variant_perf.csv"
            $rows += [PSCustomObject]@{
                run_id = $resolvedRunId
                side = $side.name
                ref = $side.ref
                commit = $side.commit
                workload_id = $workload.id
                category = $workload.category
                description = $workload.description
                iterations = $Iterations
                average_ms = $averageMs
                min_ms = $minMs
                max_ms = $maxMs
                log_path = $logPath
                artifact_path = $sideSummaryPath
            }
        }
    }

    foreach ($sideName in @("baseline", "candidate")) {
        $sideRows = @($rows | Where-Object { $_.side -eq $sideName })
        $sideSummaryPath = Join-Path (Join-Path $runRoot $sideName) "perf/variant_perf.csv"
        $sideRows | Export-Csv -Path $sideSummaryPath -NoTypeInformation
    }

    $summaryCsv = Join-Path $runRoot "variant_perf_summary.csv"
    $rows | Export-Csv -Path $summaryCsv -NoTypeInformation

    $comparisonRows = @()
    foreach ($workload in $workloads) {
        $baselineRow = $rows | Where-Object {
            $_.side -eq "baseline" -and $_.workload_id -eq $workload.id
        } | Select-Object -First 1
        $candidateRow = $rows | Where-Object {
            $_.side -eq "candidate" -and $_.workload_id -eq $workload.id
        } | Select-Object -First 1
        $deltaMs = [Math]::Round(($candidateRow.average_ms - $baselineRow.average_ms), 2)
        $deltaPercent = if ($baselineRow.average_ms -ne 0) {
            [Math]::Round((($candidateRow.average_ms - $baselineRow.average_ms) / $baselineRow.average_ms) * 100.0, 2)
        }
        else {
            0.0
        }
        $comparisonRows += [PSCustomObject]@{
            run_id = $resolvedRunId
            workload_id = $workload.id
            category = $workload.category
            baseline_ms = $baselineRow.average_ms
            candidate_ms = $candidateRow.average_ms
            delta_ms = $deltaMs
            delta_percent = $deltaPercent
            baseline_log = $baselineRow.log_path
            candidate_log = $candidateRow.log_path
        }
    }

    $comparisonCsv = Join-Path $comparisonRoot "variant_perf_summary.csv"
    $comparisonRows | Export-Csv -Path $comparisonCsv -NoTypeInformation

    $comparisonMd = Join-Path $comparisonRoot "variant_perf_summary.md"
    $lines = @(
        "# Value Model Variant Performance Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Baseline ref: $BaselineRef",
        "- Baseline commit: $baselineCommit",
        "- Candidate ref: $CandidateRef",
        "- Candidate commit: $candidateCommit",
        "- Iterations: $Iterations",
        "- Workload manifest: $manifestPath",
        "",
        "| Workload | Baseline ms | Candidate ms | Delta ms | Delta % |",
        "|---|---:|---:|---:|---:|"
    )
    foreach ($row in $comparisonRows) {
        $lines += "| $($row.workload_id) | $($row.baseline_ms) | $($row.candidate_ms) | $($row.delta_ms) | $($row.delta_percent) |"
    }
    Set-Content -Path $comparisonMd -Value ($lines -join "`n")

    Write-Host "value-model variant perf: complete (run_id=$resolvedRunId workloads=$($workloads.Count))"
    Write-Host "value-model variant perf: summary=$summaryCsv"
    Write-Host "value-model variant perf: comparison=$comparisonCsv"
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
            Write-Warning "run-value-model-variant-perf: failed to clean worktrees: $($_.Exception.Message)"
        }
    }
    Pop-Location
}
