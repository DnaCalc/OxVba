param(
    [string]$ManifestPath = "docs/validation/IDEAL_WINDOWS_X64_FIXTURE_MANIFEST_V1.csv",
    [string]$IssuesPath = ".beads/issues.jsonl",
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

function Assert-WindowsFixtureActiveOwner {
    param(
        [Parameter(Mandatory = $true)][string]$OwnerId,
        [Parameter(Mandatory = $true)][hashtable]$IssueById,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ([string]::IsNullOrWhiteSpace($OwnerId) -or $OwnerId -eq "n/a" -or -not $IssueById.ContainsKey($OwnerId)) {
        throw "validate-windows-fixture-manifest: $Owner has missing or unknown pending owner '$OwnerId'"
    }
    if ([string]$IssueById[$OwnerId].status -notin @("open", "in_progress", "blocked")) {
        throw "validate-windows-fixture-manifest: $Owner pending owner '$OwnerId' is not active"
    }
}

function Assert-WindowsFixtureControlledSourcePath {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ($Path -match '(?i)^(?:docs/evidence|synthesis|target|\.git|\.beads)(?:/|$)') {
        throw "validate-windows-fixture-manifest: $Owner source path '$Path' is historical, generated, or mutable"
    }
    if ($Path -match '(?i)\.(?:dll|exe|tlb|lib|obj|pdb|ilk|xlsm|xlsx|xls|zip|7z)$') {
        throw "validate-windows-fixture-manifest: $Owner source path '$Path' is a binary/artifact and cannot receive source/recipe credit"
    }
}

function Assert-WindowsFixtureCurrentControlledPathPolicy {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ExpectedPath,
        [Parameter(Mandatory = $true)][ValidateSet("artifact", "environment capture")][string]$Kind,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    Assert-IdealRelativePath -Path $Path -Owner "$Owner current $Kind path"
    $normalized = $Path.Trim().Replace('\', '/')
    if (Test-WindowsFixtureMutableIdentity -Value $normalized) {
        throw "validate-windows-fixture-manifest: $Owner current $Kind path '$Path' uses a mutable alias"
    }
    if ($normalized -match '(?i)^(?:docs/evidence|docs/generated|synthesis|target|\.external|\.git|\.beads)(?:/|$)' -or
        $normalized -match '(?i)(?:^|/)(?:archive|historical|old)(?:/|$)') {
        throw "validate-windows-fixture-manifest: $Owner current $Kind path '$Path' is historical or generated"
    }
    if ($normalized -cne $ExpectedPath) {
        throw "validate-windows-fixture-manifest: $Owner current $Kind path must equal immutable controlled path '$ExpectedPath'"
    }
}

Push-Location $repoRoot
try {
    & (Join-Path $PSScriptRoot "validate-windows-x64-control-surfaces.ps1") -RepositoryRoot $repoRoot

    Assert-IdealRelativePath -Path $ManifestPath -Owner "Windows fixture manifest path"
    $manifestAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $ManifestPath
    if (-not (Test-Path -LiteralPath $manifestAbs -PathType Leaf)) {
        throw "validate-windows-fixture-manifest: missing canonical manifest '$ManifestPath'"
    }

    $expectedHeader = @(Get-WindowsFixtureManifestHeader)
    $actualHeader = @(Get-IdealCsvColumns -Path $manifestAbs)
    if (($expectedHeader -join ',') -cne ($actualHeader -join ',')) {
        throw "validate-windows-fixture-manifest: manifest header must exactly match the V1 schema"
    }

    $rows = @(Import-Csv -LiteralPath $manifestAbs)
    if ($rows.Count -ne 57) {
        throw "validate-windows-fixture-manifest: expected exactly 57 rows, found $($rows.Count)"
    }
    $expectedRows = @(New-WindowsFixtureManifestRows -RepositoryRoot $repoRoot)
    if ($expectedRows.Count -ne 57) {
        throw "validate-windows-fixture-manifest: internal generated contract expected 57 rows, found $($expectedRows.Count)"
    }
    $expectedByKey = @{}
    foreach ($expectedRow in $expectedRows) {
        $expectedByKey["$([string]$expectedRow.matrix_id)|$([string]$expectedRow.row_id)"] = $expectedRow
    }

    $issues = Read-IdealIssues -RepoRoot $repoRoot -IssuesPath $IssuesPath
    $issueById = $issues.IssueById
    $environmentRows = @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv"))
    $environmentById = @{}
    foreach ($environment in $environmentRows) {
        $environmentId = [string]$environment.environment_id
        if ($environmentById.ContainsKey($environmentId)) {
            throw "validate-windows-fixture-manifest: duplicate canonical environment '$environmentId'"
        }
        $environmentById[$environmentId] = $environment
    }
    $seenKeys = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seenRowIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seenFixtureIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seenRecipeIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seenArtifactIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $forbiddenIdentityPattern = '(?i)(?:^|[-_:])(?:latest|current|rolling|mutable|nightly|head|tip)(?:$|[-_:])'
    $forbiddenTargetPattern = '(?i)(?<![A-Z0-9])x86(?![_-]64(?:$|[^A-Z0-9]))(?![A-Z0-9])|(?<![A-Z0-9])i686(?![A-Z0-9])|(?<![A-Z0-9])WOW64(?![A-Z0-9])|(?<![A-Z0-9])(?:ARM64(?:EC)?|AARCH64)(?![A-Z0-9])|(?<![A-Z0-9])32[- ]?bit[- ]*(?:Windows|Office|Excel|process|artifact|binary|target|host)(?![A-Z0-9])|(?<![A-Z0-9])(?:Office32|Excel32|process32|artifact32)(?![A-Z0-9])'

    foreach ($row in $rows) {
        $matrixId = [string]$row.matrix_id
        $rowId = [string]$row.row_id
        $key = "$matrixId|$rowId"
        if (-not $seenKeys.Add($key)) {
            throw "validate-windows-fixture-manifest: duplicate matrix/row entry '$key'"
        }
        if (-not $seenRowIds.Add($rowId)) {
            throw "validate-windows-fixture-manifest: duplicate row_id '$rowId'"
        }
        if (-not $seenFixtureIds.Add([string]$row.fixture_id)) {
            throw "validate-windows-fixture-manifest: duplicate fixture_id '$($row.fixture_id)'"
        }
        if (-not $seenRecipeIds.Add([string]$row.recipe_id)) {
            throw "validate-windows-fixture-manifest: duplicate recipe_id '$($row.recipe_id)'"
        }
        if (-not $seenArtifactIds.Add([string]$row.built_artifact_id)) {
            throw "validate-windows-fixture-manifest: duplicate built_artifact_id '$($row.built_artifact_id)'"
        }
        if (-not $expectedByKey.ContainsKey($key)) {
            throw "validate-windows-fixture-manifest: unexpected or unowned Windows row '$key'"
        }
        $expected = $expectedByKey[$key]

        foreach ($field in $expectedHeader) {
            if ([string]::IsNullOrWhiteSpace([string]$row.$field)) {
                throw "validate-windows-fixture-manifest: row '$key' has blank '$field'"
            }
        }
        if ([string]$row.target_arch -ne "x64") {
            throw "validate-windows-fixture-manifest: row '$key' target_arch must be x64"
        }
        if ([string]$row.office_bitness -notin @("64", "n/a")) {
            throw "validate-windows-fixture-manifest: row '$key' office_bitness must be 64 or n/a"
        }
        if ([string]$row.fixture_id -notmatch '^[a-z0-9]+(?:-[a-z0-9]+)*-v[1-9][0-9]*$') {
            throw "validate-windows-fixture-manifest: row '$key' fixture_id is not immutable and versioned"
        }
        if ([string]$row.recipe_id -cne "x64-$([string]$row.fixture_id)-recipe-v1" -or
            [string]$row.built_artifact_id -cne "x64-$([string]$row.fixture_id)-artifact-v1") {
            throw "validate-windows-fixture-manifest: row '$key' recipe/artifact identity is not the immutable x64 identity"
        }
        foreach ($field in @(
            "built_artifact_class", "built_artifact_root", "built_artifact_name",
            "built_artifact_type", "built_artifact_components"
        )) {
            if ([string]$row.$field -cne [string]$expected.$field) {
                throw "validate-windows-fixture-manifest: row '$key' field '$field' differs from its explicit artifact contract"
            }
        }
        if ([string]$row.built_artifact_root -notmatch '^artifacts/windows-x64/controlled-fixtures/v1/[a-z0-9-]+/[a-z0-9-]+$' -or
            (Test-WindowsFixtureMutableIdentity -Value ([string]$row.built_artifact_root))) {
            throw "validate-windows-fixture-manifest: row '$key' artifact root is not immutable and controlled"
        }
        switch ([string]$row.built_artifact_class) {
            "pe-dll-x64" {
                if ([string]$row.built_artifact_name -ne "fixture.dll" -or
                    [string]$row.built_artifact_type -ne "pe32plus-amd64-dll" -or
                    [string]$row.built_artifact_components -ne "n/a") {
                    throw "validate-windows-fixture-manifest: row '$key' PE DLL artifact contract is malformed"
                }
            }
            "pe-exe-x64" {
                if ([string]$row.built_artifact_name -ne "fixture.exe" -or
                    [string]$row.built_artifact_type -ne "pe32plus-amd64-exe" -or
                    [string]$row.built_artifact_components -ne "n/a") {
                    throw "validate-windows-fixture-manifest: row '$key' PE EXE artifact contract is malformed"
                }
            }
            "fixture-bundle-json-v1" {
                if ([string]$row.built_artifact_name -ne "fixture-bundle.json" -or
                    [string]$row.built_artifact_type -ne "oxvba-windows-x64-fixture-bundle-v1" -or
                    [string]$row.built_artifact_components -eq "n/a") {
                    throw "validate-windows-fixture-manifest: row '$key' fixture-bundle artifact contract is malformed"
                }
            }
            default {
                throw "validate-windows-fixture-manifest: row '$key' has unsupported built_artifact_class '$($row.built_artifact_class)'"
            }
        }
        foreach ($field in @(
            "environment_role", "environment_profile", "environment_target_arch",
            "environment_office_bitness", "environment_evidence_state",
            "environment_capture_root", "environment_capture_name", "environment_capture_schema"
        )) {
            if ([string]$row.$field -cne [string]$expected.$field) {
                throw "validate-windows-fixture-manifest: row '$key' field '$field' differs from its canonical environment contract"
            }
        }
        if ([string]$row.environment_profile -ne "windows-x64" -or
            [string]$row.environment_target_arch -ne "x64" -or
            [string]$row.environment_office_bitness -ne "64" -or
            [string]$row.environment_role -notin @("dev-oracle", "certification-vm") -or
            [string]$row.environment_capture_root -notmatch '^artifacts/windows-x64/controlled-environments/v1/[a-z0-9-]+$' -or
            [string]$row.environment_capture_name -ne "environment-capture.json" -or
            [string]$row.environment_capture_schema -ne "oxvba-windows-x64-environment-capture-v1") {
            throw "validate-windows-fixture-manifest: row '$key' environment contract is not a versioned Windows x64 Office64 capture"
        }
        foreach ($identityField in @("fixture_id", "recipe_id", "built_artifact_id", "environment_id")) {
            $identity = [string]$row.$identityField
            if ($identity -match $forbiddenIdentityPattern) {
                throw "validate-windows-fixture-manifest: row '$key' $identityField '$identity' is mutable"
            }
            if ($identity -match $forbiddenTargetPattern) {
                throw "validate-windows-fixture-manifest: row '$key' $identityField '$identity' is non-x64"
            }
        }
        if (-not $environmentById.ContainsKey([string]$row.environment_id)) {
            throw "validate-windows-fixture-manifest: row '$key' references unknown canonical environment '$($row.environment_id)'"
        }
        if ([string]$row.execution_recipe -notlike "target=x64; office-bitness=$([string]$row.office_bitness); fixture=$([string]$row.fixture_id); process=$([string]$row.process_shape); apartment=$([string]$row.apartment_shape);*" -or
            [string]$row.execution_recipe -notmatch 'capture=six-axis\(result,full-Err,side-effects,lifecycle-order,transport,balance\)' -or
            [string]$row.execution_recipe -notmatch 'ownership=record-before-mutation') {
            throw "validate-windows-fixture-manifest: row '$key' execution recipe does not pin x64 fixture/process/apartment and six-axis capture"
        }
        if ([string]$row.cleanup_recipe -notmatch '(?i)owned' -or
            [string]$row.cleanup_recipe -notmatch '(?i)(only recorded|only owned)' -or
            [string]$row.cleanup_recipe -match '(?i)blanket') {
            throw "validate-windows-fixture-manifest: row '$key' cleanup recipe is not owned and scoped"
        }
        if ([string]$row.capability_credit -ne "none") {
            throw "validate-windows-fixture-manifest: row '$key' capability_credit must be none"
        }

        switch ([string]$row.source_recipe_state) {
            "current" {
                if ([string]$row.source_recipe_hash -notmatch '^sha256:[0-9a-f]{64}$') {
                    throw "validate-windows-fixture-manifest: row '$key' current source_recipe_hash is malformed"
                }
                if ([string]$row.source_recipe_owner_bead -ne "n/a") {
                    throw "validate-windows-fixture-manifest: row '$key' current source recipe must not retain a pending owner"
                }
                $rawPaths = @(([string]$row.source_recipe_paths -split '\|') | ForEach-Object { $_.Trim() })
                $normalizedPaths = @(ConvertTo-WindowsFixtureNormalizedRelativePaths -Paths $rawPaths)
                if (($rawPaths -join '|') -cne ($normalizedPaths -join '|')) {
                    throw "validate-windows-fixture-manifest: row '$key' source_recipe_paths must be normalized, sorted, and deduplicated"
                }
                foreach ($path in $normalizedPaths) {
                    Assert-WindowsFixtureControlledSourcePath -Path $path -Owner "row '$key'"
                    $absolute = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $path
                    if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) {
                        throw "validate-windows-fixture-manifest: row '$key' source path '$path' does not resolve"
                    }
                }
                $actualSourceHash = Get-WindowsFixtureSourceRecipeHash -RepositoryRoot $repoRoot -Row $row -SourcePaths $normalizedPaths
                if ([string]$row.source_recipe_hash -cne $actualSourceHash) {
                    throw "validate-windows-fixture-manifest: row '$key' source_recipe_hash is forged or stale"
                }
            }
            "pending" {
                if ([string]$row.source_recipe_paths -ne "pending" -or [string]$row.source_recipe_hash -ne "pending") {
                    throw "validate-windows-fixture-manifest: row '$key' pending source recipe must use pending path and hash"
                }
                Assert-WindowsFixtureActiveOwner -OwnerId ([string]$row.source_recipe_owner_bead) -IssueById $issueById -Owner "row '$key' source recipe"
            }
            "not-applicable" {
                if ($key -cne "WIN-ABI-CARRIER|WAC-TARGET-DEV-ENV" -or
                    [string]$row.source_recipe_paths -cne "n/a" -or
                    [string]$row.source_recipe_hash -cne "n/a" -or
                    [string]$row.source_recipe_owner_bead -cne "n/a") {
                    throw "validate-windows-fixture-manifest: only the environment-only target control may omit a source recipe"
                }
            }
            default {
                throw "validate-windows-fixture-manifest: row '$key' has invalid source_recipe_state '$($row.source_recipe_state)'"
            }
        }

        switch ([string]$row.built_artifact_state) {
            "current" {
                if ([string]$row.built_artifact_hash -notmatch '^sha256:[0-9a-f]{64}$' -or [string]$row.built_artifact_owner_bead -ne "n/a") {
                    throw "validate-windows-fixture-manifest: row '$key' current built artifact hash/owner is malformed"
                }
                $artifactPath = ([string]$row.built_artifact_path).Trim().Replace('\', '/')
                $expectedArtifactPath = "$([string]$row.built_artifact_root)/$([string]$row.built_artifact_name)"
                Assert-WindowsFixtureCurrentControlledPathPolicy `
                    -Path $artifactPath `
                    -ExpectedPath $expectedArtifactPath `
                    -Kind "artifact" `
                    -Owner "row '$key'"
                $artifactAbsolute = Assert-WindowsFixtureContainedPath `
                    -RepositoryRoot $repoRoot `
                    -RelativePath $artifactPath `
                    -ControlledRoot ([string]$row.built_artifact_root) `
                    -Owner "row '$key' built artifact"
                switch ([string]$row.built_artifact_class) {
                    "pe-dll-x64" {
                        Assert-WindowsFixturePeFile -Path $artifactAbsolute -ExpectedKind "pe-dll-x64" -Owner "row '$key' built artifact"
                    }
                    "pe-exe-x64" {
                        Assert-WindowsFixturePeFile -Path $artifactAbsolute -ExpectedKind "pe-exe-x64" -Owner "row '$key' built artifact"
                    }
                    "fixture-bundle-json-v1" {
                        Assert-WindowsFixtureBundleArtifact `
                            -RepositoryRoot $repoRoot `
                            -RelativePath $artifactPath `
                            -ArtifactRoot ([string]$row.built_artifact_root) `
                            -MatrixId $matrixId `
                            -RowId $rowId `
                            -FixtureId ([string]$row.fixture_id) `
                            -ArtifactId ([string]$row.built_artifact_id) `
                            -ExpectedComponents ([string]$row.built_artifact_components) `
                            -ExpectedComponentConstraints ([string]$expected.built_artifact_component_constraints) `
                            -Owner "row '$key' built artifact"
                    }
                }
                $actualArtifactHash = Get-WindowsFixtureRawFileHash -RepositoryRoot $repoRoot -RelativePath $artifactPath
                if ([string]$row.built_artifact_hash -cne $actualArtifactHash) {
                    throw "validate-windows-fixture-manifest: row '$key' built_artifact_hash is forged or stale"
                }
            }
            "pending" {
                if ([string]$row.built_artifact_path -ne "pending" -or [string]$row.built_artifact_hash -ne "pending") {
                    throw "validate-windows-fixture-manifest: row '$key' pending built artifact must use pending path and hash"
                }
                Assert-WindowsFixtureActiveOwner -OwnerId ([string]$row.built_artifact_owner_bead) -IssueById $issueById -Owner "row '$key' built artifact"
            }
            "not-applicable" {
                if ($key -cne "WIN-ABI-CARRIER|WAC-TARGET-DEV-ENV" -or
                    [string]$row.built_artifact_path -cne "n/a" -or
                    [string]$row.built_artifact_hash -cne "n/a" -or
                    [string]$row.built_artifact_owner_bead -cne "n/a") {
                    throw "validate-windows-fixture-manifest: only the environment-only target control may omit a built artifact"
                }
            }
            default {
                throw "validate-windows-fixture-manifest: row '$key' has invalid built_artifact_state '$($row.built_artifact_state)'"
            }
        }

        switch ([string]$row.environment_state) {
            "current" {
                if ([string]$row.environment_hash -notmatch '^sha256:[0-9a-f]{64}$' -or [string]$row.environment_owner_bead -ne "n/a") {
                    throw "validate-windows-fixture-manifest: row '$key' current environment hash/owner is malformed"
                }
                $capturePath = ([string]$row.environment_capture_path).Trim().Replace('\', '/')
                $expectedCapturePath = "$([string]$row.environment_capture_root)/$([string]$row.environment_capture_name)"
                Assert-WindowsFixtureCurrentControlledPathPolicy `
                    -Path $capturePath `
                    -ExpectedPath $expectedCapturePath `
                    -Kind "environment capture" `
                    -Owner "row '$key'"
                Assert-WindowsFixtureEnvironmentCapture `
                    -RepositoryRoot $repoRoot `
                    -RelativePath $capturePath `
                    -CaptureRoot ([string]$row.environment_capture_root) `
                    -Environment $environmentById[[string]$row.environment_id] `
                    -ExpectedSchema ([string]$row.environment_capture_schema) `
                    -Owner "row '$key' environment capture"
                $captureHash = Get-WindowsFixtureCanonicalSourceFileHash -RepositoryRoot $repoRoot -RelativePath $capturePath
                if ([string]$row.environment_hash -cne $captureHash) {
                    throw "validate-windows-fixture-manifest: row '$key' environment_hash is forged or stale"
                }
            }
            "pending" {
                if ([string]$row.environment_capture_path -ne "pending" -or [string]$row.environment_hash -ne "pending") {
                    throw "validate-windows-fixture-manifest: row '$key' pending environment must use pending capture and hash"
                }
                Assert-WindowsFixtureActiveOwner -OwnerId ([string]$row.environment_owner_bead) -IssueById $issueById -Owner "row '$key' environment"
            }
            default {
                throw "validate-windows-fixture-manifest: row '$key' has invalid environment_state '$($row.environment_state)'"
            }
        }

        foreach ($field in $expectedHeader) {
            if ([string]$row.$field -cne [string]$expected.$field) {
                throw "validate-windows-fixture-manifest: row '$key' field '$field' differs from the controlled generated recipe"
            }
        }
    }

    $missingKeys = @($expectedByKey.Keys | Where-Object { -not $seenKeys.Contains($_) })
    if ($missingKeys.Count -gt 0) {
        throw "validate-windows-fixture-manifest: missing required rows '$($missingKeys -join '|')'"
    }

    $sourceCurrent = @($rows | Where-Object source_recipe_state -eq "current").Count
    $sourcePending = @($rows | Where-Object source_recipe_state -eq "pending").Count
    $sourceNotApplicable = @($rows | Where-Object source_recipe_state -eq "not-applicable").Count
    $builtPending = @($rows | Where-Object built_artifact_state -eq "pending").Count
    $builtNotApplicable = @($rows | Where-Object built_artifact_state -eq "not-applicable").Count
    $environmentPending = @($rows | Where-Object environment_state -eq "pending").Count
    Write-Host "validate-windows-fixture-manifest: ok (matrices=6 rows=57 target=x64 source_current=$sourceCurrent source_pending=$sourcePending source_n_a=$sourceNotApplicable built_pending=$builtPending built_n_a=$builtNotApplicable environment_pending=$environmentPending capability_credit=none)"
}
finally {
    Pop-Location
}
