param(
    [ValidateSet("staged", "working")]
    [string]$Mode = "staged",
    [int[]]$AllowVersions = @(),
    [switch]$IncludeUntracked
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

function Parse-ActiveRange {
    param([string]$AgentsText)

    $match = [regex]::Match($AgentsText, '`\s*v(\d+)\.\.v(\d+)\s*`', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
    if (-not $match.Success) {
        throw "profile-artifact-scope: unable to parse active ladder range from AGENTS.md"
    }
    return @{
        From = [int]$match.Groups[1].Value
        To = [int]$match.Groups[2].Value
    }
}

function Expand-AllowedVersions {
    param([hashtable]$Range)

    $versions = @()
    for ($v = $Range.From; $v -le $Range.To; $v++) {
        $versions += $v
    }
    return $versions
}

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $gitTop = (& git rev-parse --show-toplevel 2>$null)
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($gitTop)) {
        throw "profile-artifact-scope: not inside a git worktree"
    }

    $allowed = @()
    if ($AllowVersions.Count -gt 0) {
        $allowed = @($AllowVersions | Sort-Object -Unique)
    }
    else {
        $autorunPath = "docs/AUTORUN_STATE.md"
        if (-not (Test-Path $autorunPath)) {
            throw "profile-artifact-scope: missing docs/AUTORUN_STATE.md"
        }
        $autorunText = Get-Content $autorunPath -Raw
        $modeMatch = [regex]::Match($autorunText, 'Mode:\s*([^\r\n]+)', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
        if (-not $modeMatch.Success) {
            throw "profile-artifact-scope: unable to parse mode from docs/AUTORUN_STATE.md"
        }
        $executionMode = $modeMatch.Groups[1].Value.Trim()
        if ($executionMode -ne "AutoRun") {
            Write-Host "profile-artifact-scope: inactive (mode=$executionMode)"
            return
        }

        $agentsPath = "AGENTS.md"
        if (-not (Test-Path $agentsPath)) {
            throw "profile-artifact-scope: missing AGENTS.md"
        }
        $agentsText = Get-Content $agentsPath -Raw
        $activeRange = Parse-ActiveRange -AgentsText $agentsText
        $allowed = Expand-AllowedVersions -Range $activeRange
    }

    $changed = @()
    if ($Mode -eq "staged") {
        $changed += @(git diff --cached --name-only --diff-filter=ACMR)
    }
    else {
        $changed += @(git diff --name-only --diff-filter=ACMR)
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
        $version = $null

        $mProfile = [regex]::Match($normalized, '^docs/evidence/profiles/v(\d+)/', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
        if ($mProfile.Success) {
            $version = [int]$mProfile.Groups[1].Value
        }
        else {
            $mStatus = [regex]::Match($normalized, '^docs/profile-status/PROFILE_STATUS_V(\d+)\.md$', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
            if ($mStatus.Success) {
                $version = [int]$mStatus.Groups[1].Value
            }
        }

        if ($null -eq $version) {
            continue
        }

        if ($allowed -notcontains $version) {
            $violations += [PSCustomObject]@{
                path = $normalized
                version = $version
            }
        }
    }

    if ($violations.Count -gt 0) {
        $allowedText = ($allowed | Sort-Object) -join ","
        $examples = $violations | Select-Object -First 20 | ForEach-Object { "$($_.path) (v$($_.version))" }
        $details = $examples -join "; "
        throw "profile-artifact-scope: changed profile artifacts outside allowed set [$allowedText]. examples: $details"
    }

    Write-Host "profile-artifact-scope: ok (allowed_versions=$($allowed -join ',') checked_files=$($changed.Count))"
}
finally {
    Pop-Location
}
