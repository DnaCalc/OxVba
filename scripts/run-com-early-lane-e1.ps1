param(
    [string]$EvidenceDir = "docs/evidence/conformance/com_early/lanes",
    [string]$RunId = "",
    [switch]$NoCapture,
    [switch]$NoThrow
)

$args = @{
    LaneId = "E1"
    EvidenceDir = $EvidenceDir
    RunId = $RunId
    NoThrow = $NoThrow
}
if ($NoCapture) {
    $args["NoCapture"] = $true
}
& (Join-Path $PSScriptRoot "run-com-early-lane.ps1") @args
