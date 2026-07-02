param(
    [string]$RunId = ("vm3_record_array_field_oracle_{0:yyyyMMddTHHmmssZ}" -f (Get-Date).ToUniversalTime()),
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
            # Try another document pane if this one has no TextPattern.
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

function Dismiss-OwnedDialogs([int]$ExcelPid) {
    foreach ($window in Get-OwnedTopWindows $ExcelPid) {
        $buttonCond = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            [System.Windows.Automation.ControlType]::Button
        )
        $buttons = $window.FindAll([System.Windows.Automation.TreeScope]::Descendants, $buttonCond)
        foreach ($button in $buttons) {
            if ($button.Current.Name -eq "OK") {
                try {
                    $invoke = $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
                    $invoke.Invoke()
                    Start-Sleep -Milliseconds 200
                } catch {
                    # Best-effort modal cleanup, still scoped to this Excel PID.
                }
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
        Start-Sleep -Milliseconds 900
        $dialog = Get-OwnedCompileDialog $ownedExcelPid
        if ($dialog) {
            $compileStatus = "compile-error"
            $selection = Get-VbeSelection $ownedExcelPid
            if ($null -eq $selection -or [string]::IsNullOrWhiteSpace($selection.selectedLine)) {
                $selection = Get-VbeComSelection $xl.VBE
            }
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
        id = "UDT-FIXED-ARRAY-FIELD"
        purpose = "Fixed-array UDT field compiles and indexes."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Private Type State
    Buses(0 To 1) As Long
End Type

Public Function RunProbe() As Variant
    Dim s As State
    s.Buses(1) = 7
    RunProbe = s.Buses(1)
End Function
"@
        )
    },
    [pscustomobject]@{
        id = "UDT-FIXED-ARRAY-EXPLICIT-LOWER"
        purpose = "Fixed-array UDT field preserves an explicit nonzero lower bound."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Private Type State
    Buses(1 To 2) As Long
End Type

Public Function RunProbe() As Variant
    Dim s As State
    s.Buses(1) = 11
    s.Buses(2) = 22
    RunProbe = CStr(LBound(s.Buses)) & ":" & CStr(UBound(s.Buses)) & ":" & CStr(s.Buses(1)) & ":" & CStr(s.Buses(2))
End Function
"@
        )
    },
    [pscustomobject]@{
        id = "UDT-FIXED-ARRAY-OPTION-BASE"
        purpose = "Fixed-array UDT field with a single bound is zero-based even under Option Base 1."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Option Base 1
Private Type State
    Buses(2) As Long
End Type

Public Function RunProbe() As Variant
    Dim s As State
    s.Buses(1) = 11
    s.Buses(2) = 22
    RunProbe = CStr(LBound(s.Buses)) & ":" & CStr(UBound(s.Buses)) & ":" & CStr(s.Buses(1)) & ":" & CStr(s.Buses(2))
End Function
"@
        )
    },
    [pscustomobject]@{
        id = "UDT-FIXED-ARRAY-NEGATIVE-LOWER"
        purpose = "Fixed-array UDT field preserves an explicit negative lower bound."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Private Type State
    Buses(-2 To 0) As Long
End Type

Public Function RunProbe() As Variant
    Dim s As State
    s.Buses(-2) = 7
    s.Buses(0) = 9
    RunProbe = CStr(LBound(s.Buses)) & ":" & CStr(UBound(s.Buses)) & ":" & CStr(s.Buses(-2)) & ":" & CStr(s.Buses(0))
End Function
"@
        )
    },
    [pscustomobject]@{
        id = "UDT-FIXED-ARRAY-MULTIDIM"
        purpose = "Fixed-array UDT field preserves multidimensional bounds."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Private Type State
    Grid(1 To 2, 3 To 4) As Long
End Type

Public Function RunProbe() As Variant
    Dim s As State
    s.Grid(1, 3) = 13
    s.Grid(2, 4) = 24
    RunProbe = CStr(LBound(s.Grid, 1)) & ":" & CStr(UBound(s.Grid, 1)) & ":" & CStr(LBound(s.Grid, 2)) & ":" & CStr(UBound(s.Grid, 2)) & ":" & CStr(s.Grid(1, 3)) & ":" & CStr(s.Grid(2, 4))
End Function
"@
        )
    },
    [pscustomobject]@{
        id = "UDT-SCALAR-FIELD-INDEX-GET"
        purpose = "Scalar UDT field used with index in value context."
        run = $null
        modules = @(
            New-ModuleSpec "Main" 1 @"
Private Type State
    Value As Long
End Type

Public Function RunProbe() As Variant
    Dim s As State
    RunProbe = s.Value(0)
End Function
"@
        )
    },
    [pscustomobject]@{
        id = "UDT-SCALAR-FIELD-INDEX-SET"
        purpose = "Scalar UDT field used with index in assignment context."
        run = $null
        modules = @(
            New-ModuleSpec "Main" 1 @"
Private Type State
    Value As Long
End Type

Public Sub RunProbe()
    Dim s As State
    s.Value(0) = 7
End Sub
"@
        )
    }
)

if ($CaseId.Count -gt 0) {
    $wanted = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($id in $CaseId) {
        $null = $wanted.Add($id)
    }
    $cases = @($cases | Where-Object { $wanted.Contains($_.id) })
}
if ($cases.Count -eq 0) {
    throw "No oracle cases selected"
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

$summaryPath = Join-Path $outDir "summary.md"
$capturedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# VM3 Record Array Field Excel Oracle")
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
    $lines.Add("| $($result.id) | $($result.compileStatus) | $dialogText | $selected | $($result.runStatus) | $($result.runValue) |")
}
$lines.Add("")
$lines.Add("Raw JSON: results.json")
$lines | Set-Content -Encoding UTF8 $summaryPath

Write-Host "Wrote $summaryPath"
Write-Host "Wrote $jsonPath"
