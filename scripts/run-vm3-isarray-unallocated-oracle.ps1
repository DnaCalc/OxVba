param(
    [string]$RunId = ("vm3_isarray_unallocated_oracle_{0:yyyyMMddTHHmmssZ}" -f (Get-Date).ToUniversalTime()),
    [string]$OutputRoot = "docs/evidence/conformance"
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$outDir = Join-Path (Join-Path $repoRoot $OutputRoot) $RunId
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

function New-OracleCase([string]$Id, [string]$Purpose, [string]$Code, [string]$Run = "Main.RunProbe") {
    [pscustomobject]@{
        id = $Id
        purpose = $Purpose
        run = $Run
        modules = @(
            [pscustomobject]@{ name = "Main"; kind = 1; code = $Code }
        )
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

$cases = @()
$cases += (New-OracleCase "DYNAMIC-UNALLOCATED-LONG" "Declared dynamic array before ReDim." @"
Public Function RunProbe() As Variant
    Dim a() As Long
    RunProbe = CStr(IsArray(a))
End Function
"@)
$cases += (New-OracleCase "DYNAMIC-UNALLOCATED-LONG-LBOUND" "LBound on declared dynamic array before ReDim." @"
Public Function RunProbe() As Variant
    Dim a() As Long
    Dim n As Long
    On Error Resume Next
    n = LBound(a)
    RunProbe = CStr(IsArray(a)) & ":" & CStr(Err.Number)
End Function
"@)
$cases += (New-OracleCase "DYNAMIC-UNALLOCATED-LONG-UBOUND" "UBound on declared dynamic array before ReDim." @"
Public Function RunProbe() As Variant
    Dim a() As Long
    Dim n As Long
    On Error Resume Next
    n = UBound(a)
    RunProbe = CStr(IsArray(a)) & ":" & CStr(Err.Number)
End Function
"@)
$cases += (New-OracleCase "DYNAMIC-UNALLOCATED-LONG-INTROSPECTION" "VarType/TypeName on declared dynamic array before ReDim." @"
Public Function RunProbe() As Variant
    Dim a() As Long
    RunProbe = CStr(IsArray(a)) & ":" & CStr(VarType(a)) & ":" & TypeName(a)
End Function
"@)
$cases += (New-OracleCase "DYNAMIC-ALLOCATED-LONG" "Dynamic typed array after ReDim." @"
Public Function RunProbe() As Variant
    Dim a() As Long
    ReDim a(1 To 3)
    RunProbe = CStr(IsArray(a))
End Function
"@)
$cases += (New-OracleCase "DYNAMIC-ERASED-LONG" "Dynamic typed array after Erase." @"
Public Function RunProbe() As Variant
    Dim a() As Long
    ReDim a(1 To 3)
    Erase a
    RunProbe = CStr(IsArray(a))
End Function
"@)
$cases += (New-OracleCase "DYNAMIC-ERASED-LONG-LBOUND" "LBound on dynamic typed array after Erase." @"
Public Function RunProbe() As Variant
    Dim a() As Long
    Dim n As Long
    ReDim a(1 To 3)
    Erase a
    On Error Resume Next
    n = LBound(a)
    RunProbe = CStr(IsArray(a)) & ":" & CStr(Err.Number)
End Function
"@)
$cases += (New-OracleCase "DYNAMIC-ERASED-LONG-UBOUND" "UBound on dynamic typed array after Erase." @"
Public Function RunProbe() As Variant
    Dim a() As Long
    Dim n As Long
    ReDim a(1 To 3)
    Erase a
    On Error Resume Next
    n = UBound(a)
    RunProbe = CStr(IsArray(a)) & ":" & CStr(Err.Number)
End Function
"@)
$cases += (New-OracleCase "DYNAMIC-ERASED-LONG-INTROSPECTION" "VarType/TypeName on dynamic typed array after Erase." @"
Public Function RunProbe() As Variant
    Dim a() As Long
    ReDim a(1 To 3)
    Erase a
    RunProbe = CStr(IsArray(a)) & ":" & CStr(VarType(a)) & ":" & TypeName(a)
End Function
"@)
$cases += (New-OracleCase "FIXED-LONG" "Fixed-size typed array." @"
Public Function RunProbe() As Variant
    Dim a(1 To 3) As Long
    RunProbe = CStr(IsArray(a))
End Function
"@)
$cases += (New-OracleCase "FIXED-ERASED-LONG" "Fixed-size typed array after Erase." @"
Public Function RunProbe() As Variant
    Dim a(1 To 3) As Long
    Erase a
    RunProbe = CStr(IsArray(a))
End Function
"@)
$cases += (New-OracleCase "VARIANT-EMPTY" "Empty Variant control." @"
Public Function RunProbe() As Variant
    Dim v As Variant
    RunProbe = CStr(IsArray(v))
End Function
"@)
$cases += (New-OracleCase "VARIANT-ARRAY-LITERAL" "Variant holding Array() result." @"
Public Function RunProbe() As Variant
    Dim v As Variant
    v = Array(1, 2)
    RunProbe = CStr(IsArray(v))
End Function
"@)
$cases += (New-OracleCase "VARIANT-DYNAMIC-ARRAY-COPY" "Variant assigned from allocated dynamic typed array." @"
Public Function RunProbe() As Variant
    Dim a() As Long
    Dim v As Variant
    ReDim a(1 To 3)
    v = a
    RunProbe = CStr(IsArray(v))
End Function
"@)
$cases += (New-OracleCase "VARIANT-UNALLOCATED-DYNAMIC-COPY" "Variant assigned from unallocated dynamic typed array." @"
Public Function RunProbe() As Variant
    Dim a() As Long
    Dim v As Variant
    v = a
    RunProbe = CStr(IsArray(v))
End Function
"@)
$cases += (New-OracleCase "VARIANT-UNALLOCATED-DYNAMIC-COPY-INTROSPECTION" "VarType/TypeName on Variant assigned from unallocated dynamic typed array." @"
Public Function RunProbe() As Variant
    Dim a() As Long
    Dim v As Variant
    v = a
    RunProbe = CStr(IsArray(v)) & ":" & CStr(VarType(v)) & ":" & TypeName(v)
End Function
"@)
$cases += (New-OracleCase "ERASE-VARIANT-ARRAY" "Erase on Variant holding Array() result." @"
Public Function RunProbe() As Variant
    Dim v As Variant
    v = Array(1, 2)
    Erase v
    RunProbe = CStr(IsArray(v))
End Function
"@)
$cases += (New-OracleCase "ERASE-VARIANT-ARRAY-LBOUND" "LBound after Erase on Variant holding Array() result." @"
Public Function RunProbe() As Variant
    Dim v As Variant
    Dim n As Long
    v = Array(1, 2)
    Erase v
    On Error Resume Next
    n = LBound(v)
    RunProbe = CStr(IsArray(v)) & ":" & CStr(Err.Number)
End Function
"@)
$cases += (New-OracleCase "ERASE-VARIANT-ARRAY-UBOUND" "UBound after Erase on Variant holding Array() result." @"
Public Function RunProbe() As Variant
    Dim v As Variant
    Dim n As Long
    v = Array(1, 2)
    Erase v
    On Error Resume Next
    n = UBound(v)
    RunProbe = CStr(IsArray(v)) & ":" & CStr(Err.Number)
End Function
"@)
$cases += (New-OracleCase "ERASE-VARIANT-ARRAY-INTROSPECTION" "VarType/TypeName after Erase on Variant holding Array() result." @"
Public Function RunProbe() As Variant
    Dim v As Variant
    v = Array(1, 2)
    Erase v
    RunProbe = CStr(IsArray(v)) & ":" & CStr(VarType(v)) & ":" & TypeName(v)
End Function
"@)

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
$lines.Add("# VM3 IsArray Unallocated Excel Oracle")
$lines.Add("")
$lines.Add("- Run ID: $RunId")
$lines.Add("- Captured: $capturedAt")
$lines.Add("- Harness: $($MyInvocation.MyCommand.Path)")
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
