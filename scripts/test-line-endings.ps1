param(
    [string]$RepositoryRoot = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}
else {
    (Resolve-Path -LiteralPath $RepositoryRoot).Path
}
$validatorSource = Join-Path $repoRoot "scripts/validate-line-endings.ps1"
$attributesSource = Join-Path $repoRoot ".gitattributes"
$utf8 = [Text.UTF8Encoding]::new($false, $true)

function Invoke-GitIn {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    $output = @(& git -C $Root @Arguments 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "line-ending test git failed in '$Root': git $($Arguments -join ' ')`n$($output -join "`n")"
    }
    return $output
}

function Write-Utf8Fixture {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Text,
        [Parameter(Mandatory = $true)][ValidateSet("LF", "CRLF")][string]$Eol
    )

    $parent = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        [void](New-Item -ItemType Directory -Path $parent -Force)
    }
    $normalized = $Text.Replace("`r`n", "`n").Replace("`r", "`n")
    $transport = if ($Eol -eq "CRLF") { $normalized.Replace("`n", "`r`n") } else { $normalized }
    [IO.File]::WriteAllBytes($Path, $utf8.GetBytes($transport))
}

function New-FixtureRepository {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][ValidateSet("LF", "CRLF")][string]$InputEol
    )

    $root = Join-Path $tempBase $Name
    [void](New-Item -ItemType Directory -Path (Join-Path $root "scripts") -Force)
    Copy-Item -LiteralPath $validatorSource -Destination (Join-Path $root "scripts/validate-line-endings.ps1") -Force
    Copy-Item -LiteralPath $attributesSource -Destination (Join-Path $root ".gitattributes") -Force
    Write-Utf8Fixture -Path (Join-Path $root "src/lib.rs") -Eol $InputEol -Text "pub fn fixture() -> i32 {`n    42`n}`n"
    Write-Utf8Fixture -Path (Join-Path $root "src/fixture.c") -Eol $InputEol -Text "int fixture(void) {`n    return 42;`n}`n"
    Write-Utf8Fixture -Path (Join-Path $root "src/fixture.idl") -Eol $InputEol -Text "library Fixture {`n};`n"
    Write-Utf8Fixture -Path (Join-Path $root "docs/readme.md") -Eol $InputEol -Text "# Fixture`n`nLine-ending transport proof.`n"
    Write-Utf8Fixture -Path (Join-Path $root "crates/fixture/golden.snap") -Eol $InputEol -Text "---`nsource: fixture`nexpression: answer`n---`n42`n"
    [IO.File]::WriteAllBytes((Join-Path $root "fixture.dll"), [byte[]](0, 13, 10, 255, 0, 65))

    @(& git init --quiet $root 2>&1) | Out-Null
    Invoke-GitIn -Root $root -Arguments @("config", "user.name", "OxVba EOL Test") | Out-Null
    Invoke-GitIn -Root $root -Arguments @("config", "user.email", "eol-test@example.invalid") | Out-Null
    Invoke-GitIn -Root $root -Arguments @("config", "core.autocrlf", "false") | Out-Null
    Invoke-GitIn -Root $root -Arguments @("add", "--all") | Out-Null
    Invoke-GitIn -Root $root -Arguments @("commit", "--quiet", "-m", "fixture") | Out-Null
    return $root
}

