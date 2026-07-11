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

function Get-IdealLeafArtifactRefs {
    param([AllowEmptyString()][string]$Text)

    if ([string]::IsNullOrWhiteSpace($Text)) {
        return @()
    }
    return @(
        [regex]::Matches(
            $Text,
            '(?im)(?:^|[\s`|;])artifact\s*:\s*`?(?<value>[A-Za-z0-9_.\\/-]+\.(?:md|json|jsonl|csv|txt|xml|sarif|log|bin|oxi))'
        ) |
            ForEach-Object { $_.Groups['value'].Value.Replace('\', '/').ToLowerInvariant() } |
            Sort-Object -Unique
    )
}

function Get-IdealLeafCommandAnchors {
    param([AllowEmptyString()][string]$Text)

    if ([string]::IsNullOrWhiteSpace($Text)) {
        return @()
    }
    $anchors = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($match in @([regex]::Matches($Text, '(?im)(?:^|[\s`|;])command\s*:\s*`?(?<body>[^`|;\r\n]+)'))) {
        $body = $match.Groups['body'].Value
        $cargo = [regex]::Match(
            $body,
            '(?i)\bcargo\s+(?<verb>[a-z-]+)(?:\s+-p\s+(?<package>[A-Za-z0-9_-]+))?(?:\s+(?<selector>[A-Za-z0-9_:-]+))?'
        )
        if ($cargo.Success) {
            $parts = @('cargo', $cargo.Groups['verb'].Value)
            if ($cargo.Groups['package'].Success) {
                $parts += @('-p', $cargo.Groups['package'].Value)
            }
            if ($cargo.Groups['selector'].Success) {
                $parts += $cargo.Groups['selector'].Value
            }
            [void]$anchors.Add(($parts -join ' ').ToLowerInvariant())
        }
        foreach ($script in @([regex]::Matches($body, '(?i)(?:\./)?scripts/[A-Za-z0-9_.\/-]+\.ps1'))) {
            [void]$anchors.Add($script.Value.TrimStart('.').TrimStart('/').ToLowerInvariant())
        }
    }
    return @($anchors)
}

Push-Location $repoRoot
try {
    $manifestContext = Read-IdealProgramManifest -RepoRoot $repoRoot -ManifestPath $ManifestPath
    $manifest = $manifestContext.Manifest
    $issueContext = Read-IdealIssues -RepoRoot $repoRoot -IssuesPath $IssuesPath
    $issueById = $issueContext.IssueById
    $childrenByParent = New-IdealChildrenMap -Issues @($issueContext.Issues)
    $executionMode = Get-IdealExecutionMode -RepoRoot $repoRoot -AutorunPath $AutorunPath

    $expectedEpics = @(Get-IdealExpectedEpicRecords -Manifest $manifest)
    $epicById = @{}
    $descendantsByEpic = @{}
    foreach ($epic in $expectedEpics) {
        $epicById[$epic.EpicId] = $epic
        $set = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($id in @(Get-IdealDescendantIds -RootId $epic.EpicId -ChildrenByParent $childrenByParent)) {
            [void]$set.Add($id)
        }
        $descendantsByEpic[$epic.EpicId] = $set
    }

    $ownershipAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.matrix_ownership)
    $ownershipRows = @(Import-Csv -LiteralPath $ownershipAbs)
    $matrixById = @{}
    $matrixRowIds = @{}
    $matrixRowsById = @{}
    foreach ($owner in $ownershipRows) {
        $matrixId = [string]$owner.matrix_id
        $matrixById[$matrixId] = $owner
        $rowSet = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        $rowById = @{}
        $matrixAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$owner.path)
        foreach ($matrixRow in @(Import-Csv -LiteralPath $matrixAbs)) {
            $matrixRowId = [string]$matrixRow.row_id
            [void]$rowSet.Add($matrixRowId)
            $rowById[$matrixRowId] = $matrixRow
        }
        $matrixRowIds[$matrixId] = $rowSet
        $matrixRowsById[$matrixId] = $rowById
    }

    $programDescendants = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($id in @(Get-IdealDescendantIds -RootId ([string]$manifest.root_bead) -ChildrenByParent $childrenByParent)) {
        [void]$programDescendants.Add($id)
    }

    $traceAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.bead_traceability)
    $traceRows = @(Import-Csv -LiteralPath $traceAbs)
    if ($traceRows.Count -eq 0) {
        throw "validate-bead-traceability: current-program traceability is empty"
    }

    $validRelationships = @("owns", "owns-planned-row", "advances", "evidences", "projects", "matrix-scaffold")
    $seenRelationships = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $tracedBeads = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $tracedMatrices = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $tracedMatrixRows = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $traceRowsByBead = @{}
    $traceClausesByBead = @{}
    $traceClausesByEpic = @{}
    $exactTraceCountByEpic = @{}
    foreach ($epic in $expectedEpics) {
        $exactTraceCountByEpic[$epic.EpicId] = 0
        $traceClausesByEpic[$epic.EpicId] = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    }

    foreach ($row in $traceRows) {
        $beadId = [string]$row.bead_id
        $parentEpic = [string]$row.parent_epic
        $effect = [string]$row.effect
        $profile = [string]$row.profile
        $matrixId = [string]$row.matrix_id
        $rowId = [string]$row.row_id
        $relationship = [string]$row.relationship
        $composite = "$beadId|$matrixId|$rowId|$relationship"

        if (-not $traceRowsByBead.ContainsKey($beadId)) {
            $traceRowsByBead[$beadId] = [Collections.Generic.List[object]]::new()
            $traceClausesByBead[$beadId] = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        }
        $traceRowsByBead[$beadId].Add($row)

        if (-not $seenRelationships.Add($composite)) {
            throw "validate-bead-traceability: duplicate relationship '$composite'"
        }
        if (-not $issueById.ContainsKey($beadId)) {
            throw "validate-bead-traceability: bead '$beadId' does not exist"
        }
        if (-not $epicById.ContainsKey($parentEpic)) {
            throw "validate-bead-traceability: parent '$parentEpic' is not a manifest execution epic"
        }
        if (-not $descendantsByEpic[$parentEpic].Contains($beadId)) {
            throw "validate-bead-traceability: bead '$beadId' is not a descendant of parent epic '$parentEpic'"
        }
        if ($profile -ne $epicById[$parentEpic].Profile) {
            throw "validate-bead-traceability: bead '$beadId' profile '$profile' disagrees with parent epic profile '$($epicById[$parentEpic].Profile)'"
        }
        if ($effect -notin @("delivery", "support")) {
            throw "validate-bead-traceability: bead '$beadId' has invalid effect '$effect'"
        }
        if ((Get-IdealIssueLabels -Issue $issueById[$beadId]) -notcontains $effect) {
            throw "validate-bead-traceability: bead '$beadId' does not carry trace effect label '$effect'"
        }
        if (-not $matrixById.ContainsKey($matrixId)) {
            throw "validate-bead-traceability: bead '$beadId' references unknown matrix '$matrixId'"
        }
        if ($relationship -notin $validRelationships) {
            throw "validate-bead-traceability: bead '$beadId' has invalid relationship '$relationship'"
        }
        if ($relationship -eq "projects" -and [string]$matrixById[$matrixId].role -ne "projection") {
            throw "validate-bead-traceability: projects relationship '$beadId/$matrixId' requires a projection matrix"
        }
        if ($relationship -eq "owns-planned-row" -and $effect -ne "support") {
            throw "validate-bead-traceability: owns-planned-row relationship '$beadId/$matrixId' must be support work"
        }
        if ($relationship -eq "advances" -and $effect -ne "delivery") {
            throw "validate-bead-traceability: advances relationship '$beadId/$matrixId' must be delivery work"
        }
        if ([string]::IsNullOrWhiteSpace($rowId)) {
            if ($effect -ne "support" -or $relationship -ne "matrix-scaffold") {
                throw "validate-bead-traceability: only support matrix-scaffold relationships may omit row_id ($beadId/$matrixId)"
            }
        }
        else {
            if (-not $matrixRowIds[$matrixId].Contains($rowId)) {
                throw "validate-bead-traceability: bead '$beadId' references missing row '$matrixId/$rowId'"
            }
            $matrixRow = $matrixRowsById[$matrixId][$rowId]
            [void]$tracedMatrixRows.Add("$matrixId|$rowId")
            $exactTraceCountByEpic[$parentEpic] = [int]$exactTraceCountByEpic[$parentEpic] + 1
        }
        if ($effect -eq "delivery" -and [string]::IsNullOrWhiteSpace($rowId)) {
            throw "validate-bead-traceability: delivery bead '$beadId' must trace to an exact matrix row"
        }
        if (-not (Test-IdealContractClauses -Text ([string]$row.contract_clauses))) {
            throw "validate-bead-traceability: bead '$beadId' must name exact, non-wildcard contract clauses"
        }
        $traceClauses = @(Get-IdealContractClauseIds -Text ([string]$row.contract_clauses))
        foreach ($traceClause in $traceClauses) {
            [void]$traceClausesByBead[$beadId].Add($traceClause)
            [void]$traceClausesByEpic[$parentEpic].Add($traceClause)
        }
        $beadClauses = @(Get-IdealContractClauseIds -Text ([string]$issueById[$beadId].description))
        if (@($traceClauses | Where-Object { $beadClauses -notcontains $_ }).Count -gt 0) {
            throw "validate-bead-traceability: trace '$beadId/$matrixId' contains clauses outside the bead contract"
        }
        if (-not [string]::IsNullOrWhiteSpace($rowId) -and $relationship -in @("owns", "owns-planned-row", "advances")) {
            $matrixClauses = @(Get-IdealContractClauseIds -Text ([string]$matrixRow.contract_clauses))
            if (@($matrixClauses | Where-Object { $traceClauses -notcontains $_ }).Count -gt 0) {
                throw "validate-bead-traceability: trace '$beadId/$matrixId/$rowId' does not cover every matrix-row clause"
            }
        }
        if (-not (Test-IdealEvidenceReferences -RepoRoot $repoRoot -Text ([string]$row.acceptance_evidence))) {
            throw "validate-bead-traceability: bead '$beadId' has no resolvable acceptance_evidence"
        }
        $issueAcceptanceText = "$([string]$issueById[$beadId].description)`n$([string]$issueById[$beadId].acceptance_criteria)"
        $traceAcceptanceText = ([string]$row.acceptance_evidence).Replace('\', '/').ToLowerInvariant()
        foreach ($artifactRef in @(Get-IdealLeafArtifactRefs -Text $issueAcceptanceText)) {
            if (-not $traceAcceptanceText.Contains("artifact:$artifactRef")) {
                throw "validate-bead-traceability: trace '$beadId/$matrixId/$rowId' omits leaf artifact:$artifactRef"
            }
        }
        $commandAnchors = @(Get-IdealLeafCommandAnchors -Text $issueAcceptanceText)
        if ($commandAnchors.Count -gt 0 -and
            @($commandAnchors | Where-Object { $traceAcceptanceText.Contains($_) }).Count -eq 0) {
            throw "validate-bead-traceability: trace '$beadId/$matrixId/$rowId' substitutes generic acceptance for leaf command anchor(s): $($commandAnchors -join ',')"
        }
        $residualOwner = [string]$row.residual_owner_bead
        if (-not [string]::IsNullOrWhiteSpace($residualOwner)) {
            if (-not $issueById.ContainsKey($residualOwner) -or
                -not $programDescendants.Contains($residualOwner) -or
                (Get-IdealIssueLabels -Issue $issueById[$residualOwner]) -notcontains [string]$manifest.program_label) {
                throw "validate-bead-traceability: bead '$beadId' residual owner '$residualOwner' is outside the current program"
            }
            if ([string]$issueById[$residualOwner].status -notin @("open", "in_progress", "blocked")) {
                throw "validate-bead-traceability: bead '$beadId' residual owner '$residualOwner' is not active"
            }
        }
        if (-not [string]::IsNullOrWhiteSpace($rowId) -and $relationship -in @("owns", "owns-planned-row", "advances")) {
            $matrixResidualOwner = [string]$matrixRow.residual_owner_bead
            if ($matrixResidualOwner -ne $residualOwner) {
                throw "validate-bead-traceability: trace '$beadId/$matrixId/$rowId' residual owner '$residualOwner' disagrees with row owner '$matrixResidualOwner'"
            }
        }

        [void]$tracedBeads.Add($beadId)
        [void]$tracedMatrices.Add($matrixId)
    }

    foreach ($owner in $ownershipRows) {
        $matrixId = [string]$owner.matrix_id
        if (-not $tracedMatrices.Contains($matrixId)) {
            throw "validate-bead-traceability: owned matrix '$matrixId' has no current-program trace relationship"
        }
        foreach ($rowId in @($matrixRowIds[$matrixId])) {
            if (-not $tracedMatrixRows.Contains("$matrixId|$rowId")) {
                throw "validate-bead-traceability: matrix row '$matrixId/$rowId' has no bead relationship"
            }
        }
    }

    $executionLeaves = @()
    foreach ($profile in @($manifest.profiles)) {
        foreach ($id in @(Get-IdealDescendantIds -RootId ([string]$profile.workset_root) -ChildrenByParent $childrenByParent)) {
            $issue = $issueById[$id]
            $hasChildren = $childrenByParent.ContainsKey($id) -and @($childrenByParent[$id]).Count -gt 0
            if (-not $hasChildren -and [string]$issue.issue_type -ne "epic") {
                $executionLeaves += $id
            }
        }
    }
    $executionLeafSet = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($leafId in @($executionLeaves | Sort-Object -Unique)) {
        [void]$executionLeafSet.Add($leafId)
        if (-not $tracedBeads.Contains($leafId)) {
            throw "validate-bead-traceability: execution leaf '$leafId' has no matrix relationship"
        }
        $beadClauses = @(Get-IdealContractClauseIds -Text ([string]$issueById[$leafId].description))
        $traceClauseUnion = $traceClausesByBead[$leafId]
        $untracedClauses = @($beadClauses | Where-Object { -not $traceClauseUnion.Contains($_) })
        if ($untracedClauses.Count -gt 0) {
            throw "validate-bead-traceability: execution leaf '$leafId' contract clause(s) absent from trace union: $($untracedClauses -join '|')"
        }
    }
    foreach ($tracedBeadId in $tracedBeads) {
        if (-not $executionLeafSet.Contains($tracedBeadId)) {
            throw "validate-bead-traceability: traced bead '$tracedBeadId' is not an execution leaf"
        }
    }
    foreach ($epic in $expectedEpics) {
        $epicId = [string]$epic.EpicId
        $epicClauses = @(Get-IdealContractClauseIds -Text ([string]$issueById[$epicId].description))
        $traceClauseUnion = $traceClausesByEpic[$epicId]
        $untracedEpicClauses = @($epicClauses | Where-Object { -not $traceClauseUnion.Contains($_) })
        $undeclaredTraceClauses = @($traceClauseUnion | Where-Object { $epicClauses -notcontains $_ })
        if ($untracedEpicClauses.Count -gt 0 -or $undeclaredTraceClauses.Count -gt 0) {
            throw "validate-bead-traceability: execution epic '$epicId' contract differs from its trace-clause union (untraced=$($untracedEpicClauses -join '|'); undeclared=$($undeclaredTraceClauses -join '|'))"
        }
    }

    if ($executionMode -eq "AutoRun") {
        foreach ($epic in $expectedEpics) {
            if ([int]$exactTraceCountByEpic[$epic.EpicId] -eq 0) {
                throw "validate-bead-traceability: AutoRun requires execution epic $($epic.EpicId) to connect to an explicit matrix row"
            }
        }
    }

    foreach ($epic in $expectedEpics) {
        $directChildren = if ($childrenByParent.ContainsKey($epic.EpicId)) { @($childrenByParent[$epic.EpicId]) } else { @() }
        $rollouts = @($directChildren | Where-Object { (Get-IdealIssueLabels -Issue $_) -contains "rollout" })
        if ($rollouts.Count -ne 1 -or [string]$rollouts[0].status -ne "closed") {
            continue
        }
        $rolloutId = [string]$rollouts[0].id
        $rolloutTraces = @()
        if ($traceRowsByBead.ContainsKey($rolloutId)) {
            $rolloutTraces = @($traceRowsByBead[$rolloutId])
        }
        $deliveryLeaves = @(
            Get-IdealDescendantIds -RootId $epic.EpicId -ChildrenByParent $childrenByParent |
                Where-Object {
                    $id = [string]$_
                    $issue = $issueById[$id]
                    [string]$issue.issue_type -ne "epic" -and
                    (-not $childrenByParent.ContainsKey($id) -or @($childrenByParent[$id]).Count -eq 0) -and
                    (Get-IdealIssueLabels -Issue $issue) -contains "delivery"
                }
        )
        Assert-IdealClosedRolloutTraceState `
            -RolloutId $rolloutId `
            -RolloutTraces $rolloutTraces `
            -MatrixRowsById $matrixRowsById `
            -DeliveryLeafIds $deliveryLeaves `
            -TraceRowsByBead $traceRowsByBead
    }

    Write-Host "validate-bead-traceability: ok (program=$($manifest.program_id) relationships=$($traceRows.Count) leaves=$(@($executionLeaves | Sort-Object -Unique).Count) matrices=$($ownershipRows.Count))"
}
finally {
    Pop-Location
}
