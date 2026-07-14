param(
    [string]$EnvironmentId = "win-x64-dev-oracle-2026-07",
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
. (Join-Path $PSScriptRoot "lib-windows-fixture-manifest.ps1")
. (Join-Path $PSScriptRoot "lib-ideal-environment-capture.ps1")

$positive = 0
$negative = 0

function Copy-CaptureValue {
    param([Parameter(Mandatory = $true)]$Value)

    return ($Value | ConvertTo-Json -Depth 16 | ConvertFrom-Json)
}

function Invoke-ExpectedCaptureFailure {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Action,
        [Parameter(Mandatory = $true)][string]$Pattern
    )

    try {
        & $Action
    }
    catch {
        if ($_.Exception.Message -notmatch $Pattern) {
            throw "capture test '$Name' failed for the wrong reason: $($_.Exception.Message)"
        }
        $script:negative++
        return
    }
    throw "capture test '$Name' unexpectedly passed"
}

$environment = Get-IdealCaptureEnvironmentRow -RepositoryRoot $repoRoot -EnvironmentId $EnvironmentId
$capturePath = Join-Path $repoRoot "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-0/dev-oracle-environment.json"
$captureBytes = [IO.File]::ReadAllBytes($capturePath)
$capture = ConvertFrom-WindowsFixtureAuditedJson -Bytes $captureBytes -Owner "capture test" -FormatName "environment-capture"
Assert-WindowsFixtureEnvironmentCaptureValue `
    -Capture $capture `
    -Environment $environment `
    -ExpectedSchema $script:IdealEnvironmentCaptureSchema `
    -Owner "capture test"
$positive++

$rebuilt = New-IdealEnvironmentCaptureValue -Environment $environment
$rebuiltText = (($rebuilt | ConvertTo-Json -Depth 8).Replace("`r`n", "`n").Replace("`r", "`n")) + "`n"
$capturedText = [Text.UTF8Encoding]::new($false, $true).GetString($captureBytes).Replace("`r`n", "`n").Replace("`r", "`n")
if ($rebuiltText -cne $capturedText) {
    throw "capture test: canonical capture is not deterministic"
}
$positive++

$reportPath = Join-Path $repoRoot "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-0/dev-oracle-environment.md"
$reportText = [IO.File]::ReadAllText($reportPath)
foreach ($required in @("release=false", "| result |", "| full Err |", "| side effects |", "| lifecycle/order |", "| transport |", "| balance |")) {
    if (-not $reportText.Contains($required, [StringComparison]::Ordinal)) {
        throw "capture test: report omits required control evidence '$required'"
    }
}
$toolText = @(
    [IO.File]::ReadAllText((Join-Path $PSScriptRoot "capture-ideal-environment.ps1")),
    [IO.File]::ReadAllText((Join-Path $PSScriptRoot "lib-ideal-environment-capture.ps1"))
) -join "`n"
foreach ($forbidden in @("Start-Process", "Stop-Process", "New-TemporaryFile", "`$env:TEMP", "CreateSubKey", "Set-ItemProperty", "New-ItemProperty", "Remove-ItemProperty", "Excel.Application")) {
    if ($toolText.Contains($forbidden, [StringComparison]::OrdinalIgnoreCase)) {
        throw "capture test: read-only capture tool contains forbidden mutation/automation surface '$forbidden'"
    }
}
$positive++

