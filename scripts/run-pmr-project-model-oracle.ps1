param(
    [string]$OutputRoot = "docs/evidence/conformance/oracle_captures",
    [switch]$KeepArtifacts,
    [switch]$DisableDialogGuardian,
    [int]$DialogGuardianPollMs = 250,
    [int]$DialogGuardianMaxSeconds = 1800
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $workspaceRoot = (Resolve-Path ".").Path
    $stamp = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "pmr_project_model_$stamp"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null
    $artifactDir = Join-Path $runDir "artifacts"
    New-Item -ItemType Directory -Force -Path $artifactDir | Out-Null
    $dialogGuardianLog = Join-Path $runDir "excel_dialog_guardian.log"
    $dialogGuardianStop = Join-Path $runDir "excel_dialog_guardian.stop"
    $dialogGuardianProc = $null

    $rows = New-Object System.Collections.Generic.List[object]

    Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win32User32Pid {
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@

    function Get-WindowProcessId {
        param([int]$Hwnd)
        [uint32]$windowPid = 0
        [void][Win32User32Pid]::GetWindowThreadProcessId([IntPtr]::new($Hwnd), [ref]$windowPid)
        [int]$windowPid
    }

    function Add-Row {
        param(
            [string]$TopicId,
            [string]$CaseId,
            [string]$Scenario,
            [string]$VbaStatus,
            [string]$VbaObserved,
            [string]$OxvbaStatus,
            [string]$OxvbaObserved,
            [string]$Notes
        )
        $match = $false
        if ($VbaStatus -eq $OxvbaStatus) {
            if ($VbaStatus -eq "ok") {
                $match = ($VbaObserved -eq $OxvbaObserved)
            } elseif ($VbaStatus -eq "error") {
                # Error wording differs by host; status parity is tracked here.
                $match = $true
            }
        }
        $rows.Add([PSCustomObject]@{
                topic_id       = $TopicId
                case_id        = $CaseId
                scenario       = $Scenario
                vba_status     = $VbaStatus
                vba_observed   = $VbaObserved
                oxvba_status   = $OxvbaStatus
                oxvba_observed = $OxvbaObserved
                match          = if ($match) { "true" } else { "false" }
                notes          = $Notes
            }) | Out-Null
    }

    function New-UniqueName {
        param([string]$Prefix)
        $suffix = [System.Guid]::NewGuid().ToString("N").Substring(0, 8)
        "${Prefix}_${suffix}"
    }

    function Add-StdModule {
        param($Workbook, [string]$ModuleName, [string]$Code)
        $component = $Workbook.VBProject.VBComponents.Add(1)
        $component.Name = $ModuleName
        $null = $component.CodeModule.AddFromString($Code)
    }

    function Add-ClassModule {
        param($Workbook, [string]$ClassName, [string]$Code)
        $component = $Workbook.VBProject.VBComponents.Add(2)
        $component.Name = $ClassName
        $null = $component.CodeModule.AddFromString($Code)
    }

    function New-Addin {
        param(
            $Excel,
            [string]$ProjectName,
            [string]$FileName,
            [scriptblock]$Populate
        )

        $wb = $null
        try {
            $wb = $Excel.Workbooks.Add()
            $wb.VBProject.Name = $ProjectName
            & $Populate $wb
            $path = Join-Path $artifactDir $FileName
            $wb.SaveAs($path, 55)
            $path
        } finally {
            if ($wb -ne $null) {
                $wb.Close($false)
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
            }
        }
    }

    function Invoke-WorkbookProbe {
        param(
            $Excel,
            [string]$ProjectName,
            [scriptblock]$Populate,
            [string[]]$ReferencePaths,
            [string]$ProcedureName
        )

        $wb = $null
        try {
            $wb = $Excel.Workbooks.Add()
            $wb.VBProject.Name = $ProjectName
            & $Populate $wb
            foreach ($referencePath in $ReferencePaths) {
                if ($referencePath) {
                    [void]$wb.VBProject.References.AddFromFile($referencePath)
                }
            }
            try {
                $result = $Excel.Run($ProcedureName)
                @{ status = "ok"; observed = [string]$result }
            } catch {
                @{ status = "error"; observed = $_.Exception.Message }
            }
        } finally {
            if ($wb -ne $null) {
                $wb.Close($false)
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
            }
        }
    }

    function Try-GetVbProperty {
        param($Component, [string]$PropertyName)
        try {
            [string]$Component.Properties.Item($PropertyName).Value
        } catch {
            "<unsupported:$PropertyName>"
        }
    }

    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excelVersion = $excel.Version
    $excelPid = Get-WindowProcessId -Hwnd ([int]$excel.Hwnd)
    if (-not $DisableDialogGuardian -and $excelPid -gt 0) {
        if (Test-Path $dialogGuardianStop) {
            Remove-Item -Force $dialogGuardianStop
        }
        $guardianScript = Join-Path $PSScriptRoot "excel-dialog-guardian.ps1"
        $guardianArgs = @(
            "-NoLogo",
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-File", $guardianScript,
            "-ExcelPid", "$excelPid",
            "-StopFile", $dialogGuardianStop,
            "-LogFile", $dialogGuardianLog,
            "-PollMs", "$DialogGuardianPollMs",
            "-MaxSeconds", "$DialogGuardianMaxSeconds"
        )
        $dialogGuardianProc = Start-Process -FilePath "pwsh" -ArgumentList $guardianArgs -WindowStyle Hidden -PassThru
        Write-Host "excel-dialog-guardian: started (pid=$($dialogGuardianProc.Id), excel_pid=$excelPid)"
    }

    try {
        # CCT-037: reference precedence and shadowing.
        $libOnePath = New-Addin -Excel $excel -ProjectName (New-UniqueName "LibOneProj") -FileName "cct037_lib_one.xlam" -Populate {
            param($wb)
            Add-StdModule $wb "LibOneMod" @"
Public Function Compute()
    Compute = 101
End Function
"@
        }
        $libTwoPath = New-Addin -Excel $excel -ProjectName (New-UniqueName "LibTwoProj") -FileName "cct037_lib_two.xlam" -Populate {
            param($wb)
            Add-StdModule $wb "LibTwoMod" @"
Public Function Compute()
    Compute = 202
End Function
"@
        }

        $case037a = Invoke-WorkbookProbe -Excel $excel -ProjectName (New-UniqueName "Probe037A") -Populate {
            param($wb)
            Add-StdModule $wb "MainModule" @"
Public Function RunProbe()
    RunProbe = Compute()
End Function
"@
        } -ReferencePaths @($libOnePath, $libTwoPath) -ProcedureName "RunProbe"
        Add-Row -TopicId "CCT-037" -CaseId "CCT-037-A" -Scenario "unqualified call resolves first referenced project" -VbaStatus $case037a.status -VbaObserved $case037a.observed -OxvbaStatus "ok" -OxvbaObserved "101" -Notes "OxVba anchor: active_project_resolution_uses_reference_precedence_order_for_shadowing"

        $case037b = Invoke-WorkbookProbe -Excel $excel -ProjectName (New-UniqueName "Probe037B") -Populate {
            param($wb)
            Add-StdModule $wb "MainModule" @"
Public Function RunProbe()
    RunProbe = Compute()
End Function
"@
        } -ReferencePaths @($libTwoPath, $libOnePath) -ProcedureName "RunProbe"
        Add-Row -TopicId "CCT-037" -CaseId "CCT-037-B" -Scenario "reference order reversal changes unqualified resolution" -VbaStatus $case037b.status -VbaObserved $case037b.observed -OxvbaStatus "ok" -OxvbaObserved "202" -Notes "OxVba anchor: reference precedence ordering"

        $case037c = Invoke-WorkbookProbe -Excel $excel -ProjectName (New-UniqueName "Probe037C") -Populate {
            param($wb)
            Add-StdModule $wb "MainModule" @"
Public Function Compute()
    Compute = 303
End Function

Public Function RunProbe()
    RunProbe = Compute()
End Function
"@
        } -ReferencePaths @($libOnePath, $libTwoPath) -ProcedureName "RunProbe"
        Add-Row -TopicId "CCT-037" -CaseId "CCT-037-C" -Scenario "local project symbol shadows referenced project symbols" -VbaStatus $case037c.status -VbaObserved $case037c.observed -OxvbaStatus "ok" -OxvbaObserved "303" -Notes "OxVba anchor: active_project_resolution_prefers_local_symbols_before_references"

        # CCT-038: Option Private Module accessibility across project boundaries.
        $libPublicPath = New-Addin -Excel $excel -ProjectName (New-UniqueName "PublicLibProj") -FileName "cct038_public_lib.xlam" -Populate {
            param($wb)
            Add-StdModule $wb "PublicMod" @"
Public Function VisibleFn()
    VisibleFn = 11
End Function
"@
        }
        $libPrivatePath = New-Addin -Excel $excel -ProjectName (New-UniqueName "PrivateLibProj") -FileName "cct038_private_lib.xlam" -Populate {
            param($wb)
            Add-StdModule $wb "PrivateMod" @"
Option Private Module
Public Function HiddenFn()
    HiddenFn = 22
End Function
"@
        }

        $case038a = Invoke-WorkbookProbe -Excel $excel -ProjectName (New-UniqueName "Probe038A") -Populate {
            param($wb)
            Add-StdModule $wb "MainModule" @"
Public Function RunProbe()
    RunProbe = VisibleFn()
End Function
"@
        } -ReferencePaths @($libPublicPath) -ProcedureName "RunProbe"
        Add-Row -TopicId "CCT-038" -CaseId "CCT-038-A" -Scenario "public procedure in referenced module is callable" -VbaStatus $case038a.status -VbaObserved $case038a.observed -OxvbaStatus "ok" -OxvbaObserved "11" -Notes "OxVba anchor: host export registry includes non-private procedural members"

        $case038bStatus = "ok"
        $case038bObserved = ""
        try {
            $case038bObserved = [string]$excel.Run("'$libPrivatePath'!HiddenFn")
        } catch {
            $case038bStatus = "error"
            $case038bObserved = $_.Exception.Message
        }
        Add-Row -TopicId "CCT-038" -CaseId "CCT-038-B" -Scenario "Option Private Module member callable via explicit host run target" -VbaStatus $case038bStatus -VbaObserved $case038bObserved -OxvbaStatus "ok" -OxvbaObserved "22" -Notes "OxVba host-export registry now retains Option Private procedures for host-direct invocation lanes"

        # CCT-039: module header attributes and defaults.
        $metaWb = $null
        try {
            $metaWb = $excel.Workbooks.Add()
            $metaWb.VBProject.Name = (New-UniqueName "Probe039")
            Add-ClassModule $metaWb "Widget" @"
Option Explicit
"@
            $classComp = $metaWb.VBProject.VBComponents.Item("Widget")
            $clsPath = Join-Path $artifactDir "cct039_widget.cls"
            $classComp.Export($clsPath)
            $attrLines = Get-Content $clsPath | Where-Object { $_ -like "Attribute VB_*" }
            $vbaObserved = ($attrLines -join ";")
            $oxObserved = 'Attribute VB_Name = "Widget";Attribute VB_GlobalNameSpace = False;Attribute VB_Creatable = False;Attribute VB_PredeclaredId = False;Attribute VB_Exposed = False'
            Add-Row -TopicId "CCT-039" -CaseId "CCT-039-A" -Scenario "exported class default header attributes" -VbaStatus "ok" -VbaObserved $vbaObserved -OxvbaStatus "ok" -OxvbaObserved $oxObserved -Notes "OxVba anchor: module_unit_from_source defaults + source class attribute constraints"
        } finally {
            if ($metaWb -ne $null) {
                $metaWb.Close($false)
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($metaWb)
            }
        }

        # CCT-040: Implements coverage and prefix rules.
        $case040a = Invoke-WorkbookProbe -Excel $excel -ProjectName (New-UniqueName "Probe040A") -Populate {
            param($wb)
            Add-ClassModule $wb "IThing" @"
Public Sub Ping()
End Sub
"@
            Add-ClassModule $wb "ThingImpl" @"
Implements IThing

Private Sub IThing_Ping()
End Sub
"@
            Add-StdModule $wb "MainModule" @"
Public Function RunProbe()
    RunProbe = 1
End Function
"@
        } -ReferencePaths @() -ProcedureName "RunProbe"
        Add-Row -TopicId "CCT-040" -CaseId "CCT-040-A" -Scenario "Implements with required prefixed member compiles" -VbaStatus $case040a.status -VbaObserved $case040a.observed -OxvbaStatus "error" -OxvbaObserved "PMR-E-IMPLEMENTS-PROJECTGRAPH-REQUIRED" -Notes "Known divergence: class interface coverage semantics pending"

        # CCT-041: event ordering with class-level WithEvents + RaiseEvent.
        $case041 = Invoke-WorkbookProbe -Excel $excel -ProjectName (New-UniqueName "Probe041") -Populate {
            param($wb)
            Add-ClassModule $wb "Emitter" @"
Public Event Tick(ByVal n As Integer)

Public Sub Fire(ByVal n As Integer)
    RaiseEvent Tick(n)
End Sub
"@
            Add-ClassModule $wb "Sink" @"
Private WithEvents src As Emitter
Public Log As String

Public Sub Attach(ByVal e As Emitter)
    Set src = e
End Sub

Private Sub src_Tick(ByVal n As Integer)
    Log = Log & CStr(n) & ","
End Sub
"@
            Add-StdModule $wb "MainModule" @"
Public Function RunProbe()
    Dim e1 As New Emitter
    Dim e2 As New Emitter
    Dim s As New Sink
    s.Attach e1
    e1.Fire 1
    s.Attach e2
    e1.Fire 2
    e2.Fire 3
    RunProbe = s.Log
End Function
"@
        } -ReferencePaths @() -ProcedureName "RunProbe"
        Add-Row -TopicId "CCT-041" -CaseId "CCT-041-A" -Scenario "WithEvents + RaiseEvent handler ordering under reassignment" -VbaStatus $case041.status -VbaObserved $case041.observed -OxvbaStatus "error" -OxvbaObserved "PMR-E-RAISEEVENT-CLASS-MODEL-REQUIRED" -Notes "Known divergence: event model not yet implemented in OxVba"

        $csvPath = Join-Path $runDir "results.csv"
        $rows | Export-Csv -Path $csvPath -NoTypeInformation

        $summaryPath = Join-Path $runDir "summary.md"
        $total = $rows.Count
        $matches = ($rows | Where-Object { $_.match -eq "true" }).Count
        $mismatches = $total - $matches
        $byTopic = $rows | Group-Object topic_id | Sort-Object Name

        $md = @()
        $md += "# PMR Project-Model Oracle Run"
        $md += ""
        $md += "- Timestamp (UTC): $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))"
        $md += "- Excel version: $excelVersion"
        $md += "- Excel process id: $excelPid"
        $md += "- Dialog guardian enabled: $([string](-not $DisableDialogGuardian))"
        if (-not $DisableDialogGuardian) {
            $md += "- Dialog guardian log: $dialogGuardianLog"
        }
        $md += "- Output CSV: $csvPath"
        $md += "- Total cases: $total"
        $md += "- Match count: $matches"
        $md += "- Mismatch count: $mismatches"
        $md += ""
        $md += "## Topic Summary"
        foreach ($group in $byTopic) {
            $topicMatches = ($group.Group | Where-Object { $_.match -eq "true" }).Count
            $topicTotal = $group.Count
            $md += "- $($group.Name): $topicMatches/$topicTotal matched"
        }
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

        Write-Host "pmr-oracle-run: complete"
        Write-Host "run_dir=$runDir"
        Write-Host "csv=$csvPath"
        Write-Host "summary=$summaryPath"
    } finally {
        if ($dialogGuardianProc -ne $null) {
            New-Item -ItemType File -Path $dialogGuardianStop -Force | Out-Null
            Start-Sleep -Milliseconds 500
            if (-not $dialogGuardianProc.HasExited) {
                Stop-Process -Id $dialogGuardianProc.Id -Force -ErrorAction SilentlyContinue
            }
            if (Test-Path $dialogGuardianStop) {
                Remove-Item -Force $dialogGuardianStop -ErrorAction SilentlyContinue
            }
        }
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    }

    if (-not $KeepArtifacts) {
        Remove-Item -Recurse -Force -Path $artifactDir
    }
} finally {
    Pop-Location
}
