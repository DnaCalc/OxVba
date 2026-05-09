param(
    [switch]$NoQuiet
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

$isWindowsPlatform = [System.Environment]::OSVersion.Platform -eq "Win32NT"
if (-not $isWindowsPlatform) {
    Write-Error "COM-0007 wrapped COM server late-bound evidence requires Windows."
}

$cargoArgs = @(
    "test",
    "-p",
    "oxvba-build",
    "wrapped_com_server_build_compiles_dll_with_standard_exports"
)

if (-not $NoQuiet) {
    $cargoArgs += "--quiet"
}

Write-Host "COM-0007 wrapped COM server late-bound controlled client"
Write-Host "Command: cargo $($cargoArgs -join ' ')"

& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "COM-0007 wrapped COM server late-bound controlled client: PASS"
