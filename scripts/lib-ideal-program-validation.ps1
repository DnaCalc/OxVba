Set-StrictMode -Version Latest

function Resolve-IdealRepoPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    if ([IO.Path]::IsPathRooted($Path)) {
        return [IO.Path]::GetFullPath($Path)
    }
    return [IO.Path]::GetFullPath((Join-Path $RepoRoot $Path))
}

function Read-IdealProgramManifest {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,
        [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json"
    )

    $manifestAbs = Resolve-IdealRepoPath -RepoRoot $RepoRoot -Path $ManifestPath
    if (-not (Test-Path -LiteralPath $manifestAbs -PathType Leaf)) {
        throw "ideal-program: missing manifest $ManifestPath"
    }

    $manifest = Get-Content -LiteralPath $manifestAbs -Raw | ConvertFrom-Json
    foreach ($field in @(
        "schema_version",
        "program_id",
        "program_label",
        "root_bead",
        "control_epic",
        "matrix_ownership",
        "matrix_schema",
        "bead_traceability",
        "legacy_migration",
        "profiles"
    )) {
        if ($manifest.PSObject.Properties.Name -notcontains $field) {
            throw "ideal-program: manifest is missing '$field'"
        }
    }

    if ([int]$manifest.schema_version -ne 1) {
        throw "ideal-program: unsupported manifest schema_version '$($manifest.schema_version)'"
    }
    if ([string]::IsNullOrWhiteSpace([string]$manifest.program_id) -or
        [string]::IsNullOrWhiteSpace([string]$manifest.program_label) -or
        [string]::IsNullOrWhiteSpace([string]$manifest.root_bead) -or
        [string]::IsNullOrWhiteSpace([string]$manifest.control_epic)) {
        throw "ideal-program: manifest program identity is incomplete"
    }

    $profiles = @($manifest.profiles)
    if ($profiles.Count -ne 3) {
        throw "ideal-program: expected exactly 3 profiles, found $($profiles.Count)"
    }
    $expectedProfileNames = @("core", "windows-x64", "ide")
    $actualProfileNames = @($profiles | ForEach-Object { [string]$_.profile } | Sort-Object)
    if (@(Compare-Object ($expectedProfileNames | Sort-Object) $actualProfileNames).Count -ne 0) {
        throw "ideal-program: expected profiles [$($expectedProfileNames -join ',')], found [$($actualProfileNames -join ',')]"
    }

    $expectedCodesByProfile = @{
        "core" = @("CORE-0", "CORE-1", "CORE-2", "CORE-3", "CORE-4", "CORE-5", "CORE-6", "CORE-7", "CORE-8", "CORE-9", "CORE-10", "CORE-LIB")
        "windows-x64" = @(0..15 | ForEach-Object { "WIN-$_" })
        "ide" = @(0..13 | ForEach-Object { "LS-$_" })
    }
    $expectedClauseByProfile = @{
        "core" = "PROFILE-CORE-001"
        "windows-x64" = "PROFILE-WIN-001"
        "ide" = "PROFILE-IDE-001"
    }
    $seenRoots = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seenEpicIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seenCodes = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($profile in $profiles) {
        $profileName = [string]$profile.profile
        foreach ($field in @("profile_clause", "workset_root", "workset_doc", "expected_epics")) {
            if ($profile.PSObject.Properties.Name -notcontains $field) {
                throw "ideal-program: profile '$profileName' is missing '$field'"
            }
        }
        if ([string]$profile.profile_clause -ne $expectedClauseByProfile[$profileName]) {
            throw "ideal-program: profile '$profileName' must use clause '$($expectedClauseByProfile[$profileName])'"
        }
        $worksetRoot = [string]$profile.workset_root
        if ([string]::IsNullOrWhiteSpace($worksetRoot) -or -not $seenRoots.Add($worksetRoot)) {
            throw "ideal-program: profile '$profileName' has a blank or duplicate workset root '$worksetRoot'"
        }
        if ($worksetRoot -in @([string]$manifest.root_bead, [string]$manifest.control_epic)) {
            throw "ideal-program: profile '$profileName' workset root overlaps the program/control root"
        }
        $epics = @($profile.expected_epics)
        $actualCodes = @()
        foreach ($epic in $epics) {
            $code = [string]$epic.code
            $bead = [string]$epic.bead
            if ([string]::IsNullOrWhiteSpace($code) -or -not $seenCodes.Add($code)) {
                throw "ideal-program: profile '$profileName' has a blank or duplicate epic code '$code'"
            }
            if ([string]::IsNullOrWhiteSpace($bead) -or -not $seenEpicIds.Add($bead)) {
                throw "ideal-program: profile '$profileName' has a blank or duplicate epic bead '$bead'"
            }
            $actualCodes += $code
        }
        if (@(Compare-Object -ReferenceObject @($expectedCodesByProfile[$profileName] | Sort-Object) -DifferenceObject @($actualCodes | Sort-Object)).Count -ne 0) {
            throw "ideal-program: profile '$profileName' epic code set does not match the V1 program contract"
        }
    }
    if ($seenEpicIds.Count -ne 42) {
        throw "ideal-program: expected 42 unique execution epic ids, found $($seenEpicIds.Count)"
    }

    return [pscustomobject]@{
        Manifest = $manifest
        ManifestPath = $ManifestPath.Replace('\', '/')
        ManifestAbs = $manifestAbs
    }
}

function Read-IdealIssues {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,
        [string]$IssuesPath = ".beads/issues.jsonl"
    )

    $issuesAbs = Resolve-IdealRepoPath -RepoRoot $RepoRoot -Path $IssuesPath
    if (-not (Test-Path -LiteralPath $issuesAbs -PathType Leaf)) {
        throw "ideal-program: missing bead export $IssuesPath"
    }

    $issues = @(
        Get-Content -LiteralPath $issuesAbs |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
            ForEach-Object { $_ | ConvertFrom-Json }
    )
    $issueById = @{}
    foreach ($issue in $issues) {
        $id = [string]$issue.id
        if ([string]::IsNullOrWhiteSpace($id)) {
            throw "ideal-program: bead export contains an issue without an id"
        }
        if ($issueById.ContainsKey($id)) {
            throw "ideal-program: bead export contains duplicate id '$id'"
        }
        $issueById[$id] = $issue
    }

    return [pscustomobject]@{
        Issues = $issues
        IssueById = $issueById
        IssuesPath = $IssuesPath.Replace('\', '/')
        IssuesAbs = $issuesAbs
    }
}

function Get-IdealParentIds {
    param([Parameter(Mandatory = $true)]$Issue)

    $parents = @()
    if ($Issue.PSObject.Properties.Name -contains "dependencies" -and $null -ne $Issue.dependencies) {
        $parents = @(
            $Issue.dependencies |
                Where-Object { [string]$_.type -eq "parent-child" } |
                ForEach-Object { [string]$_.depends_on_id }
        )
    }
    return $parents
}

function New-IdealChildrenMap {
    param([Parameter(Mandatory = $true)][object[]]$Issues)

    $childrenByParent = @{}
    foreach ($issue in $Issues) {
        foreach ($parentId in @(Get-IdealParentIds -Issue $issue)) {
            if (-not $childrenByParent.ContainsKey($parentId)) {
                $childrenByParent[$parentId] = [Collections.Generic.List[object]]::new()
            }
            $childrenByParent[$parentId].Add($issue)
        }
    }
    return $childrenByParent
}

function Get-IdealDescendantIds {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RootId,
        [Parameter(Mandatory = $true)]
        [hashtable]$ChildrenByParent
    )

    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $queue = [Collections.Generic.Queue[string]]::new()
    $queue.Enqueue($RootId)
    while ($queue.Count -gt 0) {
        $parentId = $queue.Dequeue()
        if (-not $ChildrenByParent.ContainsKey($parentId)) {
            continue
        }
        foreach ($child in @($ChildrenByParent[$parentId])) {
            $childId = [string]$child.id
            if ($seen.Add($childId)) {
                $queue.Enqueue($childId)
            }
        }
    }
    return @($seen)
}

