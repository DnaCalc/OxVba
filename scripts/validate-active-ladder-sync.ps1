param(
    [string]$AgentsPath = "AGENTS.md",
    [string]$AutorunPath = "docs/AUTORUN_STATE.md"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$agentsAbs = Join-Path $repoRoot $AgentsPath
$autorunAbs = Join-Path $repoRoot $AutorunPath

if (-not (Test-Path $agentsAbs)) {
    throw "active-ladder-sync: missing AGENTS file: $AgentsPath"
}
if (-not (Test-Path $autorunAbs)) {
    throw "active-ladder-sync: missing AutoRun file: $AutorunPath"
}

$agents = Get-Content $agentsAbs -Raw
$autorun = Get-Content $autorunAbs -Raw

function Parse-Gate([string]$Text, [string]$Pattern, [string]$Owner) {
    $m = [regex]::Match($Text, $Pattern, [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
    if (-not $m.Success) {
        throw "active-ladder-sync: unable to parse gate from $Owner"
    }
    return $m.Groups[1].Value
}

function Parse-ActiveLadder([string]$Text, [string]$Owner) {
    $m = [regex]::Match(
        $Text,
        '-\s*`(?<range>v\d+\.\.v\d+)`\s*\(`(?<path>docs/worksets/[^`]+)`\)',
        [System.Text.RegularExpressions.RegexOptions]::IgnoreCase
    )
    if (-not $m.Success) {
        throw "active-ladder-sync: unable to parse active ladder from $Owner"
    }
    return [pscustomobject]@{
        range = $m.Groups['range'].Value
        path = $m.Groups['path'].Value
    }
}

function Parse-LadderApprovedGate([string]$LadderText) {
    $patterns = @(
        'current approved run[^`]*`(v\d+)`',
        'immediate terminal gate for approved run:\s*`?(v\d+)`?',
        'terminal gate for executed run:\s*`?(v\d+)`?',
        '^\s*-\s*Terminal gate:\s*`?(v\d+)`?'
    )
    foreach ($p in $patterns) {
        $m = [regex]::Match($LadderText, $p, [System.Text.RegularExpressions.RegexOptions]::IgnoreCase -bor [System.Text.RegularExpressions.RegexOptions]::Multiline)
        if ($m.Success) {
            return $m.Groups[1].Value
        }
    }
    throw "active-ladder-sync: unable to parse approved terminal gate from ladder"
}

function Parse-RangeParts([string]$RangeText, [string]$Owner) {
    $m = [regex]::Match($RangeText, '^v(?<from>\d+)\.\.v(?<to>\d+)$', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
    if (-not $m.Success) {
        throw "active-ladder-sync: malformed range '$RangeText' in $Owner"
    }
    return [pscustomobject]@{
        from = [int]$m.Groups['from'].Value
        to = [int]$m.Groups['to'].Value
    }
}

$agentsGate = Parse-Gate -Text $agents -Pattern 'Current required terminal gate:\s*`?(v\d+)`?' -Owner 'AGENTS.md'
$autorunGate = Parse-Gate -Text $autorun -Pattern 'Terminal gate:\s*`?(v\d+)`?' -Owner 'docs/AUTORUN_STATE.md'
if ($agentsGate -ne $autorunGate) {
    throw "active-ladder-sync mismatch: AGENTS=$agentsGate AUTORUN=$autorunGate"
}

$agentsLadder = Parse-ActiveLadder -Text $agents -Owner 'AGENTS.md'
$autorunLadder = Parse-ActiveLadder -Text $autorun -Owner 'docs/AUTORUN_STATE.md'
if ($agentsLadder.range -ne $autorunLadder.range) {
    throw "active-ladder-sync mismatch: AGENTS range=$($agentsLadder.range) AUTORUN range=$($autorunLadder.range)"
}
if ($agentsLadder.path -ne $autorunLadder.path) {
    throw "active-ladder-sync mismatch: AGENTS path=$($agentsLadder.path) AUTORUN path=$($autorunLadder.path)"
}

$ladderPath = Join-Path $repoRoot ($agentsLadder.path.Replace('/', [IO.Path]::DirectorySeparatorChar))
if (-not (Test-Path $ladderPath)) {
    throw "active-ladder-sync: active ladder file missing: $($agentsLadder.path)"
}

$ladderText = Get-Content $ladderPath -Raw
$ladderSpanMatch = [regex]::Match($ladderText, 'Ladder span:\s*`?(v\d+\.\.v\d+)`?', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
if (-not $ladderSpanMatch.Success) {
    throw "active-ladder-sync: unable to parse 'Ladder span' from $($agentsLadder.path)"
}
$ladderSpan = $ladderSpanMatch.Groups[1].Value
if ($ladderSpan -ne $agentsLadder.range) {
    throw "active-ladder-sync mismatch: active range=$($agentsLadder.range) ladder span=$ladderSpan"
}

$ladderApprovedGate = Parse-LadderApprovedGate -LadderText $ladderText
if ($ladderApprovedGate -ne $agentsGate) {
    throw "active-ladder-sync mismatch: approved ladder gate=$ladderApprovedGate expected=$agentsGate"
}

$rangeParts = Parse-RangeParts -RangeText $agentsLadder.range -Owner 'active ladder'
$gateInt = [int]([regex]::Match($agentsGate, '^v(\d+)$', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase).Groups[1].Value)
if ($gateInt -lt $rangeParts.from -or $gateInt -gt $rangeParts.to) {
    throw "active-ladder-sync: gate $agentsGate is outside active range $($agentsLadder.range)"
}

$worksetPattern = "*_V$($rangeParts.from)_V$gateInt*.md"
$worksetMatches = @(Get-ChildItem (Join-Path $repoRoot 'docs/worksets') -Filter $worksetPattern -ErrorAction SilentlyContinue)
if ($worksetMatches.Count -eq 0) {
    throw "active-ladder-sync: expected at least one workset matching docs/worksets/$worksetPattern"
}

$statusPath = Join-Path $repoRoot "docs/profile-status/PROFILE_STATUS_V$gateInt.md"
if (-not (Test-Path $statusPath)) {
    throw "active-ladder-sync: missing terminal profile status file docs/profile-status/PROFILE_STATUS_V$gateInt.md"
}

$evidenceDir = Join-Path $repoRoot "docs/evidence/profiles/v$gateInt"
if (-not (Test-Path $evidenceDir)) {
    throw "active-ladder-sync: missing terminal evidence directory docs/evidence/profiles/v$gateInt"
}

Write-Host "active-ladder-sync: ok (range=$($agentsLadder.range) gate=$agentsGate ladder=$($agentsLadder.path))"
Write-Host "active-ladder-sync: workset candidates=$($worksetMatches.Count) pattern=$worksetPattern"
