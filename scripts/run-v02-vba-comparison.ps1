param(
    [int]$Iterations = 5,
    [int]$WarmupIterations = 1,
    [string]$CorpusPath = "docs/validation/V02_PERFORMANCE_BENCHMARK_CORPUS_V1.csv",
    [string]$EvidenceDir = "docs/evidence/perf/v02_vba",
    [string]$RunId = "",
    [string]$ImportCsv = "",
    [switch]$SkipCapture,
    [switch]$NoArtifacts,
    [switch]$NoLatest
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    . "$PSScriptRoot/lib-run-context.ps1"

    $resolvedRunId = Resolve-RunId -Name "v02-vba-comparison" -RequestedRunId $RunId
    $resolvedNoLatest = $NoLatest -or $NoArtifacts
    if ($NoArtifacts) {
        $EvidenceDir = New-NoArtifactEvidenceDir -Scope "v02-vba-comparison" -RunId $resolvedRunId
        Write-Host "v02 vba comparison: no-artifacts mode writing to $EvidenceDir"
    }
    if (-not (Test-Path $EvidenceDir)) {
        New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
    }
    if (-not (Test-Path $CorpusPath)) {
        throw "v02 vba comparison: missing corpus file: $CorpusPath"
    }

    $timestampUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $hostOs = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
    $vbaRows = @(Import-Csv -Path $CorpusPath | Where-Object { $_.vba_comparison -ne "no" })
    $rows = @()

    function New-ResultRow {
        param(
            $workload,
            [string]$Mode,
            [string]$Status,
            [string]$MeanMs = "",
            [string]$MinMs = "",
            [string]$MaxMs = "",
            [string]$Reason = "",
            [string]$SourceCommand = ""
        )
        return [PSCustomObject]@{
            run_id = $resolvedRunId
            timestamp_utc = $timestampUtc
            host_os = $hostOs
            workload_id = $workload.id
            workload = $workload.workload
            engine = "excel_vba"
            mode = $Mode
            status = $Status
            iterations = $Iterations
            warmup_iterations = $WarmupIterations
            mean_ms = $MeanMs
            min_ms = $MinMs
            max_ms = $MaxMs
            comparison_baseline = "oxvba_vm_jit"
            ratio = ""
            claim_boundary = $workload.claim_boundary
            source_command = $SourceCommand
            reason = $Reason
        }
    }

    function Assert-ImportColumns($importRows) {
        $required = @("workload_id", "workload", "engine", "mean_ms", "min_ms", "max_ms")
        foreach ($column in $required) {
            if (-not ($importRows[0].PSObject.Properties.Name -contains $column)) {
                throw "v02 vba comparison: import csv missing required column '$column'"
            }
        }
    }

    if (-not [string]::IsNullOrWhiteSpace($ImportCsv)) {
        if (-not (Test-Path $ImportCsv)) {
            throw "v02 vba comparison: import csv not found: $ImportCsv"
        }
        $importRows = @(Import-Csv -Path $ImportCsv)
        if ($importRows.Count -eq 0) {
            throw "v02 vba comparison: import csv has no rows"
        }
        Assert-ImportColumns $importRows
        foreach ($import in $importRows) {
            $workload = $vbaRows | Where-Object { $_.id -eq $import.workload_id } | Select-Object -First 1
            if ($null -eq $workload) {
                throw "v02 vba comparison: import row references non-VBA workload '$($import.workload_id)'"
            }
            $rows += [PSCustomObject]@{
                run_id = $resolvedRunId
                timestamp_utc = $timestampUtc
                host_os = $hostOs
                workload_id = $import.workload_id
                workload = $import.workload
                engine = $import.engine
                mode = "imported"
                status = "imported"
                iterations = if ($import.iterations) { $import.iterations } else { $Iterations }
                warmup_iterations = if ($import.warmup_iterations) { $import.warmup_iterations } else { "" }
                mean_ms = $import.mean_ms
                min_ms = $import.min_ms
                max_ms = $import.max_ms
                comparison_baseline = if ($import.comparison_baseline) { $import.comparison_baseline } else { "oxvba_vm_jit" }
                ratio = if ($import.ratio) { $import.ratio } else { "" }
                claim_boundary = $workload.claim_boundary
                source_command = "import:$ImportCsv"
                reason = ""
            }
        }
    }
    elseif ($SkipCapture) {
        foreach ($workload in $vbaRows) {
            $rows += New-ResultRow -workload $workload -Mode "skipped" -Status "skipped" -Reason "capture skipped by -SkipCapture" -SourceCommand "skip"
        }
    }
    else {
        try {
            $excel = New-Object -ComObject Excel.Application
            $excel.Visible = $false
            $workbook = $excel.Workbooks.Add()
            $module = $workbook.VBProject.VBComponents.Add(1)
            $module.CodeModule.AddFromString(@'
Option Explicit

Public Function V02PerfScalarLoopArithmetic(ByVal Iterations As Long) As Double
    Dim startTime As Double
    Dim i As Long
    Dim j As Long
    Dim acc As Double
    startTime = Timer
    For i = 1 To Iterations
        For j = 1 To 25000
            acc = acc + ((j Mod 17) * 3.25)
        Next j
    Next i
    V02PerfScalarLoopArithmetic = (Timer - startTime) * 1000#
End Function

Public Function V02PerfStringConcatAndSlice(ByVal Iterations As Long) As Double
    Dim startTime As Double
    Dim i As Long
    Dim j As Long
    Dim text As String
    startTime = Timer
    For i = 1 To Iterations
        text = ""
        For j = 1 To 4000
            text = text & CStr(j)
            If Len(text) > 120 Then text = Mid$(text, 32, 64)
        Next j
    Next i
    V02PerfStringConcatAndSlice = (Timer - startTime) * 1000#
End Function

Public Function V02PerfArrayIterationAndRedim(ByVal Iterations As Long) As Double
    Dim startTime As Double
    Dim i As Long
    Dim j As Long
    Dim total As Long
    Dim values() As Long
    startTime = Timer
    For i = 1 To Iterations
        ReDim values(1 To 2000)
        For j = 1 To 2000
            values(j) = j
        Next j
        ReDim Preserve values(1 To 2500)
        For j = 1 To 2500
            total = total + values(j)
        Next j
    Next i
    V02PerfArrayIterationAndRedim = (Timer - startTime) * 1000#
End Function
'@)

            $macroByWorkload = @{
                "V02-PERF-005" = "V02PerfScalarLoopArithmetic"
                "V02-PERF-006" = "V02PerfStringConcatAndSlice"
                "V02-PERF-007" = "V02PerfArrayIterationAndRedim"
            }
            foreach ($workload in $vbaRows) {
                $macro = $macroByWorkload[$workload.id]
                if ([string]::IsNullOrWhiteSpace($macro)) {
                    $rows += New-ResultRow -workload $workload -Mode "skipped" -Status "skipped" -Reason "no macro mapping for workload" -SourceCommand "Excel.Application.Run"
                    continue
                }
                for ($i = 0; $i -lt $WarmupIterations; $i++) {
                    $null = $excel.Run($macro, 1)
                }
                $samples = @()
                for ($i = 0; $i -lt $Iterations; $i++) {
                    $samples += [double]$excel.Run($macro, 1)
                }
                $rows += New-ResultRow `
                    -workload $workload `
                    -Mode "captured" `
                    -Status "captured" `
                    -MeanMs ([string][math]::Round((($samples | Measure-Object -Average).Average), 3)) `
                    -MinMs ([string][math]::Round((($samples | Measure-Object -Minimum).Minimum), 3)) `
                    -MaxMs ([string][math]::Round((($samples | Measure-Object -Maximum).Maximum), 3)) `
                    -SourceCommand "Excel.Application.Run:$macro"
            }
        }
        catch {
            $reason = "Excel/VBA capture unavailable: $($_.Exception.Message)"
            foreach ($workload in $vbaRows) {
                $rows += New-ResultRow -workload $workload -Mode "skipped" -Status "skipped" -Reason $reason -SourceCommand "Excel.Application"
            }
        }
        finally {
            if ($null -ne $workbook) {
                $workbook.Close($false) | Out-Null
            }
            if ($null -ne $excel) {
                $excel.Quit() | Out-Null
                [System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel) | Out-Null
            }
        }
    }

    $csvPath = Join-Path $EvidenceDir ("V02_VBA_COMPARISON_RUN_{0}.csv" -f $resolvedRunId)
    $mdPath = Join-Path $EvidenceDir ("V02_VBA_COMPARISON_RUN_{0}.md" -f $resolvedRunId)
    $latestCsv = Join-Path $EvidenceDir "V02_VBA_COMPARISON_LATEST.csv"
    $latestMd = Join-Path $EvidenceDir "V02_VBA_COMPARISON_LATEST.md"

    $rows | Export-Csv -Path $csvPath -NoTypeInformation
    if (-not $resolvedNoLatest) {
        Copy-Item -Path $csvPath -Destination $latestCsv -Force
    }

    $lines = @(
        "# V0.2 VBA Comparison Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Timestamp (UTC): $timestampUtc",
        "- Host OS: $hostOs",
        "- Iterations: $Iterations",
        "- Warmup iterations: $WarmupIterations",
        "- Rows: $($rows.Count)",
        "- Corpus: $CorpusPath",
        "",
        "| Workload ID | Workload | Status | Mean ms | Min ms | Max ms | Reason |",
        "|---|---|---|---:|---:|---:|---|"
    )
    foreach ($row in $rows) {
        $lines += "| $($row.workload_id) | $($row.workload) | $($row.status) | $($row.mean_ms) | $($row.min_ms) | $($row.max_ms) | $($row.reason) |"
    }

    Set-Content -Path $mdPath -Value ($lines -join "`n")
    if (-not $resolvedNoLatest) {
        Copy-Item -Path $mdPath -Destination $latestMd -Force
    }

    Write-Host "v02 vba comparison: rows=$($rows.Count) csv=$csvPath md=$mdPath"
}
finally {
    Pop-Location
}
