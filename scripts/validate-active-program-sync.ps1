param(
    [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
    [string]$AutorunPath = "docs/AUTORUN_STATE.md",
    [string]$IssuesPath = ".beads/issues.jsonl"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
. (Join-Path $PSScriptRoot "lib-ideal-program-validation.ps1")

function Get-ControlField {
    param(
        [Parameter(Mandatory = $true)][string]$Text,
        [Parameter(Mandatory = $true)][string]$Name,
        [switch]$Required
    )

    $match = [regex]::Match(
        $Text,
        "(?im)^$([regex]::Escape($Name)):[ \t]*([^\r\n]+)\r?$"
    )
    if (-not $match.Success) {
        if ($Required) {
            throw "active-program-sync: missing '$Name' in docs/AUTORUN_STATE.md"
        }
        return ""
    }
    return $match.Groups[1].Value.Trim().Trim('`')
}

Push-Location $repoRoot
try {
    $manifestContext = Read-IdealProgramManifest -RepoRoot $repoRoot -ManifestPath $ManifestPath
    $manifest = $manifestContext.Manifest
    $issueContext = Read-IdealIssues -RepoRoot $repoRoot -IssuesPath $IssuesPath
    $issueById = $issueContext.IssueById

    $autorunAbs = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $AutorunPath
    if (-not (Test-Path -LiteralPath $autorunAbs -PathType Leaf)) {
        throw "active-program-sync: missing $AutorunPath"
    }
    $state = Get-Content -LiteralPath $autorunAbs -Raw
    $mode = Get-ControlField -Text $state -Name "Mode" -Required
    if ($mode -notin @("Directed", "AutoRun")) {
        throw "active-program-sync: mode must be Directed or AutoRun for the accepted Ideal program, found '$mode'"
    }

    if ($state -match '(?im)^\s*(Active ladder|Ladder span|Current required terminal gate)\s*:|^\s*(AutoRun )?Terminal gate\s*:\s*`?v\d+') {
        throw "active-program-sync: active state still contains a legacy vNNN ladder/gate field"
    }

    foreach ($id in @([string]$manifest.root_bead, [string]$manifest.control_epic) + @($manifest.profiles | ForEach-Object { [string]$_.workset_root })) {
        if (-not $issueById.ContainsKey($id)) {
            throw "active-program-sync: manifest bead '$id' does not exist"
        }
    }
    foreach ($profile in @($manifest.profiles)) {
        $worksetDoc = ([string]$profile.workset_doc).Replace('\', '/')
        Assert-IdealRelativePath -Path $worksetDoc -Owner "profile '$($profile.profile)' workset_doc"
        if (-not (Test-Path -LiteralPath (Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $worksetDoc) -PathType Leaf)) {
            throw "active-program-sync: missing accepted workset '$worksetDoc'"
        }
        if (-not $state.Contains($worksetDoc)) {
            throw "active-program-sync: $AutorunPath does not list accepted workset '$worksetDoc'"
        }
    }

    if (-not $state.Contains([string]$manifest.root_bead)) {
        throw "active-program-sync: $AutorunPath does not identify program root '$($manifest.root_bead)'"
    }
    if (-not $state.Contains([string]$manifest.control_epic)) {
        throw "active-program-sync: $AutorunPath does not identify control epic '$($manifest.control_epic)'"
    }

    $declaredManifest = Get-ControlField -Text $state -Name "Active program manifest"
    $declaredRoot = Get-ControlField -Text $state -Name "Program root"
    $terminalGate = Get-ControlField -Text $state -Name "AutoRun terminal gate"
    $queueCertification = Get-ControlField -Text $state -Name "Queue certification"

    if (-not [string]::IsNullOrWhiteSpace($declaredManifest) -and
        $declaredManifest.Replace('\', '/') -ne $manifestContext.ManifestPath) {
        throw "active-program-sync: declared manifest '$declaredManifest' does not match '$($manifestContext.ManifestPath)'"
    }
    if (-not [string]::IsNullOrWhiteSpace($declaredRoot) -and $declaredRoot -ne [string]$manifest.root_bead) {
        throw "active-program-sync: declared program root '$declaredRoot' does not match '$($manifest.root_bead)'"
    }

    if ($mode -eq "Directed") {
        if ($terminalGate -notin @("", "inactive", [string]$manifest.root_bead)) {
            throw "active-program-sync: directed rollout terminal gate must be inactive or '$($manifest.root_bead)', found '$terminalGate'"
        }
    }
    else {
        if ([string]::IsNullOrWhiteSpace($declaredManifest)) {
            throw "active-program-sync: AutoRun requires 'Active program manifest: $($manifestContext.ManifestPath)'"
        }
        if ([string]::IsNullOrWhiteSpace($declaredRoot)) {
            throw "active-program-sync: AutoRun requires 'Program root: $($manifest.root_bead)'"
        }
        if ($terminalGate -ne [string]$manifest.root_bead) {
            throw "active-program-sync: AutoRun terminal gate must be '$($manifest.root_bead)', found '$terminalGate'"
        }
        if ([string]::IsNullOrWhiteSpace($queueCertification) -or $queueCertification -notmatch '(?i)^(passed|current-only|certified)\b') {
            throw "active-program-sync: AutoRun requires a passed/current-only/certified Queue certification field"
        }
        if ([string]$issueById[[string]$manifest.root_bead].status -eq "closed") {
            throw "active-program-sync: AutoRun cannot remain active after program root '$($manifest.root_bead)' is closed"
        }
    }

    Write-Host "active-program-sync: ok (mode=$mode program=$($manifest.program_id) root=$($manifest.root_bead) profiles=$(@($manifest.profiles).Count))"
}
finally {
    Pop-Location
}
