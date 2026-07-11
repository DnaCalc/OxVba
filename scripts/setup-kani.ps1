param(
    [switch]$Install,
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if ($Install) {
        $installArguments = @("install", "kani-verifier", "--locked")
        if (-not [string]::IsNullOrWhiteSpace($Version)) {
            if ($Version -notmatch '^\d+\.\d+\.\d+$') {
                throw "kani setup: Version must be an exact semantic version, found '$Version'"
            }
            $installArguments += @("--version", $Version)
        }
        cargo @installArguments
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
    if (-not [string]::IsNullOrWhiteSpace($Version) -and
        $kaniVersion -notmatch "(?<![0-9.])$([regex]::Escape($Version))(?![0-9.])") {
        throw "kani setup: expected cargo-kani $Version, found '$kaniVersion'"
    }

    Write-Host "kani setup: ok ($kaniVersion)"
}
finally {
    Pop-Location
}
