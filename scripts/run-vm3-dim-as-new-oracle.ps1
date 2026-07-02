param(
    [string]$RunId = ("vm3_dim_as_new_oracle_{0:yyyyMMddTHHmmssZ}" -f (Get-Date).ToUniversalTime()),
    [string]$OutputRoot = "docs/evidence/conformance",
    [string[]]$CaseId = @()
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$outDir = Join-Path (Join-Path $repoRoot $OutputRoot) $RunId
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

function New-ModuleSpec([string]$Name, [int]$Kind, [string]$Code) {
    [pscustomobject]@{ name = $Name; kind = $Kind; code = $Code; importFile = $false }
}

function New-ClassFileSpec([string]$Name, [string]$Code) {
    [pscustomobject]@{ name = $Name; kind = 2; code = $Code; importFile = $true }
}

function New-OracleCase([string]$Id, [string]$Purpose, [object[]]$Modules, [string]$Run = "Main.OracleEntry") {
    [pscustomobject]@{
        id = $Id
        purpose = $Purpose
        run = $Run
        modules = $Modules
    }
}

function Get-ExcelPids {
    @(Get-Process EXCEL -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
}

function Get-OwnedExcelPid([int[]]$Before) {
    $after = Get-Process EXCEL -ErrorAction SilentlyContinue
    $owned = @($after | Where-Object { $Before -notcontains $_.Id } | Select-Object -ExpandProperty Id)
    if ($owned.Count -eq 0) {
        return $null
    }
    return [int]$owned[0]
}

function Get-VbeCompileControl($Vbe) {
    try {
        $found = $Vbe.CommandBars.FindControl($null, 578, $null, $null, $true)
        if ($null -ne $found) {
            return $found
        }
    } catch {
        # Fall back to walking command bars.
    }

    foreach ($bar in @($Vbe.CommandBars)) {
        foreach ($ctl in @($bar.Controls)) {
            try {
                foreach ($sub in @($ctl.Controls)) {
                    if ($sub.Id -eq 578) {
                        return $sub
                    }
                }
            } catch {
                # Non-popup controls do not expose nested Controls.
            }
        }
    }
    return $null
}

function Get-UiaWindowsForPid([int]$ExcelPid) {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $windows = $root.FindAll(
        [System.Windows.Automation.TreeScope]::Children,
        [System.Windows.Automation.Condition]::TrueCondition
    )
    $owned = New-Object System.Collections.Generic.List[object]
    foreach ($window in $windows) {
        if ($window.Current.ProcessId -eq $ExcelPid) {
            $owned.Add($window)
        }
    }
    return $owned
}

function Get-SelectedCodeFromUia($Windows) {
    $docCond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Document
    )
    foreach ($window in $Windows) {
        if ($window.Current.Name -notlike "Microsoft Visual Basic for Applications*") {
            continue
        }
        $docs = $window.FindAll([System.Windows.Automation.TreeScope]::Descendants, $docCond)
        foreach ($doc in $docs) {
            try {
                $pattern = $doc.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
                $selection = $pattern.GetSelection()
                if ($selection.Count -eq 0) {
                    continue
                }
                $token = $selection[0].GetText(512).Trim()
                $lineRange = $selection[0].Clone()
                $lineRange.ExpandToEnclosingUnit([System.Windows.Automation.TextUnit]::Line) | Out-Null
                $line = $lineRange.GetText(2000).Trim()
                return [pscustomobject]@{
                    selectedText = $token
                    selectedLine = $line
                    vbeWindow = $window.Current.Name
                }
            } catch {
                # Try another document pane if this one has no TextPattern.
            }
        }
    }
    return $null
}

function Get-SelectedCodeFromCom($Vbe) {
    try {
        $pane = $Vbe.ActiveCodePane
        if ($null -eq $pane) {
            return $null
        }
        $startLine = 0
        $startColumn = 0
        $endLine = 0
        $endColumn = 0
        $pane.GetSelection([ref]$startLine, [ref]$startColumn, [ref]$endLine, [ref]$endColumn)
        $module = $pane.CodeModule
        $lineText = $module.Lines($startLine, 1)
        $selectedText = $null
        if ($endLine -eq $startLine -and $endColumn -gt $startColumn) {
            $start = [Math]::Max(0, $startColumn - 1)
            $length = [Math]::Min($lineText.Length - $start, $endColumn - $startColumn)
            if ($length -gt 0) {
                $selectedText = $lineText.Substring($start, $length)
            }
        }
        return [pscustomobject]@{
            selectedText = if ($selectedText) { $selectedText.Trim() } else { $null }
            selectedLine = $lineText.Trim()
            vbeWindow = $Vbe.MainWindow.Caption
        }
    } catch {
        return $null
    }
}

function Get-OwnedDialogSnapshot([int]$ExcelPid, $Vbe) {
    $windows = Get-UiaWindowsForPid $ExcelPid
    $textCond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Text
    )
    $buttonCond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Button
    )

    foreach ($window in $windows) {
        $texts = $window.FindAll([System.Windows.Automation.TreeScope]::Descendants, $textCond)
        $names = New-Object System.Collections.Generic.List[string]
        foreach ($item in $texts) {
            if (-not [string]::IsNullOrWhiteSpace($item.Current.Name)) {
                $names.Add($item.Current.Name)
            }
        }

        $isCompileDialog = @($names | Where-Object { $_ -match "Compile error" }).Count -gt 0
        $isRuntimeDialog = @($names | Where-Object { $_ -match "Run-time error|runtime error" }).Count -gt 0
        if (-not ($isCompileDialog -or $isRuntimeDialog)) {
            continue
        }

        $buttons = $window.FindAll([System.Windows.Automation.TreeScope]::Descendants, $buttonCond)
        $buttonNames = @()
        foreach ($button in $buttons) {
            if (-not [string]::IsNullOrWhiteSpace($button.Current.Name)) {
                $buttonNames += $button.Current.Name
            }
        }
        $selected = Get-SelectedCodeFromUia $windows
        if ($null -eq $selected -or [string]::IsNullOrWhiteSpace($selected.selectedLine)) {
            $selected = Get-SelectedCodeFromCom $Vbe
        }
        return [pscustomobject]@{
            windowName = $window.Current.Name
            dialogText = ($names -join " / ")
            dialogButtons = $buttonNames
            selectedText = if ($selected) { $selected.selectedText } else { $null }
            selectedLine = if ($selected) { $selected.selectedLine } else { $null }
            vbeWindow = if ($selected) { $selected.vbeWindow } else { $null }
        }
    }
    return $null
}

