param(
    [string]$ProfileScope = "mvp-stabilization-rollup-v66",
    [string]$ReportPath = "docs/evidence/formal/latest_run.md",
    [string]$ReportCsvPath = "docs/evidence/formal/latest_run.csv",
    [string]$ObligationsPath = "docs/evidence/formal/obligations.csv",
    [switch]$RequireKani,
    [switch]$UseWslKani
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

function Convert-ToWslPath([string]$Path) {
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if ($fullPath -match '^([A-Za-z]):\\(.*)$') {
        $drive = $Matches[1].ToLowerInvariant()
        $tail = $Matches[2] -replace '\\', '/'
        return "/mnt/$drive/$tail"
    }

    throw "Unable to convert path to WSL form: $Path"
}

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $reportDir = Split-Path -Parent $ReportPath
    if (-not (Test-Path $reportDir)) {
        New-Item -ItemType Directory -Path $reportDir -Force | Out-Null
    }

    $csvDir = Split-Path -Parent $ReportCsvPath
    if (-not (Test-Path $csvDir)) {
        New-Item -ItemType Directory -Path $csvDir -Force | Out-Null
    }

    if (-not (Test-Path $ObligationsPath)) {
        throw "Missing obligations file: $ObligationsPath"
    }

    $timestampUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

    $targetVersion = 0
    if ($ProfileScope -match 'v(\d+)$') {
        $targetVersion = [int]$Matches[1]
    }

    $allObligations = Import-Csv $ObligationsPath
    $obligations = @()
    foreach ($entry in $allObligations) {
        if ($entry.active -ne "true") {
            continue
        }

        $entryVersion = $targetVersion
        if ($entry.profile -match '^v(\d+)$') {
            $entryVersion = [int]$Matches[1]
        }

        if ($entryVersion -le $targetVersion) {
            $obligations += $entry
        }
    }

    $rows = @()
    $cargoKaniAvailable = $false
    $useWslForKani = $false
    $wslRepoRoot = ""
    $cargoKaniVersion = ""
    $localCargoKaniVersion = ""
    $wslCargoKaniVersion = ""
    $wslKaniAvailable = $false
    $wslCommandAvailable = $null -ne (Get-Command wsl -ErrorAction SilentlyContinue)
    try {
        $localCargoKaniVersion = (& cargo kani --version 2>$null) -join " "
        if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($localCargoKaniVersion)) {
            $cargoKaniAvailable = $true
            $cargoKaniVersion = $localCargoKaniVersion
        }
    }
    catch {
        $localCargoKaniVersion = ""
    }

    if ($wslCommandAvailable) {
        try {
            $wslCargoKaniVersion = (& wsl bash -lc 'source $HOME/.cargo/env && cargo kani --version' 2>$null) -join " "
            if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($wslCargoKaniVersion)) {
                $wslKaniAvailable = $true
            }
        }
        catch {
            $wslCargoKaniVersion = ""
        }
    }

    if ($UseWslKani) {
        if (-not $wslCommandAvailable) {
            throw "formal lane: -UseWslKani requested but wsl is not available on PATH"
        }
        if (-not $wslKaniAvailable) {
            throw "formal lane: -UseWslKani requested but cargo-kani is unavailable in WSL"
        }
        $cargoKaniAvailable = $true
        $useWslForKani = $true
        $cargoKaniVersion = $wslCargoKaniVersion
        $wslRepoRoot = Convert-ToWslPath((Get-Location).Path)
    }

    $kaniRequired = $RequireKani -or ($env:OXVBA_REQUIRE_KANI -eq "1")
    if ($kaniRequired -and -not $cargoKaniAvailable) {
        throw "formal lane: cargo-kani is required but unavailable"
    }

    foreach ($obligation in $obligations) {
        $command = $obligation.command
        $isKaniCommand = $command.Trim().ToLowerInvariant().StartsWith("cargo kani")

        if ($isKaniCommand -and -not $cargoKaniAvailable) {
            $skipNote = if ($wslKaniAvailable -and -not $UseWslKani) {
                "cargo-kani available via WSL; rerun with -UseWslKani (recommended via run-formal-kani-async.ps1)"
            }
            else {
                "cargo-kani not available"
            }
            $rows += [PSCustomObject]@{
                obligation = $obligation.obligation_id
                profile = $obligation.profile
                command = $command
                blocking = $obligation.blocking
                status = "skipped"
                note = $skipNote
                artifact = $obligation.artifact
            }
            continue
        }

        try {
            if ($isKaniCommand -and $useWslForKani) {
                $bashSingleQuoteEscape = [string]::Concat([char]39, [char]34, [char]39, [char]34, [char]39)
                $escapedCommand = $command.Replace("'", $bashSingleQuoteEscape)
                $wslCommand = "source `$HOME/.cargo/env && cd '$wslRepoRoot' && $escapedCommand"
                & wsl bash -lc $wslCommand | Out-Null
                if ($LASTEXITCODE -ne 0) {
                    throw "command exited with code $LASTEXITCODE"
                }
            }
            else {
                Invoke-Expression $command | Out-Null
                if ($LASTEXITCODE -ne 0) {
                    throw "command exited with code $LASTEXITCODE"
                }
            }
            $rows += [PSCustomObject]@{
                obligation = $obligation.obligation_id
                profile = $obligation.profile
                command = $command
                blocking = $obligation.blocking
                status = "pass"
                note = ""
                artifact = $obligation.artifact
            }
        }
        catch {
            $rows += [PSCustomObject]@{
                obligation = $obligation.obligation_id
                profile = $obligation.profile
                command = $command
                blocking = $obligation.blocking
                status = "todo"
                note = ($_.Exception.Message -replace "\|", "/")
                artifact = $obligation.artifact
            }
            Write-Warning "formal lane: obligation $($obligation.obligation_id) did not pass (non-blocking)"
        }
    }

    $rows | Export-Csv -Path $ReportCsvPath -NoTypeInformation

    $lines = @(
        "# Formal Run Report",
        "",
        "- Timestamp (UTC): $timestampUtc",
        "- Profile scope: $ProfileScope",
        "- Overall mode: non-blocking",
        "- Kani required: $($kaniRequired.ToString().ToLowerInvariant())",
        "- Kani execution: $(if ($useWslForKani) { 'wsl' } elseif ($cargoKaniAvailable) { 'local' } elseif ($wslKaniAvailable) { 'deferred-to-wsl-async' } else { 'unavailable' })",
        "- cargo-kani (local): $(if ([string]::IsNullOrWhiteSpace($localCargoKaniVersion)) { 'unavailable' } else { $localCargoKaniVersion })",
        "- cargo-kani (wsl): $(if ([string]::IsNullOrWhiteSpace($wslCargoKaniVersion)) { 'unavailable' } else { $wslCargoKaniVersion })"
    )

    $lines += @(
        "",
        "| Obligation | Profile | Blocking | Status | Command | Artifact | Note |",
        "|---|---|---|---|---|---|---|"
    )

    foreach ($row in $rows) {
        $lines += "| $($row.obligation) | $($row.profile) | $($row.blocking) | $($row.status) | $($row.command) | $($row.artifact) | $($row.note) |"
    }

    Set-Content -Path $ReportPath -Value ($lines -join "`n")

    if (-not $cargoKaniAvailable) {
        if ($wslKaniAvailable -and -not $UseWslKani) {
            Write-Warning "formal lane: cargo-kani detected in WSL; obligations were skipped in this non-WSL run (use -UseWslKani, preferably via run-formal-kani-async.ps1)"
        }
        else {
            Write-Warning "formal lane: cargo-kani not available; obligations recorded as skipped"
        }
    }

    Write-Host "formal run: completed (non-blocking)"
}
finally {
    Pop-Location
}
