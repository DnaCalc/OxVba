param(
    [string]$OutputRoot = "docs/evidence/conformance/oracle_captures",
    [string]$RunId = "",
    [switch]$NoArtifacts,
    [switch]$KeepArtifacts,
    [switch]$NoThrow,
    [switch]$DisableDialogGuardian,
    [int]$DialogGuardianPollMs = 250,
    [int]$DialogGuardianMaxSeconds = 1800
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not $IsWindows) {
        throw "wrapped COM server Excel IMyCalc oracle runner is Windows-only"
    }

    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-wrapped-server-excel-imycalc-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) {
        $OutputRoot = New-NoArtifactEvidenceDir -Scope "com-wrapped-server-excel-imycalc-oracle" -RunId $resolvedRunId
    }

    Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win32User32Pid {
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@

    function Get-WindowProcessId {
        param([int]$Hwnd)
        [uint32]$windowPid = 0
        [void][Win32User32Pid]::GetWindowThreadProcessId([IntPtr]::new($Hwnd), [ref]$windowPid)
        [int]$windowPid
    }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) {
        $OutputRoot
    } else {
        Join-Path $workspaceRoot $OutputRoot
    }
    $runDir = Join-Path $runRoot "com_wrapped_server_excel_imycalc_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null
    $artifactDir = Join-Path $runDir "artifacts"
    New-Item -ItemType Directory -Force -Path $artifactDir | Out-Null
    $buildRoot = Join-Path $artifactDir "imycalc_server"
    New-Item -ItemType Directory -Force -Path $buildRoot | Out-Null

    $dialogGuardianLog = Join-Path $runDir "excel_dialog_guardian.log"
    $dialogGuardianStop = Join-Path $runDir "excel_dialog_guardian.stop"
    $dialogGuardianProc = $null

    $interfaceSource = @'
Attribute VB_Name = "IMyCalc"
Option Explicit
Public Function AddThem(ByVal leftValue As Double, ByVal rightValue As Double) As Double
End Function
'@

    $classSource = @'
Attribute VB_Name = "MyCalc"
Option Explicit
Implements IMyCalc
Private Function IMyCalc_AddThem(ByVal leftValue As Double, ByVal rightValue As Double) As Double
IMyCalc_AddThem = leftValue + rightValue
End Function
'@

    $serverBasproj = @'
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>ComServer</OutputType>
    <BuildTarget>WrappedComServer</BuildTarget>
    <ProjectName>MyCalcServer</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <ClassModule Include="IMyCalc.cls" />
    <ClassModule Include="MyCalc.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <ProgId>MyCalcServerLib.MyCalc</ProgId>
    </ClassModule>
  </ItemGroup>
</Project>
'@

    $consumerModule = @'
Option Explicit
Public Function RunProbe() As Double
    Dim calc As IMyCalc
    Dim resultValue As Double
    Set calc = New MyCalc
    resultValue = calc.AddThem(1.25, 2.5)
    Debug.Print resultValue
    RunProbe = resultValue
