$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$sample = "conformance/tests/smoke.bas"
if (-not (Test-Path $sample)) {
    throw "Missing smoke source: $sample"
}

cargo run -q -p oxvba-cli -- run $sample --dump-slots
Write-Host "smoke run: ok"
