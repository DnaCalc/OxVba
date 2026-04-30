param(
    [string]$OutputDir = "docs/evidence/web-shell/screen-audit-latest"
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$assetRoot = Join-Path $repoRoot "crates/oxvba-web-shell/assets"
$htmlPath = Join-Path $assetRoot "index.html"
$cssPath = Join-Path $assetRoot "styles.css"
$jsPath = Join-Path $assetRoot "app.js"
$resolvedOutput = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $OutputDir))
$repoFull = [System.IO.Path]::GetFullPath($repoRoot)
if (-not $resolvedOutput.StartsWith($repoFull, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "OutputDir must resolve inside the repository: $resolvedOutput"
}

New-Item -ItemType Directory -Force -Path $resolvedOutput | Out-Null

$expectedScreens = @("workspace", "editor", "diagnostics", "immediate", "debugger")
$html = Get-Content -Raw $htmlPath
$css = Get-Content -Raw $cssPath
$js = Get-Content -Raw $jsPath

$checks = New-Object System.Collections.Generic.List[object]
function Add-Check([string]$Name, [bool]$Pass, [string]$Detail) {
    $checks.Add([pscustomobject]@{
        name = $Name
        pass = $Pass
        detail = $Detail
    }) | Out-Null
}

Add-Check "frankentui-root" ($html -match 'data-app="oxide-frankentui"') "Root carries frankentui app marker."
Add-Check "responsive-css" ($css -match '@media \(max-width: 860px\)') "Responsive breakpoint is present."
Add-Check "run-controls" (($html -match 'data-command="run"') -and ($html -match 'data-command="reset"')) "Run and reset controls are present."
Add-Check "bridge-state" ($html -match 'v1 typed') "Bridge contract state is visible."

foreach ($screen in $expectedScreens) {
    Add-Check "screen-markup-$screen" ($html -match "data-screen=`"$screen`"") "Screen section exists."
    Add-Check "screen-tab-$screen" ($html -match "data-screen-target=`"$screen`"") "Screen tab exists."
    Add-Check "screen-js-$screen" ($js -match "`"$screen`"") "Screen is included in JS routing."
}

$chromeCandidates = @(
    "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
    "$env:ProgramFiles(x86)\Google\Chrome\Application\chrome.exe",
    "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe",
    "$env:ProgramFiles(x86)\Microsoft\Edge\Application\msedge.exe"
)
$chrome = $chromeCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $chrome) {
    Add-Check "headless-browser" $false "No Chrome or Edge executable found for capture."
} else {
    Add-Check "headless-browser" $true "Using $chrome"
}

$magick = Get-Command magick -ErrorAction SilentlyContinue
if ($magick) {
    Add-Check "image-probe" $true "Using ImageMagick for capture probes."
} else {
    Add-Check "image-probe" $false "ImageMagick not found; screenshots can be captured but not probed."
}

$captures = New-Object System.Collections.Generic.List[object]
if ($chrome) {
    $fileUri = (New-Object System.Uri($htmlPath)).AbsoluteUri
    $profileRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("oxvba-web-shell-audit-" + [System.Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force -Path $profileRoot | Out-Null
    foreach ($screen in $expectedScreens) {
        $screenshot = Join-Path $resolvedOutput "$screen.png"
        $userDataDir = Join-Path $profileRoot "chrome-profile-$screen"
        New-Item -ItemType Directory -Force -Path $userDataDir | Out-Null
        $url = "$fileUri#$screen"
        $args = @(
            "--headless=new",
            "--disable-gpu",
            "--hide-scrollbars",
            "--window-size=1440,960",
            "--user-data-dir=$userDataDir",
            "--screenshot=$screenshot",
            $url
        )
        & $chrome @args | Out-Null
        $exists = Test-Path $screenshot
        $width = $null
        $height = $null
        $mean = $null
        $nonBlank = $exists
        if ($exists -and $magick) {
            $identity = (& $magick.Source identify -format "%w,%h,%[fx:mean]" $screenshot) -join ""
            $parts = $identity.Split(",")
            $width = [int]$parts[0]
            $height = [int]$parts[1]
            $mean = [double]::Parse($parts[2], [System.Globalization.CultureInfo]::InvariantCulture)
            $nonBlank = ($width -ge 1000) -and ($height -ge 700) -and ($mean -gt 0.03) -and ($mean -lt 0.98)
        }
        Add-Check "capture-$screen" $nonBlank "Captured $screen to $screenshot"
        $captures.Add([pscustomobject]@{
            screen = $screen
            path = $screenshot
            exists = $exists
            width = $width
            height = $height
            mean = $mean
            non_blank = $nonBlank
        }) | Out-Null
    }
    if (Test-Path $profileRoot) {
        Remove-Item -LiteralPath $profileRoot -Recurse -Force
    }
}

$passed = ($checks | Where-Object { -not $_.pass }).Count -eq 0
$result = [pscustomobject]@{
    generated_at = (Get-Date).ToString("o")
    asset_root = $assetRoot
    expected_screens = $expectedScreens
    passed = $passed
    checks = $checks
    captures = $captures
}

$jsonPath = Join-Path $resolvedOutput "screen-audit.json"
$mdPath = Join-Path $resolvedOutput "screen-audit.md"
$result | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 $jsonPath

$md = New-Object System.Collections.Generic.List[string]
$md.Add("# OxIde Frankentui Screen Audit")
$md.Add("")
$md.Add("Generated: $($result.generated_at)")
$md.Add("")
$md.Add("## Result")
$md.Add("")
$md.Add("Passed: $passed")
$md.Add("")
$md.Add("## Checks")
$md.Add("")
foreach ($check in $checks) {
    $status = if ($check.pass) { "pass" } else { "fail" }
    $md.Add("- $status - $($check.name): $($check.detail)")
}
$md.Add("")
$md.Add("## Captures")
$md.Add("")
foreach ($capture in $captures) {
    $fileName = Split-Path -Leaf $capture.path
    $md.Add("- $($capture.screen): ``$fileName`` ($($capture.width)x$($capture.height), mean=$($capture.mean))")
}
$md -join "`n" | Set-Content -Encoding UTF8 $mdPath

if (-not $passed) {
    Write-Error "Web shell screen audit failed. See $mdPath"
}

Write-Output "Web shell screen audit passed. Report: $mdPath"
