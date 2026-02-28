param(
    [Parameter(Mandatory = $true)]
    [string]$Name,
    [int]$PollSeconds = 600,
    [string]$LogPath = ""
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $runDir = Join-Path (Join-Path (Join-Path "temp" "async") "formal-kani") $Name
    $statePath = Join-Path $runDir "state.json"
    $exitCodePath = Join-Path $runDir "exit_code.txt"

    if ([string]::IsNullOrWhiteSpace($LogPath)) {
        $LogPath = Join-Path $runDir "liveness.log"
    }

    $logDir = Split-Path -Parent $LogPath
    if (-not (Test-Path $logDir)) {
        New-Item -ItemType Directory -Path $logDir -Force | Out-Null
    }

    function Write-Log([string]$message) {
        $ts = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        Add-Content -Path $LogPath -Value "[$ts] $message"
    }

    function Is-Running([int]$processId) {
        if ($processId -le 0) {
            return $false
        }
        return $null -ne (Get-Process -Id $processId -ErrorAction SilentlyContinue)
    }

    Write-Log "watcher start name=$Name poll_seconds=$PollSeconds"

    while ($true) {
        try {
            if (-not (Test-Path $statePath)) {
                Write-Log "status=state-missing pid=0 exit_code=pending"
                Start-Sleep -Seconds $PollSeconds
                continue
            }

            $state = $null
            try {
                $state = Get-Content $statePath -Raw | ConvertFrom-Json
            }
            catch {
                Write-Log "status=state-parse-error pid=0 exit_code=pending detail=$($_.Exception.Message)"
                Start-Sleep -Seconds $PollSeconds
                continue
            }

            $runnerPid = 0
            if ($null -ne $state.pid) {
                $runnerPid = [int]$state.pid
            }

            $running = Is-Running $runnerPid
            $exitCode = if (Test-Path $exitCodePath) { (Get-Content $exitCodePath -Raw).Trim() } else { "" }

            if ($running) {
                Write-Log "status=running pid=$runnerPid exit_code=pending"
            }
            elseif (-not [string]::IsNullOrWhiteSpace($exitCode)) {
                Write-Log "status=completed pid=$runnerPid exit_code=$exitCode"
                break
            }
            else {
                Write-Log "status=unknown pid=$runnerPid exit_code=pending"
            }
        }
        catch {
            Write-Log "status=watch-error pid=0 exit_code=pending detail=$($_.Exception.Message)"
        }

        Start-Sleep -Seconds $PollSeconds
    }

    Write-Log "watcher stop name=$Name"
}
finally {
    Pop-Location
}
