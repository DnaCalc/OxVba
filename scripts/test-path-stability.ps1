$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

function Invoke-InDirectory {
    param(
        [string]$Path,
        [scriptblock]$Script
    )

    Push-Location $Path
    try {
        & $Script
    }
    finally {
        Pop-Location
    }
}

Invoke-InDirectory -Path $PSScriptRoot -Script {
    & "./run-smoke.ps1" | Out-Null
}

Invoke-InDirectory -Path (Join-Path $repoRoot "crates/oxvba-host") -Script {
    & (Join-Path $repoRoot "scripts/run-smoke.ps1") | Out-Null
}

Invoke-InDirectory -Path (Join-Path $repoRoot "crates/oxvba-host") -Script {
    cargo test -q -p oxvba-host formal_v17_formal_manifest_has_active_entries | Out-Null
    cargo test -q -p oxvba-host formal_v18_divergence_index_is_present | Out-Null
}

Invoke-InDirectory -Path (Join-Path $repoRoot "crates/oxvba-host") -Script {
    & (Join-Path $repoRoot "scripts/validate-active-program-sync.ps1") | Out-Null
    & (Join-Path $repoRoot "scripts/validate-validation-ownership.ps1") | Out-Null
    & (Join-Path $repoRoot "scripts/validate-ideal-legacy-migration.ps1") | Out-Null
    & (Join-Path $repoRoot "scripts/validate-closure-taxonomy.ps1") | Out-Null
    & (Join-Path $repoRoot "scripts/validate-bead-traceability.ps1") | Out-Null
    & (Join-Path $repoRoot "scripts/validate-workset-rollout.ps1") -SkipReadyQueue | Out-Null
}

Write-Host "path-stability: ok"
