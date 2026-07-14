Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$sync = Join-Path $PSScriptRoot "sync-windows-dev-environment.ps1"
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$runRoot = Join-Path $tempRoot "oxvba-dev-environment-sync-$PID-$([guid]::NewGuid().ToString('N'))"

function New-Fixture {
    param([Parameter(Mandatory = $true)][string]$Name)

    $root = Join-Path $runRoot $Name
    $manifestTarget = Join-Path $root "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv"
    $captureTarget = Join-Path $root "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-0/dev-oracle-environment.json"
    [void](New-Item -ItemType Directory -Path (Split-Path -Parent $manifestTarget) -Force)
    [void](New-Item -ItemType Directory -Path (Split-Path -Parent $captureTarget) -Force)
    Copy-Item -LiteralPath (Join-Path $repoRoot "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv") -Destination $manifestTarget
    Copy-Item -LiteralPath (Join-Path $repoRoot "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-0/dev-oracle-environment.json") -Destination $captureTarget
    return $root
}

function Invoke-ExpectedFailure {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Action
    )

    $failed = $false
    try {
        & $Action
    }
    catch {
        $failed = $true
    }
    if (-not $failed) {
        throw "test-windows-dev-environment: mutation '$Name' was accepted"
    }
}

[void](New-Item -ItemType Directory -Path $runRoot)
try {
    $positive = New-Fixture -Name "positive"
    & $sync -RepositoryRoot $positive
    & $sync -Check -RepositoryRoot $positive

    $missingSource = New-Fixture -Name "missing-source"
    Remove-Item -LiteralPath (Join-Path $missingSource "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-0/dev-oracle-environment.json")
    Invoke-ExpectedFailure -Name "missing accepted source" -Action { & $sync -RepositoryRoot $missingSource *> $null }

    $missingControlled = New-Fixture -Name "missing-controlled"
    Invoke-ExpectedFailure -Name "missing controlled publication" -Action { & $sync -Check -RepositoryRoot $missingControlled *> $null }

    $controlledDrift = New-Fixture -Name "controlled-drift"
    & $sync -RepositoryRoot $controlledDrift *> $null
    Add-Content -LiteralPath (Join-Path $controlledDrift "artifacts/windows-x64/controlled-environments/v1/win-x64-dev-oracle-2026-07/environment-capture.json") -Value " "
    Invoke-ExpectedFailure -Name "controlled byte drift" -Action { & $sync -Check -RepositoryRoot $controlledDrift *> $null }

    $badSourcePreserves = New-Fixture -Name "bad-source-preserves"
    & $sync -RepositoryRoot $badSourcePreserves *> $null
    $badSourceControlledPath = Join-Path $badSourcePreserves "artifacts/windows-x64/controlled-environments/v1/win-x64-dev-oracle-2026-07/environment-capture.json"
    $badSourceBefore = [Convert]::ToBase64String([IO.File]::ReadAllBytes($badSourceControlledPath))
    Add-Content -LiteralPath (Join-Path $badSourcePreserves "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-0/dev-oracle-environment.json") -Value " "
    Invoke-ExpectedFailure -Name "bad source before immutable publication" -Action { & $sync -RepositoryRoot $badSourcePreserves *> $null }
    if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($badSourceControlledPath)) -cne $badSourceBefore) {
        throw "test-windows-dev-environment: bad source changed an existing controlled publication"
    }

    $differentExisting = New-Fixture -Name "different-existing"
    $differentExistingPath = Join-Path $differentExisting "artifacts/windows-x64/controlled-environments/v1/win-x64-dev-oracle-2026-07/environment-capture.json"
    [void](New-Item -ItemType Directory -Path (Split-Path -Parent $differentExistingPath) -Force)
    [IO.File]::WriteAllText($differentExistingPath, "{}`n", [Text.UTF8Encoding]::new($false))
    $differentBefore = [Convert]::ToBase64String([IO.File]::ReadAllBytes($differentExistingPath))
    Invoke-ExpectedFailure -Name "different immutable publication" -Action { & $sync -RepositoryRoot $differentExisting *> $null }
    if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($differentExistingPath)) -cne $differentBefore) {
        throw "test-windows-dev-environment: differing existing publication was replaced"
    }

    $certifying = New-Fixture -Name "certifying"
    $certifyingPath = Join-Path $certifying "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-0/dev-oracle-environment.json"
    $certifyingJson = [IO.File]::ReadAllText($certifyingPath).Replace('"certification_authority": false', '"certification_authority": true')
    [IO.File]::WriteAllText($certifyingPath, $certifyingJson, [Text.UTF8Encoding]::new($false))
    Invoke-ExpectedFailure -Name "certification authority" -Action { & $sync -RepositoryRoot $certifying *> $null }

    $ownerDrift = New-Fixture -Name "owner-drift"
    $ownerPath = Join-Path $ownerDrift "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv"
    $ownerRows = @(Import-Csv -LiteralPath $ownerPath)
    ($ownerRows | Where-Object role -eq "dev-oracle").owner_bead = "bd-59co.3.1.7"
    $ownerRows | Export-Csv -LiteralPath $ownerPath -NoTypeInformation -UseQuotes Always
    Invoke-ExpectedFailure -Name "capture authority owner drift" -Action { & $sync -RepositoryRoot $ownerDrift *> $null }

    $environmentDrift = New-Fixture -Name "environment-drift"
    $environmentPath = Join-Path $environmentDrift "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv"
    $environmentRows = @(Import-Csv -LiteralPath $environmentPath)
    ($environmentRows | Where-Object role -eq "dev-oracle").office_bitness = "32"
    $environmentRows | Export-Csv -LiteralPath $environmentPath -NoTypeInformation -UseQuotes Always
    Invoke-ExpectedFailure -Name "environment fact drift" -Action { & $sync -RepositoryRoot $environmentDrift *> $null }

    $reparseStatus = "unsupported"
    $reparseFixture = New-Fixture -Name "reparse"
    $reparseParent = Join-Path $reparseFixture "artifacts/windows-x64/controlled-environments"
    $reparseTarget = Join-Path $runRoot "reparse-target"
    $reparseLink = Join-Path $reparseParent "v1"
    [void](New-Item -ItemType Directory -Path $reparseParent -Force)
    [void](New-Item -ItemType Directory -Path $reparseTarget -Force)
    try {
        [void](New-Item -ItemType Junction -Path $reparseLink -Target $reparseTarget -ErrorAction Stop)
        Invoke-ExpectedFailure -Name "controlled-root reparse" -Action { & $sync -RepositoryRoot $reparseFixture *> $null }
        if (Test-Path -LiteralPath (Join-Path $reparseTarget "win-x64-dev-oracle-2026-07/environment-capture.json")) {
            throw "test-windows-dev-environment: reparse mutation wrote outside the controlled root"
        }
        $reparseStatus = "passed"
    }
    catch {
        if (Test-Path -LiteralPath $reparseLink) {
            throw
        }
    }
    finally {
        if (Test-Path -LiteralPath $reparseLink) {
            Remove-Item -LiteralPath $reparseLink -Force
        }
    }

    Write-Host "test-windows-dev-environment: ok (positive write/check plus 8 fail-closed mutations; reparse=$reparseStatus)"
}
finally {
    if (Test-Path -LiteralPath $runRoot) {
        $resolvedRunRoot = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $runRoot).Path)
        if (-not $resolvedRunRoot.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -or
            [IO.Path]::GetFileName($resolvedRunRoot) -notlike "oxvba-dev-environment-sync-*") {
            throw "test-windows-dev-environment: refusing to remove unexpected path '$resolvedRunRoot'"
        }
        Remove-Item -LiteralPath $resolvedRunRoot -Recurse -Force
    }
}
