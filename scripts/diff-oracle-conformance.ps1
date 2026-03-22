param(
    [Parameter(Mandatory)][string]$OracleResultsCsv,
    [string]$GoldenCsv = "conformance/golden/smoke.csv",
    [string]$OutputPath = ""
)

$ErrorActionPreference = "Stop"

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not (Test-Path $OracleResultsCsv)) {
        throw "Oracle results CSV not found: $OracleResultsCsv"
    }
    if (-not (Test-Path $GoldenCsv)) {
        throw "Golden CSV not found: $GoldenCsv"
    }

    $oracle = Import-Csv $OracleResultsCsv
    $golden = Import-Csv $GoldenCsv
    $goldenMap = @{}
    foreach ($g in $golden) {
        if ($g.file -and $g.status) {
            $goldenMap[$g.file] = $g
        }
    }

    $results = @()
    $matchCount = 0
    $mismatchCount = 0
    $skipCount = 0
    $missingGolden = 0

    foreach ($row in $oracle) {
        $fileName = $row.file
        $oracleStatus = $row.oracle_status
        $oracleSlots = $row.oracle_slots

        if ($oracleStatus -eq "skip") {
            $skipCount++
            $results += [PSCustomObject]@{
                file    = $fileName
                verdict = "skip"
                oracle  = $oracleStatus
                golden  = ""
                detail  = $row.notes
            }
            continue
        }

        $goldenEntry = $goldenMap[$fileName]
        if (-not $goldenEntry) {
            $missingGolden++
            $results += [PSCustomObject]@{
                file    = $fileName
                verdict = "missing-golden"
                oracle  = "${oracleStatus}:${oracleSlots}"
                golden  = ""
                detail  = "no golden expectation"
            }
            continue
        }

        $goldenStatus = $goldenEntry.status
        $goldenSlots = $goldenEntry.slots

        $match = $false
        if ($oracleStatus -eq $goldenStatus) {
            if ($oracleStatus -eq "ok") {
                $match = ($oracleSlots -eq $goldenSlots)
            } elseif ($oracleStatus -eq "error") {
                $match = $true
            }
        }

        if ($match) {
            $matchCount++
            $verdict = "match"
        } else {
            $mismatchCount++
            $verdict = "MISMATCH"
        }

        $results += [PSCustomObject]@{
            file    = $fileName
            verdict = $verdict
            oracle  = "${oracleStatus}:${oracleSlots}"
            golden  = "${goldenStatus}:${goldenSlots}"
            detail  = ""
        }
    }

    Write-Host "Oracle vs Golden Diff"
    Write-Host "  match=$matchCount mismatch=$mismatchCount skip=$skipCount missing-golden=$missingGolden"
    Write-Host ""

    if ($mismatchCount -gt 0) {
        Write-Host "Divergences:"
        foreach ($r in ($results | Where-Object { $_.verdict -eq "MISMATCH" })) {
            Write-Host "  $($r.file): oracle=$($r.oracle) golden=$($r.golden)"
        }
    } else {
        Write-Host "No divergences found."
    }

    if ($OutputPath) {
        $results | Export-Csv -Path $OutputPath -NoTypeInformation
        Write-Host ""
        Write-Host "Detailed results written to: $OutputPath"
    }
} finally {
    Pop-Location
}
