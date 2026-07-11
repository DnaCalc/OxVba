param(
    [string]$RepositoryRoot = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$utf8 = [Text.UTF8Encoding]::new($false, $true)
$canonicalAttributeLines = @(
    "# OxVba repository line-ending contract V1.",
    "#",
    "# Git-detected text is stored and checked out as LF on every platform. The",
    "# explicit families below make source, snapshots, documentation, evidence, and",
    "# control files fail closed instead of relying on content auto-detection.",
    "* text=auto eol=lf",
    "",
    "# Rust, VBA, tooling, and project source.",
    "*.rs text eol=lf",
    "*.toml text eol=lf",
    "*.bas text eol=lf",
    "*.cls text eol=lf",
    "*.frm text eol=lf",
    "*.vbp text eol=lf",
    "*.basproj text eol=lf",
    "*.cs text eol=lf",
    "*.csproj text eol=lf",
    "*.c text eol=lf",
    "*.idl text eol=lf",
    "*.lean text eol=lf",
    "*.js text eol=lf",
    "*.sh text eol=lf",
    "*.ps1 text eol=lf",
    "*.psm1 text eol=lf",
    "*.psd1 text eol=lf",
    "",
    "# Authoritative snapshots, documentation, evidence, and configuration.",
    "*.snap text eol=lf",
    "*.md text eol=lf",
    "*.txt text eol=lf",
    "*.log text eol=lf",
    "*.csv text eol=lf",
    "*.psv text eol=lf",
    "*.json text eol=lf",
    "*.jsonl text eol=lf",
    "*.yml text eol=lf",
    "*.yaml text eol=lf",
    "*.html text eol=lf",
    "*.reg text eol=lf",
    ".gitignore text eol=lf",
    ".gitattributes text eol=lf",
    ".git-blame-ignore-revs text eol=lf",
    "formal/lean/lean-toolchain text eol=lf",
    "",
    "# Tracked product, Office, COM, database, and native fixture artifacts.",
    "*.accdb binary",
    "*.dll binary",
    "*.oxb binary",
    "*.tlb binary",
    "*.xlam binary",
    "*.xlsm binary",
    "",
    "# Captured UTF-16/terminal evidence is opaque transport, despite its suffix.",
    "docs/evidence/conformance/com/COM_LANE_L2E_LOG_OxVba.TestEventServer_20260309T000005Z.txt binary"
)
$canonicalAttributeBytes = $utf8.GetBytes(($canonicalAttributeLines -join "`n") + "`n")

function Invoke-GitBytes {
    param(
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [switch]$AllowFailure
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = "git"
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $Arguments) {
        [void]$startInfo.ArgumentList.Add($argument)
    }

    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "line-endings: could not start git"
    }
    $stdout = [IO.MemoryStream]::new()
    $stdoutCopy = $process.StandardOutput.BaseStream.CopyToAsync($stdout)
    $stderrRead = $process.StandardError.ReadToEndAsync()
    [void]$process.WaitForExit()
    [void]$stdoutCopy.GetAwaiter().GetResult()
    $stderr = $stderrRead.GetAwaiter().GetResult().Trim()
    $exitCode = $process.ExitCode
    [void]$process.Dispose()
    $stdoutBytes = $stdout.ToArray()
    [void]$stdout.Dispose()

    if ($exitCode -ne 0 -and -not $AllowFailure) {
        throw "line-endings: git $($Arguments -join ' ') failed with exit $exitCode$(if ($stderr) { ": $stderr" })"
    }
    return [pscustomobject]@{
        Bytes = $stdoutBytes
        ExitCode = $exitCode
        Stderr = $stderr
    }
}

function Assert-BytesEqual {
    param(
        [Parameter(Mandatory = $true)][byte[]]$Actual,
        [Parameter(Mandatory = $true)][byte[]]$Expected,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ($Actual.Length -ne $Expected.Length) {
        throw "line-endings: $Owner does not match the byte-exact V1 contract"
    }
    for ($index = 0; $index -lt $Expected.Length; $index++) {
        if ($Actual[$index] -ne $Expected[$index]) {
            throw "line-endings: $Owner does not match the byte-exact V1 contract"
        }
    }
}

$requestedRoot = if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    Join-Path $PSScriptRoot ".."
}
else {
    $RepositoryRoot
}
if (-not (Test-Path -LiteralPath $requestedRoot -PathType Container)) {
    throw "line-endings: repository root does not exist: $requestedRoot"
}
$requestedRoot = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $requestedRoot).Path)
$topLevelResult = Invoke-GitBytes -WorkingDirectory $requestedRoot -Arguments @("rev-parse", "--show-toplevel")
$gitTopLevel = $utf8.GetString([byte[]]$topLevelResult.Bytes).TrimEnd("`r", "`n")
$repoRoot = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $gitTopLevel).Path)
$pathComparison = if ($IsWindows) { [StringComparison]::OrdinalIgnoreCase } else { [StringComparison]::Ordinal }
if (-not $repoRoot.Equals($requestedRoot, $pathComparison)) {
    throw "line-endings: RepositoryRoot must be the Git worktree root ($repoRoot)"
}

