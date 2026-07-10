param(
    [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
    [string]$IssuesPath = ".beads/issues.jsonl"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
. (Join-Path $PSScriptRoot "lib-ideal-program-validation.ps1")

Push-Location $repoRoot
try {
    $manifestContext = Read-IdealProgramManifest -RepoRoot $repoRoot -ManifestPath $ManifestPath
    $manifest = $manifestContext.Manifest
    $issueContext = Read-IdealIssues -RepoRoot $repoRoot -IssuesPath $IssuesPath
    $issueById = $issueContext.IssueById
    $childrenByParent = New-IdealChildrenMap -Issues @($issueContext.Issues)
    $programDescendants = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($id in @(Get-IdealDescendantIds -RootId ([string]$manifest.root_bead) -ChildrenByParent $childrenByParent)) {
        [void]$programDescendants.Add($id)
    }

    $expectedEpics = @(Get-IdealExpectedEpicRecords -Manifest $manifest)
    $epicProfileById = @{}
    foreach ($epic in $expectedEpics) {
        $epicProfileById[$epic.EpicId] = $epic.Profile
    }

    $ownershipAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.matrix_ownership)
    $ownershipRows = @(Import-Csv -LiteralPath $ownershipAbs)
    $truthStates = @("planned", "in-progress", "implemented-subset", "implemented-full", "verified", "archived")
    $componentStates = @("n/a", "planned", "in-progress", "implemented-subset", "implemented-full", "verified")
    $residualDispositions = @("remaining-accepted-scope", "intentional-boundary", "external-boundary")
    $seenRowIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $primaryClaimKeys = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $totalRows = 0

    foreach ($owner in $ownershipRows) {
        if ([string]$owner.role -ne "primary") {
            continue
        }
        $matrixAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$owner.path)
        foreach ($row in @(Import-Csv -LiteralPath $matrixAbs)) {
            $claimKey = [string]$row.claim_key
            if ([string]::IsNullOrWhiteSpace($claimKey) -or -not $primaryClaimKeys.Add($claimKey)) {
                throw "validate-closure-taxonomy: blank or duplicate primary claim_key '$claimKey' in $($owner.matrix_id)"
            }
        }
    }

    foreach ($owner in $ownershipRows) {
        $matrixId = [string]$owner.matrix_id
        $matrixAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$owner.path)
        $rows = @(Import-Csv -LiteralPath $matrixAbs)
        $matrixClaimKeys = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($row in $rows) {
            $totalRows++
            $rowId = [string]$row.row_id
            $claimKey = [string]$row.claim_key
            $truthState = [string]$row.truth_state
            $rowOwner = [string]$row.owner_epic
            $rowProfile = [string]$row.profile
            $truthRole = [string]$row.truth_role
            $required = ConvertFrom-IdealBoolean -Value ([string]$row.required) -Owner "$matrixId/$rowId required"

            if ([string]::IsNullOrWhiteSpace($rowId) -or -not $seenRowIds.Add($rowId)) {
                throw "validate-closure-taxonomy: blank or duplicate row_id '$rowId' in $matrixId"
            }
            if ($rowId -notlike "$([string]$owner.row_id_prefix)-*") {
                throw "validate-closure-taxonomy: row '$rowId' does not use prefix '$($owner.row_id_prefix)-'"
            }
            if ([string]::IsNullOrWhiteSpace($claimKey) -or -not $matrixClaimKeys.Add($claimKey)) {
                throw "validate-closure-taxonomy: blank or duplicate claim_key '$claimKey' in $matrixId"
            }
            if ($truthRole -ne [string]$owner.role) {
                throw "validate-closure-taxonomy: $matrixId/$rowId truth_role '$truthRole' disagrees with matrix role '$($owner.role)'"
            }
            if ($truthRole -in @("projection", "evidence") -and [string]::IsNullOrWhiteSpace([string]$row.source_claim_key)) {
                throw "validate-closure-taxonomy: $matrixId/$rowId must name source_claim_key for role '$truthRole'"
            }
            if ($truthRole -in @("projection", "evidence") -and -not $primaryClaimKeys.Contains([string]$row.source_claim_key)) {
                throw "validate-closure-taxonomy: $matrixId/$rowId source_claim_key '$($row.source_claim_key)' does not resolve to a primary claim"
            }
            if ($rowProfile -ne [string]$owner.profile) {
                throw "validate-closure-taxonomy: $matrixId/$rowId profile '$rowProfile' disagrees with matrix profile '$($owner.profile)'"
            }
            if (-not $epicProfileById.ContainsKey($rowOwner) -or $epicProfileById[$rowOwner] -ne $rowProfile) {
                throw "validate-closure-taxonomy: $matrixId/$rowId owner '$rowOwner' is not a manifest epic for profile '$rowProfile'"
            }
            if (-not (Test-IdealContractClauses -Text ([string]$row.contract_clauses))) {
                throw "validate-closure-taxonomy: $matrixId/$rowId must name exact, non-wildcard contract clauses"
            }
            if ($truthState -notin $truthStates -or $truthState -eq "implemented") {
                throw "validate-closure-taxonomy: $matrixId/$rowId has invalid truth_state '$truthState'"
            }
            if ($required -and $truthState -eq "archived") {
                throw "validate-closure-taxonomy: required row $matrixId/$rowId cannot be archived"
            }

            foreach ($property in @($row.PSObject.Properties | Where-Object { $_.Name -like "*_state" -and $_.Name -ne "truth_state" })) {
                $value = [string]$property.Value
                if ($value -notin $componentStates) {
                    throw "validate-closure-taxonomy: $matrixId/$rowId column '$($property.Name)' has invalid component state '$value'"
                }
            }

            if ($truthState -eq "verified") {
                foreach ($property in @($row.PSObject.Properties | Where-Object { $_.Name -like "*_state" -and $_.Name -ne "truth_state" })) {
                    if ([string]$property.Value -notin @("verified", "n/a")) {
                        throw "validate-closure-taxonomy: verified row $matrixId/$rowId retains nonterminal state '$($property.Name)=$($property.Value)'"
                    }
                }

                $optionalBlankFields = @(
                    "source_claim_key",
                    "producer_dependencies",
                    "test_anchors",
                    "evidence_refs",
                    "deferred_gate_refs",
                    "divergence_refs",
                    "residual_disposition",
                    "residual_owner_bead",
                    "notes"
                )
                foreach ($property in @($row.PSObject.Properties)) {
                    $name = [string]$property.Name
                    $value = ([string]$property.Value).Trim()
                    if ($name -notin $optionalBlankFields -and [string]::IsNullOrWhiteSpace($value)) {
                        throw "validate-closure-taxonomy: verified row $matrixId/$rowId has blank closure field '$name'; use n/a only when the field is inapplicable"
                    }
                    if ($name -notin @("notes", "residual_disposition", "residual_owner_bead", "deferred_gate_refs", "divergence_refs") -and
                        $value -match '(?i)^(planned|pending|not[- ]?yet|todo|tbd|unknown|unresolved|missing|not[- ]?run|not[- ]?captured)(?:\b|[:/_-])') {
                        throw "validate-closure-taxonomy: verified row $matrixId/$rowId retains placeholder '$name=$value'"
                    }
                }
                $anchorText = @(
                    @([string]$row.test_anchors, [string]$row.evidence_refs) |
                        Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
                        ForEach-Object { $_.Trim() }
                )
                if ($anchorText.Count -eq 0 -or -not (Test-IdealEvidenceReferences -RepoRoot $repoRoot -Text ($anchorText -join ';'))) {
                    throw "validate-closure-taxonomy: verified row $matrixId/$rowId lacks resolvable test/evidence references"
                }
            }

            $residualDisposition = [string]$row.residual_disposition
            $residualOwner = [string]$row.residual_owner_bead
            if ($truthState -ne "verified") {
                if ($residualDisposition -notin $residualDispositions) {
                    throw "validate-closure-taxonomy: $matrixId/$rowId must classify its non-verified residual"
                }
                if ($required -and $residualDisposition -ne "remaining-accepted-scope") {
                    throw "validate-closure-taxonomy: required row $matrixId/$rowId must retain remaining-accepted-scope until verified"
                }
                if ($residualDisposition -eq "remaining-accepted-scope") {
                    if ([string]::IsNullOrWhiteSpace($residualOwner) -or -not $issueById.ContainsKey($residualOwner) -or -not $programDescendants.Contains($residualOwner)) {
                        throw "validate-closure-taxonomy: $matrixId/$rowId remaining accepted scope lacks a current-program residual owner"
                    }
                    if ([string]$issueById[$residualOwner].status -notin @("open", "in_progress", "blocked")) {
                        throw "validate-closure-taxonomy: $matrixId/$rowId residual owner '$residualOwner' is not active"
                    }
                }
            }
            else {
                if ($residualDisposition -eq "remaining-accepted-scope" -or -not [string]::IsNullOrWhiteSpace($residualOwner)) {
                    throw "validate-closure-taxonomy: verified row $matrixId/$rowId cannot retain accepted residual scope or a residual owner"
                }
                if (-not [string]::IsNullOrWhiteSpace($residualDisposition) -and $residualDisposition -notin $residualDispositions) {
                    throw "validate-closure-taxonomy: $matrixId/$rowId has invalid residual_disposition '$residualDisposition'"
                }
            }

            foreach ($dependencyId in @(([string]$row.producer_dependencies -split ';') | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
                if (-not $issueById.ContainsKey($dependencyId) -or
                    ($dependencyId -ne [string]$manifest.root_bead -and -not $programDescendants.Contains($dependencyId))) {
                    throw "validate-closure-taxonomy: $matrixId/$rowId producer dependency '$dependencyId' is outside the current program"
                }
            }
            foreach ($beadField in @("evidence_owner_bead", "residual_owner_bead")) {
                $beadId = [string]$row.$beadField
                if (-not [string]::IsNullOrWhiteSpace($beadId) -and (-not $issueById.ContainsKey($beadId) -or -not $programDescendants.Contains($beadId))) {
                    throw "validate-closure-taxonomy: $matrixId/$rowId $beadField '$beadId' is outside the current program"
                }
            }

            if ($truthState -in @("implemented-full", "verified") -and
                [string]::IsNullOrWhiteSpace([string]$row.test_anchors) -and
                [string]::IsNullOrWhiteSpace([string]$row.evidence_refs)) {
                throw "validate-closure-taxonomy: $matrixId/$rowId '$truthState' has no test/evidence anchor"
            }

            if ([string]$owner.profile -eq "windows-x64") {
                if ($row.PSObject.Properties.Name -contains "target_arch" -and [string]$row.target_arch -ne "x64") {
                    throw "validate-closure-taxonomy: $matrixId/$rowId target_arch must be x64"
                }
                if ($row.PSObject.Properties.Name -contains "office_bitness" -and [string]$row.office_bitness -notin @("64", "n/a")) {
                    throw "validate-closure-taxonomy: $matrixId/$rowId office_bitness must be 64 or n/a"
                }
                $targetText = "$($row.target_context) $($row.target_arch) $($row.office_bitness)"
                if ($targetText -match '(?i)(\bx86\b|\bi686\b|\bWOW64\b|\bARM64\b|32-bit Office)') {
                    throw "validate-closure-taxonomy: $matrixId/$rowId contains an excluded non-x64 target"
                }
            }
        }
    }

    $rootIssue = $issueById[[string]$manifest.root_bead]
    if ([string]$rootIssue.status -eq "closed") {
        foreach ($owner in $ownershipRows) {
            if (-not (ConvertFrom-IdealBoolean -Value ([string]$owner.required_for_terminal) -Owner "$($owner.matrix_id) required_for_terminal")) {
                continue
            }
            $matrixAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$owner.path)
            $terminalRows = @(Import-Csv -LiteralPath $matrixAbs)
            if ($terminalRows.Count -eq 0) {
                throw "validate-closure-taxonomy: closed program has no rows in required matrix '$($owner.matrix_id)'"
            }
            $requiredRows = @($terminalRows | Where-Object {
                ConvertFrom-IdealBoolean -Value ([string]$_.required) -Owner "$($owner.matrix_id)/$($_.row_id) required"
            })
            if ($requiredRows.Count -eq 0) {
                throw "validate-closure-taxonomy: closed program has no required rows in terminal matrix '$($owner.matrix_id)'"
            }
            $notVerified = @($requiredRows | Where-Object { [string]$_.truth_state -ne "verified" })
            if ($notVerified.Count -gt 0) {
                throw "validate-closure-taxonomy: closed program has non-verified required rows in '$($owner.matrix_id)'"
            }
        }
    }

    Write-Host "validate-closure-taxonomy: ok (program=$($manifest.program_id) matrices=$($ownershipRows.Count) rows=$totalRows)"
}
finally {
    Pop-Location
}
