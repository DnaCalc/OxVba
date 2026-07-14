param(
    [string]$OutputPath = "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/certification-cases.json",
    [string]$FixtureManifestPath = "docs/validation/IDEAL_WINDOWS_X64_FIXTURE_MANIFEST_V1.csv",
    [string]$ResidualLedgerPath = "docs/validation/IDEAL_WINDOWS_CURRENT_STACK_RESIDUAL_V1.csv",
    [string]$EnvironmentManifestPath = "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv",
    [string]$RepositoryRoot = "",
    [switch]$Check
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

function Resolve-RepoPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    if ([IO.Path]::IsPathRooted($Path)) {
        return [IO.Path]::GetFullPath($Path)
    }
    return [IO.Path]::GetFullPath((Join-Path $repoRoot $Path))
}

function Get-SourceHash {
    param([Parameter(Mandatory = $true)][string]$Path)

    return "sha256:$((Get-FileHash -LiteralPath (Resolve-RepoPath $Path) -Algorithm SHA256).Hash.ToLowerInvariant())"
}

function Split-List {
    param([AllowEmptyString()][string]$Text, [char]$Delimiter)

    if ([string]::IsNullOrWhiteSpace($Text)) {
        return @()
    }
    return @($Text -split [regex]::Escape([string]$Delimiter) | ForEach-Object { $_.Trim() } | Where-Object { $_ })
}

function Get-CertificationRoles {
    param([Parameter(Mandatory = $true)][string]$MatrixId, [Parameter(Mandatory = $true)]$Row)

    switch ([string]$Row.row_id) {
        "WAC-CLEAN-CERT-ENV" { return [ordered]@{ excel = "aggregate-environment"; native = "aggregate-environment" } }
        "WAC-EXCEL-COM-CERT" { return [ordered]@{ excel = "aggregate-com"; native = "controlled-com-fixture" } }
        "WAC-EXCEL-NATIVE-CERT" { return [ordered]@{ excel = "aggregate-native"; native = "aggregate-native" } }
        "WAC-RELEASE-CERT" { return [ordered]@{ excel = "aggregate-release"; native = "aggregate-release" } }
        "WAC-PROFILE-TERMINAL" { return [ordered]@{ excel = "aggregate-release"; native = "aggregate-release" } }
        "WAC-TARGET-DEV-ENV" { return [ordered]@{ excel = "development-only"; native = "not-applicable" } }
    }

    switch ($MatrixId) {
        "WIN-COM-CLIENT" { return [ordered]@{ excel = "required"; native = "controlled-com-fixture" } }
        "WIN-COM-EVENTS" { return [ordered]@{ excel = "required"; native = "controlled-com-fixture" } }
        "WIN-COM-SERVER" { return [ordered]@{ excel = "required"; native = "controlled-com-fixture" } }
        "WIN-NATIVE-IMPORT" { return [ordered]@{ excel = "required"; native = "controlled-native-fixture" } }
        "WIN-NATIVE-EXPORT" {
            $excelRole = if ([string]$Row.office_bitness -eq "64") { "required" } else { "not-applicable" }
            return [ordered]@{ excel = $excelRole; native = "external-native-client" }
        }
        "WIN-ABI-CARRIER" {
            $excelRole = if ([string]$Row.office_bitness -eq "64") { "required" } else { "not-applicable" }
            return [ordered]@{ excel = $excelRole; native = "controlled-carrier-probe" }
        }
        default { throw "sync-win14-certification-manifest: unknown matrix '$MatrixId'" }
    }
}

function Get-EvidencePath {
    param([Parameter(Mandatory = $true)][string]$CaseId, [Parameter(Mandatory = $true)][string]$RowId)

    switch ($RowId) {
        "WAC-CLEAN-CERT-ENV" { return "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/certification-vm.md" }
        "WAC-EXCEL-COM-CERT" { return "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/com-certification.md" }
        "WAC-EXCEL-NATIVE-CERT" { return "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/native-certification.md" }
        "WAC-RELEASE-CERT" { return "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/final-certification.md" }
        default { return "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/cases/$($CaseId.ToLowerInvariant()).json" }
    }
}

