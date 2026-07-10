param(
    [ValidateSet("staged", "working")]
    [string]$Mode = "staged",
    [int[]]$AllowVersions = @(),
    [string[]]$AllowProgramIds = @(),
    [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
    [string]$AutorunPath = "docs/AUTORUN_STATE.md",
    [switch]$IncludeUntracked
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
. (Join-Path $PSScriptRoot "lib-ideal-program-validation.ps1")

Push-Location $repoRoot
try {
    $gitTop = (& git rev-parse --show-toplevel 2>$null)
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($gitTop)) {
        throw "profile-artifact-scope: not inside a git worktree"
    }

    $manifestContext = Read-IdealProgramManifest -RepoRoot $repoRoot -ManifestPath $ManifestPath
    $activeProgramId = [string]$manifestContext.Manifest.program_id
    $activeProfiles = @($manifestContext.Manifest.profiles | ForEach-Object { [string]$_.profile })
    $allowedVersions = @($AllowVersions | Sort-Object -Unique)
    $allowedProgramIds = @($AllowProgramIds | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Sort-Object -Unique)
    if ($allowedProgramIds.Count -eq 0) {
        $autorunAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $AutorunPath
        if (-not (Test-Path -LiteralPath $autorunAbs -PathType Leaf)) {
            throw "profile-artifact-scope: missing $AutorunPath"
        }
        $autorunText = Get-Content -LiteralPath $autorunAbs -Raw
        $modeMatch = [regex]::Match($autorunText, '(?im)^Mode:\s*([^\r\n]+)$')
        if (-not $modeMatch.Success) {
            throw "profile-artifact-scope: unable to parse mode from $AutorunPath"
        }
        $executionMode = $modeMatch.Groups[1].Value.Trim()
        if ($executionMode -notin @("Directed", "AutoRun")) {
            Write-Host "profile-artifact-scope: inactive (mode=$executionMode)"
            return
        }
        $allowedProgramIds = @($activeProgramId)
    }

    $changed = @()
    if ($Mode -eq "staged") {
        $changed += @(git diff --cached --name-only --no-renames --diff-filter=ACMRD)
    }
    else {
        $changed += @(git diff HEAD --name-only --no-renames --diff-filter=ACMRD)
    }
    if ($IncludeUntracked) {
        $changed += @(git ls-files --others --exclude-standard)
    }
    $changed = @($changed | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Sort-Object -Unique)
    if ($changed.Count -eq 0) {
        Write-Host "profile-artifact-scope: ok (no changed files)"
        return
    }

    $violations = @()
    foreach ($path in $changed) {
        $normalized = $path.Replace('\', '/')

        $legacyVersion = $null
        $legacyEvidence = [regex]::Match($normalized, '^docs/evidence/profiles/v(\d+)/', [Text.RegularExpressions.RegexOptions]::IgnoreCase)
        $legacyStatus = [regex]::Match($normalized, '^docs/profile-status/PROFILE_STATUS_V(\d+)\.md$', [Text.RegularExpressions.RegexOptions]::IgnoreCase)
        if ($legacyEvidence.Success) {
            $legacyVersion = [int]$legacyEvidence.Groups[1].Value
        }
        elseif ($legacyStatus.Success) {
            $legacyVersion = [int]$legacyStatus.Groups[1].Value
        }
        if ($null -ne $legacyVersion) {
            if ($allowedVersions -notcontains $legacyVersion) {
                $violations += [pscustomobject]@{
                    path = $normalized
                    reason = "legacy v$legacyVersion artifact is outside the explicit historical allow-list"
                }
            }
            continue
        }

        $programId = ""
        $profileName = ""
        $programEvidence = [regex]::Match($normalized, '^docs/evidence/programs/([^/]+)/([^/]+)/', [Text.RegularExpressions.RegexOptions]::IgnoreCase)
        $programStatus = [regex]::Match($normalized, '^docs/program-status/([^/]+)/([^/]+)/', [Text.RegularExpressions.RegexOptions]::IgnoreCase)
        if ($programEvidence.Success) {
            $programId = $programEvidence.Groups[1].Value
            $profileName = $programEvidence.Groups[2].Value
        }
        elseif ($programStatus.Success) {
            $programId = $programStatus.Groups[1].Value
            $profileName = $programStatus.Groups[2].Value
        }
        if (-not [string]::IsNullOrWhiteSpace($programId) -and $allowedProgramIds -notcontains $programId) {
            $violations += [pscustomobject]@{
                path = $normalized
                reason = "program artifact belongs to '$programId', allowed program(s): $($allowedProgramIds -join ',')"
            }
        }
        elseif ($programId -eq $activeProgramId -and $profileName -notin $activeProfiles) {
            $violations += [pscustomobject]@{
                path = $normalized
                reason = "active program artifact profile '$profileName' is not one of $($activeProfiles -join ',')"
            }
        }
        elseif ($normalized -match '^docs/(evidence/programs|program-status)/' -and [string]::IsNullOrWhiteSpace($programId)) {
            $violations += [pscustomobject]@{
                path = $normalized
                reason = "named program artifacts require <program-id>/<profile>/ path segments"
            }
        }
    }

    if ($violations.Count -gt 0) {
        $details = @($violations | Select-Object -First 20 | ForEach-Object { "$($_.path) ($($_.reason))" }) -join "; "
        throw "profile-artifact-scope: changed artifacts are outside the active named program scope. $details"
    }

    $historicalText = if ($allowedVersions.Count -gt 0) { $allowedVersions -join "," } else { "none" }
    Write-Host "profile-artifact-scope: ok (program_ids=$($allowedProgramIds -join ',') historical_versions=$historicalText checked_files=$($changed.Count))"
}
finally {
    Pop-Location
}
