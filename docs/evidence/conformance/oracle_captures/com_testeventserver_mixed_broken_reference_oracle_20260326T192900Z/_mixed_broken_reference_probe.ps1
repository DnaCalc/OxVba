param(
    [string]$FirstTypeLibPath,
    [string]$SecondTypeLibPath,
    [string]$StatePath
)

$ErrorActionPreference = "Stop"

$root = Join-Path $env:TEMP ("oxvba_mixed_broken_ref_" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $root | Out-Null
$firstCopy = Join-Path $root ([System.IO.Path]::GetFileName($FirstTypeLibPath))
$secondCopy = Join-Path $root ([System.IO.Path]::GetFileName($SecondTypeLibPath))
$workbookPath = Join-Path $root "probe.xlsm"
Copy-Item $FirstTypeLibPath $firstCopy -Force
Copy-Item $SecondTypeLibPath $secondCopy -Force
$code = "Public Function RunProbe()`n    Dim obj As TestEventServer`n    Set obj = New TestEventServer`n    RunProbe = obj.Ping()`nEnd Function`n"

$excel = $null
$wb = $null
$reopened = $null
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false

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
        $result = [string]$excel.Run("RunProbe")
        @{ stage = "completed"; refs = $refs; run = $result } | ConvertTo-Json -Compress | Set-Content -Path $StatePath
    } catch {
        @{ stage = "run_error"; refs = $refs; run_error = $_.Exception.Message } | ConvertTo-Json -Compress | Set-Content -Path $StatePath
    }
} finally {
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