$matrixPaths = [ordered]@{
    "WIN-COM-CLIENT" = "docs/validation/WINDOWS_JIT_COM_CLIENT_MATRIX_V1.csv"
    "WIN-COM-EVENTS" = "docs/validation/WINDOWS_JIT_COM_EVENTS_MATRIX_V1.csv"
    "WIN-COM-SERVER" = "docs/validation/WINDOWS_JIT_COM_SERVER_MATRIX_V1.csv"
    "WIN-NATIVE-IMPORT" = "docs/validation/WINDOWS_JIT_NATIVE_IMPORT_MATRIX_V1.csv"
    "WIN-NATIVE-EXPORT" = "docs/validation/WINDOWS_NATIVE_EXPORT_AND_PACKAGING_MATRIX_V1.csv"
    "WIN-ABI-CARRIER" = "docs/validation/WINDOWS_ABI_CARRIER_MATRIX_V1.csv"
}

$sourcePaths = @($matrixPaths.Values) + @($FixtureManifestPath, $ResidualLedgerPath, $EnvironmentManifestPath)
$sourceHashes = @(
    foreach ($path in $sourcePaths) {
        [ordered]@{ path = $path.Replace('\', '/'); sha256 = Get-SourceHash -Path $path }
    }
)

$fixtureRows = @(Import-Csv -LiteralPath (Resolve-RepoPath $FixtureManifestPath))
$fixtureByKey = @{}
foreach ($fixture in $fixtureRows) {
    $key = "$([string]$fixture.matrix_id)|$([string]$fixture.row_id)"
    if ($fixtureByKey.ContainsKey($key)) {
        throw "sync-win14-certification-manifest: duplicate fixture row '$key'"
    }
    $fixtureByKey[$key] = $fixture
}

$residualRows = @(Import-Csv -LiteralPath (Resolve-RepoPath $ResidualLedgerPath))
$residualByKey = @{}
foreach ($residual in $residualRows) {
    $key = "$([string]$residual.matrix_id)|$([string]$residual.row_id)"
    if ($residualByKey.ContainsKey($key)) {
        throw "sync-win14-certification-manifest: duplicate residual row '$key'"
    }
    $residualByKey[$key] = $residual
}

$environmentRows = @(Import-Csv -LiteralPath (Resolve-RepoPath $EnvironmentManifestPath))
$certificationEnvironments = @($environmentRows | Where-Object { [string]$_.environment_id -eq "win-x64-cert-vm-pending-v1" })
if ($certificationEnvironments.Count -ne 1) {
    throw "sync-win14-certification-manifest: expected one provisional certification environment"
}
$certificationEnvironment = $certificationEnvironments[0]

$cases = @()
foreach ($matrixEntry in $matrixPaths.GetEnumerator()) {
    $matrixId = [string]$matrixEntry.Key
    foreach ($row in @(Import-Csv -LiteralPath (Resolve-RepoPath $matrixEntry.Value))) {
        $key = "$matrixId|$([string]$row.row_id)"
        if (-not $fixtureByKey.ContainsKey($key) -or -not $residualByKey.ContainsKey($key)) {
            throw "sync-win14-certification-manifest: '$key' lacks fixture or residual truth"
        }
        $fixture = $fixtureByKey[$key]
        $residual = $residualByKey[$key]
        $caseId = "WIN14-$([string]$row.row_id)"
        $producerSatisfied = [string]$row.truth_state -eq "verified"
        $producerOwner = if ($producerSatisfied) { "n/a" } else { [string]$residual.live_residual_owner_bead }
        $producerGateState = if ($producerSatisfied) { "satisfied" } else { "pending" }
        $roles = Get-CertificationRoles -MatrixId $matrixId -Row $row
        $commands = @(
            if ([string]$row.row_id -eq "WAC-CLEAN-CERT-ENV") {
                [ordered]@{
                    command = "pwsh -NoProfile -File scripts/capture-ideal-environment.ps1 -CaseId $caseId"
                    role = "environment-capture"
                    state = "blocked-runner-pending"
                }
            }
            else {
                [ordered]@{
                    command = "pwsh -NoProfile -File scripts/run-windows-profile-gates.ps1 -CaseId $caseId"
                    role = "profile-gate"
                    state = "blocked-runner-pending"
                }
            }
            if ([string]$roles.excel -notin @("not-applicable", "development-only", "aggregate-environment")) {
                [ordered]@{
                    command = "pwsh -NoProfile -File scripts/run-excel-vba-oracle.ps1 -CaseId $caseId"
                    role = "excel-vba-oracle"
                    state = "blocked-runner-pending"
                }
            }
        )
        $fixtureArtifactPath = "$([string]$fixture.built_artifact_root)/$([string]$fixture.built_artifact_name)"
        $environmentCapturePath = "artifacts/windows-x64/controlled-environments/v1/$([string]$certificationEnvironment.environment_id)/environment-capture.json"
        $case = [ordered]@{
            case_id = $caseId
            matrix_id = $matrixId
            row_id = [string]$row.row_id
            claim_key = [string]$row.claim_key
            mapping = "exactly-one-canonical-row"
            producer_gate = [ordered]@{
                owner_bead = $producerOwner
                dependency_beads = @(Split-List -Text ([string]$row.producer_dependencies) -Delimiter ';')
                required_truth_state = "verified"
                current_truth_state = [string]$row.truth_state
                state = $producerGateState
            }
            roles = $roles
            fixture = [ordered]@{
                fixture_id = [string]$fixture.fixture_id
                recipe_id = [string]$fixture.recipe_id
                source_recipe_state = [string]$fixture.source_recipe_state
                source_recipe_paths = @(Split-List -Text ([string]$fixture.source_recipe_paths) -Delimiter '|')
                source_recipe_owner_bead = [string]$fixture.source_recipe_owner_bead
                source_environment_id = [string]$fixture.environment_id
                source_environment_state = [string]$fixture.environment_state
                built_artifact_id = [string]$fixture.built_artifact_id
                built_artifact_class = [string]$fixture.built_artifact_class
                built_artifact_state = [string]$fixture.built_artifact_state
                built_artifact_owner_bead = [string]$fixture.built_artifact_owner_bead
            }
            observable_axes = [ordered]@{
                result = [ordered]@{ expectation = [string]$fixture.result_expectation; state = "pending" }
                full_err = [ordered]@{ expectation = [string]$fixture.err_expectation; state = "pending" }
                side_effects = [ordered]@{ expectation = [string]$fixture.side_effect_expectation; state = "pending" }
                lifecycle_order = [ordered]@{ expectation = [string]$fixture.lifecycle_order_expectation; state = "pending" }
                transport = [ordered]@{ expectation = [string]$fixture.transport_expectation; state = "pending" }
                balance = [ordered]@{ expectation = [string]$fixture.balance_expectation; state = "pending" }
            }
            locale = [ordered]@{
                requirement = "non-default-locale-required"
                environment_value = [string]$certificationEnvironment.locale
                state = "pending"
            }
            execution = [ordered]@{
                target_arch = [string]$row.target_arch
                office_bitness = [string]$row.office_bitness
                process_shape = [string]$row.process_shape
                apartment_shape = [string]$row.apartment_shape
            }
            commands = $commands
            artifacts = @(
                [ordered]@{
                    kind = "case-evidence"
                    path = Get-EvidencePath -CaseId $caseId -RowId ([string]$row.row_id)
                    state = "pending"
                    owner_bead = $producerOwner
                },
                [ordered]@{
                    kind = "controlled-fixture"
                    path = $fixtureArtifactPath
                    state = [string]$fixture.built_artifact_state
                    owner_bead = [string]$fixture.built_artifact_owner_bead
                },
                [ordered]@{
                    kind = "environment-capture"
                    path = $environmentCapturePath
                    state = "pending"
                    owner_bead = [string]$certificationEnvironment.owner_bead
                }
            )
            environment_gate = [ordered]@{
                environment_id = [string]$certificationEnvironment.environment_id
                required_evidence_state = "verified"
                current_evidence_state = [string]$certificationEnvironment.evidence_state
                owner_bead = [string]$certificationEnvironment.owner_bead
                state = "pending"
            }
            certification_state = "blocked"
            blocking_reasons = @(
                if (-not $producerSatisfied) { "producer-matrix-row-planned" }
                if ([string]$fixture.built_artifact_state -eq "pending") { "controlled-fixture-built-artifact-pending" }
                "certification-environment-planned-blocking",
                "certification-runner-pending"
            )
            capability_credit = "none"
            certification_credit = "none"
        }
        $cases += [pscustomobject]$case
    }
}

if ($cases.Count -ne 57) {
    throw "sync-win14-certification-manifest: expected 57 cases; found $($cases.Count)"
}

$aggregateRows = [ordered]@{
    "WAC-CLEAN-CERT-ENV" = "environment"
    "WAC-EXCEL-COM-CERT" = "com"
    "WAC-EXCEL-NATIVE-CERT" = "native"
    "WAC-RELEASE-CERT" = "release"
}
$aggregateAnchors = @(
    foreach ($entry in $aggregateRows.GetEnumerator()) {
        $case = @($cases | Where-Object { [string]$_.row_id -eq [string]$entry.Key })
        if ($case.Count -ne 1) {
            throw "sync-win14-certification-manifest: aggregate '$($entry.Key)' lacks one case"
        }
        [ordered]@{
            role = [string]$entry.Value
            matrix_id = [string]$case[0].matrix_id
            row_id = [string]$case[0].row_id
            case_id = [string]$case[0].case_id
            owner_bead = [string]$case[0].producer_gate.owner_bead
            state = "pending"
        }
    }
)

$manifest = [ordered]@{
    schema = "oxvba-win14-certification-cases-v1"
    schema_version = 1
    program_id = "ideal-2026-07"
    profile = "windows-x64"
    target_arch = "x64"
    as_of_date = "2026-07-14"
    source_hashes = $sourceHashes
    certification_environment = [ordered]@{
        environment_id = [string]$certificationEnvironment.environment_id
        role = [string]$certificationEnvironment.role
        required_evidence_state = "verified"
        current_evidence_state = [string]$certificationEnvironment.evidence_state
        locale = [string]$certificationEnvironment.locale
        owner_bead = [string]$certificationEnvironment.owner_bead
        state = "pending"
    }
    execution_policy = [ordered]@{
        case_mapping = "exactly-one-case-per-canonical-row"
        executable_state = "requires verified producer row, current built fixture, pinned verified environment, and present runner"
        blocked_state = "must enumerate every unmet producer, fixture, environment, and runner gate"
        historical_evidence_credit = "forbidden"
        development_environment_credit = "forbidden"
        capability_credit = "none"
        certification_credit = "none"
    }
    aggregate_anchors = $aggregateAnchors
    cases = $cases
}

$json = ($manifest | ConvertTo-Json -Depth 20)
$json = ($json -replace "`r`n", "`n") + "`n"
$outputAbs = Resolve-RepoPath $OutputPath

if ($Check) {
    if (-not (Test-Path -LiteralPath $outputAbs -PathType Leaf)) {
        throw "sync-win14-certification-manifest: missing generated artifact '$OutputPath'"
    }
    $actual = (Get-Content -LiteralPath $outputAbs -Raw) -replace "`r`n", "`n"
    if ($actual -ne $json) {
        throw "sync-win14-certification-manifest: '$OutputPath' is stale; regenerate without -Check"
    }
    Write-Host "sync-win14-certification-manifest: ok (mode=check cases=57 blocked=57 capability_credit=none)"
    exit 0
}

$parent = Split-Path -Parent $outputAbs
if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
}
[IO.File]::WriteAllText($outputAbs, $json, [Text.UTF8Encoding]::new($false))
Write-Host "sync-win14-certification-manifest: ok (mode=write cases=57 blocked=57 capability_credit=none)"
