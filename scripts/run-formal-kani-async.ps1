param(
    [ValidateSet("Start", "Status", "Tail", "Wait", "Stop", "WatchStart", "WatchStop")]
    [string]$Action = "Status",
    [string]$Name = "formal-kani",
    [string]$ProfileScope = "mvp-language-stdlib-consolidation-gate-v56",
    [string]$Command = "",
    [int]$TailLines = 80,
    [int]$PollSeconds = 5,
    [int]$TimeoutSeconds = 0,
    [int]$WatchPollSeconds = 600,
    [bool]$StartWatcher = $true
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
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
        $state | ConvertTo-Json | Set-Content $statePath
    }

    function Is-Running([int]$ProcessId) {
        if ($ProcessId -le 0) {
            return $false
        }
        return $null -ne (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue)
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

    if ($Action -eq "Start") {
        if (-not (Test-Path $paths.RunDir)) {
            New-Item -ItemType Directory -Path $paths.RunDir -Force | Out-Null
        }

        $existing = Read-State $paths.StatePath
        if ($null -ne $existing -and (Is-Running ([int]$existing.pid))) {
            throw "async formal run '$Name' is already running (pid=$($existing.pid))"
        }

        if ([string]::IsNullOrWhiteSpace($Command)) {
            $Command = "./scripts/run-formal.ps1 -ProfileScope $ProfileScope -RequireKani -UseWslKani"
        }

        Remove-Item `
            $paths.StdoutPath, `
            $paths.StderrPath, `
            $paths.ExitCodePath, `
            $paths.CompletedPath, `
            $paths.CommandPath, `
            $paths.LivenessPath, `
            $paths.WatcherStdoutPath, `
            $paths.WatcherStderrPath `
            -Force -ErrorAction SilentlyContinue
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
            started_utc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
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
        }
        if ($StartWatcher) {
            $watcherProcess = Start-Watcher -runName $Name -paths $paths -watchPollSeconds $WatchPollSeconds
            $state.watcher_pid = $watcherProcess.Id
        }
        Write-State -statePath $paths.StatePath -state $state

        Write-Host "async formal start: name=$Name pid=$($process.Id)"
        if ($StartWatcher) {
            Write-Host "watcher: running pid=$($state.watcher_pid) poll_seconds=$WatchPollSeconds"
            Write-Host "liveness: $($paths.LivenessPath)"
        }
        Write-Host "stdout: $($paths.StdoutPath)"
        Write-Host "stderr: $($paths.StderrPath)"
        return
    }

    $state = Read-State $paths.StatePath
    if ($null -eq $state) {
        throw "No async formal run named '$Name'. Start one with -Action Start."
    }

    if ($Action -eq "Status") {
        $running = Is-Running ([int]$state.pid)
        $exitCode = if (Test-Path $paths.ExitCodePath) { (Get-Content $paths.ExitCodePath -Raw).Trim() } else { "" }
        $completedUtc = if (Test-Path $paths.CompletedPath) { (Get-Content $paths.CompletedPath -Raw).Trim() } else { "" }
        $status = if ($running) { "running" } elseif (-not [string]::IsNullOrWhiteSpace($exitCode)) { "completed" } else { "unknown" }

        Write-Host "async formal status: name=$($state.name) status=$status pid=$($state.pid)"
        if (-not [string]::IsNullOrWhiteSpace($completedUtc)) {
            Write-Host "completed_utc: $completedUtc"
        }
        if (-not [string]::IsNullOrWhiteSpace($exitCode)) {
            Write-Host "exit_code: $exitCode"
        }
        if ($null -ne $state.watcher_pid -and [int]$state.watcher_pid -gt 0) {
            $watcherRunning = Is-Running ([int]$state.watcher_pid)
            Write-Host "watcher: status=$(if ($watcherRunning) { 'running' } else { 'stopped' }) pid=$($state.watcher_pid)"
        }
        if ($null -ne $state.liveness_log) {
            Write-Host "liveness: $($state.liveness_log)"
        }
        Write-Host "stdout: $($state.stdout)"
        Write-Host "stderr: $($state.stderr)"
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

    if ($Action -eq "Wait") {
        $started = Get-Date
        while ($true) {
            if (-not (Is-Running ([int]$state.pid))) {
                $exitCode = if (Test-Path $paths.ExitCodePath) { (Get-Content $paths.ExitCodePath -Raw).Trim() } else { "unknown" }
                Write-Host "async formal wait: completed name=$($state.name) pid=$($state.pid) exit_code=$exitCode"
                if ($exitCode -match '^\d+$') {
                    exit ([int]$exitCode)
                }
                exit 1
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
        if (Is-Running ([int]$state.pid)) {
            Stop-Process -Id ([int]$state.pid) -Force
            Write-Host "async formal stop: name=$($state.name) pid=$($state.pid)"
        }
        else {
            Write-Host "async formal stop: process already not running"
        }
        if ($null -ne $state.watcher_pid -and (Is-Running ([int]$state.watcher_pid))) {
            Stop-Process -Id ([int]$state.watcher_pid) -Force
            Write-Host "async formal stop: watcher pid=$($state.watcher_pid)"
        }
        return
    }

    if ($Action -eq "WatchStart") {
        if ($null -ne $state.watcher_pid -and (Is-Running ([int]$state.watcher_pid))) {
            Write-Host "async formal watcher: already running pid=$($state.watcher_pid)"
            return
        }

        $watcherProcess = Start-Watcher -runName $Name -paths $paths -watchPollSeconds $WatchPollSeconds
        $state | Add-Member -NotePropertyName watcher_pid -NotePropertyValue $watcherProcess.Id -Force
        $state | Add-Member -NotePropertyName liveness_log -NotePropertyValue $paths.LivenessPath -Force
        $state | Add-Member -NotePropertyName watcher_stdout -NotePropertyValue $paths.WatcherStdoutPath -Force
        $state | Add-Member -NotePropertyName watcher_stderr -NotePropertyValue $paths.WatcherStderrPath -Force
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
        return
    }
}
finally {
    Pop-Location
}
