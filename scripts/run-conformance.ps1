param(
    [ValidateSet("vm", "jit")]
    [string]$Backend = "vm",
    [ValidateSet("basic-language", "all")]
    [string]$Suite = "basic-language",
    [string]$ResultsPath = "",
    [string[]]$IncludePattern = @(),
    # Per-fixture wall-clock budget; a runaway fixture (e.g. an accidental
    # infinite loop) is killed and reported as status "timeout" instead of
    # wedging the whole lane.
    [int]$TimeoutSeconds = 60
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

    function Test-F64TokenMatch {
        param([string]$Expected, [string]$Actual)
        if (-not $Expected.StartsWith("f64:") -or -not $Actual.StartsWith("f64:")) {
            return $false
        }
        $culture = [System.Globalization.CultureInfo]::InvariantCulture
        $styles = [System.Globalization.NumberStyles]::Float
        try {
            $e = [double]::Parse($Expected.Substring(4), $styles, $culture)
            $a = [double]::Parse($Actual.Substring(4), $styles, $culture)
        } catch {
            return $false
        }
        $scale = [Math]::Max(1.0, [Math]::Max([Math]::Abs($e), [Math]::Abs($a)))
        [Math]::Abs($e - $a) -le (1e-12 * $scale)
    }

    function Test-ValuesMatch {
        param([string]$Expected, [string]$Actual)
        if ($Expected -eq $Actual) {
            return $true
        }
        $expectedParts = $Expected -split '\|'
        $actualParts = $Actual -split '\|'
        if ($expectedParts.Count -ne $actualParts.Count) {
            return $false
        }
        for ($i = 0; $i -lt $expectedParts.Count; $i++) {
            if ($expectedParts[$i] -eq $actualParts[$i]) {
                continue
            }
            if (Test-F64TokenMatch -Expected $expectedParts[$i] -Actual $actualParts[$i]) {
                continue
            }
            return $false
        }
        $true
    }

    # Build once up front so the per-fixture timeout only measures execution.
    cargo build -q -p oxvba-cli
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build -p oxvba-cli failed"
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
        $psi = [System.Diagnostics.ProcessStartInfo]::new()
        $psi.FileName = "cargo"
        foreach ($a in @("run", "-q", "-p", "oxvba-cli", "--bin", "oxvba-cli", "--", "run", $_.FullName, "--dump-values") + $backendArgs) {
            $psi.ArgumentList.Add($a)
        }
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $psi.UseShellExecute = $false
        $proc = [System.Diagnostics.Process]::Start($psi)
        $stdoutTask = $proc.StandardOutput.ReadToEndAsync()
        $null = $proc.StandardError.ReadToEndAsync()
        if (-not $proc.WaitForExit($TimeoutSeconds * 1000)) {
            $proc.Kill($true)
            $proc.WaitForExit()
            $status = "timeout"
        }
        else {
            $output = $stdoutTask.Result
            if ($proc.ExitCode -ne 0) {
                $status = "error"
            }
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

    # Collect every mismatch before failing so one run reports the full picture.
    $mismatches = @()
    foreach ($r in $results) {
        if (-not $goldenMap.ContainsKey($r.file)) {
            $mismatches += "$($r.file): no golden expectation"
            continue
        }

        $expected = $goldenMap[$r.file]
        if ($expected.status -ne $r.status) {
            $mismatches += "$($r.file): expected status $($expected.status), got $($r.status)"
            continue
        }

        if ($expected.values -and -not (Test-ValuesMatch -Expected $expected.values -Actual $r.values)) {
            $mismatches += "$($r.file): expected values $($expected.values), got $($r.values)"
        }
    }

    if ($mismatches.Count -gt 0) {
        foreach ($m in $mismatches) {
            Write-Host "MISMATCH $m"
        }
        throw "Conformance failed: $($mismatches.Count) mismatch(es) of $($results.Count) files"
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