function Dismiss-OwnedDialogs([int]$ExcelPid) {
    $windows = Get-UiaWindowsForPid $ExcelPid
    $buttonCond = New-Object System.Windows.Automation.AndCondition(
        (New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            [System.Windows.Automation.ControlType]::Button
        )),
        (New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::NameProperty,
            "OK"
        ))
    )
    foreach ($window in $windows) {
        $buttons = $window.FindAll([System.Windows.Automation.TreeScope]::Descendants, $buttonCond)
        foreach ($button in $buttons) {
            try {
                $invoke = $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
                $invoke.Invoke()
                Start-Sleep -Milliseconds 150
            } catch {
                # Best-effort modal cleanup, scoped to this Excel process.
            }
        }
    }
}

function Get-CaseDir([string]$Id) {
    $caseDir = Join-Path $outDir ($Id -replace '[^A-Za-z0-9_.-]', '_')
    New-Item -ItemType Directory -Force -Path $caseDir | Out-Null
    return $caseDir
}

function Add-ModuleToWorkbook($Workbook, $Case, $Module) {
    if ($Module.importFile) {
        $caseDir = Get-CaseDir $Case.id
        $filePath = Join-Path $caseDir ($Module.name + ".cls")
        $normalizedCode = (($Module.code -replace "`r?`n", "`r`n").TrimEnd("`r", "`n")) + "`r`n"
        [System.IO.File]::WriteAllText($filePath, $normalizedCode, [System.Text.Encoding]::ASCII)
        $Workbook.VBProject.VBComponents.Import($filePath) | Out-Null
    } else {
        $component = $Workbook.VBProject.VBComponents.Add($Module.kind)
        $component.Name = $Module.name
        [void]$component.CodeModule.AddFromString($Module.code)
    }
}

