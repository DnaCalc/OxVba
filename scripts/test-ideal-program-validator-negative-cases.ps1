param(
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

function Copy-FixtureFile {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath
    )

    $source = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $RelativePath
    $target = Resolve-IdealRepoPath -RepoRoot $FixtureRoot -Path $RelativePath
    $targetParent = Split-Path -Parent $target
    if (-not (Test-Path -LiteralPath $targetParent -PathType Container)) {
        [void](New-Item -ItemType Directory -Path $targetParent -Force)
    }
    Copy-Item -LiteralPath $source -Destination $target -Force
}

function New-IdealValidatorFixture {
    $fixtureRoot = Join-Path $tempBase ([Guid]::NewGuid().ToString("N"))
    [void](New-Item -ItemType Directory -Path $fixtureRoot -Force)
    foreach ($path in @(
        ".beads/issues.jsonl",
        "docs/AUTORUN_STATE.md",
        "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md",
        "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
        "docs/validation/IDEAL_MATRIX_OWNERSHIP_V1.csv",
        "docs/validation/IDEAL_MATRIX_BEAD_TRACEABILITY_V1.csv",
        "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv",
        "docs/validation/IDEAL_CONTRACT_CLAUSE_DISPOSITION_V1.csv"
    )) {
        Copy-FixtureFile -FixtureRoot $fixtureRoot -RelativePath $path
    }
    foreach ($owner in @(Import-Csv -LiteralPath (Join-Path $fixtureRoot "docs/validation/IDEAL_MATRIX_OWNERSHIP_V1.csv"))) {
        Copy-FixtureFile -FixtureRoot $fixtureRoot -RelativePath ([string]$owner.path)
    }
    return $fixtureRoot
}

function Invoke-ExpectedFailure {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Action,
        [Parameter(Mandatory = $true)][string]$MessagePattern
    )

    $failedAsExpected = $false
    try {
        & $Action
    }
    catch {
        if ($_.Exception.Message -notmatch $MessagePattern) {
            throw "negative validator case '$Name' failed for the wrong reason: $($_.Exception.Message)"
        }
        $failedAsExpected = $true
    }
    if (-not $failedAsExpected) {
        throw "negative validator case '$Name' unexpectedly passed"
    }
    Write-Host "negative-validator: ok ($Name)"
}

function Update-FixtureIssue {
    param(
        [Parameter(Mandatory = $true)][string]$FixtureRoot,
        [Parameter(Mandatory = $true)][string]$IssueId,
        [Parameter(Mandatory = $true)][scriptblock]$Mutation
    )

    $issuesPath = Join-Path $FixtureRoot ".beads/issues.jsonl"
    $lines = @(Get-Content -LiteralPath $issuesPath)
    $escaped = [regex]::Escape($IssueId)
    $pattern = '"id":"' + $escaped + '"'
    $indexes = @(0..($lines.Count - 1) | Where-Object { $lines[$_] -match $pattern })
    if ($indexes.Count -ne 1) {
        throw "negative validator fixture expected one issue '$IssueId', found $($indexes.Count)"
    }
    $index = $indexes[0]
    $issue = $lines[$index] | ConvertFrom-Json
    & $Mutation $issue
    $lines[$index] = $issue | ConvertTo-Json -Depth 100 -Compress
    Set-Content -LiteralPath $issuesPath -Value $lines -Encoding UTF8
}

$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
$tempBase = Join-Path $tempRoot ("oxvba-ideal-validator-negative-" + [Guid]::NewGuid().ToString("N"))
$resolvedTempBase = [IO.Path]::GetFullPath($tempBase)
if (-not $resolvedTempBase.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) {
    throw "negative validator temp root escaped the system temp directory"
}
if (-not (Test-Path -LiteralPath $tempBase -PathType Container)) {
    [void](New-Item -ItemType Directory -Path $tempBase -Force)
}

