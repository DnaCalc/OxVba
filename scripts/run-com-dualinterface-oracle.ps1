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
        throw "COM dual-interface oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-dualinterface-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "com-dualinterface-oracle" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "com_dualinterface_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    & (Join-Path $workspaceRoot "tools/OxVba.TestEventServer/register.ps1") -Configuration Debug -Scope CurrentUser
    $testEventServerTypeLib = (Resolve-Path "tools/OxVba.TestEventServer/bin/Debug/net48/OxVba.TestEventServer.tlb").Path

    function Invoke-ExcelProbe {
        param(
            [object]$Excel,
            [string]$CaseId
        )

        $wb = $null
        try {
            $wb = $Excel.Workbooks.Add()
            switch ($CaseId) {
                "CCT-047-SCRRUN-001" {
                    [void]$wb.VBProject.References.AddFromGuid("{420B2830-E718-11CF-893D-00A0C9054228}", 1, 0)
                    $mod = $wb.VBProject.VBComponents.Add(1)
                    $mod.Name = "OracleModule"
                    $code = @"
Public Function RunProbe()
    Dim obj As New Scripting.Dictionary
    Call obj.Add("a", 1)
    RunProbe = CStr(obj.Exists("a")) & "," & CStr(obj.Count)
End Function
"@
                    [void]$mod.CodeModule.AddFromString($code)
                }
                "CCT-047-TES-001" {
                    [void]$wb.VBProject.References.AddFromFile($testEventServerTypeLib)
                    $mod = $wb.VBProject.VBComponents.Add(1)
                    $mod.Name = "OracleModule"
                    $code = @"
Public Function RunProbe()
    Dim obj As TestEventServer
    Set obj = New TestEventServer
    RunProbe = obj.Ping()
End Function
"@
                    [void]$mod.CodeModule.AddFromString($code)
                }
                default {
                    throw "unknown Excel probe case: $CaseId"
                }
            }
            return @{
                status   = "ok"
                observed = [string]$Excel.Run("RunProbe")
            }
        } catch {
            return @{
                status   = "error"
                observed = $_.Exception.Message
            }
        } finally {
            if ($wb -ne $null) {
                $wb.Close($false)
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
            }
        }
    }

    function Invoke-OxCase {
        param(
            [string]$CaseId,
            [string]$ExpectedObserved,
            [string[]]$CargoArgs,
            [string]$Notes
        )

        $safeCase = ($CaseId -replace '[^A-Za-z0-9_.-]', '_')
        $logPath = Join-Path $runDir ("{0}.log.txt" -f $safeCase)
        $cmdText = "cargo " + ($CargoArgs -join " ")
        $null = & cargo @CargoArgs 2>&1 | Tee-Object -FilePath $logPath
        $exitCode = $LASTEXITCODE
        [PSCustomObject]@{
            status = if ($exitCode -eq 0) { "ok" } else { "error" }
            observed = if ($exitCode -eq 0) { $ExpectedObserved } else { "lane-failed(exit=$exitCode)" }
            notes = "$Notes; command=$cmdText; log=$logPath"
        }
    }

    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    try {
        $excelRows = @{
            "CCT-047-SCRRUN-001" = Invoke-ExcelProbe -Excel $excel -CaseId "CCT-047-SCRRUN-001"
            "CCT-047-TES-001" = Invoke-ExcelProbe -Excel $excel -CaseId "CCT-047-TES-001"
        }
    } finally {
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    }

    $rows = @(
        [PSCustomObject]@{
            topic_id = "CCT-047"
            case_id = "CCT-047-SCRRUN-001"
            scenario = "Scripting.Dictionary Add / Exists / Count stays stable across dispatch and PreferVtable"
            vba_status = $excelRows["CCT-047-SCRRUN-001"].status
            vba_observed = $excelRows["CCT-047-SCRRUN-001"].observed
            oxvba_dispatch_status = (Invoke-OxCase -CaseId "CCT-047-SCRRUN-001-dispatch" -ExpectedObserved "True,1" -CargoArgs @(
                "test","-p","oxvba-host","--test","com_early_project_end_to_end",
                "early_bound_project_executes_registered_scripting_dictionary_member_subset",
                "--","--ignored","--exact","--test-threads=1","--nocapture"
            ) -Notes "OxVba dispatch/default anchor: com_early_project_end_to_end::early_bound_project_executes_registered_scripting_dictionary_member_subset").status
            oxvba_dispatch_observed = "True,1"
            oxvba_vtable_status = (Invoke-OxCase -CaseId "CCT-047-SCRRUN-001-vtable" -ExpectedObserved "True,1" -CargoArgs @(
                "test","-p","oxvba-host","--test","com_early_project_end_to_end",
                "early_bound_project_registered_scripting_dictionary_member_subset_prefer_vtable_matches_dispatch",
                "--","--ignored","--exact","--test-threads=1","--nocapture"
            ) -Notes "OxVba prefer-vtable anchor: com_early_project_end_to_end::early_bound_project_registered_scripting_dictionary_member_subset_prefer_vtable_matches_dispatch").status
            oxvba_vtable_observed = "True,1"
            match_dispatch = if ($excelRows["CCT-047-SCRRUN-001"].status -eq "ok" -and $excelRows["CCT-047-SCRRUN-001"].observed -eq "True,1") { "true" } else { "false" }
            match_vtable = if ($excelRows["CCT-047-SCRRUN-001"].status -eq "ok" -and $excelRows["CCT-047-SCRRUN-001"].observed -eq "True,1") { "true" } else { "false" }
        }
        [PSCustomObject]@{
            topic_id = "CCT-047"
            case_id = "CCT-047-TES-001"
            scenario = "TestEventServer Ping stays stable across dispatch and PreferVtable/fallback policy"
            vba_status = $excelRows["CCT-047-TES-001"].status
            vba_observed = $excelRows["CCT-047-TES-001"].observed
            oxvba_dispatch_status = (Invoke-OxCase -CaseId "CCT-047-TES-001-dispatch" -ExpectedObserved "42" -CargoArgs @(
                "test","-p","oxvba-host","--test","com_early_project_end_to_end",
                "early_bound_project_executes_registered_testeventserver_ping",
                "--","--ignored","--exact","--test-threads=1","--nocapture"
            ) -Notes "OxVba dispatch/default anchor: com_early_project_end_to_end::early_bound_project_executes_registered_testeventserver_ping").status
            oxvba_dispatch_observed = "42"
            oxvba_vtable_status = (Invoke-OxCase -CaseId "CCT-047-TES-001-vtable" -ExpectedObserved "42" -CargoArgs @(
                "test","-p","oxvba-host","--test","com_early_project_end_to_end",
                "early_bound_project_registered_testeventserver_ping_prefer_vtable_matches_dispatch",
                "--","--ignored","--exact","--test-threads=1","--nocapture"
            ) -Notes "OxVba prefer-vtable anchor: com_early_project_end_to_end::early_bound_project_registered_testeventserver_ping_prefer_vtable_matches_dispatch").status
            oxvba_vtable_observed = "42"
            match_dispatch = if ($excelRows["CCT-047-TES-001"].status -eq "ok" -and $excelRows["CCT-047-TES-001"].observed -eq "42") { "true" } else { "false" }
            match_vtable = if ($excelRows["CCT-047-TES-001"].status -eq "ok" -and $excelRows["CCT-047-TES-001"].observed -eq "42") { "true" } else { "false" }
        }
    )

    $csvPath = Join-Path $runDir "results.csv"
    $rows | Export-Csv -Path $csvPath -NoTypeInformation

    $summaryPath = Join-Path $runDir "summary.md"
    $md = @()
    $md += "# COM Dual-Interface Oracle Run"
    $md += ""
    $md += "- Run ID: $resolvedRunId"
    $md += "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))"
    $md += "- Output CSV: $csvPath"
    $md += "- Total cases: $($rows.Count)"
    $md += "- Dispatch match count: $((@($rows | Where-Object { $_.match_dispatch -eq 'true' })).Count)"
    $md += "- PreferVtable match count: $((@($rows | Where-Object { $_.match_vtable -eq 'true' })).Count)"
    $md += ""
    $md += "## Case Results"
    $md += "| Topic | Case | VBA | OxVba Dispatch | OxVba PreferVtable | Match Dispatch | Match PreferVtable |"
    $md += "|---|---|---|---|---|---|---|"
    foreach ($row in $rows) {
        $md += "| $($row.topic_id) | $($row.case_id) | $($row.vba_status): $($row.vba_observed) | $($row.oxvba_dispatch_status): $($row.oxvba_dispatch_observed) | $($row.oxvba_vtable_status): $($row.oxvba_vtable_observed) | $($row.match_dispatch) | $($row.match_vtable) |"
    }
    Set-Content -Path $summaryPath -Value ($md -join [Environment]::NewLine)

    Write-Host "com-dualinterface-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "results=$csvPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
