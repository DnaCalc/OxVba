$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$sample = "conformance/tests/smoke.bas"
if (-not (Test-Path $sample)) {
    throw "Missing smoke source: $sample"
}

cargo run -p oxvba-cli -- run $sample
Write-Host "smoke run: ok"
