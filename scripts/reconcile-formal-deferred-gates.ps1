param(
    [string]$DeferredGatesPath = "docs/evidence/formal/DEFERRED_GATES.md",
    [string]$SshHost = "94.72.99.81",
    [string]$SshUser = "ubuntu",
    [string]$SshKeyPath = "$env:USERPROFILE\.ssh\acfs_ed25519",
    [string]$RemoteBase = "/home/ubuntu/.dnacalc_remote",
    [switch]$AllowPassToRunning,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

if (-not (Test-Path $DeferredGatesPath)) {
    throw "missing deferred gates file: $DeferredGatesPath"
}

$remotePy = @"
import glob, json, os
base = r"$RemoteBase"
root = os.path.join(base, "state", "deferred_lanes")
rows = []
for d in sorted(glob.glob(os.path.join(root, "*"))):
    if not os.path.isdir(d):
        continue
    lane = os.path.basename(d)
    st_path = os.path.join(d, "status.txt")
    if not os.path.exists(st_path):
        continue
    with open(st_path, "r", encoding="utf-8", errors="ignore") as f:
        st_lines = [ln.strip() for ln in f.readlines() if ln.strip()]
    state = st_lines[-1] if st_lines else ""
    selected = None
    phase = None
    completion = None
    prog_path = os.path.join(d, "progress.json")
    if os.path.exists(prog_path):
        try:
            with open(prog_path, "r", encoding="utf-8", errors="ignore") as f:
                p = json.load(f)
            selected = p.get("selected_count")
            phase = p.get("phase")
            status = p.get("status")
            if status:
                completion = f"completed:{status}"
        except Exception:
            pass
    rows.append({
        "lane": lane,
        "state": state,
        "selected_count": selected,
        "phase": phase,
        "completion": completion
    })
print(json.dumps(rows))
"@

$sshTarget = "$SshUser@$SshHost"
$jsonRaw = $remotePy | & ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=4 -o TCPKeepAlive=yes -o ConnectTimeout=10 -i $SshKeyPath $sshTarget "python3 -"
if ($LASTEXITCODE -ne 0) {
    throw "remote lane query failed"
}

$jsonText = ($jsonRaw -join "`n").Trim()
if ([string]::IsNullOrWhiteSpace($jsonText)) {
    throw "remote lane query returned empty result"
}

$laneRows = $jsonText | ConvertFrom-Json
$laneMap = @{}
foreach ($row in $laneRows) {
    $lane = [string]$row.lane
    $state = [string]$row.state
    if ([string]::IsNullOrWhiteSpace($lane) -or [string]::IsNullOrWhiteSpace($state)) {
        continue
    }
    $selectedCount = $row.selected_count
    $completedStatus = $row.completion

    $mapped = $null
    if ($state.StartsWith("running") -or $state.StartsWith("started")) {
        $mapped = "dg-running"
    }
    elseif ($state -eq "completed:no-op" -or ($completedStatus -eq "completed:pass" -and $selectedCount -ne $null -and [int]$selectedCount -le 0)) {
        $mapped = "dg-deferred"
    }
    elseif ($state -eq "completed:pass" -or $completedStatus -eq "completed:pass") {
        $mapped = "dg-pass"
    }
    elseif ($state -eq "completed:fail" -or $state -eq "completed:stopped") {
        $mapped = "dg-fail"
    }
    elseif ($completedStatus -eq "completed:fail" -or $completedStatus -eq "completed:stopped") {
        $mapped = "dg-fail"
    }

    if ($mapped) {
        $laneMap[$lane] = [pscustomobject]@{
            lane = $lane
            raw_state = $state
            mapped_status = $mapped
            selected_count = $selectedCount
            completed_status = $completedStatus
        }
    }
}

$lines = Get-Content $DeferredGatesPath
$updated = [System.Collections.Generic.List[string]]::new()
$changes = [System.Collections.Generic.List[string]]::new()
$ts = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

foreach ($line in $lines) {
    if ($line -notmatch '^\|\s*DG-') {
        $updated.Add($line)
        continue
    }

    $parts = $line.Split('|')
    if ($parts.Length -lt 9) {
        $updated.Add($line)
        continue
    }

    $dgId = $parts[1].Trim()
    $runName = $parts[3].Trim()
    $currentStatus = $parts[4].Trim()

    if (-not $laneMap.ContainsKey($runName)) {
        $updated.Add($line)
        continue
    }

    $laneInfo = $laneMap[$runName]
    $targetStatus = $laneInfo.mapped_status
    $note = $parts[8].Trim()

    $shouldUpdateStatus = $currentStatus -in @("dg-running", "dg-not-started", "dg-deferred", "dg-fail")
    if ($currentStatus -eq "dg-pass" -and $targetStatus -eq "dg-running" -and $AllowPassToRunning) {
        $shouldUpdateStatus = $true
    }
    if (-not $shouldUpdateStatus) {
        $updated.Add($line)
        continue
    }

    if ($targetStatus -eq $currentStatus) {
        $updated.Add($line)
        continue
    }

    $parts[4] = " $targetStatus "

    $syncFragment = "Reconciled $ts from remote lane status ($($laneInfo.raw_state))"
    if ($laneInfo.selected_count -ne $null) {
        $syncFragment += " selected_count=$($laneInfo.selected_count)"
    }

    $note = [regex]::Replace(
        $note,
        'Reconciled\s+[0-9TZ:\-]+\s+from\s+remote\s+lane\s+status\s*\([^)]*\)(\s+selected_count=-?[0-9]+)?',
        $syncFragment
    )
    if ($note -notmatch 'Reconciled\s+[0-9TZ:\-]+\s+from\s+remote\s+lane\s+status') {
        if ([string]::IsNullOrWhiteSpace($note)) {
            $note = $syncFragment
        }
        else {
            $note = "$note; $syncFragment"
        }
    }
    $parts[8] = " $note "

    $newLine = ($parts -join '|')
    $updated.Add($newLine)
    $changes.Add("${dgId}: $currentStatus -> $targetStatus ($runName)")
}

if ($changes.Count -eq 0) {
    Write-Host "deferred-gates-reconcile: no status changes"
    exit 0
}

if ($DryRun) {
    Write-Host "deferred-gates-reconcile: dry-run changes"
    $changes | ForEach-Object { Write-Host "  $_" }
    exit 0
}

Set-Content -Path $DeferredGatesPath -Value $updated
Write-Host "deferred-gates-reconcile: updated $($changes.Count) rows"
$changes | ForEach-Object { Write-Host "  $_" }