foreach ($requiredImplementation in @("ReadToEndAsync", "WaitForExit(`$TimeoutMilliseconds)", "Kill(`$true)")) {
    if (-not $toolText.Contains($requiredImplementation, [StringComparison]::Ordinal)) {
        throw "capture test: bounded owned-child implementation omits '$requiredImplementation'"
    }
}
$preimagePattern = '(?s)<!-- oxvba-dev-host-fingerprint-preimage-v1-begin -->\s*```json\n(?<json>.*?)\n```\s*<!-- oxvba-dev-host-fingerprint-preimage-v1-end -->'
$preimageMatches = @([regex]::Matches($reportText.Replace("`r`n", "`n").Replace("`r", "`n"), $preimagePattern))
if ($preimageMatches.Count -ne 1) {
    throw "capture test: expected one persisted host fingerprint preimage"
}
$preimage = ConvertFrom-WindowsFixtureAuditedJson `
    -Bytes ([Text.UTF8Encoding]::new($false).GetBytes($preimageMatches[0].Groups['json'].Value)) `
    -Owner "capture test fingerprint" `
    -FormatName "host-fingerprint"
$preimageHash = Assert-IdealDevHostFingerprintPreimage -Preimage $preimage -Environment $environment -Owner "capture test fingerprint"
if ([string]$environment.snapshot_or_image -cne "dev-host-fingerprint-v1@$preimageHash") {
    throw "capture test: fingerprint preimage does not reproduce the environment identity"
}
$positive++

$recordA = ConvertTo-IdealCaptureLengthPrefixedRecord -Fields ([ordered]@{ a = "x|y"; b = "z" })
$recordB = ConvertTo-IdealCaptureLengthPrefixedRecord -Fields ([ordered]@{ a = "x"; b = "y|z" })
$permutationOne = Get-IdealCaptureRecordSetHash -Schema "capture-test-v1" -Records @($recordA, $recordB)
$permutationTwo = Get-IdealCaptureRecordSetHash -Schema "capture-test-v1" -Records @($recordB, $recordA)
if ($permutationOne -cne $permutationTwo -or `
    (Get-IdealCaptureRecordSetHash -Schema "capture-test-v1" -Records @($recordA)) -ceq `
        (Get-IdealCaptureRecordSetHash -Schema "capture-test-v1" -Records @($recordB))) {
    throw "capture test: record-set hashing is not ordinal/permutation-stable and delimiter-safe"
}
$positive++

$rootContractRows = @(Import-Csv -LiteralPath (Join-Path $repoRoot $script:IdealWindowsFixtureManifestPath))
$rootContractBefore = Get-IdealCaptureFixtureRootContractHash -Rows $rootContractRows
$transitionedRows = @(($rootContractRows | ConvertTo-Json -Depth 8 | ConvertFrom-Json))
$transitionedRows[0].source_recipe_state = "current"
$transitionedRows[0].source_recipe_hash = "sha256:" + ("a" * 64)
$transitionedRows[0].built_artifact_state = "current"
$transitionedRows[0].built_artifact_path = "$($transitionedRows[0].built_artifact_root)/$($transitionedRows[0].built_artifact_name)"
$transitionedRows[0].built_artifact_hash = "sha256:" + ("b" * 64)
$transitionedRows[0].environment_state = "current"
$transitionedRows[0].environment_capture_path = "$($transitionedRows[0].environment_capture_root)/$($transitionedRows[0].environment_capture_name)"
$transitionedRows[0].environment_hash = "sha256:" + ("c" * 64)
$rootContractAfter = Get-IdealCaptureFixtureRootContractHash -Rows $transitionedRows
if ($rootContractBefore -cne $rootContractAfter) {
    throw "capture test: legitimate fixture state/hash transitions changed the immutable root contract"
}
$positive++

& (Join-Path $PSScriptRoot "capture-ideal-environment.ps1") `
    -RepositoryRoot $repoRoot `
    -EnvironmentId $EnvironmentId `
    -Check
$positive++

$captureWriteTime = (Get-Item -LiteralPath $capturePath).LastWriteTimeUtc
$reportWriteTime = (Get-Item -LiteralPath $reportPath).LastWriteTimeUtc
& (Join-Path $PSScriptRoot "capture-ideal-environment.ps1") `
    -RepositoryRoot $repoRoot `
    -EnvironmentId $EnvironmentId
