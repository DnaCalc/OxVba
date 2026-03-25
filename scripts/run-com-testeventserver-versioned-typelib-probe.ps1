param(
    [string]$OutputRoot = "docs/evidence/conformance/oracle_captures",
    [string]$RunId = "",
    [switch]$NoArtifacts
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not $IsWindows) {
        throw "COM TestEventServer versioned typelib probe is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-testeventserver-versioned-typelib-probe" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "com-testeventserver-versioned-typelib-probe" -RunId $resolvedRunId
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "com_testeventserver_versioned_typelib_probe_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null
    $variantRoot = Join-Path $workspaceRoot "temp\generated\com_testeventserver_versioned_typelib_probe\$resolvedRunId\variant_v2"

    function Add-Row {
        param(
            [System.Collections.Generic.List[object]]$Rows,
            [string]$CaseId,
            [string]$Scenario,
            [string]$Status,
            [string]$Observed
        )

        $Rows.Add([PSCustomObject]@{
                case_id  = $CaseId
                scenario = $Scenario
                status   = $Status
                observed = $Observed
            }) | Out-Null
    }

    function New-VersionedProjectCopy {
        param(
            [string]$SourceRoot,
            [string]$DestinationRoot,
            [string]$AssemblyVersion,
            [int]$TypeLibMajor,
            [int]$TypeLibMinor
        )

        if (Test-Path $DestinationRoot) {
            Remove-Item -Recurse -Force -Path $DestinationRoot
        }
        Copy-Item -Recurse -Force -Path $SourceRoot -Destination $DestinationRoot
        foreach ($buildDir in @("bin", "obj")) {
            $candidate = Join-Path $DestinationRoot $buildDir
            if (Test-Path $candidate) {
                Remove-Item -Recurse -Force -Path $candidate
            }
        }

        $csprojPath = Join-Path $DestinationRoot "OxVba.TestEventServer.csproj"
        $assemblyInfoPath = Join-Path $DestinationRoot "Properties/AssemblyInfo.cs"

        $csproj = Get-Content $csprojPath -Raw
        if ($csproj -notmatch "<AssemblyVersion>") {
            $injected = @"
    <AssemblyVersion>$AssemblyVersion</AssemblyVersion>
    <FileVersion>$AssemblyVersion</FileVersion>
"@
            $csproj = $csproj -replace "(?ms)(\s*<LangVersion>7\.3</LangVersion>)", "`$1`r`n$injected"
            Set-Content -Path $csprojPath -Value $csproj -Encoding UTF8
        }

        $assemblyInfo = Get-Content $assemblyInfoPath -Raw
        if ($assemblyInfo -notmatch "TypeLibVersion") {
            $assemblyInfo = $assemblyInfo.TrimEnd() + "`r`n[assembly: TypeLibVersion($TypeLibMajor, $TypeLibMinor)]`r`n"
            Set-Content -Path $assemblyInfoPath -Value $assemblyInfo -Encoding UTF8
        }
    }

    function Invoke-WorkbookProbe {
        param(
            [object]$Excel,
            [string]$TypeLibPath,
            [scriptblock]$Populate,
            [string]$ProcedureName = "RunProbe"
        )

        $wb = $null
        try {
            $wb = $Excel.Workbooks.Add()
            [void]$wb.VBProject.References.AddFromFile($TypeLibPath)
            & $Populate $wb
            $reference = $wb.VBProject.References |
                Where-Object { $_.Guid -eq "{E2A30001-0001-0001-0001-000000000001}" } |
                Select-Object -First 1
            $version = if ($null -ne $reference) { "$($reference.Major).$($reference.Minor)" } else { "<missing>" }
            $name = if ($null -ne $reference) { [string]$reference.Name } else { "<missing>" }
            $result = if ([string]::IsNullOrWhiteSpace($ProcedureName)) {
                ""
            } else {
                [string]$Excel.Run($ProcedureName)
            }
            return @{
                status   = "ok"
                observed = "name=$name;version=$version;result=$result"
            }
        } catch {
            return @{
                status   = "error"
                observed = $_.Exception.Message
            }
        } finally {
            if ($wb -ne $null) {
                $wb.Close($false)
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
            }
        }
    }

    function New-VersionedWorkbookFixture {
        param(
            [object]$Excel,
            [string]$TypeLibPath,
            [string]$WorkbookPath
        )

        $wb = $null
        try {
            $wb = $Excel.Workbooks.Add()
            [void]$wb.VBProject.References.AddFromFile($TypeLibPath)
            $mod = $wb.VBProject.VBComponents.Add(1)
            $mod.Name = "MainModule"
            $code = @"
Public Function RunProbe()
    Dim obj As TestEventServer
    Set obj = New TestEventServer
    RunProbe = obj.Ping()
End Function
"@
            [void]$mod.CodeModule.AddFromString($code)
            $wb.SaveAs($WorkbookPath, 52)
        } finally {
            if ($wb -ne $null) {
                $wb.Close($false)
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
            }
        }
    }

    function Invoke-ReopenProbe {
        param(
            [object]$Excel,
            [string]$WorkbookPath
        )

        $wb = $null
        try {
            $wb = $Excel.Workbooks.Open($WorkbookPath)
            $reference = $wb.VBProject.References |
                Where-Object { $_.Guid -eq "{E2A30001-0001-0001-0001-000000000001}" } |
                Select-Object -First 1
            if ($null -eq $reference) {
                return @{
                    status   = "ok"
                    observed = "reference=<missing>"
                }
            }
            if ($reference.IsBroken) {
                return @{
                    status   = "ok"
                    observed = "name=$($reference.Name);version=$($reference.Major).$($reference.Minor);broken=True"
                }
            }
            $result = [string]$Excel.Run("RunProbe")
            return @{
                status   = "ok"
                observed = "name=$($reference.Name);version=$($reference.Major).$($reference.Minor);broken=$($reference.IsBroken);result=$result"
            }
        } catch {
            return @{
                status   = "error"
                observed = $_.Exception.Message
            }
        } finally {
            if ($wb -ne $null) {
                $wb.Close($false)
                [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
            }
        }
    }

    & (Join-Path $workspaceRoot "tools/OxVba.TestEventServer/register.ps1") -Configuration Debug -Scope CurrentUser
    $v1TypeLibPath = (Resolve-Path "tools/OxVba.TestEventServer/bin/Debug/net48/OxVba.TestEventServer.tlb").Path

    New-VersionedProjectCopy `
        -SourceRoot (Join-Path $workspaceRoot "tools/OxVba.TestEventServer") `
        -DestinationRoot $variantRoot `
        -AssemblyVersion "2.0.0.0" `
        -TypeLibMajor 2 `
        -TypeLibMinor 0

    & (Join-Path $variantRoot "register.ps1") -Configuration Debug -ExportTypeLibOnly
    $builtV2TypeLibPath = (Resolve-Path (Join-Path $variantRoot "bin/Debug/net48/OxVba.TestEventServer.tlb")).Path
    $v2TypeLibPath = Join-Path $runDir "OxVba.TestEventServer.v2.tlb"
    Copy-Item -Force $builtV2TypeLibPath $v2TypeLibPath

    $rows = New-Object System.Collections.Generic.List[object]
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    try {
        $v1Probe = Invoke-WorkbookProbe -Excel $excel -TypeLibPath $v1TypeLibPath -Populate {
            param($wb)
            $mod = $wb.VBProject.VBComponents.Add(1)
            $mod.Name = "MainModule"
            $code = @"
Public Function RunProbe()
    Dim obj As TestEventServer
    Set obj = New TestEventServer
    RunProbe = obj.Ping()
End Function
"@
            [void]$mod.CodeModule.AddFromString($code)
        }
        Add-Row -Rows $rows -CaseId "CCT-048-TES-002" -Scenario "AddFromFile v1 typelib + New TestEventServer + Ping()" -Status $v1Probe.status -Observed $v1Probe.observed

        $v2Probe = Invoke-WorkbookProbe -Excel $excel -TypeLibPath $v2TypeLibPath -Populate {
            param($wb)
            $mod = $wb.VBProject.VBComponents.Add(1)
            $mod.Name = "MainModule"
            $code = @"
Public Function RunProbe()
    Dim obj As TestEventServer
    Set obj = New TestEventServer
    RunProbe = obj.Ping()
End Function
"@
            [void]$mod.CodeModule.AddFromString($code)
        }
        Add-Row -Rows $rows -CaseId "CCT-048-TES-003" -Scenario "AddFromFile v2 typelib + New TestEventServer + Ping()" -Status $v2Probe.status -Observed $v2Probe.observed

        $matrixRoot = Join-Path $runDir "matrix"
        New-Item -ItemType Directory -Force -Path $matrixRoot | Out-Null
        $matrixTypeLib = Join-Path $matrixRoot "OxVba.TestEventServer.tlb"
        $matrixWorkbook = Join-Path $matrixRoot "version_matrix_probe.xlsm"
        Copy-Item -Force $v1TypeLibPath $matrixTypeLib
        New-VersionedWorkbookFixture -Excel $excel -TypeLibPath $matrixTypeLib -WorkbookPath $matrixWorkbook

        Copy-Item -Force $v2TypeLibPath $matrixTypeLib
        $swapProbe = Invoke-ReopenProbe -Excel $excel -WorkbookPath $matrixWorkbook
        Add-Row -Rows $rows -CaseId "CCT-048-TES-004" -Scenario "Saved workbook reopened after referenced typelib path is replaced with v2" -Status $swapProbe.status -Observed $swapProbe.observed

        Remove-Item -Force $matrixTypeLib
        $missingProbe = Invoke-ReopenProbe -Excel $excel -WorkbookPath $matrixWorkbook
        Add-Row -Rows $rows -CaseId "CCT-048-TES-005" -Scenario "Saved workbook reopened after referenced typelib file is removed" -Status $missingProbe.status -Observed $missingProbe.observed

        Copy-Item -Force $v1TypeLibPath $matrixTypeLib
        $repairProbe = Invoke-ReopenProbe -Excel $excel -WorkbookPath $matrixWorkbook
        Add-Row -Rows $rows -CaseId "CCT-048-TES-006" -Scenario "Saved workbook reopened after missing typelib file is restored" -Status $repairProbe.status -Observed $repairProbe.observed
    } finally {
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    }

    $resultsPath = Join-Path $runDir "results.csv"
    $summaryPath = Join-Path $runDir "summary.md"
    $rows | Export-Csv -Path $resultsPath -NoTypeInformation

    $summary = @(
        "# COM TestEventServer Versioned Typelib Probe",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- v1 TypeLib: $v1TypeLibPath",
        "- v2 TypeLib: $v2TypeLibPath",
        "- Results CSV: $resultsPath",
        "",
        "## Cases",
        "| Case | Scenario | Status | Observed |",
        "|---|---|---|---|"
    )
    foreach ($row in $rows) {
        $summary += "| $($row.case_id) | $($row.scenario) | $($row.status) | $($row.observed) |"
    }
    Set-Content -Path $summaryPath -Value ($summary -join "`n")

    Write-Host "com-testeventserver-versioned-typelib-probe: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "results=$resultsPath"
    Write-Host "summary=$summaryPath"
}
finally {
    if (Test-Path $variantRoot) {
        Remove-Item -Recurse -Force -Path $variantRoot -ErrorAction SilentlyContinue
    }
    Pop-Location
}
