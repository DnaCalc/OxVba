param(
    [ValidateSet("Start", "Status", "Tail", "Wait", "Stop", "WatchStart", "WatchStop", "Probe", "Reconcile")]
    [string]$Action = "Status",
    [string]$Name = "formal-kani",
    [string]$ProfileScope = "mvp-language-stdlib-consolidation-gate-v56",
    [string]$Command = "",
    [int]$TailLines = 80,
    [int]$PollSeconds = 5,
    [int]$TimeoutSeconds = 0,
    [int]$WatchPollSeconds = 600,
    [bool]$StartWatcher = $true,
    [switch]$SkipPreflight,
    [int]$PreflightRetries = 3,
    [int]$PreflightRetryDelaySeconds = 3,
    [int]$StallThresholdSeconds = 21600
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    function Get-UtcNowText() {
        return (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    }

    function Get-RunPaths([string]$runName) {
        $root = Join-Path (Get-Location) "temp/async/formal-kani"
        $runDir = Join-Path $root $runName
        return [PSCustomObject]@{
            Root = $root
            RunDir = $runDir
            StatePath = (Join-Path $runDir "state.json")
            CommandPath = (Join-Path $runDir "command.ps1")
            StdoutPath = (Join-Path $runDir "stdout.log")
            StderrPath = (Join-Path $runDir "stderr.log")
            ExitCodePath = (Join-Path $runDir "exit_code.txt")
            CompletedPath = (Join-Path $runDir "completed_utc.txt")
            LivenessPath = (Join-Path $runDir "liveness.log")
            WatcherStdoutPath = (Join-Path $runDir "watcher.stdout.log")
            WatcherStderrPath = (Join-Path $runDir "watcher.stderr.log")
            PreflightPath = (Join-Path $runDir "preflight.json")
            StatusSnapshotPath = (Join-Path $runDir "status_snapshot.json")
        }
    }

    function Read-State([string]$statePath) {
        if (-not (Test-Path $statePath)) {
            return $null
        }
        try {
            return Get-Content $statePath -Raw | ConvertFrom-Json
        }
        catch {
            return $null
        }
    }

    function Write-State([string]$statePath, [object]$state) {
        $state | ConvertTo-Json -Depth 8 | Set-Content $statePath
    }

    function Is-Running([int]$ProcessId) {
        if ($ProcessId -le 0) {
            return $false
        }
        return $null -ne (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue)
    }

    function Read-ExitCode([string]$exitCodePath) {
        if (-not (Test-Path $exitCodePath)) {
            return ""
        }
        return (Get-Content $exitCodePath -Raw).Trim()
    }

    function Read-LastLine([string]$path) {
        if (-not (Test-Path $path)) {
            return ""
        }
        try {
            $line = Get-Content $path -Tail 1
            if ($null -eq $line) {
                return ""
            }
            return [string]$line
        }
        catch {
            return ""
        }
    }

    function Get-FileSnapshot([string]$path) {
        if (-not (Test-Path $path)) {
            return [PSCustomObject]@{
                path = $path
                exists = $false
                size_bytes = 0
                last_write_utc = ""
                age_seconds = -1
            }
        }

        $item = Get-Item $path
        $lastWriteUtc = $item.LastWriteTimeUtc
        $ageSeconds = [int]((Get-Date).ToUniversalTime().Subtract($lastWriteUtc).TotalSeconds)
        return [PSCustomObject]@{
            path = $path
            exists = $true
            size_bytes = [int64]$item.Length
            last_write_utc = $lastWriteUtc.ToString("yyyy-MM-ddTHH:mm:ssZ")
            age_seconds = $ageSeconds
        }
    }

    function Get-StatusSnapshot([object]$state, [object]$paths, [int]$stallThresholdSeconds) {
        $runnerPid = if ($null -ne $state.pid) { [int]$state.pid } else { 0 }
        $watcherPid = if ($null -ne $state.watcher_pid) { [int]$state.watcher_pid } else { 0 }
        $runnerRunning = Is-Running $runnerPid
        $watcherRunning = Is-Running $watcherPid
        $exitCode = Read-ExitCode $paths.ExitCodePath
        $completedUtc = if (Test-Path $paths.CompletedPath) { (Get-Content $paths.CompletedPath -Raw).Trim() } else { "" }
        $stdoutInfo = Get-FileSnapshot $paths.StdoutPath
        $stderrInfo = Get-FileSnapshot $paths.StderrPath
        $livenessInfo = Get-FileSnapshot $paths.LivenessPath
        $preflightInfo = Get-FileSnapshot $paths.PreflightPath
        $statusSnapshotInfo = Get-FileSnapshot $paths.StatusSnapshotPath

        $status = "stale"
        if ($runnerRunning) {
            $status = "running"
        }
        elseif (-not [string]::IsNullOrWhiteSpace($exitCode)) {
            $status = "completed"
        }

        $stalled = $false
        if ($runnerRunning) {
            $stdoutAge = if ($stdoutInfo.age_seconds -ge 0) { $stdoutInfo.age_seconds } else { $stallThresholdSeconds + 1 }
            $stderrAge = if ($stderrInfo.age_seconds -ge 0) { $stderrInfo.age_seconds } else { $stallThresholdSeconds + 1 }
            if ($stdoutAge -ge $stallThresholdSeconds -and $stderrAge -ge $stallThresholdSeconds) {
                $stalled = $true
            }
        }

        return [PSCustomObject]@{
            observed_utc = (Get-UtcNowText)
            name = $state.name
            status = $status
            runner_pid = $runnerPid
            runner_running = $runnerRunning
            watcher_pid = $watcherPid
            watcher_running = $watcherRunning
            exit_code = $exitCode
            completed_utc = $completedUtc
            stall_threshold_seconds = $stallThresholdSeconds
            stalled_hint = $stalled
            stdout = $stdoutInfo
            stderr = $stderrInfo
            liveness = $livenessInfo
            preflight = $preflightInfo
            status_snapshot = $statusSnapshotInfo
            liveness_last = (Read-LastLine $paths.LivenessPath)
        }
    }

    function Persist-StatusSnapshot([object]$paths, [object]$state, [object]$snapshot) {
        $snapshot | ConvertTo-Json -Depth 8 | Set-Content $paths.StatusSnapshotPath
        $state | Add-Member -NotePropertyName last_observed_utc -NotePropertyValue $snapshot.observed_utc -Force
        $state | Add-Member -NotePropertyName status_hint -NotePropertyValue $snapshot.status -Force
        $state | Add-Member -NotePropertyName runner_alive -NotePropertyValue $snapshot.runner_running -Force
        $state | Add-Member -NotePropertyName watcher_alive -NotePropertyValue $snapshot.watcher_running -Force
        $state | Add-Member -NotePropertyName stalled_hint -NotePropertyValue $snapshot.stalled_hint -Force
        Write-State -statePath $paths.StatePath -state $state
    }

    function Invoke-KaniPreflight(
        [string]$resolvedCommand,
        [object]$paths,
        [int]$retries,
        [int]$retryDelaySeconds
    ) {
        $usesWslKani = $resolvedCommand -match "-UseWslKani"
        $usesKani = $usesWslKani -or $resolvedCommand -match "\bcargo\s+kani\b" -or $resolvedCommand -match "-RequireKani"
        $wslCommandAvailable = $null -ne (Get-Command wsl -ErrorAction SilentlyContinue)

        $result = [ordered]@{
            timestamp_utc = (Get-UtcNowText)
            command = $resolvedCommand
            uses_kani = $usesKani
            uses_wsl_kani = $usesWslKani
            retries = $retries
            retry_delay_seconds = $retryDelaySeconds
            local_kani_available = $false
            local_kani_version = ""
            local_probe_detail = ""
            wsl_command_available = $wslCommandAvailable
            wsl_kani_available = $false
            wsl_kani_version = ""
            wsl_probe_detail = ""
            status = "ok"
        }

        if ($usesKani) {
            try {
                $localOut = (& cargo kani --version 2>&1) -join " "
                if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($localOut)) {
                    $result.local_kani_available = $true
                    $result.local_kani_version = $localOut.Trim()
                }
                else {
                    $result.local_probe_detail = "exit_code=$LASTEXITCODE output=$localOut"
                }
            }
            catch {
                $result.local_probe_detail = $_.Exception.Message
            }
        }

        if ($usesWslKani) {
            if (-not $wslCommandAvailable) {
                $result.status = "failed"
                $result.wsl_probe_detail = "wsl command unavailable on PATH"
            }
            else {
                $wslSucceeded = $false
                for ($attempt = 1; $attempt -le $retries; $attempt++) {
                    try {
                        $wslOut = (& wsl bash -lc 'source $HOME/.cargo/env && cargo kani --version' 2>&1) -join " "
                        if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($wslOut)) {
                            $result.wsl_kani_available = $true
                            $result.wsl_kani_version = $wslOut.Trim()
                            $wslSucceeded = $true
                            break
                        }
                        $result.wsl_probe_detail = "attempt=$attempt exit_code=$LASTEXITCODE output=$wslOut"
                    }
                    catch {
                        $result.wsl_probe_detail = "attempt=$attempt error=$($_.Exception.Message)"
                    }
                    if ($attempt -lt $retries -and $retryDelaySeconds -gt 0) {
                        Start-Sleep -Seconds $retryDelaySeconds
                    }
                }

                if (-not $wslSucceeded) {
                    $result.status = "failed"
                }
            }
        }

        if ($usesKani -and -not $usesWslKani -and -not $result.local_kani_available) {
            $result.status = "failed"
        }

        $resultObject = [PSCustomObject]$result
        $resultObject | ConvertTo-Json -Depth 8 | Set-Content $paths.PreflightPath
        return $resultObject
    }

    function Stop-WatcherIfRunning([object]$state) {
        if ($null -ne $state.watcher_pid -and (Is-Running ([int]$state.watcher_pid))) {
            Stop-Process -Id ([int]$state.watcher_pid) -Force
            return $true
        }
        return $false
    }

    function Archive-PreviousRunArtifacts([object]$paths) {
        $archiveStamp = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
        $archiveDir = Join-Path $paths.RunDir "archive-$archiveStamp"
        New-Item -ItemType Directory -Path $archiveDir -Force | Out-Null

        $candidatePaths = @(
            $paths.StatePath,
            $paths.CommandPath,
            $paths.StdoutPath,
            $paths.StderrPath,
            $paths.ExitCodePath,
            $paths.CompletedPath,
            $paths.LivenessPath,
            $paths.WatcherStdoutPath,
            $paths.WatcherStderrPath,
            $paths.PreflightPath,
            $paths.StatusSnapshotPath
        )

        foreach ($candidate in $candidatePaths) {
            if (Test-Path $candidate) {
                $leaf = Split-Path -Leaf $candidate
                Move-Item -Path $candidate -Destination (Join-Path $archiveDir $leaf) -Force
            }
        }

        return $archiveDir
    }

    function Start-Watcher([string]$runName, [object]$paths, [int]$watchPollSeconds) {
        $watcherScript = Join-Path $PSScriptRoot "watch-formal-kani-async.ps1"
        if (-not (Test-Path $watcherScript)) {
            throw "watch script missing: $watcherScript"
        }

        Remove-Item $paths.WatcherStdoutPath, $paths.WatcherStderrPath -Force -ErrorAction SilentlyContinue
        return Start-Process `
            -FilePath "pwsh" `
            -ArgumentList @(
                "-NoProfile",
                "-File",
                $watcherScript,
                "-Name",
                $runName,
                "-PollSeconds",
                $watchPollSeconds,
                "-LogPath",
                $paths.LivenessPath
            ) `
            -WindowStyle Hidden `
            -RedirectStandardOutput $paths.WatcherStdoutPath `
            -RedirectStandardError $paths.WatcherStderrPath `
            -PassThru
    }

    $paths = Get-RunPaths $Name
    $runner = Join-Path $PSScriptRoot "async-task-runner.ps1"

    if (-not (Test-Path $paths.RunDir)) {
        New-Item -ItemType Directory -Path $paths.RunDir -Force | Out-Null
    }

    if ($Action -eq "Probe") {
        $commandForProbe = $Command
        if ([string]::IsNullOrWhiteSpace($commandForProbe)) {
            $stateForProbe = Read-State $paths.StatePath
            if ($null -ne $stateForProbe -and -not [string]::IsNullOrWhiteSpace($stateForProbe.command)) {
                $commandForProbe = [string]$stateForProbe.command
            }
        }
        if ([string]::IsNullOrWhiteSpace($commandForProbe)) {
            $commandForProbe = "./scripts/run-formal.ps1 -ProfileScope $ProfileScope -RequireKani -UseWslKani"
        }

        $probe = Invoke-KaniPreflight `
            -resolvedCommand $commandForProbe `
            -paths $paths `
            -retries $PreflightRetries `
            -retryDelaySeconds $PreflightRetryDelaySeconds

        Write-Host "async formal probe: name=$Name status=$($probe.status)"
        Write-Host "preflight: $($paths.PreflightPath)"
        Write-Host "uses_kani: $($probe.uses_kani) uses_wsl_kani: $($probe.uses_wsl_kani)"
        Write-Host "local_kani: $(if ($probe.local_kani_available) { $probe.local_kani_version } else { 'unavailable' })"
        Write-Host "wsl_kani: $(if ($probe.wsl_kani_available) { $probe.wsl_kani_version } else { 'unavailable' })"
        if (-not [string]::IsNullOrWhiteSpace($probe.wsl_probe_detail)) {
            Write-Host "wsl_probe_detail: $($probe.wsl_probe_detail)"
        }
        return
    }

    if ($Action -eq "Start") {
        $existing = Read-State $paths.StatePath
        if ($null -ne $existing) {
            $existingSnapshot = Get-StatusSnapshot -state $existing -paths $paths -stallThresholdSeconds $StallThresholdSeconds
            if ($existingSnapshot.runner_running) {
                throw "async formal run '$Name' is already running (pid=$($existing.pid))"
            }
            [void](Stop-WatcherIfRunning $existing)
            $archiveDir = Archive-PreviousRunArtifacts $paths
            Write-Host "async formal start: archived previous artifacts to $archiveDir"
        }

        if ([string]::IsNullOrWhiteSpace($Command)) {
            $Command = "./scripts/run-formal.ps1 -ProfileScope $ProfileScope -RequireKani -UseWslKani"
        }

        if (-not $SkipPreflight) {
            $probe = Invoke-KaniPreflight `
                -resolvedCommand $Command `
                -paths $paths `
                -retries $PreflightRetries `
                -retryDelaySeconds $PreflightRetryDelaySeconds
            if ($probe.status -ne "ok") {
                throw "async formal preflight failed for '$Name'; inspect $($paths.PreflightPath)"
            }
        }

        Remove-Item `
            $paths.StdoutPath, `
            $paths.StderrPath, `
            $paths.ExitCodePath, `
            $paths.CompletedPath, `
            $paths.CommandPath, `
            $paths.LivenessPath, `
            $paths.WatcherStdoutPath, `
            $paths.WatcherStderrPath, `
            $paths.StatusSnapshotPath `
            -Force -ErrorAction SilentlyContinue

        if ($SkipPreflight) {
            Remove-Item $paths.PreflightPath -Force -ErrorAction SilentlyContinue
        }

        Set-Content -Path $paths.CommandPath -Value $Command
        $process = Start-Process `
            -FilePath "pwsh" `
            -ArgumentList @(
                "-NoProfile",
                "-File",
                $runner,
                "-CommandFilePath",
                $paths.CommandPath,
                "-ExitCodePath",
                $paths.ExitCodePath,
                "-CompletedPath",
                $paths.CompletedPath
            ) `
            -WindowStyle Hidden `
            -RedirectStandardOutput $paths.StdoutPath `
            -RedirectStandardError $paths.StderrPath `
            -PassThru

        $state = [ordered]@{
            name = $Name
            pid = $process.Id
            started_utc = (Get-UtcNowText)
            profile_scope = $ProfileScope
            command = $Command
            command_file = $paths.CommandPath
            stdout = $paths.StdoutPath
            stderr = $paths.StderrPath
            exit_code_file = $paths.ExitCodePath
            completed_utc_file = $paths.CompletedPath
            liveness_log = $paths.LivenessPath
            watcher_pid = 0
            watcher_stdout = $paths.WatcherStdoutPath
            watcher_stderr = $paths.WatcherStderrPath
            preflight = $paths.PreflightPath
            status_snapshot = $paths.StatusSnapshotPath
            last_observed_utc = ""
            status_hint = "starting"
            runner_alive = $true
            watcher_alive = $false
            stalled_hint = $false
        }
        if ($StartWatcher) {
            $watcherProcess = Start-Watcher -runName $Name -paths $paths -watchPollSeconds $WatchPollSeconds
            $state.watcher_pid = $watcherProcess.Id
            $state.watcher_alive = $true
        }
        Write-State -statePath $paths.StatePath -state $state

        $startSnapshot = Get-StatusSnapshot -state $state -paths $paths -stallThresholdSeconds $StallThresholdSeconds
        Persist-StatusSnapshot -paths $paths -state $state -snapshot $startSnapshot

        Write-Host "async formal start: name=$Name pid=$($process.Id)"
        if ($StartWatcher) {
            Write-Host "watcher: running pid=$($state.watcher_pid) poll_seconds=$WatchPollSeconds"
            Write-Host "liveness: $($paths.LivenessPath)"
        }
        Write-Host "status_snapshot: $($paths.StatusSnapshotPath)"
        if (-not $SkipPreflight) {
            Write-Host "preflight: $($paths.PreflightPath)"
        }
        Write-Host "stdout: $($paths.StdoutPath)"
        Write-Host "stderr: $($paths.StderrPath)"
        return
    }

    $state = Read-State $paths.StatePath
    if ($null -eq $state) {
        throw "No async formal run named '$Name'. Start one with -Action Start."
    }

    $snapshot = Get-StatusSnapshot -state $state -paths $paths -stallThresholdSeconds $StallThresholdSeconds
    Persist-StatusSnapshot -paths $paths -state $state -snapshot $snapshot

    if ($Action -eq "Status") {
        Write-Host "async formal status: name=$($state.name) status=$($snapshot.status) pid=$($snapshot.runner_pid)"
        if (-not [string]::IsNullOrWhiteSpace($snapshot.completed_utc)) {
            Write-Host "completed_utc: $($snapshot.completed_utc)"
        }
        if (-not [string]::IsNullOrWhiteSpace($snapshot.exit_code)) {
            Write-Host "exit_code: $($snapshot.exit_code)"
        }
        Write-Host "runner: status=$(if ($snapshot.runner_running) { 'running' } else { 'stopped' }) pid=$($snapshot.runner_pid)"
        Write-Host "watcher: status=$(if ($snapshot.watcher_running) { 'running' } else { 'stopped' }) pid=$($snapshot.watcher_pid)"
        Write-Host "stalled_hint: $($snapshot.stalled_hint) threshold_seconds=$($snapshot.stall_threshold_seconds)"
        if (-not [string]::IsNullOrWhiteSpace($snapshot.liveness_last)) {
            Write-Host "liveness_last: $($snapshot.liveness_last)"
        }
        Write-Host "stdout: $($snapshot.stdout.path) size_bytes=$($snapshot.stdout.size_bytes) age_seconds=$($snapshot.stdout.age_seconds)"
        Write-Host "stderr: $($snapshot.stderr.path) size_bytes=$($snapshot.stderr.size_bytes) age_seconds=$($snapshot.stderr.age_seconds)"
        Write-Host "liveness: $($snapshot.liveness.path) size_bytes=$($snapshot.liveness.size_bytes) age_seconds=$($snapshot.liveness.age_seconds)"
        Write-Host "preflight: $($snapshot.preflight.path) age_seconds=$($snapshot.preflight.age_seconds)"
        Write-Host "status_snapshot: $($paths.StatusSnapshotPath)"
        return
    }

    if ($Action -eq "Tail") {
        Write-Host "---- stdout ($TailLines lines) ----"
        if (Test-Path $paths.StdoutPath) {
            Get-Content $paths.StdoutPath -Tail $TailLines
        }
        else {
            Write-Host "<no stdout log>"
        }

        Write-Host "---- stderr ($TailLines lines) ----"
        if (Test-Path $paths.StderrPath) {
            Get-Content $paths.StderrPath -Tail $TailLines
        }
        else {
            Write-Host "<no stderr log>"
        }
        return
    }

    if ($Action -eq "Reconcile") {
        $watcherStopped = $false
        if ($snapshot.status -ne "running") {
            $watcherStopped = Stop-WatcherIfRunning $state
            if ($watcherStopped) {
                $state | Add-Member -NotePropertyName watcher_pid -NotePropertyValue 0 -Force
            }
        }

        $snapshot = Get-StatusSnapshot -state $state -paths $paths -stallThresholdSeconds $StallThresholdSeconds
        Persist-StatusSnapshot -paths $paths -state $state -snapshot $snapshot

        Write-Host "async formal reconcile: name=$($state.name) status=$($snapshot.status) pid=$($snapshot.runner_pid)"
        if ($watcherStopped) {
            Write-Host "watcher: stopped stale/finished watcher process"
        }
        if ($snapshot.status -eq "stale") {
            Write-Host "note: stale state detected (runner not alive and no exit_code marker)"
        }
        Write-Host "status_snapshot: $($paths.StatusSnapshotPath)"
        return
    }

    if ($Action -eq "Wait") {
        $started = Get-Date
        while ($true) {
            $loopState = Read-State $paths.StatePath
            if ($null -eq $loopState) {
                throw "async formal wait: state missing for '$Name'"
            }
            $loopSnapshot = Get-StatusSnapshot -state $loopState -paths $paths -stallThresholdSeconds $StallThresholdSeconds
            Persist-StatusSnapshot -paths $paths -state $loopState -snapshot $loopSnapshot

            if (-not $loopSnapshot.runner_running) {
                $exitCode = if ([string]::IsNullOrWhiteSpace($loopSnapshot.exit_code)) { "unknown" } else { $loopSnapshot.exit_code }
                Write-Host "async formal wait: completed name=$($loopState.name) pid=$($loopSnapshot.runner_pid) status=$($loopSnapshot.status) exit_code=$exitCode"
                if ($exitCode -match '^\d+$') {
                    exit ([int]$exitCode)
                }
                exit 3
            }

            if ($TimeoutSeconds -gt 0) {
                $elapsed = ((Get-Date) - $started).TotalSeconds
                if ($elapsed -ge $TimeoutSeconds) {
                    Write-Host "async formal wait: timed out after $TimeoutSeconds seconds"
                    exit 2
                }
            }

            Start-Sleep -Seconds $PollSeconds
        }
    }

    if ($Action -eq "Stop") {
        $runnerStopped = $false
        if (Is-Running ([int]$state.pid)) {
            Stop-Process -Id ([int]$state.pid) -Force
            $runnerStopped = $true
            Write-Host "async formal stop: name=$($state.name) pid=$($state.pid)"
        }
        else {
            Write-Host "async formal stop: process already not running"
        }

        if (Stop-WatcherIfRunning $state) {
            Write-Host "async formal stop: watcher pid=$($state.watcher_pid)"
            $state | Add-Member -NotePropertyName watcher_pid -NotePropertyValue 0 -Force
        }
        elseif ($null -ne $state.watcher_pid -and [int]$state.watcher_pid -gt 0) {
            $state | Add-Member -NotePropertyName watcher_pid -NotePropertyValue 0 -Force
        }

        $state | Add-Member -NotePropertyName stop_requested_utc -NotePropertyValue (Get-UtcNowText) -Force
        $stoppedSnapshot = Get-StatusSnapshot -state $state -paths $paths -stallThresholdSeconds $StallThresholdSeconds
        Persist-StatusSnapshot -paths $paths -state $state -snapshot $stoppedSnapshot
        if ($runnerStopped) {
            Write-Host "status_snapshot: $($paths.StatusSnapshotPath)"
        }
        return
    }

    if ($Action -eq "WatchStart") {
        if ($snapshot.status -eq "completed") {
            Write-Host "async formal watcher: not started because run is completed"
            return
        }
        if ($null -ne $state.watcher_pid -and (Is-Running ([int]$state.watcher_pid))) {
            Write-Host "async formal watcher: already running pid=$($state.watcher_pid)"
            return
        }

        $watcherProcess = Start-Watcher -runName $Name -paths $paths -watchPollSeconds $WatchPollSeconds
        $state | Add-Member -NotePropertyName watcher_pid -NotePropertyValue $watcherProcess.Id -Force
        $state | Add-Member -NotePropertyName liveness_log -NotePropertyValue $paths.LivenessPath -Force
        $state | Add-Member -NotePropertyName watcher_stdout -NotePropertyValue $paths.WatcherStdoutPath -Force
        $state | Add-Member -NotePropertyName watcher_stderr -NotePropertyValue $paths.WatcherStderrPath -Force
        $state | Add-Member -NotePropertyName watcher_alive -NotePropertyValue $true -Force
        Write-State -statePath $paths.StatePath -state $state

        Write-Host "async formal watcher start: name=$($state.name) pid=$($watcherProcess.Id) poll_seconds=$WatchPollSeconds"
        Write-Host "liveness: $($paths.LivenessPath)"
        return
    }

    if ($Action -eq "WatchStop") {
        if ($null -ne $state.watcher_pid -and (Is-Running ([int]$state.watcher_pid))) {
            Stop-Process -Id ([int]$state.watcher_pid) -Force
            Write-Host "async formal watcher stop: name=$($state.name) pid=$($state.watcher_pid)"
        }
        else {
            Write-Host "async formal watcher stop: watcher already not running"
        }
        $state | Add-Member -NotePropertyName watcher_pid -NotePropertyValue 0 -Force
        $state | Add-Member -NotePropertyName watcher_alive -NotePropertyValue $false -Force
        Write-State -statePath $paths.StatePath -state $state
        return
    }
}
finally {
    Pop-Location
}
