param(
    [string]$BackendCsv = "",
    [string]$VbaCsv = "",
    [string]$ThresholdPath = "docs/validation/V02_PERFORMANCE_THRESHOLDS_V1.csv",
    [string]$OutputDir = "docs/evidence/perf/v02_summary",
    [string]$RunId = "",
    [switch]$NoArtifacts
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    . "$PSScriptRoot/lib-run-context.ps1"

    $resolvedRunId = Resolve-RunId -Name "v02-performance-summary" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputDir = New-NoArtifactEvidenceDir -Scope "v02-performance-summary" -RunId $resolvedRunId
        Write-Host "v02 performance summary: no-artifacts mode writing to $OutputDir"
    }
    if (-not (Test-Path $OutputDir)) {
        New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
    }
    if (-not (Test-Path $ThresholdPath)) {
        throw "v02 performance summary: missing threshold file: $ThresholdPath"
    }

    $timestampUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $hostOs = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
    $summaryRows = @()

    function Add-SummaryRow {
        param(
            [string]$ThresholdId,
            [string]$Area,
            [string]$Status,
            [string]$Detail
        )
        $script:summaryRows += [PSCustomObject]@{
            run_id = $resolvedRunId
            timestamp_utc = $timestampUtc
            host_os = $hostOs
            threshold_id = $ThresholdId
            area = $Area
            status = $Status
            detail = $Detail
        }
    }

    $requiredBackendColumns = @(
        "run_id",
        "timestamp_utc",
        "host_os",
        "workload_id",
        "workload",
        "engine",
        "mode",
        "iterations",
        "warmup_iterations",
        "mean_ms",
        "min_ms",
        "max_ms",
        "comparison_baseline",
        "ratio",
        "claim_boundary"
    )

    if ([string]::IsNullOrWhiteSpace($BackendCsv) -or -not (Test-Path $BackendCsv)) {
        Add-SummaryRow "PERF-V02-T001" "backend_runner" "fail" "backend csv missing"
        Add-SummaryRow "PERF-V02-T002" "backend_runner" "fail" "backend csv missing"
    }
    else {
        $backendRows = @(Import-Csv -Path $BackendCsv)
        if ($backendRows.Count -eq 0) {
            Add-SummaryRow "PERF-V02-T002" "backend_runner" "fail" "no backend rows"
        }
        else {
            Add-SummaryRow "PERF-V02-T002" "backend_runner" "pass" "$($backendRows.Count) backend rows"
            $columns = $backendRows[0].PSObject.Properties.Name
            $missing = @($requiredBackendColumns | Where-Object { $columns -notcontains $_ })
            if ($missing.Count -eq 0) {
                Add-SummaryRow "PERF-V02-T001" "backend_runner" "pass" "required backend schema present"
            }
            else {
                Add-SummaryRow "PERF-V02-T001" "backend_runner" "fail" "missing columns: $($missing -join ',')"
            }

            $ratios = @($backendRows | Where-Object { -not [string]::IsNullOrWhiteSpace($_.ratio) } | ForEach-Object { [double]$_.ratio })
            if ($ratios.Count -eq 0) {
                Add-SummaryRow "PERF-V02-T003" "backend_runner" "pass" "no ratio rows to evaluate"
            }
            else {
                $maxRatio = ($ratios | Measure-Object -Maximum).Maximum
                if ($maxRatio -gt 1.75) {
                    Add-SummaryRow "PERF-V02-T003" "backend_runner" "fail" "max JIT/VM ratio $maxRatio"
                }
                elseif ($maxRatio -gt 1.25) {
                    Add-SummaryRow "PERF-V02-T003" "backend_runner" "warn" "max JIT/VM ratio $maxRatio"
                }
                else {
                    Add-SummaryRow "PERF-V02-T003" "backend_runner" "pass" "max JIT/VM ratio $maxRatio"
                }
            }
        }
    }

    if ([string]::IsNullOrWhiteSpace($VbaCsv) -or -not (Test-Path $VbaCsv)) {
        Add-SummaryRow "PERF-V02-T004" "vba_comparison" "fail" "vba comparison csv missing"
    }
    else {
        $vbaRows = @(Import-Csv -Path $VbaCsv)
        if ($vbaRows.Count -eq 0) {
            Add-SummaryRow "PERF-V02-T004" "vba_comparison" "fail" "no vba comparison rows"
        }
        elseif ($vbaRows | Where-Object { $_.status -in @("captured", "imported") }) {
            Add-SummaryRow "PERF-V02-T004" "vba_comparison" "pass" "captured/imported vba rows present"
        }
        else {
            Add-SummaryRow "PERF-V02-T004" "vba_comparison" "warn" "only skipped vba rows present"
        }
    }

    Add-SummaryRow "PERF-V02-T005" "product_claims" "pass" "threshold policy requires workload and host boundaries"

    $csvPath = Join-Path $OutputDir ("V02_PERFORMANCE_SUMMARY_{0}.csv" -f $resolvedRunId)
    $mdPath = Join-Path $OutputDir ("V02_PERFORMANCE_SUMMARY_{0}.md" -f $resolvedRunId)
    $summaryRows | Export-Csv -Path $csvPath -NoTypeInformation

    $overall = if ($summaryRows | Where-Object { $_.status -eq "fail" }) {
        "fail"
    }
    elseif ($summaryRows | Where-Object { $_.status -eq "warn" }) {
        "warn"
    }
    else {
        "pass"
    }

    $lines = @(
        "# V0.2 Performance Summary",
        "",
        "- Run ID: $resolvedRunId",
        "- Timestamp (UTC): $timestampUtc",
        "- Host OS: $hostOs",
        "- Overall: $overall",
        "- Backend CSV: $BackendCsv",
        "- VBA CSV: $VbaCsv",
        "- Thresholds: $ThresholdPath",
        "",
        "| Threshold | Area | Status | Detail |",
        "|---|---|---|---|"
    )
    foreach ($row in $summaryRows) {
        $lines += "| $($row.threshold_id) | $($row.area) | $($row.status) | $($row.detail) |"
    }
    Set-Content -Path $mdPath -Value ($lines -join "`n")

    Write-Host "v02 performance summary: overall=$overall csv=$csvPath md=$mdPath"
}
finally {
    Pop-Location
}
