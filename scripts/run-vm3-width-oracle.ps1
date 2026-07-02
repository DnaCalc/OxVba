param(
    [string]$RunId = ("vm3_width_oracle_{0:yyyyMMddTHHmmssZ}" -f (Get-Date).ToUniversalTime()),
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

$supportCode = @'
Private Function ProbePath(ByVal id As String) As String
    ProbePath = Environ$("TEMP") & "\oxvba_width_" & id & "_" & CStr(Int(Timer * 1000)) & ".txt"
End Function

Private Function ReadAll(ByVal path As String) As String
    Dim f As Integer
    f = FreeFile
    Open path For Binary As #f
    If LOF(f) > 0 Then
        ReadAll = Space$(LOF(f))
        Get #f, , ReadAll
    Else
        ReadAll = ""
    End If
    Close #f
End Function

Private Function Visible(ByVal text As String) As String
    text = Replace(text, " ", "_")
    text = Replace(text, vbCr, "<CR>")
    text = Replace(text, vbLf, "<LF>")
    Visible = text
End Function
'@

function New-WidthCase([string]$Id, [string]$Purpose, [string]$Body) {
    New-OracleCase $Id $Purpose @"
$supportCode

Public Function RunProbe() As Variant
$Body
End Function
"@
}

$cases = @()
$cases += (New-WidthCase "WIDTH-LONG-FIELD" "Width wraps a single long Print field." @'
    Dim path As String, f As Integer
    path = ProbePath("long")
    f = FreeFile
    Open path For Output As #f
    Width #f, 5
    Print #f, "abcdef"
    Close #f
    RunProbe = Visible(ReadAll(path))
    Kill path
'@)
$cases += (New-WidthCase "WIDTH-ADJACENT-FIELDS" "Width wraps adjacent semicolon fields." @'
    Dim path As String, f As Integer
    path = ProbePath("adjacent")
    f = FreeFile
    Open path For Output As #f
    Width #f, 5
    Print #f, "ab"; "cd"; "ef"
    Close #f
    RunProbe = Visible(ReadAll(path))
    Kill path
'@)
$cases += (New-WidthCase "WIDTH-CROSS-STATEMENT" "Width wrapping continues across a suppressed newline." @'
    Dim path As String, f As Integer
    path = ProbePath("cross")
    f = FreeFile
    Open path For Output As #f
    Width #f, 5
    Print #f, "abcde";
    Print #f, "f"
    Close #f
    RunProbe = Visible(ReadAll(path))
    Kill path
'@)
$cases += (New-WidthCase "WIDTH-NUMERIC-WRAP" "Width wraps before the next numeric print field when padded text would overflow." @'
    Dim path As String, f As Integer
    path = ProbePath("numeric")
    f = FreeFile
    Open path For Output As #f
    Width #f, 5
    Print #f, 12; 34
    Close #f
    RunProbe = Visible(ReadAll(path))
    Kill path
'@)
$cases += (New-WidthCase "WIDTH-COMMA-ZONE" "Width interacts with comma print zones." @'
    Dim path As String, f As Integer
    path = ProbePath("comma")
    f = FreeFile
    Open path For Output As #f
    Width #f, 10
    Print #f, "a", "b"
    Close #f
    RunProbe = Visible(ReadAll(path))
    Kill path
'@)
$cases += (New-WidthCase "WIDTH-SPC-TAB" "Width interacts with Spc, explicit Tab, and bare Tab." @'
    Dim path As String, f As Integer
    path = ProbePath("spctab")
    f = FreeFile
    Open path For Output As #f
    Width #f, 5
    Print #f, "A"; Spc(3); "B"; Tab(3); "C"; Tab; "D"
    Close #f
    RunProbe = Visible(ReadAll(path))
    Kill path
'@)
$cases += (New-WidthCase "WIDTH-SPC-LONG" "Width behavior for Spc count larger than the width." @'
    Dim path As String, f As Integer
    path = ProbePath("spclong")
    f = FreeFile
    Open path For Output As #f
    Width #f, 5
    Print #f, Spc(6); "A"
    Close #f
    RunProbe = Visible(ReadAll(path))
    Kill path
'@)
$cases += (New-WidthCase "WIDTH-TAB-FAR" "Width behavior for explicit Tab beyond the width." @'
    Dim path As String, f As Integer
    path = ProbePath("tabfar")
    f = FreeFile
    Open path For Output As #f
    Width #f, 5
    Print #f, Tab(10); "A"
    Close #f
    RunProbe = Visible(ReadAll(path))
    Kill path
'@)
$cases += (New-WidthCase "WIDTH-WRITE-UNAFFECTED" "Width does not wrap Write # records." @'
    Dim path As String, f As Integer
    path = ProbePath("write")
    f = FreeFile
    Open path For Output As #f
    Width #f, 5
    Write #f, "abcdef", 1
    Close #f
    RunProbe = Visible(ReadAll(path))
    Kill path
'@)
$cases += (New-WidthCase "WIDTH-ZERO-DISABLES" "Width 0 disables wrapping." @'
    Dim path As String, f As Integer
    path = ProbePath("zero")
    f = FreeFile
    Open path For Output As #f
    Width #f, 0
    Print #f, "abcdef"
    Close #f
    RunProbe = Visible(ReadAll(path))
    Kill path
'@)
$cases += (New-WidthCase "WIDTH-REOPEN-RESETS" "Closing and reopening resets the active width." @'
    Dim path As String, f As Integer
    path = ProbePath("reopen")
    f = FreeFile
    Open path For Output As #f
    Width #f, 5
    Close #f
    f = FreeFile
    Open path For Output As #f
    Print #f, "ab"; "cd"; "ef"
    Close #f
    RunProbe = Visible(ReadAll(path))
    Kill path
'@)
$cases += (New-WidthCase "WIDTH-NEGATIVE-ERROR" "Negative Width value error behavior." @'
    Dim path As String, f As Integer
    path = ProbePath("negative")
    f = FreeFile
    Open path For Output As #f
    On Error Resume Next
    Width #f, -1
    RunProbe = CStr(Err.Number) & "|" & Err.Description
    Close #f
    Kill path
'@)
$cases += (New-WidthCase "WIDTH-256-ERROR" "Width 256 boundary behavior." @'
    Dim path As String, f As Integer
    path = ProbePath("wide")
    f = FreeFile
    Open path For Output As #f
    On Error Resume Next
    Width #f, 256
    RunProbe = CStr(Err.Number) & "|" & Err.Description
    Close #f
    Kill path
'@)

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
$lines.Add("# VM3 Width # Excel Oracle")
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
