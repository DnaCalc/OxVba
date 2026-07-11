param(
    [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
    [string]$IssuesPath = ".beads/issues.jsonl",
    [string]$ContractPath = "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md",
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

function Assert-ExactClauseSet {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Expected,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Actual,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $delta = @(
        Compare-Object `
            -ReferenceObject @($Expected | Sort-Object -Unique) `
            -DifferenceObject @($Actual | Sort-Object -Unique)
    )
    if ($delta.Count -gt 0) {
        $details = @($delta | ForEach-Object { "$($_.SideIndicator)$($_.InputObject)" }) -join ", "
        throw "validate-contract-clause-disposition: $Owner differs: $details"
    }
}

Push-Location $repoRoot
try {
    $manifest = (Read-IdealProgramManifest -RepoRoot $repoRoot -ManifestPath $ManifestPath).Manifest
    $issueContext = Read-IdealIssues -RepoRoot $repoRoot -IssuesPath $IssuesPath
    $issueById = $issueContext.IssueById
    $expectedEpics = @(Get-IdealExpectedEpicRecords -Manifest $manifest)
    $epicById = @{}
    foreach ($epic in $expectedEpics) {
        $epicById[$epic.EpicId] = $epic
    }

    Assert-IdealRelativePath -Path ([string]$manifest.clause_disposition) -Owner "manifest.clause_disposition"
    $dispositionAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.clause_disposition)
    $expectedHeader = @("clause_id", "disposition", "profiles", "owner_epics", "consumer_epics", "matrix_ids", "notes")
    $actualHeader = @(Get-IdealCsvColumns -Path $dispositionAbs)
    if (($expectedHeader -join ',') -ne ($actualHeader -join ',')) {
        throw "validate-contract-clause-disposition: disposition header must exactly match the V1 schema"
    }

    $contractAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $ContractPath
    if (-not (Test-Path -LiteralPath $contractAbs -PathType Leaf)) {
        throw "validate-contract-clause-disposition: missing normative contract $ContractPath"
    }
    $contractText = Get-Content -LiteralPath $contractAbs -Raw
    $contractMatches = @(
        [regex]::Matches(
            $contractText,
            '(?m)^###\s+.*?`(?<id>[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+-\d{3})`\s*$'
        )
    )
    $contractClauses = @($contractMatches | ForEach-Object { $_.Groups['id'].Value })
    if ($contractClauses.Count -eq 0) {
        throw "validate-contract-clause-disposition: no normative clause headings found in $ContractPath"
    }
    if (@($contractClauses | Sort-Object -Unique).Count -ne $contractClauses.Count) {
        throw "validate-contract-clause-disposition: normative contract contains duplicate clause headings"
    }

    $ownershipAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.matrix_ownership)
    $ownershipRows = @(Import-Csv -LiteralPath $ownershipAbs)
    $matrixById = @{}
    foreach ($matrix in $ownershipRows) {
        $matrixById[[string]$matrix.matrix_id] = $matrix
    }

    $rows = @(Import-Csv -LiteralPath $dispositionAbs)
    $rowByClause = @{}
    $allowedDeferred = @("PROFILE-EXT-001", "DEBUG-CORE-001", "FORMS-RUNTIME-001")
    $validProfiles = @("core", "windows-x64", "ide")
    foreach ($row in $rows) {
        $clauseId = [string]$row.clause_id
        if ([string]::IsNullOrWhiteSpace($clauseId) -or $rowByClause.ContainsKey($clauseId)) {
            throw "validate-contract-clause-disposition: blank or duplicate clause_id '$clauseId'"
        }
        $rowByClause[$clauseId] = $row
        if ([string]::IsNullOrWhiteSpace([string]$row.notes)) {
            throw "validate-contract-clause-disposition: $clauseId has no disposition rationale"
        }

        $disposition = [string]$row.disposition
        if ($disposition -notin @("in-scope", "deferred-extended")) {
            throw "validate-contract-clause-disposition: $clauseId has invalid disposition '$disposition'"
        }
        if ($disposition -eq "deferred-extended") {
            if ($clauseId -notin $allowedDeferred) {
                throw "validate-contract-clause-disposition: only explicit extended clauses may be deferred; found $clauseId"
            }
            if ([string]$row.profiles -ne "extended" -or
                [string]$row.owner_epics -notin @("", "n/a") -or
                [string]$row.consumer_epics -notin @("", "n/a") -or
                [string]$row.matrix_ids -notin @("", "n/a")) {
                throw "validate-contract-clause-disposition: deferred $clauseId must use profiles=extended and no current owner/consumer/matrix"
            }
            continue
        }
        if ($clauseId -in $allowedDeferred) {
            throw "validate-contract-clause-disposition: extended clause $clauseId must be explicitly deferred for this umbrella"
        }

        $profiles = @(Split-IdealPipeList -Value ([string]$row.profiles) -Owner "$clauseId profiles")
        $owners = @(Split-IdealPipeList -Value ([string]$row.owner_epics) -Owner "$clauseId owner_epics")
        $consumers = @(Split-IdealPipeList -Value ([string]$row.consumer_epics) -Owner "$clauseId consumer_epics" -AllowNotApplicable)
        $matrices = @(Split-IdealPipeList -Value ([string]$row.matrix_ids) -Owner "$clauseId matrix_ids")
        if ($profiles.Count -eq 0 -or $owners.Count -eq 0 -or $matrices.Count -eq 0) {
            throw "validate-contract-clause-disposition: in-scope $clauseId needs profile, owner epic, and matrix coverage"
        }
        foreach ($profile in $profiles) {
            if ($profile -notin $validProfiles) {
                throw "validate-contract-clause-disposition: $clauseId has unknown profile '$profile'"
            }
        }
        foreach ($ownerId in $owners) {
            if (-not $epicById.ContainsKey($ownerId) -or -not $issueById.ContainsKey($ownerId)) {
                throw "validate-contract-clause-disposition: $clauseId owner '$ownerId' is not a current manifest execution epic"
            }
            if ($epicById[$ownerId].Profile -notin $profiles) {
                throw "validate-contract-clause-disposition: $clauseId owner '$ownerId' profile '$($epicById[$ownerId].Profile)' is not declared"
            }
        }
        foreach ($consumerId in $consumers) {
            if ($owners -contains $consumerId) {
                throw "validate-contract-clause-disposition: $clauseId epic '$consumerId' cannot be both producer owner and consumer"
            }
            if (-not $epicById.ContainsKey($consumerId) -or -not $issueById.ContainsKey($consumerId)) {
                throw "validate-contract-clause-disposition: $clauseId consumer '$consumerId' is not a current manifest execution epic"
            }
            if ($epicById[$consumerId].Profile -notin $profiles) {
                throw "validate-contract-clause-disposition: $clauseId consumer '$consumerId' profile '$($epicById[$consumerId].Profile)' is not declared"
            }
        }
        foreach ($profile in $profiles) {
            $profileEpicCount = @(
                @($owners + $consumers) |
                    Where-Object { $epicById[$_].Profile -eq $profile }
            ).Count
            if ($profileEpicCount -eq 0) {
                throw "validate-contract-clause-disposition: $clauseId profile '$profile' has no declared producer owner or consumer epic"
            }
        }
        foreach ($matrixId in $matrices) {
            if (-not $matrixById.ContainsKey($matrixId)) {
                throw "validate-contract-clause-disposition: $clauseId names unknown matrix '$matrixId'"
            }
        }
    }

    Assert-ExactClauseSet -Expected $contractClauses -Actual @($rows | ForEach-Object { [string]$_.clause_id }) -Owner "contract clause set versus disposition ledger"
    Assert-ExactClauseSet -Expected $allowedDeferred -Actual @($rows | Where-Object { [string]$_.disposition -eq "deferred-extended" } | ForEach-Object { [string]$_.clause_id }) -Owner "deferred extended clause set"

    $traceAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.bead_traceability)
    $traceRows = @(Import-Csv -LiteralPath $traceAbs)
    $seenClauses = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seenOwnersByClause = @{}
    $seenConsumersByClause = @{}
    $seenMatricesByClause = @{}
    foreach ($row in $rows | Where-Object { [string]$_.disposition -eq "in-scope" }) {
        $clauseId = [string]$row.clause_id
        $seenOwnersByClause[$clauseId] = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        $seenConsumersByClause[$clauseId] = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        $seenMatricesByClause[$clauseId] = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    }

    foreach ($trace in $traceRows) {
        $parentEpic = [string]$trace.parent_epic
        $matrixId = [string]$trace.matrix_id
        $profile = [string]$trace.profile
        foreach ($clauseId in @(Get-IdealContractClauseIds -Text ([string]$trace.contract_clauses))) {
            if (-not $rowByClause.ContainsKey($clauseId)) {
                throw "validate-contract-clause-disposition: trace references clause '$clauseId' absent from disposition ledger"
            }
            $disposition = $rowByClause[$clauseId]
            if ([string]$disposition.disposition -ne "in-scope") {
                throw "validate-contract-clause-disposition: trace claims deferred clause '$clauseId' as current work"
            }
            $profiles = @(Split-IdealPipeList -Value ([string]$disposition.profiles) -Owner "$clauseId profiles")
            $owners = @(Split-IdealPipeList -Value ([string]$disposition.owner_epics) -Owner "$clauseId owner_epics")
            $consumers = @(Split-IdealPipeList -Value ([string]$disposition.consumer_epics) -Owner "$clauseId consumer_epics" -AllowNotApplicable)
            $matrices = @(Split-IdealPipeList -Value ([string]$disposition.matrix_ids) -Owner "$clauseId matrix_ids")
            if ($profile -notin $profiles) {
                throw "validate-contract-clause-disposition: trace '$($trace.bead_id)/$matrixId' routes $clauseId through undeclared profile '$profile'"
            }
            if ($parentEpic -notin $owners -and $parentEpic -notin $consumers) {
                throw "validate-contract-clause-disposition: trace '$($trace.bead_id)/$matrixId' routes $clauseId through undeclared producer/consumer epic '$parentEpic'"
            }
            if ($matrixId -notin $matrices) {
                throw "validate-contract-clause-disposition: trace '$($trace.bead_id)/$matrixId' routes $clauseId through undeclared matrix '$matrixId'"
            }
            [void]$seenClauses.Add($clauseId)
            if ($parentEpic -in $owners) {
                [void]$seenOwnersByClause[$clauseId].Add($parentEpic)
            }
            else {
                [void]$seenConsumersByClause[$clauseId].Add($parentEpic)
            }
            [void]$seenMatricesByClause[$clauseId].Add($matrixId)
        }
    }

    foreach ($row in $rows | Where-Object { [string]$_.disposition -eq "in-scope" }) {
        $clauseId = [string]$row.clause_id
        if (-not $seenClauses.Contains($clauseId)) {
            throw "validate-contract-clause-disposition: in-scope clause $clauseId has no current row trace"
        }
        foreach ($ownerId in @(Split-IdealPipeList -Value ([string]$row.owner_epics) -Owner "$clauseId owner_epics")) {
            if (-not $seenOwnersByClause[$clauseId].Contains($ownerId)) {
                throw "validate-contract-clause-disposition: clause $clauseId owner '$ownerId' has no clause-bearing trace"
            }
        }
        foreach ($consumerId in @(Split-IdealPipeList -Value ([string]$row.consumer_epics) -Owner "$clauseId consumer_epics" -AllowNotApplicable)) {
            if (-not $seenConsumersByClause[$clauseId].Contains($consumerId)) {
                throw "validate-contract-clause-disposition: clause $clauseId consumer '$consumerId' has no clause-bearing trace"
            }
        }
        foreach ($matrixId in @(Split-IdealPipeList -Value ([string]$row.matrix_ids) -Owner "$clauseId matrix_ids")) {
            if (-not $seenMatricesByClause[$clauseId].Contains($matrixId)) {
                throw "validate-contract-clause-disposition: clause $clauseId matrix '$matrixId' has no clause-bearing trace"
            }
        }
    }

    Write-Host "validate-contract-clause-disposition: ok (program=$($manifest.program_id) clauses=$($rows.Count) in_scope=$(@($rows | Where-Object { $_.disposition -eq 'in-scope' }).Count) deferred=$($allowedDeferred.Count))"
}
finally {
    Pop-Location
}
