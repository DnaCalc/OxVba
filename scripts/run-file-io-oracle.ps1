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
        throw "file I/O oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "file-io-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "file-io-oracle" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "file_io_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    $tempDir = Join-Path $workspaceRoot "temp/file-io-oracle-test"
    New-Item -ItemType Directory -Force -Path $tempDir | Out-Null
    $tempFile = Join-Path $tempDir "roundtrip.txt"
    $tempFileLiteral = $tempFile.Replace('\', '\\')
    $filePosPath = Join-Path $tempDir "filepos.txt"
    $filePosLiteral = $filePosPath.Replace('\', '\\')
    $writePath = Join-Path $tempDir "write_input.txt"
    $writeLiteral = $writePath.Replace('\', '\\')
    $writeMultiPath = Join-Path $tempDir "write_input_multi.txt"
    $writeMultiLiteral = $writeMultiPath.Replace('\', '\\')

    function Add-StdModule {
        param($Workbook, [string]$ModuleName, [string]$Code)
        $component = $Workbook.VBProject.VBComponents.Add(1)
        $component.Name = $ModuleName
        $null = $component.CodeModule.AddFromString($Code)
    }

    function Invoke-ExcelCase {
        param(
            [string]$CaseId,
            [string]$Scenario,
            [string]$Code
        )

        $wb = $null
        try {
            $wb = $script:excel.Workbooks.Add()
            Add-StdModule -Workbook $wb -ModuleName "OracleHarness" -Code $Code
            $result = [string]$script:excel.Run("RunProbe")
            [PSCustomObject]@{
                case_id = $CaseId
                scenario = $Scenario
                status = "ok"
                observed = $result
            }
        } catch {
            [PSCustomObject]@{
                case_id = $CaseId
                scenario = $Scenario
                status = "error"
                observed = $_.Exception.Message
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
            "test", "-p", "oxvba-host", "--test", "file_io_host_backed_end_to_end",
            $TestName,
            "--", "--exact", "--test-threads=1", "--nocapture"
        )
        $cmdText = "cargo " + ($cargoArgs -join " ")
        $cargoOutput = & cargo @cargoArgs 2>&1 | Tee-Object -FilePath $logPath
        $exitCode = $LASTEXITCODE
        $captured = ($cargoOutput | Out-String)
        $observed = ""
        if ($exitCode -eq 0) {
            $match = [regex]::Match(
                $captured,
                "ODG032-OBSERVED\[$([regex]::Escape($CaseId))\]=(?<value>[^\r\n]+)"
            )
            if ($match.Success) {
                $observed = $match.Groups["value"].Value
            } else {
                $exitCode = 1
                $observed = "missing-observation"
            }
        } else {
            $observed = "lane-failed(exit=$exitCode)"
        }

        [PSCustomObject]@{
            case_id = $CaseId
            scenario = $Scenario
            status = if ($exitCode -eq 0) { "ok" } else { "error" }
            observed = $observed
            notes = "OxVba anchor: file_io_host_backed_end_to_end::$TestName; command=$cmdText; log=$logPath"
        }
    }

    $script:excel = New-Object -ComObject Excel.Application
    $script:excel.Visible = $false
    $script:excel.DisplayAlerts = $false
    $excelVersion = $script:excel.Version
    try {
        $vbaRows = @(
            (
                Invoke-ExcelCase -CaseId "CCT-033-LINE-001" -Scenario "Output/Print/Close/Input/Line Input roundtrip returns written line" -Code @"
Public Function RunProbe()
    Dim a As String
    Open "$tempFileLiteral" For Output As #1
    Print #1, "world"
    Close #1
    Open "$tempFileLiteral" For Input As #2
    Line Input #2, a
    Close #2
    RunProbe = a
End Function
"@
            ),
            (
                Invoke-ExcelCase -CaseId "CCT-033-FILEPOS-001" -Scenario "EOF/LOF/Seek around Input file position follow Excel host semantics" -Code @"
Public Function RunProbe()
    Dim observed As String
    Dim line As String
    Open "$filePosLiteral" For Output As #1
    Print #1, "world"
    Close #1
    Open "$filePosLiteral" For Input As #1
    observed = CStr(EOF(1)) & "|" & CStr(LOF(1)) & "|" & CStr(Seek(1))
    Line Input #1, line
    observed = observed & "|" & line & "|" & CStr(EOF(1)) & "|" & CStr(Seek(1))
    Close #1
    RunProbe = observed
End Function
"@
            ),
            (
                Invoke-ExcelCase -CaseId "CCT-033-WRITE-001" -Scenario "Write#/Input# preserves embedded comma inside quoted string field" -Code @"
Public Function RunProbe()
    Dim a As String
    Open "$writeLiteral" For Output As #1
    Write #1, "hello,world"
    Close #1
    Open "$writeLiteral" For Input As #1
    Input #1, a
    Close #1
    RunProbe = a
End Function
"@
            ),
            (
                Invoke-ExcelCase -CaseId "CCT-033-WRITE-002" -Scenario "Write#/Input# multi-field roundtrip preserves typed field shapes" -Code @"
Public Function RunProbe()
    Dim a
    Dim b
    Dim c
    Open "$writeMultiLiteral" For Output As #1
    Write #1, 42, True, "hello,world"
    Close #1
    Open "$writeMultiLiteral" For Input As #1
    Input #1, a, b, c
    Close #1
    RunProbe = CStr(a) & "|" & CStr(b) & "|" & CStr(c)
End Function
"@
            )
        )
    } finally {
        $script:excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($script:excel)
    }

    $oxRows = @(
        (
            Invoke-OxCase -CaseId "CCT-033-LINE-001" -Scenario "Output/Print/Close/Input/Line Input roundtrip returns written line" -TestName "windows_file_io_host_backed_end_to_end::host_backed_file_print_line_input_roundtrip_returns_written_line"
        ),
        (
            Invoke-OxCase -CaseId "CCT-033-FILEPOS-001" -Scenario "EOF/LOF/Seek around Input file position follow Excel host semantics" -TestName "windows_file_io_host_backed_end_to_end::host_backed_file_eof_lof_seek_matches_excel_shape"
        ),
        (
            Invoke-OxCase -CaseId "CCT-033-WRITE-001" -Scenario "Write#/Input# preserves embedded comma inside quoted string field" -TestName "windows_file_io_host_backed_end_to_end::host_backed_file_write_input_preserves_embedded_comma_string"
        ),
        (
            Invoke-OxCase -CaseId "CCT-033-WRITE-002" -Scenario "Write#/Input# multi-field roundtrip preserves typed field shapes" -TestName "windows_file_io_host_backed_end_to_end::host_backed_file_write_input_multi_field_typed_roundtrip_matches_excel_shape"
        )
    )

    $rows = foreach ($oxRow in $oxRows) {
        $vbaRow = $vbaRows | Where-Object { $_.case_id -eq $oxRow.case_id } | Select-Object -First 1
        if ($null -eq $vbaRow) {
            throw "missing VBA row for case $($oxRow.case_id)"
        }
        [PSCustomObject]@{
            topic_id = "CCT-033"
            case_id = $oxRow.case_id
            scenario = $oxRow.scenario
            vba_status = $vbaRow.status
            vba_observed = $vbaRow.observed
            oxvba_status = $oxRow.status
            oxvba_observed = $oxRow.observed
            match = if ($vbaRow.status -eq $oxRow.status -and $vbaRow.observed -eq $oxRow.observed) { "true" } else { "false" }
            notes = $oxRow.notes
        }
    }

    $csvPath = Join-Path $runDir "results.csv"
    $summaryPath = Join-Path $runDir "summary.md"
    $rows | Export-Csv -Path $csvPath -NoTypeInformation

    $summary = @(
        "# File I/O Oracle Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Excel version: $excelVersion",
        "- File probe path: $tempFile",
        "- Results CSV: $csvPath",
        "- Total cases: $($rows.Count)",
        "- Match count: $((@($rows | Where-Object { $_.match -eq 'true' })).Count)",
        "- Mismatch count: $((@($rows | Where-Object { $_.match -ne 'true' })).Count)",
        "",
        "## Case Results",
        "| Topic | Case | VBA | OxVba | Match | Notes |",
        "|---|---|---|---|---|---|"
    )
    foreach ($row in $rows) {
        $vbaCell = "$($row.vba_status): $($row.vba_observed)"
        $oxCell = "$($row.oxvba_status): $($row.oxvba_observed)"
        $summary += "| $($row.topic_id) | $($row.case_id) | $vbaCell | $oxCell | $($row.match) | $($row.notes) |"
    }
    Set-Content -Path $summaryPath -Value ($summary -join [Environment]::NewLine)

    Write-Host "file-io-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "results=$csvPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
