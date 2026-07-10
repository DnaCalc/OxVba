param(
    [switch]$Check,
    [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
    [string]$OutputPath = "docs/validation/IDEAL_PROGRAM_DERIVED_SUMMARY_LATEST.md"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
. (Join-Path $PSScriptRoot "lib-ideal-program-validation.ps1")

function ConvertTo-MarkdownCell {
    param([AllowEmptyString()][string]$Value)

    return (($Value -replace '\|', '\|') -replace '\r?\n', ' ').Trim()
}

Push-Location $repoRoot
try {
    $manifestContext = Read-IdealProgramManifest -RepoRoot $repoRoot -ManifestPath $ManifestPath
    $manifest = $manifestContext.Manifest
    $ownershipAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.matrix_ownership)
    $ownershipRows = @(Import-Csv -LiteralPath $ownershipAbs)
    $traceAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.bead_traceability)
    $traceRows = @(Import-Csv -LiteralPath $traceAbs)

    $allRows = @()
    foreach ($owner in $ownershipRows) {
        $matrixAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$owner.path)
        foreach ($row in @(Import-Csv -LiteralPath $matrixAbs)) {
            $allRows += [pscustomobject]@{
                matrix_id = [string]$owner.matrix_id
                matrix_path = [string]$owner.path
                profile = [string]$row.profile
                row_id = [string]$row.row_id
                capability = [string]$row.capability
                semantic_subset = [string]$row.semantic_subset
                truth_state = [string]$row.truth_state
                residual_disposition = [string]$row.residual_disposition
                residual_owner_bead = [string]$row.residual_owner_bead
            }
        }
    }

    $summary = @()
    $summary += "# Ideal Program Derived Validation Summary"
    $summary += ""
    $summary += "Program: ``$($manifest.program_id)`` / ``$($manifest.root_bead)``"
    $summary += "Manifest: ``$($manifestContext.ManifestPath)``"
    $summary += "Ownership: ``$($manifest.matrix_ownership)``"
    $summary += ""
    $summary += "This file is generated from the manifest-owned canonical matrices. It is a projection, not an independent capability claim."
    $summary += ""
    $summary += "## Profile totals"
    $summary += ""
    $summary += "| Profile | Matrices | Rows | Planned | In progress | Implemented subset | Implemented full | Verified | Archived |"
    $summary += "|---|---:|---:|---:|---:|---:|---:|---:|---:|"
    foreach ($profile in @($manifest.profiles)) {
        $profileName = [string]$profile.profile
        $profileMatrices = @($ownershipRows | Where-Object { [string]$_.profile -eq $profileName })
        $profileRows = @($allRows | Where-Object { $_.profile -eq $profileName })
        $summary += "| $profileName | $($profileMatrices.Count) | $($profileRows.Count) | $(@($profileRows | Where-Object truth_state -eq 'planned').Count) | $(@($profileRows | Where-Object truth_state -eq 'in-progress').Count) | $(@($profileRows | Where-Object truth_state -eq 'implemented-subset').Count) | $(@($profileRows | Where-Object truth_state -eq 'implemented-full').Count) | $(@($profileRows | Where-Object truth_state -eq 'verified').Count) | $(@($profileRows | Where-Object truth_state -eq 'archived').Count) |"
    }

    $summary += ""
    $summary += "## Matrix totals"
    $summary += ""
    $summary += "| Matrix | Profile | Role | Owner epic | Rows | Verified | Open | Trace relationships |"
    $summary += "|---|---|---|---|---:|---:|---:|---:|"
    foreach ($owner in $ownershipRows) {
        $matrixId = [string]$owner.matrix_id
        $matrixRows = @($allRows | Where-Object { $_.matrix_id -eq $matrixId })
        $verified = @($matrixRows | Where-Object truth_state -eq "verified").Count
        $open = @($matrixRows | Where-Object truth_state -in @("planned", "in-progress", "implemented-subset", "implemented-full")).Count
        $relationships = @($traceRows | Where-Object { [string]$_.matrix_id -eq $matrixId }).Count
        $summary += "| $matrixId | $($owner.profile) | $($owner.role) | $($owner.owner_epic) | $($matrixRows.Count) | $verified | $open | $relationships |"
    }

    $summary += ""
    $summary += "## Remaining accepted scope"
    $summary += ""
    $openRows = @($allRows | Where-Object { $_.truth_state -ne "verified" -and $_.truth_state -ne "archived" })
    if ($openRows.Count -eq 0) {
        $summary += "No non-verified rows are currently seeded."
    }
    else {
        $summary += "| Row | Matrix | Capability | Subset | Truth state | Residual disposition | Residual owner |"
        $summary += "|---|---|---|---|---|---|---|"
        foreach ($row in $openRows) {
            $summary += "| $(ConvertTo-MarkdownCell $row.row_id) | $(ConvertTo-MarkdownCell $row.matrix_id) | $(ConvertTo-MarkdownCell $row.capability) | $(ConvertTo-MarkdownCell $row.semantic_subset) | $(ConvertTo-MarkdownCell $row.truth_state) | $(ConvertTo-MarkdownCell $row.residual_disposition) | $(ConvertTo-MarkdownCell $row.residual_owner_bead) |"
        }
    }

    $rendered = ($summary -join "`n").TrimEnd() + "`n"
    Assert-IdealRelativePath -Path $OutputPath -Owner "derived summary output"
    $outputAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $OutputPath
    if ($Check) {
        if (-not (Test-Path -LiteralPath $outputAbs -PathType Leaf)) {
            throw "generate-validation-derived-summaries: missing $OutputPath"
        }
        $existing = Get-Content -LiteralPath $outputAbs -Raw
        $normalizedExisting = ($existing -replace "`r`n", "`n").TrimEnd()
        $normalizedRendered = ($rendered -replace "`r`n", "`n").TrimEnd()
        if ($normalizedExisting -ne $normalizedRendered) {
            throw "generate-validation-derived-summaries: Ideal summary drift detected; regenerate $OutputPath"
        }
        Write-Host "generate-validation-derived-summaries: ok (program=$($manifest.program_id) checked=$OutputPath)"
    }
    else {
        $parent = Split-Path -Parent $outputAbs
        if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
            New-Item -ItemType Directory -Path $parent | Out-Null
        }
        Set-Content -LiteralPath $outputAbs -Value $rendered -NoNewline
        Write-Host "generate-validation-derived-summaries: wrote $OutputPath"
    }
}
finally {
    Pop-Location
}
