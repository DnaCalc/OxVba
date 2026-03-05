param(
    [Parameter(Mandatory = $true)]
    [int]$FromVersion,
    [Parameter(Mandatory = $true)]
    [int]$ToVersion,
    [Parameter(Mandatory = $true)]
    [string]$LadderPath,
    [Parameter(Mandatory = $true)]
    [string]$WorksetPath,
    [string]$StepTitlePrefix = "Profile Step",
    [string]$ScopeSummary = "TODO: fill step scope summary.",
    [switch]$Force,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ($FromVersion -gt $ToVersion) {
    throw "new-profile-slice: FromVersion must be <= ToVersion"
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$ladderAbs = Join-Path $repoRoot ($LadderPath.Replace('/', [IO.Path]::DirectorySeparatorChar))
$worksetAbs = Join-Path $repoRoot ($WorksetPath.Replace('/', [IO.Path]::DirectorySeparatorChar))
if (-not (Test-Path $ladderAbs)) {
    throw "new-profile-slice: ladder file missing: $LadderPath"
}
if (-not (Test-Path $worksetAbs)) {
    throw "new-profile-slice: workset file missing: $WorksetPath"
}

function Write-FileSafe([string]$Path, [string]$Content, [switch]$ForceWrite, [switch]$DryRunWrite) {
    if ((Test-Path $Path) -and -not $ForceWrite) {
        Write-Host "new-profile-slice: skip existing $Path"
        return
    }
    if ($DryRunWrite) {
        Write-Host "new-profile-slice: would write $Path"
        return
    }
    $dir = Split-Path -Parent $Path
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    Set-Content -Path $Path -Value $Content
    Write-Host "new-profile-slice: wrote $Path"
}

for ($v = $FromVersion; $v -le $ToVersion; $v++) {
    $evidenceDirRel = "docs/evidence/profiles/v$v"
    $evidenceFileName = "V${v}_$($StepTitlePrefix.ToUpperInvariant().Replace(' ', '_'))" -replace '[^A-Z0-9_]', '_'
    $evidencePathRel = "$evidenceDirRel/$evidenceFileName.md"
    $evidenceAbs = Join-Path $repoRoot ($evidencePathRel.Replace('/', [IO.Path]::DirectorySeparatorChar))

    $evidence = @"
# V$v $StepTitlePrefix

## Scope
- Ladder: ``$(($LadderPath -split '/')[ -1 ])``
- Step: `v$v`
- Workset: ``$(($WorksetPath -split '/')[ -1 ])``

## Step Outcome
- $ScopeSummary

## Primary Artifacts
- `$LadderPath`
- `$WorksetPath`

## Gate Signal
- Step `v$v` artifacts are published.
"@

    Write-FileSafe -Path $evidenceAbs -Content $evidence -ForceWrite:$Force -DryRunWrite:$DryRun

    $statusRel = "docs/profile-status/PROFILE_STATUS_V$v.md"
    $statusAbs = Join-Path $repoRoot ($statusRel.Replace('/', [IO.Path]::DirectorySeparatorChar))
    $status = @"
# PROFILE_STATUS_V$v.md

## Profile
- ID: mvp-profile-v$v
- Ladder step: v$v

## Scope Summary
- $ScopeSummary

## Gate Artifacts
- $LadderPath
- $WorksetPath
- $evidencePathRel

## Closure Signals
- Step `v$v` artifacts are published and aligned to the active ladder.
"@

    Write-FileSafe -Path $statusAbs -Content $status -ForceWrite:$Force -DryRunWrite:$DryRun
}

Write-Host "new-profile-slice: complete (v$FromVersion..v$ToVersion)"
