param(
    [string]$OutputRoot = "docs/evidence/conformance/oracle_captures",
    [string]$RunId = "",
    [switch]$NoArtifacts
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not $IsWindows) {
        throw "VBA attribute oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "vba-attribute-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "vba-attribute-oracle" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "vba_attribute_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    function Get-OracleCaseDir {
        param([string]$CaseId)

        $caseDir = Join-Path $runDir ("excel_import_" + ($CaseId -replace '[^A-Za-z0-9_.-]', '_'))
        New-Item -ItemType Directory -Force -Path $caseDir | Out-Null
        $caseDir
    }

    function Import-VbComponentFile {
        param(
            $Workbook,
            [string]$CaseId,
            [string]$FileName,
            [string]$Code
        )

        $caseDir = Get-OracleCaseDir -CaseId $CaseId
        $filePath = Join-Path $caseDir $FileName
        $normalizedCode = (($Code -replace "`r?`n", "`r`n").TrimEnd("`r", "`n")) + "`r`n"
        [System.IO.File]::WriteAllText($filePath, $normalizedCode, [System.Text.Encoding]::ASCII)
        $Workbook.VBProject.VBComponents.Import($filePath)
    }

    function Capture-ImportedComponentRoundtrip {
        param(
            $Workbook,
            [string]$CaseId,
            [string]$ComponentName,
            [string[]]$ExpectedMemberAttributePrefixes
        )

        $caseDir = Get-OracleCaseDir -CaseId $CaseId
        $roundtripPath = Join-Path $caseDir ($ComponentName + "_roundtrip.cls")
        $component = $Workbook.VBProject.VBComponents.Item($ComponentName)
        $component.Export($roundtripPath)
        $exportedLines = @(Get-Content $roundtripPath)
        $exportedMemberAttrs = @(
            foreach ($line in $exportedLines) {
                foreach ($prefix in $ExpectedMemberAttributePrefixes) {
                    if ($line.StartsWith($prefix, [System.StringComparison]::Ordinal)) {
                        $line
                        break
                    }
                }
            }
        )
        $status = if ($exportedMemberAttrs.Count -gt 0) { "retained" } else { "dropped" }
        $summaryPath = Join-Path $caseDir "roundtrip_summary.md"
        $summaryLines = @(
            "# Import Roundtrip Summary",
            "",
            "- Case ID: $CaseId",
            "- Component: $ComponentName",
            "- Roundtrip status: $status",
            "- Export path: $roundtripPath",
            "- Expected member attribute prefixes: $($ExpectedMemberAttributePrefixes -join ', ')",
            "- Exported member attributes: $(if ($exportedMemberAttrs.Count -gt 0) { $exportedMemberAttrs -join '; ' } else { '<none>' })"
        )
        Set-Content -Path $summaryPath -Value ($summaryLines -join [Environment]::NewLine)

        [PSCustomObject]@{
            status = $status
            summary_path = $summaryPath
            export_path = $roundtripPath
            exported_member_attrs = if ($exportedMemberAttrs.Count -gt 0) {
                $exportedMemberAttrs -join '; '
            } else {
                '<none>'
            }
        }
    }

    function Invoke-ExcelCase {
        param(
            [string]$CaseId,
            [string]$Scenario,
            [string]$Kind
        )

        $wb = $null
        try {
            $wb = $script:excel.Workbooks.Add()
            $roundtrip = $null
            if ($Kind -eq "defaultprop") {
                $null = Import-VbComponentFile -Workbook $wb -CaseId $CaseId -FileName "Widget.cls" -Code @"
VERSION 1.0 CLASS
BEGIN
  MultiUse = -1  'True
END
Attribute VB_Name = "Widget"
Attribute VB_GlobalNameSpace = False
Attribute VB_Creatable = False
Attribute VB_PredeclaredId = False
Attribute VB_Exposed = False
Option Explicit
Private stored As Long

Public Sub Class_Initialize()
    stored = 41
End Sub

Public Property Get Value() As Long
    Value = stored + 1
End Property
Attribute Value.VB_UserMemId = 0
"@
                $roundtrip = Capture-ImportedComponentRoundtrip `
                    -Workbook $wb `
                    -CaseId $CaseId `
                    -ComponentName "Widget" `
                    -ExpectedMemberAttributePrefixes @("Attribute Value.VB_UserMemId")
                $null = Import-VbComponentFile -Workbook $wb -CaseId $CaseId -FileName "OracleHarness.bas" -Code @"
Attribute VB_Name = "OracleHarness"
Public Function RunProbe()
    On Error GoTo handler
    Dim widget As New Widget
    Dim valueOut
    valueOut = widget
    RunProbe = CStr(valueOut)
    Exit Function
handler:
    RunProbe = "ERR|" & CStr(Err.Number) & "|" & Err.Description
End Function
"@
            } elseif ($Kind -eq "newenum") {
                $null = Import-VbComponentFile -Workbook $wb -CaseId $CaseId -FileName "Widget.cls" -Code @"
VERSION 1.0 CLASS
BEGIN
  MultiUse = -1  'True
END
Attribute VB_Name = "Widget"
Attribute VB_GlobalNameSpace = False
Attribute VB_Creatable = False
Attribute VB_PredeclaredId = False
Attribute VB_Exposed = False
Option Explicit
Private items As New Collection

Public Sub Class_Initialize()
    items.Add 41
    items.Add 42
End Sub

Public Property Get NewEnum() As IUnknown
    Set NewEnum = items.[_NewEnum]
End Property
Attribute NewEnum.VB_UserMemId = -4
Attribute NewEnum.VB_MemberFlags = "40"
"@
                $roundtrip = Capture-ImportedComponentRoundtrip `
                    -Workbook $wb `
                    -CaseId $CaseId `
                    -ComponentName "Widget" `
                    -ExpectedMemberAttributePrefixes @(
                        "Attribute NewEnum.VB_UserMemId",
                        "Attribute NewEnum.VB_MemberFlags"
                    )
                $null = Import-VbComponentFile -Workbook $wb -CaseId $CaseId -FileName "OracleHarness.bas" -Code @"
Attribute VB_Name = "OracleHarness"
Public Function RunProbe()
    On Error GoTo handler
    Dim widget As New Widget
    Dim item
    Dim acc
    For Each item In widget
        acc = acc & CStr(item) & ","
    Next item
    RunProbe = acc
    Exit Function
handler:
    RunProbe = "ERR|" & CStr(Err.Number) & "|" & Err.Description
End Function
"@
            } else {
                throw "unsupported Excel oracle kind: $Kind"
            }
            $qualifiedMacro = "'" + $wb.Name + "'!OracleHarness.RunProbe"
            $result = [string]$script:excel.Run($qualifiedMacro)
            [PSCustomObject]@{
                case_id = $CaseId
                scenario = $Scenario
                status = "ok"
                observed = $result
                roundtrip_status = if ($null -ne $roundtrip) { $roundtrip.status } else { "n/a" }
                roundtrip_summary = if ($null -ne $roundtrip) { $roundtrip.summary_path } else { "" }
                roundtrip_export = if ($null -ne $roundtrip) { $roundtrip.export_path } else { "" }
                roundtrip_member_attrs = if ($null -ne $roundtrip) { $roundtrip.exported_member_attrs } else { "" }
            }
        } catch {
            [PSCustomObject]@{
                case_id = $CaseId
                scenario = $Scenario
                status = "error"
                observed = $_.Exception.Message
                roundtrip_status = if ($null -ne $roundtrip) { $roundtrip.status } else { "error" }
                roundtrip_summary = if ($null -ne $roundtrip) { $roundtrip.summary_path } else { "" }
                roundtrip_export = if ($null -ne $roundtrip) { $roundtrip.export_path } else { "" }
                roundtrip_member_attrs = if ($null -ne $roundtrip) { $roundtrip.exported_member_attrs } else { "" }
            }
        } finally {
            if ($wb -ne $null) {
                $wb.Close($false)
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
            }
        }
    }

    function Invoke-OxCase {
        param(
            [string]$CaseId,
            [string]$Scenario,
            [string]$TestName
        )

        $logPath = Join-Path $runDir ("{0}.log.txt" -f ($CaseId -replace '[^A-Za-z0-9_.-]', '_'))
        $cargoArgs = @(
            "test", "-p", "oxvba-host", "--test", "vba_attribute_oracle_lane",
            $TestName,
            "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
        )
        $cmdText = "cargo " + ($cargoArgs -join " ")
        $cargoOutput = & cargo @cargoArgs 2>&1 | Tee-Object -FilePath $logPath
        $exitCode = $LASTEXITCODE
        $captured = ($cargoOutput | Out-String)
        $observed = ""
        $match = [regex]::Match(
            $captured,
            "ODGATTR-OBSERVED\[$([regex]::Escape($CaseId))\]=(?<value>[^\r\n]+)"
        )
        if ($match.Success) {
            $observed = $match.Groups["value"].Value
        } elseif ($exitCode -eq 0) {
            $exitCode = 1
            $observed = "missing-observation"
        } else {
            $observed = "lane-failed(exit=$exitCode)"
        }
        if ($exitCode -eq 0 -and [string]::IsNullOrWhiteSpace($observed)) {
                $exitCode = 1
                $observed = "missing-observation"
        }

        [PSCustomObject]@{
            case_id = $CaseId
            scenario = $Scenario
            status = if ($exitCode -eq 0) { "ok" } else { "error" }
            observed = $observed
            notes = "OxVba anchor: vba_attribute_oracle_lane::$TestName; command=$cmdText; log=$logPath"
        }
    }

    $script:excel = New-Object -ComObject Excel.Application
    $script:excel.Visible = $false
    $script:excel.DisplayAlerts = $false
    $excelVersion = $script:excel.Version
    try {
        $vbaRows = @(
            (Invoke-ExcelCase -CaseId "CCT-049-DEFAULTPROP-001" -Scenario "Attribute-backed default property (`VB_UserMemId = 0`) resolves through bare object assignment" -Kind "defaultprop")
            (Invoke-ExcelCase -CaseId "CCT-050-NEWENUM-001" -Scenario "Attribute-backed enumerator (`VB_UserMemId = -4`) drives `For Each` over a class instance" -Kind "newenum")
        )
    } finally {
        $script:excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($script:excel)
    }

    $oxRows = @(
        (Invoke-OxCase -CaseId "CCT-049-DEFAULTPROP-001" -Scenario "Attribute-backed default property (`VB_UserMemId = 0`) resolves through bare object assignment" -TestName "windows_vba_attribute_oracle_lane::windows_defaultprop_vb_usermemid_zero_bare_assignment_matches_excel")
        (Invoke-OxCase -CaseId "CCT-050-NEWENUM-001" -Scenario "Attribute-backed enumerator (`VB_UserMemId = -4`) drives `For Each` over a class instance" -TestName "windows_vba_attribute_oracle_lane::windows_newenum_vb_usermemid_minus4_for_each_matches_excel")
    )

    $rows = foreach ($oxRow in $oxRows) {
        $vbaRow = $vbaRows | Where-Object { $_.case_id -eq $oxRow.case_id } | Select-Object -First 1
        if ($null -eq $vbaRow) {
            throw "missing VBA row for case $($oxRow.case_id)"
        }
        [PSCustomObject]@{
            topic_id = if ($oxRow.case_id -like 'CCT-049-*') { "CCT-049" } else { "CCT-050" }
            case_id = $oxRow.case_id
            scenario = $oxRow.scenario
            vba_status = $vbaRow.status
            vba_observed = $vbaRow.observed
            oxvba_status = $oxRow.status
            oxvba_observed = $oxRow.observed
            excel_roundtrip_status = $vbaRow.roundtrip_status
            excel_roundtrip_summary = $vbaRow.roundtrip_summary
            excel_roundtrip_member_attrs = $vbaRow.roundtrip_member_attrs
            match = if ($vbaRow.status -eq $oxRow.status -and $vbaRow.observed -eq $oxRow.observed) { "true" } else { "false" }
            notes = "$($oxRow.notes); Excel import/export roundtrip: $($vbaRow.roundtrip_status); summary=$($vbaRow.roundtrip_summary)"
        }
    }

    $csvPath = Join-Path $runDir "results.csv"
    $summaryPath = Join-Path $runDir "summary.md"
    $rows | Export-Csv -Path $csvPath -NoTypeInformation

    $summary = @(
        "# VBA Attribute Oracle Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Excel version: $excelVersion",
        "- Results CSV: $csvPath",
        "- Total cases: $($rows.Count)",
        "- Match count: $((@($rows | Where-Object { $_.match -eq 'true' })).Count)",
        "- Mismatch count: $((@($rows | Where-Object { $_.match -ne 'true' })).Count)",
        "",
        "## Case Results",
        "| Topic | Case | VBA | OxVba | Roundtrip | Match | Notes |",
        "|---|---|---|---|---|---|---|"
    )
    foreach ($row in $rows) {
        $vbaCell = "$($row.vba_status): $($row.vba_observed)"
        $oxCell = "$($row.oxvba_status): $($row.oxvba_observed)"
        $roundtripCell = "$($row.excel_roundtrip_status): $($row.excel_roundtrip_member_attrs)"
        $summary += "| $($row.topic_id) | $($row.case_id) | $vbaCell | $oxCell | $roundtripCell | $($row.match) | $($row.notes) |"
    }
    Set-Content -Path $summaryPath -Value ($summary -join [Environment]::NewLine)

    Write-Host "vba-attribute-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "results=$csvPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