function Invoke-Case($Case) {
    $before = Get-ExcelPids
    $xl = $null
    $ownedExcelPid = $null
    $compileStatus = "not-run"
    $dialog = $null
    $runStatus = "not-run"
    $runValue = $null
    $errorMessage = $null

    try {
        $xl = New-Object -ComObject Excel.Application
        $xl.Visible = $true
        $xl.DisplayAlerts = $false
        $ownedExcelPid = Get-OwnedExcelPid $before
        if ($null -eq $ownedExcelPid) {
            throw "Excel did not create a new owned process; refusing to probe an unowned instance"
        }

        $wb = $xl.Workbooks.Add()
        foreach ($module in $Case.modules) {
            Add-ModuleToWorkbook $wb $Case $module
        }

        $xl.VBE.MainWindow.Visible = $true
        Start-Sleep -Milliseconds 300
        $compileControl = Get-VbeCompileControl $xl.VBE
        if ($null -eq $compileControl) {
            throw "VBE compile command id 578 was not found"
        }

        $compileControl.Execute()
        Start-Sleep -Milliseconds 900
        $dialog = Get-OwnedDialogSnapshot $ownedExcelPid $xl.VBE
        if ($dialog) {
            $compileStatus = "compile-error"
            Dismiss-OwnedDialogs $ownedExcelPid
        } else {
            $compileStatus = "ok"
        }

        if ($compileStatus -eq "ok" -and $Case.run) {
            try {
                $runValue = $xl.Run("'" + $wb.Name + "'!" + $Case.run)
                $runStatus = "ok"
            } catch {
                $runStatus = "error"
                $errorMessage = $_.Exception.Message
                $dialog = Get-OwnedDialogSnapshot $ownedExcelPid $xl.VBE
                if ($dialog) {
                    Dismiss-OwnedDialogs $ownedExcelPid
                }
            }
        }
    } catch {
        $compileStatus = "harness-error"
        $errorMessage = $_.Exception.Message
    } finally {
        if ($ownedExcelPid) {
            Dismiss-OwnedDialogs $ownedExcelPid
            Stop-Process -Id $ownedExcelPid -Force -ErrorAction SilentlyContinue
        }
    }

    [pscustomobject]@{
        id = $Case.id
        purpose = $Case.purpose
        ownedExcelPid = $ownedExcelPid
        compileStatus = $compileStatus
        dialogWindow = if ($dialog) { $dialog.windowName } else { $null }
        dialogText = if ($dialog) { $dialog.dialogText } else { $null }
        dialogButtons = if ($dialog) { $dialog.dialogButtons } else { @() }
        selectedText = if ($dialog) { $dialog.selectedText } else { $null }
        selectedLine = if ($dialog) { $dialog.selectedLine } else { $null }
        vbeWindow = if ($dialog) { $dialog.vbeWindow } else { $null }
        run = $Case.run
        runStatus = $runStatus
        runValue = if ($null -ne $runValue) { [string]$runValue } else { $null }
        errorMessage = $errorMessage
        modules = @($Case.modules | ForEach-Object { $_.name })
    }
}

$counterClass = @"
VERSION 1.0 CLASS
BEGIN
  MultiUse = -1  'True
END
Attribute VB_Name = "Counter"
Attribute VB_GlobalNameSpace = False
Attribute VB_Creatable = False
Attribute VB_PredeclaredId = False
Attribute VB_Exposed = True
Option Explicit
Private n As Long

Private Sub Class_Initialize()
    n = 10
    Main.Log = Main.Log & "I;"
End Sub

Private Sub Class_Terminate()
    Main.Log = Main.Log & "T" & CStr(n) & ";"
End Sub

Public Sub Bump()
    n = n + 1
End Sub

Public Property Get Total() As Long
    Total = n
End Property
"@

$hostClass = @"
VERSION 1.0 CLASS
BEGIN
  MultiUse = -1  'True
