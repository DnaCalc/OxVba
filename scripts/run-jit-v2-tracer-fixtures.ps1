param(
    [string]$ManifestPath = "conformance/jit_v2/tracer_bullets/manifest.csv",
    [string]$ExpectedPath = "conformance/jit_v2/tracer_bullets/expected_vm_values.csv"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not (Test-Path $ManifestPath)) {
        throw "Missing JIT v2 tracer manifest: $ManifestPath"
    }
    if (-not (Test-Path $ExpectedPath)) {
        throw "Missing JIT v2 expected VM values: $ExpectedPath"
    }

    $manifest = Import-Csv $ManifestPath
    $expectedRows = Import-Csv $ExpectedPath
    $expected = @{}
    foreach ($row in $expectedRows) {
        $expected[$row.id] = $row
    }

    $cliVmStatuses = @(
        "vm-ready",
        "vm-ready-bounds-followup",
        "vm-ready-export-followup"
    )
    $hostedVmRows = @()
    $ran = 0
    foreach ($row in $manifest) {
        if ($cliVmStatuses -notcontains $row.status) {
            if ($row.status.StartsWith("vm-ready")) {
                $hostedVmRows += $row
                Write-Host "jit-v2 tracer: hosted-vm $($row.id) ($($row.status))"
                continue
            }
            Write-Host "jit-v2 tracer: skip $($row.id) ($($row.status))"
            continue
        }
        if (-not $expected.ContainsKey($row.id)) {
            throw "Missing expected VM values for $($row.id)"
        }

        $fixture = Join-Path "conformance/jit_v2/tracer_bullets" $row.file
        if (-not (Test-Path $fixture)) {
            throw "Missing JIT v2 tracer fixture: $fixture"
        }

        $status = "ok"
        $values = ""
        $output = & cargo run -q -p oxvba-cli --bin oxvba-cli -- run $fixture --dump-values 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) {
            $status = "error"
        }
        if ($status -eq "ok") {
            $valueLine = ($output -split "`r?`n" | Where-Object { $_ -like "VALUES:*" } | Select-Object -Last 1)
            if ($valueLine) {
                $values = $valueLine.Substring(7)
            }
        }

        $want = $expected[$row.id]
        if ($status -ne $want.status) {
            throw "JIT v2 tracer $($row.id) status mismatch: expected $($want.status), got $status"
        }
        if ($values -ne $want.values) {
            throw "JIT v2 tracer $($row.id) values mismatch: expected $($want.values), got $values"
        }

        Write-Host "jit-v2 tracer: ok $($row.id) $($row.file)"
        $ran += 1
    }

    if ($hostedVmRows.Count -gt 0) {
        $hostedIds = ($hostedVmRows | ForEach-Object { $_.id }) -join ","
        Write-Host "jit-v2 tracer: running hosted VM seed test for $hostedIds"
        $testOutput = & cargo test -q -p oxvba-host --test jit_v2_tracer_vm_seed -- --nocapture 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) {
            Write-Host $testOutput
            throw "JIT v2 hosted VM seed test failed for $hostedIds"
        }
        Write-Host "jit-v2 tracer: hosted VM seed test ok ($($hostedVmRows.Count) fixtures)"
    }

    Write-Host "jit-v2 tracer VM seed run: ok ($ran cli fixtures, $($hostedVmRows.Count) hosted fixtures)"
}
finally {
    Pop-Location
}
