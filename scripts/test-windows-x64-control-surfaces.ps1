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

$validator = Join-Path $PSScriptRoot "validate-windows-x64-control-surfaces.ps1"
$manifestRelativePath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json"
$manifest = Get-Content -LiteralPath (Join-Path $repoRoot $manifestRelativePath) -Raw | ConvertFrom-Json
$ownershipRelativePath = [string]$manifest.matrix_ownership
$windowsOwnership = @(Import-Csv -LiteralPath (Join-Path $repoRoot $ownershipRelativePath) | Where-Object { [string]$_.profile -eq "windows-x64" })

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

function New-WindowsControlFixture {
    $fixtureRoot = Join-Path $tempBase ([Guid]::NewGuid().ToString("N"))
    [void](New-Item -ItemType Directory -Path $fixtureRoot -Force)

    $paths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($path in @(".beads/issues.jsonl", $manifestRelativePath, $ownershipRelativePath)) {
        [void]$paths.Add($path.Replace('\', '/'))
    }
    foreach ($owner in $windowsOwnership) {
        $matrixPath = ([string]$owner.path).Replace('\', '/')
        [void]$paths.Add($matrixPath)
        foreach ($row in @(Import-Csv -LiteralPath (Join-Path $repoRoot $matrixPath))) {
            foreach ($token in @(([string]$row.authority_refs -split '[;|]') | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
                if ($token -notmatch '^[a-z][a-z0-9-]*:') {
                    [void]$paths.Add((($token -split '#', 2)[0]).Replace('\', '/'))
                }
            }
        }
    }
    foreach ($path in $paths) {
        Copy-FixtureFile -FixtureRoot $fixtureRoot -RelativePath $path
    }
    return $fixtureRoot
}

function Update-FixtureMatrixRow {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$MatrixPath,
        [Parameter(Mandatory = $true)][string]$RowId,
        [Parameter(Mandatory = $true)][scriptblock]$Mutation
    )

    $path = Resolve-IdealRepoPath -RepoRoot $FixtureRoot -Path $MatrixPath
    $rows = @(Import-Csv -LiteralPath $path)
    $matches = @($rows | Where-Object { [string]$_.row_id -eq $RowId })
    if ($matches.Count -ne 1) {
        throw "Windows control fixture expected one row '$RowId', found $($matches.Count)"
    }
    & $Mutation $matches[0]
    $rows | Export-Csv -LiteralPath $path -NoTypeInformation -Encoding UTF8
}

function Remove-FixtureMatrixRow {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$MatrixPath,
        [Parameter(Mandatory = $true)][string]$RowId
    )

    $path = Resolve-IdealRepoPath -RepoRoot $FixtureRoot -Path $MatrixPath
    $rows = @(Import-Csv -LiteralPath $path)
    $remaining = @($rows | Where-Object { [string]$_.row_id -ne $RowId })
    if ($remaining.Count -ne $rows.Count - 1) {
        throw "Windows control fixture failed to remove exactly one row '$RowId'"
    }
    $remaining | Export-Csv -LiteralPath $path -NoTypeInformation -Encoding UTF8
}

function Invoke-ExpectedFailure {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Mutation,
        [Parameter(Mandatory = $true)][string]$MessagePattern
    )

    $fixture = New-WindowsControlFixture
    & $Mutation $fixture
    $failedAsExpected = $false
    try {
        & $validator -RepositoryRoot $fixture *> $null
    }
    catch {
        if ($_.Exception.Message -notmatch $MessagePattern) {
            throw "Windows control negative case '$Name' failed for the wrong reason: $($_.Exception.Message)"
        }
        $failedAsExpected = $true
    }
    if (-not $failedAsExpected) {
        throw "Windows control negative case '$Name' unexpectedly passed"
    }
    Write-Host "windows-x64-control-negative: ok ($Name)"
}

$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
$tempBase = Join-Path $tempRoot ("oxvba-windows-x64-control-" + [Guid]::NewGuid().ToString("N"))
$resolvedTempBase = [IO.Path]::GetFullPath($tempBase)
if (-not $resolvedTempBase.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Windows control test temp root escaped the system temp directory"
}
[void](New-Item -ItemType Directory -Path $tempBase -Force)

try {
    $baseline = New-WindowsControlFixture
    & $validator -RepositoryRoot $baseline

    $clientMatrix = "docs/validation/WINDOWS_JIT_COM_CLIENT_MATRIX_V1.csv"
    $exportMatrix = "docs/validation/WINDOWS_NATIVE_EXPORT_AND_PACKAGING_MATRIX_V1.csv"

    $x64AliasFixture = New-WindowsControlFixture
    Update-FixtureMatrixRow -FixtureRoot $x64AliasFixture -MatrixPath $clientMatrix -RowId "WCC-PLAN-LATE" -Mutation {
        param($row)
        $row.notes = "$($row.notes); artifact triples x86_64-pc-windows-msvc and x86-64-pc-windows-msvc"
    }
    & $validator -RepositoryRoot $x64AliasFixture
    Write-Host "windows-x64-control-positive: ok (x86_64-and-x86-64-aliases)"

    Invoke-ExpectedFailure -Name "missing-required-row" -MessagePattern "expected 9 rows, found 8" -Mutation {
        param($fixture)
        Remove-FixtureMatrixRow -FixtureRoot $fixture -MatrixPath $clientMatrix -RowId "WCC-PLAN-LATE"
    }
    Invoke-ExpectedFailure -Name "x86-target" -MessagePattern "target_arch must be x64" -Mutation {
        param($fixture)
        Update-FixtureMatrixRow -FixtureRoot $fixture -MatrixPath $clientMatrix -RowId "WCC-PLAN-LATE" -Mutation { param($row) $row.target_arch = "x86" }
    }
    Invoke-ExpectedFailure -Name "office32" -MessagePattern "office_bitness must be 64" -Mutation {
        param($fixture)
        Update-FixtureMatrixRow -FixtureRoot $fixture -MatrixPath $clientMatrix -RowId "WCC-PLAN-LATE" -Mutation { param($row) $row.office_bitness = "32" }
    }
    Invoke-ExpectedFailure -Name "wow64-process" -MessagePattern "unrecognized x64 process_shape" -Mutation {
        param($fixture)
        Update-FixtureMatrixRow -FixtureRoot $fixture -MatrixPath $clientMatrix -RowId "WCC-PLAN-LATE" -Mutation { param($row) $row.process_shape = "WOW64-client" }
    }
    Invoke-ExpectedFailure -Name "process32-variant" -MessagePattern "unrecognized x64 process_shape" -Mutation {
        param($fixture)
        Update-FixtureMatrixRow -FixtureRoot $fixture -MatrixPath $clientMatrix -RowId "WCC-PLAN-LATE" -Mutation { param($row) $row.process_shape = "32-bit-process" }
    }
    Invoke-ExpectedFailure -Name "office32-order-variant" -MessagePattern "excluded non-x64 target" -Mutation {
        param($fixture)
        Update-FixtureMatrixRow -FixtureRoot $fixture -MatrixPath $clientMatrix -RowId "WCC-PLAN-LATE" -Mutation { param($row) $row.notes = "$($row.notes); Office-32bit artifact" }
    }
    Invoke-ExpectedFailure -Name "arm64ec-aarch64-artifact" -MessagePattern "excluded non-x64 target" -Mutation {
        param($fixture)
        Update-FixtureMatrixRow -FixtureRoot $fixture -MatrixPath $clientMatrix -RowId "WCC-PLAN-LATE" -Mutation { param($row) $row.notes = "$($row.notes); ARM64EC aarch64 artifact" }
    }
    Invoke-ExpectedFailure -Name "evidence-owner-drift" -MessagePattern "evidence_owner_bead must be" -Mutation {
        param($fixture)
        Update-FixtureMatrixRow -FixtureRoot $fixture -MatrixPath $clientMatrix -RowId "WCC-PLAN-LATE" -Mutation { param($row) $row.evidence_owner_bead = "bd-59co.3.1.2" }
    }
    Invoke-ExpectedFailure -Name "residual-owner-drift" -MessagePattern "residual_owner_bead must be" -Mutation {
        param($fixture)
        Update-FixtureMatrixRow -FixtureRoot $fixture -MatrixPath $clientMatrix -RowId "WCC-PLAN-LATE" -Mutation { param($row) $row.residual_owner_bead = "bd-59co.3.1.2" }
    }
    Invoke-ExpectedFailure -Name "authority-route-drift" -MessagePattern "authority route differs" -Mutation {
        param($fixture)
        Update-FixtureMatrixRow -FixtureRoot $fixture -MatrixPath $clientMatrix -RowId "WCC-PLAN-LATE" -Mutation { param($row) $row.authority_refs = "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md|docs/spec/OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md" }
    }
    Invoke-ExpectedFailure -Name "claim-gate-collapse" -MessagePattern "standalone native-output terminal contract clauses differs" -Mutation {
        param($fixture)
        Update-FixtureMatrixRow -FixtureRoot $fixture -MatrixPath $exportMatrix -RowId "WNE-PROFILE-TOOL-TERMINAL" -Mutation { param($row) $row.contract_clauses = "PROFILE-WIN-001" }
    }
    Invoke-ExpectedFailure -Name "wrapper-native-class-collapse" -MessagePattern "changed wrapper/native class or backend" -Mutation {
        param($fixture)
        Update-FixtureMatrixRow -FixtureRoot $fixture -MatrixPath $exportMatrix -RowId "WNE-PLAN-NATIVE" -Mutation { param($row) $row.output_class = "WrapperLibrary" }
    }

    Write-Host "test-windows-x64-control-surfaces: ok (positive_aliases=2 negative_cases=12)"
}
finally {
    if (Test-Path -LiteralPath $tempBase -PathType Container) {
        $resolved = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $tempBase).Path)
        if (-not $resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw "refusing to remove Windows control temp directory outside system temp"
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
