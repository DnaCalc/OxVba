param(
    [switch]$Check,
    [string]$ManifestPath = "docs/validation/IDEAL_WINDOWS_X64_FIXTURE_MANIFEST_V1.csv",
    [string]$RepositoryRoot = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}
else {
    (Resolve-Path $RepositoryRoot).Path
}
. (Join-Path $PSScriptRoot "lib-ideal-program-validation.ps1")
. (Join-Path $PSScriptRoot "lib-windows-fixture-manifest.ps1")

Push-Location $repoRoot
try {
    Assert-IdealRelativePath -Path $ManifestPath -Owner "Windows fixture manifest path"
    $manifestAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $ManifestPath
    $rows = @(New-WindowsFixtureManifestRows -RepositoryRoot $repoRoot)
    if ($rows.Count -ne 57) {
        throw "sync-windows-fixture-manifest: expected exactly 57 generated rows, found $($rows.Count)"
    }
    $expected = ConvertTo-WindowsFixtureManifestCsv -Rows $rows

    if ($Check) {
        if (-not (Test-Path -LiteralPath $manifestAbs -PathType Leaf)) {
            throw "sync-windows-fixture-manifest: missing canonical manifest '$ManifestPath'"
        }
        $actual = [IO.File]::ReadAllText($manifestAbs)
        $actualComparable = ConvertTo-WindowsFixtureComparableText -Text $actual
        $expectedComparable = ConvertTo-WindowsFixtureComparableText -Text $expected
        if ($actualComparable -cne $expectedComparable) {
            $actualLines = @($actualComparable -split "`n")
            $expectedLines = @($expectedComparable -split "`n")
            $limit = [Math]::Min($actualLines.Count, $expectedLines.Count)
            $firstDifference = $limit + 1
            for ($index = 0; $index -lt $limit; $index++) {
                if ($actualLines[$index] -cne $expectedLines[$index]) {
                    $firstDifference = $index + 1
                    break
                }
            }
            throw "sync-windows-fixture-manifest: canonical manifest is stale or non-deterministic (first differing line=$firstDifference); run ./scripts/sync-windows-fixture-manifest.ps1"
        }
    }
    else {
        $parent = Split-Path -Parent $manifestAbs
        if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
            [void](New-Item -ItemType Directory -Path $parent -Force)
        }
        [IO.File]::WriteAllText($manifestAbs, $expected, [Text.UTF8Encoding]::new($false))
    }

    $sourceCurrent = @($rows | Where-Object source_recipe_state -eq "current").Count
    $sourcePending = @($rows | Where-Object source_recipe_state -eq "pending").Count
    $builtCurrent = @($rows | Where-Object built_artifact_state -eq "current").Count
    $builtPending = @($rows | Where-Object built_artifact_state -eq "pending").Count
    $mode = if ($Check) { "check" } else { "write" }
    Write-Host "sync-windows-fixture-manifest: ok (mode=$mode rows=57 source_current=$sourceCurrent source_pending=$sourcePending built_current=$builtCurrent built_pending=$builtPending capability_credit=none)"
}
finally {
    Pop-Location
}
