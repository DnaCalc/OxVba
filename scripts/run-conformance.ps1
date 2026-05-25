param(
    [ValidateSet("vm", "jit")]
    [string]$Backend = "vm",
    [ValidateSet("basic-language", "all")]
    [string]$Suite = "basic-language",
    [string]$ResultsPath = "",
    [string[]]$IncludePattern = @()
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $testsDir = "conformance/tests"
    $goldenFile = "conformance/golden/values.csv"
    $manifestFile = "conformance/tests_manifest.csv"

    if (-not (Test-Path $testsDir)) {
        throw "Missing conformance test directory: $testsDir"
    }
    if (-not (Test-Path $goldenFile)) {
        throw "Missing golden file: $goldenFile"
    }
    if (-not (Test-Path $manifestFile)) {
        throw "Missing conformance manifest: $manifestFile"
    }

    $manifest = Import-Csv $manifestFile
    $manifestMap = @{}
    foreach ($entry in $manifest) {
        if (-not $entry.file) {
            continue
        }
        if ($manifestMap.ContainsKey($entry.file)) {
            throw "Duplicate conformance manifest entry for $($entry.file)"
        }
        $manifestMap[$entry.file] = $entry
    }

    $results = @()
    $backendArgs = @()
    if ($Backend -eq "jit") {
        throw "The JIT backend is disabled pending the JIT v2 design; use -Backend vm."
    }

    Get-ChildItem -Path $testsDir -Filter *.bas | Sort-Object Name | ForEach-Object {
        $name = $_.Name
        if (-not $manifestMap.ContainsKey($name)) {
            throw "No conformance manifest entry for $name"
        }
        $manifestEntry = $manifestMap[$name]
        if ($Suite -ne "all" -and $manifestEntry.suite -ne $Suite) {
            return
        }
        if ($IncludePattern -and $IncludePattern.Count -gt 0) {
            $matches = $false
            foreach ($pattern in $IncludePattern) {
                if ($name -like $pattern) {
                    $matches = $true
                    break
                }
            }
            if (-not $matches) {
                return
            }
        }
        $status = "ok"
        $values = ""

        $output = ""
        try {
            $output = & cargo run -q -p oxvba-cli --bin oxvba-cli -- run $_.FullName --dump-values @backendArgs 2>$null | Out-String
        }
        catch {
            $status = "error"
        }

        if ($status -eq "ok" -and $LASTEXITCODE -ne 0) {
            $status = "error"
        }

        if ($status -eq "ok") {
            $valueLine = ($output -split "`r?`n" | Where-Object { $_ -like "VALUES:*" } | Select-Object -Last 1)
            if ($valueLine) {
                $values = $valueLine.Substring(7)
            }
        }

        $results += [PSCustomObject]@{ file = $name; status = $status; values = $values }
    }

    $golden = Import-Csv $goldenFile
    $goldenMap = @{}
    foreach ($g in $golden) {
        if ($g.file -and $g.status) {
            $goldenMap[$g.file] = $g
        }
    }

    foreach ($r in $results) {
        if (-not $goldenMap.ContainsKey($r.file)) {
            throw "No golden expectation for $($r.file)"
        }

        $expected = $goldenMap[$r.file]
        if ($expected.status -ne $r.status) {
            throw "Conformance mismatch for $($r.file): expected status $($expected.status), got $($r.status)"
        }

        if ($expected.values -and $expected.values -ne $r.values) {
            throw "Conformance mismatch for $($r.file): expected values $($expected.values), got $($r.values)"
        }
    }

    if ($ResultsPath) {
        $results |
            Select-Object @{Name = "backend"; Expression = { $Backend } }, @{Name = "suite"; Expression = { $Suite } }, file, status, values |
            Export-Csv -Path $ResultsPath -NoTypeInformation
    }

    $filterNote = if ($IncludePattern -and $IncludePattern.Count -gt 0) {
        " filters=$($IncludePattern -join ';')"
    } else {
        ""
    }
    Write-Host "conformance run: ok ($($results.Count) files, backend=$Backend, suite=$Suite$filterNote)"
}
finally {
    Pop-Location
}
