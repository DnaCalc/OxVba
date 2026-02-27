param(
    [switch]$Install
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if ($Install) {
        cargo install kani-verifier --locked
        cargo kani setup
    }

    $kaniVersion = ""
    $kaniAvailable = $false
    try {
        $kaniVersion = (& cargo kani --version 2>$null) -join " "
        $kaniAvailable = ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($kaniVersion))
    }
    catch {
        $kaniAvailable = $false
    }

    if (-not $kaniAvailable) {
        Write-Host "kani setup: unavailable"
        Write-Host "To install locally:"
        Write-Host "1. cargo install kani-verifier --locked"
        Write-Host "2. cargo kani setup"
        Write-Host "3. ./scripts/run-formal.ps1 -RequireKani"
        exit 1
    }

    Write-Host "kani setup: ok ($kaniVersion)"
}
finally {
    Pop-Location
}
