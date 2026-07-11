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
$toolchainAssetsPath = Join-Path $PSScriptRoot "testdata/windows-fixture-toolchain/assets-v1.json"
$toolchainAssets = Get-Content -LiteralPath $toolchainAssetsPath -Raw | ConvertFrom-Json
if ([string]$toolchainAssets.schema_id -ne "oxvba-windows-fixture-admission-test-assets-v1") {
    throw "Windows fixture manifest test asset manifest has the wrong schema"
}
foreach ($source in @($toolchainAssets.source_files)) {
    $sourceName = [string]$source.path
    $sourcePath = Join-Path (Split-Path -Parent $toolchainAssetsPath) $sourceName
    if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
        throw "Windows fixture manifest test asset source '$sourceName' is missing"
    }
    $sourceHash = (Get-FileHash -LiteralPath $sourcePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($sourceHash -cne [string]$source.sha256) {
        throw "Windows fixture manifest test asset source '$sourceName' differs from recorded provenance"
    }
}

function Get-TestToolchainAssetBytes {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet("pe-dll-x64", "pe-exe-x64", "msft-tlb-v1")]
        [string]$Kind
    )

    $matches = @($toolchainAssets.assets.PSObject.Properties | Where-Object Name -ceq $Kind)
    if ($matches.Count -ne 1) {
        throw "Windows fixture manifest test has no unique toolchain asset '$Kind'"
    }
    $asset = $matches[0].Value
    try {
        [byte[]]$bytes = [Convert]::FromBase64String([string]$asset.base64)
    }
    catch {
        throw "Windows fixture manifest test asset '$Kind' is not valid base64"
    }
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $hash = [Convert]::ToHexString($sha.ComputeHash($bytes)).ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
    }
    if ($bytes.Length -ne [int]$asset.length -or $hash -cne [string]$asset.sha256) {
        throw "Windows fixture manifest test asset '$Kind' length/hash differs from controlled provenance"
    }
    return ,$bytes
}

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

function Update-MatrixRow {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$MatrixId,
        [Parameter(Mandatory = $true)][string]$RowId,
        [Parameter(Mandatory = $true)][scriptblock]$Mutation
    )

    if (-not $matrixContracts.Contains($MatrixId)) {
        throw "Windows fixture manifest test does not know matrix '$MatrixId'"
    }
    $path = Resolve-IdealRepoPath -RepoRoot $FixtureRoot -Path ([string]$matrixContracts[$MatrixId])
    $rows = @(Import-Csv -LiteralPath $path)
    $matches = @($rows | Where-Object { [string]$_.row_id -eq $RowId })
    if ($matches.Count -ne 1) {
        throw "Windows fixture manifest test expected one matrix row '$MatrixId|$RowId', found $($matches.Count)"
    }
    & $Mutation $matches[0]
    $rows | Export-Csv -LiteralPath $path -NoTypeInformation -Encoding UTF8 -UseQuotes Always
}

function Get-ManifestRow {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$RowId
    )

    $path = Resolve-IdealRepoPath -RepoRoot $FixtureRoot -Path $manifestRelativePath
    $matches = @(Import-Csv -LiteralPath $path | Where-Object { [string]$_.row_id -eq $RowId })
    if ($matches.Count -ne 1) {
        throw "Windows fixture manifest test expected one row '$RowId', found $($matches.Count)"
    }
    return $matches[0]
}

function Write-TestBytes {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath,
        [Parameter(Mandatory = $true)][byte[]]$Bytes
    )

    $absolute = Resolve-IdealRepoPath -RepoRoot $FixtureRoot -Path $RelativePath
    $parent = Split-Path -Parent $absolute
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        [void](New-Item -ItemType Directory -Path $parent -Force)
    }
    [IO.File]::WriteAllBytes($absolute, $Bytes)
    return $absolute
}

function Write-TestUtf8 {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text
    )

    $bytes = [Text.UTF8Encoding]::new($false).GetBytes($Text)
    return Write-TestBytes -FixtureRoot $FixtureRoot -RelativePath $RelativePath -Bytes $bytes
}

function Write-TestJson {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath,
        [Parameter(Mandatory = $true)]$Value
    )

    $text = $Value | ConvertTo-Json -Depth 8
    return Write-TestUtf8 -FixtureRoot $FixtureRoot -RelativePath $RelativePath -Text ($text + "`n")
}

function Replace-TestUtf8Text {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath,
        [Parameter(Mandatory = $true)][string]$OldValue,
        [Parameter(Mandatory = $true)][string]$NewValue
    )

    $absolute = Resolve-IdealRepoPath -RepoRoot $FixtureRoot -Path $RelativePath
    $text = [IO.File]::ReadAllText($absolute, [Text.UTF8Encoding]::new($false, $true))
    $first = $text.IndexOf($OldValue, [StringComparison]::Ordinal)
    $last = $text.LastIndexOf($OldValue, [StringComparison]::Ordinal)
    if ($first -lt 0 -or $first -ne $last) {
        throw "Windows fixture manifest test replacement expected one exact token '$OldValue'"
    }
    [void](Write-TestUtf8 -FixtureRoot $FixtureRoot -RelativePath $RelativePath -Text $text.Replace($OldValue, $NewValue))
}

function Set-TestUInt16 {
    param(
        [Parameter(Mandatory = $true)][byte[]]$Bytes,
        [Parameter(Mandatory = $true)][int]$Offset,
        [Parameter(Mandatory = $true)][uint16]$Value
    )

    [BitConverter]::GetBytes($Value).CopyTo($Bytes, $Offset)
}

