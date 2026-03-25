param(
    [string]$OutputRoot = "docs/evidence/conformance/oracle_captures",
    [string]$RunId = "",
    [switch]$NoArtifacts
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-testeventserver-typelib-probe" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "com-testeventserver-typelib-probe" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "com_testeventserver_typelib_probe_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    & (Join-Path $workspaceRoot "tools/OxVba.TestEventServer/register.ps1") -Configuration Debug -Scope CurrentUser

    $typeLibPath = (Resolve-Path "tools/OxVba.TestEventServer/bin/Debug/net48/OxVba.TestEventServer.tlb").Path
    $rows = New-Object System.Collections.Generic.List[object]
    $referenceInfo = $null

    function Add-Row {
        param(
            [string]$CaseId,
            [string]$Scenario,
            [string]$Status,
            [string]$Observed
        )

        $rows.Add([PSCustomObject]@{
                case_id   = $CaseId
                scenario  = $Scenario
                status    = $Status
                observed  = $Observed
            }) | Out-Null
    }

    function Invoke-WorkbookProbe {
        param(
            [object]$Excel,
            [scriptblock]$Populate,
            [string]$ProcedureName = "RunProbe"
        )

        $wb = $null
        try {
            $wb = $Excel.Workbooks.Add()
            [void]$wb.VBProject.References.AddFromFile($typeLibPath)

            if ($null -eq $script:referenceInfo) {
                $reference = $wb.VBProject.References |
                    Where-Object { $_.Description -eq "Deterministic COM event test server for OxVba registered event lane parity." } |
                    Select-Object -First 1
                if ($null -ne $reference) {
                    $script:referenceInfo = [PSCustomObject]@{
                        name      = $reference.Name
                        guid      = $reference.Guid
                        major     = $reference.Major
                        minor     = $reference.Minor
                        is_broken = $reference.IsBroken
                    }
                }
            }

            & $Populate $wb
            if ([string]::IsNullOrWhiteSpace($ProcedureName)) {
                return @{
                    status   = "ok"
                    observed = ""
                }
            }
            $result = $Excel.Run($ProcedureName)
            return @{
                status   = "ok"
                observed = [string]$result
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

    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    try {
        $pingProbe = Invoke-WorkbookProbe -Excel $excel -Populate {
            param($wb)
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
        }
        Add-Row -CaseId "CCT-027-TES-001" -Scenario "AddFromFile + New TestEventServer + Ping()" -Status $pingProbe.status -Observed $pingProbe.observed

        $eventProbe = Invoke-WorkbookProbe -Excel $excel -Populate {
            param($wb)
            $sink = $wb.VBProject.VBComponents.Add(2)
            $sink.Name = "Sink"
            $sinkCode = @"
Public Log As String
Private WithEvents src As TestEventServer

Public Sub Attach()
    Set src = New TestEventServer
End Sub

Public Sub Fire()
    src.FireValueChanged 7
End Sub

Private Sub src_OnValueChanged(ByVal value As Long)
    Log = CStr(value)
End Sub
"@
            [void]$sink.CodeModule.AddFromString($sinkCode)

            $mod = $wb.VBProject.VBComponents.Add(1)
            $mod.Name = "MainModule"
            $code = @"
Public Function RunProbe()
    Dim s As New Sink
    s.Attach
    s.Fire
    RunProbe = s.Log
End Function
"@
            [void]$mod.CodeModule.AddFromString($code)
        }
        Add-Row -CaseId "CCT-027-TES-002" -Scenario "AddFromFile + WithEvents TestEventServer source interface" -Status $eventProbe.status -Observed $eventProbe.observed

        $brokenRefRoot = Join-Path $runDir "broken_ref_probe"
        New-Item -ItemType Directory -Force -Path $brokenRefRoot | Out-Null
        $brokenRefTypeLib = Join-Path $brokenRefRoot "OxVba.TestEventServer.tlb"
        $brokenRefWorkbook = Join-Path $brokenRefRoot "broken_ref_probe.xlsm"
        Copy-Item $typeLibPath $brokenRefTypeLib -Force

        $brokenRefSave = Invoke-WorkbookProbe -Excel $excel -Populate {
            param($wb)
            $wb.SaveAs($brokenRefWorkbook, 52)
        } -ProcedureName ""
        if ($brokenRefSave.status -eq "ok") {
            Rename-Item $brokenRefTypeLib ($brokenRefTypeLib + ".missing")
            $reopened = $null
            try {
                $reopened = $excel.Workbooks.Open($brokenRefWorkbook)
                $matchingReference = $reopened.VBProject.References |
                    Where-Object { $_.Guid -eq $referenceInfo.guid } |
                    Select-Object -First 1
                if ($null -eq $matchingReference) {
                    Add-Row -CaseId "CCT-048-TES-001" -Scenario "Saved workbook reopened after referenced .tlb file is removed" -Status "ok" -Observed "reference missing from reopened VBProject.References set"
                } else {
                    Add-Row -CaseId "CCT-048-TES-001" -Scenario "Saved workbook reopened after referenced .tlb file is removed" -Status "ok" -Observed ("reference present is_broken=" + [string]$matchingReference.IsBroken)
                }
            } catch {
                Add-Row -CaseId "CCT-048-TES-001" -Scenario "Saved workbook reopened after referenced .tlb file is removed" -Status "error" -Observed $_.Exception.Message
            } finally {
                if ($reopened -ne $null) {
                    $reopened.Close($false)
                    [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($reopened)
                }
            }
        } else {
            Add-Row -CaseId "CCT-048-TES-001" -Scenario "Saved workbook reopened after referenced .tlb file is removed" -Status "error" -Observed ("save failed: " + $brokenRefSave.observed)
        }
    } finally {
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    }

    $resultsPath = Join-Path $runDir "results.csv"
    $summaryPath = Join-Path $runDir "summary.md"
    $rows | Export-Csv -Path $resultsPath -NoTypeInformation

    $summary = @(
        "# COM TestEventServer Typelib Probe",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Registration path: HKCU current-user reg import",
        "- Typelib export path: TlbExp.exe",
        "- TypeLib file: $typeLibPath",
        "- Reference name: $($referenceInfo.name)",
        "- Reference GUID: $($referenceInfo.guid)",
        "- Reference version: $($referenceInfo.major).$($referenceInfo.minor)",
        "- Reference broken: $($referenceInfo.is_broken)",
        "- Results CSV: $resultsPath",
        "",
        "## Cases",
        "| Case | Scenario | Status | Observed |",
        "|---|---|---|---|"
    )
    foreach ($row in $rows) {
        $summary += "| $($row.case_id) | $($row.scenario) | $($row.status) | $($row.observed) |"
    }
    Set-Content -Path $summaryPath -Value ($summary -join "`n")

    Write-Host "com-testeventserver-typelib-probe: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "results=$resultsPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
