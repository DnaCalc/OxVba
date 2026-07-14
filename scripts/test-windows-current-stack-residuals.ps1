Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$validator = Join-Path $PSScriptRoot "validate-windows-current-stack-residuals.ps1"
$baselineLedger = Join-Path $repoRoot "docs/validation/IDEAL_WINDOWS_CURRENT_STACK_RESIDUAL_V1.csv"
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$tempDirectory = Join-Path $tempRoot "oxvba-win-residual-validator-$PID-$([guid]::NewGuid().ToString('N'))"

function Write-MutatedLedger {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Mutation
    )

    $rows = @(Import-Csv -LiteralPath $baselineLedger)
    $mutatedRows = @(& $Mutation $rows)
    $path = Join-Path $tempDirectory "$Name.csv"
    $mutatedRows | Export-Csv -LiteralPath $path -NoTypeInformation -UseQuotes Always
    return $path
}

function Assert-ValidatorFailure {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][hashtable]$Arguments
    )

    $failed = $false
    try {
        & $validator -RepositoryRoot $repoRoot @Arguments *> $null
    }
    catch {
        $failed = $true
    }
    if (-not $failed) {
        throw "test-windows-current-stack-residuals: mutation '$Name' was accepted"
    }
}

New-Item -ItemType Directory -Path $tempDirectory | Out-Null
try {
    & $validator -RepositoryRoot $repoRoot

    $missingRow = Write-MutatedLedger -Name "missing-row" -Mutation {
        param($rows)
        @($rows | Select-Object -Skip 1)
    }
    Assert-ValidatorFailure -Name "missing canonical row" -Arguments @{ LedgerPath = $missingRow }

    $truthCredit = Write-MutatedLedger -Name "truth-credit" -Mutation {
        param($rows)
        ($rows | Where-Object row_id -eq "WCC-PLAN-LATE").canonical_truth_state = "verified"
        $rows
    }
    Assert-ValidatorFailure -Name "characterization advances capability truth" -Arguments @{ LedgerPath = $truthCredit }

    $jitCredit = Write-MutatedLedger -Name "jit-credit-from-vm-history" -Mutation {
        param($rows)
        $row = $rows | Where-Object row_id -eq "WNE-PLAN-NATIVE"
        $row.current_code_state = "current-subset"
        $row.current_test_state = "current-subset"
        $row.gap_kind = "backend-divergence"
        $row.current_code_anchors = "crates/oxvba-jit/src/lib.rs|crates/oxvba-vm3/src/lib.rs"
        $row.current_test_anchors = "docs/evidence/v0_2/V02_NATIVE_COMPILATION_SCAFFOLD_2026-04-27.md"
        $rows
    }
    Assert-ValidatorFailure -Name "JIT credit sourced only from VM3 and historical evidence" -Arguments @{ LedgerPath = $jitCredit }

    $historicalAsCurrent = Write-MutatedLedger -Name "historical-as-current" -Mutation {
        param($rows)
        $row = $rows | Where-Object row_id -eq "WCC-EXCEL-AUTHORITY"
        $row.current_test_state = "current-subset"
        $row.current_test_anchors = "docs/evidence/conformance/com/COM_LANE_L2E_RUN_Scripting.Dictionary_20260308T190000Z.md"
        $rows
    }
    Assert-ValidatorFailure -Name "historical evidence credited as current test" -Arguments @{ LedgerPath = $historicalAsCurrent }

    $supportOwner = Write-MutatedLedger -Name "support-owner" -Mutation {
        param($rows)
        ($rows | Where-Object row_id -eq "WCE-INCOMING-COMPLEX").live_residual_owner_bead = "bd-59co.3.6.4"
        $rows
    }
    Assert-ValidatorFailure -Name "capability residual parked on support bead" -Arguments @{ LedgerPath = $supportOwner }

    $lostLegacyRoute = Write-MutatedLedger -Name "lost-legacy-route" -Mutation {
        param($rows)
        ($rows | Where-Object row_id -eq "WCE-PLAN-INCOMING").legacy_route_ids = "none"
        $rows
    }
    Assert-ValidatorFailure -Name "imported event route removed" -Arguments @{ LedgerPath = $lostLegacyRoute }

    $unresolvedAnchor = Write-MutatedLedger -Name "unresolved-anchor" -Mutation {
        param($rows)
        ($rows | Where-Object row_id -eq "WAC-BSTR-LAYOUT").current_code_anchors = "crates/oxvba-runtime/src/not-a-carrier.rs"
        $rows
    }
    Assert-ValidatorFailure -Name "unresolved current code anchor" -Arguments @{ LedgerPath = $unresolvedAnchor }

    $evidenceAsCode = Write-MutatedLedger -Name "evidence-as-code" -Mutation {
        param($rows)
        ($rows | Where-Object row_id -eq "WAC-BSTR-LAYOUT").current_code_anchors = "docs/evidence/conformance/com/COM_LANE_L2E_RUN_Scripting.Dictionary_20260308T190000Z.md"
        $rows
    }
    Assert-ValidatorFailure -Name "historical evidence credited as current code" -Arguments @{ LedgerPath = $evidenceAsCode }

    $hiddenBlocker = Write-MutatedLedger -Name "hidden-blocker" -Mutation {
        param($rows)
        ($rows | Where-Object row_id -eq "WCE-PLAN-INCOMING").gap_kind = "missing-current-implementation"
        $rows
    }
    Assert-ValidatorFailure -Name "synchronous ByRef blocker reclassified" -Arguments @{ LedgerPath = $hiddenBlocker }

    $migrationPath = Join-Path $tempDirectory "legacy-migration.csv"
    $migrationRows = @(Import-Csv -LiteralPath (Join-Path $repoRoot "docs/validation/IDEAL_LEGACY_BEAD_MIGRATION_V1.csv"))
    ($migrationRows | Where-Object legacy_id -eq "bd-9sed.17").status_after = "closed"
    $migrationRows | Export-Csv -LiteralPath $migrationPath -NoTypeInformation -UseQuotes Always
    Assert-ValidatorFailure -Name "imported callback route closed" -Arguments @{ LegacyMigrationPath = $migrationPath }

    Write-Host "test-windows-current-stack-residuals: ok (positive case plus 10 fail-closed mutations)"
}
finally {
    if (Test-Path -LiteralPath $tempDirectory) {
        $resolvedTempDirectory = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $tempDirectory).Path)
        if (-not $resolvedTempDirectory.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -or
            [IO.Path]::GetFileName($resolvedTempDirectory) -notlike "oxvba-win-residual-validator-*") {
            throw "test-windows-current-stack-residuals: refusing to remove unexpected path '$resolvedTempDirectory'"
        }
        Remove-Item -LiteralPath $resolvedTempDirectory -Recurse -Force
    }
}
