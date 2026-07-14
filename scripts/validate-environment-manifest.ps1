param(
    [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
    [string]$IssuesPath = ".beads/issues.jsonl",
    [string]$AutorunPath = "docs/AUTORUN_STATE.md",
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
. (Join-Path $PSScriptRoot "lib-ideal-environment-capture.ps1")

function Test-PlaceholderValue {
    param([AllowEmptyString()][string]$Value)

    return [string]::IsNullOrWhiteSpace($Value) -or
        $Value -match '(?i)^(planned|pending|todo|tbd|unknown|unresolved|missing|not[- ]?pinned)(?:\b|[-_:])'
}

Push-Location $repoRoot
try {
    $manifest = (Read-IdealProgramManifest -RepoRoot $repoRoot -ManifestPath $ManifestPath).Manifest
    $issueContext = Read-IdealIssues -RepoRoot $repoRoot -IssuesPath $IssuesPath
    $issueById = $issueContext.IssueById
    $childrenByParent = New-IdealChildrenMap -Issues @($issueContext.Issues)
    $programIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    [void]$programIds.Add([string]$manifest.root_bead)
    foreach ($id in @(Get-IdealDescendantIds -RootId ([string]$manifest.root_bead) -ChildrenByParent $childrenByParent)) {
        [void]$programIds.Add($id)
    }

    Assert-IdealRelativePath -Path ([string]$manifest.environment_manifest) -Owner "manifest.environment_manifest"
    $environmentAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.environment_manifest)
    $expectedHeader = @(
        "environment_id",
        "role",
        "profile",
        "target_arch",
        "os_build",
        "office_product",
        "office_version",
        "office_build",
        "office_channel",
        "office_bitness",
        "locale",
        "snapshot_or_image",
        "reset_policy",
        "fixture_manifest",
        "fixture_hash",
        "owned_process_policy",
        "uia_modal_policy",
        "evidence_state",
        "owner_bead",
        "notes"
    )
    $actualHeader = @(Get-IdealCsvColumns -Path $environmentAbs)
    if (($expectedHeader -join ',') -ne ($actualHeader -join ',')) {
        throw "validate-environment-manifest: environment header must exactly match the V1 schema"
    }

    $rows = @(Import-Csv -LiteralPath $environmentAbs)
    if ($rows.Count -ne 3) {
        throw "validate-environment-manifest: expected exactly three environment roles, found $($rows.Count)"
    }
    $requiredRoles = @("dev-oracle", "certification-vm", "linux-ci")
    $roleByName = @{}
    $environmentById = @{}
    foreach ($row in $rows) {
        $environmentId = [string]$row.environment_id
        $role = [string]$row.role
        if ([string]::IsNullOrWhiteSpace($environmentId) -or $environmentById.ContainsKey($environmentId)) {
            throw "validate-environment-manifest: blank or duplicate environment_id '$environmentId'"
        }
        if ($role -notin $requiredRoles -or $roleByName.ContainsKey($role)) {
            throw "validate-environment-manifest: role '$role' is unknown or duplicated"
        }
        $environmentById[$environmentId] = $row
        $roleByName[$role] = $row

        foreach ($field in $expectedHeader) {
            if ([string]::IsNullOrWhiteSpace([string]$row.$field)) {
                throw "validate-environment-manifest: $environmentId has blank '$field'; use n/a only when inapplicable"
            }
        }
        if ([string]$row.target_arch -ne "x64") {
            throw "validate-environment-manifest: $environmentId target_arch must be x64"
        }
        $targetText = @($row.PSObject.Properties | ForEach-Object { [string]$_.Value }) -join ' '
        if ($targetText -match '(?i)(\bx86\b|\bi686\b|\bWOW64\b|\bARM64\b|32-bit Office|Office32)') {
            throw "validate-environment-manifest: $environmentId contains an excluded non-x64 target"
        }
        $ownerId = [string]$row.owner_bead
        if (-not $issueById.ContainsKey($ownerId) -or -not $programIds.Contains($ownerId)) {
            throw "validate-environment-manifest: $environmentId owner '$ownerId' is outside the current program"
        }
        if ([string]$row.evidence_state -in @("planned-blocking", "in-progress") -and
            [string]$issueById[$ownerId].status -notin @("open", "in_progress", "blocked")) {
            throw "validate-environment-manifest: blocking environment $environmentId has inactive owner '$ownerId'"
        }
        if ([string]$row.evidence_state -notin @("characterized-noncertifying", "planned-blocking", "in-progress", "verified")) {
            throw "validate-environment-manifest: $environmentId has invalid evidence_state '$($row.evidence_state)'"
        }
        if ([string]$row.owned_process_policy -notmatch '(?i)owned') {
            throw "validate-environment-manifest: $environmentId must state owned-process cleanup policy"
        }
    }
    foreach ($role in $requiredRoles) {
        if (-not $roleByName.ContainsKey($role)) {
            throw "validate-environment-manifest: missing required role '$role'"
        }
    }

    $dev = $roleByName["dev-oracle"]
    $cert = $roleByName["certification-vm"]
    $linux = $roleByName["linux-ci"]
    foreach ($windowsRow in @($dev, $cert)) {
        if ([string]$windowsRow.profile -ne "windows-x64") {
            throw "validate-environment-manifest: $($windowsRow.environment_id) must use profile windows-x64"
        }
        if ([string]$windowsRow.office_product -eq "n/a" -or
            [string]$windowsRow.office_version -eq "n/a" -or
            [string]$windowsRow.office_build -eq "n/a" -or
            [string]$windowsRow.office_channel -eq "n/a" -or
            [string]$windowsRow.office_bitness -ne "64") {
            throw "validate-environment-manifest: $($windowsRow.environment_id) must describe 64-bit Excel/Office"
        }
        if ([string]$windowsRow.uia_modal_policy -notmatch '(?i)(UIA|UI Automation).*(modal|dialog)') {
            throw "validate-environment-manifest: $($windowsRow.environment_id) must define owned Excel/VBE modal interception"
        }
    }
    if ([string]$dev.evidence_state -ne "characterized-noncertifying") {
        throw "validate-environment-manifest: dev-oracle must remain characterized-noncertifying"
    }
    if ([string]$dev.snapshot_or_image -notmatch '^(?:[A-Za-z0-9._-]+@)?sha256:[0-9a-f]{64}$' -or (Test-PlaceholderValue -Value ([string]$dev.snapshot_or_image))) {
        throw "validate-environment-manifest: dev-oracle must bind an immutable characterized host fingerprint"
    }
    Assert-IdealRelativePath -Path ([string]$dev.fixture_manifest) -Owner "dev-oracle fixture_manifest"
    $devFixturePath = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$dev.fixture_manifest)
    if (-not (Test-Path -LiteralPath $devFixturePath -PathType Leaf)) {
        throw "validate-environment-manifest: dev-oracle fixture manifest does not resolve"
    }
    $devFixtureFacts = Get-IdealCaptureFixtureFacts -RepositoryRoot $repoRoot -EnvironmentId ([string]$dev.environment_id) -FixtureManifestPath ([string]$dev.fixture_manifest)
    $devFixtureHash = [string]$devFixtureFacts.controlled_artifact_root_contract_sha256
    if ([string]$dev.fixture_hash -cne $devFixtureHash) {
        throw "validate-environment-manifest: dev-oracle fixture hash is forged or stale"
    }
    $devCaptureRelative = "docs/evidence/programs/$($manifest.program_id)/windows-x64/WIN-0/dev-oracle-environment.json"
    $devCapturePath = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $devCaptureRelative
    if (-not (Test-Path -LiteralPath $devCapturePath -PathType Leaf)) {
        throw "validate-environment-manifest: dev-oracle capture does not resolve at '$devCaptureRelative'"
    }
    $devCapture = ConvertFrom-WindowsFixtureAuditedJson `
        -Bytes ([IO.File]::ReadAllBytes($devCapturePath)) `
        -Owner "dev-oracle capture" `
        -FormatName "environment-capture"
    Assert-WindowsFixtureEnvironmentCaptureValue `
        -Capture $devCapture `
        -Environment $dev `
        -ExpectedSchema "oxvba-windows-x64-environment-capture-v1" `
        -Owner "dev-oracle capture"
    & (Join-Path $PSScriptRoot "sync-windows-dev-environment.ps1") -Check -RepositoryRoot $repoRoot
    $devControlledRelative = "artifacts/windows-x64/controlled-environments/v1/$([string]$dev.environment_id)/environment-capture.json"
    $devControlledHash = Get-WindowsFixtureCanonicalSourceFileHash -RepositoryRoot $repoRoot -RelativePath $devControlledRelative
    $devReportRelative = "docs/evidence/programs/$($manifest.program_id)/windows-x64/WIN-0/dev-oracle-environment.md"
    $devReportPath = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $devReportRelative
    if (-not (Test-Path -LiteralPath $devReportPath -PathType Leaf)) {
        throw "validate-environment-manifest: dev-oracle report does not resolve at '$devReportRelative'"
    }
    $devReportText = [IO.File]::ReadAllText($devReportPath).Replace("`r`n", "`n").Replace("`r", "`n")
    $preimagePattern = '(?s)<!-- oxvba-dev-host-fingerprint-preimage-v1-begin -->\s*```json\n(?<json>.*?)\n```\s*<!-- oxvba-dev-host-fingerprint-preimage-v1-end -->'
    $preimageMatches = @([regex]::Matches($devReportText, $preimagePattern))
    if ($preimageMatches.Count -ne 1 -or -not $devReportText.Contains('release=false', [StringComparison]::Ordinal)) {
        throw "validate-environment-manifest: dev-oracle report lacks one fingerprint preimage or release=false authority boundary"
    }
    $preimageJson = $preimageMatches[0].Groups['json'].Value
    $preimage = ConvertFrom-WindowsFixtureAuditedJson `
        -Bytes ([Text.UTF8Encoding]::new($false).GetBytes($preimageJson)) `
        -Owner "dev-oracle fingerprint preimage" `
        -FormatName "host-fingerprint"
    $preimageHash = Assert-IdealDevHostFingerprintPreimage `
        -Preimage $preimage `
        -Environment $dev `
        -Owner "dev-oracle fingerprint preimage"
    if ([string]$dev.snapshot_or_image -cne "dev-host-fingerprint-v1@$preimageHash") {
        throw "validate-environment-manifest: dev-oracle fingerprint preimage does not reproduce snapshot_or_image"
    }
    if ([string]$dev.snapshot_or_image -eq [string]$cert.snapshot_or_image) {
        throw "validate-environment-manifest: dev-oracle and certification-vm must be distinct hosts/images"
    }
    $certPolicy = "$($cert.snapshot_or_image) $($cert.reset_policy) $($cert.notes)"
    if ($certPolicy -notmatch '(?i)clean' -or $certPolicy -notmatch '(?i)(pinned|snapshot|image)' -or $certPolicy -notmatch '(?i)(reset|revert)') {
        throw "validate-environment-manifest: certification-vm must state clean pinned image and reset/revert policy"
    }
    if ([string]$linux.profile -ne "core") {
        throw "validate-environment-manifest: linux-ci must use profile core"
    }
    foreach ($field in @("office_product", "office_version", "office_build", "office_channel", "office_bitness")) {
        if ([string]$linux.$field -ne "n/a") {
            throw "validate-environment-manifest: linux-ci $field must be n/a"
        }
    }

    foreach ($terminal in @($cert, $linux)) {
        if ([string]$terminal.evidence_state -eq "verified") {
            if ([string]$terminal.environment_id -match '(?i)(planned|pending|unknown|unresolved)') {
                throw "validate-environment-manifest: verified $($terminal.role) retains provisional environment_id '$($terminal.environment_id)'"
            }
            $pinnedFields = @("os_build", "snapshot_or_image", "fixture_manifest", "fixture_hash")
            if ([string]$terminal.role -eq "certification-vm") {
                $pinnedFields += @("office_version", "office_build", "office_channel", "locale")
            }
            foreach ($field in $pinnedFields) {
                if (Test-PlaceholderValue -Value ([string]$terminal.$field)) {
                    throw "validate-environment-manifest: verified $($terminal.role) retains placeholder '$field=$($terminal.$field)'"
                }
            }
            $fixturePath = [string]$terminal.fixture_manifest
            Assert-IdealRelativePath -Path $fixturePath -Owner "$($terminal.role) fixture_manifest"
            if (-not (Test-Path -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $fixturePath) -PathType Leaf)) {
                throw "validate-environment-manifest: verified $($terminal.role) fixture manifest does not resolve"
            }
            if ([string]$terminal.fixture_hash -notmatch '(?i)^(?:sha256:)?[0-9a-f]{64}$') {
                throw "validate-environment-manifest: verified $($terminal.role) fixture_hash must be SHA-256"
            }
        }
    }

    $rootIssue = $issueById[[string]$manifest.root_bead]
    if ([string]$rootIssue.status -eq "closed") {
        foreach ($terminal in @($cert, $linux)) {
            if ([string]$terminal.evidence_state -ne "verified") {
                throw "validate-environment-manifest: closed program requires verified $($terminal.role)"
            }
        }
    }

    $ownershipRows = @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.matrix_ownership)))
    $certId = [string]$cert.environment_id
    $devConsumerRows = [Collections.Generic.List[object]]::new()
    foreach ($owner in $ownershipRows) {
        $matrixId = [string]$owner.matrix_id
        $isTerminalMatrix = ConvertFrom-IdealBoolean -Value ([string]$owner.required_for_terminal) -Owner "$matrixId required_for_terminal"
        $matrixRows = @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$owner.path)))
        foreach ($matrixRow in $matrixRows) {
            if ($matrixRow.PSObject.Properties.Name -contains "environment_id") {
                $rowEnvironmentId = [string]$matrixRow.environment_id
                if (-not [string]::IsNullOrWhiteSpace($rowEnvironmentId) -and
                    $rowEnvironmentId -ne "n/a" -and
                    -not $environmentById.ContainsKey($rowEnvironmentId)) {
                    throw "validate-environment-manifest: $matrixId/$($matrixRow.row_id) environment_id references unknown environment '$rowEnvironmentId'"
                }
                if ($rowEnvironmentId -ceq [string]$dev.environment_id) {
                    $devConsumerRows.Add([pscustomobject]@{ MatrixId = $matrixId; Row = $matrixRow })
                }
            }
            foreach ($match in @(Get-IdealTypedReferenceMatches -Text ([string]$matrixRow.evidence_refs) -Prefixes @("environment"))) {
                $environmentId = $match.Groups['value'].Value.Trim().Trim('`')
                if (-not $environmentById.ContainsKey($environmentId)) {
                    throw "validate-environment-manifest: $matrixId/$($matrixRow.row_id) references unknown environment '$environmentId'"
                }
            }
            $required = ConvertFrom-IdealBoolean -Value ([string]$matrixRow.required) -Owner "$matrixId/$($matrixRow.row_id) required"
            if (-not $isTerminalMatrix -or -not $required -or [string]$matrixRow.truth_state -ne "verified") {
                continue
            }
            if ($matrixId -ceq "WIN-ABI-CARRIER" -and [string]$matrixRow.row_id -ceq "WAC-TARGET-DEV-ENV") {
                # This is the noncertifying development-host control row. Its
                # exact authority is checked below; it must never be rewritten
                # to claim the clean certification environment.
                continue
            }
            $context = @(
                [string]$matrixRow.target_context,
                [string]$matrixRow.semantic_subset,
                [string]$matrixRow.capability,
                [string]$matrixRow.authority_refs
            ) -join ' '
            $requiresCertificationVm = [string]$owner.profile -eq "windows-x64" -or
                $matrixId -eq "EXCEL-ORACLE" -or
                $context -match '(?i)(Windows|Excel|Office|VBA oracle)'
            if (-not $requiresCertificationVm) {
                continue
            }
            $environmentMatches = @(Get-IdealTypedReferenceMatches -Text ([string]$matrixRow.evidence_refs) -Prefixes @("environment"))
            $environmentIds = @($environmentMatches | ForEach-Object { $_.Groups['value'].Value.Trim().Trim('`') })
            if ($environmentIds -notcontains $certId) {
                throw "validate-environment-manifest: terminal verified $matrixId/$($matrixRow.row_id) must reference environment:$certId"
            }
            if ([string]$cert.evidence_state -ne "verified") {
                throw "validate-environment-manifest: terminal verified $matrixId/$($matrixRow.row_id) uses an unverified certification-vm"
            }
            if ($matrixRow.PSObject.Properties.Name -contains "environment_id" -and
                -not [string]::IsNullOrWhiteSpace([string]$matrixRow.environment_id) -and
                [string]$matrixRow.environment_id -ne $certId) {
                throw "validate-environment-manifest: terminal verified $matrixId/$($matrixRow.row_id) environment_id must be $certId"
            }
        }
    }

    if ($devConsumerRows.Count -ne 12) {
        throw "validate-environment-manifest: expected exactly 12 development-environment consumers, found $($devConsumerRows.Count)"
    }
    foreach ($consumer in $devConsumerRows) {
        $key = "$([string]$consumer.MatrixId)/$([string]$consumer.Row.row_id)"
        if ([string]$consumer.Row.environment_hash -cne $devControlledHash) {
            throw "validate-environment-manifest: $key does not bind the controlled development-environment hash"
        }
    }
    $targetConsumers = @($devConsumerRows | Where-Object { [string]$_.MatrixId -ceq "WIN-ABI-CARRIER" -and [string]$_.Row.row_id -ceq "WAC-TARGET-DEV-ENV" })
    if ($targetConsumers.Count -ne 1) {
        throw "validate-environment-manifest: missing unique WAC-TARGET-DEV-ENV handoff row"
    }
    $target = $targetConsumers[0].Row
    if ([string]$target.metadata_revision -cne "win-x64-dev-oracle-2026-07-capture-v1" -or
        [string]$target.fixture_hash -cne "n/a" -or
        [string]$target.truth_state -cne "verified" -or
        [string]$target.evidence_state -cne "verified" -or
        [string]$target.evidence_owner_bead -cne "bd-59co.3.1.2" -or
        -not [string]::IsNullOrWhiteSpace([string]$target.residual_disposition) -or
        -not [string]::IsNullOrWhiteSpace([string]$target.residual_owner_bead)) {
        throw "validate-environment-manifest: WAC-TARGET-DEV-ENV is not the verified residual-free noncertifying handoff"
    }
    foreach ($backendState in @("compiler_state", "package_state", "vm3_state", "jit_state", "build_state")) {
        if ([string]$target.$backendState -cne "n/a") {
            throw "validate-environment-manifest: WAC-TARGET-DEV-ENV cannot credit backend state '$backendState'"
        }
    }
    $expectedTargetAnchors = "scripts/sync-windows-dev-environment.ps1|scripts/test-windows-dev-environment.ps1|scripts/validate-environment-manifest.ps1|scripts/validate-windows-fixture-manifest.ps1"
    if ([string]$target.test_anchors -cne $expectedTargetAnchors) {
        throw "validate-environment-manifest: WAC-TARGET-DEV-ENV test anchors drifted"
    }
    $targetEvidence = @(([string]$target.evidence_refs -split '[;|]') | ForEach-Object { $_.Trim().Replace('\', '/') } | Where-Object { $_ })
    $expectedTargetEvidence = @(
        "environment:$([string]$dev.environment_id)",
        "artifact:$devControlledRelative",
        "artifact:$devCaptureRelative",
        "artifact:docs/evidence/programs/$($manifest.program_id)/windows-x64/WIN-0/environment-and-owner-handoff.md",
        "observables:result=verified,full-err=n/a,side-effects=verified,lifecycle-order=verified,transport=verified,balance=verified"
    )
    $actualTargetEvidenceSet = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($entry in $targetEvidence) { [void]$actualTargetEvidenceSet.Add($entry) }
    $expectedTargetEvidenceSet = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($entry in $expectedTargetEvidence) { [void]$expectedTargetEvidenceSet.Add($entry) }
    if ($targetEvidence.Count -ne $expectedTargetEvidence.Count -or
        $actualTargetEvidenceSet.Count -ne $targetEvidence.Count -or
        -not $actualTargetEvidenceSet.SetEquals($expectedTargetEvidenceSet)) {
        throw "validate-environment-manifest: WAC-TARGET-DEV-ENV evidence set must be exact and case-sensitive"
    }

    Write-Host "validate-environment-manifest: ok (program=$($manifest.program_id) roles=3 dev_consumers=12 dev_hash=$devControlledHash cert=$certId linux=$($linux.environment_id))"
}
finally {
    Pop-Location
}
