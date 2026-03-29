param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$BrArgs
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $lockDir = ".beads"
    if (-not (Test-Path $lockDir)) {
        New-Item -ItemType Directory -Path $lockDir -Force | Out-Null
    }

    $lockPath = Join-Path $lockDir "br-command.lock"
    $deadline = (Get-Date).AddMinutes(2)
    $stream = $null

    while (-not $stream) {
        try {
            $stream = [System.IO.File]::Open($lockPath, [System.IO.FileMode]::OpenOrCreate, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
        }
        catch {
            if ((Get-Date) -gt $deadline) {
                throw "invoke-br-serialized: timed out waiting for bead mutation lock at $lockPath"
            }
            Start-Sleep -Milliseconds 200
        }
    }

    try {
        if (-not $BrArgs -or $BrArgs.Count -eq 0) {
            throw "invoke-br-serialized: provide br arguments, e.g. ./scripts/invoke-br-serialized.ps1 create --title '...'"
        }

        & br @BrArgs
        exit $LASTEXITCODE
    }
    finally {
        $stream.Dispose()
    }
}
finally {
    Pop-Location
}
