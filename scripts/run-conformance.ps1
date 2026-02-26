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
    $status = "ok"
    $slots = ""

    try {
        $output = cargo run -q -p oxvba-cli -- run $_.FullName --dump-slots | Out-String
        $slotLine = ($output -split "`r?`n" | Where-Object { $_ -like "SLOTS:*" } | Select-Object -Last 1)
        if ($slotLine) {
            $slots = $slotLine.Substring(6)
        }
    }
    catch {
        $status = "error"
    }

    $results += [PSCustomObject]@{ file = $name; status = $status; slots = $slots }
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

    if ($expected.slots -and $expected.slots -ne $r.slots) {
        throw "Conformance mismatch for $($r.file): expected slots $($expected.slots), got $($r.slots)"
    }
}

Write-Host "conformance run: ok ($($results.Count) files)"