if ((Get-Item -LiteralPath $capturePath).LastWriteTimeUtc -ne $captureWriteTime -or `
    (Get-Item -LiteralPath $reportPath).LastWriteTimeUtc -ne $reportWriteTime) {
    throw "capture test: identical immutable capture unexpectedly rewrote evidence"
}
$positive++

Invoke-ExpectedCaptureFailure -Name "extra-property" -Pattern "exact case-sensitive schema" -Action {
    $mutated = Copy-CaptureValue -Value $capture
    $mutated | Add-Member -NotePropertyName release -NotePropertyValue $false
    Assert-WindowsFixtureEnvironmentCaptureValue -Capture $mutated -Environment $environment -ExpectedSchema $script:IdealEnvironmentCaptureSchema -Owner "extra-property"
}

Invoke-ExpectedCaptureFailure -Name "office32" -Pattern "x64 and Office64" -Action {
    $mutated = Copy-CaptureValue -Value $capture
    $mutated.office_bitness = "32"
    Assert-WindowsFixtureEnvironmentCaptureValue -Capture $mutated -Environment $environment -ExpectedSchema $script:IdealEnvironmentCaptureSchema -Owner "office32"
}

Invoke-ExpectedCaptureFailure -Name "certification-authority" -Pattern "explicitly noncertifying" -Action {
    $mutated = Copy-CaptureValue -Value $capture
    $mutated.certification_authority = $true
    Assert-WindowsFixtureEnvironmentCaptureValue -Capture $mutated -Environment $environment -ExpectedSchema $script:IdealEnvironmentCaptureSchema -Owner "certification-authority"
}

Invoke-ExpectedCaptureFailure -Name "mutable-image" -Pattern "mutable or lacks an immutable SHA-256" -Action {
    $mutatedEnvironment = $environment | ConvertTo-Json -Depth 8 | ConvertFrom-Json
    $mutatedCapture = Copy-CaptureValue -Value $capture
    $mutatedEnvironment.snapshot_or_image = "current-host"
    $mutatedCapture.snapshot_or_image = "current-host"
    Assert-WindowsFixtureEnvironmentCaptureValue -Capture $mutatedCapture -Environment $mutatedEnvironment -ExpectedSchema $script:IdealEnvironmentCaptureSchema -Owner "mutable-image"
}

Invoke-ExpectedCaptureFailure -Name "wrong-capture-id" -Pattern "environment/capture identity" -Action {
    $mutated = Copy-CaptureValue -Value $capture
    $mutated.capture_id = "unbound-capture-v1"
    Assert-WindowsFixtureEnvironmentCaptureValue -Capture $mutated -Environment $environment -ExpectedSchema $script:IdealEnvironmentCaptureSchema -Owner "wrong-capture-id"
}

Invoke-ExpectedCaptureFailure -Name "fingerprint-canonical-binding" -Pattern "os.build differs from the canonical environment" -Action {
    $mutated = Copy-CaptureValue -Value $preimage
    $mutated.os.build = "10.0.0.0"
    [void](Assert-IdealDevHostFingerprintPreimage -Preimage $mutated -Environment $environment -Owner "fingerprint-canonical-binding")
}

Invoke-ExpectedCaptureFailure -Name "fingerprint-nested-schema" -Pattern "exact case-sensitive schema" -Action {
    $mutated = Copy-CaptureValue -Value $preimage
    $mutated.office | Add-Member -NotePropertyName unexpected -NotePropertyValue "value"
    [void](Assert-IdealDevHostFingerprintPreimage -Preimage $mutated -Environment $environment -Owner "fingerprint-nested-schema")
}

Invoke-ExpectedCaptureFailure -Name "fingerprint-blank-observation" -Pattern "client_culture must not be blank" -Action {
    $mutated = Copy-CaptureValue -Value $preimage
    $mutated.office.client_culture = ""
    [void](Assert-IdealDevHostFingerprintPreimage -Preimage $mutated -Environment $environment -Owner "fingerprint-blank-observation")
}

Invoke-ExpectedCaptureFailure -Name "native-reader-missing-command" -Pattern "does not resolve" -Action {
    [void](Invoke-IdealCaptureNativeRead -Command "oxvba-definitely-missing-observer" -TimeoutMilliseconds 100)
}

Invoke-ExpectedCaptureFailure -Name "native-reader-timeout-owned-cleanup" -Pattern "timed out.*owned PID.*terminated and reaped" -Action {
    [void](Invoke-IdealCaptureNativeRead `
        -Command "pwsh" `
        -Arguments @("-NoProfile", "-NonInteractive", "-Command", "[Threading.Thread]::Sleep(30000)") `
        -TimeoutMilliseconds 100)
}

