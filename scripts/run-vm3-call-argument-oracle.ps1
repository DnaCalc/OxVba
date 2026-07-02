param(
    [string]$RunId = ("vm3_call_argument_oracle_{0:yyyyMMddTHHmmssZ}" -f (Get-Date).ToUniversalTime()),
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

function Get-OwnedTopWindows([int]$ExcelPid) {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $windows = $root.FindAll(
        [System.Windows.Automation.TreeScope]::Children,
        [System.Windows.Automation.Condition]::TrueCondition
    )
    @($windows | Where-Object { $_.Current.ProcessId -eq $ExcelPid })
}

function Get-OwnedVbeWindow([int]$ExcelPid) {
    foreach ($window in Get-OwnedTopWindows $ExcelPid) {
        if ($window.Current.Name -like "Microsoft Visual Basic for Applications*") {
            return $window
        }
    }
    return $null
}

function Get-VbeSelection([int]$ExcelPid) {
    $vbe = Get-OwnedVbeWindow $ExcelPid
    if ($null -eq $vbe) {
        return $null
    }

    $docCond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Document
    )
    $docs = $vbe.FindAll([System.Windows.Automation.TreeScope]::Descendants, $docCond)
    if ($docs.Count -eq 0) {
        return $null
    }

    for ($i = 0; $i -lt $docs.Count; $i++) {
        $doc = $docs.Item($i)
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
                vbeWindow = $vbe.Current.Name
                selectedText = $token
                selectedLine = $line
            }
        } catch {
            # Try another document pane if VBE exposes a non-text document first.
        }
    }
    return [pscustomobject]@{
        vbeWindow = $vbe.Current.Name
        selectedText = $null
        selectedLine = $null
    }
}

function Get-VbeComSelection($Vbe) {
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
            vbeWindow = $Vbe.MainWindow.Caption
            selectedText = if ($selectedText) { $selectedText.Trim() } else { $null }
            selectedLine = $lineText.Trim()
        }
    } catch {
        return $null
    }
}

function Get-OwnedCompileDialog([int]$ExcelPid) {
    foreach ($window in Get-OwnedTopWindows $ExcelPid) {
        $desc = $window.FindAll(
            [System.Windows.Automation.TreeScope]::Descendants,
            [System.Windows.Automation.Condition]::TrueCondition
        )
        $names = New-Object System.Collections.Generic.List[string]
        foreach ($item in $desc) {
            if (-not [string]::IsNullOrWhiteSpace($item.Current.Name)) {
                $names.Add($item.Current.Name)
            }
        }

        $dialogText = @($names | Where-Object { $_ -match "Compile error" }) | Select-Object -First 1
        if ($dialogText) {
            $buttons = @($names | Where-Object { $_ -in @("OK", "Help", "Cancel", "Yes", "No") })
            return [pscustomobject]@{
                element = $window
                windowName = $window.Current.Name
                dialogText = $dialogText
                buttons = $buttons
            }
        }
    }
    return $null
}

function Dismiss-OwnedDialog($Dialog) {
    if ($null -eq $Dialog -or $null -eq $Dialog.element) {
        return
    }
    $buttonCond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Button
    )
    $buttons = $Dialog.element.FindAll([System.Windows.Automation.TreeScope]::Descendants, $buttonCond)
    foreach ($button in $buttons) {
        if ($button.Current.Name -eq "OK") {
            $invoke = $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
            $invoke.Invoke()
            Start-Sleep -Milliseconds 200
            return
        }
    }
}