function Get-IdealIssueLabels {
    param([Parameter(Mandatory = $true)]$Issue)

    if ($Issue.PSObject.Properties.Name -notcontains "labels" -or $null -eq $Issue.labels) {
        return @()
    }
    return @($Issue.labels | ForEach-Object { [string]$_ })
}

function Get-IdealExpectedEpicRecords {
    param([Parameter(Mandatory = $true)]$Manifest)

    $records = @()
    foreach ($profile in @($Manifest.profiles)) {
        foreach ($epic in @($profile.expected_epics)) {
            $records += [pscustomobject]@{
                Profile = [string]$profile.profile
                ProfileClause = [string]$profile.profile_clause
                ProfileLabel = switch ([string]$profile.profile) {
                    "core" { "profile-core" }
                    "windows-x64" { "profile-win-x64" }
                    "ide" { "profile-ide" }
                    default { throw "ideal-program: unknown profile '$($profile.profile)'" }
                }
                WorksetRoot = [string]$profile.workset_root
                WorksetDoc = [string]$profile.workset_doc
                Code = [string]$epic.code
                EpicId = [string]$epic.bead
                EpicLabel = "epic-$(([string]$epic.code).ToLowerInvariant())"
            }
        }
    }
    return $records
}

function Test-IdealContractClauses {
    param([AllowEmptyString()][string]$Text)

    if ([string]::IsNullOrWhiteSpace($Text)) {
        return $false
    }
    if ($Text -match '[A-Z][A-Z0-9-]*-\*') {
        return $false
    }
    return [regex]::IsMatch($Text, '(?<![A-Z0-9-])[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+-\d{3}(?![A-Z0-9-])')
}

