param(
    [string]$RunId = ("vm3_scoping_followup_oracle_{0:yyyyMMddTHHmmssZ}" -f (Get-Date).ToUniversalTime()),
    [string]$OutputRoot = "docs/evidence/conformance"
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$outDir = Join-Path (Join-Path $repoRoot $OutputRoot) $RunId
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

function New-ModuleSpec([string]$Name, [int]$Kind, [string]$Code) {
    [pscustomobject]@{ name = $Name; kind = $Kind; code = $Code }
}

function New-ProjectSpec([string]$Name, [object[]]$Modules) {
    [pscustomobject]@{ name = $Name; modules = $Modules }
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
        # Fall back to an explicit walk below.
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

function Get-SelectedCode($Windows) {
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
                $textPattern = $doc.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
                $selection = $textPattern.GetSelection()
                if ($selection.Count -gt 0) {
                    $token = $selection[0].GetText(512)
                    $line = $selection[0].Clone()
                    $line.ExpandToEnclosingUnit([System.Windows.Automation.TextUnit]::Line) | Out-Null
                    return [pscustomobject]@{
                        selectedText = $token
                        selectedLine = $line.GetText(2000)
                        vbeWindow = $window.Current.Name
                    }
                }
            } catch {
                # Some VBE documents do not expose TextPattern during modal states.
            }
        }
    }
    return $null
}

function Get-OwnedDialogSnapshot([int]$ExcelPid) {
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
        $selected = Get-SelectedCode $windows
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
            } catch {
                # Best-effort modal cleanup, scoped to this Excel process.
            }
        }
    }
}

function Get-ComSelectedCode($Vbe) {
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
        if ($startLine -le 0) {
            return $null
        }
        $line = $pane.CodeModule.Lines($startLine, 1)
        $token = $null
        if ($startLine -eq $endLine -and $endColumn -gt $startColumn -and $startColumn -gt 0) {
            $token = $line.Substring($startColumn - 1, $endColumn - $startColumn)
        }
        return [pscustomobject]@{
            selectedText = $token
            selectedLine = $line
            vbeWindow = $Vbe.MainWindow.Caption
        }
    } catch {
        return $null
    }
}

function Add-Modules($Workbook, [object[]]$Modules) {
    foreach ($module in $Modules) {
        $component = $Workbook.VBProject.VBComponents.Add($module.kind)
        $component.Name = $module.name
        [void]$component.CodeModule.AddFromString($module.code)
    }
}

function Show-FirstCodePane($Workbook) {
    try {
        $component = $Workbook.VBProject.VBComponents.Item(1)
        $component.CodeModule.CodePane.Show()
    } catch {
        # Helpful for selected-line capture, but not required for compile.
    }
}

function New-WorkbookProject($Excel, $Project, [string]$Path) {
    $wb = $Excel.Workbooks.Add()
    $wb.VBProject.Name = $Project.name
    Add-Modules $wb $Project.modules
    if ($Path) {
        $wb.SaveAs($Path, 52)
    }
    return $wb
}

