param(
    [switch]$Unregister,
    [string]$Configuration = "Debug"
)

$ErrorActionPreference = "Stop"

Push-Location $PSScriptRoot
try {
    $assemblyPath = Join-Path (Join-Path (Join-Path (Join-Path $PSScriptRoot "bin") $Configuration) "net48") "OxVba.TestEventServer.dll"

    if (-not (Test-Path $assemblyPath)) {
        Write-Host "[oxvba] Building OxVba.TestEventServer ($Configuration)..."
        dotnet build -c $Configuration
        if ($LASTEXITCODE -ne 0) {
            throw "dotnet build failed (exit=$LASTEXITCODE)"
        }
    }

    if (-not (Test-Path $assemblyPath)) {
        throw "Assembly not found at $assemblyPath after build"
    }

    $regasmCandidates = @(
        "${env:WINDIR}\Microsoft.NET\Framework64\v4.0.30319\RegAsm.exe",
        "${env:WINDIR}\Microsoft.NET\Framework\v4.0.30319\RegAsm.exe"
    )
    $regasm = $regasmCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $regasm) {
        throw "RegAsm.exe not found in .NET Framework directories"
    }

    if ($Unregister) {
        Write-Host "[oxvba] Unregistering OxVba.TestEventServer..."
        & $regasm /unregister /codebase $assemblyPath
        if ($LASTEXITCODE -ne 0) {
            throw "RegAsm /unregister failed (exit=$LASTEXITCODE)"
        }
        Write-Host "[oxvba] OxVba.TestEventServer unregistered."
    } else {
        Write-Host "[oxvba] Registering OxVba.TestEventServer..."
        & $regasm /codebase $assemblyPath
        if ($LASTEXITCODE -ne 0) {
            throw "RegAsm /codebase failed (exit=$LASTEXITCODE)"
        }
        Write-Host "[oxvba] OxVba.TestEventServer registered successfully."
        Write-Host "[oxvba] ProgID: OxVba.TestEventServer"
        Write-Host "[oxvba] Assembly: $assemblyPath"
    }
} finally {
    Pop-Location
}
