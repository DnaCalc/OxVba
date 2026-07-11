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
        "environment_manifest",
        "clause_disposition",
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
            elseif ($prefix -eq "artifact") {
                $path = ($value -split '#', 2)[0]
                $path = ($path -split '::', 2)[0]
                $normalized = $path.Replace('\', '/')
                if ([string]::IsNullOrWhiteSpace($path) -or [IO.Path]::IsPathRooted($path) -or $normalized -match '(^|/)\.\.(/|$)') {
                    return $false
                }
            }
            elseif ($prefix -notin @("br", "command", "cargo", "test", "oracle", "environment", "excel", "spec", "external", "transcript", "observables")) {
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

function Split-IdealPipeList {
    param(
        [AllowEmptyString()][string]$Value,
        [Parameter(Mandatory = $true)][string]$Owner,
        [switch]$AllowNotApplicable
    )

    $trimmed = $Value.Trim()
    if ([string]::IsNullOrWhiteSpace($trimmed)) {
        return @()
    }
    if ($AllowNotApplicable -and $trimmed -eq "n/a") {
        return @()
    }
    if ($trimmed.Contains(";")) {
        throw "ideal-program: $Owner must use pipe-delimited values, not semicolons"
    }

    $values = @($trimmed -split '\|' | ForEach-Object { $_.Trim() })
    if (@($values | Where-Object { [string]::IsNullOrWhiteSpace($_) }).Count -gt 0) {
        throw "ideal-program: $Owner contains an empty pipe-delimited value"
    }
    $unique = @($values | Sort-Object -Unique)
    if ($unique.Count -ne $values.Count) {
        throw "ideal-program: $Owner contains duplicate values"
    }
    return $values
}

function Get-IdealExecutionMode {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [string]$AutorunPath = "docs/AUTORUN_STATE.md"
    )

    $autorunAbs = Resolve-IdealRepoPath -RepoRoot $RepoRoot -Path $AutorunPath
    if (-not (Test-Path -LiteralPath $autorunAbs -PathType Leaf)) {
        throw "ideal-program: missing execution control $AutorunPath"
    }
    $text = Get-Content -LiteralPath $autorunAbs -Raw
    $match = [regex]::Match($text, '(?im)^Mode:\s*(?<mode>[^\r\n]+)$')
    if (-not $match.Success) {
        throw "ideal-program: execution control has no Mode field"
    }
    $mode = $match.Groups['mode'].Value.Trim().Trim('`')
    if ($mode -notin @("Directed", "AutoRun")) {
        throw "ideal-program: execution control mode must be Directed or AutoRun, found '$mode'"
    }
    return $mode
}

function Assert-IdealExecutableAcceptanceGrammar {
    param(
        [AllowEmptyString()][string]$Text,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ([string]::IsNullOrWhiteSpace($Text)) {
        throw "ideal-program: $Owner has no executable acceptance text"
    }
    if (-not [regex]::IsMatch($Text, '(?im)(?:^|[\s`])command\s*:\s*[^\s`;|][^\r\n]*')) {
        throw "ideal-program: $Owner must include a nonempty typed command: acceptance command"
    }
    if (-not [regex]::IsMatch($Text, '(?im)(?:^|[\s`])expected(?:-|\s+)observable\s*:\s*[^\s`;|][^\r\n]*')) {
        throw "ideal-program: $Owner must include a nonempty expected-observable: statement"
    }
    if (-not [regex]::IsMatch($Text, '(?im)(?:^|[\s`])(?:artifact|transcript|oracle|environment)\s*:\s*[^\s`;|][^\r\n]*')) {
        throw "ideal-program: $Owner must include an artifact:/transcript:/oracle:/environment: evidence reference"
    }
}

function Get-IdealTypedReferenceMatches {
    param(
        [AllowEmptyString()][string]$Text,
        [Parameter(Mandatory = $true)][string[]]$Prefixes
    )

    if ([string]::IsNullOrWhiteSpace($Text)) {
        return @()
    }
    $escaped = @($Prefixes | ForEach-Object { [regex]::Escape($_) }) -join '|'
    return @(
        [regex]::Matches(
            $Text,
            "(?im)(?:^|[;|])\s*(?<prefix>$escaped)\s*:\s*(?<value>[^;|\r\n]+)"
        )
    )
}

function Assert-IdealVerifiedEvidenceGrammar {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [AllowEmptyString()][string]$EvidenceRefs,
        [Parameter(Mandatory = $true)][string]$Owner,
        [hashtable]$EnvironmentById
    )

    if ([string]::IsNullOrWhiteSpace($EvidenceRefs)) {
        throw "ideal-program: verified $Owner has no evidence_refs"
    }

    $observableMatch = [regex]::Match(
        $EvidenceRefs,
        '(?im)(?:^|[;|])\s*observables\s*:\s*(?<value>[^;|\r\n]+)'
    )
    if (-not $observableMatch.Success) {
        throw "ideal-program: verified $Owner must classify observables: result, full-err, side-effects, lifecycle-order, transport, and balance"
    }
    $axisStates = @{}
    foreach ($entry in @($observableMatch.Groups['value'].Value -split ',')) {
        $pair = @($entry.Trim() -split '=', 2)
        if ($pair.Count -ne 2 -or [string]::IsNullOrWhiteSpace($pair[0]) -or [string]::IsNullOrWhiteSpace($pair[1])) {
            throw "ideal-program: verified $Owner has malformed observables entry '$entry'"
        }
        $axis = $pair[0].Trim().ToLowerInvariant()
        $state = $pair[1].Trim().ToLowerInvariant()
        if ($axisStates.ContainsKey($axis)) {
            throw "ideal-program: verified $Owner repeats observable axis '$axis'"
        }
        if ($state -notin @("verified", "n/a")) {
            throw "ideal-program: verified $Owner observable '$axis' must be verified or n/a"
        }
        $axisStates[$axis] = $state
    }
    $requiredAxes = @("result", "full-err", "side-effects", "lifecycle-order", "transport", "balance")
    foreach ($axis in $requiredAxes) {
        if (-not $axisStates.ContainsKey($axis)) {
            throw "ideal-program: verified $Owner does not classify observable axis '$axis'"
        }
    }
    if ($axisStates.Count -ne $requiredAxes.Count) {
        $unexpected = @($axisStates.Keys | Where-Object { $_ -notin $requiredAxes }) -join ', '
        throw "ideal-program: verified $Owner has unexpected observable axes: $unexpected"
    }
    if ($axisStates["result"] -ne "verified") {
        throw "ideal-program: verified $Owner must have result=verified"
    }

    $actualMatches = @(Get-IdealTypedReferenceMatches -Text $EvidenceRefs -Prefixes @("artifact", "transcript", "oracle"))
    if ($actualMatches.Count -eq 0) {
        throw "ideal-program: verified $Owner needs a typed artifact:/transcript:/oracle: actual-evidence reference"
    }
    foreach ($match in $actualMatches) {
        $prefix = $match.Groups['prefix'].Value.ToLowerInvariant()
        $value = $match.Groups['value'].Value.Trim().Trim('`')
        $path = ($value -split '#', 2)[0]
        $path = ($path -split '::', 2)[0]
        Assert-IdealRelativePath -Path $path -Owner "verified $Owner $prefix reference"
        if (-not (Test-Path -LiteralPath (Resolve-IdealRepoPath -RepoRoot $RepoRoot -Path $path) -PathType Leaf)) {
            throw "ideal-program: verified $Owner $prefix evidence does not resolve: '$path'"
        }
    }

    foreach ($match in @(Get-IdealTypedReferenceMatches -Text $EvidenceRefs -Prefixes @("environment"))) {
        $environmentId = $match.Groups['value'].Value.Trim().Trim('`')
        if ($null -eq $EnvironmentById -or -not $EnvironmentById.ContainsKey($environmentId)) {
            throw "ideal-program: verified $Owner references unknown environment '$environmentId'"
        }
    }
}

function Assert-IdealLspAdvertisement {
    param(
        [Parameter(Mandatory = $true)]$Row,
        [Parameter(Mandatory = $true)][hashtable]$DirectRowsById,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $advertised = ConvertFrom-IdealBoolean -Value ([string]$Row.capability_advertised) -Owner "$Owner capability_advertised"
    if (-not $advertised) {
        return
    }

    foreach ($field in @("truth_state", "projection_state", "equivalence_state")) {
        if ([string]$Row.$field -ne "verified") {
            throw "ideal-program: $Owner cannot advertise while $field='$($Row.$field)'"
        }
    }
    $directRowId = [string]$Row.direct_matrix_row
    if ([string]::IsNullOrWhiteSpace($directRowId) -or -not $DirectRowsById.ContainsKey($directRowId)) {
        throw "ideal-program: $Owner advertises without a resolvable direct_matrix_row"
    }
    $directRow = $DirectRowsById[$directRowId]
    if ([string]$directRow.truth_state -ne "verified") {
        throw "ideal-program: $Owner advertises before direct row '$directRowId' is verified"
    }
    $directState = ""
    if ($directRow.PSObject.Properties.Name -contains "direct_state") {
        $directState = [string]$directRow.direct_state
    }
    elseif ($directRow.PSObject.Properties.Name -contains "direct_query_state") {
        $directState = [string]$directRow.direct_query_state
    }
    if ($directState -ne "verified") {
        throw "ideal-program: $Owner advertises before direct row '$directRowId' has a verified direct result"
    }
    if ($Row.PSObject.Properties.Name -contains "source_claim_key" -and
        $directRow.PSObject.Properties.Name -contains "claim_key" -and
        [string]$Row.source_claim_key -ne [string]$directRow.claim_key) {
        throw "ideal-program: $Owner source_claim_key does not identify direct row '$directRowId'"
    }
    if ($directRow.PSObject.Properties.Name -contains "direct_api_method" -and
        -not [string]::IsNullOrWhiteSpace([string]$directRow.direct_api_method) -and
        [string]$directRow.direct_api_method -ne [string]$Row.direct_api_method) {
        throw "ideal-program: $Owner direct_api_method disagrees with '$directRowId'"
    }
}

function Assert-IdealClosedRolloutTraceState {
    param(
        [Parameter(Mandatory = $true)][string]$RolloutId,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$RolloutTraces,
        [Parameter(Mandatory = $true)][hashtable]$MatrixRowsById,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$DeliveryLeafIds,
        [Parameter(Mandatory = $true)][hashtable]$TraceRowsByBead
    )

    foreach ($trace in $RolloutTraces) {
        if ([string]$trace.relationship -in @("matrix-scaffold", "owns-planned-row")) {
            throw "ideal-program: closed rollout $RolloutId retains '$($trace.relationship)' trace $($trace.matrix_id)/$($trace.row_id)"
        }
    }
    foreach ($matrixId in @($MatrixRowsById.Keys)) {
        foreach ($matrixRow in @($MatrixRowsById[$matrixId].Values)) {
            if ([string]$matrixRow.truth_state -eq "planned" -and
                ([string]$matrixRow.evidence_owner_bead -eq $RolloutId -or [string]$matrixRow.residual_owner_bead -eq $RolloutId)) {
                throw "ideal-program: closed rollout $RolloutId still owns planned row $matrixId/$($matrixRow.row_id)"
            }
        }
    }
    if ($DeliveryLeafIds.Count -eq 0) {
        throw "ideal-program: closed rollout $RolloutId has no delivery leaf with an exact row path"
    }
    foreach ($deliveryLeafId in $DeliveryLeafIds) {
        $leafTraces = @()
        if ($TraceRowsByBead.ContainsKey($deliveryLeafId)) {
            $leafTraces = @($TraceRowsByBead[$deliveryLeafId])
        }
        $exactLeafTraces = @($leafTraces | Where-Object {
            -not [string]::IsNullOrWhiteSpace([string]$_.row_id) -and
            [string]$_.relationship -notin @("matrix-scaffold", "owns-planned-row")
        })
        if ($exactLeafTraces.Count -eq 0) {
            throw "ideal-program: delivery leaf $deliveryLeafId under closed rollout $RolloutId lacks an exact row trace"
        }
    }
}