function Invoke-Case($Case) {
    $before = Get-ExcelPids
    $xl = $null
    $ownedExcelPid = $null
    $tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("oxvba-scoping-oracle-" + [System.Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force -Path $tempDir | Out-Null

    $compileStatus = "not-run"
    $dialog = $null
    $runStatus = "not-run"
    $runValue = $null
    $errorMessage = $null
    $referencePaths = @()
    $workbooks = @()

    try {
        $xl = New-Object -ComObject Excel.Application
        $xl.Visible = $true
        $xl.DisplayAlerts = $false
        $xl.AutomationSecurity = 1
        $ownedExcelPid = Get-OwnedExcelPid $before
        if ($null -eq $ownedExcelPid) {
            throw "Excel did not create a new owned process; refusing to probe an unowned instance"
        }

        foreach ($reference in @($Case.references)) {
            $path = Join-Path $tempDir ($reference.name + ".xlsm")
            $refWb = New-WorkbookProject $xl $reference $path
            $workbooks += $refWb
            $referencePaths += $path
        }

        $activePath = Join-Path $tempDir ($Case.project.name + ".xlsm")
        $activeWb = New-WorkbookProject $xl $Case.project $activePath
        $workbooks += $activeWb

        foreach ($path in $referencePaths) {
            [void]$activeWb.VBProject.References.AddFromFile($path)
        }

        $activeWb.Activate()
        $xl.VBE.MainWindow.Visible = $true
        Show-FirstCodePane $activeWb
        Start-Sleep -Milliseconds 300

        $compileControl = Get-VbeCompileControl $xl.VBE
        if ($null -eq $compileControl) {
            throw "VBE compile command id 578 was not found"
        }

        $compileControl.Execute()
        Start-Sleep -Milliseconds 700
        $dialog = Get-OwnedDialogSnapshot $ownedExcelPid
        if ($dialog) {
            $comSelected = Get-ComSelectedCode $xl.VBE
            if ($comSelected) {
                if (-not $dialog.selectedText) {
                    $dialog.selectedText = $comSelected.selectedText
                }
                if (-not $dialog.selectedLine) {
                    $dialog.selectedLine = $comSelected.selectedLine
                }
                if (-not $dialog.vbeWindow) {
                    $dialog.vbeWindow = $comSelected.vbeWindow
                }
            }
            $compileStatus = "compile-error"
            Dismiss-OwnedDialogs $ownedExcelPid
        } else {
            $compileStatus = "ok"
        }

        if ($compileStatus -eq "ok" -and $Case.run) {
            try {
                $runValue = $xl.Run("'" + $activeWb.Name + "'!" + $Case.run)
                $runStatus = "ok"
            } catch {
                Start-Sleep -Milliseconds 500
                $dialog = Get-OwnedDialogSnapshot $ownedExcelPid
                if ($dialog) {
                    $comSelected = Get-ComSelectedCode $xl.VBE
                    if ($comSelected) {
                        if (-not $dialog.selectedText) {
                            $dialog.selectedText = $comSelected.selectedText
                        }
                        if (-not $dialog.selectedLine) {
                            $dialog.selectedLine = $comSelected.selectedLine
                        }
                        if (-not $dialog.vbeWindow) {
                            $dialog.vbeWindow = $comSelected.vbeWindow
                        }
                    }
                    Dismiss-OwnedDialogs $ownedExcelPid
                }
                $runStatus = "error"
                $errorMessage = $_.Exception.Message
            }
        }
    } catch {
        $compileStatus = "harness-error"
        $errorMessage = $_.Exception.Message
    } finally {
        if ($ownedExcelPid) {
            Dismiss-OwnedDialogs $ownedExcelPid
        }
        foreach ($wb in @($workbooks)) {
            try {
                $wb.Close($false)
            } catch {
                # Process-scoped cleanup follows.
            }
        }
        if ($xl) {
            try {
                $xl.Quit()
            } catch {
                # Process-scoped cleanup follows.
            }
        }
        if ($ownedExcelPid) {
            Stop-Process -Id $ownedExcelPid -Force -ErrorAction SilentlyContinue
        }
        Remove-Item -LiteralPath $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    }

    [pscustomobject]@{
        id = $Case.id
        purpose = $Case.purpose
        ownedExcelPid = $ownedExcelPid
        project = $Case.project.name
        references = @($Case.references | ForEach-Object { $_.name })
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
        modules = @($Case.project.modules | ForEach-Object { $_.name })
    }
}

$refTools = New-ProjectSpec "LibProj" @(
    New-ModuleSpec "RefTools" 1 @"
Public Function RefValue() As Long
    RefValue = 30
End Function
"@
)

$libA = New-ProjectSpec "LibA" @(
    New-ModuleSpec "PickTools" 1 @"
Public Function Pick() As Long
    Pick = 1
End Function
"@
)

$libB = New-ProjectSpec "LibB" @(
    New-ModuleSpec "PickTools" 1 @"
Public Function Pick() As Long
    Pick = 2
End Function
"@
)

$hiddenLib = New-ProjectSpec "LibProj" @(
    New-ModuleSpec "HiddenTools" 1 @"
Option Private Module

Public Function HiddenValue() As Long
    HiddenValue = 77
End Function
"@
)

$activeWithEvents = New-ProjectSpec "AppProj" @(
    (New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    Dim clock As Clock
    Dim listener As Listener
    Set clock = New Clock
    Set listener = New Listener
    listener.Hook clock
    clock.Fire
    RunProbe = listener.Fired
End Function
"@),
    (New-ModuleSpec "Clock" 2 @"
Public Event Tick(ByVal n As Long)

Public Sub Fire()
    RaiseEvent Tick(23)
End Sub
"@),
    (New-ModuleSpec "Listener" 2 @"
Private WithEvents src As Clock
Public Fired As Long

Public Sub Hook(ByVal clock As Clock)
    Set src = clock
End Sub

Private Sub src_Tick(ByVal n As Long)
    Fired = n
End Sub
"@)
)

$cases = @(
    [pscustomobject]@{
        id = "SCOPING-XREF-BASELINE"
        purpose = "Active project has two standard modules and calls a referenced project public function by unqualified name."
        run = "Main.RunProbe"
        references = @($refTools)
        project = New-ProjectSpec "AppProj" @(
            (New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    RunProbe = LocalValue() + RefValue()
End Function
"@),
            (New-ModuleSpec "LocalTools" 1 @"
Public Function LocalValue() As Long
    LocalValue = 12
End Function
"@)
        )
    },
    [pscustomobject]@{
        id = "SCOPING-XREF-MODULE-QUALIFIED"
        purpose = "Active and referenced standard-module members are callable through Module.Member qualification."
        run = "Main.RunProbe"
        references = @($refTools)
        project = New-ProjectSpec "AppProj" @(
            (New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    RunProbe = LocalTools.LocalValue() + RefTools.RefValue()
End Function
"@),
            (New-ModuleSpec "LocalTools" 1 @"
Public Function LocalValue() As Long
    LocalValue = 12
End Function
"@)
        )
    },
    [pscustomobject]@{
        id = "SCOPING-XREF-PROJECT-QUALIFIED"
        purpose = "A referenced project member is reachable through Project.Module.Member qualification."
        run = "Main.RunProbe"
        references = @($refTools)
        project = New-ProjectSpec "AppProj" @(
            (New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    RunProbe = LibProj.RefTools.RefValue()
End Function
"@),
            (New-ModuleSpec "LocalTools" 1 @"
Public Function LocalValue() As Long
    LocalValue = 12
End Function
"@)
        )
    },
    [pscustomobject]@{
        id = "SCOPING-CONST-VAR-COLLISION"
        purpose = "A Public Const and Public variable with the same name in standard modules make unqualified use ambiguous."
        run = $null
        references = @($refTools)
        project = New-ProjectSpec "AppProj" @(
            (New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    RunProbe = SharedName
End Function
"@),
            (New-ModuleSpec "Alpha" 1 "Public Const SharedName As Long = 1`r`n"),
            (New-ModuleSpec "Beta" 1 "Public SharedName As Long`r`n")
        )
    },
    [pscustomobject]@{
        id = "SCOPING-UDT-ENUM-COLLISION"
        purpose = "A Public Type and Public Enum with the same name make a type reference ambiguous."
        run = $null
        references = @($refTools)
        project = New-ProjectSpec "AppProj" @(
            (New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    Dim value As Payload
    RunProbe = 1
End Function
"@),
            (New-ModuleSpec "Types" 1 @"
Public Type Payload
    Value As Long
End Type
"@),
            (New-ModuleSpec "Enums" 1 @"
Public Enum Payload
    PayloadA = 1
End Enum
"@)
        )
    },
    [pscustomobject]@{
        id = "SCOPING-OPTION-PRIVATE-XREF"
        purpose = "Option Private Module in a referenced project hides the module's Public procedure from external callers."
        run = $null
        references = @($hiddenLib)
        project = New-ProjectSpec "AppProj" @(
            (New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    RunProbe = HiddenValue()
End Function
"@),
            (New-ModuleSpec "LocalTools" 1 @"
Public Function LocalValue() As Long
    LocalValue = 12
End Function
"@)
        )
    },
    [pscustomobject]@{
        id = "SCOPING-XREF-PRECEDENCE"
        purpose = "Reference order chooses the first project for an unqualified duplicate member, while Project.Module.Member disambiguates another reference."
        run = "Main.RunProbe"
        references = @($libA, $libB)
        project = New-ProjectSpec "AppProj" @(
            (New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    RunProbe = Pick() * 100 + LibB.PickTools.Pick()
End Function
"@),
            (New-ModuleSpec "LocalTools" 1 @"
Public Function LocalValue() As Long
    LocalValue = 0
End Function
"@)
        )
    },
    [pscustomobject]@{
        id = "SCOPING-WITHEVENTS-ACTIVE"
        purpose = "An active-project WithEvents source routes its event to the sink handler."
        run = "Main.RunProbe"
        references = @()
        project = $activeWithEvents
    }
)

$results = @()
foreach ($case in $cases) {
    Write-Host "Running $($case.id)"
    $results += Invoke-Case $case
}

$jsonPath = Join-Path $outDir "results.json"
$results | ConvertTo-Json -Depth 10 | Set-Content -Encoding UTF8 $jsonPath

$summaryPath = Join-Path $outDir "summary.md"
$capturedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# VM3 Scoping Follow-up Excel Oracle")
$lines.Add("")
$lines.Add("- Run ID: $RunId")
$lines.Add("- Captured: $capturedAt")
$lines.Add("- Harness: $($MyInvocation.MyCommand.Path)")
$lines.Add("- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE document when exposed, owned-dialog dismissal, then PID-scoped process cleanup.")
$lines.Add("")
$lines.Add("| Case | Compile | Dialog | Selected | Run | Value |")
$lines.Add("|---|---|---|---|---|---|")
foreach ($result in $results) {
    $dialogText = if ($result.dialogText) { ($result.dialogText -replace "`r?`n", " / ") } else { "" }
    $selected = if ($result.selectedText) { ($result.selectedText -replace "`r?`n", " / ") } else { "" }
    $lines.Add("| $($result.id) | $($result.compileStatus) | $dialogText | $selected | $($result.runStatus) | $($result.runValue) |")
}
$lines.Add("")
$lines.Add("Raw JSON: results.json")
$lines | Set-Content -Encoding UTF8 $summaryPath

Write-Host "Wrote $summaryPath"
Write-Host "Wrote $jsonPath"
