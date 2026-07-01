param(
    [string]$RunId = ("vm3_err_help_oracle_{0:yyyyMMddTHHmmssZ}" -f (Get-Date).ToUniversalTime()),
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
    [pscustomobject]@{ name = $Name; kind = $Kind; code = $Code }
}

function New-OracleCase([string]$Id, [string]$Purpose, [string]$Code, [string]$Run = "Main.RunProbe") {
    [pscustomobject]@{
        id = $Id
        purpose = $Purpose
        run = $Run
        modules = @(New-ModuleSpec "Main" 1 $Code)
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
            $component = $wb.VBProject.VBComponents.Add($module.kind)
            $component.Name = $module.name
            [void]$component.CodeModule.AddFromString($module.code)
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

function New-ErrCase([string]$Id, [string]$Purpose, [string]$Code) {
    New-OracleCase $Id $Purpose $Code
}

$fieldExpr = 'CStr(Err.Number) & "|" & Err.Description & "|" & Err.Source & "|" & Err.HelpFile & "|" & CStr(Err.HelpContext) & "|" & CStr(Err.LastDllError)'
$raiseFieldExpr = 'CStr(Err.Number) & "|" & Err.Description & "|" & Err.Source & "|" & Err.HelpFile & "|" & CStr(Err.HelpContext)'

$cases = @()
$cases += (New-ErrCase "ERR-INITIAL-FIELDS" "Initial Err help fields." @"
Public Function RunProbe() As Variant
    RunProbe = $fieldExpr
End Function
"@)
$cases += (New-ErrCase "ERR-HELP-WRITE-READ" "Err.HelpFile and Err.HelpContext writes are readable." @"
Public Function RunProbe() As Variant
    Err.HelpFile = "help.chm"
    Err.HelpContext = 42
    RunProbe = Err.HelpFile & "|" & CStr(Err.HelpContext)
End Function
"@)
$cases += (New-ErrCase "ERR-CLEAR-RESETS-HELP" "Err.Clear resets help fields." @"
Public Function RunProbe() As Variant
    Err.HelpFile = "help.chm"
    Err.HelpContext = 42
    Err.Clear
    RunProbe = $fieldExpr
End Function
"@)
$cases += (New-ErrCase "ERROR-STATEMENT-HELP-DEFAULTS" "The legacy Error statement populates default help fields." @"
Public Function RunProbe() As Variant
    On Error Resume Next
    Error 9
    RunProbe = $fieldExpr
End Function
"@)
$cases += (New-ErrCase "ERR-RAISE-EXPLICIT-HELP" "Err.Raise explicit positional help fields." @"
Public Function RunProbe() As Variant
    On Error GoTo EH
    Err.Raise 77, "src", "desc", "help.chm", 42
    RunProbe = "miss"
    Exit Function
EH:
    RunProbe = $raiseFieldExpr
End Function
"@)
$cases += (New-ErrCase "ERR-RAISE-NAMED-HELP" "Err.Raise named help fields." @"
Public Function RunProbe() As Variant
    On Error GoTo EH
    Err.Raise Number:=78, HelpContext:=43, HelpFile:="named.hlp", Description:="desc2", Source:="src2"
    RunProbe = "miss"
    Exit Function
EH:
    RunProbe = $raiseFieldExpr
End Function
"@)
$cases += (New-ErrCase "ERR-RAISE-OMITTED-INHERITS-HELP" "Err.Raise omitted fields inherit un-cleared Err help fields." @"
Public Function RunProbe() As Variant
    Err.Description = "prevdesc"
    Err.Source = "prevsrc"
    Err.HelpFile = "prev.hlp"
    Err.HelpContext = 9
    On Error GoTo EH
    Err.Raise 79
    RunProbe = "miss"
    Exit Function
EH:
    RunProbe = $raiseFieldExpr
End Function
"@)
$cases += (New-ErrCase "ERR-RAISE-AFTER-CLEAR-DEFAULTS-HELP" "Err.Clear prevents omitted help inheritance." @"
Public Function RunProbe() As Variant
    Err.Description = "prevdesc"
    Err.Source = "prevsrc"
    Err.HelpFile = "prev.hlp"
    Err.HelpContext = 9
    Err.Clear
    On Error GoTo EH
    Err.Raise 80
    RunProbe = "miss"
    Exit Function
EH:
    RunProbe = $raiseFieldExpr
End Function
"@)
$cases += (New-ErrCase "ERR-RAISE-PARTIAL-HELP-INHERIT" "Explicit HelpFile with omitted HelpContext." @"
Public Function RunProbe() As Variant
    Err.HelpFile = "prev.hlp"
    Err.HelpContext = 9
    On Error GoTo EH
    Err.Raise 81, , , "explicit.hlp"
    RunProbe = "miss"
    Exit Function
EH:
    RunProbe = $raiseFieldExpr
End Function
"@)
$cases += (New-ErrCase "ERR-RAISE-RESUME-NEXT-INHERITS-ACTUAL" "Omitted Err.Raise fields after a prior caught Err.Raise." @"
Public Function RunProbe() As Variant
    On Error Resume Next
    Err.Raise 5, "prevsrc", "prevdesc", "prev.hlp", 9
    Err.Raise 79
    RunProbe = $raiseFieldExpr
End Function
"@)
$cases += (New-ErrCase "ERR-RAISE-RESUME-NEXT-DIRECT-WRITES" "Omitted Err.Raise fields after direct Err property writes." @"
Public Function RunProbe() As Variant
    On Error Resume Next
    Err.Description = "prevdesc"
    Err.Source = "prevsrc"
    Err.HelpFile = "prev.hlp"
    Err.HelpContext = 9
    Err.Raise 79
    RunProbe = $raiseFieldExpr
End Function
"@)
$cases += (New-ErrCase "ERR-RAISE-PARTIAL-HELP-AFTER-ACTUAL" "Explicit HelpFile with omitted HelpContext after a prior caught Err.Raise." @"
Public Function RunProbe() As Variant
    On Error Resume Next
    Err.Raise 5, "prevsrc", "prevdesc", "prev.hlp", 9
    Err.Raise 81, , , "explicit.hlp"
    RunProbe = $raiseFieldExpr
End Function
"@)

if ($CaseId.Count -gt 0) {
    $wanted = @{}
    foreach ($id in $CaseId) {
        $wanted[$id] = $true
    }
    $cases = @($cases | Where-Object { $wanted.ContainsKey($_.id) })
    if ($cases.Count -eq 0) {
        throw "No cases matched -CaseId: $($CaseId -join ', ')"
    }
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
$lines.Add("# VM3 Err HelpFile/HelpContext Excel Oracle")
$lines.Add("")
$lines.Add("- Run ID: $RunId")
$lines.Add("- Captured: $capturedAt")
$lines.Add("- Harness: $($MyInvocation.MyCommand.Path)")
$lines.Add("- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE, owned-dialog dismissal, then PID-scoped process cleanup.")
$lines.Add("")
$lines.Add("| Case | Compile | Dialog | Selected | Run | Value |")
$lines.Add("|---|---|---|---|---|---|")
foreach ($result in $results) {
    $dialogText = if ($result.dialogText) { ($result.dialogText -replace "`r?`n", " / ") } else { "" }
    $selected = if ($result.selectedLine) { ($result.selectedLine -replace "`r?`n", " / ") } else { $result.selectedText }
    $value = if ($result.runValue) { $result.runValue.Replace("|", "\|") } else { "" }
    $lines.Add("| $($result.id) | $($result.compileStatus) | $dialogText | $selected | $($result.runStatus) | $value |")
}
$lines.Add("")
$lines.Add("Raw JSON: results.json")
$lines | Set-Content -Encoding UTF8 $summaryPath

Write-Host "Wrote $summaryPath"
Write-Host "Wrote $jsonPath"
