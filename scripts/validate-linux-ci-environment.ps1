param(
    [string]$ContractPath = "ci/linux-x64/contract-v1.json",
    [string]$EnvironmentManifestPath = "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv",
    [string]$RepositoryRoot = "",
    [switch]$Runtime,
    [switch]$VerifyExternalProvenance
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$utf8 = [Text.UTF8Encoding]::new($false, $true)

function Resolve-RepoPath {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $normalized = $Path.Replace('\', '/')
    if ([string]::IsNullOrWhiteSpace($Path) -or [IO.Path]::IsPathRooted($Path) -or
        $normalized -match '(^|/)\.\.(/|$)') {
        throw "linux-ci-environment: $Owner must be a repository-relative path, found '$Path'"
    }
    $resolved = [IO.Path]::GetFullPath((Join-Path $Root $Path))
    $rootPrefix = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    $comparison = if ($IsWindows) { [StringComparison]::OrdinalIgnoreCase } else { [StringComparison]::Ordinal }
    if (-not $resolved.StartsWith($rootPrefix, $comparison)) {
        throw "linux-ci-environment: $Owner escapes the repository: '$Path'"
    }
    return $resolved
}

function Get-CanonicalTextBytes {
    param([Parameter(Mandatory = $true)][string]$Path)

    $text = $utf8.GetString([IO.File]::ReadAllBytes($Path))
    return $utf8.GetBytes($text.Replace("`r`n", "`n").Replace("`r", "`n"))
}

function Get-Sha256Hex {
    param([Parameter(Mandatory = $true)][byte[]]$Bytes)

    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Bytes)).ToLowerInvariant()
}

