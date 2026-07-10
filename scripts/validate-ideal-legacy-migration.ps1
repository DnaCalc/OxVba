param(
    [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
    [string]$IssuesPath = ".beads/issues.jsonl"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
. (Join-Path $PSScriptRoot "lib-ideal-program-validation.ps1")

function Invoke-BrMigrationJson {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)

    $output = @(& br @Arguments --no-auto-flush)
    if ($LASTEXITCODE -ne 0) {
        throw "validate-ideal-legacy-migration: br $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
    return (($output -join "`n") | ConvertFrom-Json)
}

function Split-MigrationIds {
    param([AllowEmptyString()][string]$Value)

    return @($Value -split '\|' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
}

Push-Location $repoRoot
try {
    $manifestContext = Read-IdealProgramManifest -RepoRoot $repoRoot -ManifestPath $ManifestPath
    $manifest = $manifestContext.Manifest
    $migrationPath = [string]$manifest.legacy_migration
    Assert-IdealRelativePath -Path $migrationPath -Owner "manifest.legacy_migration"
    $migrationAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $migrationPath
    if (-not (Test-Path -LiteralPath $migrationAbs -PathType Leaf)) {
        throw "validate-ideal-legacy-migration: missing $migrationPath"
    }
    $expectedColumns = @("legacy_id", "disposition", "status_after", "successor_owner_ids", "successor_rollout_ids", "terminal_effect", "notes")
    if (($expectedColumns -join ',') -ne ((Get-IdealCsvColumns -Path $migrationAbs) -join ',')) {
        throw "validate-ideal-legacy-migration: migration CSV header does not match V1"
    }

    $rows = @(Import-Csv -LiteralPath $migrationAbs)
    if ($rows.Count -ne 42) {
        throw "validate-ideal-legacy-migration: expected exactly 42 legacy rows, found $($rows.Count)"
    }
    $expectedDispositionCounts = @{
        "imported-in-place" = 5
        "superseded-split" = 24
        "already-satisfied" = 6
        "tombstoned-tracker" = 5
        "deferred-profile-ext" = 2
    }
    foreach ($disposition in $expectedDispositionCounts.Keys) {
        $count = @($rows | Where-Object { [string]$_.disposition -eq $disposition }).Count
        if ($count -ne $expectedDispositionCounts[$disposition]) {
            throw "validate-ideal-legacy-migration: disposition '$disposition' expected $($expectedDispositionCounts[$disposition]), found $count"
        }
    }

    $issueContext = Read-IdealIssues -RepoRoot $repoRoot -IssuesPath $IssuesPath
    $issueById = $issueContext.IssueById
    $directedMigrationActive = [string]$issueById[[string]$manifest.control_epic].status -ne "closed"
    $childrenByParent = New-IdealChildrenMap -Issues @($issueContext.Issues)
    $programDescendants = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($id in @(Get-IdealDescendantIds -RootId ([string]$manifest.root_bead) -ChildrenByParent $childrenByParent)) {
        [void]$programDescendants.Add($id)
    }
    $expectedEpicIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($epic in @(Get-IdealExpectedEpicRecords -Manifest $manifest)) {
        [void]$expectedEpicIds.Add($epic.EpicId)
    }
    $traceAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.bead_traceability)
    $tracedBeads = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($trace in @(Import-Csv -LiteralPath $traceAbs)) {
        [void]$tracedBeads.Add([string]$trace.bead_id)
    }

    $ledgerIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($row in $rows) {
        $legacyId = [string]$row.legacy_id
        if ([string]::IsNullOrWhiteSpace($legacyId) -or -not $ledgerIds.Add($legacyId)) {
            throw "validate-ideal-legacy-migration: blank or duplicate legacy_id '$legacyId'"
        }
        if (-not $issueById.ContainsKey($legacyId)) {
            throw "validate-ideal-legacy-migration: legacy issue '$legacyId' does not exist"
        }
        if ((Get-IdealIssueLabels -Issue $issueById[$legacyId]) -notcontains "legacy-reconciled") {
            throw "validate-ideal-legacy-migration: issue '$legacyId' lacks legacy-reconciled label"
        }
        $actualStatus = [string]$issueById[$legacyId].status
        $statusMatches = $actualStatus -eq [string]$row.status_after
        if ([string]$row.disposition -eq "imported-in-place" -and -not $directedMigrationActive) {
            $statusMatches = [string]$row.status_after -eq "open" -and $actualStatus -in @("open", "in_progress", "blocked", "closed")
        }
        if (-not $statusMatches) {
            throw "validate-ideal-legacy-migration: issue '$legacyId' status '$actualStatus' disagrees with migration state '$($row.status_after)'"
        }
        if ([string]::IsNullOrWhiteSpace([string]$row.notes)) {
            throw "validate-ideal-legacy-migration: issue '$legacyId' has no migration note"
        }

        $owners = @(Split-MigrationIds -Value ([string]$row.successor_owner_ids))
        $rollouts = @(Split-MigrationIds -Value ([string]$row.successor_rollout_ids))
        if ([string]$row.disposition -ne "deferred-profile-ext") {
            foreach ($ownerId in $owners) {
                if (-not $expectedEpicIds.Contains($ownerId)) {
                    throw "validate-ideal-legacy-migration: issue '$legacyId' successor owner '$ownerId' is not a manifest epic"
                }
            }
            if ($owners.Count -ne $rollouts.Count) {
                throw "validate-ideal-legacy-migration: issue '$legacyId' successor owner/rollout counts differ"
            }
            for ($index = 0; $index -lt $rollouts.Count; $index++) {
                $rolloutId = $rollouts[$index]
                if (-not $issueById.ContainsKey($rolloutId) -or
                    (Get-IdealIssueLabels -Issue $issueById[$rolloutId]) -notcontains "rollout" -or
                    @(Get-IdealParentIds -Issue $issueById[$rolloutId]) -notcontains $owners[$index]) {
                    throw "validate-ideal-legacy-migration: issue '$legacyId' rollout '$rolloutId' does not belong to '$($owners[$index])'"
                }
            }
        }

        switch ([string]$row.disposition) {
            "imported-in-place" {
                if ($owners.Count -ne 1 -or $rollouts.Count -ne 1 -or [string]$row.terminal_effect -ne "delivery") {
                    throw "validate-ideal-legacy-migration: imported issue '$legacyId' must have one owner/rollout and delivery effect"
                }
                if (-not $programDescendants.Contains($legacyId) -or
                    @(Get-IdealParentIds -Issue $issueById[$legacyId]) -notcontains $owners[0] -or
                    ($childrenByParent.ContainsKey($legacyId) -and @($childrenByParent[$legacyId]).Count -gt 0) -or
                    -not $tracedBeads.Contains($legacyId)) {
                    throw "validate-ideal-legacy-migration: imported issue '$legacyId' is not a current traced leaf of '$($owners[0])'"
                }
                foreach ($label in @([string]$manifest.program_label, "delivery")) {
                    if ((Get-IdealIssueLabels -Issue $issueById[$legacyId]) -notcontains $label) {
                        throw "validate-ideal-legacy-migration: imported issue '$legacyId' lacks '$label'"
                    }
                }
            }
            "superseded-split" {
                if ($owners.Count -eq 0 -or [string]$row.status_after -ne "closed" -or [string]$row.terminal_effect -ne "none") {
                    throw "validate-ideal-legacy-migration: superseded issue '$legacyId' must be closed with successors"
                }
                if ($programDescendants.Contains($legacyId)) {
                    throw "validate-ideal-legacy-migration: superseded issue '$legacyId' remains inside the current program"
                }
            }
            { $_ -in @("already-satisfied", "tombstoned-tracker") } {
                if ([string]$row.status_after -ne "closed" -or [string]$row.terminal_effect -ne "none" -or $programDescendants.Contains($legacyId)) {
                    throw "validate-ideal-legacy-migration: retired issue '$legacyId' has invalid terminal disposition"
                }
            }
            "deferred-profile-ext" {
                if ([string]$row.status_after -ne "deferred" -or
                    [string]$row.successor_owner_ids -ne "PROFILE-EXT-001" -or
                    $rollouts.Count -ne 0 -or
                    [string]$row.terminal_effect -ne "outside-umbrella" -or
                    $programDescendants.Contains($legacyId)) {
                    throw "validate-ideal-legacy-migration: PROFILE-EXT issue '$legacyId' is not cleanly deferred outside the umbrella"
                }
            }
            default { throw "validate-ideal-legacy-migration: issue '$legacyId' has unknown disposition '$($row.disposition)'" }
        }
    }

    $labelledIds = @(
        $issueContext.Issues |
            Where-Object { (Get-IdealIssueLabels -Issue $_) -contains "legacy-reconciled" } |
            ForEach-Object { [string]$_.id }
    )
    if (@(Compare-Object -ReferenceObject @($ledgerIds) -DifferenceObject @($labelledIds)).Count -ne 0) {
        throw "validate-ideal-legacy-migration: ledger IDs do not exactly match legacy-reconciled issue labels"
    }

    $brListResult = Invoke-BrMigrationJson -Arguments @("list", "-a", "--deferred", "-l", "legacy-reconciled", "--limit", "0", "--json")
    $brIssues = if ($brListResult.PSObject.Properties.Name -contains "issues") { @($brListResult.issues) } else { @($brListResult) }
    if (@(Compare-Object -ReferenceObject @($ledgerIds) -DifferenceObject @($brIssues | ForEach-Object { [string]$_.id })).Count -ne 0) {
        throw "validate-ideal-legacy-migration: br legacy-reconciled population disagrees with the ledger"
    }
    foreach ($brIssue in $brIssues) {
        if ([string]$brIssue.status -ne [string]$issueById[[string]$brIssue.id].status) {
            throw "validate-ideal-legacy-migration: br status for '$($brIssue.id)' disagrees with the exported issue state"
        }
    }

    $globalReady = @(Invoke-BrMigrationJson -Arguments @("ready", "--limit", "0", "--json"))
    foreach ($ready in $globalReady) {
        $readyId = [string]$ready.id
        if (-not $programDescendants.Contains($readyId)) {
            throw "validate-ideal-legacy-migration: global ready contains unlisted legacy/non-program issue '$readyId'"
        }
        if ($ledgerIds.Contains($readyId) -and [string]($rows | Where-Object legacy_id -eq $readyId).disposition -eq "deferred-profile-ext") {
            throw "validate-ideal-legacy-migration: deferred PROFILE-EXT issue '$readyId' entered ready"
        }
    }

    Write-Host "validate-ideal-legacy-migration: ok (rows=42 imported=5 superseded=24 satisfied=6 tracker=5 deferred=2)"
}
finally {
    Pop-Location
}
