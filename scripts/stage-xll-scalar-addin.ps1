param(
    [string]$ProjectPath = "examples/xll/scalar_addin",
    [string]$OutputRoot = "target/xll-host-validation",
    [string]$RunId = ""
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if ([string]::IsNullOrWhiteSpace($RunId)) {
        $RunId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    }

    if (-not (Test-Path -LiteralPath $ProjectPath)) {
        throw "Missing XLL scalar Addin project path: $ProjectPath"
    }

    $resolvedProject = (Resolve-Path -LiteralPath $ProjectPath).Path
    $projectName = "ScalarAddin"
    $runDir = Join-Path $OutputRoot (Join-Path "scalar_addin" $RunId)
    $sourceDir = Join-Path $runDir "source"
    $artifactPath = Join-Path $runDir "$projectName.xll"
    $buildLogPath = Join-Path $runDir "build.log"
    $manifestPath = Join-Path $runDir "manifest.json"

    New-Item -ItemType Directory -Force -Path $sourceDir | Out-Null

    Copy-Item -LiteralPath (Join-Path $resolvedProject "ScalarAddin.basproj") -Destination $sourceDir -Force
    Copy-Item -LiteralPath (Join-Path $resolvedProject "ScalarExports.bas") -Destination $sourceDir -Force
    Copy-Item -LiteralPath (Join-Path $resolvedProject "expected.csv") -Destination $sourceDir -Force

    $startedAt = (Get-Date).ToUniversalTime().ToString("o")
    $command = @(
        "cargo",
        "run",
        "-p",
        "oxvba-cli",
        "--",
        "build",
        $ProjectPath,
        "-o",
        $artifactPath
    )

    & cargo run -p oxvba-cli -- build $ProjectPath -o $artifactPath 2>&1 |
        Tee-Object -FilePath $buildLogPath
    $exitCode = $LASTEXITCODE
    $endedAt = (Get-Date).ToUniversalTime().ToString("o")

    if ($exitCode -ne 0) {
        throw "XLL scalar Addin staging failed with exit code $exitCode; see $buildLogPath"
    }
    if (-not (Test-Path -LiteralPath $artifactPath)) {
        throw "XLL scalar Addin staging did not produce artifact: $artifactPath"
    }

    $artifact = Get-Item -LiteralPath $artifactPath
    $hash = Get-FileHash -LiteralPath $artifactPath -Algorithm SHA256
    $manifest = [ordered]@{
        run_id = $RunId
        project_path = $resolvedProject
        output_root = (Resolve-Path -LiteralPath $runDir).Path
        command = $command -join " "
        started_at = $startedAt
        ended_at = $endedAt
        exit_code = $exitCode
        artifact_path = (Resolve-Path -LiteralPath $artifactPath).Path
        artifact_bytes = $artifact.Length
        artifact_sha256 = $hash.Hash
        build_log = (Resolve-Path -LiteralPath $buildLogPath).Path
        source_snapshot = (Resolve-Path -LiteralPath $sourceDir).Path
        excel_host_validated = $false
    }
    $manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $manifestPath -Encoding utf8

    Write-Host "xll scalar addin staged: $runDir"
    Write-Host "artifact: $artifactPath ($($artifact.Length) bytes)"
    Write-Host "manifest: $manifestPath"
}
finally {
    Pop-Location
}
