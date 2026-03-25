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
        throw "COM TestEventServer marshaling oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-testeventserver-marshaling-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "com-testeventserver-marshaling-oracle" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "com_testeventserver_marshaling_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    & (Join-Path $workspaceRoot "tools/OxVba.TestEventServer/register.ps1") -Configuration Debug -Scope CurrentUser

    function Invoke-ExcelCase {
        param(
            [string]$CaseId,
            [string]$Scenario,
            [scriptblock]$Populate
        )

        $wb = $null
        try {
            $wb = $script:excel.Workbooks.Add()
            $mod = $wb.VBProject.VBComponents.Add(1)
            $mod.Name = "MainModule"
            & $Populate $mod
            $result = $script:excel.Run("RunProbe")
            [PSCustomObject]@{
                case_id = $CaseId
                scenario = $Scenario
                status = "ok"
                observed = [string]$result
            }
        } catch {
            [PSCustomObject]@{
                case_id = $CaseId
                scenario = $Scenario
                status = "error"
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
            [string]$Scenario,
            [string]$ExpectedObserved,
            [string]$TestName
        )

        $logPath = Join-Path $runDir ("{0}.log.txt" -f ($CaseId -replace '[^A-Za-z0-9_.-]', '_'))
        $previousProgId = $env:OXVBA_REGISTERED_COM_PROGID
        $hadPreviousProgId = Test-Path Env:OXVBA_REGISTERED_COM_PROGID
        try {
            $env:OXVBA_REGISTERED_COM_PROGID = "OxVba.TestEventServer"
            $cargoArgs = @(
                "test", "-p", "oxvba-host", "--test", "com_client_registered_lane",
                $TestName,
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            )
            $cmdText = "cargo " + ($cargoArgs -join " ")
            $null = & cargo @cargoArgs 2>&1 | Tee-Object -FilePath $logPath
            $exitCode = $LASTEXITCODE
            [PSCustomObject]@{
                case_id = $CaseId
                scenario = $Scenario
                status = if ($exitCode -eq 0) { "ok" } else { "error" }
                observed = if ($exitCode -eq 0) { $ExpectedObserved } else { "lane-failed(exit=$exitCode)" }
                notes = "OxVba anchor: com_client_registered_lane::$TestName; command=$cmdText; log=$logPath"
            }
        } finally {
            if ($hadPreviousProgId) {
                $env:OXVBA_REGISTERED_COM_PROGID = $previousProgId
            } else {
                Remove-Item Env:OXVBA_REGISTERED_COM_PROGID -ErrorAction SilentlyContinue
            }
        }
    }

    $script:excel = New-Object -ComObject Excel.Application
    $script:excel.Visible = $false
    $script:excel.DisplayAlerts = $false
    try {
        $vbaRows = @(
            Invoke-ExcelCase -CaseId "CCT-026-TES-001" -Scenario "Late-bound scalar arg/return" -Populate {
                param($mod)
                $code = @"
Public Function RunProbe()
    Dim obj As Object
    Set obj = CreateObject("OxVba.TestEventServer")
    RunProbe = CStr(obj.SumPair(3, 14))
End Function
"@
                [void]$mod.CodeModule.AddFromString($code)
            }
            Invoke-ExcelCase -CaseId "CCT-026-TES-002" -Scenario "Late-bound array argument shape" -Populate {
                param($mod)
                $code = @"
Public Function RunProbe()
    Dim obj As Object
    Dim inputItems(0 To 2) As Variant
    Set obj = CreateObject("OxVba.TestEventServer")
    inputItems(0) = 1
    inputItems(1) = 2
    inputItems(2) = 3
    RunProbe = CStr(obj.DescribeArrayShape(inputItems))
End Function
"@
                [void]$mod.CodeModule.AddFromString($code)
            }
            Invoke-ExcelCase -CaseId "CCT-026-TES-003" -Scenario "Late-bound self object roundtrip" -Populate {
                param($mod)
                $code = @"
Public Function RunProbe()
    Dim obj As Object
    Set obj = CreateObject("OxVba.TestEventServer")
    RunProbe = CStr(obj.IsSelf(obj))
End Function
"@
                [void]$mod.CodeModule.AddFromString($code)
            }
            Invoke-ExcelCase -CaseId "CCT-026-TES-004" -Scenario "Late-bound scalar array return" -Populate {
                param($mod)
                $code = @"
Public Function RunProbe()
    Dim obj As Object
    Dim items As Variant
    Set obj = CreateObject("OxVba.TestEventServer")
    items = obj.ReturnLongArray()
    RunProbe = CStr(UBound(items) - LBound(items) + 1) & "," & CStr(items(LBound(items)))
End Function
"@
                [void]$mod.CodeModule.AddFromString($code)
            }
            Invoke-ExcelCase -CaseId "CCT-026-TES-005" -Scenario "Late-bound dispatch element inside returned array" -Populate {
                param($mod)
                $code = @"
Public Function RunProbe()
    Dim obj As Object
    Dim items As Variant
    Set obj = CreateObject("OxVba.TestEventServer")
    items = obj.ReturnSelfArray()
    RunProbe = CStr(items(LBound(items)).Ping())
End Function
"@
                [void]$mod.CodeModule.AddFromString($code)
            }
        )
    } finally {
        $script:excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($script:excel)
    }

    $oxRows = @(
        Invoke-OxCase -CaseId "CCT-026-TES-001" -Scenario "Late-bound scalar arg/return" -ExpectedObserved "17" -TestName "windows_registered_com_lane::registered_testeventserver_scalar_sum_pair_supported_subset"
        Invoke-OxCase -CaseId "CCT-026-TES-002" -Scenario "Late-bound array argument shape" -ExpectedObserved "rank=1;len=3;lb=0;ub=2;first=1" -TestName "windows_registered_com_lane::registered_testeventserver_array_argument_supported_subset"
        Invoke-OxCase -CaseId "CCT-026-TES-003" -Scenario "Late-bound self object roundtrip" -ExpectedObserved "True" -TestName "windows_registered_com_lane::registered_testeventserver_object_argument_supported_subset"
        Invoke-OxCase -CaseId "CCT-026-TES-004" -Scenario "Late-bound scalar array return" -ExpectedObserved "3,4" -TestName "windows_registered_com_lane::registered_testeventserver_scalar_array_return_supported_subset"
        Invoke-OxCase -CaseId "CCT-026-TES-005" -Scenario "Late-bound dispatch element inside returned array" -ExpectedObserved "42" -TestName "windows_registered_com_lane::registered_testeventserver_dispatch_array_return_supported_subset"
    )

    $rows = foreach ($oxRow in $oxRows) {
        $vbaRow = $vbaRows | Where-Object { $_.case_id -eq $oxRow.case_id } | Select-Object -First 1
        if ($null -eq $vbaRow) {
            throw "missing VBA row for case $($oxRow.case_id)"
        }
        [PSCustomObject]@{
            topic_id = "CCT-026"
            case_id = $oxRow.case_id
            scenario = $oxRow.scenario
            vba_status = $vbaRow.status
            vba_observed = $vbaRow.observed
            oxvba_status = $oxRow.status
            oxvba_observed = $oxRow.observed
            match = if ($vbaRow.status -eq $oxRow.status -and $vbaRow.observed -eq $oxRow.observed) { "true" } else { "false" }
            notes = $oxRow.notes
        }
    }

    $csvPath = Join-Path $runDir "results.csv"
    $summaryPath = Join-Path $runDir "summary.md"
    $rows | Export-Csv -Path $csvPath -NoTypeInformation

    $summary = @(
        "# COM TestEventServer Marshaling Oracle Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Registration path: HKCU current-user reg import",
        "- Results CSV: $csvPath",
        "- Total cases: $($rows.Count)",
        "- Match count: $((@($rows | Where-Object { $_.match -eq 'true' })).Count)",
        "- Mismatch count: $((@($rows | Where-Object { $_.match -ne 'true' })).Count)",
        "",
        "## Case Results",
        "| Topic | Case | VBA | OxVba | Match | Notes |",
        "|---|---|---|---|---|---|"
    )
    foreach ($row in $rows) {
        $vbaCell = "$($row.vba_status): $($row.vba_observed)"
        $oxCell = "$($row.oxvba_status): $($row.oxvba_observed)"
        $summary += "| $($row.topic_id) | $($row.case_id) | $vbaCell | $oxCell | $($row.match) | $($row.notes) |"
    }
    Set-Content -Path $summaryPath -Value ($summary -join [Environment]::NewLine)

    Write-Host "com-testeventserver-marshaling-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "results=$csvPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