End Function
'@

    $interfacePath = Join-Path $buildRoot "IMyCalc.cls"
    $classPath = Join-Path $buildRoot "MyCalc.cls"
    $basprojPath = Join-Path $buildRoot "MyCalcServer.basproj"
    $consumerModulePath = Join-Path $artifactDir "OracleModule.bas"
    Set-Content -Path $interfacePath -Value $interfaceSource -Encoding UTF8
    Set-Content -Path $classPath -Value $classSource -Encoding UTF8
    Set-Content -Path $basprojPath -Value $serverBasproj -Encoding UTF8
    Set-Content -Path $consumerModulePath -Value $consumerModule -Encoding UTF8

    $dllPath = Join-Path $buildRoot "MyCalcServer.dll"
    $tlbPath = Join-Path $buildRoot "MyCalcServer.tlb"
    $buildLog = Join-Path $runDir "wrapped_server_build.log.txt"
    $buildCmd = @("run", "--quiet", "-p", "oxvba-cli", "--", "build", $basprojPath, "-o", $dllPath)
    $buildCmdText = "cargo " + ($buildCmd -join " ")
    $null = & cargo @buildCmd 2>&1 | Tee-Object -FilePath $buildLog
    $buildExitCode = $LASTEXITCODE
    if ($buildExitCode -ne 0) {
        throw "wrapped COM server build failed (exit=$buildExitCode); log=$buildLog"
    }
    if (-not (Test-Path $dllPath)) {
        throw "wrapped COM server build did not produce DLL: $dllPath"
    }
    if (-not (Test-Path $tlbPath)) {
        throw "wrapped COM server build did not produce TLB: $tlbPath"
    }

    $registered = $false
    $registrationHResult = ""
    $excel = $null
    $wb = $null
    $module = $null
    $excelVersion = ""
    $excelPid = -1
    $automationSecurityOriginal = $null
    $status = "fail"
    $observed = ""
    $expected = "3.75"
    $errorMessage = ""

    try {
        & regsvr32 /s $dllPath
        if ($LASTEXITCODE -ne 0) {
            throw "regsvr32 /s failed for $dllPath (exit=$LASTEXITCODE)"
        }
        $registered = $true

        $excel = New-Object -ComObject Excel.Application
        $excel.Visible = $false
        $excel.DisplayAlerts = $false
        try {
            $automationSecurityOriginal = $excel.AutomationSecurity
            # msoAutomationSecurityLow = 1
            $excel.AutomationSecurity = 1
        } catch {
            # Continue if Office security automation property is unavailable.
        }
        $excelVersion = [string]$excel.Version
        $excelPid = Get-WindowProcessId -Hwnd ([int]$excel.Hwnd)
        if (-not $DisableDialogGuardian -and $excelPid -gt 0) {
            if (Test-Path $dialogGuardianStop) {
                Remove-Item -Force $dialogGuardianStop
            }
            $guardianScript = Join-Path $PSScriptRoot "excel-dialog-guardian.ps1"
            $guardianArgs = @(
                "-NoLogo",
                "-NoProfile",
                "-ExecutionPolicy", "Bypass",
                "-File", $guardianScript,
                "-ExcelPid", "$excelPid",
                "-StopFile", $dialogGuardianStop,
                "-LogFile", $dialogGuardianLog,
                "-PollMs", "$DialogGuardianPollMs",
                "-MaxSeconds", "$DialogGuardianMaxSeconds"
            )
            $dialogGuardianProc = Start-Process -FilePath "pwsh" -ArgumentList $guardianArgs -WindowStyle Hidden -PassThru
        }

        $wb = $excel.Workbooks.Add()
        [void]$wb.VBProject.References.AddFromFile($tlbPath)
        $module = $wb.VBProject.VBComponents.Add(1)
        $module.Name = "OracleModule"
        [void]$module.CodeModule.AddFromString($consumerModule)
        $macroName = "'{0}'!OracleModule.RunProbe" -f $wb.Name
        $result = $excel.Run($macroName)
        $observed = [string][double]$result

        if ([Math]::Abs(([double]$result) - 3.75) -lt 0.0000001) {
            $status = "pass"
        } else {
            $status = "fail"
            $errorMessage = "unexpected result value"
        }
    } catch {
        $status = "fail"
        $errorMessage = $_.Exception.Message
        if ([string]::IsNullOrWhiteSpace($observed)) {
            $observed = $errorMessage
        }
    } finally {
        if ($wb -ne $null) {
            $wb.Close($false)
            [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
        }
        if ($module -ne $null) {
            [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($module)
        }
        if ($dialogGuardianProc -ne $null) {
            New-Item -ItemType File -Path $dialogGuardianStop -Force | Out-Null
            Start-Sleep -Milliseconds 500
            if (-not $dialogGuardianProc.HasExited) {
                Stop-Process -Id $dialogGuardianProc.Id -Force -ErrorAction SilentlyContinue
            }
            if (Test-Path $dialogGuardianStop) {
                Remove-Item -Force $dialogGuardianStop -ErrorAction SilentlyContinue
            }
        }
        if ($excel -ne $null) {
            if ($null -ne $automationSecurityOriginal) {
                try {
                    $excel.AutomationSecurity = $automationSecurityOriginal
                } catch {
                    # Best-effort restore.
                }
            }
            $excel.Quit()
            [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
        }
        if ($registered) {
            & regsvr32 /s /u $dllPath
            if ($LASTEXITCODE -ne 0) {
                $registrationHResult = "unregister failed exit=$LASTEXITCODE"
            }
        }
    }

    $resultRow = [PSCustomObject]@{
        run_id = $resolvedRunId
        status = $status
        expected = $expected
        observed = $observed
        excel_version = $excelVersion
        excel_pid = $excelPid
        wrapped_server_basproj = $basprojPath
        wrapped_server_dll = $dllPath
        wrapped_server_tlb = $tlbPath
        wrapped_server_build_command = $buildCmdText
        wrapped_server_build_log = $buildLog
        registration_note = if ([string]::IsNullOrWhiteSpace($registrationHResult)) { "ok" } else { $registrationHResult }
        error = $errorMessage
    }

    $csvPath = Join-Path $runDir "results.csv"
    $resultRow | Export-Csv -Path $csvPath -NoTypeInformation

    $summaryPath = Join-Path $runDir "summary.md"
    $summary = @(
        "# Wrapped COM Server Excel IMyCalc Oracle Run",
        "",
        "- Run ID: $resolvedRunId",
        "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Status: $status",
        "- Expected: $expected",
        "- Observed: $observed",
        "- Excel version: $excelVersion",
        "- Excel process id: $excelPid",
        "- Dialog guardian enabled: $([string](-not $DisableDialogGuardian))",
        "- Wrapped server build command: $buildCmdText",
        "- Wrapped server build log: $buildLog",
        "- Wrapped server basproj: $basprojPath",
        "- Wrapped server DLL: $dllPath",
        "- Wrapped server TLB: $tlbPath",
        "- Consumer VBA module source: $consumerModulePath",
        "",
        "## Executed VBA Probe",
        '```vb',
        $consumerModule.TrimEnd(),
        '```',
        "",
        "## Notes",
        "- `RunProbe` creates `Dim calc As IMyCalc`, assigns `Set calc = New MyCalc`, calls `AddThem`, and executes `Debug.Print resultValue`.",
        "- Validation compares the returned probe value against `3.75`."
    )
    Set-Content -Path $summaryPath -Value ($summary -join [Environment]::NewLine)

    Write-Host "com-wrapped-server-excel-imycalc-oracle: complete"
    Write-Host "run_dir=$runDir"
    Write-Host "results=$csvPath"
    Write-Host "summary=$summaryPath"

    if ($status -ne "pass" -and -not $NoThrow) {
        throw "wrapped COM server Excel IMyCalc oracle failed: $observed"
    }
    return $resultRow
}
finally {
    if (-not $KeepArtifacts -and (Test-Path $artifactDir)) {
        try {
            Remove-Item -Recurse -Force -Path $artifactDir
        } catch {
            Write-Warning "artifact cleanup skipped due lock: $($_.Exception.Message)"
        }
    }
    Pop-Location
}
