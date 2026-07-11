param(
    [string]$RepositoryRoot = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$utf8 = [Text.UTF8Encoding]::new($false, $true)

$repoRoot = if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    [IO.Path]::GetFullPath((Resolve-Path (Join-Path $PSScriptRoot "..")).Path)
}
else {
    [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $RepositoryRoot).Path)
}
$validator = Join-Path $repoRoot "scripts/validate-linux-ci-environment.ps1"
$contractRelative = "ci/linux-x64/contract-v1.json"
$environmentRelative = "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv"

function Get-CanonicalHash {
    param([Parameter(Mandatory = $true)][string]$Path)

    $text = $utf8.GetString([IO.File]::ReadAllBytes($Path))
    $bytes = $utf8.GetBytes($text.Replace("`r`n", "`n").Replace("`r", "`n"))
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
}

function Copy-ControlledFile {
    param(
        [Parameter(Mandatory = $true)][string]$FromRoot,
        [Parameter(Mandatory = $true)][string]$ToRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath
    )

    $source = Join-Path $FromRoot $RelativePath
    $destination = Join-Path $ToRoot $RelativePath
    $parent = Split-Path -Parent $destination
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        [void](New-Item -ItemType Directory -Path $parent -Force)
    }
    Copy-Item -LiteralPath $source -Destination $destination -Force
}

function New-Fixture {
    param([Parameter(Mandatory = $true)][string]$Name)

    $root = Join-Path $tempBase $Name
    [void](New-Item -ItemType Directory -Path $root -Force)
    Copy-ControlledFile -FromRoot $repoRoot -ToRoot $root -RelativePath $contractRelative
    Copy-ControlledFile -FromRoot $repoRoot -ToRoot $root -RelativePath $environmentRelative
    $contract = Get-Content -LiteralPath (Join-Path $root $contractRelative) -Raw | ConvertFrom-Json
    foreach ($source in @($contract.source_files)) {
        Copy-ControlledFile -FromRoot $repoRoot -ToRoot $root -RelativePath ([string]$source.path)
    }
    return $root
}

function Write-Contract {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)]$Contract
    )

    $path = Join-Path $Root $contractRelative
    $json = $Contract | ConvertTo-Json -Depth 20
    [IO.File]::WriteAllText($path, $json + "`n", $utf8)
}

function Update-ControlledSourceHash {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$RelativePath
    )

    $contract = Get-Content -LiteralPath (Join-Path $Root $contractRelative) -Raw | ConvertFrom-Json
    $rows = @($contract.source_files | Where-Object { [string]$_.path -ceq $RelativePath })
    if ($rows.Count -ne 1) { throw "test fixture has no unique source row '$RelativePath'" }
    $rows[0].sha256 = Get-CanonicalHash -Path (Join-Path $Root $RelativePath)
    Write-Contract -Root $Root -Contract $contract
}

function Update-TextFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Old,
        [Parameter(Mandatory = $true)][string]$New
    )

    $text = $utf8.GetString([IO.File]::ReadAllBytes($Path))
    if (-not $text.Contains($Old, [StringComparison]::Ordinal)) {
        throw "test fixture mutation anchor is missing: $Old"
    }
    [IO.File]::WriteAllText($Path, $text.Replace($Old, $New, [StringComparison]::Ordinal), $utf8)
}

function Set-SealedLedger {
    param([Parameter(Mandatory = $true)][string]$Root)

    $contractPath = Join-Path $Root $contractRelative
    $contract = Get-Content -LiteralPath $contractPath -Raw | ConvertFrom-Json
    $contractHash = Get-CanonicalHash -Path $contractPath
    $manifestPath = Join-Path $Root $environmentRelative
    $rows = @(Import-Csv -LiteralPath $manifestPath)
    $linux = @($rows | Where-Object role -eq "linux-ci")
    if ($linux.Count -ne 1) { throw "test fixture has no unique linux-ci row" }
    $row = $linux[0]
    $imageDigest = "sha256:4ec71e955e6c08aeb238885083222ddff79d82eb87654a96c76e38e94da1a53b"
    $resetPolicy = "github-hosted-new-vm-per-job;fresh-digest-pinned-job-container;clean-checkout;no-actions-cache;owned-state-under-RUNNER_TEMP;delete-owned-processes-and-state-only"
    $row.environment_id = [string]$contract.environment_id
    $row.role = "linux-ci"
    $row.profile = "core"
    $row.target_arch = "x64"
    $row.os_build = "debian-12-bookworm-amd64@$imageDigest"
    $row.office_product = "n/a"
    $row.office_version = "n/a"
    $row.office_build = "n/a"
    $row.office_channel = "n/a"
    $row.office_bitness = "n/a"
    $row.locale = "C.UTF-8"
    $row.snapshot_or_image = "docker.io/library/rust@$imageDigest"
    $row.reset_policy = $resetPolicy
    $row.fixture_manifest = $contractRelative
    $row.fixture_hash = "sha256:$contractHash"
    $row.owned_process_policy = "github-hosted-new-VM-per-job;fresh-job-container;no-actions-cache;record-and-clean-owned-processes-and-RUNNER_TEMP-state-only"
    $row.uia_modal_policy = "n/a-no-Excel-UIA"
    $row.evidence_state = "planned-blocking"
    $row.owner_bead = "bd-59co.2.2.11"
    $row.notes = "Immutable Linux x64 execution contract is sealed; the host label is scheduling only; the canonical baseline transcript remains pending under bd-59co.2.2.11"
    $csv = $rows | ConvertTo-Csv -NoTypeInformation
    [IO.File]::WriteAllLines($manifestPath, $csv, $utf8)
}

