param(
    [string]$ProfileScope = "mvp-full-v146",
    [string]$OutputDir = "docs/evidence/profiles/v146",
    [int]$BenchIterations = 3,
    [switch]$SkipBench
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$lockPath = $null
Push-Location (Join-Path $PSScriptRoot "..")
try {
    $lockDir = "temp/profile-gates"
    if (-not (Test-Path $lockDir)) {
        New-Item -ItemType Directory -Path $lockDir -Force | Out-Null
    }

    $safeScope = ($ProfileScope -replace "[^A-Za-z0-9_.-]", "_")
    $lockPath = Join-Path $lockDir "$safeScope.lock.json"

    if (Test-Path $lockPath) {
        $activePid = 0
        try {
            $existing = Get-Content $lockPath -Raw | ConvertFrom-Json
            $activePid = [int]$existing.pid
        }
        catch {
            $activePid = 0
        }

        if ($activePid -gt 0 -and (Get-Process -Id $activePid -ErrorAction SilentlyContinue)) {
            throw "profile gate already running for scope '$ProfileScope' (pid=$activePid, lock=$lockPath)"
        }

        Remove-Item $lockPath -Force -ErrorAction SilentlyContinue
    }

    [PSCustomObject]@{
        pid = $PID
        profile_scope = $ProfileScope
        output_dir = $OutputDir
        started_utc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    } | ConvertTo-Json | Set-Content $lockPath

    if (-not (Test-Path $OutputDir)) {
        New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
    }

    $matrixCsv = Join-Path $OutputDir "matrix_latest.csv"
    $matrixReport = Join-Path $OutputDir "gate_report.md"
    $benchMd = Join-Path $OutputDir "benchmark_latest.md"
    $benchCsv = Join-Path $OutputDir "benchmark_latest.csv"
    $integratedMd = Join-Path $OutputDir "integrated_gate.md"
    $integratedCsv = Join-Path $OutputDir "integrated_gate.csv"

    & "$PSScriptRoot/run-formal.ps1" -ProfileScope $ProfileScope
    & "$PSScriptRoot/run-matrix.ps1" -ProfileScope $ProfileScope -OutputDir $OutputDir -OutputCsv $matrixCsv -SummaryPath $matrixReport
    if ($SkipBench) {
        $benchLines = @(
            "# Performance Benchmark",
            "",
            "- Timestamp (UTC): $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
            "- Profile scope: $ProfileScope",
            "- Status: skipped (`-SkipBench`)"
        )
        Set-Content -Path $benchMd -Value ($benchLines -join "`n")
        @(
            [PSCustomObject]@{
                profile_scope = $ProfileScope
                workload = "skipped"
                backend = "n/a"
                include_pattern = ""
                baseline_ms = 0
                optimized_ms = 0
                gain_percent = 0
            }
        ) | Export-Csv -Path $benchCsv -NoTypeInformation
        Write-Host "bench run: skipped for profile=$ProfileScope"
    }
    else {
        & "$PSScriptRoot/run-bench.ps1" -ProfileScope $ProfileScope -Iterations $BenchIterations -OutputPath $benchMd -OutputCsvPath $benchCsv
    }

    $matrixPass = $false
    if (Test-Path $matrixReport) {
        $matrixText = Get-Content $matrixReport -Raw
        $matrixPass = $matrixText.Contains("Final gate status: PASS")
    }

    $formalCsv = "docs/evidence/formal/latest_run.csv"
    $formalBlockingPass = $true
    if (Test-Path $formalCsv) {
        $formalRows = Import-Csv $formalCsv
        $blockingRows = $formalRows | Where-Object { $_.blocking -eq "yes" }
        if ($blockingRows) {
            $failedBlocking = $blockingRows | Where-Object { $_.status -ne "pass" }
            $formalBlockingPass = -not $failedBlocking
        }
    }

    $benchPresent = (Test-Path $benchMd) -and (Test-Path $benchCsv)

    $rows = @(
        [PSCustomObject]@{ lane = "formal"; status = $(if ($formalBlockingPass) { "pass" } else { "fail" }); artifact = $formalCsv; note = "blocking obligations must pass" },
        [PSCustomObject]@{ lane = "matrix"; status = $(if ($matrixPass) { "pass" } else { "fail" }); artifact = $matrixReport; note = "required cells gate" },
        [PSCustomObject]@{ lane = "bench"; status = $(if ($benchPresent) { "pass" } else { "fail" }); artifact = $benchMd; note = "mixed workload benchmark artifacts" }
    )

    $rows | Export-Csv -Path $integratedCsv -NoTypeInformation

    $finalPass = ($rows | Where-Object { $_.status -ne "pass" }).Count -eq 0
    $timestampUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

    $lines = @(
        "# Integrated Gate Report",
        "",
        "- Timestamp (UTC): $timestampUtc",
        "- Profile scope: $ProfileScope",
        "- Final gate status: $(if ($finalPass) { 'PASS' } else { 'FAIL' })",
        "",
        "| Lane | Status | Artifact | Note |",
        "|---|---|---|---|"
    )

    foreach ($row in $rows) {
        $lines += "| $($row.lane) | $($row.status) | $($row.artifact) | $($row.note) |"
    }

    Set-Content -Path $integratedMd -Value ($lines -join "`n")
    Write-Host "integrated gate: $(if ($finalPass) { 'PASS' } else { 'FAIL' })"
}
finally {
    if ($lockPath -and (Test-Path $lockPath)) {
        try {
            $existing = Get-Content $lockPath -Raw | ConvertFrom-Json
            if ([int]$existing.pid -eq $PID) {
                Remove-Item $lockPath -Force -ErrorAction SilentlyContinue
            }
        }
        catch {
            Remove-Item $lockPath -Force -ErrorAction SilentlyContinue
        }
    }
    Pop-Location
}
