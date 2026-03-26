param(
    [string]$OutputRoot = "docs/evidence/conformance/oracle_captures",
    [string]$RunId = "",
    [switch]$NoArtifacts
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not $IsWindows) {
        throw "COM TestEventServer basproj oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-testeventserver-basproj-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "com-testeventserver-basproj-oracle" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "com_testeventserver_basproj_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    $probeDir = Join-Path $runDir "probe"
    $null = & "$PSScriptRoot/run-com-testeventserver-typelib-probe.ps1" -OutputRoot $probeDir -RunId $resolvedRunId
    $probeCsv = Join-Path $probeDir "com_testeventserver_typelib_probe_$resolvedRunId\results.csv"
    if (-not (Test-Path $probeCsv)) {
        throw "probe CSV not found at $probeCsv"
    }
    $probeRows = Import-Csv $probeCsv
    $probeRow = $probeRows | Where-Object { $_.case_id -eq "CCT-027-TES-001" } | Select-Object -First 1
    if ($null -eq $probeRow) {
        throw "probe row not found for CCT-027-TES-001"
    }

    $logPath = Join-Path $runDir "CCT-043-TES-001.log.txt"
    $cargoArgs = @(
        "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end",
        "early_bound_loaded_basproj_executes_registered_testeventserver_ping",
        "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
    )
    $cmdText = "cargo " + ($cargoArgs -join " ")
    $null = & cargo @cargoArgs 2>&1 | Tee-Object -FilePath $logPath
    $exitCode = $LASTEXITCODE

    $row = [PSCustomObject]@{
        topic_id = "CCT-043"
        case_id = "CCT-043-TES-001"
        scenario = "Loaded .basproj file-backed typelib reference executes New TestEventServer + Ping()"
        vba_status = $probeRow.status
        vba_observed = $probeRow.observed
        oxvba_status = if ($exitCode -eq 0) { "ok" } else { "error" }
        oxvba_observed = if ($exitCode -eq 0) { "42" } else { "lane-failed(exit=$exitCode)" }
        match = if ($probeRow.status -eq "ok" -and $exitCode -eq 0 -and $probeRow.observed -eq "42") { "true" } else { "false" }
        notes = "Excel baseline reuses file-backed .tlb AddFromFile probe CCT-027-TES-001; OxVba anchor: com_early_project_end_to_end::early_bound_loaded_basproj_executes_registered_testeventserver_ping; command=$cmdText; log=$logPath"
    }

    $csvPath = Join-Path $runDir "results.csv"
    $summaryPath = Join-Path $runDir "summary.md"
    @($row) | Export-Csv -Path $csvPath -NoTypeInformation

    $summary = @(
        "# COM TestEventServer BasProj Oracle Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Probe CSV: $probeCsv",
        "- Output CSV: $csvPath",
        "- Total cases: 1",
        "- Match count: $(if ($row.match -eq 'true') { 1 } else { 0 })",
        "- Mismatch count: $(if ($row.match -eq 'true') { 0 } else { 1 })",
        "",
        "## Case Results",
        "| Topic | Case | VBA | OxVba | Match | Notes |",
        "|---|---|---|---|---|---|",
        "| $($row.topic_id) | $($row.case_id) | $($row.vba_status): $($row.vba_observed) | $($row.oxvba_status): $($row.oxvba_observed) | $($row.match) | $($row.notes) |"
    )
    Set-Content -Path $summaryPath -Value ($summary -join [Environment]::NewLine)

    Write-Host "com-testeventserver-basproj-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "csv=$csvPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
