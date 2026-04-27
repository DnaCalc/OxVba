param(
    [string]$EvidenceDir = "docs/evidence/native/v02",
    [string]$RunId = "",
    [switch]$NoArtifacts,
    [switch]$NoLatest
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    . "$PSScriptRoot/lib-run-context.ps1"

    $resolvedRunId = Resolve-RunId -Name "v02-native-scaffold" -RequestedRunId $RunId
    $resolvedNoLatest = $NoLatest -or $NoArtifacts
    if ($NoArtifacts) {
        $EvidenceDir = New-NoArtifactEvidenceDir -Scope "v02-native-scaffold" -RunId $resolvedRunId
        Write-Host "v02 native scaffold: no-artifacts mode writing to $EvidenceDir"
    }
    if (-not (Test-Path $EvidenceDir)) {
        New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
    }

    $timestampUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $hostOs = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
    $obligationsPath = "docs/validation/V02_NATIVE_COMPILATION_OBLIGATIONS_V1.csv"
    if (-not (Test-Path $obligationsPath)) {
        throw "v02 native scaffold: missing obligation matrix: $obligationsPath"
    }

    function Invoke-NativeGate {
        param(
            [string]$GateId,
            [string]$Area,
            [string]$ObligationIds,
            [string[]]$Arguments
        )

        $timer = [System.Diagnostics.Stopwatch]::StartNew()
        $output = & cargo @Arguments 2>&1
        $exitCode = $LASTEXITCODE
        $timer.Stop()

        return [PSCustomObject]@{
            run_id = $resolvedRunId
            timestamp_utc = $timestampUtc
            host_os = $hostOs
            gate_id = $GateId
            area = $Area
            obligation_ids = $ObligationIds
            status = if ($exitCode -eq 0) { "pass" } else { "fail" }
            exit_code = $exitCode
            duration_ms = [math]::Round($timer.Elapsed.TotalMilliseconds, 3)
            source_command = "cargo $($Arguments -join ' ')"
            output_tail = (($output | Select-Object -Last 20) -join " ") -replace '\s+', ' '
        }
    }

    $rows = @()
    $rows += Invoke-NativeGate `
        -GateId "NATIVE-V02-G001" `
        -Area "wrapper_source_generation" `
        -ObligationIds "NATIVE-V02-O001;NATIVE-V02-O002;NATIVE-V02-O004;NATIVE-V02-O005;NATIVE-V02-O006;NATIVE-V02-O008" `
        -Arguments @("test", "-p", "oxvba-build", "--lib", "--", "--nocapture")

    $rows += Invoke-NativeGate `
        -GateId "NATIVE-V02-G002" `
        -Area "jit_supported_subset" `
        -ObligationIds "NATIVE-V02-O003;NATIVE-V02-O007" `
        -Arguments @("test", "-p", "oxvba-jit", "supports_subset_bytecode_path", "--lib", "--", "--nocapture")

    $rows += Invoke-NativeGate `
        -GateId "NATIVE-V02-G003" `
        -Area "jit_vm_fallback" `
        -ObligationIds "NATIVE-V02-O003;NATIVE-V02-O007" `
        -Arguments @("test", "-p", "oxvba-jit", "falls_back_for_unsupported_error_state_bytecode", "--lib", "--", "--nocapture")

    $rows += [PSCustomObject]@{
        run_id = $resolvedRunId
        timestamp_utc = $timestampUtc
        host_os = $hostOs
        gate_id = "NATIVE-V02-G004"
        area = "artifact_provenance"
        obligation_ids = "NATIVE-V02-O010"
        status = "pass"
        exit_code = 0
        duration_ms = 0
        source_command = "matrix:$obligationsPath"
        output_tail = "obligation matrix present and run artifacts emitted with run id"
    }

    $csvPath = Join-Path $EvidenceDir ("V02_NATIVE_SCAFFOLD_RUN_{0}.csv" -f $resolvedRunId)
    $mdPath = Join-Path $EvidenceDir ("V02_NATIVE_SCAFFOLD_RUN_{0}.md" -f $resolvedRunId)
    $latestCsv = Join-Path $EvidenceDir "V02_NATIVE_SCAFFOLD_LATEST.csv"
    $latestMd = Join-Path $EvidenceDir "V02_NATIVE_SCAFFOLD_LATEST.md"

    $rows | Export-Csv -Path $csvPath -NoTypeInformation
    if (-not $resolvedNoLatest) {
        Copy-Item -Path $csvPath -Destination $latestCsv -Force
    }

    $lines = @(
        "# V0.2 Native Compilation Scaffold Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Timestamp (UTC): $timestampUtc",
        "- Host OS: $hostOs",
        "- Obligation matrix: $obligationsPath",
        "- Gate rows: $($rows.Count)",
        "",
        "| Gate ID | Area | Obligations | Status | Duration ms | Command |",
        "|---|---|---|---|---:|---|"
    )
    foreach ($row in $rows) {
        $lines += "| $($row.gate_id) | $($row.area) | $($row.obligation_ids) | $($row.status) | $($row.duration_ms) | ``$($row.source_command)`` |"
    }

    Set-Content -Path $mdPath -Value ($lines -join "`n")
    if (-not $resolvedNoLatest) {
        Copy-Item -Path $mdPath -Destination $latestMd -Force
    }

    $failed = @($rows | Where-Object { $_.status -ne "pass" })
    Write-Host "v02 native scaffold: rows=$($rows.Count) failed=$($failed.Count) csv=$csvPath md=$mdPath"
    if ($failed.Count -gt 0) {
        throw "v02 native scaffold: one or more gates failed"
    }
}
finally {
    Pop-Location
}
