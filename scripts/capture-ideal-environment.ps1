param(
    [string]$EnvironmentId = "",
    [string]$CaseId = "",
    [string]$OutputPath = "",
    [string]$ReportPath = "",
    [switch]$Check,
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

function Resolve-CaptureOutputPath {
    param(
        [Parameter(Mandatory = $true)][string]$RelativePath,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    Assert-IdealRelativePath -Path $RelativePath -Owner $Owner
    $absolute = [IO.Path]::GetFullPath((Join-Path $repoRoot $RelativePath))
    $root = [IO.Path]::GetFullPath($repoRoot).TrimEnd('\', '/')
    if (-not $absolute.StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "capture-ideal-environment: $Owner escapes the repository"
    }
    $cursor = $root
    foreach ($segment in @([IO.Path]::GetRelativePath($root, $absolute) -split '[\\/]')) {
        $cursor = Join-Path $cursor $segment
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "capture-ideal-environment: $Owner crosses a reparse point"
            }
        }
    }
    return $absolute
}

if ([string]::IsNullOrWhiteSpace($EnvironmentId) -eq [string]::IsNullOrWhiteSpace($CaseId)) {
    throw "capture-ideal-environment: specify exactly one of -EnvironmentId or -CaseId"
}

$case = $null
if (-not [string]::IsNullOrWhiteSpace($CaseId)) {
    $casePath = Join-Path $repoRoot "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-14/certification-cases.json"
    $caseDocument = ConvertFrom-WindowsFixtureAuditedJson `
        -Bytes ([IO.File]::ReadAllBytes($casePath)) `
        -Owner "certification case manifest" `
        -FormatName "certification-case"
    $cases = @($caseDocument.cases | Where-Object case_id -ceq $CaseId)
    if ($cases.Count -ne 1) {
        throw "capture-ideal-environment: expected one certification case '$CaseId', found $($cases.Count)"
    }
    $case = $cases[0]
    $EnvironmentId = [string]$case.environment_gate.environment_id
}

$environment = Get-IdealCaptureEnvironmentRow -RepositoryRoot $repoRoot -EnvironmentId $EnvironmentId
if ($null -ne $case) {
    $contract = Get-WindowsFixtureEnvironmentCaptureContract -Environment $environment
    $fixtureRows = @(
        Import-Csv -LiteralPath (Join-Path $repoRoot $script:IdealWindowsFixtureManifestPath) |
            Where-Object { [string]$_.matrix_id -ceq [string]$case.matrix_id -and [string]$_.row_id -ceq [string]$case.row_id }
    )
    if ($fixtureRows.Count -ne 1) {
        throw "capture-ideal-environment: certification case '$CaseId' does not resolve one canonical fixture row"
    }
    $expectedOutputPath = Assert-IdealCaptureCertificationCaseContract `
        -Case $case `
        -Environment $environment `
        -FixtureRow $fixtureRows[0] `
        -Contract $contract `
        -Owner "certification case '$CaseId'"
    if (-not [string]::IsNullOrWhiteSpace($OutputPath) -and $OutputPath.Replace('\', '/') -cne $expectedOutputPath) {
        throw "capture-ideal-environment: -CaseId output must be the exact controlled environment-capture path '$expectedOutputPath'"
    }
    if (-not [string]::IsNullOrWhiteSpace($ReportPath)) {
        throw "capture-ideal-environment: certification reports require the later trusted-attestation producer"
    }
    $OutputPath = $expectedOutputPath
}
else {
    $expectedOutputPath = "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-0/dev-oracle-environment.json"
    $expectedReportPath = "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-0/dev-oracle-environment.md"
    if ([string]$environment.role -ne "dev-oracle") {
        throw "capture-ideal-environment: certification environments must be selected through an exact -CaseId"
    }
    if (-not [string]::IsNullOrWhiteSpace($OutputPath) -and $OutputPath.Replace('\', '/') -cne $expectedOutputPath) {
        throw "capture-ideal-environment: dev capture output must remain '$expectedOutputPath'"
    }
    if (-not [string]::IsNullOrWhiteSpace($ReportPath) -and $ReportPath.Replace('\', '/') -cne $expectedReportPath) {
        throw "capture-ideal-environment: dev capture report must remain '$expectedReportPath'"
    }
    $OutputPath = $expectedOutputPath
    $ReportPath = $expectedReportPath
}

$outputAbsolute = Resolve-CaptureOutputPath -RelativePath $OutputPath -Owner "capture output"
$reportAbsolute = if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    ""
}
else {
    Resolve-CaptureOutputPath -RelativePath $ReportPath -Owner "capture report"
}
if (-not [string]::IsNullOrWhiteSpace($reportAbsolute) -and `
    $outputAbsolute.Equals($reportAbsolute, [StringComparison]::OrdinalIgnoreCase)) {
    throw "capture-ideal-environment: capture output and report must be distinct paths"
}
if ([string]$environment.role -eq "certification-vm") {
    throw "capture-ideal-environment: certification capture is disabled until a trusted pinned-image restore/session attestation is implemented"
}

$excelBefore = @(Get-IdealCaptureExcelProcessIds)
if ($excelBefore.Count -ne 0) {
    throw "capture-ideal-environment: Excel must be closed; refusing to observe or clean unowned PIDs '$($excelBefore -join '|')'"
}
$registryBefore = Get-IdealCaptureRegistryValues
$registryHashBefore = Get-IdealCaptureObjectHash -Value $registryBefore
$observation = Get-IdealCaptureHostObservation -RegistryValues $registryBefore
$fixtureFacts = Get-IdealCaptureFixtureFacts -RepositoryRoot $repoRoot -EnvironmentId $EnvironmentId
$registryAfter = Get-IdealCaptureRegistryValues
$registryHashAfter = Get-IdealCaptureObjectHash -Value $registryAfter
$excelAfter = @(Get-IdealCaptureExcelProcessIds)
if ($registryHashBefore -cne $registryHashAfter) {
    throw "capture-ideal-environment: selected registry facts changed during the read-only capture"
}
if ($excelAfter.Count -ne 0) {
    throw "capture-ideal-environment: Excel PID balance changed during capture; refusing cleanup of '$($excelAfter -join '|')'"
}

Assert-IdealCaptureObservedEnvironment -Environment $environment -Observation $observation -FixtureFacts $fixtureFacts
$capture = New-IdealEnvironmentCaptureValue -Environment $environment
Assert-WindowsFixtureEnvironmentCaptureValue `
    -Capture $capture `
    -Environment $environment `
    -ExpectedSchema $script:IdealEnvironmentCaptureSchema `
    -Owner "capture-ideal-environment"
$json = (($capture | ConvertTo-Json -Depth 8).Replace("`r`n", "`n").Replace("`r", "`n")) + "`n"
$captureHash = Get-IdealCaptureSha256Text -Text $json
$report = if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    ""
}
else {
    New-IdealEnvironmentCaptureMarkdown `
        -Environment $environment `
        -Observation $observation `
        -FixtureFacts $fixtureFacts `
        -Capture $capture `
        -CaptureHash $captureHash `
        -RegistryHash $registryHashBefore `
        -OutputPath $OutputPath
}

if ($Check) {
    if (-not (Test-Path -LiteralPath $outputAbsolute -PathType Leaf) -or
        [IO.File]::ReadAllText($outputAbsolute).Replace("`r`n", "`n").Replace("`r", "`n") -cne $json) {
        throw "capture-ideal-environment: capture output is stale: $OutputPath"
    }
    if (-not [string]::IsNullOrWhiteSpace($ReportPath) -and
        (-not (Test-Path -LiteralPath $reportAbsolute -PathType Leaf) -or
            [IO.File]::ReadAllText($reportAbsolute).Replace("`r`n", "`n").Replace("`r", "`n") -cne $report)) {
        throw "capture-ideal-environment: capture report is stale: $ReportPath"
    }
}
else {
    $writeCapture = -not (Test-Path -LiteralPath $outputAbsolute -PathType Leaf)
    if (-not $writeCapture -and
        [IO.File]::ReadAllText($outputAbsolute).Replace("`r`n", "`n").Replace("`r", "`n") -cne $json) {
        throw "capture-ideal-environment: refusing to replace immutable capture '$OutputPath'; use a new environment identity"
    }
    $writeReport = -not [string]::IsNullOrWhiteSpace($ReportPath) -and -not (Test-Path -LiteralPath $reportAbsolute -PathType Leaf)
    if (-not [string]::IsNullOrWhiteSpace($ReportPath) -and -not $writeReport -and
        [IO.File]::ReadAllText($reportAbsolute).Replace("`r`n", "`n").Replace("`r", "`n") -cne $report) {
        throw "capture-ideal-environment: refusing to replace immutable report '$ReportPath'; use a new environment identity"
    }
    if ($writeCapture) {
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($outputAbsolute)) | Out-Null
        [IO.File]::WriteAllText($outputAbsolute, $json, [Text.UTF8Encoding]::new($false))
    }
    if ($writeReport) {
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($reportAbsolute)) | Out-Null
        [IO.File]::WriteAllText($reportAbsolute, $report, [Text.UTF8Encoding]::new($false))
    }
}

Write-Host "capture-ideal-environment: ok (environment=$EnvironmentId release=$([string]$environment.role -eq 'certification-vm') hash=$captureHash check=$([bool]$Check))"
