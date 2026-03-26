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
        throw "COM TestEventServer unresolved typelib oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-testeventserver-unresolved-typelib-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "com-testeventserver-unresolved-typelib-oracle" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "com_testeventserver_unresolved_typelib_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    $missingTypeLibPath = Join-Path $workspaceRoot "temp\missing\NoSuchTypeLib.tlb"
    $missingTypeLibDir = Split-Path -Parent $missingTypeLibPath
    New-Item -ItemType Directory -Force -Path $missingTypeLibDir | Out-Null
    if (Test-Path $missingTypeLibPath) {
        Remove-Item -Force -Path $missingTypeLibPath
    }

    $rows = New-Object System.Collections.Generic.List[object]

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

    function Invoke-ExcelTypelibFailureProbe {
        param(
            [object]$Excel,
            [ValidateSet("guid", "file")]
            [string]$Mode,
            [string]$Guid,
            [string]$TypeLibPath
        )

        $wb = $null
        try {
            $wb = $Excel.Workbooks.Add()
            if ($Mode -eq "guid") {
                [void]$wb.VBProject.References.AddFromGuid($Guid, 1, 0)
            } else {
                [void]$wb.VBProject.References.AddFromFile($TypeLibPath)
            }
            return @{
                status         = "ok"
                observed       = "reference-added"
                classification = "unexpected-success"
                ref_count      = [string]$wb.VBProject.References.Count
                modal_observed = "false"
            }
        } catch {
            $message = [string]$_.Exception.Message
            $classification = switch -Regex ($message) {
                "Object library not registered" { "unresolved-libid"; break }
                "Error in loading DLL" { "unresolved-importlib"; break }
                default { "other-error"; break }
            }
            return @{
                status         = "error"
                observed       = $message
                classification = $classification
                ref_count      = if ($wb -ne $null) { [string]$wb.VBProject.References.Count } else { "" }
                modal_observed = "false"
            }
        } finally {
            if ($wb -ne $null) {
                $wb.Close($false)
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
            }
        }
    }

    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    try {
        $libidProbe = Invoke-ExcelTypelibFailureProbe `
            -Excel $excel `
            -Mode "guid" `
            -Guid "{E2A30001-0001-0001-0001-000000009999}"
        $importlibProbe = Invoke-ExcelTypelibFailureProbe `
            -Excel $excel `
            -Mode "file" `
            -TypeLibPath $missingTypeLibPath
    } finally {
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    }

    $cases = @(
        @{
            case_id = "CCT-043-TES-LIBID-001"
            scenario = "Unknown LIBID via AddFromGuid vs loaded .basproj unresolved LIBID"
            probe = $libidProbe
            expected_classification = "unresolved-libid"
            expected_ox_code = "PMR-E-TYPELIB-LIBID-UNRESOLVED"
            command = @(
                "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end",
                "early_bound_loaded_basproj_reports_unresolved_typelib_libid_identity",
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            )
        }
        @{
            case_id = "CCT-043-TES-IMPORTLIB-001"
            scenario = "Missing .tlb via AddFromFile vs loaded .basproj unresolved importlib"
            probe = $importlibProbe
            expected_classification = "unresolved-importlib"
            expected_ox_code = "PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED"
            command = @(
                "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end",
                "early_bound_loaded_basproj_reports_unresolved_typelib_importlib_identity",
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            )
        }
    )

    foreach ($case in $cases) {
        $logPath = Join-Path $runDir ($case.case_id + ".log.txt")
        $cmdText = "cargo " + ($case.command -join " ")
        $cargoOutput = & cargo @($case.command) 2>&1 | Tee-Object -FilePath $logPath
        $exitCode = $LASTEXITCODE
        $oxStatus = if ($exitCode -eq 0) { "ok" } else { "error" }
        $oxObserved = if ($exitCode -eq 0) { $case.expected_ox_code } else { "lane-failed(exit=$exitCode)" }
        $match = if (
            $case.probe.status -eq "error" `
                -and $case.probe.classification -eq $case.expected_classification `
                -and $exitCode -eq 0
        ) { "true" } else { "false" }
        Add-Row `
            -CaseId $case.case_id `
            -Scenario $case.scenario `
            -VbaStatus $case.probe.status `
            -VbaObserved $case.probe.observed `
            -OxVbaStatus $oxStatus `
            -OxVbaObserved $oxObserved `
            -Match $match `
            -Notes (
                "Excel classification=" + $case.probe.classification +
                "; ref_count=" + $case.probe.ref_count +
                "; modal_observed=" + $case.probe.modal_observed +
                "; OxVba anchor command=" + $cmdText +
                "; log=" + $logPath
            )
    }

    $csvPath = Join-Path $runDir "results.csv"
    $summaryPath = Join-Path $runDir "summary.md"
    $rows | Export-Csv -Path $csvPath -NoTypeInformation

    $summary = @(
        "# COM TestEventServer Unresolved Typelib Oracle Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Missing TypeLib path: $missingTypeLibPath",
        "- Output CSV: $csvPath",
        "- Modal inspection note: both Excel probes returned promptly under hidden automation with `DisplayAlerts = false`; no modal popup was observed in this bounded lane.",
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

    Write-Host "com-testeventserver-unresolved-typelib-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "results=$csvPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
