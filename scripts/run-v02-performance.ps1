param(
    [int]$Iterations = 3,
    [int]$WarmupIterations = 1,
    [string[]]$WorkloadId = @(),
    [string]$CorpusPath = "docs/validation/V02_PERFORMANCE_BENCHMARK_CORPUS_V1.csv",
    [string]$EvidenceDir = "docs/evidence/perf/v02",
    [string]$RunId = "",
    [switch]$NoArtifacts,
    [switch]$NoLatest
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    . "$PSScriptRoot/lib-run-context.ps1"

    $resolvedRunId = Resolve-RunId -Name "v02-performance" -RequestedRunId $RunId
    $resolvedNoLatest = $NoLatest -or $NoArtifacts
    if ($NoArtifacts) {
        $EvidenceDir = New-NoArtifactEvidenceDir -Scope "v02-performance" -RunId $resolvedRunId
        Write-Host "v02 performance: no-artifacts mode writing to $EvidenceDir"
    }
    if (-not (Test-Path $EvidenceDir)) {
        New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
    }
    if (-not (Test-Path $CorpusPath)) {
        throw "v02 performance: missing corpus file: $CorpusPath"
    }

    $corpus = @(Import-Csv -Path $CorpusPath)
    if ($WorkloadId.Count -gt 0) {
        $wanted = @{}
        foreach ($id in $WorkloadId) {
            $wanted[$id] = $true
        }
        $corpus = @($corpus | Where-Object { $wanted.ContainsKey($_.id) })
        foreach ($id in $WorkloadId) {
            if (-not ($corpus | Where-Object { $_.id -eq $id })) {
                throw "v02 performance: unknown workload id '$id'"
            }
        }
    }
    else {
        $corpus = @($corpus | Where-Object { $_.vba_comparison -eq "no" })
    }

    function Split-List([string]$Text) {
        if ([string]::IsNullOrWhiteSpace($Text)) {
            return @()
        }
        return @($Text -split ';' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    }

    function Get-IncludePattern($workload) {
        if ($workload.id -eq "V02-PERF-002") {
            return @(
                "string_compare_option_binary.bas",
                "string_compare_option_text.bas",
                "string_join_array_tag_count.bas",
                "string_mid_statement_mutation.bas",
                "string_vbnullstring_basic.bas",
                "string_vbnullstring_long_error.bas",
                "string_vbnullstring_object_error.bas",
                "string_vbnullstring_predicates.bas"
            )
        }
        return @()
    }

    function Measure-Workload {
        param(
            [scriptblock]$Command,
            [int]$Iterations,
            [int]$WarmupIterations
        )

        for ($i = 0; $i -lt $WarmupIterations; $i++) {
            & $Command | Out-Null
        }

        $samples = @()
        for ($i = 0; $i -lt $Iterations; $i++) {
            $timer = [System.Diagnostics.Stopwatch]::StartNew()
            & $Command | Out-Null
            $timer.Stop()
            $samples += [math]::Round($timer.Elapsed.TotalMilliseconds, 3)
        }

        return [PSCustomObject]@{
            mean_ms = [math]::Round((($samples | Measure-Object -Average).Average), 3)
            min_ms = [math]::Round((($samples | Measure-Object -Minimum).Minimum), 3)
            max_ms = [math]::Round((($samples | Measure-Object -Maximum).Maximum), 3)
        }
    }

    $timestampUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $hostOs = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
    $rows = @()

    foreach ($workload in $corpus) {
        if ($workload.id -in @("V02-PERF-001", "V02-PERF-002")) {
            $include = @(Get-IncludePattern $workload)
            foreach ($backend in (Split-List $workload.oxvba_backends)) {
                foreach ($mode in @("baseline_no_opt", "optimized")) {
                    $commandText = "scripts/run-conformance.ps1 -Backend $backend"
                    if ($include.Count -gt 0) {
                        $commandText += " -IncludePattern $($include -join ';')"
                    }
                    $command = {
                        if ($mode -eq "baseline_no_opt") {
                            $env:OXVBA_DISABLE_OPT = "1"
                        }
                        else {
                            Remove-Item Env:OXVBA_DISABLE_OPT -ErrorAction SilentlyContinue
                        }
                        if ($include.Count -gt 0) {
                            & "$PSScriptRoot/run-conformance.ps1" -Backend $backend -IncludePattern $include | Out-Null
                        }
                        else {
                            & "$PSScriptRoot/run-conformance.ps1" -Backend $backend | Out-Null
                        }
                    }.GetNewClosure()
                    $stats = Measure-Workload -Command $command -Iterations $Iterations -WarmupIterations $WarmupIterations
                    $rows += [PSCustomObject]@{
                        run_id = $resolvedRunId
                        timestamp_utc = $timestampUtc
                        host_os = $hostOs
                        workload_id = $workload.id
                        workload = $workload.workload
                        engine = $backend
                        mode = $mode
                        iterations = $Iterations
                        warmup_iterations = $WarmupIterations
                        mean_ms = $stats.mean_ms
                        min_ms = $stats.min_ms
                        max_ms = $stats.max_ms
                        comparison_baseline = ""
                        ratio = ""
                        claim_boundary = $workload.claim_boundary
                        source_command = $commandText
                    }
                }
            }
        }
        elseif ($workload.id -eq "V02-PERF-003") {
            $commandText = "cargo test -p oxvba-host --test project_hosting_examples_end_to_end -- --nocapture"
            $command = { cargo test -p oxvba-host --test project_hosting_examples_end_to_end -- --nocapture }.GetNewClosure()
            $stats = Measure-Workload -Command $command -Iterations $Iterations -WarmupIterations $WarmupIterations
            $rows += [PSCustomObject]@{
                run_id = $resolvedRunId
                timestamp_utc = $timestampUtc
                host_os = $hostOs
                workload_id = $workload.id
                workload = $workload.workload
                engine = "host"
                mode = "optimized"
                iterations = $Iterations
                warmup_iterations = $WarmupIterations
                mean_ms = $stats.mean_ms
                min_ms = $stats.min_ms
                max_ms = $stats.max_ms
                comparison_baseline = ""
                ratio = ""
                claim_boundary = $workload.claim_boundary
                source_command = $commandText
            }
        }
        elseif ($workload.id -eq "V02-PERF-004") {
            $commands = @(
                @{
                    engine = "compiler"
                    label = "compile-earlybind-fixture"
                    text = "cargo test -p oxvba-compiler compile_project_module_aware_matches_rewrite_bridge_for_early_bound_fixture -- --nocapture"
                    block = { cargo test -p oxvba-compiler compile_project_module_aware_matches_rewrite_bridge_for_early_bound_fixture -- --nocapture }
                },
                @{
                    engine = "host"
                    label = "runtime-earlybind-vm-jit"
                    text = "cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_vm_jit_snapshots_match_for_subset -- --nocapture"
                    block = { cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_vm_jit_snapshots_match_for_subset -- --nocapture }
                }
            )
            foreach ($entry in $commands) {
                $stats = Measure-Workload -Command $entry.block -Iterations $Iterations -WarmupIterations $WarmupIterations
                $rows += [PSCustomObject]@{
                    run_id = $resolvedRunId
                    timestamp_utc = $timestampUtc
                    host_os = $hostOs
                    workload_id = $workload.id
                    workload = $entry.label
                    engine = $entry.engine
                    mode = "optimized"
                    iterations = $Iterations
                    warmup_iterations = $WarmupIterations
                    mean_ms = $stats.mean_ms
                    min_ms = $stats.min_ms
                    max_ms = $stats.max_ms
                    comparison_baseline = ""
                    ratio = ""
                    claim_boundary = $workload.claim_boundary
                    source_command = $entry.text
                }
            }
        }
        else {
            Write-Host "v02 performance: skipping workload $($workload.id) in OxVba runner"
        }
    }

    foreach ($group in ($rows | Group-Object workload_id, workload, mode)) {
        $baseline = @($group.Group | Where-Object { $_.engine -eq "vm" } | Select-Object -First 1)
        if ($baseline.Count -eq 1 -and $baseline[0].mean_ms -gt 0) {
            foreach ($row in $group.Group) {
                if ($row.engine -ne "vm") {
                    $row.comparison_baseline = "vm"
                    $row.ratio = [math]::Round(($row.mean_ms / $baseline[0].mean_ms), 4)
                }
            }
        }
    }

    $csvPath = Join-Path $EvidenceDir ("V02_PERFORMANCE_RUN_{0}.csv" -f $resolvedRunId)
    $mdPath = Join-Path $EvidenceDir ("V02_PERFORMANCE_RUN_{0}.md" -f $resolvedRunId)
    $latestCsv = Join-Path $EvidenceDir "V02_PERFORMANCE_LATEST.csv"
    $latestMd = Join-Path $EvidenceDir "V02_PERFORMANCE_LATEST.md"

    $rows | Export-Csv -Path $csvPath -NoTypeInformation
    if (-not $resolvedNoLatest) {
        Copy-Item -Path $csvPath -Destination $latestCsv -Force
    }

    $lines = @(
        "# V0.2 Performance Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Timestamp (UTC): $timestampUtc",
        "- Host OS: $hostOs",
        "- Iterations: $Iterations",
        "- Warmup iterations: $WarmupIterations",
        "- Workload rows: $($rows.Count)",
        "- Corpus: $CorpusPath",
        "",
        "| Workload ID | Workload | Engine | Mode | Mean ms | Min ms | Max ms | Ratio |",
        "|---|---|---|---|---:|---:|---:|---:|"
    )
    foreach ($row in $rows) {
        $lines += "| $($row.workload_id) | $($row.workload) | $($row.engine) | $($row.mode) | $($row.mean_ms) | $($row.min_ms) | $($row.max_ms) | $($row.ratio) |"
    }

    Set-Content -Path $mdPath -Value ($lines -join "`n")
    if (-not $resolvedNoLatest) {
        Copy-Item -Path $mdPath -Destination $latestMd -Force
    }

    Write-Host "v02 performance: rows=$($rows.Count) csv=$csvPath md=$mdPath"
}
finally {
    Remove-Item Env:OXVBA_DISABLE_OPT -ErrorAction SilentlyContinue
    Pop-Location
}
