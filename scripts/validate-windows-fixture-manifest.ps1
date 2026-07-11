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

function Assert-WindowsFixtureCurrentArtifactPath {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ($Path -match '(?i)^(?:docs/evidence|synthesis|target|\.external|\.git|\.beads)(?:/|$)' -or
        $Path -match '(?i)(?:^|/)(?:archive|historical|old|latest|current)(?:/|$)') {
        throw "validate-windows-fixture-manifest: $Owner current artifact path '$Path' is historical or mutable"
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
        foreach ($identityField in @("fixture_id", "recipe_id", "built_artifact_id", "environment_id")) {
            $identity = [string]$row.$identityField
            if ($identity -match $forbiddenIdentityPattern) {
                throw "validate-windows-fixture-manifest: row '$key' $identityField '$identity' is mutable"
            }
            if ($identity -match $forbiddenTargetPattern) {
                throw "validate-windows-fixture-manifest: row '$key' $identityField '$identity' is non-x64"
            }
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
                Assert-IdealRelativePath -Path $artifactPath -Owner "row '$key' built artifact path"
                Assert-WindowsFixtureCurrentArtifactPath -Path $artifactPath -Owner "row '$key'"
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
                Assert-IdealRelativePath -Path $capturePath -Owner "row '$key' environment capture path"
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
    $builtPending = @($rows | Where-Object built_artifact_state -eq "pending").Count
    $environmentPending = @($rows | Where-Object environment_state -eq "pending").Count
    Write-Host "validate-windows-fixture-manifest: ok (matrices=6 rows=57 target=x64 source_current=$sourceCurrent source_pending=$sourcePending built_pending=$builtPending environment_pending=$environmentPending capability_credit=none)"
}
finally {
    Pop-Location
}