function New-FixtureCheckout {
    param(
        [Parameter(Mandatory = $true)][string]$Origin,
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][ValidateSet("true", "false")][string]$AutoCrlf
    )

    $checkout = Join-Path $tempBase $Name
    $output = @(& git -c "core.autocrlf=$AutoCrlf" clone --quiet --no-local $Origin $checkout 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "line-ending test clone failed: $($output -join "`n")"
    }
    return $checkout
}

function Invoke-Validator {
    param([Parameter(Mandatory = $true)][string]$Root)

    & (Join-Path $Root "scripts/validate-line-endings.ps1") -RepositoryRoot $Root
}

function Invoke-ExpectedFailure {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Action,
        [Parameter(Mandatory = $true)][string]$MessagePattern
    )

    $failed = $false
    try {
        & $Action
    }
    catch {
        if ($_.Exception.Message -notmatch $MessagePattern) {
            throw "line-ending mutation '$Name' failed for the wrong reason: $($_.Exception.Message)"
        }
        $failed = $true
    }
    if (-not $failed) {
        throw "line-ending mutation '$Name' unexpectedly passed"
    }
    Write-Host "line-ending mutation: ok ($Name)"
}

function Add-BytesToFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][byte[]]$Suffix
    )

    $original = [IO.File]::ReadAllBytes($Path)
    $combined = [byte[]]::new($original.Length + $Suffix.Length)
    [Array]::Copy($original, 0, $combined, 0, $original.Length)
    [Array]::Copy($Suffix, 0, $combined, $original.Length, $Suffix.Length)
    [IO.File]::WriteAllBytes($Path, $combined)
}

function Set-RawIndexBlob {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][byte[]]$Bytes
    )

    $rawPath = Join-Path $Root ".git/line-ending-raw-index"
    [IO.File]::WriteAllBytes($rawPath, $Bytes)
    $hash = [string](Invoke-GitIn -Root $Root -Arguments @("hash-object", "-w", "--no-filters", "--", $rawPath) | Select-Object -Last 1)
    Invoke-GitIn -Root $Root -Arguments @("update-index", "--cacheinfo", "100644,$($hash.Trim()),$Path") | Out-Null
}

function Get-NormalizedSha256 {
    param([Parameter(Mandatory = $true)][string]$Path)

    $text = $utf8.GetString([IO.File]::ReadAllBytes($Path))
    $bytes = $utf8.GetBytes($text.Replace("`r`n", "`n").Replace("`r", "`n"))
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
}

$systemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
$tempBase = Join-Path $systemTemp ("oxvba-line-endings-" + [Guid]::NewGuid().ToString("N"))
$resolvedTempBase = [IO.Path]::GetFullPath($tempBase)
if (-not $resolvedTempBase.StartsWith($systemTemp, [StringComparison]::OrdinalIgnoreCase)) {
    throw "line-ending test temp root escaped the system temp directory"
}
[void](New-Item -ItemType Directory -Path $tempBase -Force)

