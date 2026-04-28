param(
    [string]$BaselineRef = "pre-value-model-migration-2026-04-20",
    [string]$CandidateRef = "HEAD",
    [string]$OutputRoot = "docs/evidence/value_model_migration",
    [string]$RunId = "",
    [string[]]$IncludeWorkload = @(),
    [string[]]$ExcludeWorkload = @(),
    [string[]]$IncludeSnapshot = @(),
    [string[]]$ExcludeSnapshot = @(),
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
        throw "run-value-model-memory: path '$resolvedCandidate' escapes root '$resolvedRoot'"
    }
}

function Resolve-GitRef {
    param(
        [string]$RepoRoot,
        [string]$Ref
    )

    $resolved = (& git -C $RepoRoot rev-parse $Ref 2>$null | Select-Object -First 1).Trim()
    if ([string]::IsNullOrWhiteSpace($resolved)) {
        throw "run-value-model-memory: unable to resolve git ref '$Ref'"
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
            Write-Warning "run-value-model-memory: failed to remove worktree '$WorktreePath': $($_.Exception.Message)"
        }
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
        $chunks.Add(('"{0}"' -f (Escape-VbaStringLiteral $Value.Substring($index, $count))))
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

function New-SmallStringWorkloadSource {
    $payload = "abc123xy"
    $expression = Format-VbaStringExpression -Value $payload
    return @"
Option Explicit
Public Sub Main()
    Dim i As Long
    Dim total As Long
    Dim s As String
    s = $expression
    For i = 1 To 12000
        total = total + Len(s)
    Next i
End Sub
"@
}

function New-ManyStringWorkloadSource {
    $pieces = for ($index = 1; $index -le 256; $index++) {
        "{0}{1}" -f ("p{0:d4}" -f $index), (New-RepeatString -Unit "x" -RepeatCount 19)
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
    For i = 1 To 120
        parts = Split(joined, "|")
        total = total + Len(Join(parts, ""))
    Next i
End Sub
"@
}

function New-CodeStringWorkloadSource {
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("Option Explicit")
    $lines.Add("Public Sub Main()")
    $lines.Add("    Dim total As Long")
    $lines.Add("    total = 0")
    for ($index = 1; $index -le 1800; $index++) {
        $suffix = ("{0:d4}" -f $index)
        $literal = ("tok{0}{1}" -f $suffix, (New-RepeatString -Unit "c" -RepeatCount 41))
        $lines.Add(('    total = total + Len("{0}")' -f (Escape-VbaStringLiteral $literal)))
    }
    $lines.Add("End Sub")
    return ($lines -join "`n")
}

function Get-MemoryWorkloads {
    return @(
        @{
            id = "cli_small_strings"
            category = "string_churn"
            description = "CLI run on short scalar string churn"
            command = {
                param($sourcePath)
                @("cargo", "run", "-q", "-p", "oxvba-cli", "--", "run", $sourcePath, "--dump-values")
            }
            source = (New-SmallStringWorkloadSource)
        }
        @{
            id = "cli_many_strings"
            category = "string_churn"
            description = "CLI run on split/join many-string churn"
            command = {
                param($sourcePath)
                @("cargo", "run", "-q", "-p", "oxvba-cli", "--", "run", $sourcePath, "--dump-values")
            }
            source = (New-ManyStringWorkloadSource)
        }
        @{
            id = "cli_code_strings"
            category = "code_strings"
            description = "CLI run on large source-text module with many string literals"
            command = {
                param($sourcePath)
                @("cargo", "run", "-q", "-p", "oxvba-cli", "--", "run", $sourcePath, "--dump-values")
            }
            source = (New-CodeStringWorkloadSource)
        }
        @{
            id = "com_variant_bstr_array"
            category = "com_variant"
            description = "Windows variant bridge typed BSTR SAFEARRAY roundtrip"
            command = {
                param($sourcePath)
                @("cargo", "test", "-q", "-p", "oxvba-com", "typed_bstr_safe_array_roundtrips_through_windows_bridge", "--", "--nocapture")
            }
            source = $null
        }
        @{
            id = "com_variant_wide_i64_array_boundary"
            category = "com_variant"
            description = "Host COM boundary wide I64 variant-array normalization"
            command = {
                param($sourcePath)
                @(
                    "cargo", "test", "-q", "-p", "oxvba-host", "--test", "com_client_end_to_end",
                    "windows_com_e2e::dispatchinvoke_wide_i64_variant_array_elements_normalize_to_vt_i8_at_com_boundary",
                    "--", "--exact", "--test-threads=1", "--nocapture"
                )
            }
            source = $null
        }
        @{
            id = "com_variant_decimal_array"
            category = "com_variant"
            description = "Host COM boundary typed decimal SAFEARRAY result"
            command = {
                param($sourcePath)
                @(
                    "cargo", "test", "-q", "-p", "oxvba-host", "--test", "com_client_end_to_end",
                    "windows_com_e2e::dispatchinvoke_accepts_typed_decimal_safe_array_variant_results",
                    "--", "--exact", "--test-threads=1", "--nocapture"
                )
            }
            source = $null
        }
        @{
            id = "com_variant_object_result"
            category = "com_variant"
            description = "Host COM boundary object-valued Variant rebinding"
            command = {
                param($sourcePath)
                @(
                    "cargo", "test", "-q", "-p", "oxvba-host", "--test", "com_client_end_to_end",
                    "windows_com_e2e::dispatchinvoke_accepts_object_variant_results",
                    "--", "--exact", "--test-threads=1", "--nocapture"
                )
            }
            source = $null
        }
        @{
            id = "com_variant_matrix_result"
            category = "com_variant"
            description = "Host COM boundary multidimensional Variant matrix result"
            command = {
                param($sourcePath)
                @(
                    "cargo", "test", "-q", "-p", "oxvba-host", "--test", "com_client_end_to_end",
                    "windows_com_e2e::dispatchinvoke_multidim_variant_array_results_preserve_two_dimensional_shape",
                    "--", "--exact", "--test-threads=1", "--nocapture"
                )
            }
            source = $null
        }
    )
}

function Get-PointerSnapshots {
    return @(
        @{
            id = "strptr_wide_call"
            description = "StrPtr targets BSTR payload for native wide call"
            command = @(
                "cargo", "test", "-q", "-p", "oxvba-host", "--test", "pointer_helpers_end_to_end",
                "windows_pointer_helper_e2e::strptr_supports_wide_native_call_in_vm_and_jit",
                "--", "--exact", "--nocapture"
            )
        }
        @{
            id = "varptr_string_cell"
            description = "VarPtr(String) exposes BSTR container cell"
            command = @(
                "cargo", "test", "-q", "-p", "oxvba-host", "--test", "pointer_helpers_end_to_end",
                "windows_pointer_helper_e2e::varptr_string_variable_exposes_bstr_container_cell_in_vm_and_jit",
                "--", "--exact", "--nocapture"
            )
        }
        @{
            id = "varptr_variant_cell"
            description = "VarPtr(Variant) exposes VARIANT container cell"
            command = @(
                "cargo", "test", "-q", "-p", "oxvba-host", "--test", "pointer_helpers_end_to_end",
                "windows_pointer_helper_e2e::varptr_variant_variable_exposes_variant_container_in_vm_and_jit",
                "--", "--exact", "--nocapture"
            )
        }
    )
}

function Write-GeneratedSources {
    param(
        [array]$Workloads,
        [string]$SourceRoot
    )

    if (-not (Test-Path $SourceRoot)) {
        New-Item -ItemType Directory -Path $SourceRoot -Force | Out-Null
    }

    $manifestRows = @()
    foreach ($workload in $Workloads) {
        if ($null -eq $workload.source) {
            continue
        }
        $sourcePath = Join-Path $SourceRoot ("{0}.bas" -f $workload.id)
        Set-Content -Path $sourcePath -Value $workload.source
        $workload.source_path = $sourcePath
        $manifestRows += [PSCustomObject]@{
            workload_id = $workload.id
            category = $workload.category
            description = $workload.description
            source_path = $sourcePath
        }
    }

    $manifestPath = Join-Path $SourceRoot "workload_manifest.csv"
    $manifestRows | Export-Csv -Path $manifestPath -NoTypeInformation
    return $manifestPath
}

function Write-LayoutProbeProject {
    param(
        [string]$ProbeRoot,
        [string]$WorktreePath
    )

    if (-not (Test-Path $ProbeRoot)) {
        New-Item -ItemType Directory -Path $ProbeRoot -Force | Out-Null
    }
    $srcRoot = Join-Path $ProbeRoot "src"
    if (-not (Test-Path $srcRoot)) {
        New-Item -ItemType Directory -Path $srcRoot -Force | Out-Null
    }

    $runtimePath = (Join-Path $WorktreePath "crates/oxvba-runtime").Replace('\', '/')
    $comPath = (Join-Path $WorktreePath "crates/oxvba-com").Replace('\', '/')
    $runtimeLibPath = Join-Path $WorktreePath "crates/oxvba-runtime/src/lib.rs"
    $runtimeLibText = Get-Content $runtimeLibPath -Raw
    $usesObjectRef = $runtimeLibText.Contains("ObjectRef")
    $usesObjectHandle = $runtimeLibText.Contains("ObjectHandle")
    if ($usesObjectRef -and $usesObjectHandle) {
        throw "run-value-model-memory: layout probe cannot choose between ObjectRef and ObjectHandle in '$runtimeLibPath'"
    }
    if ((-not $usesObjectRef) -and (-not $usesObjectHandle)) {
        throw "run-value-model-memory: layout probe could not find ObjectRef or ObjectHandle export in '$runtimeLibPath'"
    }

    $identityImport = if ($usesObjectRef) { "ObjectRef" } else { "ObjectHandle" }

    Set-Content -Path (Join-Path $ProbeRoot "Cargo.toml") -Value @"
[package]
name = "value-model-layout-probe"
version = "0.1.0"
edition = "2021"

[workspace]

[dependencies]
oxvba-runtime = { path = "$runtimePath" }
oxvba-com = { path = "$comPath" }
"@

    Set-Content -Path (Join-Path $srcRoot "main.rs") -Value @"
use std::mem::{align_of, size_of};

use oxvba_com::{ComCallbackPayload, ComInvokeArg, ComValue};
use oxvba_runtime::{
    BindingHandle, CurrencyValue, F64Value, $identityImport, RuntimeValue, Variant,
    bstr::BStr,
    safe_array::{SafeArray, SafeArrayBound},
};

fn emit<T>(name: &str) {
    println!("{name},{},{}", size_of::<T>(), align_of::<T>());
}

fn main() {
    println!("type_name,size_bytes,align_bytes");
    emit::<String>("RustString");
    emit::<BStr>("BStr");
    emit::<RuntimeValue>("RuntimeValue");
    emit::<Variant>("Variant");
    emit::<SafeArray>("SafeArray");
    emit::<SafeArrayBound>("SafeArrayBound");
    emit::<F64Value>("F64Value");
    emit::<CurrencyValue>("CurrencyValue");
    emit::<$identityImport>("ObjectIdentityCarrier");
    emit::<BindingHandle>("BindingHandle");
    emit::<ComValue>("ComValue");
    emit::<ComInvokeArg>("ComInvokeArg");
    emit::<ComCallbackPayload>("ComCallbackPayload");
}
"@
}

function Write-CombinedLog {
    param(
        [string]$LogPath,
        [string]$Header,
        [string]$StdoutPath,
        [string]$StderrPath
    )

    $logDir = Split-Path -Parent $LogPath
    if (-not (Test-Path $logDir)) {
        New-Item -ItemType Directory -Path $logDir -Force | Out-Null
    }

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add($Header)
    $lines.Add("")
    $lines.Add("## stdout")
    if (Test-Path $StdoutPath) {
        foreach ($line in Get-Content $StdoutPath) {
            $lines.Add($line)
        }
    }
    $lines.Add("")
    $lines.Add("## stderr")
    if (Test-Path $StderrPath) {
        foreach ($line in Get-Content $StderrPath) {
            $lines.Add($line)
        }
    }
    Set-Content -Path $LogPath -Value $lines
}

function Invoke-MemoryMeasuredProcess {
    param(
        [string]$WorkingDirectory,
        [string[]]$Command,
        [string]$LogPath,
        [string]$CargoTargetDir
    )

    $commandName = $Command[0]
    $commandArgs = @($Command | Select-Object -Skip 1)
    $logDir = Split-Path -Parent $LogPath
    if (-not (Test-Path $logDir)) {
        New-Item -ItemType Directory -Path $logDir -Force | Out-Null
    }

    $stdoutPath = Join-Path $logDir "stdout.tmp.txt"
    $stderrPath = Join-Path $logDir "stderr.tmp.txt"
    Remove-Item $stdoutPath, $stderrPath -ErrorAction SilentlyContinue

    $previousTargetDir = $env:CARGO_TARGET_DIR
    $env:CARGO_TARGET_DIR = $CargoTargetDir
    try {
        $commandText = $Command -join " "
        $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
        $process = Start-Process -FilePath $commandName `
            -ArgumentList $commandArgs `
            -WorkingDirectory $WorkingDirectory `
            -RedirectStandardOutput $stdoutPath `
            -RedirectStandardError $stderrPath `
            -PassThru

        $peakWorkingSet = 0L
        $peakPrivate = 0L
        $peakVirtual = 0L
        $steadyWorkingSet = 0L
        while (-not $process.HasExited) {
            $process.Refresh()
            $peakWorkingSet = [Math]::Max($peakWorkingSet, $process.WorkingSet64)
            $peakPrivate = [Math]::Max($peakPrivate, $process.PrivateMemorySize64)
            $peakVirtual = [Math]::Max($peakVirtual, $process.VirtualMemorySize64)
            $steadyWorkingSet = $process.WorkingSet64
            Start-Sleep -Milliseconds 200
        }

        $process.Refresh()
        $stopwatch.Stop()
        $peakWorkingSet = [Math]::Max($peakWorkingSet, $process.PeakWorkingSet64)
        $peakPrivate = [Math]::Max($peakPrivate, $process.PrivateMemorySize64)
        $peakVirtual = [Math]::Max($peakVirtual, $process.VirtualMemorySize64)
        $steadyWorkingSet = [Math]::Max($steadyWorkingSet, $process.WorkingSet64)

        Write-CombinedLog -LogPath $LogPath -Header @"
# Value Model Memory Lane Log

- Workdir: $WorkingDirectory
- Command: $commandText
- ExitCode: $($process.ExitCode)
- ElapsedMs: $([Math]::Round($stopwatch.Elapsed.TotalMilliseconds, 2))
- PeakWorkingSetBytes: $peakWorkingSet
- SteadyWorkingSetBytes: $steadyWorkingSet
- PeakPrivateBytes: $peakPrivate
- PeakVirtualBytes: $peakVirtual
"@ -StdoutPath $stdoutPath -StderrPath $stderrPath

        if ($process.ExitCode -ne 0) {
            throw "command failed (exit=$($process.ExitCode)): $commandText"
        }

        return [PSCustomObject]@{
            elapsed_ms = [Math]::Round($stopwatch.Elapsed.TotalMilliseconds, 2)
            peak_working_set_bytes = $peakWorkingSet
            steady_working_set_bytes = $steadyWorkingSet
            peak_private_bytes = $peakPrivate
            peak_virtual_bytes = $peakVirtual
            log_path = $LogPath
        }
    }
    finally {
        Remove-Item $stdoutPath, $stderrPath -ErrorAction SilentlyContinue
        if ($null -eq $previousTargetDir) {
            Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
        }
        else {
            $env:CARGO_TARGET_DIR = $previousTargetDir
        }
    }
}

Push-Location (Join-Path $PSScriptRoot "..")
try {
    . "$PSScriptRoot/lib-run-context.ps1"

    $repoRoot = Get-RepoRoot
    $resolvedRunId = Resolve-RunId -Name "value-model-memory" -RequestedRunId $RunId
    $env:OXVBA_RUN_ID = $resolvedRunId

    $resolvedOutputRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    }
    else {
        Join-Path $repoRoot $OutputRoot
    }

    $runRoot = Join-Path $resolvedOutputRoot (Join-Path "runs" ("value_model_memory_{0}" -f $resolvedRunId))
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
    $includeSnapshotFilter = Normalize-SelectorList -Values $IncludeSnapshot
    $excludeSnapshotFilter = Normalize-SelectorList -Values $ExcludeSnapshot

    $workloads = @(Get-MemoryWorkloads)
    if ($includeWorkloadFilter -and $includeWorkloadFilter.Count -gt 0) {
        $workloads = @($workloads | Where-Object { $_.id -in $includeWorkloadFilter })
    }
    if ($excludeWorkloadFilter -and $excludeWorkloadFilter.Count -gt 0) {
        $workloads = @($workloads | Where-Object { $_.id -notin $excludeWorkloadFilter })
    }
    if ($workloads.Count -eq 0) {
        throw "run-value-model-memory: no workloads selected"
    }

    $snapshots = @(Get-PointerSnapshots)
    if ($includeSnapshotFilter -and $includeSnapshotFilter.Count -gt 0) {
        $snapshots = @($snapshots | Where-Object { $_.id -in $includeSnapshotFilter })
    }
    if ($excludeSnapshotFilter -and $excludeSnapshotFilter.Count -gt 0) {
        $snapshots = @($snapshots | Where-Object { $_.id -notin $excludeSnapshotFilter })
    }

    $sourceManifestPath = Write-GeneratedSources -Workloads $workloads -SourceRoot (Join-Path $generatedRoot "sources")

    $worktreeRoot = Join-Path $repoRoot (Join-Path "temp" (Join-Path "value-model-migration" "worktrees"))
    $targetRoot = Join-Path $repoRoot (Join-Path "temp" (Join-Path "value-model-migration" (Join-Path "target" $resolvedRunId)))
    $baselineWorktree = Join-Path $worktreeRoot ("baseline_{0}" -f $resolvedRunId)
    $candidateWorktree = Join-Path $worktreeRoot ("candidate_{0}" -f $resolvedRunId)

    $baselineCommit = Resolve-GitRef -RepoRoot $repoRoot -Ref $BaselineRef
    $candidateCommit = Resolve-GitRef -RepoRoot $repoRoot -Ref $CandidateRef

    Ensure-DetachedWorktree -RepoRoot $repoRoot -WorktreePath $baselineWorktree -Ref $baselineCommit
    Ensure-DetachedWorktree -RepoRoot $repoRoot -WorktreePath $candidateWorktree -Ref $candidateCommit

    $layoutRows = @()
    $processRows = @()
    $snapshotRows = @()
    $sides = @(
        @{ name = "baseline"; ref = $BaselineRef; commit = $baselineCommit; worktree = $baselineWorktree; root = $baselineRoot },
        @{ name = "candidate"; ref = $CandidateRef; commit = $candidateCommit; worktree = $candidateWorktree; root = $candidateRoot }
    )

    foreach ($side in $sides) {
        $memoryDir = Join-Path $side.root "memory"
        $logDir = Join-Path $memoryDir "logs"
        if (-not (Test-Path $logDir)) {
            New-Item -ItemType Directory -Path $logDir -Force | Out-Null
        }

        $sideTargetDir = Join-Path $targetRoot $side.name
        if (-not (Test-Path $sideTargetDir)) {
            New-Item -ItemType Directory -Path $sideTargetDir -Force | Out-Null
        }

        $probeRoot = Join-Path $generatedRoot ("layout_probe_{0}" -f $side.name)
        Write-LayoutProbeProject -ProbeRoot $probeRoot -WorktreePath $side.worktree
        $layoutLogPath = Join-Path $logDir "layout_probe.log.txt"
        $layoutMetrics = Invoke-MemoryMeasuredProcess -WorkingDirectory $side.worktree -Command @(
            "cargo", "run", "-q", "--manifest-path", (Join-Path $probeRoot "Cargo.toml")
        ) -LogPath $layoutLogPath -CargoTargetDir (Join-Path $sideTargetDir "layout_probe")

        $layoutCsvPath = Join-Path $memoryDir "layout_metrics.csv"
        $layoutOutput = Get-Content $layoutLogPath | Where-Object { $_ -match '^[A-Za-z0-9_]+,\d+,\d+$' }
        if ($layoutOutput.Count -eq 0) {
            throw "run-value-model-memory: layout probe emitted no metric rows for '$($side.name)'"
        }
        @("type_name,size_bytes,align_bytes") + $layoutOutput | Set-Content -Path $layoutCsvPath
        $layoutParsed = Import-Csv -Path $layoutCsvPath
        foreach ($row in $layoutParsed) {
            $layoutRows += [PSCustomObject]@{
                run_id = $resolvedRunId
                side = $side.name
                ref = $side.ref
                commit = $side.commit
                type_name = $row.type_name
                size_bytes = [int]$row.size_bytes
                align_bytes = [int]$row.align_bytes
                artifact_path = $layoutCsvPath
                log_path = $layoutLogPath
                probe_peak_working_set_bytes = $layoutMetrics.peak_working_set_bytes
            }
        }

        foreach ($workload in $workloads) {
            $workloadLog = Join-Path $logDir ("{0}.log.txt" -f $workload.id)
            $command = & $workload.command $workload.source_path
            $metrics = Invoke-MemoryMeasuredProcess -WorkingDirectory $side.worktree -Command $command -LogPath $workloadLog -CargoTargetDir (Join-Path $sideTargetDir $workload.id)
            $processRows += [PSCustomObject]@{
                run_id = $resolvedRunId
                side = $side.name
                ref = $side.ref
                commit = $side.commit
                workload_id = $workload.id
                category = $workload.category
                description = $workload.description
                elapsed_ms = $metrics.elapsed_ms
                peak_working_set_bytes = $metrics.peak_working_set_bytes
                steady_working_set_bytes = $metrics.steady_working_set_bytes
                peak_private_bytes = $metrics.peak_private_bytes
                peak_virtual_bytes = $metrics.peak_virtual_bytes
                artifact_path = (Join-Path $memoryDir "process_memory.csv")
                log_path = $metrics.log_path
            }
        }

        foreach ($snapshot in $snapshots) {
            $snapshotLog = Join-Path $logDir ("pointer_{0}.log.txt" -f $snapshot.id)
            $metrics = Invoke-MemoryMeasuredProcess -WorkingDirectory $side.worktree -Command $snapshot.command -LogPath $snapshotLog -CargoTargetDir (Join-Path $sideTargetDir ("pointer_{0}" -f $snapshot.id))
            $snapshotRows += [PSCustomObject]@{
                run_id = $resolvedRunId
                side = $side.name
                ref = $side.ref
                commit = $side.commit
                snapshot_id = $snapshot.id
                description = $snapshot.description
                elapsed_ms = $metrics.elapsed_ms
                peak_working_set_bytes = $metrics.peak_working_set_bytes
                log_path = $snapshotLog
                artifact_path = (Join-Path $memoryDir "pointer_snapshot_summary.csv")
            }
        }

        $sideProcessRows = @($processRows | Where-Object { $_.side -eq $side.name })
        if ($sideProcessRows.Count -gt 0) {
            $sideProcessRows | Export-Csv -Path (Join-Path $memoryDir "process_memory.csv") -NoTypeInformation
        }

        $sideSnapshotRows = @($snapshotRows | Where-Object { $_.side -eq $side.name })
        if ($sideSnapshotRows.Count -gt 0) {
            $sideSnapshotRows | Export-Csv -Path (Join-Path $memoryDir "pointer_snapshot_summary.csv") -NoTypeInformation
        }
    }

    $layoutSummaryPath = Join-Path $runRoot "layout_metrics_summary.csv"
    $processSummaryPath = Join-Path $runRoot "process_memory_summary.csv"
    $snapshotSummaryPath = Join-Path $runRoot "pointer_snapshot_summary.csv"
    $layoutRows | Export-Csv -Path $layoutSummaryPath -NoTypeInformation
    $processRows | Export-Csv -Path $processSummaryPath -NoTypeInformation
    $snapshotRows | Export-Csv -Path $snapshotSummaryPath -NoTypeInformation

    $layoutComparisonRows = @()
    foreach ($typeName in ($layoutRows.type_name | Select-Object -Unique)) {
        $baselineRow = $layoutRows | Where-Object { $_.side -eq "baseline" -and $_.type_name -eq $typeName } | Select-Object -First 1
        $candidateRow = $layoutRows | Where-Object { $_.side -eq "candidate" -and $_.type_name -eq $typeName } | Select-Object -First 1
        $layoutComparisonRows += [PSCustomObject]@{
            run_id = $resolvedRunId
            type_name = $typeName
            baseline_size_bytes = $baselineRow.size_bytes
            candidate_size_bytes = $candidateRow.size_bytes
            delta_size_bytes = $candidateRow.size_bytes - $baselineRow.size_bytes
            baseline_align_bytes = $baselineRow.align_bytes
            candidate_align_bytes = $candidateRow.align_bytes
            delta_align_bytes = $candidateRow.align_bytes - $baselineRow.align_bytes
        }
    }
    $layoutComparisonCsv = Join-Path $comparisonRoot "layout_metrics.csv"
    $layoutComparisonRows | Export-Csv -Path $layoutComparisonCsv -NoTypeInformation

    $processComparisonRows = @()
    foreach ($workloadId in ($processRows.workload_id | Select-Object -Unique)) {
        $baselineRow = $processRows | Where-Object { $_.side -eq "baseline" -and $_.workload_id -eq $workloadId } | Select-Object -First 1
        $candidateRow = $processRows | Where-Object { $_.side -eq "candidate" -and $_.workload_id -eq $workloadId } | Select-Object -First 1
        $processComparisonRows += [PSCustomObject]@{
            run_id = $resolvedRunId
            workload_id = $workloadId
            category = $baselineRow.category
            baseline_peak_working_set_bytes = $baselineRow.peak_working_set_bytes
            candidate_peak_working_set_bytes = $candidateRow.peak_working_set_bytes
            delta_peak_working_set_bytes = $candidateRow.peak_working_set_bytes - $baselineRow.peak_working_set_bytes
            baseline_steady_working_set_bytes = $baselineRow.steady_working_set_bytes
            candidate_steady_working_set_bytes = $candidateRow.steady_working_set_bytes
            delta_steady_working_set_bytes = $candidateRow.steady_working_set_bytes - $baselineRow.steady_working_set_bytes
            baseline_elapsed_ms = $baselineRow.elapsed_ms
            candidate_elapsed_ms = $candidateRow.elapsed_ms
            delta_elapsed_ms = $candidateRow.elapsed_ms - $baselineRow.elapsed_ms
            baseline_log = $baselineRow.log_path
            candidate_log = $candidateRow.log_path
        }
    }
    $processComparisonCsv = Join-Path $comparisonRoot "process_memory.csv"
    $processComparisonRows | Export-Csv -Path $processComparisonCsv -NoTypeInformation

    $snapshotComparisonMd = Join-Path $comparisonRoot "pointer_snapshot_summary.md"
    $snapshotLines = @(
        "# Value Model Pointer Snapshot Summary",
        "",
        "- Run ID: $resolvedRunId",
        "- Baseline ref: $BaselineRef",
        "- Candidate ref: $CandidateRef",
        "- Source manifest: $sourceManifestPath",
        "",
        "| Snapshot | Baseline log | Candidate log |",
        "|---|---|---|"
    )
    foreach ($snapshotId in ($snapshotRows.snapshot_id | Select-Object -Unique)) {
        $baselineRow = $snapshotRows | Where-Object { $_.side -eq "baseline" -and $_.snapshot_id -eq $snapshotId } | Select-Object -First 1
        $candidateRow = $snapshotRows | Where-Object { $_.side -eq "candidate" -and $_.snapshot_id -eq $snapshotId } | Select-Object -First 1
        $snapshotLines += "| $snapshotId | $($baselineRow.log_path) | $($candidateRow.log_path) |"
    }
    Set-Content -Path $snapshotComparisonMd -Value ($snapshotLines -join "`n")

    Write-Host "value-model memory: complete (run_id=$resolvedRunId workloads=$($workloads.Count) snapshots=$($snapshots.Count))"
    Write-Host "value-model memory: layout=$layoutSummaryPath"
    Write-Host "value-model memory: process=$processSummaryPath"
    Write-Host "value-model memory: snapshots=$snapshotSummaryPath"
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
            Write-Warning "run-value-model-memory: failed to clean worktrees: $($_.Exception.Message)"
        }
    }
    Pop-Location
}
