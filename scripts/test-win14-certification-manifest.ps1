Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$validator = Join-Path $PSScriptRoot "validate-win14-certification-manifest.ps1"
$sync = Join-Path $PSScriptRoot "sync-win14-certification-manifest.ps1"
$baselinePath = Join-Path $repoRoot "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/certification-cases.json"
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$tempDirectory = Join-Path $tempRoot "oxvba-win14-cert-manifest-$PID-$([guid]::NewGuid().ToString('N'))"

function Write-MutatedManifest {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Mutation
    )

    $manifest = Get-Content -LiteralPath $baselinePath -Raw | ConvertFrom-Json
    & $Mutation $manifest
    $path = Join-Path $tempDirectory "$Name.json"
    $json = ($manifest | ConvertTo-Json -Depth 20) -replace "`r`n", "`n"
    [IO.File]::WriteAllText($path, $json + "`n", [Text.UTF8Encoding]::new($false))
    return $path
}

function Assert-ValidatorFailure {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $failed = $false
    try {
        & $validator -RepositoryRoot $repoRoot -ManifestPath $Path *> $null
    }
    catch {
        $failed = $true
    }
    if (-not $failed) {
        throw "test-win14-certification-manifest: mutation '$Name' was accepted"
    }
}

New-Item -ItemType Directory -Path $tempDirectory | Out-Null
try {
    & $sync -RepositoryRoot $repoRoot -Check
    & $validator -RepositoryRoot $repoRoot

    $missingCase = Write-MutatedManifest -Name "missing-case" -Mutation {
        param($manifest)
        $manifest.cases = @($manifest.cases | Select-Object -Skip 1)
    }
    Assert-ValidatorFailure -Name "missing canonical case" -Path $missingCase

    $duplicateMapping = Write-MutatedManifest -Name "duplicate-mapping" -Mutation {
        param($manifest)
        $manifest.cases[1].matrix_id = $manifest.cases[0].matrix_id
        $manifest.cases[1].row_id = $manifest.cases[0].row_id
    }
    Assert-ValidatorFailure -Name "duplicate row mapping" -Path $duplicateMapping

    $unownedProducer = Write-MutatedManifest -Name "unowned-producer" -Mutation {
        param($manifest)
        $manifest.cases[0].producer_gate.owner_bead = "bd-not-a-live-owner"
    }
    Assert-ValidatorFailure -Name "unowned producer gate" -Path $unownedProducer

    $missingAxis = Write-MutatedManifest -Name "missing-axis" -Mutation {
        param($manifest)
        $manifest.cases[0].observable_axes.PSObject.Properties.Remove("balance")
    }
    Assert-ValidatorFailure -Name "missing balance axis" -Path $missingAxis

    $promotedProducer = Write-MutatedManifest -Name "promoted-producer" -Mutation {
        param($manifest)
        $manifest.cases[0].producer_gate.current_truth_state = "verified"
        $manifest.cases[0].producer_gate.state = "ready"
    }
    Assert-ValidatorFailure -Name "producer promoted ahead of matrix" -Path $promotedProducer

    $promotedEnvironment = Write-MutatedManifest -Name "promoted-environment" -Mutation {
        param($manifest)
        $manifest.certification_environment.current_evidence_state = "verified"
        $manifest.certification_environment.state = "ready"
    }
    Assert-ValidatorFailure -Name "environment promoted ahead of sealing" -Path $promotedEnvironment

    $developmentEnvironmentCredit = Write-MutatedManifest -Name "development-environment-credit" -Mutation {
        param($manifest)
        $artifact = $manifest.cases[0].artifacts | Where-Object kind -eq "environment-capture"
        $artifact.path = "artifacts/windows-x64/controlled-environments/v1/win-x64-dev-oracle-2026-07/environment-capture.json"
    }
    Assert-ValidatorFailure -Name "development host used as certification evidence" -Path $developmentEnvironmentCredit

    $promotedFixture = Write-MutatedManifest -Name "promoted-fixture" -Mutation {
        param($manifest)
        $manifest.cases[0].fixture.built_artifact_state = "current"
        ($manifest.cases[0].artifacts | Where-Object kind -eq "controlled-fixture").state = "current"
    }
    Assert-ValidatorFailure -Name "built fixture promoted without artifact" -Path $promotedFixture

    $capabilityCredit = Write-MutatedManifest -Name "capability-credit" -Mutation {
        param($manifest)
        $manifest.cases[0].capability_credit = "verified"
    }
    Assert-ValidatorFailure -Name "support manifest grants capability credit" -Path $capabilityCredit

    $missingAggregate = Write-MutatedManifest -Name "missing-aggregate" -Mutation {
        param($manifest)
        $manifest.aggregate_anchors = @($manifest.aggregate_anchors | Select-Object -Skip 1)
    }
    Assert-ValidatorFailure -Name "missing aggregate anchor" -Path $missingAggregate

    $prematureExecution = Write-MutatedManifest -Name "premature-execution" -Mutation {
        param($manifest)
        $manifest.cases[0].certification_state = "executable"
        $manifest.cases[0].blocking_reasons = @()
        foreach ($command in $manifest.cases[0].commands) { $command.state = "ready" }
    }
    Assert-ValidatorFailure -Name "case executable with pending gates" -Path $prematureExecution

    Write-Host "test-win14-certification-manifest: ok (positive case plus 11 fail-closed mutations)"
}
finally {
    if (Test-Path -LiteralPath $tempDirectory) {
        $resolvedTempDirectory = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $tempDirectory).Path)
        if (-not $resolvedTempDirectory.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -or
            [IO.Path]::GetFileName($resolvedTempDirectory) -notlike "oxvba-win14-cert-manifest-*") {
            throw "test-win14-certification-manifest: refusing to remove unexpected path '$resolvedTempDirectory'"
        }
        Remove-Item -LiteralPath $resolvedTempDirectory -Recurse -Force
    }
}
