param(
    [string]$FirstTypeLibPath,
    [string]$SecondTypeLibPath,
    [string]$StatePath,
    [string]$VbaDialogHandlerScriptPath,
    [string]$VbaDialogHandlerLogPath,
    [int]$RunTimeoutSeconds = 15
)

$ErrorActionPreference = "Stop"

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class ProbeWin32Pid {
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@

function Get-WindowProcessId {
    param([int]$Hwnd)
    [uint32]$windowPid = 0
    [void][ProbeWin32Pid]::GetWindowThreadProcessId([IntPtr]::new($Hwnd), [ref]$windowPid)
    [int]$windowPid
}

$root = Join-Path $env:TEMP ("oxvba_mixed_broken_ref_" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $root | Out-Null
$firstCopy = Join-Path $root ([System.IO.Path]::GetFileName($FirstTypeLibPath))
$secondCopy = Join-Path $root ([System.IO.Path]::GetFileName($SecondTypeLibPath))
$workbookPath = Join-Path $root "probe.xlsm"
$vbaDialogHandlerStop = Join-Path $root "_vba_dialog_handler.stop"
$deadlineFile = Join-Path $root "_run_deadline.txt"
Copy-Item $FirstTypeLibPath $firstCopy -Force
Copy-Item $SecondTypeLibPath $secondCopy -Force
$code = "Public Function RunProbe()`n    Dim obj As TestEventServer`n    Set obj = New TestEventServer`n    RunProbe = obj.Ping()`nEnd Function`n"

$excel = $null
$wb = $null
$reopened = $null
$vbaDialogHandler = $null
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $excelPid = Get-WindowProcessId -Hwnd ([int]$excel.Hwnd)
    if ($excelPid -gt 0 -and (Test-Path $VbaDialogHandlerScriptPath)) {
        if (Test-Path $VbaDialogHandlerLogPath) {
            Remove-Item -Force -Path $VbaDialogHandlerLogPath
        }
        $vbaDialogHandler = Start-Process `
            -FilePath (Get-Command pwsh).Source `
            -ArgumentList @(
                "-NoProfile",
                "-NonInteractive",
                "-File",
                $VbaDialogHandlerScriptPath,
                $excelPid,
                $vbaDialogHandlerStop,
                $VbaDialogHandlerLogPath,
                $deadlineFile,
                200
            ) `
            -PassThru `
            -WindowStyle Hidden
    }

    $wb = $excel.Workbooks.Add()
    [void]$wb.VBProject.References.AddFromFile($firstCopy)
    [void]$wb.VBProject.References.AddFromFile($secondCopy)
    $mod = $wb.VBProject.VBComponents.Add(1)
    $mod.Name = "MainModule"
    [void]$mod.CodeModule.AddFromString($code)
    $wb.SaveAs($workbookPath, 52)
    $wb.Close($false)
    [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
    $wb = $null

    Rename-Item $firstCopy ($firstCopy + ".missing")

    $reopened = $excel.Workbooks.Open($workbookPath)
    $refs = @(
        $reopened.VBProject.References |
            Where-Object {
                $_.Guid -in @(
                    "{E2A30001-0001-0001-0001-000000000001}",
                    "{E2A30001-0001-0001-0001-000000000101}"
                )
            } |
            ForEach-Object { "name={0};guid={1};broken={2}" -f $_.Name, $_.Guid, [string]$_.IsBroken }
    )
    @{ stage = "reopened"; refs = $refs } | ConvertTo-Json -Compress | Set-Content -Path $StatePath

    try {
        [DateTime]::UtcNow.AddSeconds($RunTimeoutSeconds).Ticks | Set-Content -Path $deadlineFile
        $result = [string]$excel.Run("RunProbe")
        @{ stage = "completed"; refs = $refs; run = $result; handler_log = $VbaDialogHandlerLogPath } | ConvertTo-Json -Compress | Set-Content -Path $StatePath
    } catch {
        @{ stage = "run_error"; refs = $refs; run_error = $_.Exception.Message; handler_log = $VbaDialogHandlerLogPath } | ConvertTo-Json -Compress | Set-Content -Path $StatePath
    } finally {
        if (Test-Path $deadlineFile) {
            Remove-Item -Force -Path $deadlineFile
        }
    }
} finally {
    Set-Content -Path $vbaDialogHandlerStop -Value "stop" -Encoding UTF8
    if ($vbaDialogHandler -ne $null) {
        $null = $vbaDialogHandler.WaitForExit(2000)
        if (-not $vbaDialogHandler.HasExited) {
            Stop-Process -Id $vbaDialogHandler.Id -Force -ErrorAction SilentlyContinue
        }
    }
    if ($reopened -ne $null) {
        $reopened.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($reopened)
    }
    if ($wb -ne $null) {
        $wb.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
    }
    if ($excel -ne $null) {
        $excel.Quit()
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    }
}
