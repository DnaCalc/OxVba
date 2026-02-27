param(
    [string]$Command = "",
    [string]$CommandFilePath = "",
    [Parameter(Mandatory = $true)]
    [string]$ExitCodePath,
    [Parameter(Mandatory = $true)]
    [string]$CompletedPath
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$exitCode = 1
try {
    if ([string]::IsNullOrWhiteSpace($Command) -and [string]::IsNullOrWhiteSpace($CommandFilePath)) {
        throw "async-task-runner: either -Command or -CommandFilePath must be provided"
    }

    $commandText = $Command
    if (-not [string]::IsNullOrWhiteSpace($CommandFilePath)) {
        if (-not (Test-Path $CommandFilePath)) {
            throw "async-task-runner: missing command file: $CommandFilePath"
        }
        $commandText = Get-Content $CommandFilePath -Raw
    }

    Invoke-Expression $commandText
    if ($null -ne $LASTEXITCODE) {
        $exitCode = [int]$LASTEXITCODE
    }
    else {
        $exitCode = 0
    }
}
catch {
    Write-Error $_
    $exitCode = 1
}
finally {
    $exitDir = Split-Path -Parent $ExitCodePath
    if (-not (Test-Path $exitDir)) {
        New-Item -ItemType Directory -Path $exitDir -Force | Out-Null
    }

    $completedDir = Split-Path -Parent $CompletedPath
    if (-not (Test-Path $completedDir)) {
        New-Item -ItemType Directory -Path $completedDir -Force | Out-Null
    }

    Set-Content -Path $ExitCodePath -Value "$exitCode"
    Set-Content -Path $CompletedPath -Value ((Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ"))
}

exit $exitCode
