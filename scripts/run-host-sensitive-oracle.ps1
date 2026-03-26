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
        throw "host-sensitive oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "host-sensitive-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "host-sensitive-oracle" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "host_sensitive_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    $envName = "OXVBA_ORACLE_ENV"
    $envValue = "oracle-033-value"
    $tempDir = Join-Path $workspaceRoot "temp/odg033-oracle-test"
    New-Item -ItemType Directory -Force -Path $tempDir | Out-Null
    $tempFile = Join-Path $tempDir "probe-file.txt"
    Set-Content -Path $tempFile -Value "probe"
    $tempFileLiteral = $tempFile.Replace('\', '\\')

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
        $hadPreviousEnv = Test-Path "Env:$envName"
        $previousEnvValue = if ($hadPreviousEnv) {
            (Get-Item "Env:$envName").Value
        } else {
            $null
        }
        try {
            Set-Item -Path "Env:$envName" -Value $envValue
            $cargoArgs = @(
                "test", "-p", "oxvba-host", "--test", "host_sensitive_oracle_lane",
                $TestName,
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            )
            $cmdText = "cargo " + ($cargoArgs -join " ")
            $cargoOutput = & cargo @cargoArgs 2>&1 | Tee-Object -FilePath $logPath
            $exitCode = $LASTEXITCODE
            $captured = ($cargoOutput | Out-String)
            $observed = ""
            if ($exitCode -eq 0) {
                $match = [regex]::Match(
                    $captured,
                    "ODG033-OBSERVED\[$([regex]::Escape($CaseId))\]=(?<value>[^\r\n]+)"
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
                notes = "OxVba anchor: host_sensitive_oracle_lane::$TestName; command=$cmdText; log=$logPath"
            }
        } finally {
            if ($hadPreviousEnv) {
                Set-Item -Path "Env:$envName" -Value $previousEnvValue
            } else {
                Remove-Item "Env:$envName" -ErrorAction SilentlyContinue
            }
        }
    }

    $hadPreviousExcelEnv = Test-Path "Env:$envName"
    $previousExcelEnv = if ($hadPreviousExcelEnv) {
        (Get-Item "Env:$envName").Value
    } else {
        $null
    }
    try {
        Set-Item -Path "Env:$envName" -Value $envValue
        $script:excel = New-Object -ComObject Excel.Application
        $script:excel.Visible = $false
        $script:excel.DisplayAlerts = $false
        $excelVersion = $script:excel.Version
        try {
            $vbaRows = @(
                Invoke-ExcelCase -CaseId "CCT-035-ENV-001" -Scenario "Host-backed Environ(name) returns the environment string value" -Code @"
Public Function RunProbe()
    RunProbe = CStr(Environ("$envName"))
End Function
"@
                Invoke-ExcelCase -CaseId "CCT-035-DIR-001" -Scenario "Host-backed Dir(path) returns the matching file name" -Code @"
Public Function RunProbe()
    RunProbe = CStr(Dir("$tempFileLiteral"))
End Function
"@
                Invoke-ExcelCase -CaseId "CCT-035-SHELL-001" -Scenario "Host-backed Shell(command) returns a positive task identifier" -Code @"
Public Function RunProbe()
    Dim taskId As Variant
    taskId = Shell("cmd.exe /c exit 0")
    If CLng(taskId) > 0 Then
        RunProbe = "pid>0"
    Else
        RunProbe = "pid<=0"
    End If
End Function
"@
            )
        } finally {
            $script:excel.Quit()
            [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($script:excel)
        }
    } finally {
        if ($hadPreviousExcelEnv) {
            Set-Item -Path "Env:$envName" -Value $previousExcelEnv
        } else {
            Remove-Item "Env:$envName" -ErrorAction SilentlyContinue
        }
    }

    $oxRows = @(
        Invoke-OxCase -CaseId "CCT-035-ENV-001" -Scenario "Host-backed Environ(name) returns the environment string value" -TestName "windows_host_sensitive_oracle_lane::windows_host_backed_environ_string_returns_actual_value"
        Invoke-OxCase -CaseId "CCT-035-DIR-001" -Scenario "Host-backed Dir(path) returns the matching file name" -TestName "windows_host_sensitive_oracle_lane::windows_host_backed_dir_existing_file_returns_filename"
        Invoke-OxCase -CaseId "CCT-035-SHELL-001" -Scenario "Host-backed Shell(command) returns a positive task identifier" -TestName "windows_host_sensitive_oracle_lane::windows_host_backed_shell_returns_positive_process_identifier"
    )

    $rows = foreach ($oxRow in $oxRows) {
        $vbaRow = $vbaRows | Where-Object { $_.case_id -eq $oxRow.case_id } | Select-Object -First 1
        if ($null -eq $vbaRow) {
            throw "missing VBA row for case $($oxRow.case_id)"
        }
        [PSCustomObject]@{
            topic_id = "CCT-035"
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
        "# Host-Sensitive Oracle Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Excel version: $excelVersion",
        "- Environment variable: $envName=$envValue",
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

    Write-Host "host-sensitive-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "results=$csvPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
