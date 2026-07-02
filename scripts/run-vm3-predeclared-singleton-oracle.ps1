param(
    [string]$RunId = ("vm3_predeclared_singleton_oracle_{0:yyyyMMddTHHmmssZ}" -f (Get-Date).ToUniversalTime()),
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
Attribute VB_PredeclaredId = True
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

$cases = @()
$cases += (New-OracleCase "PERSISTENCE" "Repeated predeclared class access uses one initialized default instance." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Counter.Bump
    Counter.Bump
    RunProbe = CStr(Counter.Total) & "|" & Log
End Function
"@),
    (New-CounterClass)
))
$cases += (New-OracleCase "LOCAL-REF-SET-NOTHING" "Dropping an ordinary local reference to the default instance does not reset it." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim o As Object
    Counter.Bump
    Set o = Counter
    Set o = Nothing
    RunProbe = CStr(Counter.Total) & "|" & Log
End Function
"@),
    (New-CounterClass)
))
$cases += (New-OracleCase "SET-PREDECLARED-NOTHING" "Assigning Nothing to the predeclared class name." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim beforeTotal As Long
    Dim afterTotal As Long
    Counter.Bump
    beforeTotal = Counter.Total
    Set Counter = Nothing
    afterTotal = Counter.Total
    RunProbe = CStr(beforeTotal) & ":" & CStr(afterTotal) & "|" & Log
End Function
"@),
    (New-CounterClass)
))
$cases += (New-OracleCase "SET-PREDECLARED-NOTHING-IDENTITY" "Object identity after assigning Nothing to the predeclared class name and accessing again." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Dim beforePtr As LongPtr
    Dim afterPtr As LongPtr
    beforePtr = ObjPtr(Counter)
    Counter.Bump
    Set Counter = Nothing
    afterPtr = ObjPtr(Counter)
    RunProbe = CStr(beforePtr = afterPtr) & ":" & CStr(Counter.Total) & "|" & Log
End Function
"@),
    (New-CounterClass)
))
$cases += (New-OracleCase "SET-PREDECLARED-NEW" "Assigning a fresh instance to the predeclared class name." @(
    (New-StandardModule @"
Public Log As String

Public Function RunProbe() As Variant
    Counter.Bump
    Set Counter = New Counter
    RunProbe = CStr(Counter.Total) & "|" & Log
End Function
"@),
    (New-CounterClass)
))
$cases += (New-OracleCase "HELD-OLD-REF-THEN-RESET" "An ordinary object reference still points at the old default instance when the default name is reset." @(
    (New-StandardModule @"
Public Log As String
Public Held As Object

Public Function RunProbe() As Variant
    Set Held = Counter
    Counter.Bump
    Set Counter = Nothing
    RunProbe = CStr(Held.Total) & ":" & CStr(Counter.Total) & "|" & Log
End Function
"@),
    (New-CounterClass)
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
$lines.Add("# VM3 Predeclared Singleton Excel Oracle")
$lines.Add("")
$lines.Add("- Run ID: $RunId")
$lines.Add("- Captured: $capturedAt")
$lines.Add("- Harness: $($MyInvocation.MyCommand.Path)")
$lines.Add("- Class setup: imported `.cls` file with `Attribute VB_PredeclaredId = True`, not CodeModule text injection.")
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