try {
    Invoke-ExpectedFailure -Name "acceptance-command" -MessagePattern "typed command" -Action {
        Assert-IdealExecutableAcceptanceGrammar `
            -Text "Expected observable: green. artifact:docs/evidence/example.md" `
            -Owner "synthetic leaf"
    }

    Invoke-ExpectedFailure -Name "verified-observable-axes" -MessagePattern "observable axis" -Action {
        Assert-IdealVerifiedEvidenceGrammar `
            -RepoRoot $repoRoot `
            -EvidenceRefs "observables:result=verified,full-err=n/a;artifact:CHARTER.md" `
            -Owner "synthetic verified row" `
            -EnvironmentById @{}
    }

    $directRow = [pscustomobject]@{
        row_id = "LSB-DIRECT"
        truth_state = "planned"
        direct_state = "planned"
        direct_api_method = "definition"
    }
    $lspRow = [pscustomobject]@{
        capability_advertised = "true"
        truth_state = "verified"
        projection_state = "verified"
        equivalence_state = "verified"
        direct_matrix_row = "LSB-DIRECT"
        direct_api_method = "definition"
    }
    Invoke-ExpectedFailure -Name "lsp-advertisement" -MessagePattern "direct row.*verified" -Action {
        Assert-IdealLspAdvertisement `
            -Row $lspRow `
            -DirectRowsById @{ "LSB-DIRECT" = $directRow } `
            -Owner "synthetic LSP method"
    }

    $fixture = New-IdealValidatorFixture
    $clausePath = Join-Path $fixture "docs/validation/IDEAL_CONTRACT_CLAUSE_DISPOSITION_V1.csv"
    $clauseRows = @(Import-Csv -LiteralPath $clausePath)
    @($clauseRows | Select-Object -Skip 1) | Export-Csv -LiteralPath $clausePath -NoTypeInformation
    Invoke-ExpectedFailure -Name "missing-contract-clause" -MessagePattern "clause set versus disposition ledger.*differs" -Action {
        & (Join-Path $PSScriptRoot "validate-contract-clause-disposition.ps1") `
            -RepositoryRoot $fixture
    }

    $fixture = New-IdealValidatorFixture
    $tracePath = Join-Path $fixture "docs/validation/IDEAL_MATRIX_BEAD_TRACEABILITY_V1.csv"
    $traceRows = @(Import-Csv -LiteralPath $tracePath)
    $removed = 0
    foreach ($trace in @($traceRows | Where-Object parent_epic -eq "bd-59co.2.1")) {
        $before = @([string]$trace.contract_clauses -split '\|')
        $after = @($before | Where-Object { $_ -ne "AUTH-CLEAN-001" })
        if ($after.Count -ne $before.Count) {
            $trace.contract_clauses = $after -join '|'
            $removed++
        }
    }
    if ($removed -eq 0) {
        throw "negative validator fixture has no AUTH-CLEAN-001 owner witness to remove"
    }
    $traceRows | Export-Csv -LiteralPath $tracePath -NoTypeInformation
    Invoke-ExpectedFailure -Name "missing-clause-owner-witness" -MessagePattern "AUTH-CLEAN-001 owner 'bd-59co\.2\.1'.*no clause-bearing trace" -Action {
        & (Join-Path $PSScriptRoot "validate-contract-clause-disposition.ps1") `
            -RepositoryRoot $fixture
    }

    $fixture = New-IdealValidatorFixture
    $tracePath = Join-Path $fixture "docs/validation/IDEAL_MATRIX_BEAD_TRACEABILITY_V1.csv"
    $traceRows = @(Import-Csv -LiteralPath $tracePath)
    $removed = 0
    foreach ($trace in @($traceRows | Where-Object matrix_id -eq "OXIR-BACKENDS")) {
        $before = @([string]$trace.contract_clauses -split '\|')
        $after = @($before | Where-Object { $_ -ne "CONF-MATRIX-001" })
        if ($after.Count -ne $before.Count) {
            $trace.contract_clauses = $after -join '|'
            $removed++
        }
    }
    if ($removed -eq 0) {
        throw "negative validator fixture has no CONF-MATRIX-001 matrix witness to remove"
    }
    $traceRows | Export-Csv -LiteralPath $tracePath -NoTypeInformation
    Invoke-ExpectedFailure -Name "missing-clause-matrix-witness" -MessagePattern "CONF-MATRIX-001 matrix 'OXIR-BACKENDS'.*no clause-bearing trace" -Action {
        & (Join-Path $PSScriptRoot "validate-contract-clause-disposition.ps1") `
            -RepositoryRoot $fixture
    }

    $fixture = New-IdealValidatorFixture
    $tracePath = Join-Path $fixture "docs/validation/IDEAL_MATRIX_BEAD_TRACEABILITY_V1.csv"
    $traceRows = @(Import-Csv -LiteralPath $tracePath)
    $removed = 0
    foreach ($trace in @($traceRows | Where-Object bead_id -eq "bd-59co.3.1.1")) {
        $before = @([string]$trace.contract_clauses -split '\|')
        $after = @($before | Where-Object { $_ -ne "DOC-AUTH-001" })
        if ($after.Count -ne $before.Count) {
            $trace.contract_clauses = $after -join '|'
            $removed++
        }
    }
    if ($removed -eq 0) {
        throw "negative validator fixture has no DOC-AUTH-001 leaf trace clause to remove"
    }
    $traceRows | Export-Csv -LiteralPath $tracePath -NoTypeInformation
    Invoke-ExpectedFailure -Name "leaf-clause-trace-union" -MessagePattern "execution leaf 'bd-59co\.3\.1\.1'.*contract clause.*absent from trace union.*DOC-AUTH-001" -Action {
        & (Join-Path $PSScriptRoot "validate-bead-traceability.ps1") `
            -RepositoryRoot $fixture
    }

    $fixture = New-IdealValidatorFixture
    $clausePath = Join-Path $fixture "docs/validation/IDEAL_CONTRACT_CLAUSE_DISPOSITION_V1.csv"
    $clauseRows = @(Import-Csv -LiteralPath $clausePath)
    $consumerRow = @($clauseRows | Where-Object clause_id -eq "COMP-BIND-001")[0]
    $consumerRow.consumer_epics = @([string]$consumerRow.consumer_epics -split '\|' | Where-Object { $_ -ne "bd-59co.3.5" }) -join '|'
    $clauseRows | Export-Csv -LiteralPath $clausePath -NoTypeInformation
    Invoke-ExpectedFailure -Name "undeclared-clause-consumer" -MessagePattern "routes COMP-BIND-001 through undeclared producer/consumer epic 'bd-59co\.3\.5'" -Action {
        & (Join-Path $PSScriptRoot "validate-contract-clause-disposition.ps1") `
            -RepositoryRoot $fixture
    }

    $fixture = New-IdealValidatorFixture
    $clausePath = Join-Path $fixture "docs/validation/IDEAL_CONTRACT_CLAUSE_DISPOSITION_V1.csv"
    $clauseRows = @(Import-Csv -LiteralPath $clausePath)
    $matrixRow = @($clauseRows | Where-Object clause_id -eq "CONF-MATRIX-001")[0]
    $matrixRow.matrix_ids = @([string]$matrixRow.matrix_ids -split '\|' | Where-Object { $_ -ne "LS-PERFORMANCE" }) -join '|'
    $clauseRows | Export-Csv -LiteralPath $clausePath -NoTypeInformation
    Invoke-ExpectedFailure -Name "undeclared-clause-matrix" -MessagePattern "routes CONF-MATRIX-001 through undeclared matrix 'LS-PERFORMANCE'" -Action {
        & (Join-Path $PSScriptRoot "validate-contract-clause-disposition.ps1") `
            -RepositoryRoot $fixture
    }

    $fixture = New-IdealValidatorFixture
    $environmentPath = Join-Path $fixture "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv"
    $environmentRows = @(Import-Csv -LiteralPath $environmentPath)
    $environmentRows[0].target_arch = "x86"
    $environmentRows | Export-Csv -LiteralPath $environmentPath -NoTypeInformation
    Invoke-ExpectedFailure -Name "x86-environment" -MessagePattern "target_arch must be x64" -Action {
        & (Join-Path $PSScriptRoot "validate-environment-manifest.ps1") `
            -RepositoryRoot $fixture
    }

    $fixture = New-IdealValidatorFixture
    $ownershipRows = @(Import-Csv -LiteralPath (Join-Path $fixture "docs/validation/IDEAL_MATRIX_OWNERSHIP_V1.csv"))
    $windowsOwner = @($ownershipRows | Where-Object { [string]$_.profile -eq "windows-x64" } | Select-Object -First 1)[0]
    $windowsMatrixPath = Resolve-IdealRepoPath -RepoRoot $fixture -Path ([string]$windowsOwner.path)
    $windowsRows = @(Import-Csv -LiteralPath $windowsMatrixPath)
    if ($windowsRows.Count -eq 0 -or $windowsRows[0].PSObject.Properties.Name -notcontains "environment_id") {
        throw "negative validator fixture has no Windows environment_id row"
    }
    $windowsRows[0].environment_id = "win-x64-dev-oracle-stale-v1"
    $windowsRows | Export-Csv -LiteralPath $windowsMatrixPath -NoTypeInformation
    Invoke-ExpectedFailure -Name "stale-matrix-environment" -MessagePattern "environment_id references unknown environment" -Action {
        & (Join-Path $PSScriptRoot "validate-environment-manifest.ps1") `
            -RepositoryRoot $fixture
    }

    $fixture = New-IdealValidatorFixture
    $environmentPath = Join-Path $fixture "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv"
    $environmentRows = @(Import-Csv -LiteralPath $environmentPath)
    $devEnvironmentId = [string]@($environmentRows | Where-Object { [string]$_.role -eq "dev-oracle" } | Select-Object -First 1)[0].environment_id
    $certEnvironmentId = [string]@($environmentRows | Where-Object { [string]$_.role -eq "certification-vm" } | Select-Object -First 1)[0].environment_id
    $ownershipRows = @(Import-Csv -LiteralPath (Join-Path $fixture "docs/validation/IDEAL_MATRIX_OWNERSHIP_V1.csv"))
    $windowsOwner = @($ownershipRows | Where-Object { [string]$_.profile -eq "windows-x64" } | Select-Object -First 1)[0]
    $windowsMatrixPath = Resolve-IdealRepoPath -RepoRoot $fixture -Path ([string]$windowsOwner.path)
    $windowsRows = @(Import-Csv -LiteralPath $windowsMatrixPath)
    if ($windowsRows.Count -eq 0) {
        throw "negative validator fixture has no Windows row for certification provenance case"
    }
    $windowsRows[0].required = "true"
    $windowsRows[0].truth_state = "verified"
    $windowsRows[0].evidence_refs = "environment:$devEnvironmentId"
    $windowsRows | Export-Csv -LiteralPath $windowsMatrixPath -NoTypeInformation
    Invoke-ExpectedFailure -Name "noncertifying-windows-evidence" -MessagePattern "must reference environment:$([regex]::Escape($certEnvironmentId))" -Action {
        & (Join-Path $PSScriptRoot "validate-environment-manifest.ps1") `
            -RepositoryRoot $fixture
    }

    $fixture = New-IdealValidatorFixture
    $autorunPath = Join-Path $fixture "docs/AUTORUN_STATE.md"
    $autorunText = Get-Content -LiteralPath $autorunPath -Raw
    $autorunText = [regex]::Replace($autorunText, '(?im)^Mode:\s*[^\r\n]+$', 'Mode: AutoRun')
    Set-Content -LiteralPath $autorunPath -Value $autorunText -Encoding UTF8
    $ownershipRows = @(Import-Csv -LiteralPath (Join-Path $fixture "docs/validation/IDEAL_MATRIX_OWNERSHIP_V1.csv"))
    $matrixPath = Resolve-IdealRepoPath -RepoRoot $fixture -Path ([string]$ownershipRows[0].path)
    $matrixHeader = Get-Content -LiteralPath $matrixPath -First 1
    Set-Content -LiteralPath $matrixPath -Value $matrixHeader -Encoding UTF8
    Invoke-ExpectedFailure -Name "autorun-empty-matrix" -MessagePattern "AutoRun cannot start with empty matrix" -Action {
        & (Join-Path $PSScriptRoot "validate-closure-taxonomy.ps1") `
            -RepositoryRoot $fixture
    }

    Invoke-ExpectedFailure -Name "closed-rollout-scaffold" -MessagePattern "closed rollout.*matrix-scaffold" -Action {
        Assert-IdealClosedRolloutTraceState `
            -RolloutId "synthetic-rollout" `
            -RolloutTraces @([pscustomobject]@{
                relationship = "matrix-scaffold"
                matrix_id = "CORE-READINESS"
                row_id = ""
            }) `
            -MatrixRowsById @{} `
            -DeliveryLeafIds @("synthetic-delivery") `
            -TraceRowsByBead @{}
    }

    $fixture = New-IdealValidatorFixture
    $tracePath = Join-Path $fixture "docs/validation/IDEAL_MATRIX_BEAD_TRACEABILITY_V1.csv"
    $traceRows = @(Import-Csv -LiteralPath $tracePath)
    $targetTrace = @($traceRows | Where-Object { [string]$_.bead_id -eq "bd-59co.2.1.1" } | Select-Object -First 1)[0]
    if ($null -eq $targetTrace) {
        throw "negative validator fixture has no CORE-0 rollout trace"
    }
    $leafArtifactPattern = '(?i)^artifact:docs/evidence/programs/ideal-2026-07/core/CORE-0/rollout-acceptance\.md$'
    $tokensBefore = @([string]$targetTrace.acceptance_evidence -split '\|')
    $tokensAfter = @($tokensBefore | Where-Object { $_ -notmatch $leafArtifactPattern })
    if ($tokensAfter.Count -eq $tokensBefore.Count) {
        throw "negative validator fixture CORE-0 trace has no exact leaf artifact to remove"
    }
    $targetTrace.acceptance_evidence = $tokensAfter -join '|'
    $traceRows | Export-Csv -LiteralPath $tracePath -NoTypeInformation
    Invoke-ExpectedFailure -Name "trace-leaf-acceptance" -MessagePattern "omits leaf artifact" -Action {
        & (Join-Path $PSScriptRoot "validate-bead-traceability.ps1") `
            -RepositoryRoot $fixture
    }

    $fixture = New-IdealValidatorFixture
    $syntheticChild = [pscustomobject][ordered]@{
        id = "bd-negative-nonleaf-child"
        title = "Synthetic child that makes a traced task non-leaf"
        description = "Negative validator fixture only."
        acceptance_criteria = "n/a"
        status = "open"
        priority = 4
        issue_type = "epic"
        estimated_minutes = 0
        created_at = "2026-07-11T00:00:00Z"
        created_by = "negative-validator"
        updated_at = "2026-07-11T00:00:00Z"
        source_repo = "."
        compaction_level = 0
        original_size = 0
        labels = @("ideal-2026-07", "support")
        dependencies = @([pscustomobject][ordered]@{
            issue_id = "bd-negative-nonleaf-child"
            depends_on_id = "bd-59co.2.1.1"
            type = "parent-child"
            created_at = "2026-07-11T00:00:00Z"
            created_by = "negative-validator"
            metadata = "{}"
            thread_id = ""
        })
    }
    Add-Content -LiteralPath (Join-Path $fixture ".beads/issues.jsonl") -Value ($syntheticChild | ConvertTo-Json -Depth 20 -Compress) -Encoding UTF8
    Invoke-ExpectedFailure -Name "traced-nonleaf" -MessagePattern "traced bead 'bd-59co\.2\.1\.1' is not an execution leaf" -Action {
        & (Join-Path $PSScriptRoot "validate-bead-traceability.ps1") `
            -RepositoryRoot $fixture
    }

    $fixture = New-IdealValidatorFixture
    Update-FixtureIssue -FixtureRoot $fixture -IssueId "bd-59co.2.1.1" -Mutation {
        param($issue)
        $issue.labels = @($issue.labels | Where-Object { [string]$_ -notlike "resource-*" })
    }
    Invoke-ExpectedFailure -Name "missing-resource-metadata" -MessagePattern "explicit resource-\* scheduling metadata" -Action {
        & (Join-Path $PSScriptRoot "validate-workset-rollout.ps1") `
            -RepositoryRoot $fixture `
            -SkipReadyQueue `
            -SkipCycleCheck
    }

    $fixture = New-IdealValidatorFixture
    Update-FixtureIssue -FixtureRoot $fixture -IssueId "bd-59co.2.1.1" -Mutation {
        param($issue)
        $issue.labels = @($issue.labels | Where-Object { [string]$_ -notlike "resource-*" }) + @("resource-large-jit")
    }
    Invoke-ExpectedFailure -Name "large-writer-implies-rust" -MessagePattern "resource-large-\* must also carry resource-rust-writer" -Action {
        & (Join-Path $PSScriptRoot "validate-workset-rollout.ps1") `
            -RepositoryRoot $fixture `
            -SkipReadyQueue `
            -SkipCycleCheck
    }

    $fixture = New-IdealValidatorFixture
    foreach ($issueId in @("bd-59co.2.2.2", "bd-h4oh.8", "bd-59co.2.4.3")) {
        Update-FixtureIssue -FixtureRoot $fixture -IssueId $issueId -Mutation {
            param($issue)
            $issue.status = "in_progress"
        }
    }
    Invoke-ExpectedFailure -Name "rust-writer-limit" -MessagePattern "active Rust writers exceed limit 2" -Action {
        & (Join-Path $PSScriptRoot "validate-workset-rollout.ps1") `
            -RepositoryRoot $fixture `
            -SkipReadyQueue `
            -SkipCycleCheck
    }

    Write-Host "test-ideal-program-validator-negative-cases: ok (cases=19)"
}
finally {
    if (Test-Path -LiteralPath $tempBase -PathType Container) {
        $resolved = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $tempBase).Path)
        if (-not $resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw "refusing to remove validator temp directory outside system temp"
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
