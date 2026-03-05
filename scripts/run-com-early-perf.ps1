param(
    [int]$Iterations = 5,
    [string]$EvidenceDir = "docs/evidence/perf/com_early",
    [string]$RunId = "",
    [switch]$NoArtifacts,
    [switch]$NoLatest
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

function Measure-CommandMs {
    param([string]$Command)
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    $null = Invoke-Expression "$Command 2>&1"
    $timer.Stop()
    if ($LASTEXITCODE -ne 0) {
        throw "command failed: $Command"
    }
    return [math]::Round($timer.Elapsed.TotalMilliseconds, 3)
}

Push-Location (Join-Path $PSScriptRoot "..")
try {
    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-early-perf" -RequestedRunId $RunId
    $resolvedNoLatest = $NoLatest -or $NoArtifacts
    if ($NoArtifacts) {
        $EvidenceDir = New-NoArtifactEvidenceDir -Scope "com-early-perf" -RunId $resolvedRunId
    }

    if (-not (Test-Path $EvidenceDir)) {
        New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
    }

    $rows = @()

    for ($i = 1; $i -le $Iterations; $i++) {
        $rows += [PSCustomObject]@{
            run_id = $resolvedRunId
            iteration = $i
            workload = "compile-earlybind-fixture"
            command = "cargo test -p oxvba-compiler compile_project_module_aware_matches_rewrite_bridge_for_early_bound_fixture -- --nocapture"
            elapsed_ms = (Measure-CommandMs -Command "cargo test -p oxvba-compiler compile_project_module_aware_matches_rewrite_bridge_for_early_bound_fixture -- --nocapture")
        }
        $rows += [PSCustomObject]@{
            run_id = $resolvedRunId
            iteration = $i
            workload = "runtime-earlybind-vm-jit"
            command = "cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_vm_jit_snapshots_match_for_subset -- --nocapture"
            elapsed_ms = (Measure-CommandMs -Command "cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_vm_jit_snapshots_match_for_subset -- --nocapture")
        }
    }

    $csvPath = Join-Path $EvidenceDir ("COM_EARLY_PERF_RUN_{0}.csv" -f $resolvedRunId)
    $mdPath = Join-Path $EvidenceDir ("COM_EARLY_PERF_RUN_{0}.md" -f $resolvedRunId)
    $latestCsv = Join-Path $EvidenceDir "COM_EARLY_PERF_LATEST.csv"
    $latestMd = Join-Path $EvidenceDir "COM_EARLY_PERF_LATEST.md"

    $rows | Export-Csv -Path $csvPath -NoTypeInformation
    if (-not $resolvedNoLatest) {
        Copy-Item -Path $csvPath -Destination $latestCsv -Force
    }

    $lines = @(
        "# COM Early Perf Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Iterations: $Iterations",
        "- Latest pointers updated: $((-not $resolvedNoLatest).ToString().ToLowerInvariant())",
        "",
        "| Workload | Mean ms | Min ms | Max ms |",
        "|---|---:|---:|---:|"
    )
    foreach ($workload in ($rows | Select-Object -ExpandProperty workload -Unique)) {
        $slice = @($rows | Where-Object { $_.workload -eq $workload })
        $mean = [math]::Round((($slice | Measure-Object -Property elapsed_ms -Average).Average), 3)
        $min = [math]::Round((($slice | Measure-Object -Property elapsed_ms -Minimum).Minimum), 3)
        $max = [math]::Round((($slice | Measure-Object -Property elapsed_ms -Maximum).Maximum), 3)
        $lines += "| $workload | $mean | $min | $max |"
    }

    Set-Content -Path $mdPath -Value ($lines -join "`n")
    if (-not $resolvedNoLatest) {
        Copy-Item -Path $mdPath -Destination $latestMd -Force
    }
}
finally {
    Pop-Location
}
