$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$testsDir = "conformance/tests"
$goldenFile = "conformance/golden/smoke.csv"

if (-not (Test-Path $testsDir)) {
    throw "Missing conformance test directory: $testsDir"
}
if (-not (Test-Path $goldenFile)) {
    throw "Missing golden file: $goldenFile"
}

$results = @()
Get-ChildItem -Path $testsDir -Filter *.bas | Sort-Object Name | ForEach-Object {
    $name = $_.Name
    try {
        cargo run -p oxvba-cli -- run $_.FullName | Out-Null
        $status = "ok"
    }
    catch {
        $status = "error"
    }
    $results += [PSCustomObject]@{ file = $name; status = $status }
}

$golden = Import-Csv $goldenFile -Header file,status,tag
$goldenMap = @{}
foreach ($g in $golden) {
    if ($g.file -and $g.status) {
        $goldenMap[$g.file] = $g.status
    }
}

foreach ($r in $results) {
    if (-not $goldenMap.ContainsKey($r.file)) {
        throw "No golden expectation for $($r.file)"
    }
    if ($goldenMap[$r.file] -ne $r.status) {
        throw "Conformance mismatch for $($r.file): expected $($goldenMap[$r.file]), got $($r.status)"
    }
}

Write-Host "conformance run: ok ($($results.Count) files)"
