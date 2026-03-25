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
        throw "COM TestEventServer oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-testeventserver-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "com-testeventserver-oracle" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "com_testeventserver_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    $probeDir = Join-Path $runDir "probe"
    $probe = & "$PSScriptRoot/run-com-testeventserver-typelib-probe.ps1" -OutputRoot $probeDir -RunId $resolvedRunId
    $probeCsv = Join-Path $probeDir "com_testeventserver_typelib_probe_$resolvedRunId\results.csv"
    if (-not (Test-Path $probeCsv)) {
        throw "probe CSV not found at $probeCsv"
    }
    $probeRows = Import-Csv $probeCsv

    function Get-ProbeRow {
        param([string]$CaseId)
        $row = $probeRows | Where-Object { $_.case_id -eq $CaseId } | Select-Object -First 1
        if ($null -eq $row) {
            throw "probe row not found for case $CaseId"
        }
        return $row
    }

    function Invoke-OxCase {
        param(
            [string]$CaseId,
            [string]$Scenario,
            [string]$ExpectedObserved,
            [string[]]$CargoArgs,
            [string]$Notes
        )

        $safeCaseId = ($CaseId -replace '[^A-Za-z0-9_.-]', '_')
        $logPath = Join-Path $runDir ("{0}.log.txt" -f $safeCaseId)
        $cmdText = "cargo " + ($CargoArgs -join " ")
        $null = & cargo @CargoArgs 2>&1 | Tee-Object -FilePath $logPath
        $exitCode = $LASTEXITCODE
        [PSCustomObject]@{
            case_id = $CaseId
            scenario = $Scenario
            status = if ($exitCode -eq 0) { "ok" } else { "error" }
            observed = if ($exitCode -eq 0) { $ExpectedObserved } else { "lane-failed(exit=$exitCode)" }
            notes = "$Notes; command=$cmdText; log=$logPath"
        }
    }

    $oxRows = @(
        Invoke-OxCase `
            -CaseId "CCT-027-TES-001" `
            -Scenario "AddFromFile + New TestEventServer + Ping()" `
            -ExpectedObserved "42" `
            -CargoArgs @(
                "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end",
                "early_bound_project_executes_registered_testeventserver_ping",
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            ) `
            -Notes "OxVba anchor: com_early_project_end_to_end::early_bound_project_executes_registered_testeventserver_ping"
        Invoke-OxCase `
            -CaseId "CCT-027-TES-002" `
            -Scenario "AddFromFile + WithEvents TestEventServer source interface" `
            -ExpectedObserved "7" `
            -CargoArgs @(
                "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end",
                "early_bound_project_registered_testeventserver_withevents_callback_preserves_value_payload",
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            ) `
            -Notes "OxVba anchor: com_early_project_end_to_end::early_bound_project_registered_testeventserver_withevents_callback_preserves_value_payload (encodes payload 7 as runtime error 7007)"
    )

    $rows = foreach ($oxRow in $oxRows) {
        $probeRow = Get-ProbeRow -CaseId $oxRow.case_id
        [PSCustomObject]@{
            topic_id = "CCT-027"
            case_id = $oxRow.case_id
            scenario = $oxRow.scenario
            vba_status = $probeRow.status
            vba_observed = $probeRow.observed
            oxvba_status = $oxRow.status
            oxvba_observed = $oxRow.observed
            match = if ($probeRow.status -eq $oxRow.status -and $probeRow.observed -eq $oxRow.observed) { "true" } else { "false" }
            notes = $oxRow.notes
        }
    }

    $csvPath = Join-Path $runDir "results.csv"
    $rows | Export-Csv -Path $csvPath -NoTypeInformation

    $summaryPath = Join-Path $runDir "summary.md"
    $md = @()
    $md += "# COM TestEventServer Oracle Run"
    $md += ""
    $md += "- Run ID: $resolvedRunId"
    $md += "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))"
    $md += "- Probe CSV: $probeCsv"
    $md += "- Output CSV: $csvPath"
    $md += "- Total cases: $($rows.Count)"
    $md += "- Match count: $((@($rows | Where-Object { $_.match -eq 'true' })).Count)"
    $md += "- Mismatch count: $((@($rows | Where-Object { $_.match -ne 'true' })).Count)"
    $md += ""
    $md += "## Case Results"
    $md += "| Topic | Case | VBA | OxVba | Match | Notes |"
    $md += "|---|---|---|---|---|---|"
    foreach ($row in $rows) {
        $vbaCell = "$($row.vba_status): $($row.vba_observed)"
        $oxCell = "$($row.oxvba_status): $($row.oxvba_observed)"
        $md += "| $($row.topic_id) | $($row.case_id) | $vbaCell | $oxCell | $($row.match) | $($row.notes) |"
    }
    Set-Content -Path $summaryPath -Value ($md -join [Environment]::NewLine)

    Write-Host "com-testeventserver-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "csv=$csvPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
