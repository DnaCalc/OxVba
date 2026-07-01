param(
    [string]$RunId = ("vm3_scoping_visibility_oracle_{0:yyyyMMddTHHmmssZ}" -f (Get-Date).ToUniversalTime()),
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

function Get-OwnedCompileDialog([int]$ExcelPid) {
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $windows = $root.FindAll(
        [System.Windows.Automation.TreeScope]::Children,
        [System.Windows.Automation.Condition]::TrueCondition
    )

    foreach ($window in $windows) {
        if ($window.Current.ProcessId -ne $ExcelPid) {
            continue
        }

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
                windowName = $window.Current.Name
                dialogText = $dialogText
                buttons = $buttons
            }
        }
    }
    return $null
}

function Invoke-Case($Case) {
    $before = Get-ExcelPids
    $xl = $null
    $ownedPid = $null
    $compileStatus = "not-run"
    $dialog = $null
    $runStatus = "not-run"
    $runValue = $null
    $errorMessage = $null

    try {
        $xl = New-Object -ComObject Excel.Application
        $xl.Visible = $true
        $xl.DisplayAlerts = $false
        $ownedPid = Get-OwnedExcelPid $before
        if ($null -eq $ownedPid) {
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
        Start-Sleep -Milliseconds 500
        $dialog = Get-OwnedCompileDialog $ownedPid
        if ($dialog) {
            $compileStatus = "compile-error"
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
        if ($ownedPid) {
            Stop-Process -Id $ownedPid -Force -ErrorAction SilentlyContinue
        }
    }

    [pscustomobject]@{
        id = $Case.id
        purpose = $Case.purpose
        ownedPid = $ownedPid
        compileStatus = $compileStatus
        dialogWindow = if ($dialog) { $dialog.windowName } else { $null }
        dialogText = if ($dialog) { $dialog.dialogText } else { $null }
        dialogButtons = if ($dialog) { $dialog.buttons } else { @() }
        run = $Case.run
        runStatus = $runStatus
        runValue = if ($null -ne $runValue) { [string]$runValue } else { $null }
        errorMessage = $errorMessage
        modules = @($Case.modules | ForEach-Object { $_.name })
    }
}

$cases = @(
    [pscustomobject]@{
        id = "SCOPING-SAME-MODULE-PRIVATE"
        purpose = "Private function is callable from the declaring standard module."
        run = "Main.RunProbe"
        modules = @(
            New-ModuleSpec "Main" 1 @"
Private Function Secret() As Long
    Secret = 7
End Function

Public Function RunProbe() As Variant
    RunProbe = Secret()
End Function
"@
        )
    },
    [pscustomobject]@{
        id = "SCOPING-CROSS-UNQUAL-PRIVATE"
        purpose = "Private function in another standard module is not visible by unqualified name."
        run = $null
        modules = @(
            (New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    RunProbe = Secret()
End Function
"@),
            (New-ModuleSpec "Lib" 1 @"
Private Function Secret() As Long
    Secret = 9
End Function
"@)
        )
    },
    [pscustomobject]@{
        id = "SCOPING-CROSS-QUAL-PRIVATE"
        purpose = "Private function in another standard module is not visible through Module.Member."
        run = $null
        modules = @(
            (New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    RunProbe = Lib.Secret()
End Function
"@),
            (New-ModuleSpec "Lib" 1 @"
Private Function Secret() As Long
    Secret = 11
End Function
"@)
        )
    },
    [pscustomobject]@{
        id = "SCOPING-DUP-PUBLIC"
        purpose = "Duplicate Public procedures in standard modules make an unqualified call ambiguous."
        run = $null
        modules = @(
            (New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    RunProbe = Dup()
End Function
"@),
            (New-ModuleSpec "Alpha" 1 @"
Public Function Dup() As Long
    Dup = 1
End Function
"@),
            (New-ModuleSpec "Beta" 1 @"
Public Function Dup() As Long
    Dup = 2
End Function
"@)
        )
    },
    [pscustomobject]@{
        id = "SCOPING-MODULE-MEMBER-COLLISION"
        purpose = "A standard module name colliding with a Public member name is ambiguous."
        run = $null
        modules = @(
            (New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    RunProbe = Clash()
End Function
"@),
            (New-ModuleSpec "Clash" 1 @"
Public Function Value() As Long
    Value = 3
End Function
"@),
            (New-ModuleSpec "Other" 1 @"
Public Function Clash() As Long
    Clash = 4
End Function
"@)
        )
    },
    [pscustomobject]@{
        id = "SCOPING-VALID-PROJECT-QUALIFIER"
        purpose = "The default Excel project qualifier can qualify a module member."
        run = "Main.RunProbe"
        modules = @(
            (New-ModuleSpec "Main" 1 @"
Option Explicit

Public Function RunProbe() As Variant
    RunProbe = VBAProject.Lib.Pub()
End Function
"@),
            (New-ModuleSpec "Lib" 1 @"
Public Function Pub() As Long
    Pub = 13
End Function
"@)
        )
    },
    [pscustomobject]@{
        id = "SCOPING-WRONG-PROJECT-QUALIFIER"
        purpose = "A nonexistent project qualifier must not be ignored."
        run = $null
        modules = @(
            (New-ModuleSpec "Main" 1 @"
Option Explicit

Public Function RunProbe() As Variant
    RunProbe = WrongProject.Lib.Pub()
End Function
"@),
            (New-ModuleSpec "Lib" 1 @"
Public Function Pub() As Long
    Pub = 17
End Function
"@)
        )
    },
    [pscustomobject]@{
        id = "SCOPING-FRIEND-STANDARD-MODULE"
        purpose = "Friend is rejected on standard-module procedures."
        run = $null
        modules = @(
            New-ModuleSpec "Main" 1 @"
Friend Sub Helper()
End Sub
"@
        )
    },
    [pscustomobject]@{
        id = "SCOPING-FRIEND-CLASS-MODULE"
        purpose = "Friend is accepted on class-module procedures and callable inside the same project."
        run = "Main.RunProbe"
        modules = @(
            (New-ModuleSpec "Main" 1 @"
Public Function RunProbe() As Variant
    Dim w As Widget
    Set w = New Widget
    RunProbe = w.FriendValue()
End Function
"@),
            (New-ModuleSpec "Widget" 2 @"
Friend Function FriendValue() As Long
    FriendValue = 19
End Function
"@)
        )
    }
)

$results = @()
foreach ($case in $cases) {
    Write-Host "Running $($case.id)"
    $results += Invoke-Case $case
}

$jsonPath = Join-Path $outDir "results.json"
$results | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 $jsonPath

$summaryPath = Join-Path $outDir "summary.md"
$capturedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# VM3 Scoping Visibility Excel Oracle")
$lines.Add("")
$lines.Add("- Run ID: $RunId")
$lines.Add("- Captured: $capturedAt")
$lines.Add("- Harness: $($MyInvocation.MyCommand.Path)")
$lines.Add("- Modal handling: VBE Debug -> Compile VBAProject (ID=578), UI Automation capture scoped to the owned Excel PID, then PID-scoped process cleanup.")
$lines.Add("")
$lines.Add("| Case | Compile | Dialog | Run | Value |")
$lines.Add("|---|---|---|---|---|")
foreach ($result in $results) {
    $dialogText = if ($result.dialogText) { ($result.dialogText -replace "`r?`n", " / ") } else { "" }
    $lines.Add("| $($result.id) | $($result.compileStatus) | $dialogText | $($result.runStatus) | $($result.runValue) |")
}
$lines.Add("")
$lines.Add("Raw JSON: results.json")
$lines | Set-Content -Encoding UTF8 $summaryPath

Write-Host "Wrote $summaryPath"
Write-Host "Wrote $jsonPath"
