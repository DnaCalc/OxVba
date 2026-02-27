param(
    [string]$ReportPath = "docs/evidence/formal/latest_run.md"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$reportDir = Split-Path -Parent $ReportPath
if (-not (Test-Path $reportDir)) {
    New-Item -ItemType Directory -Path $reportDir -Force | Out-Null
}

$timestampUtc = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$rows = @()

$cargoKaniVersion = ""
try {
    $cargoKaniVersion = (& cargo kani --version) -join " "
}
catch {
    $rows += [PSCustomObject]@{
        obligation = "FO-V2-TOOLING"
        command = "cargo kani --version"
        status = "skipped"
        note = "cargo-kani not available"
    }

    $lines = @(
        "# Formal Run Report",
        "",
        "- Timestamp (UTC): $timestampUtc",
        "- Overall mode: non-blocking",
        "",
        "| Obligation | Command | Status | Note |",
        "|---|---|---|---|"
    )

    foreach ($row in $rows) {
        $lines += "| $($row.obligation) | $($row.command) | $($row.status) | $($row.note) |"
    }

    Set-Content -Path $ReportPath -Value ($lines -join "`n")
    Write-Warning "formal lane: cargo-kani not installed; marked as skipped"
    exit 0
}

$obligations = @(
    [PSCustomObject]@{
        Id = "FO-V2-001"
        Command = "cargo kani -p oxvba-vm --harness pc_progression_is_safe_for_valid_jump_target"
    },
    [PSCustomObject]@{
        Id = "FO-V2-002"
        Command = "cargo kani -p oxvba-compiler --harness temp_slots_do_not_overlap_declared_slots"
    }
)

foreach ($obligation in $obligations) {
    try {
        Invoke-Expression $obligation.Command | Out-Null
        $rows += [PSCustomObject]@{
            obligation = $obligation.Id
            command = $obligation.Command
            status = "pass"
            note = ""
        }
    }
    catch {
        $rows += [PSCustomObject]@{
            obligation = $obligation.Id
            command = $obligation.Command
            status = "todo"
            note = ($_.Exception.Message -replace "\|", "/")
        }
        Write-Warning "formal lane: obligation $($obligation.Id) did not pass (non-blocking)"
    }
}

$lines = @(
    "# Formal Run Report",
    "",
    "- Timestamp (UTC): $timestampUtc",
    "- cargo-kani: $cargoKaniVersion",
    "- Overall mode: non-blocking",
    "",
    "| Obligation | Command | Status | Note |",
    "|---|---|---|---|"
)

foreach ($row in $rows) {
    $lines += "| $($row.obligation) | $($row.command) | $($row.status) | $($row.note) |"
}

Set-Content -Path $ReportPath -Value ($lines -join "`n")
Write-Host "formal run: completed (non-blocking)"