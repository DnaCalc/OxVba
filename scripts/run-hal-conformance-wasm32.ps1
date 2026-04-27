param(
    [string]$OutputDir = "docs/evidence/hal_wasm32",
    [switch]$SkipTests,
    [string]$WasmtimeVersion = "42.0.1"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

function Get-WasmtimeBinaryPath {
    param(
        [string]$RepoRoot,
        [string]$Version
    )

    $wasmtime = Get-Command wasmtime -ErrorAction SilentlyContinue
    if ($wasmtime) {
        return $wasmtime.Source
    }

    $toolsDir = Join-Path $RepoRoot "temp/wasmtime"
    New-Item -ItemType Directory -Force $toolsDir | Out-Null

    if ($IsWindows) {
        $archiveName = "wasmtime-v$Version-x86_64-windows.zip"
        $downloadUrl = "https://github.com/bytecodealliance/wasmtime/releases/download/v$Version/$archiveName"
        $archivePath = Join-Path $toolsDir $archiveName
        $extractRoot = Join-Path $toolsDir "wasmtime-v$Version-x86_64-windows"
        $binaryPath = Join-Path $extractRoot "wasmtime.exe"
        if (-not (Test-Path $binaryPath)) {
            if (-not (Test-Path $archivePath)) {
                Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath
            }
            Expand-Archive -Path $archivePath -DestinationPath $toolsDir -Force
        }
        return $binaryPath
    }

    $archiveName = "wasmtime-v$Version-x86_64-linux.tar.xz"
    $downloadUrl = "https://github.com/bytecodealliance/wasmtime/releases/download/v$Version/$archiveName"
    $archivePath = Join-Path $toolsDir $archiveName
    $extractRoot = Join-Path $toolsDir "wasmtime-v$Version-x86_64-linux"
    $binaryPath = Join-Path $extractRoot "wasmtime"
    if (-not (Test-Path $binaryPath)) {
        if (-not (Test-Path $archivePath)) {
            Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath
        }
        tar -xf $archivePath -C $toolsDir
    }
    return $binaryPath
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $repoRoot
try {
    Write-Host "[hal-wasm32] installing wasm32-wasip1 target"
    rustup target add wasm32-wasip1

    $wasmtimePath = Get-WasmtimeBinaryPath -RepoRoot $repoRoot -Version $WasmtimeVersion
    if (-not (Test-Path $wasmtimePath)) {
        throw "wasmtime binary not found at $wasmtimePath"
    }
    Write-Host "[hal-wasm32] using wasmtime: $wasmtimePath"

    $env:CARGO_TARGET_WASM32_WASIP1_RUNNER = "$wasmtimePath run --dir ."

    if (-not $SkipTests) {
        Write-Host "[hal-wasm32] running crate tests on wasm32-wasip1"
        cargo test -p oxvba-hal --target wasm32-wasip1
    }

    Write-Host "[hal-wasm32] building wasm32 conformance binary"
    cargo build -q -p oxvba-hal --bin hal-conformance --target wasm32-wasip1

    New-Item -ItemType Directory -Force $OutputDir | Out-Null
    Write-Host "[hal-wasm32] generating wasm32 conformance artifacts"
    & $wasmtimePath run --dir . "target/wasm32-wasip1/debug/hal-conformance.wasm" --output-dir $OutputDir
}
finally {
    Pop-Location
}
