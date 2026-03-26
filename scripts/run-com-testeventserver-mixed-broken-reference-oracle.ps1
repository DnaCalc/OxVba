param(
    [string]$OutputRoot = "docs/evidence/conformance/oracle_captures",
    [string]$RunId = "",
    [switch]$NoArtifacts,
    [int]$ProbeTimeoutSeconds = 15
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not $IsWindows) {
        throw "COM TestEventServer mixed broken-reference oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-testeventserver-mixed-broken-reference-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "com-testeventserver-mixed-broken-reference-oracle" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "com_testeventserver_mixed_broken_reference_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    $baseTypeLibPath = (Resolve-Path "tools/OxVba.TestEventServer/bin/Debug/net48/OxVba.TestEventServer.tlb").Path
    $altTypeLibPath = (Get-ChildItem -Path "temp\generated\com_testeventserver_reference_order" -Recurse -Filter "OxVba.TestEventServerAlt.tlb" |
        Sort-Object `
            @{ Expression = "LastWriteTimeUtc"; Descending = $true }, `
            @{ Expression = "FullName"; Descending = $false } |
        Select-Object -First 1 -ExpandProperty FullName)
    if (-not $altTypeLibPath) {
        throw "alt TestEventServer typelib not found under temp\\generated\\com_testeventserver_reference_order"
    }

    $rows = New-Object System.Collections.Generic.List[object]
    $probeScriptPath = Join-Path $runDir "_mixed_broken_reference_probe.ps1"
    $probeScript = @'
param(
    [string]$FirstTypeLibPath,
    [string]$SecondTypeLibPath,
    [string]$StatePath
)

$ErrorActionPreference = "Stop"

$root = Join-Path $env:TEMP ("oxvba_mixed_broken_ref_" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $root | Out-Null
$firstCopy = Join-Path $root ([System.IO.Path]::GetFileName($FirstTypeLibPath))
$secondCopy = Join-Path $root ([System.IO.Path]::GetFileName($SecondTypeLibPath))
$workbookPath = Join-Path $root "probe.xlsm"
Copy-Item $FirstTypeLibPath $firstCopy -Force
Copy-Item $SecondTypeLibPath $secondCopy -Force
$code = "Public Function RunProbe()`n    Dim obj As TestEventServer`n    Set obj = New TestEventServer`n    RunProbe = obj.Ping()`nEnd Function`n"

$excel = $null
$wb = $null
$reopened = $null
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false

    $wb = $excel.Workbooks.Add()
    [void]$wb.VBProject.References.AddFromFile($firstCopy)
    [void]$wb.VBProject.References.AddFromFile($secondCopy)
    $mod = $wb.VBProject.VBComponents.Add(1)
    $mod.Name = "MainModule"
    [void]$mod.CodeModule.AddFromString($code)
    $wb.SaveAs($workbookPath, 52)
    $wb.Close($false)
    [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
    $wb = $null

    Rename-Item $firstCopy ($firstCopy + ".missing")

    $reopened = $excel.Workbooks.Open($workbookPath)
    $refs = @(
        $reopened.VBProject.References |
            Where-Object {
                $_.Guid -in @(
                    "{E2A30001-0001-0001-0001-000000000001}",
                    "{E2A30001-0001-0001-0001-000000000101}"
                )
            } |
            ForEach-Object { "name={0};guid={1};broken={2}" -f $_.Name, $_.Guid, [string]$_.IsBroken }
    )
    @{ stage = "reopened"; refs = $refs } | ConvertTo-Json -Compress | Set-Content -Path $StatePath

    try {
        $result = [string]$excel.Run("RunProbe")
        @{ stage = "completed"; refs = $refs; run = $result } | ConvertTo-Json -Compress | Set-Content -Path $StatePath
    } catch {
        @{ stage = "run_error"; refs = $refs; run_error = $_.Exception.Message } | ConvertTo-Json -Compress | Set-Content -Path $StatePath
    }
} finally {
    if ($reopened -ne $null) {
        $reopened.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($reopened)
    }
    if ($wb -ne $null) {
        $wb.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
    }
    if ($excel -ne $null) {
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    }
}
'@
    Set-Content -Path $probeScriptPath -Value $probeScript -Encoding UTF8

    function Add-Row {
        param(
            [string]$CaseId,
            [string]$Scenario,
            [string]$VbaStatus,
            [string]$VbaObserved,
            [string]$OxVbaStatus,
            [string]$OxVbaObserved,
            [string]$Match,
            [string]$Notes
        )

        $rows.Add([PSCustomObject]@{
                topic_id        = "CCT-043"
                case_id         = $CaseId
                scenario        = $Scenario
                vba_status      = $VbaStatus
                vba_observed    = $VbaObserved
                oxvba_status    = $OxVbaStatus
                oxvba_observed  = $OxVbaObserved
                match           = $Match
                notes           = $Notes
            }) | Out-Null
    }

    function Invoke-MixedBrokenReferenceProbe {
        param(
            [string]$CaseId,
            [string]$FirstTypeLibPath,
            [string]$SecondTypeLibPath
        )

        $baselineExcelPids = @(Get-Process EXCEL -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
        $statePath = Join-Path $runDir ($CaseId + ".vba-state.json")
        $stdoutPath = Join-Path $runDir ($CaseId + ".probe.stdout.txt")
        $stderrPath = Join-Path $runDir ($CaseId + ".probe.stderr.txt")
        if (Test-Path $statePath) {
            Remove-Item -Force -Path $statePath
        }
        $probeProcess = Start-Process `
            -FilePath (Get-Command pwsh).Source `
            -ArgumentList @(
                "-NoProfile",
                "-NonInteractive",
                "-File",
                $probeScriptPath,
                $FirstTypeLibPath,
                $SecondTypeLibPath,
                $statePath
            ) `
            -RedirectStandardOutput $stdoutPath `
            -RedirectStandardError $stderrPath `
            -PassThru `
            -WindowStyle Hidden

        $completed = $probeProcess.WaitForExit($ProbeTimeoutSeconds * 1000)
        $state = $null
        if (Test-Path $statePath) {
            $state = Get-Content $statePath -Raw | ConvertFrom-Json
        }
        if (-not $completed) {
            Stop-Process -Id $probeProcess.Id -Force -ErrorAction SilentlyContinue
            $newExcelPids = @(Get-Process EXCEL -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
            $orphanedPids = $newExcelPids | Where-Object { $_ -notin $baselineExcelPids }
            $orphanedWindowTitles = @(
                Get-Process -Id $orphanedPids -ErrorAction SilentlyContinue |
                    Where-Object { -not [string]::IsNullOrWhiteSpace($_.MainWindowTitle) } |
                    ForEach-Object { $_.MainWindowTitle }
            )
            foreach ($orphanedPid in $orphanedPids) {
                Stop-Process -Id $orphanedPid -Force -ErrorAction SilentlyContinue
            }
            return @{
                status         = "timeout"
                observed       = "execution-did-not-return-within-${ProbeTimeoutSeconds}s"
                refs           = if ($state) { ($state.refs -join "|") } else { "" }
                stage          = if ($state) { [string]$state.stage } else { "" }
                modal_observed = if ($state -and $state.stage -eq "reopened") { "possible" } else { "unknown" }
                probe_exit_code = ""
                window_titles  = ($orphanedWindowTitles -join "|")
            }
        }

        $probeExitCode = [string]$probeProcess.ExitCode
        if ($null -eq $state) {
            return @{
                status         = "error"
                observed       = "no-state-captured(exit=$probeExitCode)"
                refs           = ""
                stage          = ""
                modal_observed = "unknown"
                probe_exit_code = $probeExitCode
                window_titles  = ""
            }
        }
        if ($state.stage -eq "completed") {
            return @{
                status         = "ok"
                observed       = [string]$state.run
                refs           = ($state.refs -join "|")
                stage          = [string]$state.stage
                modal_observed = "false"
                probe_exit_code = $probeExitCode
                window_titles  = ""
            }
        }
        return @{
            status         = "error"
            observed       = [string]$state.run_error
            refs           = ($state.refs -join "|")
            stage          = [string]$state.stage
            modal_observed = "false"
            probe_exit_code = $probeExitCode
            window_titles  = ""
        }
    }

    $cases = @(
        @{
            case_id = "CCT-043-TES-MIXED-001"
            scenario = "Saved workbook with base then alt references; first typelib removed before reopen"
            first = $baseTypeLibPath
            second = $altTypeLibPath
            command = @(
                "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end",
                "early_bound_loaded_basproj_mixed_broken_base_then_valid_alt_executes_alt",
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            )
            expected_ox = "84"
        }
        @{
            case_id = "CCT-043-TES-MIXED-002"
            scenario = "Saved workbook with alt then base references; first typelib removed before reopen"
            first = $altTypeLibPath
            second = $baseTypeLibPath
            command = @(
                "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end",
                "early_bound_loaded_basproj_mixed_broken_alt_then_valid_base_executes_base",
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            )
            expected_ox = "42"
        }
    )

    foreach ($case in $cases) {
        $probe = Invoke-MixedBrokenReferenceProbe -CaseId $case.case_id -FirstTypeLibPath $case.first -SecondTypeLibPath $case.second
        $logPath = Join-Path $runDir ($case.case_id + ".log.txt")
        $cmdText = "cargo " + ($case.command -join " ")
        $null = & cargo @($case.command) 2>&1 | Tee-Object -FilePath $logPath
        $exitCode = $LASTEXITCODE
        $oxStatus = if ($exitCode -eq 0) { "ok" } else { "error" }
        $oxObserved = if ($exitCode -eq 0) { $case.expected_ox } else { "lane-failed(exit=$exitCode)" }
        $match = if (
            $probe.status -eq "ok" `
                -and $exitCode -eq 0 `
                -and $probe.observed -eq $case.expected_ox
        ) { "true" } else { "false" }
        Add-Row `
            -CaseId $case.case_id `
            -Scenario $case.scenario `
            -VbaStatus $probe.status `
            -VbaObserved $probe.observed `
            -OxVbaStatus $oxStatus `
            -OxVbaObserved $oxObserved `
            -Match $match `
            -Notes (
                "Excel stage=" + $probe.stage +
                "; refs=" + $probe.refs +
                "; modal_observed=" + $probe.modal_observed +
                "; window_titles=" + $probe.window_titles +
                "; probe_exit_code=" + $probe.probe_exit_code +
                "; OxVba anchor command=" + $cmdText +
                "; log=" + $logPath
            )
    }

    $csvPath = Join-Path $runDir "results.csv"
    $summaryPath = Join-Path $runDir "summary.md"
    $rows | Export-Csv -Path $csvPath -NoTypeInformation

    $summary = @(
        "# COM TestEventServer Mixed Broken Reference Oracle Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Base TypeLib: $baseTypeLibPath",
        "- Alt TypeLib: $altTypeLibPath",
        "- Probe timeout seconds: $ProbeTimeoutSeconds",
        "- Output CSV: $csvPath",
        "- Modal inspection note: timeout after successful reopen is treated as likely blocked/modal Excel behavior; the runner records the last captured stage and reference state before forcing cleanup.",
        "",
        "- Total cases: $($rows.Count)",
        "- Match count: $(($rows | Where-Object { $_.match -eq 'true' }).Count)",
        "- Mismatch count: $(($rows | Where-Object { $_.match -ne 'true' }).Count)",
        "",
        "## Case Results",
        "| Topic | Case | VBA | OxVba | Match | Notes |",
        "|---|---|---|---|---|---|"
    )
    foreach ($row in $rows) {
        $summary += "| $($row.topic_id) | $($row.case_id) | $($row.vba_status): $($row.vba_observed) | $($row.oxvba_status): $($row.oxvba_observed) | $($row.match) | $($row.notes) |"
    }
    Set-Content -Path $summaryPath -Value ($summary -join "`n")

    Write-Host "com-testeventserver-mixed-broken-reference-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "results=$csvPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
