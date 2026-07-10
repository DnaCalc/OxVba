param(
    [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json"
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

    foreach ($field in @("matrix_ownership", "matrix_schema", "bead_traceability", "legacy_migration")) {
        $path = [string]$manifest.$field
        Assert-IdealRelativePath -Path $path -Owner "manifest.$field"
        $abs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $path
        if (-not (Test-Path -LiteralPath $abs -PathType Leaf)) {
            throw "validate-validation-ownership: missing manifest artifact '$path'"
        }
    }

    $ownershipPath = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.matrix_ownership)
    $ownershipColumns = @(Get-IdealCsvColumns -Path $ownershipPath)
    $requiredOwnershipColumns = @(
        "matrix_id",
        "profile",
        "path",
        "role",
        "owner_epic",
        "row_id_prefix",
        "required_for_terminal",
        "summary_group",
        "predecessor_disposition",
        "notes"
    )
    if (($requiredOwnershipColumns -join ',') -ne ($ownershipColumns -join ',')) {
        throw "validate-validation-ownership: ownership header must exactly match the V1 schema"
    }

    $ownershipRows = @(Import-Csv -LiteralPath $ownershipPath)
    if ($ownershipRows.Count -ne 15) {
        throw "validate-validation-ownership: expected exactly 15 owned matrices, found $($ownershipRows.Count)"
    }

    $expectedEpics = @(Get-IdealExpectedEpicRecords -Manifest $manifest)
    $epicProfileById = @{}
    foreach ($epic in $expectedEpics) {
        $epicProfileById[$epic.EpicId] = $epic.Profile
    }
    $validProfiles = @($manifest.profiles | ForEach-Object { [string]$_.profile })
    $validRoles = @("primary", "projection", "evidence", "quality")
    $seenIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seenPaths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seenPrefixes = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)

    $commonEnvelope = @(
        "row_id",
        "claim_key",
        "truth_role",
        "source_claim_key",
        "profile",
        "required",
        "capability",
        "semantic_subset",
        "target_context",
        "contract_clauses",
        "authority_refs",
        "owner_epic",
        "producer_dependencies",
        "truth_state",
        "test_anchors",
        "evidence_refs",
        "deferred_gate_refs",
        "divergence_refs",
        "evidence_owner_bead",
        "residual_disposition",
        "residual_owner_bead",
        "notes"
    )

    foreach ($row in $ownershipRows) {
        $matrixId = [string]$row.matrix_id
        $matrixPath = ([string]$row.path).Replace('\', '/')
        $profile = [string]$row.profile
        $ownerEpic = [string]$row.owner_epic
        $prefix = [string]$row.row_id_prefix

        if ([string]::IsNullOrWhiteSpace($matrixId) -or -not $seenIds.Add($matrixId)) {
            throw "validate-validation-ownership: blank or duplicate matrix_id '$matrixId'"
        }
        if ([string]::IsNullOrWhiteSpace($matrixPath) -or -not $seenPaths.Add($matrixPath)) {
            throw "validate-validation-ownership: blank or duplicate matrix path '$matrixPath'"
        }
        if ([string]::IsNullOrWhiteSpace($prefix) -or -not $seenPrefixes.Add($prefix)) {
            throw "validate-validation-ownership: blank or duplicate row_id_prefix '$prefix'"
        }
        if ($profile -notin $validProfiles) {
            throw "validate-validation-ownership: matrix '$matrixId' has unknown profile '$profile'"
        }
        if ([string]$row.role -notin $validRoles) {
            throw "validate-validation-ownership: matrix '$matrixId' has unknown role '$($row.role)'"
        }
        if (-not $epicProfileById.ContainsKey($ownerEpic)) {
            throw "validate-validation-ownership: matrix '$matrixId' owner '$ownerEpic' is not a manifest execution epic"
        }
        if ($epicProfileById[$ownerEpic] -ne $profile) {
            throw "validate-validation-ownership: matrix '$matrixId' profile '$profile' disagrees with owner '$ownerEpic' profile '$($epicProfileById[$ownerEpic])'"
        }
        if (-not (ConvertFrom-IdealBoolean -Value ([string]$row.required_for_terminal) -Owner "matrix '$matrixId' required_for_terminal")) {
            throw "validate-validation-ownership: all V1 owned matrices are required for the umbrella terminal gate; '$matrixId' is not"
        }
        if ([string]$row.summary_group -ne $profile) {
            throw "validate-validation-ownership: matrix '$matrixId' summary_group must equal profile '$profile'"
        }
        if ([string]$row.predecessor_disposition -notin @("migrate", "replace")) {
            throw "validate-validation-ownership: matrix '$matrixId' predecessor_disposition must be migrate or replace"
        }

        Assert-IdealRelativePath -Path $matrixPath -Owner "matrix '$matrixId' path"
        $matrixAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $matrixPath
        if (-not (Test-Path -LiteralPath $matrixAbs -PathType Leaf)) {
            throw "validate-validation-ownership: matrix '$matrixId' file is missing: $matrixPath"
        }
        $matrixColumns = @(Get-IdealCsvColumns -Path $matrixAbs)
        foreach ($column in $commonEnvelope) {
            if ($matrixColumns -notcontains $column) {
                throw "validate-validation-ownership: matrix '$matrixId' is missing common column '$column'"
            }
        }
        if ($matrixColumns[0] -ne "row_id" -or $matrixColumns[1] -ne "claim_key") {
            throw "validate-validation-ownership: matrix '$matrixId' must start with row_id,claim_key"
        }
        if ($matrixColumns[-1] -ne "notes") {
            throw "validate-validation-ownership: matrix '$matrixId' must end with notes"
        }
    }

    $traceAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.bead_traceability)
    $traceColumns = @(Get-IdealCsvColumns -Path $traceAbs)
    $expectedTraceColumns = @(
        "bead_id",
        "parent_epic",
        "effect",
        "profile",
        "matrix_id",
        "row_id",
        "relationship",
        "contract_clauses",
        "acceptance_evidence",
        "residual_owner_bead",
        "notes"
    )
    if (($expectedTraceColumns -join ',') -ne ($traceColumns -join ',')) {
        throw "validate-validation-ownership: traceability header must exactly match the V1 schema"
    }

    $schemaAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.matrix_schema)
    $schemaText = Get-Content -LiteralPath $schemaAbs -Raw
    foreach ($needle in @(
        [string]$manifest.program_id,
        [IO.Path]::GetFileName([string]$manifest.matrix_ownership),
        "remaining-accepted-scope",
        "target_arch=x64",
        "Bare ``implemented`` is invalid"
    )) {
        if (-not $schemaText.Contains($needle)) {
            throw "validate-validation-ownership: matrix schema is missing required policy text '$needle'"
        }
    }

    Write-Host "validate-validation-ownership: ok (program=$($manifest.program_id) matrices=$($ownershipRows.Count) profiles=$($validProfiles.Count))"
}
finally {
    Pop-Location
}
