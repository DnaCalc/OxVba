param(
    [string]$OutputDir = "docs/evidence/hal_wasm32",
    [switch]$SkipTests,
    [string]$RustToolchain = "1.94.1",
    [string]$WasmtimeVersion = "42.0.1",
    [string]$WasmtimeLinuxSha256 = "dd5253f3cb521bb094f9951c3d2c45c746b31e5723b07ce56f162ec9bab44d59",
    [string]$WasmtimeLinuxBinarySha256 = "21f8e8f994a96d2267afe4a4c06a6302e78aca20e9438afbf01dd443fe32108b",
    [string]$WasmtimeWindowsSha256 = "daa52754776eabdbbf82037d41a26f556ccd4fd5723dcab328b12c680894c072",
    [string]$WasmtimeWindowsBinarySha256 = "b86766999318183c37f5a51c56d4ae26ecdf34099cd0ebbbdf0108e1013ba4b1"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

function Get-WasmtimeBinaryPath {
    param(
        [string]$RepoRoot,
        [string]$Version,
        [string]$LinuxSha256,
        [string]$LinuxBinarySha256,
        [string]$WindowsSha256,
        [string]$WindowsBinarySha256
    )

    $toolsDir = Join-Path $RepoRoot "temp/wasmtime"
    New-Item -ItemType Directory -Force $toolsDir | Out-Null

    if ($IsWindows) {
        $archiveName = "wasmtime-v$Version-x86_64-windows.zip"
        $downloadUrl = "https://github.com/bytecodealliance/wasmtime/releases/download/v$Version/$archiveName"
        $archivePath = Join-Path $toolsDir $archiveName
        $extractRoot = Join-Path $toolsDir "wasmtime-v$Version-x86_64-windows"
        $binaryPath = Join-Path $extractRoot "wasmtime.exe"
        $expectedHash = $WindowsSha256
        $expectedBinaryHash = $WindowsBinarySha256
        if (-not (Test-Path $binaryPath)) {
            if (-not (Test-Path $archivePath)) {
                Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath
            }
            $actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
            if ($actualHash -cne $expectedHash) {
                throw "wasmtime archive hash mismatch for '$archiveName': expected $expectedHash, found $actualHash"
            }
            Expand-Archive -Path $archivePath -DestinationPath $toolsDir -Force
        }
        $actualBinaryHash = (Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actualBinaryHash -cne $expectedBinaryHash) {
            throw "wasmtime binary hash mismatch for '$binaryPath': expected $expectedBinaryHash, found $actualBinaryHash"
        }
        return $binaryPath
    }

    $archiveName = "wasmtime-v$Version-x86_64-linux.tar.xz"
    $downloadUrl = "https://github.com/bytecodealliance/wasmtime/releases/download/v$Version/$archiveName"
    $archivePath = Join-Path $toolsDir $archiveName
    $extractRoot = Join-Path $toolsDir "wasmtime-v$Version-x86_64-linux"
    $binaryPath = Join-Path $extractRoot "wasmtime"
    $expectedHash = $LinuxSha256
    $expectedBinaryHash = $LinuxBinarySha256
    if (-not (Test-Path $binaryPath)) {
        if (-not (Test-Path $archivePath)) {
            Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath
        }
        $actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actualHash -cne $expectedHash) {
            throw "wasmtime archive hash mismatch for '$archiveName': expected $expectedHash, found $actualHash"
        }
        tar -xf $archivePath -C $toolsDir
    }
    $actualBinaryHash = (Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualBinaryHash -cne $expectedBinaryHash) {
        throw "wasmtime binary hash mismatch for '$binaryPath': expected $expectedBinaryHash, found $actualBinaryHash"
    }
    return $binaryPath
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $repoRoot
try {
    Write-Host "[hal-wasm32] installing wasm32-wasip1 target"
    if ($RustToolchain -notmatch '^\d+\.\d+\.\d+$') {
        throw "RustToolchain must be an exact semantic version, found '$RustToolchain'"
    }
    rustup target add --toolchain $RustToolchain wasm32-wasip1

    $wasmtimePath = Get-WasmtimeBinaryPath `
        -RepoRoot $repoRoot `
        -Version $WasmtimeVersion `
        -LinuxSha256 $WasmtimeLinuxSha256 `
        -LinuxBinarySha256 $WasmtimeLinuxBinarySha256 `
        -WindowsSha256 $WasmtimeWindowsSha256 `
        -WindowsBinarySha256 $WasmtimeWindowsBinarySha256
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
