param(
    [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
    [string]$AutorunPath = "docs/AUTORUN_STATE.md",
    [string]$IssuesPath = ".beads/issues.jsonl",
    [string]$AgentsPath = "AGENTS.md"
)

# Compatibility entry point for historical automation. AgentsPath is retained
# so old callers fail neither parsing nor invocation; AGENTS.md no longer owns
# volatile ladder state.
$null = $AgentsPath
& (Join-Path $PSScriptRoot "validate-active-program-sync.ps1") `
    -ManifestPath $ManifestPath `
    -AutorunPath $AutorunPath `
    -IssuesPath $IssuesPath