Invoke-ExpectedCaptureFailure -Name "path-escape" -Pattern "dev capture output must remain" -Action {
    & (Join-Path $PSScriptRoot "capture-ideal-environment.ps1") `
        -RepositoryRoot $repoRoot `
        -EnvironmentId $EnvironmentId `
        -OutputPath "../escaped.json" `
        -Check
}

Invoke-ExpectedCaptureFailure -Name "certification-requires-attestation" -Pattern "trusted pinned-image restore/session attestation" -Action {
    & (Join-Path $PSScriptRoot "capture-ideal-environment.ps1") `
        -RepositoryRoot $repoRoot `
        -CaseId "WIN14-WAC-CLEAN-CERT-ENV"
}

Invoke-ExpectedCaptureFailure -Name "certification-case-id-is-exact" -Pattern "expected one certification case.*found 0" -Action {
    & (Join-Path $PSScriptRoot "capture-ideal-environment.ps1") `
        -RepositoryRoot $repoRoot `
        -CaseId "win14-wac-clean-cert-env"
}

Invoke-ExpectedCaptureFailure -Name "certification-path-override" -Pattern "must be the exact controlled environment-capture path" -Action {
    & (Join-Path $PSScriptRoot "capture-ideal-environment.ps1") `
        -RepositoryRoot $repoRoot `
        -CaseId "WIN14-WAC-CLEAN-CERT-ENV" `
        -OutputPath "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/not-controlled.json"
}

$casePath = Join-Path $repoRoot "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/certification-cases.json"
$caseDocument = ConvertFrom-WindowsFixtureAuditedJson -Bytes ([IO.File]::ReadAllBytes($casePath)) -Owner "capture case test" -FormatName "certification-case"
$certificationCase = @($caseDocument.cases | Where-Object case_id -ceq "WIN14-WAC-CLEAN-CERT-ENV")[0]
$certEnvironment = Get-IdealCaptureEnvironmentRow -RepositoryRoot $repoRoot -EnvironmentId ([string]$certificationCase.environment_gate.environment_id)
$certContract = Get-WindowsFixtureEnvironmentCaptureContract -Environment $certEnvironment
$fixtureRows = @(Import-Csv -LiteralPath (Join-Path $repoRoot $script:IdealWindowsFixtureManifestPath))
$certFixtureRow = @($fixtureRows | Where-Object { [string]$_.matrix_id -ceq [string]$certificationCase.matrix_id -and [string]$_.row_id -ceq [string]$certificationCase.row_id })[0]
$expectedCertPath = Assert-IdealCaptureCertificationCaseContract -Case $certificationCase -Environment $certEnvironment -FixtureRow $certFixtureRow -Contract $certContract -Owner "capture case test"
if ($expectedCertPath -cne "$($certContract.Root)/$($certContract.Name)") {
    throw "capture test: exact certification case path was not reproduced"
}
$positive++

Invoke-ExpectedCaptureFailure -Name "certification-case-artifact-binding" -Pattern "exact controlled environment-capture path" -Action {
    $mutatedCase = Copy-CaptureValue -Value $certificationCase
    @($mutatedCase.artifacts | Where-Object kind -ceq "environment-capture")[0].path = "artifacts/windows-x64/controlled-environments/v1/wrong/environment-capture.json"
    [void](Assert-IdealCaptureCertificationCaseContract -Case $mutatedCase -Environment $certEnvironment -FixtureRow $certFixtureRow -Contract $certContract -Owner "certification-case-artifact-binding")
}

Invoke-ExpectedCaptureFailure -Name "certification-case-fixture-binding" -Pattern "canonical fixture capture contract" -Action {
    $mutatedRow = Copy-CaptureValue -Value $certFixtureRow
    $mutatedRow.environment_capture_schema = "wrong-schema-v1"
    [void](Assert-IdealCaptureCertificationCaseContract -Case $certificationCase -Environment $certEnvironment -FixtureRow $mutatedRow -Contract $certContract -Owner "certification-case-fixture-binding")
}

Write-Host "test-capture-ideal-environment: ok (positive=$positive negative=$negative release=false residual_mutation=none owned_timeout_cleanup=proved)"