function Invoke-Validator {
    param([Parameter(Mandatory = $true)][string]$Root)

    & $validator -RepositoryRoot $Root
}

function Invoke-ExpectedFailure {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$MessagePattern,
        [Parameter(Mandatory = $true)][scriptblock]$Mutation
    )

    $fixture = New-Fixture -Name $Name
    & $Mutation $fixture
    $failed = $false
    try {
        Invoke-Validator -Root $fixture
    }
    catch {
        if ($_.Exception.Message -notmatch $MessagePattern) {
            throw "linux CI mutation '$Name' failed for the wrong reason: $($_.Exception.Message)"
        }
        $failed = $true
    }
    if (-not $failed) { throw "linux CI mutation '$Name' unexpectedly passed" }
    Write-Host "linux-ci-environment mutation: ok ($Name)"
}

$systemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
$tempBase = Join-Path $systemTemp ("oxvba-linux-ci-environment-" + [guid]::NewGuid().ToString("N"))
[void](New-Item -ItemType Directory -Path $tempBase)

try {
    $pending = New-Fixture -Name "positive-pending-ledger"
    Invoke-Validator -Root $pending

    $sealed = New-Fixture -Name "positive-sealed-ledger"
    Set-SealedLedger -Root $sealed
    Invoke-Validator -Root $sealed

    Invoke-ExpectedFailure -Name "ubuntu-latest" -MessagePattern "runs-on" -Mutation {
        param($root)
        $path = Join-Path $root ".github/workflows/ci.yml"
        Update-TextFile -Path $path -Old "runs-on: ubuntu-24.04" -New "runs-on: ubuntu-latest"
        Update-ControlledSourceHash -Root $root -RelativePath ".github/workflows/ci.yml"
    }
    Invoke-ExpectedFailure -Name "container-tag" -MessagePattern "image:" -Mutation {
        param($root)
        $path = Join-Path $root ".github/workflows/ci.yml"
        Update-TextFile -Path $path -Old "image: rust@sha256:4ec71e955e6c08aeb238885083222ddff79d82eb87654a96c76e38e94da1a53b" -New "image: rust:1.94.1-bookworm"
        Update-ControlledSourceHash -Root $root -RelativePath ".github/workflows/ci.yml"
    }
    Invoke-ExpectedFailure -Name "container-wrong-digest" -MessagePattern "image:" -Mutation {
        param($root)
        $path = Join-Path $root ".github/workflows/ci.yml"
        Update-TextFile -Path $path -Old "4ec71e955e6c08aeb238885083222ddff79d82eb87654a96c76e38e94da1a53b" -New ("1" * 64)
        Update-ControlledSourceHash -Root $root -RelativePath ".github/workflows/ci.yml"
    }
    Invoke-ExpectedFailure -Name "checkout-tag" -MessagePattern "uses: actions/checkout" -Mutation {
        param($root)
        $path = Join-Path $root ".github/workflows/ci.yml"
        Update-TextFile -Path $path -Old "actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5" -New "actions/checkout@v4"
        Update-ControlledSourceHash -Root $root -RelativePath ".github/workflows/ci.yml"
    }
    Invoke-ExpectedFailure -Name "stable-toolchain-action" -MessagePattern "uses: actions/checkout" -Mutation {
        param($root)
        $path = Join-Path $root ".github/workflows/ci.yml"
        Update-TextFile -Path $path -Old "actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5" -New "dtolnay/rust-toolchain@stable"
        Update-ControlledSourceHash -Root $root -RelativePath ".github/workflows/ci.yml"
    }
    Invoke-ExpectedFailure -Name "retained-cache-action" -MessagePattern "mutable alias or retained cache" -Mutation {
        param($root)
        $path = Join-Path $root ".github/workflows/ci.yml"
        Update-TextFile -Path $path -Old "          persist-credentials: false" -New "          persist-credentials: false`n      - uses: Swatinem/rust-cache@401aff9a7a08acb9d27b64936a90db81024cff97"
        Update-ControlledSourceHash -Root $root -RelativePath ".github/workflows/ci.yml"
    }
    Invoke-ExpectedFailure -Name "locale-drift" -MessagePattern "LANG: C.UTF-8" -Mutation {
        param($root)
        $path = Join-Path $root ".github/workflows/ci.yml"
        Update-TextFile -Path $path -Old "LANG: C.UTF-8" -New "LANG: en_US.UTF-8"
        Update-ControlledSourceHash -Root $root -RelativePath ".github/workflows/ci.yml"
    }
    Invoke-ExpectedFailure -Name "runtime-preflight-removed" -MessagePattern "validate-linux-ci-environment" -Mutation {
        param($root)
        $path = Join-Path $root ".github/workflows/ci.yml"
        Update-TextFile -Path $path -Old "run: ./scripts/validate-linux-ci-environment.ps1 -Runtime" -New "run: echo skipped-runtime-contract"
        Update-ControlledSourceHash -Root $root -RelativePath ".github/workflows/ci.yml"
    }
    Invoke-ExpectedFailure -Name "mutable-kani-version" -MessagePattern "setup-kani" -Mutation {
        param($root)
        $path = Join-Path $root ".github/workflows/ci.yml"
        Update-TextFile -Path $path -Old "./scripts/setup-kani.ps1 -Install -Version 0.67.0" -New "./scripts/setup-kani.ps1 -Install"
        Update-ControlledSourceHash -Root $root -RelativePath ".github/workflows/ci.yml"
    }
    Invoke-ExpectedFailure -Name "contract-stable-rust" -MessagePattern "toolchains.rust.version" -Mutation {
        param($root)
        $contract = Get-Content -LiteralPath (Join-Path $root $contractRelative) -Raw | ConvertFrom-Json
        $contract.toolchains.rust.version = "stable"
        Write-Contract -Root $root -Contract $contract
    }
    Invoke-ExpectedFailure -Name "runner-image-version-drift" -MessagePattern "scheduler.image_version" -Mutation {
        param($root)
        $contract = Get-Content -LiteralPath (Join-Path $root $contractRelative) -Raw | ConvertFrom-Json
        $contract.scheduler.image_version = "latest"
        Write-Contract -Root $root -Contract $contract
    }
    Invoke-ExpectedFailure -Name "contract-image-tag" -MessagePattern "execution_image.reference" -Mutation {
        param($root)
        $contract = Get-Content -LiteralPath (Join-Path $root $contractRelative) -Raw | ConvertFrom-Json
        $contract.execution_image.reference = "rust:latest"
        Write-Contract -Root $root -Contract $contract
    }
    Invoke-ExpectedFailure -Name "contract-reset-retains-state" -MessagePattern "determinism.reset_policy" -Mutation {
        param($root)
        $contract = Get-Content -LiteralPath (Join-Path $root $contractRelative) -Raw | ConvertFrom-Json
        $contract.determinism.reset_policy = "reuse-workspace-and-cache"
        Write-Contract -Root $root -Contract $contract
    }
    Invoke-ExpectedFailure -Name "forged-source-hash" -MessagePattern "controlled source hash mismatch" -Mutation {
        param($root)
        $contract = Get-Content -LiteralPath (Join-Path $root $contractRelative) -Raw | ConvertFrom-Json
        $contract.source_files[0].sha256 = "1" * 64
        Write-Contract -Root $root -Contract $contract
    }
    Invoke-ExpectedFailure -Name "duplicate-json-key" -MessagePattern "duplicate JSON property 'contract_id'" -Mutation {
        param($root)
        $path = Join-Path $root $contractRelative
        Update-TextFile -Path $path -Old '  "contract_id":' -New "  `"contract_id`": `"duplicate`",`n  `"contract_id`":"
    }
    Invoke-ExpectedFailure -Name "mis-cased-json-key" -MessagePattern "missing exact property 'schema_id'|mis-cased property 'Schema_Id'" -Mutation {
        param($root)
        $path = Join-Path $root $contractRelative
        Update-TextFile -Path $path -Old '"schema_id"' -New '"Schema_Id"'
    }
    Invoke-ExpectedFailure -Name "ledger-mutable-alias" -MessagePattern "neither the exact pending handoff nor sealed" -Mutation {
        param($root)
        $path = Join-Path $root $environmentRelative
        Update-TextFile -Path $path -Old "linux-x64-ci-pending-v1" -New "ubuntu-latest"
    }
    Invoke-ExpectedFailure -Name "ledger-owner-drift" -MessagePattern "pending linux-ci ledger owner_bead" -Mutation {
        param($root)
        $path = Join-Path $root $environmentRelative
        Update-TextFile -Path $path -Old "bd-59co.2.2.8,Immutable Linux" -New "bd-59co.2.2.11,Immutable Linux"
    }

    Write-Host "test-linux-ci-environment: ok (positive=2 mutations=18)"
}
finally {
    if (Test-Path -LiteralPath $tempBase -PathType Container) {
        $resolved = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $tempBase).Path)
        if (-not $resolved.StartsWith($systemTemp + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
            -not ([IO.Path]::GetFileName($resolved)).StartsWith("oxvba-linux-ci-environment-", [StringComparison]::Ordinal)) {
            throw "refusing unsafe Linux CI test cleanup: $resolved"
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
