param(
    [string]$EvidenceDir = "docs/evidence/conformance/com_early/lanes",
    [string]$RunId = "",
    [switch]$NoCapture,
    [switch]$NoThrow,
    [switch]$NoLatest
)

$args = @{
    LaneId = "E6"
    EvidenceDir = $EvidenceDir
    RunId = $RunId
    NoThrow = $NoThrow
    NoLatest = $NoLatest
}
if ($NoCapture) {
    $args["NoCapture"] = $true
}
& (Join-Path $PSScriptRoot "run-com-early-lane.ps1") @args
