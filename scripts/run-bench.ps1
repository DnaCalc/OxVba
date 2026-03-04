param(
    [int]$Iterations = 3,
    [string]$ProfileScope = "mvp-profile-v386",
    [string]$OutputPath = "docs/evidence/profiles/v386/benchmark_latest.md",
    [string]$OutputCsvPath = "docs/evidence/profiles/v386/benchmark_latest.csv"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    function Measure-Run([string]$backend, [string]$disableOpt, [string[]]$includePattern) {
        $total = 0.0
        for ($i = 0; $i -lt $Iterations; $i++) {
            if ($disableOpt -eq "1") {
                $env:OXVBA_DISABLE_OPT = "1"
            } else {
                Remove-Item Env:OXVBA_DISABLE_OPT -ErrorAction SilentlyContinue
            }
            $elapsed = (Measure-Command {
                if ($includePattern -and $includePattern.Count -gt 0) {
                    & "$PSScriptRoot/run-conformance.ps1" -Backend $backend -IncludePattern $includePattern | Out-Null
                } else {
                    & "$PSScriptRoot/run-conformance.ps1" -Backend $backend | Out-Null
                }
            }).TotalMilliseconds
            $total += $elapsed
        }
        return [Math]::Round(($total / $Iterations), 2)
    }

    $workloads = @(
        @{ name = "conformance_vm"; backend = "vm"; include = @() },
        @{ name = "conformance_jit"; backend = "jit"; include = @() },
        @{
            name = "subset_err_string_financial_vm"
            backend = "vm"
            include = @("err_*", "string_*", "financial_*", "coercion_*", "stdlib_error_*", "stdlib_random_financial_expansion.bas")
        },
        @{
            name = "subset_err_string_financial_jit"
            backend = "jit"
            include = @("err_*", "string_*", "financial_*", "coercion_*", "stdlib_error_*", "stdlib_random_financial_expansion.bas")
        }
    )

    $rows = @()
    foreach ($workload in $workloads) {
        $baseline = Measure-Run $workload.backend "1" $workload.include
        $optimized = Measure-Run $workload.backend "0" $workload.include
        $gain = if ($baseline -gt 0) {
            [Math]::Round((($baseline - $optimized) / $baseline) * 100.0, 2)
        } else {
            0.0
        }

        $rows += [PSCustomObject]@{
            profile_scope = $ProfileScope
            workload = $workload.name
            backend = $workload.backend
            include_pattern = ($workload.include -join ";")
            baseline_ms = $baseline
            optimized_ms = $optimized
            gain_percent = $gain
        }
    }

    $aggregateGain = if ($rows.Count -gt 0) {
        [Math]::Round((($rows | Measure-Object -Property gain_percent -Average).Average), 2)
    } else {
        0.0
    }

    $dir = Split-Path -Parent $OutputPath
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    $csvDir = Split-Path -Parent $OutputCsvPath
    if (-not (Test-Path $csvDir)) {
        New-Item -ItemType Directory -Path $csvDir -Force | Out-Null
    }

    $rows | Export-Csv -Path $OutputCsvPath -NoTypeInformation

    $timestampUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $lines = @(
        "# Performance Benchmark",
        "",
        "- Timestamp (UTC): $timestampUtc",
        "- Profile scope: $ProfileScope",
        "- Iterations: $Iterations",
        "- Workloads: $($rows.Count)",
        "- Aggregate gain percent: $aggregateGain",
        "",
        "| Workload | Backend | Baseline ms | Optimized ms | Gain percent |",
        "|---|---|---:|---:|---:|"
    )

    foreach ($row in $rows) {
        $lines += "| $($row.workload) | $($row.backend) | $($row.baseline_ms) | $($row.optimized_ms) | $($row.gain_percent) |"
    }

    $lines | Set-Content $OutputPath
    Write-Host "bench run: profile=$ProfileScope workloads=$($rows.Count) aggregate_gain=${aggregateGain}%"
}
finally {
    Remove-Item Env:OXVBA_DISABLE_OPT -ErrorAction SilentlyContinue
    Pop-Location
}