function Set-TestUInt32 {
    param(
        [Parameter(Mandatory = $true)][byte[]]$Bytes,
        [Parameter(Mandatory = $true)][int]$Offset,
        [Parameter(Mandatory = $true)][uint32]$Value
    )

    [BitConverter]::GetBytes($Value).CopyTo($Bytes, $Offset)
}

function New-SyntheticTestPeHeader {
    param(
        [uint16]$Machine = 0x8664,
        [bool]$Dll = $true,
        [uint16]$OptionalMagic = 0x020B
    )

    $bytes = [byte[]]::new(1024)
    $bytes[0] = 0x4D
    $bytes[1] = 0x5A
    $peOffset = 0x80
    Set-TestUInt32 -Bytes $bytes -Offset 0x3C -Value $peOffset
    $bytes[$peOffset] = 0x50
    $bytes[$peOffset + 1] = 0x45
    Set-TestUInt16 -Bytes $bytes -Offset ($peOffset + 4) -Value $Machine
    Set-TestUInt16 -Bytes $bytes -Offset ($peOffset + 6) -Value 1
    Set-TestUInt16 -Bytes $bytes -Offset ($peOffset + 20) -Value 0x00F0
    $characteristics = [uint16]0x0002
    if ($Dll) {
        $characteristics = [uint16]($characteristics -bor 0x2000)
    }
    Set-TestUInt16 -Bytes $bytes -Offset ($peOffset + 22) -Value $characteristics
    $optionalOffset = $peOffset + 24
    Set-TestUInt16 -Bytes $bytes -Offset $optionalOffset -Value $OptionalMagic
    Set-TestUInt32 -Bytes $bytes -Offset ($optionalOffset + 56) -Value 0x2000
    Set-TestUInt32 -Bytes $bytes -Offset ($optionalOffset + 60) -Value 0x200
    $sectionOffset = $optionalOffset + 0xF0
    [Text.Encoding]::ASCII.GetBytes(".text") | ForEach-Object -Begin { $index = 0 } -Process {
        $bytes[$sectionOffset + $index] = $_
        $index++
    }
    Set-TestUInt32 -Bytes $bytes -Offset ($sectionOffset + 8) -Value 0x100
    Set-TestUInt32 -Bytes $bytes -Offset ($sectionOffset + 12) -Value 0x1000
    Set-TestUInt32 -Bytes $bytes -Offset ($sectionOffset + 16) -Value 0x200
    Set-TestUInt32 -Bytes $bytes -Offset ($sectionOffset + 20) -Value 0x200
    Set-TestUInt32 -Bytes $bytes -Offset ($sectionOffset + 36) -Value 0x60000020
    $bytes[0x200] = 0xC3
    return $bytes
}

function Set-ManifestBuiltArtifactCurrent {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$RowId,
        [Parameter(Mandatory = $true)][string]$ArtifactPath
    )

    $manifestRow = Get-ManifestRow -FixtureRoot $FixtureRoot -RowId $RowId
    $matrixId = [string]$manifestRow.matrix_id
    $hash = Get-WindowsFixtureRawFileHash -RepositoryRoot $FixtureRoot -RelativePath $ArtifactPath
    Update-MatrixRow -FixtureRoot $FixtureRoot -MatrixId $matrixId -RowId $RowId -Mutation {
        param($row)
        $row.fixture_hash = $hash
    }
    Update-ManifestRow -FixtureRoot $FixtureRoot -RowId $RowId -Mutation {
        param($row)
        $row.built_artifact_state = "current"
        $row.built_artifact_path = $ArtifactPath
        $row.built_artifact_hash = $hash
        $row.built_artifact_owner_bead = "n/a"
    }
}

function Set-ManifestEnvironmentCurrent {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$RowId,
        [Parameter(Mandatory = $true)][string]$CapturePath
    )

    $manifestRow = Get-ManifestRow -FixtureRoot $FixtureRoot -RowId $RowId
    $matrixId = [string]$manifestRow.matrix_id
    $hash = Get-WindowsFixtureCanonicalSourceFileHash -RepositoryRoot $FixtureRoot -RelativePath $CapturePath
    Update-MatrixRow -FixtureRoot $FixtureRoot -MatrixId $matrixId -RowId $RowId -Mutation {
        param($row)
        $row.environment_hash = $hash
    }
    Update-ManifestRow -FixtureRoot $FixtureRoot -RowId $RowId -Mutation {
        param($row)
        $row.environment_state = "current"
        $row.environment_capture_path = $CapturePath
        $row.environment_hash = $hash
        $row.environment_owner_bead = "n/a"
    }
}

function New-TestEnvironmentCapture {
    param(
        [Parameter(Mandatory = $true)]$Environment
    )

    $isCertification = [string]$Environment.role -eq "certification-vm"
    return [pscustomobject][ordered]@{
        schema_id = "oxvba-windows-x64-environment-capture-v1"
        schema_version = 1
        capture_id = "$([string]$Environment.environment_id)-capture-v1"
        environment_id = [string]$Environment.environment_id
        role = [string]$Environment.role
        profile = [string]$Environment.profile
        target_arch = [string]$Environment.target_arch
        os_build = [string]$Environment.os_build
        office_product = [string]$Environment.office_product
        office_version = [string]$Environment.office_version
        office_build = [string]$Environment.office_build
        office_channel = [string]$Environment.office_channel
        office_bitness = [string]$Environment.office_bitness
        locale = [string]$Environment.locale
        snapshot_or_image = [string]$Environment.snapshot_or_image
        reset_policy = [string]$Environment.reset_policy
        reset_policy_hash = Get-WindowsFixtureResetPolicyHash -ResetPolicy ([string]$Environment.reset_policy)
        evidence_state = [string]$Environment.evidence_state
        certification_authority = $isCertification
        noncertifying = -not $isCertification
    }
}

