param(
    [int]$Iterations = 3,
    [string]$OutputPath = "docs/evidence/profiles/v56/benchmark_latest.md"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    function Measure-Run([string]$disableOpt) {
        $total = 0.0
        for ($i = 0; $i -lt $Iterations; $i++) {
            if ($disableOpt -eq "1") {
                $env:OXVBA_DISABLE_OPT = "1"
            } else {
                Remove-Item Env:OXVBA_DISABLE_OPT -ErrorAction SilentlyContinue
            }
            $elapsed = (Measure-Command {
                & "$PSScriptRoot/run-conformance.ps1" -Backend vm | Out-Null
            }).TotalMilliseconds
            $total += $elapsed
        }
        return [Math]::Round(($total / $Iterations), 2)
    }

    $baseline = Measure-Run "1"
    $optimized = Measure-Run "0"
    $gain = if ($baseline -gt 0) { [Math]::Round((($baseline - $optimized) / $baseline) * 100.0, 2) } else { 0.0 }

    $dir = Split-Path -Parent $OutputPath
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }

    $timestampUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $lines = @(
        "# Performance Benchmark",
        "",
        "- Timestamp (UTC): $timestampUtc",
        "- Iterations: $Iterations",
        "- Baseline (opt disabled) ms: $baseline",
        "- Optimized ms: $optimized",
        "- Gain percent: $gain"
    )
    $lines | Set-Content $OutputPath
    Write-Host "bench run: baseline=${baseline}ms optimized=${optimized}ms gain=${gain}%"
}
finally {
    Remove-Item Env:OXVBA_DISABLE_OPT -ErrorAction SilentlyContinue
    Pop-Location
}
