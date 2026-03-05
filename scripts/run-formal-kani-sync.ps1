param(
    [string]$PolicyCsv = "docs/evidence/formal/KANI_OBLIGATION_POLICY_V1.csv",
    [string]$VersionList = "",
    [int]$DeferredConcurrency = 0,
    [int]$ObligationTimeoutSeconds = 10800,
    [int]$ObligationTimeoutRetries = 1,
    [double]$ObligationTimeoutMultiplier = 10.0,
    [int]$MemorySoftUsedPercent = 85,
    [int]$MemoryHardUsedPercent = 92,
    [ValidateSet("pause", "halt-one", "halt-all", "none")]
    [string]$HardPressureAction = "pause",
    [string]$DispatchJobName = "deferred-sync",
    [string]$SshHost = "94.72.99.81",
    [string]$SshUser = "ubuntu",
    [string]$SshKeyPath = "$env:USERPROFILE\.ssh\acfs_ed25519",
    [string]$RemoteBase = "/home/ubuntu/.dnacalc_remote",
    [switch]$NoStart,
    [switch]$ForceStart
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

if (-not (Test-Path $PolicyCsv)) {
    throw "missing policy csv: $PolicyCsv"
}

function Get-RemoteLaneRows {
    param(
        [string]$RemoteHost,
        [string]$User,
        [string]$KeyPath,
        [string]$Base
    )

    $remotePy = @"
import glob, json, os
base = r"$Base"
root = os.path.join(base, "state", "deferred_lanes")
rows = []
for d in sorted(glob.glob(os.path.join(root, "*"))):
    if not os.path.isdir(d):
        continue
    lane = os.path.basename(d)
    st_path = os.path.join(d, "status.txt")
    state = ""
    if os.path.exists(st_path):
        with open(st_path, "r", encoding="utf-8", errors="ignore") as f:
            lines = [ln.strip() for ln in f.readlines() if ln.strip()]
        if lines:
            state = lines[-1]
    if not state:
        continue
    rows.append({"lane": lane, "state": state})
print(json.dumps(rows))
"@

    $sshTarget = "$User@$RemoteHost"
    $jsonRaw = $remotePy | & ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=4 -o TCPKeepAlive=yes -o ConnectTimeout=10 -i $KeyPath $sshTarget "python3 -"
    if ($LASTEXITCODE -ne 0) {
        throw "remote lane query failed"
    }
    $jsonText = ($jsonRaw -join "`n").Trim()
    if ([string]::IsNullOrWhiteSpace($jsonText)) {
        return @()
    }
    return @($jsonText | ConvertFrom-Json)
}

function Get-RemoteRecommendedConcurrency {
    param(
        [string]$RemoteHost,
        [string]$User,
        [string]$KeyPath,
        [string]$Base
    )

    $sshTarget = "$User@$RemoteHost"
    $probeRaw = & ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=4 -o TCPKeepAlive=yes -o ConnectTimeout=10 -i $KeyPath $sshTarget "set -euo pipefail; source '$Base/bin/env.sh'; '$Base/bin/probe_capacity.sh'"
    if ($LASTEXITCODE -ne 0) {
        throw "remote capacity probe failed"
    }
    $probeText = ($probeRaw -join "`n")
    if ($probeText -match 'recommended_concurrency=(\d+)') {
        return [int]$Matches[1]
    }
    return 1
}

$versions = @()
if (-not [string]::IsNullOrWhiteSpace($VersionList)) {
    $versions = ($VersionList -split "[\s,]+" | Where-Object { $_ -match '^\d+$' } | ForEach-Object { [int]$_ })
}
else {
    $versions = Import-Csv $PolicyCsv |
        Where-Object { $_.current_state -and $_.current_state.Trim().ToLowerInvariant() -ne "pass" } |
        ForEach-Object {
            if ($_.profile -match '^v(\d+)$') { [int]$Matches[1] }
        } |
        Sort-Object -Unique
}

if ($versions.Count -eq 0) {
    Write-Host "formal-sync: no candidate versions from policy"
    exit 0
}

$versionText = ($versions | ForEach-Object { $_.ToString() }) -join " "
Write-Host "formal-sync: candidate versions=$versionText"

./scripts/reconcile-formal-deferred-gates.ps1 -SshHost $SshHost -SshUser $SshUser -SshKeyPath $SshKeyPath -RemoteBase $RemoteBase

$laneRows = Get-RemoteLaneRows -RemoteHost $SshHost -User $SshUser -KeyPath $SshKeyPath -Base $RemoteBase
$running = @($laneRows | Where-Object { $_.state -like "running*" -or $_.state -like "started*" })
if ($running.Count -gt 0 -and -not $ForceStart) {
    Write-Host "formal-sync: active deferred lanes detected; skipping start"
    foreach ($r in $running) {
        Write-Host "  $($r.lane) $($r.state)"
    }
    ./scripts/run-formal-kani-remote.ps1 -Action Status -SshHost $SshHost -SshUser $SshUser -SshKeyPath $SshKeyPath -RemoteBase $RemoteBase
    exit 0
}

if ($NoStart) {
    Write-Host "formal-sync: no-start requested"
    ./scripts/run-formal-kani-remote.ps1 -Action Status -SshHost $SshHost -SshUser $SshUser -SshKeyPath $SshKeyPath -RemoteBase $RemoteBase
    exit 0
}

if ($DeferredConcurrency -le 0) {
    $DeferredConcurrency = Get-RemoteRecommendedConcurrency -RemoteHost $SshHost -User $SshUser -KeyPath $SshKeyPath -Base $RemoteBase
    if ($DeferredConcurrency -le 0) {
        $DeferredConcurrency = 1
    }
}

Write-Host "formal-sync: starting deferred dispatch concurrency=$DeferredConcurrency"
./scripts/run-formal-kani-remote.ps1 -Action StartDeferred -DeferredMode exact -DeferredStrategy dedup -DeferredVersions $versionText -DeferredConcurrency $DeferredConcurrency -ObligationTimeoutSeconds $ObligationTimeoutSeconds -ObligationTimeoutRetries $ObligationTimeoutRetries -ObligationTimeoutMultiplier $ObligationTimeoutMultiplier -MemorySoftUsedPercent $MemorySoftUsedPercent -MemoryHardUsedPercent $MemoryHardUsedPercent -HardPressureAction $HardPressureAction -DispatchJobName $DispatchJobName -SshHost $SshHost -SshUser $SshUser -SshKeyPath $SshKeyPath -RemoteBase $RemoteBase

Start-Sleep -Seconds 3
./scripts/reconcile-formal-deferred-gates.ps1 -SshHost $SshHost -SshUser $SshUser -SshKeyPath $SshKeyPath -RemoteBase $RemoteBase
./scripts/run-formal-kani-remote.ps1 -Action Status -SshHost $SshHost -SshUser $SshUser -SshKeyPath $SshKeyPath -RemoteBase $RemoteBase