END
Attribute VB_Name = "Host"
Attribute VB_GlobalNameSpace = False
Attribute VB_Creatable = False
Attribute VB_PredeclaredId = False
Attribute VB_Exposed = True
Option Explicit
Private child As New Counter

Public Function FieldDimOnly() As String
    FieldDimOnly = CStr(Len(Main.Log)) & "|" & Main.Log
End Function

Public Function FieldFirstMember() As String
    child.Bump
    FieldFirstMember = CStr(child.Total) & "|" & Main.Log
End Function

Public Function FieldIsNothing() As String
    FieldIsNothing = CStr(child Is Nothing) & "|" & Main.Log
End Function

Public Function FieldSetNothingBeforeAccess() As String
    Set child = Nothing
    FieldSetNothingBeforeAccess = CStr(Len(Main.Log)) & "|" & Main.Log
End Function

Public Function FieldSetNothingResurrect() As String
    child.Bump
    Set child = Nothing
    FieldSetNothingResurrect = CStr(child.Total) & "|" & Main.Log
End Function

Public Function FieldBumpTotal() As Long
    child.Bump
    FieldBumpTotal = child.Total
End Function
"@

function New-StandardModule([string]$Code) {
    $wrapped = $Code + @"

Public Function OracleEntry() As Variant
    On Error GoTo EH
    OracleEntry = CStr(RunProbe())
    Exit Function
EH:
    OracleEntry = "ERR:" & CStr(Err.Number) & ":" & Err.Description
End Function
"@
    New-ModuleSpec "Main" 1 $wrapped
}

function New-CounterClass {
    New-ClassFileSpec "Counter" $counterClass
}

function New-HostClass {
    New-ClassFileSpec "Host" $hostClass
}

$cases = @()
$cases += (New-OracleCase "LOCAL-DIM-ONLY" "A local As New declaration alone does not instantiate." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim c As New Counter
    RunProbe = CStr(Len(Log)) & "|" & Log
End Function
"@),
    (New-CounterClass)
))
$cases += (New-OracleCase "LOCAL-FIRST-MEMBER" "The first member access lazily creates the object." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim c As New Counter
    c.Bump
    RunProbe = CStr(c.Total) & "|" & Log
End Function
"@),
    (New-CounterClass)
))
$cases += (New-OracleCase "LOCAL-IS-NOTHING" "`Is Nothing` against an As New local." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim c As New Counter
    RunProbe = CStr(c Is Nothing) & "|" & Log
End Function
"@),
    (New-CounterClass)
))
$cases += (New-OracleCase "LOCAL-SET-NOTHING-BEFORE-ACCESS" "Assigning Nothing before any read/member access does not create the object." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim c As New Counter
    Set c = Nothing
    RunProbe = CStr(Len(Log)) & "|" & Log
End Function
"@),
    (New-CounterClass)
))
$cases += (New-OracleCase "LOCAL-SET-NOTHING-RESURRECT" "Set Nothing clears the slot; the next access resurrects a fresh object." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim c As New Counter
    c.Bump
    Set c = Nothing
    RunProbe = CStr(c.Total) & "|" & Log
End Function
"@),
    (New-CounterClass)
))
$cases += (New-OracleCase "GLOBAL-DIM-ONLY" "A module-level As New variable is also lazy." @(
    (New-StandardModule @"
Public Log As String
Private g As New Counter

Public Function RunProbe() As Variant
    RunProbe = CStr(Len(Log)) & "|" & Log
End Function
"@),
    (New-CounterClass)
))
$cases += (New-OracleCase "GLOBAL-SET-NOTHING-RESURRECT" "A module-level As New variable resurrects after Set Nothing." @(
    (New-StandardModule @"
Public Log As String
Private g As New Counter

Public Function RunProbe() As Variant
    g.Bump
    Set g = Nothing
    RunProbe = CStr(g.Total) & "|" & Log
End Function
"@),
    (New-CounterClass)
))
$cases += (New-OracleCase "FIELD-DIM-ONLY" "A class-field As New declaration does not instantiate during host construction or unrelated method execution." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim h As New Host
    RunProbe = h.FieldDimOnly()
End Function
"@),
    (New-CounterClass),
    (New-HostClass)
))
$cases += (New-OracleCase "FIELD-FIRST-MEMBER" "The first member access on a class-field As New slot lazily creates the child object." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim h As New Host
    RunProbe = h.FieldFirstMember()
End Function
"@),
    (New-CounterClass),
    (New-HostClass)
))
$cases += (New-OracleCase "FIELD-IS-NOTHING" "`Is Nothing` against a class-field As New slot." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim h As New Host
    RunProbe = h.FieldIsNothing()
End Function
"@),
    (New-CounterClass),
    (New-HostClass)
))
$cases += (New-OracleCase "FIELD-SET-NOTHING-BEFORE-ACCESS" "Assigning Nothing to a class-field As New slot before any read/member access does not create the child object." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim h As New Host
    RunProbe = h.FieldSetNothingBeforeAccess()
End Function
"@),
    (New-CounterClass),
    (New-HostClass)
))
$cases += (New-OracleCase "FIELD-SET-NOTHING-RESURRECT" "Set Nothing clears a class-field As New slot; the next access resurrects a fresh child object." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim h As New Host
    RunProbe = h.FieldSetNothingResurrect()
End Function
"@),
    (New-CounterClass),
    (New-HostClass)
))
$cases += (New-OracleCase "FIELD-INSTANCE-ISOLATION" "Two host instances keep independent class-field As New child slots." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim a As New Host
    Dim b As New Host
    RunProbe = CStr(a.FieldBumpTotal()) & "/" & CStr(b.FieldBumpTotal()) & "/" & CStr(a.FieldBumpTotal()) & "|" & Log
End Function
"@),
    (New-CounterClass),
    (New-HostClass)
))

