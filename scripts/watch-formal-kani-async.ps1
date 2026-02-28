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

    function Get-FileStat([string]$path) {
        if (-not (Test-Path $path)) {
            return [PSCustomObject]@{
                size_bytes = 0
                age_seconds = -1
            }
        }
        $item = Get-Item $path
        $age = [int]((Get-Date).ToUniversalTime().Subtract($item.LastWriteTimeUtc).TotalSeconds)
        return [PSCustomObject]@{
            size_bytes = [int64]$item.Length
            age_seconds = $age
        }
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
            $stdoutPath = Join-Path $runDir "stdout.log"
            $stderrPath = Join-Path $runDir "stderr.log"
            $stdoutStat = Get-FileStat $stdoutPath
            $stderrStat = Get-FileStat $stderrPath

            if ($running) {
                Write-Log "status=running pid=$runnerPid exit_code=pending stdout_bytes=$($stdoutStat.size_bytes) stdout_age_s=$($stdoutStat.age_seconds) stderr_bytes=$($stderrStat.size_bytes) stderr_age_s=$($stderrStat.age_seconds)"
            }
            elseif (-not [string]::IsNullOrWhiteSpace($exitCode)) {
                Write-Log "status=completed pid=$runnerPid exit_code=$exitCode stdout_bytes=$($stdoutStat.size_bytes) stdout_age_s=$($stdoutStat.age_seconds) stderr_bytes=$($stderrStat.size_bytes) stderr_age_s=$($stderrStat.age_seconds)"
                break
            }
            else {
                Write-Log "status=stale pid=$runnerPid exit_code=pending stdout_bytes=$($stdoutStat.size_bytes) stdout_age_s=$($stdoutStat.age_seconds) stderr_bytes=$($stderrStat.size_bytes) stderr_age_s=$($stderrStat.age_seconds)"
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
