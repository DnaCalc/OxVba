function Get-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}

function Get-ActiveGateTag {
    param(
        [string]$AgentsPath = "AGENTS.md"
    )

    if (-not (Test-Path $AgentsPath)) {
        throw "run-context: missing AGENTS file: $AgentsPath"
    }

    $agents = Get-Content $AgentsPath -Raw
    $match = [regex]::Match($agents, 'Current required terminal gate:\s*`?(v\d+)`?', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
    if (-not $match.Success) {
        throw "run-context: unable to parse terminal gate from $AgentsPath"
    }

    return $match.Groups[1].Value.ToLowerInvariant()
}

function Get-ActiveGateInt {
    param(
        [string]$AgentsPath = "AGENTS.md"
    )

    $tag = Get-ActiveGateTag -AgentsPath $AgentsPath
    $match = [regex]::Match($tag, '^v(\d+)$', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
    if (-not $match.Success) {
        throw "run-context: malformed gate tag '$tag'"
    }
    return [int]$match.Groups[1].Value
}

function Get-DefaultProfileScope {
    param(
        [string]$AgentsPath = "AGENTS.md"
    )

    $gate = Get-ActiveGateTag -AgentsPath $AgentsPath
    return "mvp-profile-$gate"
}

function Get-DefaultProfileOutputDir {
    param(
        [string]$AgentsPath = "AGENTS.md"
    )

    $gateInt = Get-ActiveGateInt -AgentsPath $AgentsPath
    return "docs/evidence/profiles/v$gateInt"
}

function Resolve-RunId {
    param(
        [string]$Name,
        [string]$RequestedRunId = "",
        [int]$ReuseWindowMinutes = 60,
        [string]$LockRoot = "temp/run-id-locks"
    )

    if ([string]::IsNullOrWhiteSpace($Name)) {
        throw "run-context: Resolve-RunId requires -Name"
    }

    if (-not [string]::IsNullOrWhiteSpace($RequestedRunId)) {
        return $RequestedRunId
    }

    $envRunId = [string]$env:OXVBA_RUN_ID
    if (-not [string]::IsNullOrWhiteSpace($envRunId)) {
        return $envRunId
    }

    if (-not (Test-Path $LockRoot)) {
        New-Item -ItemType Directory -Path $LockRoot -Force | Out-Null
    }

    $safeName = ($Name -replace '[^A-Za-z0-9_.-]', '_')
    $lockPath = Join-Path $LockRoot ("{0}.lock.json" -f $safeName)
    $nowUtc = (Get-Date).ToUniversalTime()

    if (Test-Path $lockPath) {
        try {
            $lock = Get-Content $lockPath -Raw | ConvertFrom-Json
            if ($null -ne $lock -and -not [string]::IsNullOrWhiteSpace([string]$lock.run_id) -and -not [string]::IsNullOrWhiteSpace([string]$lock.generated_utc)) {
                $generated = [DateTime]::Parse([string]$lock.generated_utc).ToUniversalTime()
                $age = $nowUtc - $generated
                if ($age.TotalMinutes -ge 0 -and $age.TotalMinutes -le $ReuseWindowMinutes) {
                    return [string]$lock.run_id
                }
            }
        }
        catch {
            # Corrupt/stale lock; replace below.
        }
    }

    $runId = $nowUtc.ToString("yyyyMMddTHHmmssZ")
    [PSCustomObject]@{
        name = $Name
        run_id = $runId
        generated_utc = $nowUtc.ToString("o")
        pid = $PID
    } | ConvertTo-Json | Set-Content -Path $lockPath
    return $runId
}

function New-NoArtifactEvidenceDir {
    param(
        [string]$Scope,
        [string]$RunId
    )

    if ([string]::IsNullOrWhiteSpace($Scope)) {
        throw "run-context: New-NoArtifactEvidenceDir requires -Scope"
    }
    if ([string]::IsNullOrWhiteSpace($RunId)) {
        throw "run-context: New-NoArtifactEvidenceDir requires -RunId"
    }

    $safeScope = ($Scope -replace '[^A-Za-z0-9_.-]', '_')
    return (Join-Path "temp/no-artifacts" (Join-Path $safeScope $RunId))
}
