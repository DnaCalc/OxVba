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
        throw "COM TestEventServer reference-order oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    . "$PSScriptRoot/lib-com-testeventserver-alt-project.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-testeventserver-reference-order-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "com-testeventserver-reference-order-oracle" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "com_testeventserver_reference_order_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    $generatedRoot = Join-Path $workspaceRoot "temp\generated\com_testeventserver_reference_order\$resolvedRunId"
    $altProjectRoot = Join-Path $generatedRoot "OxVba.TestEventServerAlt"
    New-AltTestEventServerProject -WorkspaceRoot $workspaceRoot -DestinationRoot $altProjectRoot

    & (Join-Path $workspaceRoot "tools/OxVba.TestEventServer/register.ps1") -Configuration Debug -Scope CurrentUser
    & (Join-Path $altProjectRoot "register.ps1") -Configuration Debug -Scope CurrentUser

    $baseTypeLibPath = (Resolve-Path "tools/OxVba.TestEventServer/bin/Debug/net48/OxVba.TestEventServer.tlb").Path
    $altTypeLibPath = (Resolve-Path (Join-Path $altProjectRoot "bin/Debug/net48/OxVba.TestEventServerAlt.tlb")).Path
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

    function Invoke-ReferenceOrderProbe {
        param(
            [object]$Excel,
            [string[]]$TypeLibPaths
        )

        $wb = $null
        try {
            $wb = $Excel.Workbooks.Add()
            foreach ($typeLibPath in $TypeLibPaths) {
                [void]$wb.VBProject.References.AddFromFile($typeLibPath)
            }

            $mod = $wb.VBProject.VBComponents.Add(1)
            $mod.Name = "MainModule"
            $code = @"
Public Function RunProbe()
    Dim obj As TestEventServer
    Set obj = New TestEventServer
    RunProbe = obj.Ping()
End Function
"@
            [void]$mod.CodeModule.AddFromString($code)

            $referenceOrder = @(
                $wb.VBProject.References |
                    Where-Object { $_.Guid -in @(
                        "{E2A30001-0001-0001-0001-000000000001}",
                        "{E2A30001-0001-0001-0001-000000000101}"
                    ) } |
                    ForEach-Object { "{0}:{1}" -f $_.Name, $_.Guid }
            ) -join ";"
            $result = [string]$Excel.Run("RunProbe")
            return @{
                status         = "ok"
                observed       = $result
                reference_order = $referenceOrder
            }
        } catch {
            return @{
                status         = "error"
                observed       = $_.Exception.Message
                reference_order = ""
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
        $probeA = Invoke-ReferenceOrderProbe -Excel $excel -TypeLibPaths @($baseTypeLibPath, $altTypeLibPath)
        $probeB = Invoke-ReferenceOrderProbe -Excel $excel -TypeLibPaths @($altTypeLibPath, $baseTypeLibPath)
    } finally {
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    }

    $cases = @(
        @{
            case_id = "CCT-043-TES-ORDER-001"
            scenario = "Two typelib references, base then alt, unqualified TestEventServer"
            probe = $probeA
            expected = "42"
            expected_prog_id = "OxVba.TestEventServer"
            command = @(
                "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end",
                "early_bound_loaded_basproj_prefers_first_typelib_reference_for_unqualified_testeventserver",
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            )
        }
        @{
            case_id = "CCT-043-TES-ORDER-002"
            scenario = "Two typelib references, alt then base, unqualified TestEventServer"
            probe = $probeB
            expected = "84"
            expected_prog_id = "OxVba.TestEventServerAlt"
            command = @(
                "test", "-p", "oxvba-host", "--test", "com_early_project_end_to_end",
                "early_bound_loaded_basproj_prefers_reversed_first_typelib_reference_for_unqualified_testeventserver",
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            )
        }
    )

    foreach ($case in $cases) {
        $logPath = Join-Path $runDir ($case.case_id + ".log.txt")
        $cmdText = "cargo " + ($case.command -join " ")
        $null = & cargo @($case.command) 2>&1 | Tee-Object -FilePath $logPath
        $exitCode = $LASTEXITCODE
        $oxStatus = if ($exitCode -eq 0) { "ok" } else { "error" }
        $oxObserved = if ($exitCode -eq 0) {
            "compile-selected-progid=$($case.expected_prog_id)"
        } else {
            "lane-failed(exit=$exitCode)"
        }
        $match = if (
            $case.probe.status -eq "ok" `
                -and $exitCode -eq 0 `
                -and $case.probe.observed -eq $case.expected
        ) { "true" } else { "false" }
        Add-Row `
            -CaseId $case.case_id `
            -Scenario $case.scenario `
            -VbaStatus $case.probe.status `
            -VbaObserved $case.probe.observed `
            -OxVbaStatus $oxStatus `
            -OxVbaObserved $oxObserved `
            -Match $match `
            -Notes ("Excel reference-order=" + $case.probe.reference_order + "; OxVba anchor command=" + $cmdText + "; log=" + $logPath)
    }

    $csvPath = Join-Path $runDir "results.csv"
    $summaryPath = Join-Path $runDir "summary.md"
    $rows | Export-Csv -Path $csvPath -NoTypeInformation

    $summary = @(
        "# COM TestEventServer Reference Order Oracle Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Base TypeLib: $baseTypeLibPath",
        "- Alt TypeLib: $altTypeLibPath",
        "- Output CSV: $csvPath",
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
    Set-Content -Path $summaryPath -Value ($summary -join [Environment]::NewLine)

    Write-Host "com-testeventserver-reference-order-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "csv=$csvPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
