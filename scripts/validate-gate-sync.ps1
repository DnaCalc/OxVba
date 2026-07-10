param(
    [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
    [string]$AutorunPath = "docs/AUTORUN_STATE.md",
    [string]$IssuesPath = ".beads/issues.jsonl"
)

# Compatibility entry point for historical automation. Current execution is
# program/manifest based; vNNN terminal-gate synchronization is retired.
& (Join-Path $PSScriptRoot "validate-active-program-sync.ps1") `
    -ManifestPath $ManifestPath `
    -AutorunPath $AutorunPath `
    -IssuesPath $IssuesPath
