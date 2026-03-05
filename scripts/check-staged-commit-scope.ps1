param(
    [switch]$AllowMixed,
    [switch]$IncludeUntracked
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

function Get-StagedFiles {
    $files = @()
    $files += @(git diff --cached --name-only --diff-filter=ACMR)
    if ($IncludeUntracked) {
        $files += @(git ls-files --others --exclude-standard)
    }
    return @($files | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Sort-Object -Unique)
}

function Is-EvidenceFile {
    param([string]$Path)
    $p = $Path.Replace('\', '/')
    if ($p -like "docs/evidence/*") {
        return $true
    }
    if ($p -like "docs/profile-status/*") {
        return $true
    }
    return $false
}

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $gitTop = (& git rev-parse --show-toplevel 2>$null)
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($gitTop)) {
        throw "check-staged-commit-scope: not inside a git worktree"
    }

    $staged = Get-StagedFiles
    if ($staged.Count -eq 0) {
        Write-Host "check-staged-commit-scope: ok (no staged files)"
        return
    }

    $evidence = @()
    $nonEvidence = @()
    foreach ($path in $staged) {
        if (Is-EvidenceFile -Path $path) {
            $evidence += $path
        }
        else {
            $nonEvidence += $path
        }
    }

    if (-not $AllowMixed -and $evidence.Count -gt 0 -and $nonEvidence.Count -gt 0) {
        $e = ($evidence | Select-Object -First 10) -join "; "
        $n = ($nonEvidence | Select-Object -First 10) -join "; "
        throw "check-staged-commit-scope: mixed staged set detected (evidence + code/spec). split commits. evidence examples: $e ; non-evidence examples: $n"
    }

    Write-Host "check-staged-commit-scope: ok (staged=$($staged.Count) evidence=$($evidence.Count) non_evidence=$($nonEvidence.Count))"
}
finally {
    Pop-Location
}