try {
    $lfOrigin = New-FixtureRepository -Name "origin-lf" -InputEol LF
    Invoke-Validator -Root $lfOrigin
    $lfCheckout = New-FixtureCheckout -Origin $lfOrigin -Name "checkout-lf-autocrlf-false" -AutoCrlf false
    Invoke-Validator -Root $lfCheckout
    $windowsCheckout = New-FixtureCheckout -Origin $lfOrigin -Name "checkout-lf-autocrlf-true" -AutoCrlf true
    Invoke-Validator -Root $windowsCheckout

    $crlfOrigin = New-FixtureRepository -Name "origin-crlf" -InputEol CRLF
    Invoke-ExpectedFailure -Name "crlf-input-working-tree" -MessagePattern "working-tree EOL is 'crlf'" -Action {
        Invoke-Validator -Root $crlfOrigin
    }
    $crlfSemantic = Get-NormalizedSha256 -Path (Join-Path $crlfOrigin "crates/fixture/golden.snap")
    $crlfCheckout = New-FixtureCheckout -Origin $crlfOrigin -Name "checkout-crlf-input-autocrlf-true" -AutoCrlf true
    Invoke-Validator -Root $crlfCheckout
    $checkoutSemantic = Get-NormalizedSha256 -Path (Join-Path $crlfCheckout "crates/fixture/golden.snap")
    if ($crlfSemantic -ne $checkoutSemantic) {
        throw "CRLF input and LF checkout semantic hashes differ"
    }

    $fixture = New-FixtureCheckout -Origin $lfOrigin -Name "mutation-missing-root" -AutoCrlf false
    Remove-Item -LiteralPath (Join-Path $fixture ".gitattributes") -Force
    Invoke-ExpectedFailure -Name "missing-root-attributes" -MessagePattern "root \.gitattributes is missing" -Action {
        Invoke-Validator -Root $fixture
    }

    $fixture = New-FixtureCheckout -Origin $lfOrigin -Name "mutation-working-attributes" -AutoCrlf false
    Add-BytesToFile -Path (Join-Path $fixture ".gitattributes") -Suffix ($utf8.GetBytes("# mutable`n"))
    Invoke-ExpectedFailure -Name "working-attributes-mutation" -MessagePattern "working-tree \.gitattributes does not match" -Action {
        Invoke-Validator -Root $fixture
    }

    $fixture = New-FixtureCheckout -Origin $lfOrigin -Name "mutation-index-attributes" -AutoCrlf false
    $mutatedAttributes = [IO.File]::ReadAllBytes((Join-Path $fixture ".gitattributes")) + $utf8.GetBytes("# staged mutation`n")
    Set-RawIndexBlob -Root $fixture -Path ".gitattributes" -Bytes $mutatedAttributes
    Invoke-ExpectedFailure -Name "index-attributes-mutation" -MessagePattern "index \.gitattributes does not match" -Action {
        Invoke-Validator -Root $fixture
    }

    $fixture = New-FixtureCheckout -Origin $lfOrigin -Name "mutation-conflicting-attributes" -AutoCrlf false
    Add-BytesToFile -Path (Join-Path $fixture ".gitattributes") -Suffix ($utf8.GetBytes("*.snap text eol=crlf`n"))
    Invoke-GitIn -Root $fixture -Arguments @("add", "--", ".gitattributes") | Out-Null
    Invoke-ExpectedFailure -Name "conflicting-attributes" -MessagePattern "byte-exact V1 contract" -Action {
        Invoke-Validator -Root $fixture
    }

    $fixture = New-FixtureCheckout -Origin $lfOrigin -Name "mutation-nested-attributes" -AutoCrlf false
    Write-Utf8Fixture -Path (Join-Path $fixture "docs/.gitattributes") -Eol LF -Text "# nested attributes are forbidden`n"
    Invoke-GitIn -Root $fixture -Arguments @("add", "--", "docs/.gitattributes") | Out-Null
    Invoke-ExpectedFailure -Name "nested-attributes" -MessagePattern "nested or duplicate \.gitattributes" -Action {
        Invoke-Validator -Root $fixture
    }

    $fixture = New-FixtureCheckout -Origin $lfOrigin -Name "mutation-working-crlf" -AutoCrlf false
    $snapshotPath = Join-Path $fixture "crates/fixture/golden.snap"
    $snapshotText = $utf8.GetString([IO.File]::ReadAllBytes($snapshotPath))
    [IO.File]::WriteAllBytes($snapshotPath, $utf8.GetBytes($snapshotText.Replace("`n", "`r`n")))
    Invoke-ExpectedFailure -Name "working-tree-crlf" -MessagePattern "working-tree EOL is 'crlf'" -Action {
        Invoke-Validator -Root $fixture
    }

    $fixture = New-FixtureCheckout -Origin $lfOrigin -Name "mutation-index-crlf" -AutoCrlf false
    $snapshotText = $utf8.GetString([IO.File]::ReadAllBytes((Join-Path $fixture "crates/fixture/golden.snap")))
    Set-RawIndexBlob -Root $fixture -Path "crates/fixture/golden.snap" -Bytes ($utf8.GetBytes($snapshotText.Replace("`n", "`r`n")))
    Invoke-ExpectedFailure -Name "index-crlf" -MessagePattern "index EOL is 'crlf'" -Action {
        Invoke-Validator -Root $fixture
    }

    $fixture = New-FixtureCheckout -Origin $lfOrigin -Name "mutation-forced-text-nul" -AutoCrlf false
    $snapshotPath = Join-Path $fixture "crates/fixture/golden.snap"
    $nulSnapshot = [IO.File]::ReadAllBytes($snapshotPath) + [byte[]](0, 10)
    [IO.File]::WriteAllBytes($snapshotPath, $nulSnapshot)
    Set-RawIndexBlob -Root $fixture -Path "crates/fixture/golden.snap" -Bytes $nulSnapshot
    Invoke-ExpectedFailure -Name "forced-text-nul" -MessagePattern "contains a NUL byte" -Action {
        Invoke-Validator -Root $fixture
    }

    Write-Host "line-ending tests: ok (positive=5 mutations=8 semantic_sha256=$checkoutSemantic)"
}
finally {
    if (Test-Path -LiteralPath $tempBase -PathType Container) {
        $resolvedCleanup = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $tempBase).Path)
        if (-not $resolvedCleanup.StartsWith($systemTemp, [StringComparison]::OrdinalIgnoreCase) -or
            -not ([IO.Path]::GetFileName($resolvedCleanup)).StartsWith("oxvba-line-endings-", [StringComparison]::Ordinal)) {
            throw "refusing unsafe line-ending test cleanup: $resolvedCleanup"
        }
        Remove-Item -LiteralPath $resolvedCleanup -Recurse -Force
    }
}
