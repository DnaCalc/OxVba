param(
    [string]$EvidenceDir = "docs/evidence/conformance/oracle_captures/wrapped_com_events_20260509T000000Z"
)

$ErrorActionPreference = "Stop"

New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
$log = Join-Path $EvidenceDir "controlled_sink_test.log"
$excelLog = Join-Path $EvidenceDir "office_probe.log"

try {
    $excel = New-Object -ComObject Excel.Application
    $version = $excel.Version
    $excel.Quit()
    "Excel available: $version" | Set-Content -Path $excelLog
} catch {
    "Excel unavailable: $($_.Exception.Message)" | Set-Content -Path $excelLog
}

cargo test -p oxvba-build wrapped_com_server_build_compiles_dll_with_standard_exports --quiet 2>&1 |
    Tee-Object -FilePath $log

if ($LASTEXITCODE -ne 0) {
    throw "wrapped COM event controlled sink test failed; see $log"
}

Write-Output "wrapped COM event controlled sink evidence written to $EvidenceDir"
