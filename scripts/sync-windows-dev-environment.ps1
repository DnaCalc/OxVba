param(
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

$environmentId = "win-x64-dev-oracle-2026-07"
$environmentManifestRelative = "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv"
$sourceRelative = "docs/evidence/programs/ideal-2026-07/windows-x64/WIN-0/dev-oracle-environment.json"
$controlledRootRelative = "artifacts/windows-x64/controlled-environments/v1/$environmentId"
$controlledRelative = "$controlledRootRelative/environment-capture.json"

function Assert-NoDevelopmentEnvironmentReparsePath {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$RelativePath
    )

    $rootFull = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    $candidateFull = [IO.Path]::GetFullPath((Join-Path $rootFull $RelativePath))
    if (-not (Test-WindowsFixturePathWithin -Candidate $candidateFull -Root $rootFull)) {
        throw "sync-windows-dev-environment: publication path escapes the repository"
    }
    $cursor = $rootFull
    foreach ($segment in @([IO.Path]::GetRelativePath($rootFull, $candidateFull) -split '[\\/]')) {
        $cursor = Join-Path $cursor $segment
        if (-not (Test-Path -LiteralPath $cursor)) {
            continue
        }
        $item = Get-Item -LiteralPath $cursor -Force
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "sync-windows-dev-environment: publication path crosses a reparse point"
        }
    }
}

Push-Location $repoRoot
try {
    foreach ($relative in @($environmentManifestRelative, $sourceRelative, $controlledRelative)) {
        Assert-IdealRelativePath -Path $relative -Owner "development environment publication path"
    }

    $environmentRows = @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $environmentManifestRelative))
    $matches = @($environmentRows | Where-Object { [string]$_.environment_id -eq $environmentId })
    if ($matches.Count -ne 1) {
        throw "sync-windows-dev-environment: expected exactly one '$environmentId' manifest row"
    }
    $environment = $matches[0]
    if ([string]$environment.environment_id -cne $environmentId -or
        [string]$environment.role -cne "dev-oracle" -or
        [string]$environment.evidence_state -cne "characterized-noncertifying" -or
        [string]$environment.owner_bead -cne "bd-59co.3.1.2") {
        throw "sync-windows-dev-environment: canonical development environment authority drifted"
    }

    $sourcePath = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $sourceRelative
    if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
        throw "sync-windows-dev-environment: accepted source capture is missing"
    }
    $sourceBytes = [IO.File]::ReadAllBytes($sourcePath)
    $capture = ConvertFrom-WindowsFixtureAuditedJson `
        -Bytes $sourceBytes `
        -Owner "accepted development environment capture" `
        -FormatName "environment-capture"
    Assert-WindowsFixtureEnvironmentCaptureValue `
        -Capture $capture `
        -Environment $environment `
        -ExpectedSchema "oxvba-windows-x64-environment-capture-v1" `
        -Owner "accepted development environment capture"
    $sourceHash = Get-WindowsFixtureCanonicalSourceFileHash -RepositoryRoot $repoRoot -RelativePath $sourceRelative
    $acceptedHash = "sha256:6616a1302f787f77f1acf022315a92f428f425279ef46d5752666c8ff3e1edf1"
    if ($sourceHash -cne $acceptedHash) {
        throw "sync-windows-dev-environment: accepted source capture hash drifted"
    }

    $controlledPath = Join-Path $repoRoot ($controlledRelative.Replace('/', [IO.Path]::DirectorySeparatorChar))
    Assert-NoDevelopmentEnvironmentReparsePath -Root $repoRoot -RelativePath $controlledRelative
    $controlledExists = Test-Path -LiteralPath $controlledPath -PathType Leaf
    if ($controlledExists) {
        $resolvedControlled = Assert-WindowsFixtureContainedPath `
            -RepositoryRoot $repoRoot `
            -RelativePath $controlledRelative `
            -ControlledRoot $controlledRootRelative `
            -Owner "controlled development environment publication"
        $controlledBytes = [IO.File]::ReadAllBytes($resolvedControlled)
        if ([Convert]::ToBase64String($sourceBytes) -cne [Convert]::ToBase64String($controlledBytes)) {
            throw "sync-windows-dev-environment: controlled publication differs byte-for-byte from the accepted capture; immutable publication was not replaced"
        }
    }
    elseif ($Check) {
            throw "sync-windows-dev-environment: controlled publication is missing at '$controlledRelative'"
    }
    else {
        $controlledParent = Split-Path -Parent $controlledPath
        if (-not (Test-Path -LiteralPath $controlledParent -PathType Container)) {
            [void](New-Item -ItemType Directory -Path $controlledParent -Force)
        }
        Assert-NoDevelopmentEnvironmentReparsePath -Root $repoRoot -RelativePath $controlledRelative
        $stream = [IO.FileStream]::new(
            $controlledPath,
            [IO.FileMode]::CreateNew,
            [IO.FileAccess]::Write,
            [IO.FileShare]::None,
            4096,
            [IO.FileOptions]::WriteThrough
        )
        try {
            $stream.Write($sourceBytes, 0, $sourceBytes.Length)
            $stream.Flush($true)
        }
        finally {
            $stream.Dispose()
        }
    }

    Assert-WindowsFixtureEnvironmentCapture `
        -RepositoryRoot $repoRoot `
        -RelativePath $controlledRelative `
        -CaptureRoot $controlledRootRelative `
        -Environment $environment `
        -ExpectedSchema "oxvba-windows-x64-environment-capture-v1" `
        -Owner "controlled development environment publication"
    $hash = Get-WindowsFixtureCanonicalSourceFileHash -RepositoryRoot $repoRoot -RelativePath $controlledRelative
    if ($hash -cne $acceptedHash) {
        throw "sync-windows-dev-environment: controlled publication hash differs from the accepted immutable capture"
    }

    $mode = if ($Check) { "check" } else { "write" }
    Write-Host "sync-windows-dev-environment: ok (mode=$mode environment=$environmentId authority=noncertifying hash=$hash)"
}
finally {
    Pop-Location
}
