param(
    [string]$BaselineRef = "pre-value-model-migration-2026-04-20",
    [string]$CandidateRef = "HEAD",
    [string]$OutputRoot = "docs/evidence/value_model_migration",
    [string]$RunId = "",
    [int]$Iterations = 3,
    [string[]]$Backend = @("vm", "jit"),
    [string[]]$IncludeWorkload = @(),
    [string[]]$ExcludeWorkload = @(),
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
        throw "run-value-model-string-perf: path '$resolvedCandidate' escapes root '$resolvedRoot'"
    }
}

function Resolve-GitRef {
    param(
        [string]$RepoRoot,
        [string]$Ref
    )

    $resolved = (& git -C $RepoRoot rev-parse $Ref 2>$null | Select-Object -First 1).Trim()
    if ([string]::IsNullOrWhiteSpace($resolved)) {
        throw "run-value-model-string-perf: unable to resolve git ref '$Ref'"
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

function Escape-VbaStringLiteral {
    param([string]$Value)

    return $Value.Replace('"', '""')
}

function New-RepeatString {
    param(
        [string]$Unit,
        [int]$RepeatCount
    )

    if ($RepeatCount -le 0) {
        return ""
    }

    return [string]::Concat((1..$RepeatCount | ForEach-Object { $Unit }))
}

function Format-VbaStringExpression {
    param(
        [string]$Value,
        [int]$ChunkSize = 120
    )

    $chunks = New-Object System.Collections.Generic.List[string]
    for ($index = 0; $index -lt $Value.Length; $index += $ChunkSize) {
        $count = [Math]::Min($ChunkSize, $Value.Length - $index)
        $chunk = $Value.Substring($index, $count)
        $chunks.Add(('"{0}"' -f (Escape-VbaStringLiteral $chunk)))
    }

    if ($chunks.Count -eq 0) {
        return '""'
    }

    if ($chunks.Count -eq 1) {
        return $chunks[0]
    }

    $lines = New-Object System.Collections.Generic.List[string]
    for ($index = 0; $index -lt $chunks.Count; $index++) {
        if ($index -lt ($chunks.Count - 1)) {
            $lines.Add(("        {0} & _" -f $chunks[$index]))
        }
        else {
            $lines.Add(("        {0}" -f $chunks[$index]))
        }
    }
    return ($lines -join "`n")
}

function New-ScalarStringWorkloadSource {
    param(
        [string]$Payload,
        [int]$Iterations,
        [int]$SliceWidth
    )

    $expression = Format-VbaStringExpression -Value $Payload
    $middleStart = [Math]::Max(1, [Math]::Floor($Payload.Length / 3))
    $middleWidth = [Math]::Max(1, [Math]::Min($SliceWidth, $Payload.Length - $middleStart + 1))
    return @"
Option Explicit
Public Sub Main()
    Dim i As Long
    Dim total As Long
    Dim s As String
    s = $expression
    For i = 1 To $Iterations
        total = total + Len(s)
        total = total + Len(Left`$(s, $SliceWidth))
        total = total + Len(Right`$(s, $SliceWidth))
        total = total + Len(Mid`$(s, $middleStart, $middleWidth))
    Next i
End Sub
"@
}

function New-ManyStringWorkloadSource {
    param(
        [int]$PieceCount,
        [int]$PieceWidth,
        [int]$Iterations
    )

    $pieces = for ($index = 1; $index -le $PieceCount; $index++) {
        "{0}{1}" -f ("p{0:d4}" -f $index), (New-RepeatString -Unit "x" -RepeatCount ([Math]::Max(0, $PieceWidth - 5)))
    }
    $payload = $pieces -join "|"
    $expression = Format-VbaStringExpression -Value $payload
    return @"
Option Explicit
Public Sub Main()
    Dim i As Long
    Dim total As Long
    Dim joined As String
    Dim parts As Variant
    joined = $expression
    For i = 1 To $Iterations
        parts = Split(joined, "|")
        total = total + Len(Join(parts, ""))
    Next i
End Sub
"@
}

function New-CodeStringWorkloadSource {
    param(
        [int]$StatementCount,
        [int]$LiteralWidth
    )

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("Option Explicit")
    $lines.Add("Public Sub Main()")
    $lines.Add("    Dim total As Long")
    $lines.Add("    total = 0")
    for ($index = 1; $index -le $StatementCount; $index++) {
        $suffix = ("{0:d4}" -f $index)
        $literal = ("tok{0}{1}" -f $suffix, (New-RepeatString -Unit "c" -RepeatCount ([Math]::Max(0, $LiteralWidth - 7))))
        $escaped = Escape-VbaStringLiteral $literal
        $lines.Add(('    total = total + Len("{0}")' -f $escaped))
    }
    $lines.Add("End Sub")
    return ($lines -join "`n")
}

function Get-StringPerfWorkloads {
    $smallPayload = "abc123xy"
    $mediumPayload = New-RepeatString -Unit "m2" -RepeatCount 128
    $longPayload = New-RepeatString -Unit "L0123456789abcdef" -RepeatCount 128

    return @(
        @{
            id = "small_strings"
            description = "Short scalar string intrinsic churn"
            category = "small"
            string_length = $smallPayload.Length
            loop_count = 12000
            source = (New-ScalarStringWorkloadSource -Payload $smallPayload -Iterations 12000 -SliceWidth 4)
        }
        @{
            id = "medium_strings"
            description = "Medium scalar string intrinsic churn"
            category = "medium"
            string_length = $mediumPayload.Length
            loop_count = 4000
            source = (New-ScalarStringWorkloadSource -Payload $mediumPayload -Iterations 4000 -SliceWidth 32)
        }
        @{
            id = "long_strings"
            description = "Long scalar string intrinsic churn"
            category = "long"
            string_length = $longPayload.Length
            loop_count = 400
            source = (New-ScalarStringWorkloadSource -Payload $longPayload -Iterations 400 -SliceWidth 128)
        }
        @{
            id = "many_strings"
            description = "Split/join churn across many delimited strings"
            category = "many"
            string_length = 24
            item_count = 256
            loop_count = 120
            source = (New-ManyStringWorkloadSource -PieceCount 256 -PieceWidth 24 -Iterations 120)
        }
        @{
            id = "code_strings"
            description = "Large source-text module with many string literals"
            category = "code"
            statement_count = 1800
            string_length = 48
            source = (New-CodeStringWorkloadSource -StatementCount 1800 -LiteralWidth 48)
        }
    )
}

function Write-WorkloadSources {
    param(
        [array]$Workloads,
        [string]$SourceRoot
    )

    if (-not (Test-Path $SourceRoot)) {
        New-Item -ItemType Directory -Path $SourceRoot -Force | Out-Null
    }

    $manifestRows = @()
    foreach ($workload in $Workloads) {
        $sourcePath = Join-Path $SourceRoot ("{0}.bas" -f $workload.id)
        Set-Content -Path $sourcePath -Value $workload.source
        $manifestRows += [PSCustomObject]@{
            workload_id = $workload.id
            category = $workload.category
            description = $workload.description
            string_length = [string]$(if ($null -ne $workload.string_length) { $workload.string_length } else { "" })
            loop_count = [string]$(if ($null -ne $workload.loop_count) { $workload.loop_count } else { "" })
            item_count = [string]$(if ($null -ne $workload.item_count) { $workload.item_count } else { "" })
            statement_count = [string]$(if ($null -ne $workload.statement_count) { $workload.statement_count } else { "" })
            source_path = $sourcePath
        }
        $workload.source_path = $sourcePath
    }

    $manifestPath = Join-Path $SourceRoot "workload_manifest.csv"
    $manifestRows | Export-Csv -Path $manifestPath -NoTypeInformation
    return $manifestPath
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
        throw "run-value-model-string-perf: -Iterations must be at least 1"
    }

    $repoRoot = Get-RepoRoot
    $resolvedRunId = Resolve-RunId -Name "value-model-string-perf" -RequestedRunId $RunId
    $env:OXVBA_RUN_ID = $resolvedRunId

    $resolvedOutputRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    }
    else {
        Join-Path $repoRoot $OutputRoot
    }

    $runRoot = Join-Path $resolvedOutputRoot (Join-Path "runs" ("value_model_string_perf_{0}" -f $resolvedRunId))
    $baselineRoot = Join-Path $runRoot "baseline"
    $candidateRoot = Join-Path $runRoot "candidate"
    $comparisonRoot = Join-Path $runRoot "comparison"
    $generatedSourceRoot = Join-Path $runRoot "generated_sources"
    foreach ($dir in @($runRoot, $baselineRoot, $candidateRoot, $comparisonRoot, $generatedSourceRoot)) {
        if (-not (Test-Path $dir)) {
            New-Item -ItemType Directory -Path $dir -Force | Out-Null
        }
    }

    $selectedBackends = @($Backend | ForEach-Object { $_.ToLowerInvariant() } | Select-Object -Unique)
    foreach ($backendName in $selectedBackends) {
        if ($backendName -notin @("vm", "jit")) {
            throw "run-value-model-string-perf: unsupported backend '$backendName'"
        }
    }

    $workloads = @(Get-StringPerfWorkloads)
    if ($IncludeWorkload -and $IncludeWorkload.Count -gt 0) {
        $workloads = @($workloads | Where-Object { $_.id -in $IncludeWorkload })
    }
    if ($ExcludeWorkload -and $ExcludeWorkload.Count -gt 0) {
        $workloads = @($workloads | Where-Object { $_.id -notin $ExcludeWorkload })
    }
    if ($workloads.Count -eq 0) {
        throw "run-value-model-string-perf: no workloads selected"
    }

    $manifestPath = Write-WorkloadSources -Workloads $workloads -SourceRoot $generatedSourceRoot

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

        foreach ($backendName in $selectedBackends) {
            foreach ($workload in $workloads) {
                $durations = New-Object System.Collections.Generic.List[double]
                $logPath = Join-Path $logDir ("{0}_{1}.log.txt" -f $backendName, $workload.id)
                Set-Content -Path $logPath -Value @(
                    "# Value Model String Perf Lane Log",
                    "",
                    "- Run ID: $resolvedRunId",
                    "- Side: $($side.name)",
                    "- Ref: $($side.ref)",
                    "- Commit: $($side.commit)",
                    "- Backend: $backendName",
                    "- Workload: $($workload.id)",
                    "- Iterations: $Iterations",
                    "- Source: $($workload.source_path)",
                    ""
                )

                for ($iteration = 1; $iteration -le $Iterations; $iteration++) {
                    Add-Content -Path $logPath -Value ("## Iteration {0}" -f $iteration)
                    $command = @("cargo", "run", "-q", "-p", "oxvba-cli", "--", "run", $workload.source_path, "--dump-slots")
                    if ($backendName -eq "jit") {
                        $command += "--jit"
                    }
                    $elapsedMs = Invoke-PerfCommand -WorkingDirectory $side.worktree -Command $command -LogPath $logPath -CargoTargetDir $sideTargetDir
                    Add-Content -Path $logPath -Value ("elapsed_ms={0}" -f $elapsedMs)
                    Add-Content -Path $logPath -Value ""
                    $durations.Add($elapsedMs)
                }

                $averageMs = [Math]::Round((($durations | Measure-Object -Average).Average), 2)
                $minMs = [Math]::Round((($durations | Measure-Object -Minimum).Minimum), 2)
                $maxMs = [Math]::Round((($durations | Measure-Object -Maximum).Maximum), 2)
                $sideSummaryPath = Join-Path $perfDir "string_perf.csv"
                $rows += [PSCustomObject]@{
                    run_id = $resolvedRunId
                    side = $side.name
                    ref = $side.ref
                    commit = $side.commit
                    backend = $backendName
                    workload_id = $workload.id
                    category = $workload.category
                    description = $workload.description
                    iterations = $Iterations
                    average_ms = $averageMs
                    min_ms = $minMs
                    max_ms = $maxMs
                    source_path = $workload.source_path
                    log_path = $logPath
                    artifact_path = $sideSummaryPath
                }
            }
        }
    }

    foreach ($sideName in @("baseline", "candidate")) {
        $sideRows = @($rows | Where-Object { $_.side -eq $sideName })
        $sideSummaryPath = Join-Path (Join-Path $runRoot $sideName) "perf/string_perf.csv"
        $sideRows | Export-Csv -Path $sideSummaryPath -NoTypeInformation
    }

    $summaryCsv = Join-Path $runRoot "string_perf_summary.csv"
    $rows | Export-Csv -Path $summaryCsv -NoTypeInformation

    $comparisonRows = @()
    foreach ($backendName in $selectedBackends) {
        foreach ($workload in $workloads) {
            $baselineRow = $rows | Where-Object {
                $_.side -eq "baseline" -and $_.backend -eq $backendName -and $_.workload_id -eq $workload.id
            } | Select-Object -First 1
            $candidateRow = $rows | Where-Object {
                $_.side -eq "candidate" -and $_.backend -eq $backendName -and $_.workload_id -eq $workload.id
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
                backend = $backendName
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
    }

    $comparisonCsv = Join-Path $comparisonRoot "string_perf_summary.csv"
    $comparisonRows | Export-Csv -Path $comparisonCsv -NoTypeInformation

    $comparisonMd = Join-Path $comparisonRoot "string_perf_summary.md"
    $lines = @(
        "# Value Model String Performance Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Baseline ref: $BaselineRef",
        "- Baseline commit: $baselineCommit",
        "- Candidate ref: $CandidateRef",
        "- Candidate commit: $candidateCommit",
        "- Iterations: $Iterations",
        "- Backends: $($selectedBackends -join ', ')",
        "- Workload manifest: $manifestPath",
        "",
        "| Backend | Workload | Baseline ms | Candidate ms | Delta ms | Delta % |",
        "|---|---|---:|---:|---:|---:|"
    )
    foreach ($row in $comparisonRows) {
        $lines += "| $($row.backend) | $($row.workload_id) | $($row.baseline_ms) | $($row.candidate_ms) | $($row.delta_ms) | $($row.delta_percent) |"
    }
    Set-Content -Path $comparisonMd -Value ($lines -join "`n")

    Write-Host "value-model string perf: complete (run_id=$resolvedRunId workloads=$($workloads.Count) backends=$($selectedBackends.Count))"
    Write-Host "value-model string perf: summary=$summaryCsv"
    Write-Host "value-model string perf: comparison=$comparisonCsv"
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
            Write-Warning "run-value-model-string-perf: failed to clean worktrees: $($_.Exception.Message)"
        }
    }
    Pop-Location
}
