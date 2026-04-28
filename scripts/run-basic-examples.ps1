param(
    [ValidateSet("vm", "jit")]
    [string]$Backend = "vm"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $examplesDir = "examples/basic"
    $expectedFile = Join-Path $examplesDir "expected.csv"
    $projectExpectedFile = Join-Path $examplesDir "projects/expected.csv"

    if (-not (Test-Path $examplesDir)) {
        throw "Missing examples directory: $examplesDir"
    }
    if (-not (Test-Path $expectedFile)) {
        throw "Missing examples expectation file: $expectedFile"
    }
    if (-not (Test-Path $projectExpectedFile)) {
        throw "Missing project examples expectation file: $projectExpectedFile"
    }

    $backendArgs = @()
    if ($Backend -eq "jit") {
        $backendArgs += "--jit"
    }

    $expected = Import-Csv $expectedFile
    foreach ($row in $expected) {
        $source = Join-Path $examplesDir $row.file
        if (-not (Test-Path $source)) {
            throw "Missing example source: $source"
        }

        $output = & cargo run -q -p oxvba-cli -- run $source --dump-values @backendArgs 2>$null | Out-String
        if ($LASTEXITCODE -ne 0) {
            throw "Example failed: $($row.file)"
        }

        $valueLine = ($output -split "`r?`n" | Where-Object { $_ -like "VALUES:*" } | Select-Object -Last 1)
        if (-not $valueLine) {
            throw "Example emitted no VALUES line: $($row.file)"
        }

        $values = $valueLine.Substring(7)
        if ($values -ne $row.values) {
            throw "Example mismatch for $($row.file): expected values $($row.values), got $values"
        }
    }

    $projectExpected = Import-Csv $projectExpectedFile
    foreach ($row in $projectExpected) {
        $source = Join-Path (Join-Path $examplesDir "projects") $row.path
        if (-not (Test-Path $source)) {
            throw "Missing project example: $source"
        }

        $output = & cargo run -q -p oxvba-cli -- run-project $source --dump-values @backendArgs 2>$null | Out-String
        if ($LASTEXITCODE -ne 0) {
            throw "Project example failed: $($row.path)"
        }

        $valueLine = ($output -split "`r?`n" | Where-Object { $_ -like "VALUES:*" } | Select-Object -Last 1)
        if (-not $valueLine) {
            throw "Project example emitted no VALUES line: $($row.path)"
        }

        $values = $valueLine.Substring(7)
        if ($values -ne $row.values) {
            throw "Project example mismatch for $($row.path): expected values $($row.values), got $values"
        }
    }

    Write-Host "basic examples: ok ($($expected.Count) files, $($projectExpected.Count) projects, backend=$Backend)"
}
finally {
    Pop-Location
}