function Get-IdealContractClauseIds {
    param([AllowEmptyString()][string]$Text)

    if ([string]::IsNullOrWhiteSpace($Text)) {
        return @()
    }
    return @(
        [regex]::Matches($Text, '(?<![A-Z0-9-])[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+-\d{3}(?![A-Z0-9-])') |
            ForEach-Object { $_.Value } |
            Sort-Object -Unique
    )
}

function Test-IdealEvidenceReferences {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [AllowEmptyString()][string]$Text
    )

    if ([string]::IsNullOrWhiteSpace($Text)) {
        return $false
    }
    $tokens = @($Text -split '[;|]' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    if ($tokens.Count -eq 0) {
        return $false
    }
    foreach ($token in $tokens) {
        $prefixMatch = [regex]::Match($token, '^(?<prefix>[a-z][a-z0-9-]*):(?<value>.+)$', [Text.RegularExpressions.RegexOptions]::IgnoreCase)
        if ($prefixMatch.Success) {
            $prefix = $prefixMatch.Groups['prefix'].Value.ToLowerInvariant()
            $value = $prefixMatch.Groups['value'].Value.Trim()
            if ([string]::IsNullOrWhiteSpace($value)) {
                return $false
            }
            if ($prefix -in @("matrix", "workset", "file")) {
                $path = ($value -split '#', 2)[0]
                $path = ($path -split '::', 2)[0]
                if (-not (Test-Path -LiteralPath (Resolve-IdealRepoPath -RepoRoot $RepoRoot -Path $path) -PathType Leaf)) {
                    return $false
                }
            }
            elseif ($prefix -notin @("br", "command", "cargo", "test", "oracle", "environment", "excel", "spec", "external", "transcript")) {
                return $false
            }
            continue
        }

        $path = ($token -split '#', 2)[0]
        $path = ($path -split '::', 2)[0]
        if (-not (Test-Path -LiteralPath (Resolve-IdealRepoPath -RepoRoot $RepoRoot -Path $path) -PathType Leaf)) {
            return $false
        }
    }
    return $true
}

function ConvertFrom-IdealBoolean {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value,
        [Parameter(Mandatory = $true)]
        [string]$Owner
    )

    switch ($Value.Trim().ToLowerInvariant()) {
        "true" { return $true }
        "false" { return $false }
        default { throw "ideal-program: $Owner has invalid boolean '$Value'" }
    }
}

function Get-IdealCsvColumns {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "ideal-program: missing CSV $Path"
    }
    $header = Get-Content -LiteralPath $Path -First 1
    if ([string]::IsNullOrWhiteSpace($header)) {
        throw "ideal-program: CSV has no header: $Path"
    }
    return @($header -split ',' | ForEach-Object { $_.Trim().Trim('"') })
}

function Assert-IdealRelativePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Owner
    )

    $normalized = $Path.Replace('\', '/')
    if ([IO.Path]::IsPathRooted($Path) -or $normalized -match '(^|/)\.\.(/|$)') {
        throw "ideal-program: $Owner must use a repository-relative path, found '$Path'"
    }
}
