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
        throw "host extension oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "host-extension-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "host-extension-oracle" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "host_extension_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    function Get-QuotedLiteral {
        param([string]$Value)
        "'" + $Value.Replace("'", "''") + "'"
    }

    function Invoke-ExcelDirectProbe {
        param(
            [string]$WorkbookPath,
            [string]$ObservedPath,
            [string]$ModuleName,
            [string]$Source,
            [string]$SeedSource = ""
        )

        $quotedWorkbookPath = Get-QuotedLiteral $WorkbookPath
        $quotedObservedPath = Get-QuotedLiteral $ObservedPath
        $quotedModuleName = Get-QuotedLiteral $ModuleName
        $quotedSource = Get-QuotedLiteral $Source
        $quotedSeedSource = Get-QuotedLiteral $SeedSource
        $script = @'
$ErrorActionPreference = "Stop"
$excel = New-Object -ComObject Excel.Application
$excel.Visible = $false
$excel.DisplayAlerts = $false
$wb = $null
$component = $null
$codeModule = $null
$status = "ok"
$payload = ""
try {
    $wb = $excel.Workbooks.Add()
    $wb.VBProject.Name = "Workbook"
    if (__SEED_SOURCE__.Length -gt 0) {
        $component = $wb.VBProject.VBComponents.Item(__MODULE_NAME__)
        $codeModule = $component.CodeModule
        if ($codeModule.CountOfLines -gt 0) {
            $codeModule.DeleteLines(1, $codeModule.CountOfLines)
        }
        [void]$codeModule.AddFromString(__SEED_SOURCE__)
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($codeModule)
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($component)
        $codeModule = $null
        $component = $null
    }
    $wb.SaveAs(__WORKBOOK_PATH__, 52)

    try {
        $component = $wb.VBProject.VBComponents.Item(__MODULE_NAME__)
        $codeModule = $component.CodeModule
        if ($codeModule.CountOfLines -gt 0) {
            $codeModule.DeleteLines(1, $codeModule.CountOfLines)
        }
        [void]$codeModule.AddFromString(__SOURCE__)
        $wb.Save()
        if ($codeModule.CountOfLines -gt 0) {
            $payload = $codeModule.Lines(1, $codeModule.CountOfLines)
        }
    } catch {
        $status = "error"
        $payload = $_.Exception.Message
    }
    Set-Content -Path __OBSERVED_PATH__ -Value (@{ status = $status; payload = $payload } | ConvertTo-Json -Compress)
}
finally {
    if ($codeModule -ne $null) {
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($codeModule)
    }
    if ($component -ne $null) {
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($component)
    }
    if ($wb -ne $null) {
        $wb.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
    }
    $excel.Quit()
    [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
}
'@
        $script = $script.Replace("__SOURCE__", $quotedSource)
        $script = $script.Replace("__SEED_SOURCE__", $quotedSeedSource)
        $script = $script.Replace("__MODULE_NAME__", $quotedModuleName)
        $script = $script.Replace("__WORKBOOK_PATH__", $quotedWorkbookPath)
        $script = $script.Replace("__OBSERVED_PATH__", $quotedObservedPath)
        & pwsh -NoLogo -NoProfile -ExecutionPolicy Bypass -Command $script
        if ($LASTEXITCODE -ne 0) {
            throw "Excel direct probe failed with exit code $LASTEXITCODE"
        }
    }

    function Invoke-OxVbaProbe {
        param(
            [string]$TestName,
            [string]$ObservedPath,
            [string]$LogPath
        )

        $hadCaptureEnv = Test-Path Env:OXVBA_HOST_EXTENSION_CAPTURE_PATH
        $previousCaptureEnv = if ($hadCaptureEnv) {
            (Get-Item Env:OXVBA_HOST_EXTENSION_CAPTURE_PATH).Value
        } else {
            $null
        }

        try {
            Set-Item Env:OXVBA_HOST_EXTENSION_CAPTURE_PATH $ObservedPath
            $cargoArgs = @(
                "test", "-p", "oxvba-host",
                "--test", "host_project_excel_vbide_callbacks",
                $TestName,
                "--", "--ignored", "--exact", "--test-threads=1", "--nocapture"
            )
            & cargo @cargoArgs *> $LogPath
            if ($LASTEXITCODE -ne 0) {
                throw "OxVba host extension probe failed with exit code $LASTEXITCODE"
            }
        } finally {
            if ($hadCaptureEnv) {
                Set-Item Env:OXVBA_HOST_EXTENSION_CAPTURE_PATH $previousCaptureEnv
            } else {
                Remove-Item Env:OXVBA_HOST_EXTENSION_CAPTURE_PATH -ErrorAction SilentlyContinue
            }
        }
    }

    function Read-CaptureJson {
        param([string]$Path)
        if (-not (Test-Path $Path)) {
            return [PSCustomObject]@{
                status = "error"
                payload = "missing-capture"
            }
        }
        return (Get-Content -Path $Path -Raw | ConvertFrom-Json)
    }

    $cases = @(
        [PSCustomObject]@{
            case_id = "CCT-042-THISWORKBOOK-001"
            scenario = "ThisWorkbook extension-source attachment preserves the injected procedure body"
            module_name = "ThisWorkbook"
            source = "Public Sub Sync()`nEnd Sub"
            seed_source = ""
            test_name = "excel_vbide_host_callbacks_attach_source_to_thisworkbook"
            notes = "bounded supported host-extension attach on ThisWorkbook"
        },
        [PSCustomObject]@{
            case_id = "CCT-042-MISSING-TARGET-001"
            scenario = "Missing host extension targets fail instead of creating a new arbitrary host module"
            module_name = "MissingHostTarget"
            source = "Public Sub Sync()`nEnd Sub"
            seed_source = ""
            test_name = "excel_vbide_host_callbacks_missing_target_reports_error"
            notes = "error parity is status-normalized because Excel/VBIDE error strings are host-specific"
        },
        [PSCustomObject]@{
            case_id = "CCT-042-THISWORKBOOK-OVERWRITE-001"
            scenario = "Occupied ThisWorkbook target is overwritten by the new host extension source"
            module_name = "ThisWorkbook"
            source = "Public Sub AfterSync()`nEnd Sub"
            seed_source = "Public Sub BeforeSync()`nEnd Sub"
            test_name = "excel_vbide_host_callbacks_replace_existing_thisworkbook_source"
            notes = "bounded overwrite-on-occupied-target behavior"
        }
    )

    $rows = foreach ($case in $cases) {
        $caseSlug = $case.case_id.ToLowerInvariant().Replace("-", "_")
        $excelWorkbook = Join-Path $runDir "$caseSlug.excel_probe.xlsm"
        $excelObservedPath = Join-Path $runDir "$caseSlug.excel_observed.json"
        $oxObservedPath = Join-Path $runDir "$caseSlug.oxvba_observed.json"
        $oxLogPath = Join-Path $runDir "$caseSlug.oxvba_test.log.txt"

        Invoke-ExcelDirectProbe `
            -WorkbookPath $excelWorkbook `
            -ObservedPath $excelObservedPath `
            -ModuleName $case.module_name `
            -Source $case.source `
            -SeedSource $case.seed_source
        Invoke-OxVbaProbe `
            -TestName $case.test_name `
            -ObservedPath $oxObservedPath `
            -LogPath $oxLogPath

        $excelObserved = Read-CaptureJson -Path $excelObservedPath
        $oxObserved = Read-CaptureJson -Path $oxObservedPath
        $isMatch = if ($excelObserved.status -ne $oxObserved.status) {
            $false
        } elseif ($excelObserved.status -eq "ok") {
            $excelObserved.payload -eq $oxObserved.payload
        } else {
            $true
        }

        [PSCustomObject]@{
            topic_id = "CCT-042"
            case_id = $case.case_id
            scenario = $case.scenario
            vba_status = $excelObserved.status
            vba_observed = $excelObserved.payload
            oxvba_status = $oxObserved.status
            oxvba_observed = $oxObserved.payload
            match = if ($isMatch) { "true" } else { "false" }
            notes = "OxVba anchor: host_project_excel_vbide_callbacks::$($case.test_name); log=$oxLogPath; $($case.notes)"
        }
    }

    $csvPath = Join-Path $runDir "results.csv"
    $summaryPath = Join-Path $runDir "summary.md"
    $rows | Export-Csv -Path $csvPath -NoTypeInformation

    $summary = @(
        "# Host Extension Oracle Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Results CSV: $csvPath",
        "- Output directory: $runDir",
        "",
        "## Case Results",
        "| Topic | Case | VBA | OxVba | Match | Notes |",
        "|---|---|---|---|---|---|"
    )
    foreach ($row in $rows) {
        $summary += "| $($row.topic_id) | $($row.case_id) | $($row.vba_status): $($row.vba_observed) | $($row.oxvba_status): $($row.oxvba_observed) | $($row.match) | $($row.notes) |"
    }
    Set-Content -Path $summaryPath -Value ($summary -join [Environment]::NewLine)

    Write-Host "host-extension-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "results=$csvPath"
    Write-Host "summary=$summaryPath"
}
finally {
    Pop-Location
}