function Assert-NoDuplicateJsonProperties {
    param(
        [Parameter(Mandatory = $true)][Text.Json.JsonElement]$Element,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    switch ($Element.ValueKind) {
        ([Text.Json.JsonValueKind]::Object) {
            $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
            foreach ($property in $Element.EnumerateObject()) {
                if (-not $seen.Add($property.Name)) {
                    throw "linux-ci-environment: duplicate JSON property '$($property.Name)' in $Owner"
                }
                Assert-NoDuplicateJsonProperties -Element $property.Value -Owner "$Owner.$($property.Name)"
            }
        }
        ([Text.Json.JsonValueKind]::Array) {
            $index = 0
            foreach ($item in $Element.EnumerateArray()) {
                Assert-NoDuplicateJsonProperties -Element $item -Owner "$Owner[$index]"
                $index++
            }
        }
    }
}

function Read-StrictJson {
    param([Parameter(Mandatory = $true)][string]$Path)

    [byte[]]$bytes = [IO.File]::ReadAllBytes($Path)
    $options = [Text.Json.JsonDocumentOptions]::new()
    $options.AllowTrailingCommas = $false
    $options.CommentHandling = [Text.Json.JsonCommentHandling]::Disallow
    $stream = [IO.MemoryStream]::new($bytes, $false)
    try {
        $document = [Text.Json.JsonDocument]::Parse($stream, $options)
    }
    catch {
        throw "linux-ci-environment: contract is not strict JSON: $($_.Exception.Message)"
    }
    finally {
        $stream.Dispose()
    }
    try {
        Assert-NoDuplicateJsonProperties -Element $document.RootElement -Owner "contract"
    }
    finally {
        $document.Dispose()
    }
    try {
        return $utf8.GetString($bytes) | ConvertFrom-Json
    }
    catch {
        throw "linux-ci-environment: contract JSON cannot be decoded: $($_.Exception.Message)"
    }
}

function Assert-ExactKeys {
    param(
        [Parameter(Mandatory = $true)]$Object,
        [Parameter(Mandatory = $true)][string[]]$Expected,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $actual = @($Object.PSObject.Properties.Name)
    $expectedSet = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($name in $Expected) { [void]$expectedSet.Add($name) }
    $actualSet = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($name in $actual) { [void]$actualSet.Add($name) }
    if ($actual.Count -ne $Expected.Count -or $actualSet.Count -ne $Expected.Count) {
        throw "linux-ci-environment: $Owner properties must be exactly [$($Expected -join ', ')]"
    }
    foreach ($name in $Expected) {
        if (-not $actualSet.Contains($name)) {
            throw "linux-ci-environment: $Owner is missing exact property '$name'"
        }
    }
    foreach ($name in $actual) {
        if (-not $expectedSet.Contains($name)) {
            throw "linux-ci-environment: $Owner has unexpected or mis-cased property '$name'"
        }
    }
}

function Assert-ExactValue {
    param(
        [AllowNull()]$Actual,
        [AllowNull()]$Expected,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ($Actual -is [bool] -or $Expected -is [bool]) {
        if ([bool]$Actual -ne [bool]$Expected) {
            throw "linux-ci-environment: $Owner must be '$Expected', found '$Actual'"
        }
        return
    }
    if ([string]$Actual -cne [string]$Expected) {
        throw "linux-ci-environment: $Owner must be '$Expected', found '$Actual'"
    }
}

function Get-WorkflowJobBlock {
    param(
        [Parameter(Mandatory = $true)][string]$WorkflowText,
        [Parameter(Mandatory = $true)][string]$JobName
    )

    $lines = @($WorkflowText.Replace("`r`n", "`n").Replace("`r", "`n") -split "`n")
    $startMatches = @(0..($lines.Count - 1) | Where-Object { $lines[$_] -ceq "  $JobName`:" })
    if ($startMatches.Count -ne 1) {
        throw "linux-ci-environment: workflow must contain exactly one '$JobName' job"
    }
    $start = $startMatches[0]
    $end = $lines.Count
    for ($index = $start + 1; $index -lt $lines.Count; $index++) {
        if ($lines[$index] -cmatch '^  [A-Za-z0-9_-]+:\s*$') {
            $end = $index
            break
        }
    }
    return ($lines[$start..($end - 1)] -join "`n")
}

function Assert-JobLine {
    param(
        [Parameter(Mandatory = $true)][string]$Block,
        [Parameter(Mandatory = $true)][string]$Line,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $count = [regex]::Matches($Block, "(?m)^\s*$([regex]::Escape($Line))\s*$").Count
    if ($count -ne 1) {
        throw "linux-ci-environment: $Owner must contain exactly one '$Line' line, found $count"
    }
}

function Get-WebResponseBytes {
    param([Parameter(Mandatory = $true)]$Response)

    if ($Response.Content -is [byte[]]) { return [byte[]]$Response.Content }
    return $utf8.GetBytes([string]$Response.Content)
}

function Invoke-GitHubApi {
    param([Parameter(Mandatory = $true)][string]$Uri)

    return Invoke-RestMethod -Uri $Uri -Headers @{
        Accept = "application/vnd.github+json"
        "User-Agent" = "OxVba-linux-ci-contract-v1"
        "X-GitHub-Api-Version" = "2022-11-28"
    }
}

function Resolve-GitHubTagCommit {
    param(
        [Parameter(Mandatory = $true)][string]$Repository,
        [Parameter(Mandatory = $true)][string]$Tag
    )

    $encodedTag = [uri]::EscapeDataString($Tag)
    $reference = Invoke-GitHubApi -Uri "https://api.github.com/repos/$Repository/git/ref/tags/$encodedTag"
    $object = $reference.object
    for ($depth = 0; $depth -lt 3; $depth++) {
        if ([string]$object.type -eq "commit") { return [string]$object.sha }
        if ([string]$object.type -ne "tag") {
            throw "linux-ci-environment: unsupported tag object type '$($object.type)' for $Repository/$Tag"
        }
        $tagObject = Invoke-GitHubApi -Uri "https://api.github.com/repos/$Repository/git/tags/$($object.sha)"
        $object = $tagObject.object
    }
    throw "linux-ci-environment: tag indirection is too deep for $Repository/$Tag"
}

$repoRoot = if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    [IO.Path]::GetFullPath((Resolve-Path (Join-Path $PSScriptRoot "..")).Path)
}
else {
    [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $RepositoryRoot).Path)
}
$contractAbs = Resolve-RepoPath -Root $repoRoot -Path $ContractPath -Owner "ContractPath"
$environmentAbs = Resolve-RepoPath -Root $repoRoot -Path $EnvironmentManifestPath -Owner "EnvironmentManifestPath"
if (-not (Test-Path -LiteralPath $contractAbs -PathType Leaf)) {
    throw "linux-ci-environment: contract is missing: $ContractPath"
}
if (-not (Test-Path -LiteralPath $environmentAbs -PathType Leaf)) {
    throw "linux-ci-environment: environment manifest is missing: $EnvironmentManifestPath"
}

$contract = Read-StrictJson -Path $contractAbs
Assert-ExactKeys -Object $contract -Expected @(
    "schema_id", "contract_id", "environment_id", "profile", "target", "scheduler",
    "execution_image", "toolchains", "actions", "workflow", "determinism", "source_files",
    "availability"
) -Owner "contract"
Assert-ExactValue $contract.schema_id "oxvba-linux-x64-ci-contract-v1" "contract.schema_id"
Assert-ExactValue $contract.contract_id "linux-x64-ci-rust-1.94.1-bookworm-amd64-v1" "contract.contract_id"
Assert-ExactValue $contract.environment_id $contract.contract_id "contract.environment_id"
Assert-ExactValue $contract.profile "core" "contract.profile"

Assert-ExactKeys $contract.target @("os", "architecture", "rust_target", "os_release_id", "os_release_version") "contract.target"
Assert-ExactValue $contract.target.os "linux" "contract.target.os"
Assert-ExactValue $contract.target.architecture "x86_64" "contract.target.architecture"
Assert-ExactValue $contract.target.rust_target "x86_64-unknown-linux-gnu" "contract.target.rust_target"
Assert-ExactValue $contract.target.os_release_id "debian" "contract.target.os_release_id"
Assert-ExactValue $contract.target.os_release_version "12" "contract.target.os_release_version"

Assert-ExactKeys $contract.scheduler @(
    "provider", "runs_on", "image_os", "image_version", "image_release", "image_release_commit",
    "sbom_asset_sha256", "execution_authority", "fresh_vm_per_job", "release_url", "reset_policy_url"
) "contract.scheduler"
Assert-ExactValue $contract.scheduler.provider "github-hosted" "contract.scheduler.provider"
Assert-ExactValue $contract.scheduler.runs_on "ubuntu-24.04" "contract.scheduler.runs_on"
Assert-ExactValue $contract.scheduler.image_os "ubuntu24" "contract.scheduler.image_os"
Assert-ExactValue $contract.scheduler.image_version "20260705.232" "contract.scheduler.image_version"
Assert-ExactValue $contract.scheduler.image_release "ubuntu24/20260705.232" "contract.scheduler.image_release"
Assert-ExactValue $contract.scheduler.image_release_commit "7a421938a88d5f98ff2cf22875b5237aa80f54c1" "contract.scheduler.image_release_commit"
Assert-ExactValue $contract.scheduler.sbom_asset_sha256 "3a0031ca049f21bd6a8af509c4b21fa967e75bd66617fb0786cc9a91042dafdb" "contract.scheduler.sbom_asset_sha256"
Assert-ExactValue $contract.scheduler.execution_authority $false "contract.scheduler.execution_authority"
Assert-ExactValue $contract.scheduler.fresh_vm_per_job $true "contract.scheduler.fresh_vm_per_job"
Assert-ExactValue $contract.scheduler.release_url "https://github.com/actions/runner-images/releases/tag/ubuntu24/20260705.232" "contract.scheduler.release_url"
Assert-ExactValue $contract.scheduler.reset_policy_url "https://docs.github.com/en/actions/reference/runners/github-hosted-runners" "contract.scheduler.reset_policy_url"

$imageDigest = "sha256:4ec71e955e6c08aeb238885083222ddff79d82eb87654a96c76e38e94da1a53b"
$imageReference = "rust@$imageDigest"
Assert-ExactKeys $contract.execution_image @("repository", "reference", "tag_provenance", "index_digest", "manifest_digest", "registry_manifest_url", "provenance_url") "contract.execution_image"
Assert-ExactValue $contract.execution_image.repository "docker.io/library/rust" "contract.execution_image.repository"
Assert-ExactValue $contract.execution_image.reference $imageReference "contract.execution_image.reference"
Assert-ExactValue $contract.execution_image.tag_provenance "1.94.1-bookworm" "contract.execution_image.tag_provenance"
Assert-ExactValue $contract.execution_image.index_digest "sha256:6ae102bdbf528294bc79ad6e1fae682f6f7c2a6e6621506ba959f9685b308a55" "contract.execution_image.index_digest"
Assert-ExactValue $contract.execution_image.manifest_digest $imageDigest "contract.execution_image.manifest_digest"
Assert-ExactValue $contract.execution_image.registry_manifest_url "https://registry-1.docker.io/v2/library/rust/manifests/1.94.1-bookworm" "contract.execution_image.registry_manifest_url"
Assert-ExactValue $contract.execution_image.provenance_url "https://hub.docker.com/_/rust/" "contract.execution_image.provenance_url"

Assert-ExactKeys $contract.toolchains @("rust", "powershell", "kani", "wasmtime") "contract.toolchains"
Assert-ExactKeys $contract.toolchains.rust @("version", "rustc_commit", "cargo_version", "cargo_commit", "dist_manifest_url", "dist_manifest_sha256", "release_url") "contract.toolchains.rust"
Assert-ExactValue $contract.toolchains.rust.version "1.94.1" "contract.toolchains.rust.version"
Assert-ExactValue $contract.toolchains.rust.rustc_commit "e408947bfd200af42db322daf0fadfe7e26d3bd1" "contract.toolchains.rust.rustc_commit"
Assert-ExactValue $contract.toolchains.rust.cargo_version "1.94.1" "contract.toolchains.rust.cargo_version"
Assert-ExactValue $contract.toolchains.rust.cargo_commit "29ea6fb6a5db279426f4cc4e17aa385f05a0cfbc" "contract.toolchains.rust.cargo_commit"
Assert-ExactValue $contract.toolchains.rust.dist_manifest_url "https://static.rust-lang.org/dist/channel-rust-1.94.1.toml" "contract.toolchains.rust.dist_manifest_url"
Assert-ExactValue $contract.toolchains.rust.dist_manifest_sha256 "cc2f04dfc883549d683c8cc2a9393f523a3dfbd931f5d5eaef00303cca64a60d" "contract.toolchains.rust.dist_manifest_sha256"
Assert-ExactValue $contract.toolchains.rust.release_url "https://blog.rust-lang.org/2026/03/26/1.94.1-release/" "contract.toolchains.rust.release_url"

Assert-ExactKeys $contract.toolchains.powershell @("version", "asset_url", "asset_sha256", "release_commit", "release_url") "contract.toolchains.powershell"
Assert-ExactValue $contract.toolchains.powershell.version "7.5.7" "contract.toolchains.powershell.version"
Assert-ExactValue $contract.toolchains.powershell.asset_url "https://github.com/PowerShell/PowerShell/releases/download/v7.5.7/powershell-7.5.7-linux-x64.tar.gz" "contract.toolchains.powershell.asset_url"
Assert-ExactValue $contract.toolchains.powershell.asset_sha256 "207a3c0b2f630e8e1226cc9beb651e2e16789f07729197f45fd3ad0902d1c593" "contract.toolchains.powershell.asset_sha256"
Assert-ExactValue $contract.toolchains.powershell.release_commit "3416dd0145a9530d93c0a9be8c066ca4212a8c16" "contract.toolchains.powershell.release_commit"
Assert-ExactValue $contract.toolchains.powershell.release_url "https://github.com/PowerShell/PowerShell/releases/tag/v7.5.7" "contract.toolchains.powershell.release_url"

Assert-ExactKeys $contract.toolchains.kani @("version", "release_commit", "release_url") "contract.toolchains.kani"
Assert-ExactValue $contract.toolchains.kani.version "0.67.0" "contract.toolchains.kani.version"
Assert-ExactValue $contract.toolchains.kani.release_commit "4feaaad1d6a2378a6ff6caa3b4fc5d6999c7bb5d" "contract.toolchains.kani.release_commit"
Assert-ExactValue $contract.toolchains.kani.release_url "https://github.com/model-checking/kani/releases/tag/kani-0.67.0" "contract.toolchains.kani.release_url"

Assert-ExactKeys $contract.toolchains.wasmtime @("version", "linux_asset_sha256", "linux_binary_sha256", "windows_asset_sha256", "windows_binary_sha256", "release_commit", "release_url") "contract.toolchains.wasmtime"
Assert-ExactValue $contract.toolchains.wasmtime.version "42.0.1" "contract.toolchains.wasmtime.version"
Assert-ExactValue $contract.toolchains.wasmtime.linux_asset_sha256 "dd5253f3cb521bb094f9951c3d2c45c746b31e5723b07ce56f162ec9bab44d59" "contract.toolchains.wasmtime.linux_asset_sha256"
Assert-ExactValue $contract.toolchains.wasmtime.linux_binary_sha256 "21f8e8f994a96d2267afe4a4c06a6302e78aca20e9438afbf01dd443fe32108b" "contract.toolchains.wasmtime.linux_binary_sha256"
Assert-ExactValue $contract.toolchains.wasmtime.windows_asset_sha256 "daa52754776eabdbbf82037d41a26f556ccd4fd5723dcab328b12c680894c072" "contract.toolchains.wasmtime.windows_asset_sha256"
Assert-ExactValue $contract.toolchains.wasmtime.windows_binary_sha256 "b86766999318183c37f5a51c56d4ae26ecdf34099cd0ebbbdf0108e1013ba4b1" "contract.toolchains.wasmtime.windows_binary_sha256"
Assert-ExactValue $contract.toolchains.wasmtime.release_commit "6844a83b530f86ad13a65175282fe2dbcd33cad1" "contract.toolchains.wasmtime.release_commit"
Assert-ExactValue $contract.toolchains.wasmtime.release_url "https://github.com/bytecodealliance/wasmtime/releases/tag/v42.0.1" "contract.toolchains.wasmtime.release_url"

$actions = @($contract.actions)
if ($actions.Count -ne 1) { throw "linux-ci-environment: contract.actions must contain exactly one action identity" }
Assert-ExactKeys $actions[0] @("repository", "commit", "provenance_url") "contract.actions[0]"
$checkoutCommit = "34e114876b0b11c390a56381ad16ebd13914f8d5"
Assert-ExactValue $actions[0].repository "actions/checkout" "contract.actions[0].repository"
Assert-ExactValue $actions[0].commit $checkoutCommit "contract.actions[0].commit"
Assert-ExactValue $actions[0].provenance_url "https://github.com/actions/checkout/commit/$checkoutCommit" "contract.actions[0].provenance_url"

Assert-ExactKeys $contract.workflow @("path", "jobs", "forbidden_aliases") "contract.workflow"
Assert-ExactValue $contract.workflow.path ".github/workflows/ci.yml" "contract.workflow.path"
$expectedJobs = @("formal-kani", "linux-ready", "wasm-hal-ready")
if ((@($contract.workflow.jobs) -join '|') -cne ($expectedJobs -join '|')) {
    throw "linux-ci-environment: contract.workflow.jobs must be exactly [$($expectedJobs -join ', ')]"
}
$expectedAliases = @("@main", "@master", "@stable", "@vN", ":latest", "ubuntu-latest")
if ((@($contract.workflow.forbidden_aliases) -join '|') -cne ($expectedAliases -join '|')) {
    throw "linux-ci-environment: contract.workflow.forbidden_aliases drifted"
}

Assert-ExactKeys $contract.determinism @("locale", "timezone", "reset_policy", "retained_state_sentinel", "cross_run_cache") "contract.determinism"
Assert-ExactValue $contract.determinism.locale "C.UTF-8" "contract.determinism.locale"
Assert-ExactValue $contract.determinism.timezone "UTC" "contract.determinism.timezone"
$resetPolicy = "github-hosted-new-vm-per-job;fresh-digest-pinned-job-container;clean-checkout;no-actions-cache;owned-state-under-RUNNER_TEMP;delete-owned-processes-and-state-only"
Assert-ExactValue $contract.determinism.reset_policy $resetPolicy "contract.determinism.reset_policy"
Assert-ExactValue $contract.determinism.retained_state_sentinel "oxvba-linux-ci-retained-state-v1" "contract.determinism.retained_state_sentinel"
Assert-ExactValue $contract.determinism.cross_run_cache "forbidden" "contract.determinism.cross_run_cache"

Assert-ExactKeys $contract.availability @("contract_state", "execution_evidence_state", "execution_owner_bead", "reason") "contract.availability"
Assert-ExactValue $contract.availability.contract_state "sealed" "contract.availability.contract_state"
Assert-ExactValue $contract.availability.execution_evidence_state "planned-blocking" "contract.availability.execution_evidence_state"
Assert-ExactValue $contract.availability.execution_owner_bead "bd-59co.2.2.11" "contract.availability.execution_owner_bead"
Assert-ExactValue $contract.availability.reason "The immutable execution contract is sealed, but no GitHub-hosted run of this exact image has yet produced the canonical Linux baseline transcript." "contract.availability.reason"

$expectedSourcePaths = @(
    ".github/workflows/ci.yml",
    "scripts/check-governance.ps1",
    "scripts/install-pinned-pwsh.sh",
    "scripts/run-hal-conformance-wasm32.ps1",
    "scripts/setup-kani.ps1",
    "scripts/test-linux-ci-environment.ps1",
    "scripts/validate-linux-ci-environment.ps1"
)
$sourceRows = @($contract.source_files)
if ($sourceRows.Count -ne $expectedSourcePaths.Count) {
    throw "linux-ci-environment: contract.source_files must contain exactly $($expectedSourcePaths.Count) rows"
}
for ($index = 0; $index -lt $sourceRows.Count; $index++) {
    $row = $sourceRows[$index]
    Assert-ExactKeys $row @("path", "sha256") "contract.source_files[$index]"
    Assert-ExactValue $row.path $expectedSourcePaths[$index] "contract.source_files[$index].path"
    if ([string]$row.sha256 -cnotmatch '^[0-9a-f]{64}$' -or [string]$row.sha256 -eq ("0" * 64)) {
        throw "linux-ci-environment: contract.source_files[$index].sha256 is not a sealed SHA-256"
    }
    $sourceAbs = Resolve-RepoPath -Root $repoRoot -Path ([string]$row.path) -Owner "contract.source_files[$index].path"
    if (-not (Test-Path -LiteralPath $sourceAbs -PathType Leaf)) {
        throw "linux-ci-environment: controlled source is missing: $($row.path)"
    }
    $actualHash = Get-Sha256Hex -Bytes (Get-CanonicalTextBytes -Path $sourceAbs)
    if ($actualHash -cne [string]$row.sha256) {
        throw "linux-ci-environment: controlled source hash mismatch for '$($row.path)': expected $($row.sha256), found $actualHash"
    }
}

$workflowAbs = Resolve-RepoPath -Root $repoRoot -Path ([string]$contract.workflow.path) -Owner "contract.workflow.path"
$workflowText = $utf8.GetString([IO.File]::ReadAllBytes($workflowAbs))
foreach ($jobName in $expectedJobs) {
    $block = Get-WorkflowJobBlock -WorkflowText $workflowText -JobName $jobName
    Assert-JobLine $block "runs-on: $($contract.scheduler.runs_on)" "workflow job $jobName"
    Assert-JobLine $block "image: $imageReference" "workflow job $jobName"
    Assert-JobLine $block "LANG: $($contract.determinism.locale)" "workflow job $jobName"
    Assert-JobLine $block "LC_ALL: $($contract.determinism.locale)" "workflow job $jobName"
    Assert-JobLine $block "TZ: $($contract.determinism.timezone)" "workflow job $jobName"
    Assert-JobLine $block "- uses: actions/checkout@$checkoutCommit" "workflow job $jobName"
    Assert-JobLine $block "run: bash ./scripts/install-pinned-pwsh.sh" "workflow job $jobName"
    Assert-JobLine $block "run: ./scripts/validate-linux-ci-environment.ps1 -Runtime" "workflow job $jobName"
    Assert-JobLine $block "clean: true" "workflow job $jobName"
    Assert-JobLine $block "persist-credentials: false" "workflow job $jobName"

    if ($block -match '(?i)(ubuntu-latest|@stable|@main|@master|@v\d+(?:\b|\.)|:latest|rust-cache|actions/cache|\bcache\s*:)') {
        throw "linux-ci-environment: workflow job $jobName contains a mutable alias or retained cache path"
    }
    $uses = @([regex]::Matches($block, '(?m)^\s*-\s+uses:\s*(?<value>\S+)\s*$'))
    if ($uses.Count -ne 1 -or $uses[0].Groups['value'].Value -cne "actions/checkout@$checkoutCommit") {
        throw "linux-ci-environment: workflow job $jobName must use only the pinned checkout action"
    }
}
$kaniBlock = Get-WorkflowJobBlock -WorkflowText $workflowText -JobName "formal-kani"
Assert-JobLine $kaniBlock "run: ./scripts/setup-kani.ps1 -Install -Version 0.67.0" "workflow job formal-kani"
$wasmBlock = Get-WorkflowJobBlock -WorkflowText $workflowText -JobName "wasm-hal-ready"
Assert-JobLine $wasmBlock "run: ./scripts/run-hal-conformance-wasm32.ps1 -SkipTests -OutputDir temp/no-artifacts/hal_wasm32_ci" "workflow job wasm-hal-ready"
$readyBlock = Get-WorkflowJobBlock -WorkflowText $workflowText -JobName "linux-ready"
Assert-JobLine $readyBlock "run: ./scripts/meta-check.ps1 -Fast -NoArtifacts" "workflow job linux-ready"

$contractSha = Get-Sha256Hex -Bytes (Get-CanonicalTextBytes -Path $contractAbs)
$linuxRows = @(Import-Csv -LiteralPath $environmentAbs | Where-Object { [string]$_.role -eq "linux-ci" })
if ($linuxRows.Count -ne 1) {
    throw "linux-ci-environment: environment manifest must contain exactly one linux-ci row"
}
$linuxRow = $linuxRows[0]
$ledgerState = ""
if ([string]$linuxRow.environment_id -eq "linux-x64-ci-pending-v1") {
    foreach ($pair in @(
        @("profile", "core"), @("target_arch", "x64"), @("snapshot_or_image", "planned-runner-image-digest"),
        @("fixture_manifest", "pending-CORE-1-fixture-manifest"), @("fixture_hash", "pending-CORE-1-fixture-hash"),
        @("evidence_state", "planned-blocking"), @("owner_bead", "bd-59co.2.2.8")
    )) {
        Assert-ExactValue $linuxRow.($pair[0]) $pair[1] "pending linux-ci ledger $($pair[0])"
    }
    $ledgerState = "pending-controller-handoff"
}
elseif ([string]$linuxRow.environment_id -eq [string]$contract.environment_id) {
    $expectedLedger = [ordered]@{
        role = "linux-ci"
        profile = "core"
        target_arch = "x64"
        os_build = "debian-12-bookworm-amd64@$imageDigest"
        office_product = "n/a"
        office_version = "n/a"
        office_build = "n/a"
        office_channel = "n/a"
        office_bitness = "n/a"
        locale = "C.UTF-8"
        snapshot_or_image = "docker.io/library/rust@$imageDigest"
        reset_policy = $resetPolicy
        fixture_manifest = "ci/linux-x64/contract-v1.json"
        fixture_hash = "sha256:$contractSha"
        owned_process_policy = "github-hosted-new-VM-per-job;fresh-job-container;no-actions-cache;record-and-clean-owned-processes-and-RUNNER_TEMP-state-only"
        uia_modal_policy = "n/a-no-Excel-UIA"
        evidence_state = "planned-blocking"
        owner_bead = "bd-59co.2.2.11"
        notes = "Immutable Linux x64 execution contract is sealed; the host label is scheduling only; the canonical baseline transcript remains pending under bd-59co.2.2.11"
    }
    foreach ($field in $expectedLedger.Keys) {
        Assert-ExactValue $linuxRow.$field $expectedLedger[$field] "sealed linux-ci ledger $field"
    }
    $ledgerState = "sealed-execution-pending"
}
else {
    throw "linux-ci-environment: linux-ci ledger has neither the exact pending handoff nor sealed environment identity: '$($linuxRow.environment_id)'"
}

if ($VerifyExternalProvenance) {
    $runnerRelease = Invoke-GitHubApi -Uri "https://api.github.com/repos/actions/runner-images/releases/tags/ubuntu24%2F20260705.232"
    Assert-ExactValue $runnerRelease.tag_name $contract.scheduler.image_release "live GitHub runner image release"
    Assert-ExactValue $runnerRelease.target_commitish $contract.scheduler.image_release_commit "live GitHub runner image release commit"
    $runnerSbom = @($runnerRelease.assets | Where-Object { $_.name -eq "sbom.ubuntu-24.04.json.zip" })
    if ($runnerSbom.Count -ne 1) { throw "linux-ci-environment: live runner image release lacks its SBOM asset" }
    Assert-ExactValue $runnerSbom[0].digest "sha256:$($contract.scheduler.sbom_asset_sha256)" "live GitHub runner image SBOM digest"

    $rustResponse = Invoke-WebRequest -Uri ([string]$contract.toolchains.rust.dist_manifest_url)
    $rustHash = Get-Sha256Hex -Bytes (Get-WebResponseBytes -Response $rustResponse)
    Assert-ExactValue $rustHash $contract.toolchains.rust.dist_manifest_sha256 "live Rust dist manifest SHA-256"

    $scope = [uri]::EscapeDataString("repository:library/rust:pull")
    $token = (Invoke-RestMethod -Uri "https://auth.docker.io/token?service=registry.docker.io&scope=$scope").token
    $dockerResponse = Invoke-WebRequest -Uri ([string]$contract.execution_image.registry_manifest_url) -Headers @{
        Authorization = "Bearer $token"
        Accept = "application/vnd.oci.image.index.v1+json"
    }
    Assert-ExactValue ([string]$dockerResponse.Headers["Docker-Content-Digest"]) $contract.execution_image.index_digest "live Docker index digest"
    $dockerIndex = $utf8.GetString((Get-WebResponseBytes -Response $dockerResponse)) | ConvertFrom-Json
    $amd64 = @($dockerIndex.manifests | Where-Object { $_.platform.os -eq "linux" -and $_.platform.architecture -eq "amd64" })
    if ($amd64.Count -ne 1) { throw "linux-ci-environment: live Docker index lacks one Linux amd64 manifest" }
    Assert-ExactValue $amd64[0].digest $contract.execution_image.manifest_digest "live Docker Linux amd64 digest"

    $checkout = Invoke-GitHubApi -Uri "https://api.github.com/repos/actions/checkout/commits/$checkoutCommit"
    Assert-ExactValue $checkout.sha $checkoutCommit "live checkout action commit"

    $pwshRelease = Invoke-GitHubApi -Uri "https://api.github.com/repos/PowerShell/PowerShell/releases/tags/v7.5.7"
    $pwshAssets = @($pwshRelease.assets | Where-Object { $_.name -eq "powershell-7.5.7-linux-x64.tar.gz" })
    if ($pwshAssets.Count -ne 1) { throw "linux-ci-environment: live PowerShell release lacks the pinned Linux x64 asset" }
    Assert-ExactValue $pwshAssets[0].digest "sha256:$($contract.toolchains.powershell.asset_sha256)" "live PowerShell asset digest"
    Assert-ExactValue (Resolve-GitHubTagCommit -Repository "PowerShell/PowerShell" -Tag "v7.5.7") $contract.toolchains.powershell.release_commit "live PowerShell release commit"

    $wasmtimeRelease = Invoke-GitHubApi -Uri "https://api.github.com/repos/bytecodealliance/wasmtime/releases/tags/v42.0.1"
    foreach ($assetExpectation in @(
        @("wasmtime-v42.0.1-x86_64-linux.tar.xz", "sha256:$($contract.toolchains.wasmtime.linux_asset_sha256)"),
        @("wasmtime-v42.0.1-x86_64-windows.zip", "sha256:$($contract.toolchains.wasmtime.windows_asset_sha256)")
    )) {
        $assets = @($wasmtimeRelease.assets | Where-Object { $_.name -eq $assetExpectation[0] })
        if ($assets.Count -ne 1) { throw "linux-ci-environment: live Wasmtime release lacks '$($assetExpectation[0])'" }
        Assert-ExactValue $assets[0].digest $assetExpectation[1] "live Wasmtime asset $($assetExpectation[0]) digest"
    }
    Assert-ExactValue (Resolve-GitHubTagCommit -Repository "bytecodealliance/wasmtime" -Tag "v42.0.1") $contract.toolchains.wasmtime.release_commit "live Wasmtime release commit"
    Assert-ExactValue (Resolve-GitHubTagCommit -Repository "model-checking/kani" -Tag "kani-0.67.0") $contract.toolchains.kani.release_commit "live Kani release commit"
}

if ($Runtime) {
    if (-not $IsLinux -or [Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne [Runtime.InteropServices.Architecture]::X64) {
        throw "linux-ci-environment: runtime must be Linux x64"
    }
    foreach ($pair in @(
        @("LANG", "C.UTF-8"), @("LC_ALL", "C.UTF-8"), @("TZ", "UTC"),
        @("GITHUB_ACTIONS", "true"), @("CI", "true"), @("RUNNER_ENVIRONMENT", "github-hosted"),
        @("RUNNER_ARCH", "X64"), @("RUNNER_OS", "Linux"), @("ImageOS", "ubuntu24"),
        @("ImageVersion", "20260705.232")
    )) {
        Assert-ExactValue ([Environment]::GetEnvironmentVariable($pair[0])) $pair[1] "runtime environment $($pair[0])"
    }
    $unameSystem = (& uname -s).Trim()
    $unameMachine = (& uname -m).Trim()
    Assert-ExactValue $unameSystem "Linux" "runtime uname -s"
    Assert-ExactValue $unameMachine "x86_64" "runtime uname -m"

    $osRelease = @{}
    foreach ($line in @(Get-Content -LiteralPath "/etc/os-release")) {
        if ($line -match '^(?<key>[A-Z0-9_]+)=(?<value>.*)$') {
            $osRelease[$Matches.key] = $Matches.value.Trim('"')
        }
    }
    Assert-ExactValue $osRelease.ID $contract.target.os_release_id "runtime /etc/os-release ID"
    Assert-ExactValue $osRelease.VERSION_ID $contract.target.os_release_version "runtime /etc/os-release VERSION_ID"

    $rustcVerbose = (& rustc --version --verbose) -join "`n"
    if ($rustcVerbose -notmatch '(?m)^release: 1\.94\.1$' -or
        $rustcVerbose -notmatch '(?m)^commit-hash: e408947bfd200af42db322daf0fadfe7e26d3bd1$' -or
        $rustcVerbose -notmatch '(?m)^host: x86_64-unknown-linux-gnu$') {
        throw "linux-ci-environment: runtime rustc identity differs from the contract"
    }
    $cargoVerbose = (& cargo --version --verbose) -join "`n"
    if ($cargoVerbose -notmatch '(?m)^release: 1\.94\.1$' -or
        $cargoVerbose -notmatch '(?m)^commit-hash: 29ea6fb6a5db279426f4cc4e17aa385f05a0cfbc$' -or
        $cargoVerbose -notmatch '(?m)^host: x86_64-unknown-linux-gnu$') {
        throw "linux-ci-environment: runtime Cargo identity differs from the contract"
    }
    Assert-ExactValue $PSVersionTable.PSVersion.ToString() $contract.toolchains.powershell.version "runtime PowerShell version"

    if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP) -or -not [IO.Path]::IsPathRooted($env:RUNNER_TEMP)) {
        throw "linux-ci-environment: runtime RUNNER_TEMP must be an absolute owned state root"
    }
    $runnerTemp = [IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\', '/')
    $sentinel = Join-Path $runnerTemp ([string]$contract.determinism.retained_state_sentinel)
    if (Test-Path -LiteralPath $sentinel) {
        throw "linux-ci-environment: retained state sentinel already exists: $sentinel"
    }
    try {
        [IO.File]::WriteAllText($sentinel, "$($env:GITHUB_RUN_ID)/$($env:GITHUB_RUN_ATTEMPT)`n", $utf8)
        if (-not (Test-Path -LiteralPath $sentinel -PathType Leaf)) {
            throw "linux-ci-environment: could not create the owned state sentinel"
        }
    }
    finally {
        if (Test-Path -LiteralPath $sentinel -PathType Leaf) {
            Remove-Item -LiteralPath $sentinel -Force
        }
    }
    if (Test-Path -LiteralPath $sentinel) {
        throw "linux-ci-environment: owned state sentinel cleanup failed"
    }

    $dirtyTracked = @(& git -C $repoRoot status --porcelain --untracked-files=no)
    if ($LASTEXITCODE -ne 0 -or $dirtyTracked.Count -ne 0) {
        throw "linux-ci-environment: runtime checkout has tracked modifications before gates"
    }
}

$runtimeState = if ($Runtime) { "verified-current-process" } else { "not-requested" }
$externalState = if ($VerifyExternalProvenance) { "verified-live" } else { "recorded-offline" }
Write-Host "validate-linux-ci-environment: ok (contract=$($contract.contract_id) fixture_sha256=$contractSha ledger=$ledgerState runtime=$runtimeState external=$externalState)"