function New-TestBundleArtifact {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)]$ManifestRow
    )

    $artifactRoot = [string]$ManifestRow.built_artifact_root
    $componentKinds = @([string]$ManifestRow.built_artifact_components -split '\|')
    $components = [Collections.Generic.List[object]]::new()
    for ($index = 0; $index -lt $componentKinds.Count; $index++) {
        $kind = $componentKinds[$index]
        $ordinal = $index + 1
        $componentId = switch ($kind) {
            "pe-dll-x64" { "server-$ordinal-v1" }
            "pe-exe-x64" { "host-$ordinal-v1" }
            "msft-tlb-v1" { "typelib-$ordinal-v1" }
            "vba-source-utf8-v1" { "client-$ordinal-v1" }
            default { throw "Windows fixture manifest test cannot materialize bundle kind '$kind'" }
        }
        $extension = switch ($kind) {
            "pe-dll-x64" { ".dll" }
            "pe-exe-x64" { ".exe" }
            "msft-tlb-v1" { ".tlb" }
            "vba-source-utf8-v1" { ".bas" }
        }
        $componentRelative = "components/$componentId$extension"
        $componentRepoPath = "$artifactRoot/$componentRelative"
        switch ($kind) {
            "pe-dll-x64" {
                [byte[]]$peBytes = Get-TestToolchainAssetBytes -Kind "pe-dll-x64"
                [void](Write-TestBytes -FixtureRoot $FixtureRoot -RelativePath $componentRepoPath -Bytes $peBytes)
            }
            "pe-exe-x64" {
                [byte[]]$peBytes = Get-TestToolchainAssetBytes -Kind "pe-exe-x64"
                [void](Write-TestBytes -FixtureRoot $FixtureRoot -RelativePath $componentRepoPath -Bytes $peBytes)
            }
            "msft-tlb-v1" {
                [byte[]]$typeLibBytes = Get-TestToolchainAssetBytes -Kind "msft-tlb-v1"
                [void](Write-TestBytes -FixtureRoot $FixtureRoot -RelativePath $componentRepoPath -Bytes $typeLibBytes)
            }
            "vba-source-utf8-v1" {
                [void](Write-TestUtf8 -FixtureRoot $FixtureRoot -RelativePath $componentRepoPath -Text "Attribute VB_Name = `"FixtureClient`"`nOption Explicit`n")
            }
        }
        $components.Add([pscustomobject][ordered]@{
            component_id = $componentId
            kind = $kind
            relative_path = $componentRelative
            sha256 = Get-WindowsFixtureRawFileHash -RepositoryRoot $FixtureRoot -RelativePath $componentRepoPath
        })
    }

    $bundle = [pscustomobject][ordered]@{
        schema_id = "oxvba-windows-x64-fixture-bundle-v1"
        schema_version = 1
        matrix_id = [string]$ManifestRow.matrix_id
        row_id = [string]$ManifestRow.row_id
        fixture_id = [string]$ManifestRow.fixture_id
        artifact_id = [string]$ManifestRow.built_artifact_id
        target_arch = "x64"
        artifact_class = "fixture-bundle-json-v1"
        components = @($components)
    }
    $bundlePath = "$artifactRoot/$([string]$ManifestRow.built_artifact_name)"
    [void](Write-TestJson -FixtureRoot $FixtureRoot -RelativePath $bundlePath -Value $bundle)
    return $bundlePath
}

function Set-TestBundleComponentBytes {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$BundlePath,
        [Parameter(Mandatory = $true)][string]$Kind,
        [Parameter(Mandatory = $true)][byte[]]$Bytes
    )

    $bundleAbsolute = Resolve-IdealRepoPath -RepoRoot $FixtureRoot -Path $BundlePath
    $bundle = Get-Content -LiteralPath $bundleAbsolute -Raw | ConvertFrom-Json
    $components = @($bundle.components | Where-Object { [string]$_.kind -ceq $Kind })
    if ($components.Count -ne 1) {
        throw "Windows fixture manifest test expected one '$Kind' component in '$BundlePath'"
    }
    $artifactRoot = Split-Path -Parent $BundlePath
    $componentPath = "$artifactRoot/$([string]$components[0].relative_path)".Replace('\', '/')
    [void](Write-TestBytes -FixtureRoot $FixtureRoot -RelativePath $componentPath -Bytes $Bytes)
    $components[0].sha256 = Get-WindowsFixtureRawFileHash -RepositoryRoot $FixtureRoot -RelativePath $componentPath
    [void](Write-TestJson -FixtureRoot $FixtureRoot -RelativePath $BundlePath -Value $bundle)
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
$windowsLoaderNegativeCount = if (
    [Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([Runtime.InteropServices.OSPlatform]::Windows)
) { 1 } else { 0 }
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

    $dllFixture = New-WindowsFixtureManifestTestRoot
    $dllRow = Get-ManifestRow -FixtureRoot $dllFixture -RowId "WCC-PLAN-LATE"
    $dllPath = "$([string]$dllRow.built_artifact_root)/$([string]$dllRow.built_artifact_name)"
    [byte[]]$dllBytes = Get-TestToolchainAssetBytes -Kind "pe-dll-x64"
    [void](Write-TestBytes -FixtureRoot $dllFixture -RelativePath $dllPath -Bytes $dllBytes)
    Set-ManifestBuiltArtifactCurrent -FixtureRoot $dllFixture -RowId "WCC-PLAN-LATE" -ArtifactPath $dllPath
    & $validator -RepositoryRoot $dllFixture
    Write-Host "windows-fixture-manifest-positive: ok (current-controlled-pe32plus-amd64-dll)"

    $exeFixture = New-WindowsFixtureManifestTestRoot
    $exeRow = Get-ManifestRow -FixtureRoot $exeFixture -RowId "WAC-BSTR-LAYOUT"
    $exePath = "$([string]$exeRow.built_artifact_root)/$([string]$exeRow.built_artifact_name)"
    [byte[]]$exeBytes = Get-TestToolchainAssetBytes -Kind "pe-exe-x64"
    [void](Write-TestBytes -FixtureRoot $exeFixture -RelativePath $exePath -Bytes $exeBytes)
    Set-ManifestBuiltArtifactCurrent -FixtureRoot $exeFixture -RowId "WAC-BSTR-LAYOUT" -ArtifactPath $exePath
    & $validator -RepositoryRoot $exeFixture
    Write-Host "windows-fixture-manifest-positive: ok (current-controlled-pe32plus-amd64-exe)"

    $bundleFixture = New-WindowsFixtureManifestTestRoot
    $bundleRow = Get-ManifestRow -FixtureRoot $bundleFixture -RowId "WCC-EXCEL-AUTHORITY"
    $bundlePath = New-TestBundleArtifact -FixtureRoot $bundleFixture -ManifestRow $bundleRow
    Set-ManifestBuiltArtifactCurrent -FixtureRoot $bundleFixture -RowId "WCC-EXCEL-AUTHORITY" -ArtifactPath $bundlePath
    & $validator -RepositoryRoot $bundleFixture
    Write-Host "windows-fixture-manifest-positive: ok (current-controlled-exact-bundle-schema)"

    $typeLibFixture = New-WindowsFixtureManifestTestRoot
    $typeLibRow = Get-ManifestRow -FixtureRoot $typeLibFixture -RowId "WAC-TYPELIB-METADATA"
    $typeLibBundlePath = New-TestBundleArtifact -FixtureRoot $typeLibFixture -ManifestRow $typeLibRow
    Set-ManifestBuiltArtifactCurrent -FixtureRoot $typeLibFixture -RowId "WAC-TYPELIB-METADATA" -ArtifactPath $typeLibBundlePath
    & $validator -RepositoryRoot $typeLibFixture
    Write-Host "windows-fixture-manifest-positive: ok (current-toolchain-generated-msft-typelib-bundle)"

    $environmentFixture = New-WindowsFixtureManifestTestRoot
    $environmentPath = Resolve-IdealRepoPath -RepoRoot $environmentFixture -Path $environmentRelativePath
    $environmentRows = @(Import-Csv -LiteralPath $environmentPath)
    $devEnvironments = @($environmentRows | Where-Object { [string]$_.environment_id -eq "win-x64-dev-oracle-2026-07" })
    if ($devEnvironments.Count -ne 1) {
        throw "Windows fixture manifest test expected one development oracle environment"
    }
    $devEnvironments[0].snapshot_or_image = "dev-oracle-2026-07@sha256:" + ("a" * 64)
    $environmentRows | Export-Csv -LiteralPath $environmentPath -NoTypeInformation -Encoding UTF8 -UseQuotes Always
    $environmentRow = Get-ManifestRow -FixtureRoot $environmentFixture -RowId "WAC-BSTR-LAYOUT"
    $capturePath = "$([string]$environmentRow.environment_capture_root)/$([string]$environmentRow.environment_capture_name)"
    $capture = New-TestEnvironmentCapture -Environment $devEnvironments[0]
    [void](Write-TestJson -FixtureRoot $environmentFixture -RelativePath $capturePath -Value $capture)
    Set-ManifestEnvironmentCurrent -FixtureRoot $environmentFixture -RowId "WAC-BSTR-LAYOUT" -CapturePath $capturePath
    & $validator -RepositoryRoot $environmentFixture
    Write-Host "windows-fixture-manifest-positive: ok (current-versioned-environment-capture-bound-to-canonical-row)"

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
    Invoke-ExpectedFailure -Name "artifact-source-text-masquerade" -MessagePattern "structurally valid bounded PE image" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-PLAN-LATE"
        $path = "$([string]$row.built_artifact_root)/$([string]$row.built_artifact_name)"
        $sourceText = "Attribute VB_Name = `"NotABinary`"`n" + (("' controlled source text masquerade`n") * 40)
        [void](Write-TestUtf8 -FixtureRoot $fixture -RelativePath $path -Text $sourceText)
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-PLAN-LATE" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-synthetic-header-blob" -MessagePattern "file/section alignments are invalid|structurally valid bounded PE image" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-PLAN-LATE"
        $path = "$([string]$row.built_artifact_root)/$([string]$row.built_artifact_name)"
        [byte[]]$bytes = New-SyntheticTestPeHeader -Dll $true
        [void](Write-TestBytes -FixtureRoot $fixture -RelativePath $path -Bytes $bytes)
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-PLAN-LATE" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-mutable-alias" -MessagePattern "uses a mutable alias" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-PLAN-LATE"
        $path = "$([string]$row.built_artifact_root)/latest.dll"
        [byte[]]$bytes = Get-TestToolchainAssetBytes -Kind "pe-dll-x64"
        [void](Write-TestBytes -FixtureRoot $fixture -RelativePath $path -Bytes $bytes)
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-PLAN-LATE" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-historical-generated-path" -MessagePattern "historical or generated" -Mutation {
        param($fixture)
        $path = "docs/evidence/historical-fixture.dll"
        [byte[]]$bytes = Get-TestToolchainAssetBytes -Kind "pe-dll-x64"
        [void](Write-TestBytes -FixtureRoot $fixture -RelativePath $path -Bytes $bytes)
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-PLAN-LATE" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-path-escape" -MessagePattern "repository-relative path" -Mutation {
        param($fixture)
        $manifestRow = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-PLAN-LATE"
        $hash = "sha256:" + ("2" * 64)
        Update-MatrixRow -FixtureRoot $fixture -MatrixId ([string]$manifestRow.matrix_id) -RowId "WCC-PLAN-LATE" -Mutation {
            param($row)
            $row.fixture_hash = $hash
        }
        Update-ManifestRow -FixtureRoot $fixture -RowId "WCC-PLAN-LATE" -Mutation {
            param($row)
            $row.built_artifact_state = "current"
            $row.built_artifact_path = "$([string]$row.built_artifact_root)/../escape.dll"
            $row.built_artifact_hash = $hash
            $row.built_artifact_owner_bead = "n/a"
        }
    }
    Invoke-ExpectedFailure -Name "artifact-x86-pe" -MessagePattern "PE machine must be AMD64/x64" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-PLAN-LATE"
        $path = "$([string]$row.built_artifact_root)/$([string]$row.built_artifact_name)"
        [byte[]]$bytes = Get-TestToolchainAssetBytes -Kind "pe-dll-x64"
        $peOffset = [int][BitConverter]::ToUInt32($bytes, 0x3C)
        Set-TestUInt16 -Bytes $bytes -Offset ($peOffset + 4) -Value 0x014C
        [void](Write-TestBytes -FixtureRoot $fixture -RelativePath $path -Bytes $bytes)
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-PLAN-LATE" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-wrong-pe-class" -MessagePattern "DLL/EXE characteristics do not match" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-PLAN-LATE"
        $path = "$([string]$row.built_artifact_root)/$([string]$row.built_artifact_name)"
        [byte[]]$bytes = Get-TestToolchainAssetBytes -Kind "pe-exe-x64"
        [void](Write-TestBytes -FixtureRoot $fixture -RelativePath $path -Bytes $bytes)
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-PLAN-LATE" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-truncated-pe" -MessagePattern "raw data|structurally valid bounded PE image" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-PLAN-LATE"
        $path = "$([string]$row.built_artifact_root)/$([string]$row.built_artifact_name)"
        [byte[]]$bytes = Get-TestToolchainAssetBytes -Kind "pe-dll-x64"
        [byte[]]$truncated = $bytes[0..1199]
        [void](Write-TestBytes -FixtureRoot $fixture -RelativePath $path -Bytes $truncated)
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-PLAN-LATE" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-invalid-pe-alignment" -MessagePattern "file/section alignments are invalid" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-PLAN-LATE"
        $path = "$([string]$row.built_artifact_root)/$([string]$row.built_artifact_name)"
        [byte[]]$bytes = Get-TestToolchainAssetBytes -Kind "pe-dll-x64"
        $peOffset = [int][BitConverter]::ToUInt32($bytes, 0x3C)
        Set-TestUInt32 -Bytes $bytes -Offset ($peOffset + 24 + 36) -Value 0x300
        [void](Write-TestBytes -FixtureRoot $fixture -RelativePath $path -Bytes $bytes)
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-PLAN-LATE" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-overlapping-pe-sections" -MessagePattern "overlap" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-PLAN-LATE"
        $path = "$([string]$row.built_artifact_root)/$([string]$row.built_artifact_name)"
        [byte[]]$bytes = Get-TestToolchainAssetBytes -Kind "pe-dll-x64"
        $peOffset = [int][BitConverter]::ToUInt32($bytes, 0x3C)
        $optionalSize = [int][BitConverter]::ToUInt16($bytes, $peOffset + 20)
        $sectionTable = $peOffset + 24 + $optionalSize
        $firstRawPointer = [BitConverter]::ToUInt32($bytes, $sectionTable + 20)
        Set-TestUInt32 -Bytes $bytes -Offset ($sectionTable + 40 + 20) -Value $firstRawPointer
        [void](Write-TestBytes -FixtureRoot $fixture -RelativePath $path -Bytes $bytes)
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-PLAN-LATE" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-corrupt-pe-image-size" -MessagePattern "escapes SizeOfImage|aggregate section sizes" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-PLAN-LATE"
        $path = "$([string]$row.built_artifact_root)/$([string]$row.built_artifact_name)"
        [byte[]]$bytes = Get-TestToolchainAssetBytes -Kind "pe-dll-x64"
        $peOffset = [int][BitConverter]::ToUInt32($bytes, 0x3C)
        Set-TestUInt32 -Bytes $bytes -Offset ($peOffset + 24 + 56) -Value 0x2000
        [void](Write-TestBytes -FixtureRoot $fixture -RelativePath $path -Bytes $bytes)
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-PLAN-LATE" -ArtifactPath $path
    }
    if ([Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([Runtime.InteropServices.OSPlatform]::Windows)) {
        Invoke-ExpectedFailure -Name "artifact-structural-but-windows-unloadable" -MessagePattern "Windows LoadLibraryExW.*rejected" -Mutation {
            param($fixture)
            $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-PLAN-LATE"
            $path = "$([string]$row.built_artifact_root)/$([string]$row.built_artifact_name)"
            [byte[]]$bytes = Get-TestToolchainAssetBytes -Kind "pe-dll-x64"
            $peOffset = [int][BitConverter]::ToUInt32($bytes, 0x3C)
            $flagsOffset = $peOffset + 24 + 70
            $flags = [BitConverter]::ToUInt16($bytes, $flagsOffset)
            Set-TestUInt16 -Bytes $bytes -Offset $flagsOffset -Value ([uint16]($flags -bor 0x0080))
            [void](Write-TestBytes -FixtureRoot $fixture -RelativePath $path -Bytes $bytes)
            Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-PLAN-LATE" -ArtifactPath $path
        }
    }
    else {
        Write-Host "windows-fixture-manifest-negative: skipped (artifact-structural-but-windows-unloadable; Windows-only loader gate)"
    }
    Invoke-ExpectedFailure -Name "artifact-bundle-wrong-schema" -MessagePattern "exact case-sensitive schema" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-EXCEL-AUTHORITY"
        $path = "$([string]$row.built_artifact_root)/$([string]$row.built_artifact_name)"
        [void](Write-TestJson -FixtureRoot $fixture -RelativePath $path -Value ([pscustomobject]@{ schema_id = "arbitrary-text-container" }))
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-EXCEL-AUTHORITY" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-bundle-duplicate-root-field" -MessagePattern "duplicate JSON property 'schema_id'" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-EXCEL-AUTHORITY"
        $path = New-TestBundleArtifact -FixtureRoot $fixture -ManifestRow $row
        Replace-TestUtf8Text `
            -FixtureRoot $fixture `
            -RelativePath $path `
            -OldValue '  "schema_id": "oxvba-windows-x64-fixture-bundle-v1",' `
            -NewValue "  `"schema_id`": `"attacker-controlled`",`n  `"schema_id`": `"oxvba-windows-x64-fixture-bundle-v1`","
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-EXCEL-AUTHORITY" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-bundle-miscased-root-field" -MessagePattern "exact case-sensitive schema" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-EXCEL-AUTHORITY"
        $path = New-TestBundleArtifact -FixtureRoot $fixture -ManifestRow $row
        Replace-TestUtf8Text -FixtureRoot $fixture -RelativePath $path -OldValue '"schema_id"' -NewValue '"Schema_id"'
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-EXCEL-AUTHORITY" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-bundle-duplicate-component-field" -MessagePattern "duplicate JSON property 'component_id'" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-EXCEL-AUTHORITY"
        $path = New-TestBundleArtifact -FixtureRoot $fixture -ManifestRow $row
        Replace-TestUtf8Text `
            -FixtureRoot $fixture `
            -RelativePath $path `
            -OldValue '      "component_id": "server-1-v1",' `
            -NewValue "      `"component_id`": `"attacker-v1`",`n      `"component_id`": `"server-1-v1`","
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-EXCEL-AUTHORITY" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-bundle-miscased-component-field" -MessagePattern "exact case-sensitive schema" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WCC-EXCEL-AUTHORITY"
        $path = New-TestBundleArtifact -FixtureRoot $fixture -ManifestRow $row
        Replace-TestUtf8Text -FixtureRoot $fixture -RelativePath $path -OldValue '"component_id": "server-1-v1"' -NewValue '"Component_id": "server-1-v1"'
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WCC-EXCEL-AUTHORITY" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-typelib-eight-byte-stub" -MessagePattern "length is outside" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-TYPELIB-METADATA"
        $path = New-TestBundleArtifact -FixtureRoot $fixture -ManifestRow $row
        Set-TestBundleComponentBytes -FixtureRoot $fixture -BundlePath $path -Kind "msft-tlb-v1" -Bytes ([byte[]](0x4D, 0x53, 0x46, 0x54, 2, 0, 1, 0))
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WAC-TYPELIB-METADATA" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-typelib-truncated" -MessagePattern "segment directory|out of bounds|missing" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-TYPELIB-METADATA"
        $path = New-TestBundleArtifact -FixtureRoot $fixture -ManifestRow $row
        [byte[]]$bytes = Get-TestToolchainAssetBytes -Kind "msft-tlb-v1"
        [byte[]]$truncated = $bytes[0..1199]
        Set-TestBundleComponentBytes -FixtureRoot $fixture -BundlePath $path -Kind "msft-tlb-v1" -Bytes $truncated
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WAC-TYPELIB-METADATA" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "artifact-typelib-segment-corruption" -MessagePattern "segment directory|out of bounds|overlapping" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-TYPELIB-METADATA"
        $path = New-TestBundleArtifact -FixtureRoot $fixture -ManifestRow $row
        [byte[]]$bytes = Get-TestToolchainAssetBytes -Kind "msft-tlb-v1"
        $typeInfoCount = [BitConverter]::ToUInt32($bytes, 0x20)
        $directoryOffset = 0x54 + (4 * $typeInfoCount)
        Set-TestUInt32 `
            -Bytes $bytes `
            -Offset ($directoryOffset + 4) `
            -Value ([Convert]::ToUInt32("FFFFFFFC", 16))
        Set-TestBundleComponentBytes -FixtureRoot $fixture -BundlePath $path -Kind "msft-tlb-v1" -Bytes $bytes
        Set-ManifestBuiltArtifactCurrent -FixtureRoot $fixture -RowId "WAC-TYPELIB-METADATA" -ArtifactPath $path
    }
    Invoke-ExpectedFailure -Name "pending-artifact-unowned" -MessagePattern "missing or unknown pending owner" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation { param($row) $row.built_artifact_owner_bead = "n/a" }
    }
    Invoke-ExpectedFailure -Name "pending-environment-unowned" -MessagePattern "missing or unknown pending owner" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation { param($row) $row.environment_owner_bead = "n/a" }
    }
    Invoke-ExpectedFailure -Name "environment-arbitrary-text" -MessagePattern "not valid environment-capture JSON" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT"
        $path = "$([string]$row.environment_capture_root)/$([string]$row.environment_capture_name)"
        [void](Write-TestUtf8 -FixtureRoot $fixture -RelativePath $path -Text "arbitrary environment notes are not a capture")
        Set-ManifestEnvironmentCurrent -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -CapturePath $path
    }
    Invoke-ExpectedFailure -Name "environment-duplicate-field" -MessagePattern "duplicate JSON property 'environment_id'" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT"
        $path = "$([string]$row.environment_capture_root)/$([string]$row.environment_capture_name)"
        $environmentPath = Resolve-IdealRepoPath -RepoRoot $fixture -Path $environmentRelativePath
        $environment = @(Import-Csv -LiteralPath $environmentPath | Where-Object environment_id -eq "win-x64-dev-oracle-2026-07")[0]
        $capture = New-TestEnvironmentCapture -Environment $environment
        [void](Write-TestJson -FixtureRoot $fixture -RelativePath $path -Value $capture)
        Replace-TestUtf8Text `
            -FixtureRoot $fixture `
            -RelativePath $path `
            -OldValue '  "environment_id": "win-x64-dev-oracle-2026-07",' `
            -NewValue "  `"environment_id`": `"attacker-environment-v1`",`n  `"environment_id`": `"win-x64-dev-oracle-2026-07`","
        Set-ManifestEnvironmentCurrent -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -CapturePath $path
    }
    Invoke-ExpectedFailure -Name "environment-miscased-field" -MessagePattern "exact case-sensitive schema" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT"
        $path = "$([string]$row.environment_capture_root)/$([string]$row.environment_capture_name)"
        $environmentPath = Resolve-IdealRepoPath -RepoRoot $fixture -Path $environmentRelativePath
        $environment = @(Import-Csv -LiteralPath $environmentPath | Where-Object environment_id -eq "win-x64-dev-oracle-2026-07")[0]
        $capture = New-TestEnvironmentCapture -Environment $environment
        [void](Write-TestJson -FixtureRoot $fixture -RelativePath $path -Value $capture)
        Replace-TestUtf8Text -FixtureRoot $fixture -RelativePath $path -OldValue '"environment_id"' -NewValue '"Environment_id"'
        Set-ManifestEnvironmentCurrent -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -CapturePath $path
    }
    Invoke-ExpectedFailure -Name "environment-mutable-alias" -MessagePattern "uses a mutable alias" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT"
        $path = "$([string]$row.environment_capture_root)/latest.json"
        [void](Write-TestUtf8 -FixtureRoot $fixture -RelativePath $path -Text "{}")
        Set-ManifestEnvironmentCurrent -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -CapturePath $path
    }
    Invoke-ExpectedFailure -Name "environment-wrong-id" -MessagePattern "environment/capture identity" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT"
        $path = "$([string]$row.environment_capture_root)/$([string]$row.environment_capture_name)"
        $environmentPath = Resolve-IdealRepoPath -RepoRoot $fixture -Path $environmentRelativePath
        $environment = @(Import-Csv -LiteralPath $environmentPath | Where-Object environment_id -eq "win-x64-dev-oracle-2026-07")[0]
        $capture = New-TestEnvironmentCapture -Environment $environment
        $capture.environment_id = "win-x64-other-2026-07"
        $capture.capture_id = "win-x64-other-2026-07-capture-v1"
        [void](Write-TestJson -FixtureRoot $fixture -RelativePath $path -Value $capture)
        Set-ManifestEnvironmentCurrent -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -CapturePath $path
    }
    Invoke-ExpectedFailure -Name "environment-wrong-bitness" -MessagePattern "bind x64 and Office64" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT"
        $path = "$([string]$row.environment_capture_root)/$([string]$row.environment_capture_name)"
        $environmentPath = Resolve-IdealRepoPath -RepoRoot $fixture -Path $environmentRelativePath
        $environment = @(Import-Csv -LiteralPath $environmentPath | Where-Object environment_id -eq "win-x64-dev-oracle-2026-07")[0]
        $capture = New-TestEnvironmentCapture -Environment $environment
        $capture.office_bitness = "32"
        [void](Write-TestJson -FixtureRoot $fixture -RelativePath $path -Value $capture)
        Set-ManifestEnvironmentCurrent -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -CapturePath $path
    }
    Invoke-ExpectedFailure -Name "environment-wrong-target" -MessagePattern "bind x64 and Office64" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT"
        $path = "$([string]$row.environment_capture_root)/$([string]$row.environment_capture_name)"
        $environmentPath = Resolve-IdealRepoPath -RepoRoot $fixture -Path $environmentRelativePath
        $environment = @(Import-Csv -LiteralPath $environmentPath | Where-Object environment_id -eq "win-x64-dev-oracle-2026-07")[0]
        $capture = New-TestEnvironmentCapture -Environment $environment
        $capture.target_arch = "x86"
        [void](Write-TestJson -FixtureRoot $fixture -RelativePath $path -Value $capture)
        Set-ManifestEnvironmentCurrent -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -CapturePath $path
    }
    Invoke-ExpectedFailure -Name "environment-wrong-role" -MessagePattern "role does not match" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT"
        $path = "$([string]$row.environment_capture_root)/$([string]$row.environment_capture_name)"
        $environmentPath = Resolve-IdealRepoPath -RepoRoot $fixture -Path $environmentRelativePath
        $environment = @(Import-Csv -LiteralPath $environmentPath | Where-Object environment_id -eq "win-x64-dev-oracle-2026-07")[0]
        $capture = New-TestEnvironmentCapture -Environment $environment
        $capture.role = "certification-vm"
        [void](Write-TestJson -FixtureRoot $fixture -RelativePath $path -Value $capture)
        Set-ManifestEnvironmentCurrent -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -CapturePath $path
    }
    Invoke-ExpectedFailure -Name "environment-path-escape" -MessagePattern "repository-relative path" -Mutation {
        param($fixture)
        $manifestRow = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT"
        $hash = "sha256:" + ("3" * 64)
        Update-MatrixRow -FixtureRoot $fixture -MatrixId ([string]$manifestRow.matrix_id) -RowId "WAC-BSTR-LAYOUT" -Mutation {
            param($row)
            $row.environment_hash = $hash
        }
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation {
            param($row)
            $row.environment_state = "current"
            $row.environment_capture_path = "$([string]$row.environment_capture_root)/../escape.json"
            $row.environment_hash = $hash
            $row.environment_owner_bead = "n/a"
        }
    }
    Invoke-ExpectedFailure -Name "environment-mutable-image" -MessagePattern "environment/image identity is mutable" -Mutation {
        param($fixture)
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT"
        $path = "$([string]$row.environment_capture_root)/$([string]$row.environment_capture_name)"
        $environmentPath = Resolve-IdealRepoPath -RepoRoot $fixture -Path $environmentRelativePath
        $environment = @(Import-Csv -LiteralPath $environmentPath | Where-Object environment_id -eq "win-x64-dev-oracle-2026-07")[0]
        $capture = New-TestEnvironmentCapture -Environment $environment
        [void](Write-TestJson -FixtureRoot $fixture -RelativePath $path -Value $capture)
        Set-ManifestEnvironmentCurrent -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -CapturePath $path
    }
    Invoke-ExpectedFailure -Name "environment-dev-certifying-flags" -MessagePattern "dev-oracle capture must remain explicitly noncertifying" -Mutation {
        param($fixture)
        $environmentPath = Resolve-IdealRepoPath -RepoRoot $fixture -Path $environmentRelativePath
        $environmentRows = @(Import-Csv -LiteralPath $environmentPath)
        $environment = @($environmentRows | Where-Object environment_id -eq "win-x64-dev-oracle-2026-07")[0]
        $environment.snapshot_or_image = "dev-oracle-2026-07@sha256:" + ("b" * 64)
        $environmentRows | Export-Csv -LiteralPath $environmentPath -NoTypeInformation -Encoding UTF8 -UseQuotes Always
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT"
        $path = "$([string]$row.environment_capture_root)/$([string]$row.environment_capture_name)"
        $capture = New-TestEnvironmentCapture -Environment $environment
        $capture.certification_authority = $true
        $capture.noncertifying = $false
        [void](Write-TestJson -FixtureRoot $fixture -RelativePath $path -Value $capture)
        Set-ManifestEnvironmentCurrent -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -CapturePath $path
    }
    Invoke-ExpectedFailure -Name "environment-reset-policy-hash" -MessagePattern "reset_policy_hash does not bind" -Mutation {
        param($fixture)
        $environmentPath = Resolve-IdealRepoPath -RepoRoot $fixture -Path $environmentRelativePath
        $environmentRows = @(Import-Csv -LiteralPath $environmentPath)
        $environment = @($environmentRows | Where-Object environment_id -eq "win-x64-dev-oracle-2026-07")[0]
        $environment.snapshot_or_image = "dev-oracle-2026-07@sha256:" + ("c" * 64)
        $environmentRows | Export-Csv -LiteralPath $environmentPath -NoTypeInformation -Encoding UTF8 -UseQuotes Always
        $row = Get-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT"
        $path = "$([string]$row.environment_capture_root)/$([string]$row.environment_capture_name)"
        $capture = New-TestEnvironmentCapture -Environment $environment
        $capture.reset_policy_hash = "sha256:" + ("d" * 64)
        [void](Write-TestJson -FixtureRoot $fixture -RelativePath $path -Value $capture)
        Set-ManifestEnvironmentCurrent -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -CapturePath $path
    }
    Invoke-ExpectedFailure -Name "blank-cleanup" -MessagePattern "has blank 'cleanup_recipe'" -Mutation {
        param($fixture)
        Update-ManifestRow -FixtureRoot $fixture -RowId "WAC-BSTR-LAYOUT" -Mutation { param($row) $row.cleanup_recipe = "" }
    }

    $negativeCount = 46 + $windowsLoaderNegativeCount
    Write-Host "test-windows-fixture-manifest: ok (positive=7 negative=$negativeCount windows_loader_negative=$windowsLoaderNegativeCount rows=57 capability_credit=none)"
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
