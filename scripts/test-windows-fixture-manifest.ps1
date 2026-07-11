param(
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

$validator = Join-Path $PSScriptRoot "validate-windows-fixture-manifest.ps1"
$sync = Join-Path $PSScriptRoot "sync-windows-fixture-manifest.ps1"
$manifestRelativePath = "docs/validation/IDEAL_WINDOWS_X64_FIXTURE_MANIFEST_V1.csv"
$programManifestRelativePath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json"
$programManifest = Get-Content -LiteralPath (Join-Path $repoRoot $programManifestRelativePath) -Raw | ConvertFrom-Json
$ownershipRelativePath = [string]$programManifest.matrix_ownership
$environmentRelativePath = [string]$programManifest.environment_manifest
$matrixContracts = Get-WindowsFixtureMatrixContracts

function Copy-FixtureFile {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath
    )

    $source = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $RelativePath
    $target = Resolve-IdealRepoPath -RepoRoot $FixtureRoot -Path $RelativePath
    $targetParent = Split-Path -Parent $target
    if (-not (Test-Path -LiteralPath $targetParent -PathType Container)) {
        [void](New-Item -ItemType Directory -Path $targetParent -Force)
    }
    Copy-Item -LiteralPath $source -Destination $target -Force
}

function New-WindowsFixtureManifestTestRoot {
    $fixtureRoot = Join-Path $tempBase ([Guid]::NewGuid().ToString("N"))
    [void](New-Item -ItemType Directory -Path $fixtureRoot -Force)

    $paths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($path in @(
        ".beads/issues.jsonl",
        $programManifestRelativePath,
        $ownershipRelativePath,
        $environmentRelativePath,
        $manifestRelativePath
    )) {
        [void]$paths.Add($path.Replace('\', '/'))
    }
    foreach ($matrixPath in $matrixContracts.Values) {
        [void]$paths.Add(([string]$matrixPath).Replace('\', '/'))
        foreach ($row in @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$matrixPath)))) {
            foreach ($token in @(([string]$row.authority_refs -split '[;|]') | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
                if ($token -notmatch '^[a-z][a-z0-9-]*:') {
                    [void]$paths.Add((($token -split '#', 2)[0]).Replace('\', '/'))
                }
            }
        }
    }
    foreach ($row in @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $manifestRelativePath))) {
        if ([string]$row.source_recipe_state -eq "current") {
            foreach ($path in @(([string]$row.source_recipe_paths -split '\|') | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
                [void]$paths.Add($path.Replace('\', '/'))
            }
        }
    }
    foreach ($path in $paths) {
        Copy-FixtureFile -FixtureRoot $fixtureRoot -RelativePath $path
    }
    return $fixtureRoot
}

function Update-ManifestRow {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$RowId,
        [Parameter(Mandatory = $true)][scriptblock]$Mutation
    )

    $path = Resolve-IdealRepoPath -RepoRoot $FixtureRoot -Path $manifestRelativePath
    $rows = @(Import-Csv -LiteralPath $path)
    $matches = @($rows | Where-Object { [string]$_.row_id -eq $RowId })
    if ($matches.Count -ne 1) {
        throw "Windows fixture manifest test expected one row '$RowId', found $($matches.Count)"
    }
    & $Mutation $matches[0]
    $rows | Export-Csv -LiteralPath $path -NoTypeInformation -Encoding UTF8 -UseQuotes Always
}

function Invoke-ExpectedFailure {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Mutation,
        [Parameter(Mandatory = $true)][string]$MessagePattern
    )

    $fixture = New-WindowsFixtureManifestTestRoot
    & $Mutation $fixture
    $failedAsExpected = $false
    try {
        & $validator -RepositoryRoot $fixture *> $null
    }
    catch {
        if ($_.Exception.Message -notmatch $MessagePattern) {
            throw "Windows fixture manifest negative case '$Name' failed for the wrong reason: $($_.Exception.Message)"
        }
        $failedAsExpected = $true
    }
    if (-not $failedAsExpected) {
        throw "Windows fixture manifest negative case '$Name' unexpectedly passed"
    }
    Write-Host "windows-fixture-manifest-negative: ok ($Name)"
}

$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
$tempBase = Join-Path $tempRoot ("oxvba-windows-fixture-manifest-" + [Guid]::NewGuid().ToString("N"))
$resolvedTempBase = [IO.Path]::GetFullPath($tempBase)
if (-not $resolvedTempBase.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Windows fixture manifest test temp root escaped the system temp directory"
}
[void](New-Item -ItemType Directory -Path $tempBase -Force)

try {
    $baseline = New-WindowsFixtureManifestTestRoot
    & $sync -RepositoryRoot $baseline -Check
    & $validator -RepositoryRoot $baseline
    $baselineRows = @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $baseline -Path $manifestRelativePath))
    $legalPending = @($baselineRows | Where-Object {
        [string]$_.source_recipe_state -eq "pending" -and
        [string]$_.source_recipe_hash -eq "pending" -and
        [string]$_.source_recipe_owner_bead -match '^bd-'
    }).Count
    if ($legalPending -le 0) {
        throw "Windows fixture manifest baseline has no legal pending-with-owner source rows"
    }
    Write-Host "windows-fixture-manifest-positive: ok (pending-with-owner=$legalPending)"

    $eolFixture = New-WindowsFixtureManifestTestRoot
    $eolSource = Resolve-IdealRepoPath -RepoRoot $eolFixture -Path "crates/oxvba-runtime/src/bstr.rs"
    $canonicalText = ConvertTo-WindowsFixtureCanonicalText -Bytes ([IO.File]::ReadAllBytes($eolSource)) -Owner "EOL stability fixture"
    [IO.File]::WriteAllText($eolSource, $canonicalText.Replace("`n", "`r`n"), [Text.UTF8Encoding]::new($false))
    & $validator -RepositoryRoot $eolFixture
    Write-Host "windows-fixture-manifest-positive: ok (canonical-LF-hash-accepts-CRLF-checkout)"

    Invoke-ExpectedFailure -Name "missing-row" -MessagePattern "expected exactly 57 rows, found 56" -Mutation {
        param($fixture)
        $path = Resolve-IdealRepoPath -RepoRoot $fixture -Path $manifestRelativePath
        $rows = @(Import-Csv -LiteralPath $path | Where-Object row_id -ne "WCC-PLAN-LATE")
        $rows | Export-Csv -LiteralPath $path -NoTypeInformation -Encoding UTF8 -UseQuotes Always
    }
    Invoke-ExpectedFailure -Name "duplicate-row" -MessagePattern "duplicate matrix/row entry" -Mutation {
        param($fixture)
        $path = Resolve-IdealRepoPath -RepoRoot $fixture -Path $manifestRelativePath
        $rows = @(Import-Csv -LiteralPath $path)
        $rows[$rows.Count - 1] = $rows[0].PSObject.Copy()
        $rows | Export-Csv -LiteralPath $path -NoTypeInformation -Encoding UTF8 -UseQuotes Always
    }
    Invoke-ExpectedFailure -Name "pending-source-unowned" -MessagePattern "missing or unknown pending owner" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WNI-PLAN-DECLARE" -Mutation { param($row) $row.source_recipe_owner_bead = "n/a" }
    }
    Invoke-ExpectedFailure -Name "current-source-forged-hash" -MessagePattern "source_recipe_hash is forged or stale" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation { param($row) $row.source_recipe_hash = "sha256:" + ("0" * 64) }
    }
    Invoke-ExpectedFailure -Name "current-source-malformed-hash" -MessagePattern "current source_recipe_hash is malformed" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation { param($row) $row.source_recipe_hash = "sha256:abc" }
    }
    Invoke-ExpectedFailure -Name "pending-source-forged-hash" -MessagePattern "pending source recipe must use pending path and hash" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WNI-PLAN-DECLARE" -Mutation { param($row) $row.source_recipe_hash = "sha256:" + ("1" * 64) }
    }
    Invoke-ExpectedFailure -Name "mutable-identity" -MessagePattern "environment_id .* is mutable" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation { param($row) $row.environment_id = "win-x64-latest-v1" }
    }
    Invoke-ExpectedFailure -Name "non-x64-identity" -MessagePattern "environment_id .* is non-x64" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation { param($row) $row.environment_id = "win-x86-cert-v1" }
    }
    Invoke-ExpectedFailure -Name "office32" -MessagePattern "office_bitness must be 64 or n/a" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-CARRIER-EXCEL-ROUNDTRIP" -Mutation { param($row) $row.office_bitness = "32" }
    }
    Invoke-ExpectedFailure -Name "capability-credit" -MessagePattern "capability_credit must be none" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation { param($row) $row.capability_credit = "source-only" }
    }
    Invoke-ExpectedFailure -Name "noncanonical-source-paths" -MessagePattern "normalized, sorted, and deduplicated" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation {
            param($row)
            $paths = @([string]$row.source_recipe_paths -split '\|')
            [array]::Reverse($paths)
            $row.source_recipe_paths = $paths -join '|'
        }
    }
    Invoke-ExpectedFailure -Name "historical-binary-source" -MessagePattern "historical, generated, or mutable|binary/artifact" -Mutation {
        param($fixture)
        $relative = "docs/evidence/historical-fixture.dll"
        $absolute = Resolve-IdealRepoPath -RepoRoot $fixture -Path $relative
        [void](New-Item -ItemType Directory -Path (Split-Path -Parent $absolute) -Force)
        [IO.File]::WriteAllBytes($absolute, [byte[]](1, 2, 3, 4))
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation { param($row) $row.source_recipe_paths = $relative }
    }
    Invoke-ExpectedFailure -Name "pending-artifact-unowned" -MessagePattern "missing or unknown pending owner" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation { param($row) $row.built_artifact_owner_bead = "n/a" }
    }
    Invoke-ExpectedFailure -Name "pending-environment-unowned" -MessagePattern "missing or unknown pending owner" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation { param($row) $row.environment_owner_bead = "n/a" }
    }
    Invoke-ExpectedFailure -Name "blank-cleanup" -MessagePattern "has blank 'cleanup_recipe'" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation { param($row) $row.cleanup_recipe = "" }
    }

    Write-Host "test-windows-fixture-manifest: ok (positive=3 negative=15 rows=57 capability_credit=none)"
}
finally {
    if (Test-Path -LiteralPath $tempBase -PathType Container) {
        $resolved = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $tempBase).Path)
        if (-not $resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw "refusing to remove Windows fixture manifest temp directory outside system temp"
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