$attributesPath = Join-Path $repoRoot ".gitattributes"
if (-not (Test-Path -LiteralPath $attributesPath -PathType Leaf)) {
    throw "line-endings: root .gitattributes is missing"
}
if (([IO.File]::GetAttributes($attributesPath) -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw "line-endings: root .gitattributes must be a regular file"
}
Assert-BytesEqual -Actual ([IO.File]::ReadAllBytes($attributesPath)) -Expected $canonicalAttributeBytes -Owner "working-tree .gitattributes"

$attributeStage = Invoke-GitBytes -WorkingDirectory $repoRoot -Arguments @("ls-files", "--stage", "--", ".gitattributes")
$attributeStageText = $utf8.GetString([byte[]]$attributeStage.Bytes).TrimEnd("`r", "`n")
if ($attributeStageText -notmatch '^100644 [0-9a-f]+ 0\t\.gitattributes$') {
    throw "line-endings: root .gitattributes must be tracked in the index as a regular file"
}
$indexAttributes = Invoke-GitBytes -WorkingDirectory $repoRoot -Arguments @("show", ":.gitattributes")
Assert-BytesEqual -Actual ([byte[]]$indexAttributes.Bytes) -Expected $canonicalAttributeBytes -Owner "index .gitattributes"

$eolResult = Invoke-GitBytes -WorkingDirectory $repoRoot -Arguments @(
    "-c", "core.quotepath=false", "ls-files", "--eol", "-z"
)
$eolText = $utf8.GetString([byte[]]$eolResult.Bytes)
$records = @($eolText.Split([char]0, [StringSplitOptions]::RemoveEmptyEntries))
if ($records.Count -eq 0) {
    throw "line-endings: repository has no tracked files"
}

$sourceExtensions = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($extension in @(".rs", ".toml", ".bas", ".cls", ".frm", ".vbp", ".basproj", ".cs", ".csproj", ".c", ".idl", ".lean", ".js", ".sh", ".ps1", ".psm1", ".psd1")) {
    [void]$sourceExtensions.Add($extension)
}
$documentExtensions = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($extension in @(".md", ".txt", ".log", ".csv", ".psv", ".json", ".jsonl", ".yml", ".yaml", ".html", ".reg")) {
    [void]$documentExtensions.Add($extension)
}
$binaryExtensions = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($extension in @(".accdb", ".dll", ".oxb", ".tlb", ".xlam", ".xlsm")) {
    [void]$binaryExtensions.Add($extension)
}
$forcedNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($name in @(".gitignore", ".gitattributes", ".git-blame-ignore-revs", "formal/lean/lean-toolchain")) {
    [void]$forcedNames.Add($name)
}
$binaryNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
[void]$binaryNames.Add("docs/evidence/conformance/com/COM_LANE_L2E_LOG_OxVba.TestEventServer_20260309T000005Z.txt")

$attributePaths = [Collections.Generic.List[string]]::new()
$sourceCount = 0
$snapshotCount = 0
$documentCount = 0
$textCount = 0
$binaryCount = 0
$emptyCount = 0

foreach ($record in $records) {
    $tab = $record.IndexOf("`t", [StringComparison]::Ordinal)
    if ($tab -lt 0) {
        throw "line-endings: malformed git ls-files --eol record"
    }
    $metadata = $record.Substring(0, $tab)
    $path = $record.Substring($tab + 1)
    if ($metadata -notmatch '^i/(?<index>\S+)\s+w/(?<worktree>\S+)\s+attr/(?<attributes>.*)$') {
        throw "line-endings: malformed EOL metadata for '$path': $metadata"
    }
    $indexEol = $Matches.index
    $worktreeEol = $Matches.worktree
    $attributes = $Matches.attributes.Trim()
    $extension = [IO.Path]::GetExtension($path)
    $isSource = $sourceExtensions.Contains($extension)
    $isSnapshot = $extension.Equals(".snap", [StringComparison]::OrdinalIgnoreCase)
    $isDocument = $documentExtensions.Contains($extension)
    $isBinaryFamily = $binaryExtensions.Contains($extension) -or $binaryNames.Contains($path)
    $isForcedText = $isSource -or $isSnapshot -or $isDocument -or $forcedNames.Contains($path)

    if ($path -match '(^|/)\.gitattributes$') {
        [void]$attributePaths.Add($path)
    }
    if (-not $isBinaryFamily) {
        if ($isSource) { $sourceCount++ }
        if ($isSnapshot) { $snapshotCount++ }
        if ($isDocument) { $documentCount++ }
    }

    if ($isBinaryFamily) {
        if ($indexEol -ne "-text" -or $worktreeEol -ne "-text" -or $attributes -ne "-text") {
            throw "line-endings: binary artifact '$path' must be -text in index, worktree, and attributes"
        }
        $binaryCount++
        continue
    }

    if ($isForcedText -and $indexEol -eq "-text" -and $worktreeEol -eq "-text") {
        if ($attributes -ne "text eol=lf") {
            throw "line-endings: '$path' effective attributes must be 'text eol=lf', found '$attributes'"
        }
        $workingBytes = [IO.File]::ReadAllBytes((Join-Path $repoRoot $path))
        $indexBytes = Invoke-GitBytes -WorkingDirectory $repoRoot -Arguments @("show", ":$path")
        if ([Array]::IndexOf[byte]($workingBytes, [byte]13) -ge 0 -or
            [Array]::IndexOf[byte]([byte[]]$indexBytes.Bytes, [byte]13) -ge 0) {
            throw "line-endings: '$path' contains a carriage-return byte under the LF-only contract"
        }
        if ([Array]::IndexOf[byte]($workingBytes, [byte]0) -ge 0 -or
            [Array]::IndexOf[byte]([byte[]]$indexBytes.Bytes, [byte]0) -ge 0) {
            throw "line-endings: '$path' contains a NUL byte; declare an exact binary exception"
        }
        $textCount++
        continue
    }
    if ($indexEol -eq "-text" -and $worktreeEol -eq "-text" -and $attributes -eq "text=auto eol=lf") {
        $binaryCount++
        continue
    }
    if ($indexEol -notin @("lf", "none")) {
        throw "line-endings: '$path' index EOL is '$indexEol', expected lf (or none for an empty file)"
    }
    if ($worktreeEol -notin @("lf", "none")) {
        throw "line-endings: '$path' working-tree EOL is '$worktreeEol', expected lf (or none for an empty file)"
    }
    if ($indexEol -ne $worktreeEol) {
        throw "line-endings: '$path' index/worktree EOL states differ ($indexEol/$worktreeEol)"
    }
    if ($attributes -notmatch '(^|\s)(?:text|text=auto)(\s|$)' -or $attributes -notmatch '(^|\s)eol=lf(\s|$)') {
        throw "line-endings: '$path' is not governed by an LF text attribute"
    }
    if ($isForcedText -and $attributes -ne "text eol=lf") {
        throw "line-endings: '$path' effective attributes must be 'text eol=lf', found '$attributes'"
    }
    if ($isForcedText -and $worktreeEol -ne "none") {
        $workingBytes = [IO.File]::ReadAllBytes((Join-Path $repoRoot $path))
        if ([Array]::IndexOf[byte]($workingBytes, [byte]13) -ge 0) {
            throw "line-endings: '$path' contains a carriage-return byte under the LF-only contract"
        }
    }
    if ($indexEol -eq "none") { $emptyCount++ } else { $textCount++ }
}

if ($attributePaths.Count -ne 1 -or $attributePaths[0] -ne ".gitattributes") {
    $found = if ($attributePaths.Count -eq 0) { "none" } else { $attributePaths -join ", " }
    throw "line-endings: nested or duplicate .gitattributes are forbidden; found: $found"
}
if ($sourceCount -eq 0 -or $snapshotCount -eq 0 -or $documentCount -eq 0) {
    throw "line-endings: source, snapshot, and document policy classes must each have tracked witnesses"
}

Write-Host "line-endings: ok (contract=V1 tracked=$($records.Count) text=$textCount empty=$emptyCount binary=$binaryCount source=$sourceCount snapshots=$snapshotCount documents=$documentCount)"
