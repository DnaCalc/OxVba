param(
    [string]$LedgerPath = "docs/validation/IDEAL_WINDOWS_CURRENT_STACK_RESIDUAL_V1.csv",
    [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
    [string]$IssuesPath = ".beads/issues.jsonl",
    [string]$LegacyMigrationPath = "docs/validation/IDEAL_LEGACY_BEAD_MIGRATION_V1.csv",
    [string]$TraceabilityPath = "docs/validation/IDEAL_MATRIX_BEAD_TRACEABILITY_V1.csv",
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

function Assert-ExactStringSet {
    param(
        [Parameter(Mandatory = $true)][string[]]$Actual,
        [Parameter(Mandatory = $true)][string[]]$Expected,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $difference = @(Compare-Object -ReferenceObject @($Expected | Sort-Object -Unique) -DifferenceObject @($Actual | Sort-Object -Unique))
    if ($difference.Count -gt 0) {
        $missing = @($difference | Where-Object SideIndicator -eq '<=' | ForEach-Object InputObject)
        $unexpected = @($difference | Where-Object SideIndicator -eq '=>' | ForEach-Object InputObject)
        throw "validate-windows-current-stack-residuals: $Owner differs (missing=$($missing -join '|'); unexpected=$($unexpected -join '|'))"
    }
}

function Test-BeadInEpicScope {
    param(
        [Parameter(Mandatory = $true)][string]$BeadId,
        [Parameter(Mandatory = $true)][string]$EpicId,
        [Parameter(Mandatory = $true)][hashtable]$ParentIdsById
    )

    $pending = [Collections.Generic.Queue[string]]::new()
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $pending.Enqueue($BeadId)
    while ($pending.Count -gt 0) {
        $candidate = $pending.Dequeue()
        if (-not $seen.Add($candidate)) {
            continue
        }
        if ($candidate -eq $EpicId) {
            return $true
        }
        if ($ParentIdsById.ContainsKey($candidate)) {
            foreach ($parentId in @($ParentIdsById[$candidate])) {
                $pending.Enqueue($parentId)
            }
        }
    }
    return $false
}

function Get-CheckedPathList {
    param(
        [Parameter(Mandatory = $true)][string]$Text,
        [Parameter(Mandatory = $true)][string]$Owner,
        [Parameter(Mandatory = $true)][string]$RepoRoot
    )

    if ($Text -eq "none") {
        return @()
    }
    if ([string]::IsNullOrWhiteSpace($Text) -or $Text -match ';') {
        throw "validate-windows-current-stack-residuals: $Owner must be 'none' or a pipe-delimited repository path list"
    }
    $paths = @($Text -split '\|' | ForEach-Object { $_.Trim() })
    if (@($paths | Where-Object { [string]::IsNullOrWhiteSpace($_) }).Count -gt 0 -or
        @($paths | Sort-Object -Unique).Count -ne $paths.Count) {
        throw "validate-windows-current-stack-residuals: $Owner contains an empty or duplicate path"
    }
    foreach ($pathWithAnchor in $paths) {
        $path = ($pathWithAnchor -split '#', 2)[0]
        Assert-IdealRelativePath -Path $path -Owner $Owner
        if (-not (Test-Path -LiteralPath (Resolve-IdealRepoPath -RepoRoot $RepoRoot -Path $path) -PathType Leaf)) {
            throw "validate-windows-current-stack-residuals: $Owner does not resolve '$pathWithAnchor'"
        }
    }
    return $paths
}

$expectedColumns = @(
    "matrix_id",
    "row_id",
    "claim_key",
    "canonical_truth_state",
    "current_code_state",
    "current_test_state",
    "historical_evidence_state",
    "gap_kind",
    "current_code_anchors",
    "current_test_anchors",
    "historical_evidence_refs",
    "canonical_owner_epic",
    "live_residual_owner_bead",
    "legacy_route_ids",
    "assessment_notes"
)

$matrixPaths = [ordered]@{
    "WIN-COM-CLIENT" = "docs/validation/WINDOWS_JIT_COM_CLIENT_MATRIX_V1.csv"
    "WIN-COM-EVENTS" = "docs/validation/WINDOWS_JIT_COM_EVENTS_MATRIX_V1.csv"
    "WIN-COM-SERVER" = "docs/validation/WINDOWS_JIT_COM_SERVER_MATRIX_V1.csv"
    "WIN-NATIVE-IMPORT" = "docs/validation/WINDOWS_JIT_NATIVE_IMPORT_MATRIX_V1.csv"
    "WIN-NATIVE-EXPORT" = "docs/validation/WINDOWS_NATIVE_EXPORT_AND_PACKAGING_MATRIX_V1.csv"
    "WIN-ABI-CARRIER" = "docs/validation/WINDOWS_ABI_CARRIER_MATRIX_V1.csv"
}

$allowedCodeStates = @("current-subset", "current-divergence", "absent", "n/a")
$allowedTestStates = @("current-subset", "historical-only", "absent", "n/a")
$allowedHistoricalStates = @("provenance-only", "none")
$allowedGapKinds = @(
    "missing-current-implementation",
    "backend-divergence",
    "known-blocker",
    "missing-controlled-fixture",
    "missing-current-evidence",
    "environment-pending",
    "aggregate-pending"
)
$activeStatuses = @("open", "in_progress", "blocked")
$legacyRouteByKey = @{
    "WIN-COM-EVENTS|WCE-PLAN-INCOMING" = "bd-aprs.8.8.9"
    "WIN-NATIVE-IMPORT|WNI-PLAN-CALLBACK" = "bd-9sed.17"
}
$legacyOwnerAndRolloutById = @{
    "bd-aprs.8.8.9" = @("bd-59co.3.6", "bd-59co.3.6.1")
    "bd-9sed.17" = @("bd-59co.3.11", "bd-59co.3.11.1")
}
$supportOwnerByKey = @{
    "WIN-ABI-CARRIER|WAC-TARGET-DEV-ENV" = "bd-59co.3.1.2"
    "WIN-ABI-CARRIER|WAC-CLEAN-CERT-ENV" = "bd-59co.3.15.3"
    "WIN-NATIVE-EXPORT|WNE-PROFILE-TOOL-TERMINAL" = "bd-59co.3.16.1"
    "WIN-ABI-CARRIER|WAC-PROFILE-TERMINAL" = "bd-59co.3.16.1"
}
$expectedDivergenceCodeKeys = @("WIN-COM-EVENTS|WCE-PLAN-INCOMING")
$expectedAbsentCodeKeys = @(
    "WIN-NATIVE-EXPORT|WNE-PLAN-NATIVE",
    "WIN-NATIVE-EXPORT|WNE-NATIVE-EXE",
    "WIN-NATIVE-EXPORT|WNE-NATIVE-ABI-BREADTH",
    "WIN-NATIVE-EXPORT|WNE-NATIVE-REPRO-DEPLOY",
    "WIN-ABI-CARRIER|WAC-VERIFIED-INTEROP-PLAN"
)
$expectedNotApplicableCodeKeys = @(
    "WIN-COM-CLIENT|WCC-EXCEL-AUTHORITY",
    "WIN-NATIVE-EXPORT|WNE-PROFILE-TOOL-TERMINAL",
    "WIN-ABI-CARRIER|WAC-TARGET-DEV-ENV",
    "WIN-ABI-CARRIER|WAC-CLEAN-CERT-ENV",
    "WIN-ABI-CARRIER|WAC-RELEASE-CERT",
    "WIN-ABI-CARRIER|WAC-EXCEL-COM-CERT",
    "WIN-ABI-CARRIER|WAC-EXCEL-NATIVE-CERT",
    "WIN-ABI-CARRIER|WAC-PROFILE-TERMINAL"
)
$expectedHistoricalTestKeys = @(
    "WIN-COM-CLIENT|WCC-EXCEL-AUTHORITY",
    "WIN-COM-SERVER|WCS-LATE-LOCALSERVER",
    "WIN-COM-SERVER|WCS-IMPLEMENTS-CUSTOM",
    "WIN-COM-SERVER|WCS-EARLY-OUTPROC",
    "WIN-NATIVE-EXPORT|WNE-PLAN-NATIVE",
    "WIN-NATIVE-EXPORT|WNE-NATIVE-EXE",
    "WIN-NATIVE-EXPORT|WNE-NATIVE-ABI-BREADTH",
    "WIN-NATIVE-EXPORT|WNE-NATIVE-REPRO-DEPLOY",
    "WIN-ABI-CARRIER|WAC-CARRIER-EXCEL-ROUNDTRIP",
    "WIN-ABI-CARRIER|WAC-RELEASE-CERT",
    "WIN-ABI-CARRIER|WAC-EXCEL-COM-CERT",
    "WIN-ABI-CARRIER|WAC-EXCEL-NATIVE-CERT"
)
$expectedAbsentTestKeys = @(
    "WIN-ABI-CARRIER|WAC-TARGET-DEV-ENV",
    "WIN-ABI-CARRIER|WAC-VERIFIED-INTEROP-PLAN",
    "WIN-ABI-CARRIER|WAC-CLEAN-CERT-ENV"
)
$expectedNotApplicableTestKeys = @(
    "WIN-NATIVE-EXPORT|WNE-PROFILE-TOOL-TERMINAL",
    "WIN-ABI-CARRIER|WAC-PROFILE-TERMINAL"
)
$expectedNoHistoricalEvidenceKeys = @(
    "WIN-NATIVE-EXPORT|WNE-PROFILE-TOOL-TERMINAL",
    "WIN-ABI-CARRIER|WAC-TARGET-DEV-ENV",
    "WIN-ABI-CARRIER|WAC-VERIFIED-INTEROP-PLAN",
    "WIN-ABI-CARRIER|WAC-WINDOWS-DESCRIPTORS",
    "WIN-ABI-CARRIER|WAC-CLEAN-CERT-ENV",
    "WIN-ABI-CARRIER|WAC-PROFILE-TERMINAL"
)
$backendDivergenceKeys = @(
    "WIN-COM-CLIENT|WCC-PLAN-LATE", "WIN-COM-CLIENT|WCC-LATE-ARGS",
    "WIN-COM-CLIENT|WCC-LATE-STRUCTURAL", "WIN-COM-CLIENT|WCC-LATE-OUTPROC-ERROR",
    "WIN-COM-CLIENT|WCC-PLAN-EARLY", "WIN-COM-CLIENT|WCC-EARLY-COMPLEX",
    "WIN-COM-CLIENT|WCC-EARLY-CUSTOM", "WIN-COM-CLIENT|WCC-EARLY-OUTPROC",
    "WIN-COM-SERVER|WCS-LATE-INPROC", "WIN-COM-SERVER|WCS-LATE-LOCALSERVER",
    "WIN-COM-SERVER|WCS-LATE-COMPLEX", "WIN-COM-SERVER|WCS-DUAL-INPROC",
    "WIN-COM-SERVER|WCS-IMPLEMENTS-CUSTOM", "WIN-COM-SERVER|WCS-EARLY-OUTPROC",
    "WIN-COM-SERVER|WCS-SERVER-SAFETY",
    "WIN-NATIVE-IMPORT|WNI-PLAN-DECLARE", "WIN-NATIVE-IMPORT|WNI-DECLARE-STRINGS",
    "WIN-NATIVE-IMPORT|WNI-DECLARE-STRUCTURAL", "WIN-NATIVE-IMPORT|WNI-DECLARE-LOADER-ERROR",
    "WIN-NATIVE-IMPORT|WNI-POINTER-HELPERS", "WIN-NATIVE-IMPORT|WNI-CALLBACK-SYNC",
    "WIN-NATIVE-EXPORT|WNE-WRAPPER-EXE", "WIN-NATIVE-EXPORT|WNE-WRAPPER-LIBRARY",
    "WIN-NATIVE-EXPORT|WNE-PLAN-WRAPPED"
)
$missingFixtureKeys = @(
    "WIN-ABI-CARRIER|WAC-BSTR-LAYOUT",
    "WIN-ABI-CARRIER|WAC-VARIANT-LAYOUT",
    "WIN-ABI-CARRIER|WAC-SAFEARRAY-LAYOUT",
    "WIN-ABI-CARRIER|WAC-IUNKNOWN-IDENTITY",
    "WIN-ABI-CARRIER|WAC-NUMERIC-LONGPTR"
)
$missingEvidenceKeys = @(
    "WIN-COM-CLIENT|WCC-EXCEL-AUTHORITY",
    "WIN-ABI-CARRIER|WAC-CARRIER-EXCEL-ROUNDTRIP",
    "WIN-ABI-CARRIER|WAC-SAFETY-MUTATION",
    "WIN-ABI-CARRIER|WAC-RELEASE-CERT",
    "WIN-ABI-CARRIER|WAC-EXCEL-COM-CERT",
    "WIN-ABI-CARRIER|WAC-EXCEL-NATIVE-CERT"
)
$environmentPendingKeys = @(
    "WIN-ABI-CARRIER|WAC-TARGET-DEV-ENV",
    "WIN-ABI-CARRIER|WAC-CLEAN-CERT-ENV"
)
$aggregatePendingKeys = @(
    "WIN-NATIVE-EXPORT|WNE-PROFILE-TOOL-TERMINAL",
    "WIN-ABI-CARRIER|WAC-PROFILE-TERMINAL"
)

Push-Location $repoRoot
try {
    [void](Read-IdealProgramManifest -RepoRoot $repoRoot -ManifestPath $ManifestPath)
    $ledgerAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $LedgerPath
    $actualColumns = @(Get-IdealCsvColumns -Path $ledgerAbs)
    $schemaDrifted = @($actualColumns).Count -ne @($expectedColumns).Count
    if (-not $schemaDrifted) {
        for ($columnIndex = 0; $columnIndex -lt @($expectedColumns).Count; $columnIndex++) {
            if ([string]$actualColumns[$columnIndex] -ne [string]$expectedColumns[$columnIndex]) {
                $schemaDrifted = $true
                break
            }
        }
    }
    if ($schemaDrifted) {
        throw "validate-windows-current-stack-residuals: ledger schema or column order drifted"
    }
    $ledgerRows = @(Import-Csv -LiteralPath $ledgerAbs)
    if ($ledgerRows.Count -ne 57) {
        throw "validate-windows-current-stack-residuals: ledger must contain exactly 57 rows; found $($ledgerRows.Count)"
    }

    $canonicalByKey = @{}
    foreach ($matrix in $matrixPaths.GetEnumerator()) {
        $rows = @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $matrix.Value))
        foreach ($row in $rows) {
            $key = "$($matrix.Key)|$([string]$row.row_id)"
            if ($canonicalByKey.ContainsKey($key)) {
                throw "validate-windows-current-stack-residuals: duplicate canonical row '$key'"
            }
            $canonicalByKey[$key] = $row
        }
    }
    if ($canonicalByKey.Count -ne 57) {
        throw "validate-windows-current-stack-residuals: canonical Windows inventory must contain exactly 57 rows"
    }

    $issues = Read-IdealIssues -RepoRoot $repoRoot -IssuesPath $IssuesPath
    $issueById = $issues.IssueById
    $parentIdsById = @{}
    foreach ($issue in @($issues.Issues)) {
        $parentIdsById[[string]$issue.id] = @(Get-IdealParentIds -Issue $issue)
    }

    $seenKeys = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($row in $ledgerRows) {
        $key = "$([string]$row.matrix_id)|$([string]$row.row_id)"
        if (-not $seenKeys.Add($key)) {
            throw "validate-windows-current-stack-residuals: duplicate ledger row '$key'"
        }
        if (-not $canonicalByKey.ContainsKey($key)) {
            throw "validate-windows-current-stack-residuals: ledger row '$key' is not canonical"
        }
        $canonical = $canonicalByKey[$key]
        if ([string]$canonical.truth_state -ne "planned" -or [string]$row.canonical_truth_state -ne "planned") {
            throw "validate-windows-current-stack-residuals: '$key' must remain planned; current-stack characterization is not capability credit"
        }
        if ([string]$row.claim_key -ne [string]$canonical.claim_key -or
            [string]$row.canonical_owner_epic -ne [string]$canonical.owner_epic) {
            throw "validate-windows-current-stack-residuals: '$key' claim or canonical owner disagrees with its matrix"
        }
        if ([string]$row.current_code_state -notin $allowedCodeStates -or
            [string]$row.current_test_state -notin $allowedTestStates -or
            [string]$row.historical_evidence_state -notin $allowedHistoricalStates -or
            [string]$row.gap_kind -notin $allowedGapKinds) {
            throw "validate-windows-current-stack-residuals: '$key' uses a value outside the closed residual vocabulary"
        }
        $expectedCodeState = if ($key -in $expectedDivergenceCodeKeys) { "current-divergence" }
            elseif ($key -in $expectedAbsentCodeKeys) { "absent" }
            elseif ($key -in $expectedNotApplicableCodeKeys) { "n/a" }
            else { "current-subset" }
        $expectedTestState = if ($key -in $expectedHistoricalTestKeys) { "historical-only" }
            elseif ($key -in $expectedAbsentTestKeys) { "absent" }
            elseif ($key -in $expectedNotApplicableTestKeys) { "n/a" }
            else { "current-subset" }
        $expectedHistoricalState = if ($key -in $expectedNoHistoricalEvidenceKeys) { "none" } else { "provenance-only" }
        $expectedGapKind = if ($key -in $backendDivergenceKeys) { "backend-divergence" }
            elseif ($key -eq "WIN-COM-EVENTS|WCE-PLAN-INCOMING") { "known-blocker" }
            elseif ($key -in $missingFixtureKeys) { "missing-controlled-fixture" }
            elseif ($key -in $missingEvidenceKeys) { "missing-current-evidence" }
            elseif ($key -in $environmentPendingKeys) { "environment-pending" }
            elseif ($key -in $aggregatePendingKeys) { "aggregate-pending" }
            else { "missing-current-implementation" }
        if ([string]$row.current_code_state -ne $expectedCodeState -or
            [string]$row.current_test_state -ne $expectedTestState -or
            [string]$row.historical_evidence_state -ne $expectedHistoricalState -or
            [string]$row.gap_kind -ne $expectedGapKind) {
            throw "validate-windows-current-stack-residuals: '$key' characterization drifted; expected code=$expectedCodeState test=$expectedTestState historical=$expectedHistoricalState gap=$expectedGapKind"
        }
        if ([string]::IsNullOrWhiteSpace([string]$row.assessment_notes)) {
            throw "validate-windows-current-stack-residuals: '$key' has no assessment note"
        }

        $codePaths = @(Get-CheckedPathList -Text ([string]$row.current_code_anchors) -Owner "$key current_code_anchors" -RepoRoot $repoRoot)
        $testPaths = @(Get-CheckedPathList -Text ([string]$row.current_test_anchors) -Owner "$key current_test_anchors" -RepoRoot $repoRoot)
        $historicalPaths = @(Get-CheckedPathList -Text ([string]$row.historical_evidence_refs) -Owner "$key historical_evidence_refs" -RepoRoot $repoRoot)
        if (([string]$row.current_code_state -in @("current-subset", "current-divergence")) -ne ($codePaths.Count -gt 0)) {
            throw "validate-windows-current-stack-residuals: '$key' code state and anchors disagree"
        }
        if (([string]$row.current_test_state -eq "current-subset") -ne ($testPaths.Count -gt 0)) {
            throw "validate-windows-current-stack-residuals: '$key' current test state and anchors disagree"
        }
        if (([string]$row.historical_evidence_state -eq "provenance-only") -ne ($historicalPaths.Count -gt 0)) {
            throw "validate-windows-current-stack-residuals: '$key' historical evidence state and references disagree"
        }
        if (@($testPaths | Where-Object { $_.Replace('\', '/') -like 'docs/evidence/*' }).Count -gt 0) {
            throw "validate-windows-current-stack-residuals: '$key' credits historical evidence as a current test"
        }
        if (@($codePaths | Where-Object { $_.Replace('\', '/') -notlike 'crates/*' }).Count -gt 0) {
            throw "validate-windows-current-stack-residuals: '$key' current code anchors must resolve under crates/"
        }
        if (@($testPaths | Where-Object { $_.Replace('\', '/') -notlike 'crates/*' }).Count -gt 0) {
            throw "validate-windows-current-stack-residuals: '$key' current test anchors must resolve under crates/"
        }
        if (@($historicalPaths | Where-Object { $_.Replace('\', '/') -notlike 'docs/evidence/*' }).Count -gt 0) {
            throw "validate-windows-current-stack-residuals: '$key' historical references must stay under docs/evidence"
        }

        switch ([string]$row.gap_kind) {
            "backend-divergence" {
                if ([string]$row.current_code_state -ne "current-subset" -or
                    @($codePaths | Where-Object { $_.Replace('\', '/') -eq 'crates/oxvba-jit/src/lib.rs' }).Count -eq 0) {
                    throw "validate-windows-current-stack-residuals: '$key' backend divergence must show a current subset and the JIT boundary"
                }
            }
            "known-blocker" {
                if ($key -ne "WIN-COM-EVENTS|WCE-PLAN-INCOMING" -or [string]$row.current_code_state -ne "current-divergence") {
                    throw "validate-windows-current-stack-residuals: '$key' is not the admitted synchronous ByRef event blocker"
                }
            }
            "missing-controlled-fixture" {
                if ([string]$row.current_code_state -ne "current-subset" -or [string]$row.current_test_state -ne "current-subset") {
                    throw "validate-windows-current-stack-residuals: '$key' fixture gap must be backed by a current implementation/test subset"
                }
            }
            "environment-pending" {
                if ([string]$row.current_code_state -ne "n/a" -or [string]$row.current_test_state -ne "absent" -or [string]$row.historical_evidence_state -ne "none") {
                    throw "validate-windows-current-stack-residuals: '$key' environment gap has implementation or evidence credit"
                }
            }
            "aggregate-pending" {
                if ([string]$row.current_code_state -ne "n/a" -or [string]$row.current_test_state -ne "n/a" -or [string]$row.historical_evidence_state -ne "none") {
                    throw "validate-windows-current-stack-residuals: '$key' aggregate gap has premature implementation or evidence credit"
                }
            }
        }

        $expectedLegacy = if ($legacyRouteByKey.ContainsKey($key)) { $legacyRouteByKey[$key] } else { "none" }
        if ([string]$row.legacy_route_ids -ne $expectedLegacy) {
            throw "validate-windows-current-stack-residuals: '$key' legacy route must be '$expectedLegacy'"
        }

        $ownerId = [string]$row.live_residual_owner_bead
        $epicId = [string]$row.canonical_owner_epic
        if (-not $issueById.ContainsKey($ownerId) -or [string]$issueById[$ownerId].status -notin $activeStatuses) {
            throw "validate-windows-current-stack-residuals: '$key' has no active live residual owner '$ownerId'"
        }
        if (-not (Test-BeadInEpicScope -BeadId $ownerId -EpicId $epicId -ParentIdsById $parentIdsById)) {
            throw "validate-windows-current-stack-residuals: '$key' residual owner '$ownerId' is outside '$epicId'"
        }
        $labels = @(Get-IdealIssueLabels -Issue $issueById[$ownerId])
        if ("profile-win-x64" -notin $labels) {
            throw "validate-windows-current-stack-residuals: '$key' residual owner '$ownerId' is not Windows x64"
        }
        if ($supportOwnerByKey.ContainsKey($key)) {
            if ($ownerId -ne $supportOwnerByKey[$key] -or "support" -notin $labels) {
                throw "validate-windows-current-stack-residuals: '$key' must use exact support/control owner '$($supportOwnerByKey[$key])'"
            }
        }
        elseif ("delivery" -notin $labels) {
            throw "validate-windows-current-stack-residuals: '$key' capability residual cannot be parked on support-only owner '$ownerId'"
        }
    }
    Assert-ExactStringSet -Actual @($seenKeys) -Expected @($canonicalByKey.Keys) -Owner "ledger row identity set"

    $migrationRows = @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $LegacyMigrationPath))
    $traceRows = @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $TraceabilityPath))
    foreach ($entry in $legacyRouteByKey.GetEnumerator()) {
        $parts = @($entry.Key -split '\|', 2)
        $legacyId = [string]$entry.Value
        $expectedOwnerAndRollout = @($legacyOwnerAndRolloutById[$legacyId])
        $migration = @($migrationRows | Where-Object { [string]$_.legacy_id -eq $legacyId })
        if ($migration.Count -ne 1 -or [string]$migration[0].disposition -ne "imported-in-place" -or
            [string]$migration[0].status_after -ne "open" -or [string]$migration[0].terminal_effect -ne "delivery" -or
            [string]$migration[0].successor_owner_ids -ne $expectedOwnerAndRollout[0] -or
            [string]$migration[0].successor_rollout_ids -ne $expectedOwnerAndRollout[1]) {
            throw "validate-windows-current-stack-residuals: legacy route '$legacyId' is not retained as open imported delivery work"
        }
        if (-not $issueById.ContainsKey($legacyId) -or [string]$issueById[$legacyId].status -notin $activeStatuses -or
            "delivery" -notin @(Get-IdealIssueLabels -Issue $issueById[$legacyId])) {
            throw "validate-windows-current-stack-residuals: imported legacy route '$legacyId' is not an active delivery bead"
        }
        $trace = @($traceRows | Where-Object {
            [string]$_.bead_id -eq $legacyId -and [string]$_.matrix_id -eq $parts[0] -and [string]$_.row_id -eq $parts[1]
        })
        if ($trace.Count -ne 1 -or [string]$trace[0].effect -ne "delivery" -or [string]$trace[0].profile -ne "windows-x64") {
            throw "validate-windows-current-stack-residuals: legacy route '$legacyId' lacks its exact Windows delivery trace"
        }
    }

    Write-Host "validate-windows-current-stack-residuals: ok (57 planned rows characterized; no capability-state advancement)"
}
finally {
    Pop-Location
}
