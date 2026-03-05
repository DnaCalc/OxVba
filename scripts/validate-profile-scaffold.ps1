param(
    [int]$FromVersion = 0,
    [int]$ToVersion = 9999
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$errors = New-Object System.Collections.Generic.List[string]
$warnings = New-Object System.Collections.Generic.List[string]

function Add-Error([string]$Message) {
    $script:errors.Add($Message) | Out-Null
}

function Add-Warning([string]$Message) {
    $script:warnings.Add($Message) | Out-Null
}

function Extract-Version([string]$Name, [string]$Pattern) {
    $m = [regex]::Match($Name, $Pattern)
    if (-not $m.Success) {
        return $null
    }
    return [int]$m.Groups["v"].Value
}

function In-Range([int]$Version) {
    return $Version -ge $FromVersion -and $Version -le $ToVersion
}

function Resolve-RepoPath([string]$RelativePath) {
    $normalized = $RelativePath.Replace("/", [IO.Path]::DirectorySeparatorChar)
    return Join-Path $repoRoot $normalized
}

function Check-ArtifactPath([string]$Owner, [string]$ArtifactPath) {
    $abs = Resolve-RepoPath $ArtifactPath
    if (-not (Test-Path $abs)) {
        Add-Error("$Owner references missing artifact: $ArtifactPath")
    }
}

Write-Host "validate-profile-scaffold: scanning v$FromVersion..v$ToVersion"

# 1) File-name hygiene checks (global).
$badWorksetNames = Get-ChildItem (Join-Path $repoRoot "docs/worksets") -Filter "WORKSET_*__V*.md" -ErrorAction SilentlyContinue
foreach ($f in $badWorksetNames) {
    Add-Error("Malformed workset filename (double underscore before V): docs/worksets/$($f.Name)")
}

# 2) Workset checks.
$worksets = Get-ChildItem (Join-Path $repoRoot "docs/worksets") -Filter "WORKSET_*_V*.md" -ErrorAction SilentlyContinue
foreach ($f in $worksets) {
    $version = Extract-Version $f.Name "_V(?<v>\d+)\.md$"
    if ($null -eq $version -or -not (In-Range $version)) {
        continue
    }
    $lines = Get-Content $f.FullName
    if ($lines.Count -lt 8) {
        Add-Error("Workset appears truncated (too few lines): docs/worksets/$($f.Name)")
    }
}

# 3) Profile status checks.
$statusFiles = Get-ChildItem (Join-Path $repoRoot "docs/profile-status") -Filter "PROFILE_STATUS_V*.md" -ErrorAction SilentlyContinue
foreach ($f in $statusFiles) {
    $version = Extract-Version $f.Name "_V(?<v>\d+)\.md$"
    if ($null -eq $version -or -not (In-Range $version)) {
        continue
    }

    $lines = Get-Content $f.FullName
    if ($lines.Count -lt 8) {
        Add-Error("Profile status appears truncated (too few lines): docs/profile-status/$($f.Name)")
        continue
    }

    $artifactLines = $lines | Where-Object { $_ -match '^- docs/' }
    if ($artifactLines.Count -eq 0) {
        Add-Warning("Profile status has no listed docs/* gate artifacts: docs/profile-status/$($f.Name)")
    }
    foreach ($line in $artifactLines) {
        $artifact = ($line -replace '^- ', '').Trim()
        if ($artifact -match "WORKSET_\d{4}-\d{2}-\d{2}__V\d+") {
            Add-Error("Profile status references malformed workset name: $artifact")
        }
        Check-ArtifactPath "docs/profile-status/$($f.Name)" $artifact
    }
}

# 4) Integrated gate checks.
$gateFiles = Get-ChildItem (Join-Path $repoRoot "docs/evidence/profiles") -Recurse -Filter "integrated_gate.md" -ErrorAction SilentlyContinue
foreach ($f in $gateFiles) {
    $version = Extract-Version $f.FullName.Replace("\", "/"), "/v(?<v>\d+)/integrated_gate\.md$"
    if ($null -eq $version -or -not (In-Range $version)) {
        continue
    }

    $lines = Get-Content $f.FullName
    if ($lines.Count -lt 6) {
        Add-Error("Integrated gate appears truncated (too few lines): $($f.FullName.Replace($repoRoot, '.'))")
        continue
    }

    $artifactLines = $lines | Where-Object { $_ -match '^  - docs/' }
    if ($artifactLines.Count -eq 0) {
        Add-Warning("Integrated gate has no artifact bullets: $($f.FullName.Replace($repoRoot, '.'))")
    }
    foreach ($line in $artifactLines) {
        $artifact = ($line -replace '^  - ', '').Trim()
        Check-ArtifactPath $f.FullName $artifact
    }

    $gateDir = Split-Path -Parent $f.FullName
    $gateJson = Join-Path $gateDir "gate.json"
    if (-not (Test-Path $gateJson)) {
        Add-Warning("Integrated gate is missing machine-readable manifest: $($gateJson.Replace($repoRoot, '.'))")
    }
}

if ($warnings.Count -gt 0) {
    Write-Host ""
    Write-Host "Warnings:"
    foreach ($w in $warnings) {
        Write-Host "- $w"
    }
}

if ($errors.Count -gt 0) {
    Write-Host ""
    Write-Host "Errors:"
    foreach ($e in $errors) {
        Write-Host "- $e"
    }
    Write-Host ""
    Write-Host "validate-profile-scaffold: FAIL"
    exit 1
}

Write-Host "validate-profile-scaffold: PASS"
