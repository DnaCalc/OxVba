param(
    [string]$Target
)

if (-not $Target) {
    if ($IsWindows) {
        $Target = "x86_64-pc-windows-msvc"
    } elseif ($IsLinux) {
        $Target = "x86_64-unknown-linux-gnu"
    } else {
        throw "Unsupported host OS for default Bruto target selection."
    }
}

$exeName = if ($Target -like "*windows*") { "oxvba-bruto.exe" } else { "oxvba-bruto" }

Write-Host "Building oxvba-bruto for $Target"
cargo build --release -p oxvba-bruto --target $Target
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$artifact = Join-Path "target/$Target/release" $exeName
Write-Host "Built $artifact"