function Invoke-Case($Case) {
    $before = Get-ExcelPids
    $xl = $null
    $ownedExcelPid = $null
    $compileStatus = "not-run"
    $dialog = $null
    $selection = $null
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
        Start-Sleep -Milliseconds 700
        $dialog = Get-OwnedCompileDialog $ownedExcelPid
        if ($dialog) {
            $compileStatus = "compile-error"
            $selection = Get-VbeSelection $ownedExcelPid
            if ($null -eq $selection -or [string]::IsNullOrWhiteSpace($selection.selectedLine)) {
                $selection = Get-VbeComSelection $xl.VBE
            }
            Dismiss-OwnedDialog $dialog
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
            }
        }
    } catch {
        $compileStatus = "harness-error"
        $errorMessage = $_.Exception.Message
    } finally {
        if ($ownedExcelPid) {
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
        dialogButtons = if ($dialog) { $dialog.buttons } else { @() }
        selectedText = if ($selection) { $selection.selectedText } else { $null }
        selectedLine = if ($selection) { $selection.selectedLine } else { $null }
        vbeWindow = if ($selection) { $selection.vbeWindow } else { $null }
        run = $Case.run
        runStatus = $runStatus
        runValue = if ($null -ne $runValue) { [string]$runValue } else { $null }
        errorMessage = $errorMessage
        modules = @($Case.modules | ForEach-Object { $_.name })
    }
}