if ($CaseId.Count -gt 0) {
    $caseSet = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($id in $CaseId) {
        [void]$caseSet.Add($id)
    }
    $cases = @($cases | Where-Object { $caseSet.Contains($_.id) })
}

$results = @()
$partialJsonPath = Join-Path $outDir "results.partial.json"
foreach ($case in $cases) {
    Write-Host "Running $($case.id)"
    $result = Invoke-Case $case
    $results += $result
    $results | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 $partialJsonPath
}

$jsonPath = Join-Path $outDir "results.json"
$results | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 $jsonPath
Remove-Item -LiteralPath $partialJsonPath -Force -ErrorAction SilentlyContinue

$summaryPath = Join-Path $outDir "summary.md"
$capturedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# VM3 Dim As New Excel Oracle")
$lines.Add("")
$lines.Add("- Run ID: $RunId")
$lines.Add("- Captured: $capturedAt")
$lines.Add("- Harness: $($MyInvocation.MyCommand.Path)")
$lines.Add("- Class setup: imported `.cls` file with `Attribute VB_PredeclaredId = False`, so only `Dim As New` can create instances.")
$lines.Add("- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.")
$lines.Add("")
$lines.Add("| Case | Compile | Dialog | Selected | Run | Value | Error |")
$lines.Add("|---|---|---|---|---|---|---|")
foreach ($result in $results) {
    $dialogText = if ($result.dialogText) { ($result.dialogText -replace "`r?`n", " / ").Replace("|", "\|") } else { "" }
    $selected = if ($result.selectedLine) { ($result.selectedLine -replace "`r?`n", " / ").Replace("|", "\|") } else { $result.selectedText }
    $value = if ($result.runValue) { $result.runValue.Replace([string][char]0, "\0").Replace("|", "\|") } else { "" }
    $errorCell = if ($result.errorMessage) { ($result.errorMessage -replace "`r?`n", " / ").Replace("|", "\|") } else { "" }
    $lines.Add("| $($result.id) | $($result.compileStatus) | $dialogText | $selected | $($result.runStatus) | $value | $errorCell |")
}
$lines.Add("")
$lines.Add("Raw JSON: results.json")
$lines | Set-Content -Encoding UTF8 $summaryPath

Write-Host "Wrote $summaryPath"
Write-Host "Wrote $jsonPath"
