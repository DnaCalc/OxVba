param(
    [string]$ManifestPath = "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/certification-cases.json",
    [string]$FixtureManifestPath = "docs/validation/IDEAL_WINDOWS_X64_FIXTURE_MANIFEST_V1.csv",
    [string]$ResidualLedgerPath = "docs/validation/IDEAL_WINDOWS_CURRENT_STACK_RESIDUAL_V1.csv",
    [string]$EnvironmentManifestPath = "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv",
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

function Assert-ExactStringSet {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Actual,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Expected,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $difference = @(Compare-Object -ReferenceObject @($Expected | Sort-Object -Unique) -DifferenceObject @($Actual | Sort-Object -Unique))
    if ($difference.Count -gt 0 -or @($Actual).Count -ne @($Expected).Count) {
        $missing = @($difference | Where-Object SideIndicator -eq '<=' | ForEach-Object InputObject)
        $unexpected = @($difference | Where-Object SideIndicator -eq '=>' | ForEach-Object InputObject)
        throw "validate-win14-certification-manifest: $Owner differs (missing=$($missing -join '|'); unexpected=$($unexpected -join '|'))"
    }
}

function Assert-RequiredProperties {
    param(
        [Parameter(Mandatory = $true)]$Object,
        [Parameter(Mandatory = $true)][string[]]$Properties,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $actual = @($Object.PSObject.Properties.Name)
    foreach ($property in $Properties) {
        if ($property -notin $actual) {
            throw "validate-win14-certification-manifest: $Owner is missing '$property'"
        }
    }
}

function Get-ExpectedRoles {
    param([Parameter(Mandatory = $true)][string]$MatrixId, [Parameter(Mandatory = $true)]$Row)

    switch ([string]$Row.row_id) {
        "WAC-CLEAN-CERT-ENV" { return @("aggregate-environment", "aggregate-environment") }
        "WAC-EXCEL-COM-CERT" { return @("aggregate-com", "controlled-com-fixture") }
        "WAC-EXCEL-NATIVE-CERT" { return @("aggregate-native", "aggregate-native") }
        "WAC-RELEASE-CERT" { return @("aggregate-release", "aggregate-release") }
        "WAC-PROFILE-TERMINAL" { return @("aggregate-release", "aggregate-release") }
        "WAC-TARGET-DEV-ENV" { return @("development-only", "not-applicable") }
    }
    switch ($MatrixId) {
        "WIN-COM-CLIENT" { return @("required", "controlled-com-fixture") }
        "WIN-COM-EVENTS" { return @("required", "controlled-com-fixture") }
        "WIN-COM-SERVER" { return @("required", "controlled-com-fixture") }
        "WIN-NATIVE-IMPORT" { return @("required", "controlled-native-fixture") }
        "WIN-NATIVE-EXPORT" {
            $excel = if ([string]$Row.office_bitness -eq "64") { "required" } else { "not-applicable" }
            return @($excel, "external-native-client")
        }
        "WIN-ABI-CARRIER" {
            $excel = if ([string]$Row.office_bitness -eq "64") { "required" } else { "not-applicable" }
            return @($excel, "controlled-carrier-probe")
        }
        default { throw "validate-win14-certification-manifest: unknown matrix '$MatrixId'" }
    }
}

function Get-ExpectedEvidencePath {
    param([Parameter(Mandatory = $true)][string]$CaseId, [Parameter(Mandatory = $true)][string]$RowId)

    switch ($RowId) {
        "WAC-CLEAN-CERT-ENV" { return "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/certification-vm.md" }
        "WAC-EXCEL-COM-CERT" { return "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/com-certification.md" }
        "WAC-EXCEL-NATIVE-CERT" { return "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/native-certification.md" }
        "WAC-RELEASE-CERT" { return "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/final-certification.md" }
        default { return "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/cases/$($CaseId.ToLowerInvariant()).json" }
    }
}

function Get-SourceHash {
    param([Parameter(Mandatory = $true)][string]$Path)

    return "sha256:$((Get-FileHash -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $Path) -Algorithm SHA256).Hash.ToLowerInvariant())"
}

$matrixPaths = [ordered]@{
    "WIN-COM-CLIENT" = "docs/validation/WINDOWS_JIT_COM_CLIENT_MATRIX_V1.csv"
    "WIN-COM-EVENTS" = "docs/validation/WINDOWS_JIT_COM_EVENTS_MATRIX_V1.csv"
    "WIN-COM-SERVER" = "docs/validation/WINDOWS_JIT_COM_SERVER_MATRIX_V1.csv"
    "WIN-NATIVE-IMPORT" = "docs/validation/WINDOWS_JIT_NATIVE_IMPORT_MATRIX_V1.csv"
    "WIN-NATIVE-EXPORT" = "docs/validation/WINDOWS_NATIVE_EXPORT_AND_PACKAGING_MATRIX_V1.csv"
    "WIN-ABI-CARRIER" = "docs/validation/WINDOWS_ABI_CARRIER_MATRIX_V1.csv"
}
$expectedSourcePaths = @($matrixPaths.Values) + @($FixtureManifestPath, $ResidualLedgerPath, $EnvironmentManifestPath)
$defaultExpectedBlockers = @(
    "producer-matrix-row-planned",
    "controlled-fixture-built-artifact-pending",
    "certification-environment-planned-blocking",
    "certification-runner-pending"
)
$expectedAxes = [ordered]@{
    result = "result_expectation"
    full_err = "err_expectation"
    side_effects = "side_effect_expectation"
    lifecycle_order = "lifecycle_order_expectation"
    transport = "transport_expectation"
    balance = "balance_expectation"
}
$aggregateRoles = [ordered]@{
    "WAC-CLEAN-CERT-ENV" = "environment"
    "WAC-EXCEL-COM-CERT" = "com"
    "WAC-EXCEL-NATIVE-CERT" = "native"
    "WAC-RELEASE-CERT" = "release"
}
$activeStatuses = @("open", "in_progress", "blocked")

Push-Location $repoRoot
try {
    $manifestAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $ManifestPath
    if (-not (Test-Path -LiteralPath $manifestAbs -PathType Leaf)) {
        throw "validate-win14-certification-manifest: missing manifest '$ManifestPath'"
    }
    $manifest = Get-Content -LiteralPath $manifestAbs -Raw | ConvertFrom-Json
    Assert-RequiredProperties -Object $manifest -Properties @(
        "schema", "schema_version", "program_id", "profile", "target_arch", "as_of_date",
        "source_hashes", "certification_environment", "execution_policy", "aggregate_anchors", "cases"
    ) -Owner "manifest"
    if ([string]$manifest.schema -ne "oxvba-win14-certification-cases-v1" -or
        [int]$manifest.schema_version -ne 1 -or [string]$manifest.program_id -ne "ideal-2026-07" -or
        [string]$manifest.profile -ne "windows-x64" -or [string]$manifest.target_arch -ne "x64" -or
        [string]$manifest.as_of_date -ne "2026-07-14") {
        throw "validate-win14-certification-manifest: manifest identity drifted"
    }
    if ([string]$manifest.execution_policy.case_mapping -ne "exactly-one-case-per-canonical-row" -or
        [string]$manifest.execution_policy.executable_state -ne "requires verified producer row, current built fixture, pinned verified environment, and present runner" -or
        [string]$manifest.execution_policy.blocked_state -ne "must enumerate every unmet producer, fixture, environment, and runner gate" -or
        [string]$manifest.execution_policy.capability_credit -ne "none" -or
        [string]$manifest.execution_policy.certification_credit -ne "none" -or
        [string]$manifest.execution_policy.historical_evidence_credit -ne "forbidden" -or
        [string]$manifest.execution_policy.development_environment_credit -ne "forbidden") {
        throw "validate-win14-certification-manifest: execution policy grants premature credit"
    }

    $sourceHashes = @($manifest.source_hashes)
    Assert-ExactStringSet -Actual @($sourceHashes.path) -Expected $expectedSourcePaths -Owner "source hash paths"
    foreach ($source in $sourceHashes) {
        if ([string]$source.sha256 -ne (Get-SourceHash -Path ([string]$source.path))) {
            throw "validate-win14-certification-manifest: source hash drift for '$($source.path)'"
        }
    }

    $environmentRows = @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $EnvironmentManifestPath))
    $certEnvironments = @($environmentRows | Where-Object environment_id -eq "win-x64-cert-vm-pending-v1")
    if ($certEnvironments.Count -ne 1) {
        throw "validate-win14-certification-manifest: provisional certification environment is not unique"
    }
    $certEnvironment = $certEnvironments[0]
    if ([string]$manifest.certification_environment.environment_id -ne [string]$certEnvironment.environment_id -or
        [string]$manifest.certification_environment.role -ne [string]$certEnvironment.role -or
        [string]$manifest.certification_environment.current_evidence_state -ne "planned-blocking" -or
        [string]$manifest.certification_environment.required_evidence_state -ne "verified" -or
        [string]$manifest.certification_environment.locale -ne "non-default-locale-required" -or
        [string]$manifest.certification_environment.owner_bead -ne [string]$certEnvironment.owner_bead -or
        [string]$manifest.certification_environment.state -ne "pending") {
        throw "validate-win14-certification-manifest: certification environment gate drifted or was promoted"
    }

    $canonicalByKey = @{}
    foreach ($matrix in $matrixPaths.GetEnumerator()) {
        foreach ($row in @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $matrix.Value))) {
            $key = "$($matrix.Key)|$([string]$row.row_id)"
            if ($canonicalByKey.ContainsKey($key)) {
                throw "validate-win14-certification-manifest: duplicate canonical row '$key'"
            }
            $canonicalByKey[$key] = $row
        }
    }
    if ($canonicalByKey.Count -ne 57) {
        throw "validate-win14-certification-manifest: canonical Windows inventory must contain 57 rows"
    }

    $fixtureByKey = @{}
    foreach ($fixture in @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $FixtureManifestPath))) {
        $key = "$([string]$fixture.matrix_id)|$([string]$fixture.row_id)"
        if ($fixtureByKey.ContainsKey($key)) { throw "validate-win14-certification-manifest: duplicate fixture '$key'" }
        $fixtureByKey[$key] = $fixture
    }
    $residualByKey = @{}
    foreach ($residual in @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $ResidualLedgerPath))) {
        $key = "$([string]$residual.matrix_id)|$([string]$residual.row_id)"
        if ($residualByKey.ContainsKey($key)) { throw "validate-win14-certification-manifest: duplicate residual '$key'" }
        $residualByKey[$key] = $residual
    }
    Assert-ExactStringSet -Actual @($fixtureByKey.Keys) -Expected @($canonicalByKey.Keys) -Owner "fixture identity set"
    Assert-ExactStringSet -Actual @($residualByKey.Keys) -Expected @($canonicalByKey.Keys) -Owner "residual identity set"

    $issues = Read-IdealIssues -RepoRoot $repoRoot -IssuesPath $IssuesPath
    $issueById = $issues.IssueById
    $cases = @($manifest.cases)
    if ($cases.Count -ne 57) {
        throw "validate-win14-certification-manifest: expected 57 certification cases; found $($cases.Count)"
    }
    $caseByKey = @{}
    $caseIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($case in $cases) {
        Assert-RequiredProperties -Object $case -Properties @(
            "case_id", "matrix_id", "row_id", "claim_key", "mapping", "producer_gate", "roles", "fixture",
            "observable_axes", "locale", "execution", "commands", "artifacts", "environment_gate",
            "certification_state", "blocking_reasons", "capability_credit", "certification_credit"
        ) -Owner "certification case"
        $key = "$([string]$case.matrix_id)|$([string]$case.row_id)"
        if ($caseByKey.ContainsKey($key) -or -not $caseIds.Add([string]$case.case_id)) {
            throw "validate-win14-certification-manifest: duplicate case mapping or case id for '$key'"
        }
        if (-not $canonicalByKey.ContainsKey($key)) {
            throw "validate-win14-certification-manifest: case '$key' does not map a canonical row"
        }
        $caseByKey[$key] = $case
        $canonical = $canonicalByKey[$key]
        $fixture = $fixtureByKey[$key]
        $residual = $residualByKey[$key]
        $producerSatisfied = $key -ceq "WIN-ABI-CARRIER|WAC-TARGET-DEV-ENV"
        $expectedCanonicalTruth = if ($producerSatisfied) { "verified" } else { "planned" }
        $expectedProducerState = if ($producerSatisfied) { "satisfied" } else { "pending" }
        $expectedCaseId = "WIN14-$([string]$canonical.row_id)"
        if ([string]$case.case_id -ne $expectedCaseId -or [string]$case.claim_key -ne [string]$canonical.claim_key -or
            [string]$case.mapping -ne "exactly-one-canonical-row") {
            throw "validate-win14-certification-manifest: '$key' case identity drifted"
        }
        if ([string]$canonical.truth_state -ne $expectedCanonicalTruth -or
            [string]$case.producer_gate.current_truth_state -ne $expectedCanonicalTruth -or
            [string]$case.producer_gate.required_truth_state -ne "verified" -or
            [string]$case.producer_gate.state -ne $expectedProducerState) {
            throw "validate-win14-certification-manifest: '$key' producer gate disagrees with canonical truth"
        }
        $ownerId = [string]$case.producer_gate.owner_bead
        if ($producerSatisfied) {
            if ($ownerId -ne "n/a" -or [string]$residual.live_residual_owner_bead -ne "n/a") {
                throw "validate-win14-certification-manifest: '$key' retains a live owner after producer satisfaction"
            }
        }
        elseif ($ownerId -ne [string]$residual.live_residual_owner_bead -or -not $issueById.ContainsKey($ownerId) -or
                [string]$issueById[$ownerId].status -notin $activeStatuses -or
                "profile-win-x64" -notin @(Get-IdealIssueLabels -Issue $issueById[$ownerId])) {
            throw "validate-win14-certification-manifest: '$key' has an unowned or inactive producer gate '$ownerId'"
        }
        $expectedDependencies = @(([string]$canonical.producer_dependencies -split ';') | ForEach-Object { $_.Trim() } | Where-Object { $_ })
        Assert-ExactStringSet -Actual @($case.producer_gate.dependency_beads) -Expected $expectedDependencies -Owner "$key producer dependencies"

        $roles = @(Get-ExpectedRoles -MatrixId ([string]$case.matrix_id) -Row $canonical)
        if ([string]$case.roles.excel -ne $roles[0] -or [string]$case.roles.native -ne $roles[1]) {
            throw "validate-win14-certification-manifest: '$key' Excel/native role drifted"
        }
        foreach ($field in @(
            "fixture_id", "recipe_id", "source_recipe_state", "source_recipe_owner_bead",
            "built_artifact_id", "built_artifact_class", "built_artifact_state", "built_artifact_owner_bead"
        )) {
            if ([string]$case.fixture.$field -ne [string]$fixture.$field) {
                throw "validate-win14-certification-manifest: '$key' fixture field '$field' drifted"
            }
        }
        if ([string]$case.fixture.source_environment_id -ne [string]$fixture.environment_id -or
            [string]$case.fixture.source_environment_state -ne [string]$fixture.environment_state) {
            throw "validate-win14-certification-manifest: '$key' source environment provenance drifted"
        }
        $expectedSourcePathsForCase = @(([string]$fixture.source_recipe_paths -split '\|') | ForEach-Object { $_.Trim() } | Where-Object { $_ })
        Assert-ExactStringSet -Actual @($case.fixture.source_recipe_paths) -Expected $expectedSourcePathsForCase -Owner "$key source recipe paths"
        $expectedBuiltState = if ($producerSatisfied) { "not-applicable" } else { "pending" }
        if ([string]$case.fixture.built_artifact_state -ne $expectedBuiltState) {
            throw "validate-win14-certification-manifest: '$key' grants built fixture credit before promotion"
        }

        Assert-ExactStringSet -Actual @($case.observable_axes.PSObject.Properties.Name) -Expected @($expectedAxes.Keys) -Owner "$key observable axes"
        foreach ($axis in $expectedAxes.GetEnumerator()) {
            $actualAxis = $case.observable_axes.($axis.Key)
            $expectationField = [string]$axis.Value
            if ([string]::IsNullOrWhiteSpace([string]$actualAxis.expectation) -or
                [string]$actualAxis.expectation -ne [string]$fixture.$expectationField -or [string]$actualAxis.state -ne "pending") {
                throw "validate-win14-certification-manifest: '$key' observable '$($axis.Key)' is incomplete or promoted"
            }
        }
        if ([string]$case.locale.requirement -ne "non-default-locale-required" -or
            [string]$case.locale.environment_value -ne "non-default-locale-required" -or [string]$case.locale.state -ne "pending") {
            throw "validate-win14-certification-manifest: '$key' locale gate drifted"
        }
        foreach ($field in @("target_arch", "office_bitness", "process_shape", "apartment_shape")) {
            if ([string]$case.execution.$field -ne [string]$canonical.$field) {
                throw "validate-win14-certification-manifest: '$key' execution field '$field' drifted"
            }
        }

        $commands = @($case.commands)
        $requiresOracle = [string]$case.roles.excel -notin @("not-applicable", "development-only", "aggregate-environment")
        $expectedCommandRoles = if ($requiresOracle) { @("profile-gate", "excel-vba-oracle") } else { @("profile-gate") }
        if ([string]$case.row_id -eq "WAC-CLEAN-CERT-ENV") { $expectedCommandRoles = @("environment-capture") }
        Assert-ExactStringSet -Actual @($commands.role) -Expected $expectedCommandRoles -Owner "$key command roles"
        foreach ($command in $commands) {
            if ([string]$command.state -ne "blocked-runner-pending" -or
                [string]::IsNullOrWhiteSpace([string]$command.command) -or
                [string]$command.command -notmatch [regex]::Escape($expectedCaseId) -or
                [string]$command.command -notmatch '^pwsh -NoProfile -File scripts/') {
                throw "validate-win14-certification-manifest: '$key' has an invalid or prematurely executable command"
            }
        }

        $artifacts = @($case.artifacts)
        Assert-ExactStringSet -Actual @($artifacts.kind) -Expected @("case-evidence", "controlled-fixture", "environment-capture") -Owner "$key artifact kinds"
        $artifactByKind = @{}
        foreach ($artifact in $artifacts) {
            $artifactByKind[[string]$artifact.kind] = $artifact
            Assert-IdealRelativePath -Path ([string]$artifact.path) -Owner "$key artifact path"
        }
        $fixtureArtifactPath = "$([string]$fixture.built_artifact_root)/$([string]$fixture.built_artifact_name)"
        $environmentCapturePath = "artifacts/windows-x64/controlled-environments/v1/win-x64-cert-vm-pending-v1/environment-capture.json"
        if ([string]$artifactByKind["case-evidence"].path -ne (Get-ExpectedEvidencePath -CaseId $expectedCaseId -RowId ([string]$case.row_id)) -or
            [string]$artifactByKind["case-evidence"].state -ne "pending" -or
            [string]$artifactByKind["case-evidence"].owner_bead -ne $ownerId -or
            [string]$artifactByKind["controlled-fixture"].path -ne $fixtureArtifactPath -or
            [string]$artifactByKind["controlled-fixture"].state -ne $expectedBuiltState -or
            [string]$artifactByKind["controlled-fixture"].owner_bead -ne [string]$fixture.built_artifact_owner_bead -or
            [string]$artifactByKind["environment-capture"].path -ne $environmentCapturePath -or
            [string]$artifactByKind["environment-capture"].state -ne "pending" -or
            [string]$artifactByKind["environment-capture"].owner_bead -ne "bd-59co.3.15.3") {
            throw "validate-win14-certification-manifest: '$key' artifact contract drifted or was promoted"
        }

        if ([string]$case.environment_gate.environment_id -ne "win-x64-cert-vm-pending-v1" -or
            [string]$case.environment_gate.required_evidence_state -ne "verified" -or
            [string]$case.environment_gate.current_evidence_state -ne "planned-blocking" -or
            [string]$case.environment_gate.owner_bead -ne "bd-59co.3.15.3" -or
            [string]$case.environment_gate.state -ne "pending") {
            throw "validate-win14-certification-manifest: '$key' certification environment was bypassed"
        }
        $expectedBlockers = if ($producerSatisfied) {
            @("certification-environment-planned-blocking", "certification-runner-pending")
        }
        else {
            $defaultExpectedBlockers
        }
        Assert-ExactStringSet -Actual @($case.blocking_reasons) -Expected $expectedBlockers -Owner "$key blocking reasons"
        if ([string]$case.certification_state -ne "blocked" -or [string]$case.capability_credit -ne "none" -or
            [string]$case.certification_credit -ne "none") {
            throw "validate-win14-certification-manifest: '$key' grants premature capability or certification credit"
        }
    }
    Assert-ExactStringSet -Actual @($caseByKey.Keys) -Expected @($canonicalByKey.Keys) -Owner "case-to-row coverage"

    $anchors = @($manifest.aggregate_anchors)
    if ($anchors.Count -ne 4) {
        throw "validate-win14-certification-manifest: exactly four aggregate anchors are required"
    }
    Assert-ExactStringSet -Actual @($anchors.row_id) -Expected @($aggregateRoles.Keys) -Owner "aggregate anchor rows"
    foreach ($anchor in $anchors) {
        $rowId = [string]$anchor.row_id
        $case = @($cases | Where-Object row_id -eq $rowId)
        if ($case.Count -ne 1 -or [string]$anchor.matrix_id -ne "WIN-ABI-CARRIER" -or
            [string]$anchor.role -ne [string]$aggregateRoles[$rowId] -or
            [string]$anchor.case_id -ne [string]$case[0].case_id -or
            [string]$anchor.owner_bead -ne [string]$case[0].producer_gate.owner_bead -or
            [string]$anchor.state -ne "pending") {
            throw "validate-win14-certification-manifest: aggregate anchor '$rowId' drifted or was promoted"
        }
    }

    Write-Host "validate-win14-certification-manifest: ok (cases=57 blocked=57 axes=342 aggregate_anchors=4 capability_credit=none)"
}
finally {
    Pop-Location
}