$cases = @(
    [pscustomobject]@{
        id = "CALL-BARE-BYREF-MUTATES"
        purpose = "Unparenthesized statement-form ByRef argument writes back."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    Dim x As Long
    x = 5
    Inc x
    RunProbe = x
End Function

Private Sub Inc(ByRef n As Long)
    n = n + 100
End Sub
"@
        )
    },
    [pscustomobject]@{
        id = "CALL-BYVAL-PARAM-NO-MUTATE"
        purpose = "ByVal parameter mutation does not write back."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    Dim x As Long
    x = 5
    Touch x
    RunProbe = x
End Function

Private Sub Touch(ByVal n As Long)
    n = n + 100
End Sub
"@
        )
    },
    [pscustomobject]@{
        id = "CALL-STMT-PAREN-BYVAL"
        purpose = "Parenthesized statement-form l-value argument is forced ByVal."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    Dim x As Long
    x = 5
    Inc (x)
    RunProbe = x
End Function

Private Sub Inc(ByRef n As Long)
    n = n + 100
End Sub
"@
        )
    },
    [pscustomobject]@{
        id = "CALL-FORM-PARENS-BYREF"
        purpose = "Call form with parentheses still passes the l-value ByRef."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    Dim x As Long
    x = 5
    Call Inc(x)
    RunProbe = x
End Function

Private Sub Inc(ByRef n As Long)
    n = n + 100
End Sub
"@
        )
    },
    [pscustomobject]@{
        id = "CALL-BYREF-TYPE-MISMATCH"
        purpose = "ByRef argument with mismatched declared type is rejected."
        run = $null
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Sub RunProbe()
    Dim x As Integer
    TakeLong x
End Sub

Private Sub TakeLong(ByRef n As Long)
    n = 7
End Sub
"@
        )
    },
    [pscustomobject]@{
        id = "CALL-EXTRA-ARG"
        purpose = "Extra procedure argument is rejected."
        run = $null
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Sub RunProbe()
    TakeOne 1, 2
End Sub

Private Sub TakeOne(ByVal n As Long)
End Sub
"@
        )
    },
    [pscustomobject]@{
        id = "CALL-MISSING-ARG"
        purpose = "Missing required procedure argument is rejected."
        run = $null
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Sub RunProbe()
    TakeTwo 1
End Sub

Private Sub TakeTwo(ByVal a As Long, ByVal b As Long)
End Sub
"@
        )
    },
    [pscustomobject]@{
        id = "CALL-OPTIONAL-MISSING-OK"
        purpose = "Missing optional argument uses its default."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    RunProbe = AddOpt(5)
End Function

Private Function AddOpt(ByVal n As Long, Optional ByVal bonus As Long = 7) As Long
    AddOpt = n + bonus
End Function
"@
        )
    },
    [pscustomobject]@{
        id = "CALL-PARAMARRAY-EXTRA-OK"
        purpose = "ParamArray accepts extra positional arguments."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    RunProbe = SumAll(1, 2, 3)
End Function

Private Function SumAll(ParamArray xs() As Variant) As Long
    Dim i As Long
    For i = LBound(xs) To UBound(xs)
        SumAll = SumAll + CLng(xs(i))
    Next i
End Function
"@
        )
    },
    [pscustomobject]@{
        id = "CALL-PARAMARRAY-SCALAR-ELEMENT-ALIASES-CALLER"
        purpose = "Assigning a ParamArray element sourced from a scalar variable writes back to the caller in real VBA."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    Dim x As Long
    x = 5
    Touch x
    RunProbe = x
End Function

Private Sub Touch(ParamArray xs() As Variant)
    xs(0) = 99
End Sub
"@
        )
    },
    [pscustomobject]@{
        id = "CALL-PARAMARRAY-VARIANT-ELEMENT-ALIASES-CALLER"
        purpose = "Assigning a ParamArray element sourced from a Variant variable writes back to the caller in real VBA."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    Dim v As Variant
    v = 5
    Touch v
    RunProbe = v
End Function

Private Sub Touch(ParamArray xs() As Variant)
    xs(0) = 99
End Sub
"@
        )
    },
    [pscustomobject]@{
        id = "CALL-PARAMARRAY-ARRAY-ELEMENT-LVALUE-ALIASES-CALLER"
        purpose = "Assigning a ParamArray element sourced from an array-element l-value writes back to the caller in real VBA."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    Dim a(0 To 0) As Long
    a(0) = 5
    Touch a(0)
    RunProbe = a(0)
End Function

Private Sub Touch(ParamArray xs() As Variant)
    xs(0) = 99
End Sub
"@
        )
    },
    [pscustomobject]@{
        id = "CALL-PARAMARRAY-OBJECT-ELEMENT-REBIND-ALIASES-CALLER"
        purpose = "ParamArray object element mutation/rebinding affects the caller in real VBA."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    Dim box As Object
    Set box = CreateObject("Scripting.Dictionary")
    box("Value") = 5
    On Error GoTo Failed
    Touch box
    RunProbe = "ok:" & CStr(box("Value"))
    Exit Function
Failed:
    RunProbe = "err:" & CStr(Err.Number) & ":" & Err.Description
End Function

Private Sub Touch(ParamArray xs() As Variant)
    xs(0)("Value") = xs(0)("Value") + 10
    Set xs(0) = Nothing
End Sub
"@
        )
    },
    [pscustomobject]@{
        id = "CALL-PARAMARRAY-VARIANT-ARRAY-ELEMENT-MUTATION-ALIASES-CALLER"
        purpose = "Mutating an array stored inside a ParamArray element affects the caller in real VBA."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    Dim v As Variant
    v = Array(5)
    On Error GoTo Failed
    Touch v
    RunProbe = "ok:" & CStr(v(0))
    Exit Function
Failed:
    RunProbe = "err:" & CStr(Err.Number) & ":" & Err.Description
End Function

Private Sub Touch(ParamArray xs() As Variant)
    xs(0)(0) = 99
End Sub
"@
        )
    }
)

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

$summaryPath = Join-Path $outDir "summary.md"
$capturedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# VM3 Call Argument Excel Oracle")
$lines.Add("")
$lines.Add("- Run ID: $RunId")
$lines.Add("- Captured: $capturedAt")
$lines.Add("- Harness: $($MyInvocation.MyCommand.Path)")
$lines.Add("- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, selected token/line capture from the VBE document, owned-dialog dismissal, then PID-scoped process cleanup.")
$lines.Add("")
$lines.Add("| Case | Compile | Dialog | Selected | Run | Value |")
$lines.Add("|---|---|---|---|---|---|")
foreach ($result in $results) {
    $dialogText = if ($result.dialogText) { ($result.dialogText -replace "`r?`n", " / ") } else { "" }
    $selected = if ($result.selectedLine) { ($result.selectedLine -replace "`r?`n", " / ") } else { $result.selectedText }
    $lines.Add("| $($result.id) | $($result.compileStatus) | $dialogText | $selected | $($result.runStatus) | $($result.runValue) |")
}
$lines.Add("")
$lines.Add("Raw JSON: results.json")
$lines | Set-Content -Encoding UTF8 $summaryPath

Write-Host "Wrote $summaryPath"
Write-Host "Wrote $jsonPath"
