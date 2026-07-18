param(
    [string]$ManifestPath = "ci/core-profile/gates-v1.json",
    [ValidateSet("ValidateManifest", "NoArtifacts")]
    [string]$Mode = "ValidateManifest",
    [switch]$List,
    [switch]$DryRun,
    [string]$RepositoryRoot = "",
    [string]$RunId = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$utf8 = [Text.UTF8Encoding]::new($false, $true)

function Get-Sha256Hex {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][byte[]]$Bytes)

    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Bytes)).ToLowerInvariant()
}

function Get-CanonicalTextBytes {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [AllowNull()][byte[]]$Bytes = $null
    )

    [byte[]]$sourceBytes = if ($null -eq $Bytes) { [IO.File]::ReadAllBytes($Path) } else { $Bytes }
    $text = $utf8.GetString($sourceBytes).Replace("`r`n", "`n")
    if ($text.Contains("`r", [StringComparison]::Ordinal)) {
        throw "core-profile-gates: controlled text contains a bare carriage return: $Path"
    }
    return $utf8.GetBytes($text)
}

function Assert-NoDuplicateJsonProperties {
    param(
        [Parameter(Mandatory = $true)][Text.Json.JsonElement]$Element,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    switch ($Element.ValueKind) {
        ([Text.Json.JsonValueKind]::Object) {
            $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
            foreach ($property in $Element.EnumerateObject()) {
                if (-not $seen.Add($property.Name)) {
                    throw "core-profile-gates: duplicate JSON property '$($property.Name)' in $Owner"
                }
                Assert-NoDuplicateJsonProperties -Element $property.Value -Owner "$Owner.$($property.Name)"
            }
        }
        ([Text.Json.JsonValueKind]::Array) {
            $index = 0
            foreach ($item in $Element.EnumerateArray()) {
                Assert-NoDuplicateJsonProperties -Element $item -Owner "$Owner[$index]"
                $index++
            }
        }
    }
}

function Read-StrictJson {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner,
        [AllowNull()][byte[]]$Bytes = $null
    )

    [byte[]]$sourceBytes = if ($null -eq $Bytes) { [IO.File]::ReadAllBytes($Path) } else { $Bytes }
    [void]$utf8.GetString($sourceBytes)
    $options = [Text.Json.JsonDocumentOptions]::new()
    $options.AllowTrailingCommas = $false
    $options.CommentHandling = [Text.Json.JsonCommentHandling]::Disallow
    $stream = [IO.MemoryStream]::new($sourceBytes, $false)
    try {
        $document = [Text.Json.JsonDocument]::Parse($stream, $options)
    }
    catch {
        throw "core-profile-gates: $Owner is not strict JSON: $($_.Exception.Message)"
    }
    finally {
        $stream.Dispose()
    }
    try {
        Assert-NoDuplicateJsonProperties -Element $document.RootElement -Owner $Owner
    }
    finally {
        $document.Dispose()
    }
    try {
        return $utf8.GetString($sourceBytes) | ConvertFrom-Json -Depth 100
    }
    catch {
        throw "core-profile-gates: $Owner cannot be decoded: $($_.Exception.Message)"
    }
}

function Assert-ManifestJsonArrayShapes {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [AllowNull()][byte[]]$Bytes = $null
    )

    [byte[]]$sourceBytes = if ($null -eq $Bytes) { [IO.File]::ReadAllBytes($Path) } else { $Bytes }
    $options = [Text.Json.JsonDocumentOptions]::new()
    $options.AllowTrailingCommas = $false
    $options.CommentHandling = [Text.Json.JsonCommentHandling]::Disallow
    $document = [Text.Json.JsonDocument]::Parse([ReadOnlyMemory[byte]]$sourceBytes, $options)
    try {
        $root = $document.RootElement
        foreach ($propertyName in @("supported_platforms", "gates")) {
            try {
                $property = $root.GetProperty($propertyName)
            }
            catch {
                throw "core-profile-gates: manifest.$propertyName must be a present JSON array"
            }
            if ($property.ValueKind -ne [Text.Json.JsonValueKind]::Array) {
                throw "core-profile-gates: manifest.$propertyName must be a JSON array"
            }
        }
        $gateIndex = 0
        foreach ($gate in $root.GetProperty("gates").EnumerateArray()) {
            foreach ($propertyName in @("platforms", "arguments", "environment")) {
                try {
                    $property = $gate.GetProperty($propertyName)
                }
                catch {
                    throw "core-profile-gates: manifest.gates[$gateIndex].$propertyName must be a present JSON array"
                }
                if ($property.ValueKind -ne [Text.Json.JsonValueKind]::Array) {
                    throw "core-profile-gates: manifest.gates[$gateIndex].$propertyName must be a JSON array"
                }
            }
            $gateIndex++
        }
        try {
            $ambientProperty = $root.GetProperty("supervision").GetProperty("ambient_descendant_names")
        }
        catch {
            throw "core-profile-gates: manifest.supervision.ambient_descendant_names must be a present JSON array"
        }
        if ($ambientProperty.ValueKind -ne [Text.Json.JsonValueKind]::Array) {
            throw "core-profile-gates: manifest.supervision.ambient_descendant_names must be a JSON array"
        }
    }
    finally {
        $document.Dispose()
    }
}

function Assert-ExactKeys {
    param(
        [Parameter(Mandatory = $true)]$Object,
        [Parameter(Mandatory = $true)][string[]]$Expected,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ($null -eq $Object) {
        throw "core-profile-gates: $Owner must be an object"
    }
    $actual = @($Object.PSObject.Properties.Name)
    $expectedSet = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($name in $Expected) { [void]$expectedSet.Add($name) }
    $actualSet = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($name in $actual) { [void]$actualSet.Add($name) }
    if ($actual.Count -ne $Expected.Count -or $actualSet.Count -ne $Expected.Count) {
        throw "core-profile-gates: $Owner properties must be exactly [$($Expected -join ', ')]"
    }
    foreach ($name in $actual) {
        if (-not $expectedSet.Contains($name)) {
            throw "core-profile-gates: $Owner has unexpected or mis-cased property '$name'"
        }
    }
    foreach ($name in $Expected) {
        if (-not $actualSet.Contains($name)) {
            throw "core-profile-gates: $Owner is missing exact property '$name'"
        }
    }
}

function Assert-ExactString {
    param(
        [AllowNull()]$Actual,
        [Parameter(Mandatory = $true)][string]$Expected,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ($Actual -isnot [string] -or [string]$Actual -cne $Expected) {
        throw "core-profile-gates: $Owner must be '$Expected', found '$Actual'"
    }
}

function Assert-SafeScalarString {
    param(
        [AllowNull()]$Actual,
        [Parameter(Mandatory = $true)][string]$Owner,
        [int]$MaximumLength = 512,
        [switch]$AllowEmpty
    )

    if ($Actual -isnot [string]) {
        throw "core-profile-gates: $Owner must be a string"
    }
    $value = [string]$Actual
    if ((-not $AllowEmpty -and [string]::IsNullOrWhiteSpace($value)) -or
        $value.Length -gt $MaximumLength -or $value.IndexOf([char]0) -ge 0 -or
        $value.Contains("`r", [StringComparison]::Ordinal) -or
        $value.Contains("`n", [StringComparison]::Ordinal)) {
        throw "core-profile-gates: $Owner is not a bounded single-line string"
    }
}

function Resolve-RepoRelativePath {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    Assert-SafeScalarString -Actual $Path -Owner $Owner
    $normalized = $Path.Replace('\', '/')
    if ([IO.Path]::IsPathRooted($Path) -or $normalized.StartsWith("./", [StringComparison]::Ordinal) -or
        $normalized -match '(^|/)\.\.(/|$)' -or $normalized.Contains('//', [StringComparison]::Ordinal)) {
        throw "core-profile-gates: $Owner must be a normalized repository-relative path, found '$Path'"
    }
    $resolved = [IO.Path]::GetFullPath((Join-Path $Root $Path))
    $rootPrefix = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    $comparison = if ($IsWindows) { [StringComparison]::OrdinalIgnoreCase } else { [StringComparison]::Ordinal }
    if (-not $resolved.StartsWith($rootPrefix, $comparison)) {
        throw "core-profile-gates: $Owner escapes the repository: '$Path'"
    }
    return $resolved
}

function Assert-NoReparseAncestor {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Target
    )

    $relative = [IO.Path]::GetRelativePath($Root, $Target)
    $current = [IO.Path]::GetFullPath($Root)
    foreach ($segment in $relative.Split([IO.Path]::DirectorySeparatorChar, [StringSplitOptions]::RemoveEmptyEntries)) {
        $current = Join-Path $current $segment
        if (Test-Path -LiteralPath $current) {
            $item = Get-Item -LiteralPath $current -Force
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "core-profile-gates: no-artifact evidence path traverses a reparse point: $current"
            }
        }
    }
}

function Assert-NoReparsePath {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $resolvedRoot = [IO.Path]::GetFullPath($Root)
    $resolvedTarget = [IO.Path]::GetFullPath($Target)
    $rootAnchor = [IO.Path]::GetPathRoot($resolvedRoot)
    $currentAncestor = $rootAnchor
    $rootRemainder = $resolvedRoot.Substring($rootAnchor.Length)
    foreach ($segment in $rootRemainder.Split(@([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar), [StringSplitOptions]::RemoveEmptyEntries)) {
        $currentAncestor = Join-Path $currentAncestor $segment
        if ((Get-Item -LiteralPath $currentAncestor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "core-profile-gates: $Owner traverses a reparse/symlink repository ancestor: $currentAncestor"
        }
    }
    $relative = [IO.Path]::GetRelativePath($resolvedRoot, $resolvedTarget)
    $current = $resolvedRoot
    foreach ($segment in $relative.Split([IO.Path]::DirectorySeparatorChar, [StringSplitOptions]::RemoveEmptyEntries)) {
        $current = Join-Path $current $segment
        if (Test-Path -LiteralPath $current) {
            if (((Get-Item -LiteralPath $current -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "core-profile-gates: $Owner traverses a reparse/symlink path: $current"
            }
        }
    }
}

function Assert-EvidencePathConfined {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $root = [IO.Path]::GetFullPath($EvidenceRoot)
    $path = [IO.Path]::GetFullPath($Target)
    $comparison = if ($IsWindows) { [StringComparison]::OrdinalIgnoreCase } else { [StringComparison]::Ordinal }
    $prefix = $root.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if (-not $path.Equals($root, $comparison) -and -not $path.StartsWith($prefix, $comparison)) {
        throw "core-profile-gates: $Owner escapes the held evidence root: $path"
    }

    # Re-evaluate the complete repository -> evidence-root chain and the
    # evidence-root -> target chain at every evidence boundary. A gate may
    # rename and replace either the root or a descendant after launch; no
    # subsequent read or write is allowed to follow that replacement.
    Assert-NoReparsePath -Root $RepoRoot -Target $root -Owner "$Owner evidence root"
    Assert-NoReparsePath -Root $root -Target $path -Owner $Owner
}

function New-EvidenceDirectory {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    Assert-EvidencePathConfined -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Target $Path -Owner $Owner
    [void](New-Item -ItemType Directory -Path $Path)
    Assert-EvidencePathConfined -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Target $Path -Owner $Owner
}

function Write-EvidenceBytes {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][byte[]]$Bytes,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    Assert-EvidencePathConfined -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Target $Path -Owner $Owner
    [IO.File]::WriteAllBytes($Path, $Bytes)
    Assert-EvidencePathConfined -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Target $Path -Owner $Owner
}

function Write-EvidenceText {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    Assert-EvidencePathConfined -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Target $Path -Owner $Owner
    [IO.File]::WriteAllText($Path, $Text, $utf8)
    Assert-EvidencePathConfined -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Target $Path -Owner $Owner
}

function Read-EvidenceBytes {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    Assert-EvidencePathConfined -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Target $Path -Owner $Owner
    [byte[]]$bytes = [IO.File]::ReadAllBytes($Path)
    Assert-EvidencePathConfined -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Target $Path -Owner $Owner
    return ,$bytes
}

function Test-EvidencePath {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    Assert-EvidencePathConfined -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Target $Path -Owner $Owner
    return Test-Path -LiteralPath $Path
}

function Remove-EvidenceFile {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    Assert-EvidencePathConfined -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Target $Path -Owner $Owner
    if (Test-Path -LiteralPath $Path) { Remove-Item -LiteralPath $Path -Force }
    Assert-EvidencePathConfined -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Target $Path -Owner $Owner
}

function Get-EvidenceSha256 {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    return Get-Sha256Hex -Bytes (Read-EvidenceBytes -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $Path -Owner $Owner)
}

function Get-ArchitectureIdentity {
    $osArchitecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
    $processArchitecture = [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString().ToLowerInvariant()
    $is64BitProcess = [Environment]::Is64BitProcess
    $injected = [Environment]::GetEnvironmentVariable("OXVBA_CORE_GATE_TEST_FORCE_PROCESS_ARCH")
    if (-not [string]::IsNullOrWhiteSpace($injected)) {
        if ($injected -cne "x86") {
            throw "core-profile-gates: unsupported architecture-test injection '$injected'"
        }
        $processArchitecture = "x86"
        $is64BitProcess = $false
    }
    if ($osArchitecture -cne "x64" -or $processArchitecture -cne "x64" -or -not $is64BitProcess) {
        throw "core-profile-gates: execution requires OSArchitecture=x64, ProcessArchitecture=x64 and Is64BitProcess=true; found os=$osArchitecture process=$processArchitecture is64=$is64BitProcess"
    }
    $platform = if ($IsWindows) { "windows-x64" } elseif ($IsLinux) { "linux-x64" } else { "" }
    if ([string]::IsNullOrEmpty($platform)) {
        throw "core-profile-gates: unsupported operating system; expected Windows x64 or Linux x64"
    }
    return [pscustomobject][ordered]@{
        platform = $platform
        os_architecture = $osArchitecture
        process_architecture = $processArchitecture
        is_64_bit_process = $is64BitProcess
    }
}

function Get-CurrentPlatformId {
    $identity = Get-ArchitectureIdentity
    if ($null -eq $identity) {
        throw "core-profile-gates: architecture identity unavailable"
    }
    return [string]$identity.platform
}

function Get-RawFileSha256 {
    param([Parameter(Mandatory = $true)][string]$Path)

    return Get-Sha256Hex -Bytes ([IO.File]::ReadAllBytes($Path))
}

function Get-LinkTargetText {
    param([Parameter(Mandatory = $true)][string]$Path)

    $item = Get-Item -LiteralPath $Path -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0) { return "" }
    return (@($item.Target) | ForEach-Object { [string]$_ }) -join '|'
}

function Assert-ExactFileIdentity {
    param(
        [Parameter(Mandatory = $true)]$Identity,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if (-not (Test-Path -LiteralPath ([string]$Identity.path) -PathType Leaf)) {
        throw "core-profile-gates: $Owner disappeared: $($Identity.path)"
    }
    $hash = Get-RawFileSha256 -Path ([string]$Identity.path)
    $linkTarget = Get-LinkTargetText -Path ([string]$Identity.path)
    if ($hash -cne [string]$Identity.sha256 -or $linkTarget -cne [string]$Identity.link_target) {
        throw "core-profile-gates: $Owner identity changed: $($Identity.path)"
    }
}

function Get-UniqueBoundInputs {
    param([Parameter(Mandatory = $true)][object[]]$Identities)

    $comparison = if ($IsWindows) { [StringComparer]::OrdinalIgnoreCase } else { [StringComparer]::Ordinal }
    $byPath = [Collections.Generic.Dictionary[string, object]]::new($comparison)
    foreach ($identity in $Identities) {
        $candidatePath = if ($null -ne $identity.PSObject.Properties["absolute_path"]) {
            [string]$identity.absolute_path
        }
        else { [string]$identity.path }
        $path = [IO.Path]::GetFullPath($candidatePath)
        $digest = [string]$identity.sha256
        if ([string]::IsNullOrWhiteSpace($path) -or $digest -cnotmatch '^[0-9a-f]{64}$') {
            throw "core-profile-gates: bound input identity is incomplete"
        }
        if ($byPath.ContainsKey($path)) {
            if ([string]$byPath[$path].sha256 -cne $digest) {
                throw "core-profile-gates: bound input has conflicting admitted digests: $path"
            }
            continue
        }
        $byPath.Add($path, [pscustomobject][ordered]@{ path = $path; sha256 = $digest })
    }
    return @($byPath.Values)
}

function Invoke-BoundedCapture {
    param(
        [Parameter(Mandatory = $true)][string]$Executable,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)]$ProbeContext,
        [int]$TimeoutSeconds = 5
    )

    $matchingTools = @($ProbeContext.tools | Where-Object {
            $comparison = if ($IsWindows) { [StringComparison]::OrdinalIgnoreCase } else { [StringComparison]::Ordinal }
            ([string]$_.path).Equals([IO.Path]::GetFullPath($Executable), $comparison)
        })
    if ($matchingTools.Count -ne 1) {
        throw "core-profile-gates: tool probe executable has no unique sealed identity: $Executable"
    }
    Assert-ExactFileIdentity -Identity $matchingTools[0] -Owner "tool probe executable"
    Assert-ExactFileIdentity -Identity $ProbeContext.native_source -Owner "native process supervisor"
    if ($IsLinux) {
        Assert-ExactFileIdentity -Identity (Get-ToolIdentityById -Tools $ProbeContext.tools -Id "setsid") -Owner "Linux setsid tool"
        Assert-ExactFileIdentity -Identity (Get-ToolIdentityById -Tools $ProbeContext.tools -Id "bash") -Owner "Linux Bash tool"
        Assert-ExactFileIdentity -Identity $ProbeContext.linux_supervisor -Owner "Linux process supervisor"
    }

    $ProbeContext.sequence = [int]$ProbeContext.sequence + 1
    $probeName = "probe-{0:D4}" -f [int]$ProbeContext.sequence
    $stdoutPath = Join-Path ([string]$ProbeContext.probe_root) "$probeName.stdout.log"
    $stderrPath = Join-Path ([string]$ProbeContext.probe_root) "$probeName.stderr.log"
    $processShape = [pscustomobject]@{ executable = [IO.Path]::GetFullPath($Executable); arguments = @($Arguments) }
    $environmentComparison = if ($IsWindows) { [StringComparer]::OrdinalIgnoreCase } else { [StringComparer]::Ordinal }
    $environment = [Collections.Generic.Dictionary[string, string]]::new($environmentComparison)
    foreach ($entry in [Environment]::GetEnvironmentVariables().GetEnumerator()) {
        $environment[[string]$entry.Key] = [string]$entry.Value
    }
    $execution = $null
    try {
        $boundCandidates = @($matchingTools[0], $ProbeContext.native_source)
        $setsid = $null
        $bash = $null
        if ($IsLinux) {
            $setsid = Get-ToolIdentityById -Tools $ProbeContext.tools -Id "setsid"
            $bash = Get-ToolIdentityById -Tools $ProbeContext.tools -Id "bash"
            $boundCandidates += @($setsid, $bash, $ProbeContext.linux_supervisor)
        }
        $boundInputs = Get-UniqueBoundInputs -Identities $boundCandidates
        if ($IsWindows) {
            $execution = Invoke-WindowsOwnedProcess -ProcessShape $processShape -WorkingDirectory $WorkingDirectory `
                -EvidenceRoot ([string]$ProbeContext.evidence_root) -Environment $environment `
                -StdoutPath $stdoutPath -StderrPath $stderrPath -TimeoutSeconds $TimeoutSeconds `
                -CleanupReserveMs ([int]$ProbeContext.cleanup_reserve_ms) -BoundInputs $boundInputs `
                -AmbientDescendantNames ([string[]]$ProbeContext.ambient_descendant_names)
        }
        else {
            $execution = Invoke-LinuxOwnedProcess -ProcessShape $processShape -WorkingDirectory $WorkingDirectory `
                -EvidenceRoot ([string]$ProbeContext.evidence_root) -Environment $environment `
                -StdoutPath $stdoutPath -StderrPath $stderrPath -TimeoutSeconds $TimeoutSeconds `
                -CleanupReserveMs ([int]$ProbeContext.cleanup_reserve_ms) -SetsidIdentity $setsid `
                -BashIdentity $bash -SupervisorIdentity $ProbeContext.linux_supervisor -BoundInputs $boundInputs `
                -AmbientDescendantNames ([string[]]$ProbeContext.ambient_descendant_names)
        }
        [byte[]]$stdoutBytes = Read-EvidenceBytes -RepoRoot $WorkingDirectory `
            -EvidenceRoot ([string]$ProbeContext.evidence_root) -Path $stdoutPath -Owner "tool probe stdout"
        [byte[]]$stderrBytes = Read-EvidenceBytes -RepoRoot $WorkingDirectory `
            -EvidenceRoot ([string]$ProbeContext.evidence_root) -Path $stderrPath -Owner "tool probe stderr"
        if ($IsLinux) {
            if (-not [Collections.StructuralComparisons]::StructuralEqualityComparer.Equals($stdoutBytes, [byte[]]$execution.stdout_bytes) -or
                -not [Collections.StructuralComparisons]::StructuralEqualityComparer.Equals($stderrBytes, [byte[]]$execution.stderr_bytes)) {
                throw "core-profile-gates: fd-bound tool-probe output no longer matches its admitted evidence path"
            }
        }
        $stdout = $utf8.GetString($stdoutBytes)
        $stderr = $utf8.GetString($stderrBytes)
        if ([string]$execution.tree_cleanup -cne "complete") {
            throw "tool probe containment cleanup was incomplete ($($execution.reason)): $Executable"
        }
        if ([string]$execution.status -cne "passed" -or $null -eq $execution.exit_code -or [int]$execution.exit_code -ne 0) {
            throw "tool probe failed under owned containment ($($execution.reason)): $Executable $($Arguments -join ' '): $stderr"
        }
        return [pscustomobject]@{ stdout = $stdout; stderr = $stderr; exit_code = [int]$execution.exit_code }
    }
    finally {
        Remove-EvidenceFile -RepoRoot $WorkingDirectory -EvidenceRoot ([string]$ProbeContext.evidence_root) `
            -Path $stdoutPath -Owner "tool probe stdout cleanup"
        Remove-EvidenceFile -RepoRoot $WorkingDirectory -EvidenceRoot ([string]$ProbeContext.evidence_root) `
            -Path $stderrPath -Owner "tool probe stderr cleanup"
    }
}

function Resolve-ExactApplicationIdentity {
    param(
        [Parameter(Mandatory = $true)][string]$Id,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Version,
        [Parameter(Mandatory = $true)][string]$RepoRoot
    )

    $absolute = [IO.Path]::GetFullPath($Path)
    if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) {
        throw "core-profile-gates: required tool '$Id' is unavailable at '$absolute'"
    }
    return [pscustomobject][ordered]@{
        id = $Id
        path = $absolute
        sha256 = Get-RawFileSha256 -Path $absolute
        version = $Version.Trim()
        link_target = Get-LinkTargetText -Path $absolute
    }
}

function Get-ToolCandidates {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$Platform
    )

    $pwshPath = [Environment]::ProcessPath
    if ([string]::IsNullOrWhiteSpace($pwshPath) -or [IO.Path]::GetFileNameWithoutExtension($pwshPath) -cne "pwsh") {
        throw "core-profile-gates: runner must execute under exact PowerShell Core pwsh"
    }
    $gitCommand = Get-Command git -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    $cargoCommand = Get-Command cargo -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $gitCommand) { throw "core-profile-gates: required tool 'git' is unavailable" }
    if ($null -eq $cargoCommand) { throw "core-profile-gates: required tool 'cargo' is unavailable" }
    $candidates = @(
        (Resolve-ExactApplicationIdentity -Id "git" -Path ([string]$gitCommand.Source) -Version "" -RepoRoot $RepoRoot),
        (Resolve-ExactApplicationIdentity -Id "pwsh" -Path $pwshPath -Version $PSVersionTable.PSVersion.ToString() -RepoRoot $RepoRoot),
        (Resolve-ExactApplicationIdentity -Id "cargo" -Path ([string]$cargoCommand.Source) -Version "" -RepoRoot $RepoRoot)
    )
    if ($Platform -ceq "linux-x64") {
        $candidates += Resolve-ExactApplicationIdentity -Id "setsid" -Path ([string]$Manifest.supervision.linux_launcher_path) -Version "" -RepoRoot $RepoRoot
        $candidates += Resolve-ExactApplicationIdentity -Id "bash" -Path ([string]$Manifest.supervision.linux_bash_path) -Version "" -RepoRoot $RepoRoot
    }
    return @($candidates)
}

function Get-ToolIdentities {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$Platform,
        [Parameter(Mandatory = $true)][object[]]$Candidates,
        [Parameter(Mandatory = $true)]$ProbeContext
    )

    $versions = @{
        git = @("--version")
        cargo = @("--version", "--verbose")
    }
    if ($Platform -ceq "linux-x64") {
        $versions.setsid = @("--version")
        $versions.bash = @("--version")
    }
    $identities = @()
    foreach ($candidate in $Candidates) {
        $version = if ([string]$candidate.id -ceq "pwsh") {
            [string]$candidate.version
        }
        else {
            (Invoke-BoundedCapture -Executable ([string]$candidate.path) -Arguments ([string[]]$versions[[string]$candidate.id]) `
                    -WorkingDirectory $RepoRoot -ProbeContext $ProbeContext).stdout.Trim()
        }
        $identities += Resolve-ExactApplicationIdentity -Id ([string]$candidate.id) -Path ([string]$candidate.path) `
            -Version $version -RepoRoot $RepoRoot
    }
    return @($identities)
}

function Assert-ToolIdentities {
    param([Parameter(Mandatory = $true)][object[]]$Tools)

    foreach ($tool in $Tools) { Assert-ExactFileIdentity -Identity $tool -Owner "tool '$($tool.id)'" }
}

function Get-ToolIdentityById {
    param(
        [Parameter(Mandatory = $true)][object[]]$Tools,
        [Parameter(Mandatory = $true)][string]$Id
    )

    $rows = @($Tools | Where-Object { [string]$_.id -ceq $Id })
    if ($rows.Count -ne 1) { throw "core-profile-gates: exact tool identity '$Id' is unavailable" }
    return $rows[0]
}

function Invoke-ExactGit {
    param(
        [Parameter(Mandatory = $true)]$Git,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)]$ProbeContext
    )

    Assert-ExactFileIdentity -Identity $Git -Owner "Git tool"
    return (Invoke-BoundedCapture -Executable ([string]$Git.path) -Arguments $Arguments -WorkingDirectory $RepoRoot `
            -ProbeContext $ProbeContext).stdout.TrimEnd("`r", "`n")
}

function Get-SourceIdentity {
    param(
        [Parameter(Mandatory = $true)]$Git,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string[]]$RequiredTrackedPaths,
        [Parameter(Mandatory = $true)]$ProbeContext
    )

    $revisions = @((Invoke-ExactGit -Git $Git -RepoRoot $RepoRoot `
                -Arguments @("rev-parse", "HEAD", "HEAD^{tree}") -ProbeContext $ProbeContext) -split "`n")
    if ($revisions.Count -ne 2 -or $revisions[0] -cnotmatch '^[0-9a-f]{40,64}$' -or $revisions[1] -cnotmatch '^[0-9a-f]{40,64}$') {
        throw "core-profile-gates: Git did not return exact HEAD/tree identities"
    }
    $head = $revisions[0]
    $tree = $revisions[1]
    $status = Invoke-ExactGit -Git $Git -RepoRoot $RepoRoot -Arguments @("--no-optional-locks", "status", "--porcelain=v1", "--untracked-files=all") -ProbeContext $ProbeContext
    if (-not [string]::IsNullOrEmpty($status)) {
        throw "core-profile-gates: source checkout must be clean with no staged, working or untracked drift"
    }
    [void](Invoke-ExactGit -Git $Git -RepoRoot $RepoRoot `
        -Arguments (@("ls-files", "--error-unmatch", "--") + @($RequiredTrackedPaths)) -ProbeContext $ProbeContext)
    return [pscustomobject][ordered]@{ head = $head; tree = $tree; status = "clean" }
}

function Assert-SourceIdentity {
    param(
        [Parameter(Mandatory = $true)]$Expected,
        [Parameter(Mandatory = $true)]$Git,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string[]]$RequiredTrackedPaths,
        [Parameter(Mandatory = $true)]$ProbeContext
    )

    $actual = Get-SourceIdentity -Git $Git -RepoRoot $RepoRoot -RequiredTrackedPaths $RequiredTrackedPaths -ProbeContext $ProbeContext
    if ([string]$actual.head -cne [string]$Expected.head -or [string]$actual.tree -cne [string]$Expected.tree) {
        throw "core-profile-gates: committed source HEAD/tree changed during execution"
    }
}

function Get-CommandFileIdentities {
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$ManifestPath,
        [Parameter(Mandatory = $true)][string]$RunnerPath
    )

    $relativePaths = @(
        [IO.Path]::GetRelativePath($RepoRoot, $RunnerPath).Replace('\', '/'),
        [IO.Path]::GetRelativePath($RepoRoot, $ManifestPath).Replace('\', '/'),
        [string]$Manifest.supervision.native_source_path,
        [string]$Manifest.supervision.linux_supervisor_path
    )
    foreach ($gate in @($Manifest.gates)) {
        if ([string]$gate.kind -ceq "powershell") { $relativePaths += [string]$gate.command }
    }
    $identities = @()
    foreach ($relativePath in @($relativePaths | Select-Object -Unique)) {
        $absolute = Resolve-RepoRelativePath -Root $RepoRoot -Path $relativePath -Owner "command/source identity"
        if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) {
            throw "core-profile-gates: tracked command/source file is missing: $relativePath"
        }
        Assert-NoReparsePath -Root $RepoRoot -Target $absolute -Owner "command/source '$relativePath'"
        $identities += [pscustomobject][ordered]@{
            path = $relativePath
            absolute_path = $absolute
            sha256 = Get-RawFileSha256 -Path $absolute
            link_target = Get-LinkTargetText -Path $absolute
        }
    }
    return @($identities)
}

function Assert-CommandFileIdentities {
    param([Parameter(Mandatory = $true)][object[]]$Commands)

    foreach ($command in $Commands) {
        Assert-ExactFileIdentity -Identity ([pscustomobject]@{
                path = [string]$command.absolute_path
                sha256 = [string]$command.sha256
                link_target = [string]$command.link_target
            }) -Owner "command/source '$($command.path)'"
    }
}

function Get-CommandFileIdentityByPath {
    param(
        [Parameter(Mandatory = $true)][object[]]$Commands,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $rows = @($Commands | Where-Object { [string]$_.path -ceq $Path })
    if ($rows.Count -ne 1) { throw "core-profile-gates: command identity is unavailable for '$Path'" }
    return $rows[0]
}

function Assert-ExecutionInputs {
    param(
        [Parameter(Mandatory = $true)]$Source,
        [Parameter(Mandatory = $true)]$Git,
        [Parameter(Mandatory = $true)][object[]]$Tools,
        [Parameter(Mandatory = $true)][object[]]$Commands,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string[]]$RequiredTrackedPaths,
        [Parameter(Mandatory = $true)][string]$ManifestPath,
        [Parameter(Mandatory = $true)][string]$ManifestSha256,
        [Parameter(Mandatory = $true)]$ProbeContext
    )

    Assert-SourceIdentity -Expected $Source -Git $Git -RepoRoot $RepoRoot -RequiredTrackedPaths $RequiredTrackedPaths `
        -ProbeContext $ProbeContext
    Assert-ToolIdentities -Tools $Tools
    Assert-CommandFileIdentities -Commands $Commands
    Assert-ManifestUnchanged -Path $ManifestPath -ExpectedSha256 $ManifestSha256
}

function Test-ForbiddenMutationSurface {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Value)

    return $Value -match '(?i)(bless|snapshot[-_ ]*(update|accept)|(?:update|accept)[-_ ]*snapshot|insta[-_ ]*(accept|review)|update_expect)'
}

function Test-HostileInheritedEnvironmentName {
    param([Parameter(Mandatory = $true)][string]$Name)

    return $Name -match '(?i)^(?:OXVBA_BLESS.*|OXVBA_(?:UPDATE_EXPECT|INSTA_UPDATE)|OXVBA_.*(?:SNAPSHOT.*(?:UPDATE|ACCEPT|BLESS)|(?:UPDATE|ACCEPT|BLESS).*SNAPSHOT).*|INSTA_UPDATE|UPDATE_EXPECT|SNAPSHOT_(?:UPDATE|ACCEPT|BLESS)|(?:UPDATE|ACCEPT|BLESS)_SNAPSHOT)$'
}

function Assert-ManifestUnchanged {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ExpectedSha256
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "core-profile-gates: versioned manifest disappeared during execution"
    }
    [void](Read-StrictJson -Path $Path -Owner "live versioned manifest")
    Assert-ManifestJsonArrayShapes -Path $Path
    $actualSha256 = Get-Sha256Hex -Bytes (Get-CanonicalTextBytes -Path $Path)
    if ($actualSha256 -cne $ExpectedSha256) {
        throw "core-profile-gates: versioned manifest changed during execution: expected $ExpectedSha256, found $actualSha256"
    }
}

function Assert-Manifest {
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$Platform
    )

    Assert-ExactKeys $Manifest @(
        "schema_id", "plan_id", "version", "profile", "supported_platforms", "evidence",
        "cargo_lock", "supervision", "gates"
    ) "manifest"
    Assert-ExactString $Manifest.schema_id "oxvba-core-profile-gate-plan-v1" "manifest.schema_id"
    Assert-ExactString $Manifest.plan_id "core-profile-portable-gates-v1" "manifest.plan_id"
    if (($Manifest.version -isnot [int] -and $Manifest.version -isnot [long]) -or [int64]$Manifest.version -ne 1) {
        throw "core-profile-gates: manifest.version must be integer 1"
    }
    Assert-ExactString $Manifest.profile "core" "manifest.profile"

    $supportedPlatforms = @($Manifest.supported_platforms)
    $expectedPlatforms = @("windows-x64", "linux-x64")
    if (($supportedPlatforms -join '|') -cne ($expectedPlatforms -join '|')) {
        throw "core-profile-gates: manifest.supported_platforms must be exactly [windows-x64, linux-x64]"
    }
    if ($supportedPlatforms -cnotcontains $Platform) {
        throw "core-profile-gates: current platform '$Platform' is not supported by the manifest"
    }

    Assert-ExactKeys $Manifest.evidence @(
        "no_artifact_root", "plan_path", "run_manifest_path", "run_manifest_digest_path", "summary_path"
    ) "manifest.evidence"
    Assert-ExactString $Manifest.evidence.no_artifact_root "temp/no-artifacts/core-profile-gates" "manifest.evidence.no_artifact_root"
    Assert-ExactString $Manifest.evidence.plan_path "plan.json" "manifest.evidence.plan_path"
    Assert-ExactString $Manifest.evidence.run_manifest_path "run-manifest.json" "manifest.evidence.run_manifest_path"
    Assert-ExactString $Manifest.evidence.run_manifest_digest_path "run-manifest.sha256" "manifest.evidence.run_manifest_digest_path"
    Assert-ExactString $Manifest.evidence.summary_path "summary.txt" "manifest.evidence.summary_path"

    Assert-ExactKeys $Manifest.cargo_lock @("name_prefix", "acquire_timeout_seconds") "manifest.cargo_lock"
    Assert-ExactString $Manifest.cargo_lock.name_prefix "oxvba-core-profile-cargo-v1" "manifest.cargo_lock.name_prefix"
    if (($Manifest.cargo_lock.acquire_timeout_seconds -isnot [int] -and
            $Manifest.cargo_lock.acquire_timeout_seconds -isnot [long]) -or
        [int64]$Manifest.cargo_lock.acquire_timeout_seconds -lt 1 -or
        [int64]$Manifest.cargo_lock.acquire_timeout_seconds -gt 3600) {
        throw "core-profile-gates: manifest.cargo_lock.acquire_timeout_seconds must be an integer from 1 through 3600"
    }

    Assert-ExactKeys $Manifest.supervision @(
        "cleanup_reserve_ms", "native_source_path", "windows_transport", "linux_launcher_path",
        "linux_bash_path", "linux_supervisor_path", "linux_transport", "ambient_descendant_names"
    ) "manifest.supervision"
    if (($Manifest.supervision.cleanup_reserve_ms -isnot [int] -and
            $Manifest.supervision.cleanup_reserve_ms -isnot [long]) -or
        [int64]$Manifest.supervision.cleanup_reserve_ms -lt 100 -or
        [int64]$Manifest.supervision.cleanup_reserve_ms -gt 2000) {
        throw "core-profile-gates: manifest.supervision.cleanup_reserve_ms must be an integer from 100 through 2000"
    }
    $ambientDescendantNames = @($Manifest.supervision.ambient_descendant_names)
    if ($ambientDescendantNames.Count -gt 16) {
        throw "core-profile-gates: manifest.supervision.ambient_descendant_names must contain at most 16 names"
    }
    foreach ($ambientName in $ambientDescendantNames) {
        if ([string]$ambientName -cnotmatch '^[A-Za-z0-9_.-]+\.[A-Za-z0-9]{2,5}$') {
            throw "core-profile-gates: manifest.supervision.ambient_descendant_names entry '$ambientName' is not a plain executable image name"
        }
    }
    Assert-ExactString $Manifest.supervision.native_source_path "scripts/core-gate-process-supervisor.cs" "manifest.supervision.native_source_path"
    Assert-ExactString $Manifest.supervision.windows_transport "job-object-v3:identity-bound-input-handles;startupinfoex-handle-list;suspended-assign-resume;kill-on-close;owned-file-stdout-stderr" "manifest.supervision.windows_transport"
    Assert-ExactString $Manifest.supervision.linux_launcher_path "/usr/bin/setsid" "manifest.supervision.linux_launcher_path"
    Assert-ExactString $Manifest.supervision.linux_bash_path "/usr/bin/bash" "manifest.supervision.linux_bash_path"
    Assert-ExactString $Manifest.supervision.linux_supervisor_path "scripts/core-gate-linux-supervisor.sh" "manifest.supervision.linux_supervisor_path"
    Assert-ExactString $Manifest.supervision.linux_transport "setsid-fd-posix-spawn-pidfd-subreaper-v6:child-dup2-bound-inputs;no-ambient-parent-inheritance;pinned-glibc-x64-abi;direct-ready;builtin-ack-poll;parent-freeze;pidfd-kill;owned-file-stdout-stderr" "manifest.supervision.linux_transport"
    foreach ($supervisorPath in @($Manifest.supervision.native_source_path, $Manifest.supervision.linux_supervisor_path)) {
        $supervisorAbsolute = Resolve-RepoRelativePath -Root $RepoRoot -Path ([string]$supervisorPath) -Owner "manifest.supervision path"
        if (-not (Test-Path -LiteralPath $supervisorAbsolute -PathType Leaf)) {
            throw "core-profile-gates: manifest supervision source is missing: $supervisorPath"
        }
        Assert-NoReparsePath -Root $RepoRoot -Target $supervisorAbsolute -Owner "manifest supervision source"
    }

    $gates = @($Manifest.gates)
    if ($gates.Count -lt 1 -or $gates.Count -gt 64) {
        throw "core-profile-gates: manifest.gates must contain from 1 through 64 rows"
    }
    $ids = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $evidencePaths = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $coverage = @{
        "windows-x64" = 0
        "linux-x64" = 0
    }
    for ($index = 0; $index -lt $gates.Count; $index++) {
        $gate = $gates[$index]
        $owner = "manifest.gates[$index]"
        Assert-ExactKeys $gate @(
            "order", "id", "description", "platforms", "kind", "command", "arguments",
            "environment", "timeout_seconds", "cargo_workspace", "evidence_path"
        ) $owner
        if (($gate.order -isnot [int] -and $gate.order -isnot [long]) -or
            [int64]$gate.order -ne ($index + 1)) {
            throw "core-profile-gates: $owner.order must be the contiguous integer $($index + 1)"
        }
        Assert-SafeScalarString $gate.id "$owner.id" 64
        if ([string]$gate.id -cnotmatch '^[a-z0-9]+(?:-[a-z0-9]+)*$' -or -not $ids.Add([string]$gate.id)) {
            throw "core-profile-gates: $owner.id must be a unique lowercase kebab-case identity"
        }
        Assert-SafeScalarString $gate.description "$owner.description" 240

        $gatePlatforms = @($gate.platforms)
        if ($gatePlatforms.Count -lt 1 -or $gatePlatforms.Count -gt 2) {
            throw "core-profile-gates: $owner.platforms must contain one or two explicit platform identities"
        }
        $gatePlatformSet = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($gatePlatform in $gatePlatforms) {
            if ($gatePlatform -isnot [string] -or $supportedPlatforms -cnotcontains [string]$gatePlatform -or
                -not $gatePlatformSet.Add([string]$gatePlatform)) {
                throw "core-profile-gates: $owner.platforms contains an unknown or duplicate platform '$gatePlatform'"
            }
            $coverage[[string]$gatePlatform]++
        }

        Assert-SafeScalarString $gate.kind "$owner.kind" 16
        if ([string]$gate.kind -cnotin @("powershell", "cargo")) {
            throw "core-profile-gates: $owner.kind must be powershell or cargo"
        }
        Assert-SafeScalarString $gate.command "$owner.command" 240
        if ([string]$gate.kind -ceq "powershell") {
            $commandText = [string]$gate.command
            if (-not $commandText.StartsWith("scripts/", [StringComparison]::Ordinal) -or
                -not $commandText.EndsWith(".ps1", [StringComparison]::Ordinal)) {
                throw "core-profile-gates: $owner.command must be a repository scripts/*.ps1 path"
            }
            $commandAbs = Resolve-RepoRelativePath -Root $RepoRoot -Path $commandText -Owner "$owner.command"
            if (-not (Test-Path -LiteralPath $commandAbs -PathType Leaf)) {
                throw "core-profile-gates: $owner.command is missing: $commandText"
            }
            Assert-NoReparsePath -Root $RepoRoot -Target $commandAbs -Owner "$owner.command"
        }
        elseif ([string]$gate.command -cne "cargo") {
            throw "core-profile-gates: $owner.command must be exact executable 'cargo' for a cargo gate"
        }

        $arguments = @($gate.arguments)
        if ($arguments.Count -gt 32) {
            throw "core-profile-gates: $owner.arguments exceeds 32 entries"
        }
        for ($argumentIndex = 0; $argumentIndex -lt $arguments.Count; $argumentIndex++) {
            Assert-SafeScalarString $arguments[$argumentIndex] "$owner.arguments[$argumentIndex]" 240 -AllowEmpty
        }

        $environment = @($gate.environment)
        if ($environment.Count -gt 16) {
            throw "core-profile-gates: $owner.environment exceeds 16 entries"
        }
        $environmentNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        for ($environmentIndex = 0; $environmentIndex -lt $environment.Count; $environmentIndex++) {
            $entry = $environment[$environmentIndex]
            $environmentOwner = "$owner.environment[$environmentIndex]"
            Assert-ExactKeys $entry @("name", "action", "value") $environmentOwner
            Assert-SafeScalarString $entry.name "$environmentOwner.name" 80
            Assert-SafeScalarString $entry.action "$environmentOwner.action" 16
            Assert-SafeScalarString $entry.value "$environmentOwner.value" 512 -AllowEmpty
            if ([string]$entry.name -cnotmatch '^(RUST_TEST_THREADS|RUST_BACKTRACE|OXVBA_CORE_GATE_TEST_[A-Z0-9_]+)$' -or
                -not $environmentNames.Add([string]$entry.name)) {
                throw "core-profile-gates: $environmentOwner.name is not an allowed unique gate variable"
            }
            if ([string]$entry.action -cnotin @("set", "remove")) {
                throw "core-profile-gates: $environmentOwner.action must be set or remove"
            }
            if ([string]$entry.action -ceq "set" -and [string]::IsNullOrEmpty([string]$entry.value)) {
                throw "core-profile-gates: $environmentOwner.value must be nonempty for action=set"
            }
            if ([string]$entry.action -ceq "remove" -and -not [string]::IsNullOrEmpty([string]$entry.value)) {
                throw "core-profile-gates: $environmentOwner.value must be empty for action=remove"
            }
        }

        if (($gate.timeout_seconds -isnot [int] -and $gate.timeout_seconds -isnot [long]) -or
            [int64]$gate.timeout_seconds -lt 1 -or [int64]$gate.timeout_seconds -gt 14400) {
            throw "core-profile-gates: $owner.timeout_seconds must be an integer from 1 through 14400"
        }
        if ($gate.cargo_workspace -isnot [bool]) {
            throw "core-profile-gates: $owner.cargo_workspace must be a JSON boolean"
        }
        if ([string]$gate.kind -ceq "cargo" -and -not [bool]$gate.cargo_workspace) {
            throw "core-profile-gates: $owner must mark every cargo command as cargo_workspace=true"
        }

        Assert-SafeScalarString $gate.evidence_path "$owner.evidence_path" 160
        $expectedEvidence = "commands/{0:D3}-{1}" -f [int]$gate.order, [string]$gate.id
        if ([string]$gate.evidence_path -cne $expectedEvidence -or
            -not $evidencePaths.Add([string]$gate.evidence_path)) {
            throw "core-profile-gates: $owner.evidence_path must be the unique exact path '$expectedEvidence'"
        }

        $surface = @([string]$gate.command) + @($arguments | ForEach-Object { [string]$_ }) +
            @($environment | ForEach-Object { [string]$_.value })
        foreach ($value in $surface) {
            if (Test-ForbiddenMutationSurface -Value $value) {
                throw "core-profile-gates: $owner contains a forbidden snapshot mutation surface"
            }
        }
    }
    foreach ($supportedPlatform in $supportedPlatforms) {
        if ([int]$coverage[[string]$supportedPlatform] -eq 0) {
            throw "core-profile-gates: manifest has no explicit gate lane for $supportedPlatform"
        }
    }
}

function Convert-ArrayToCompactJson {
    param([AllowEmptyCollection()][object[]]$Value)

    return ConvertTo-Json -InputObject ([object[]]@($Value)) -Depth 8 -Compress
}

function Get-CommandDisplay {
    param([Parameter(Mandatory = $true)]$Gate)

    $argumentsJson = Convert-ArrayToCompactJson -Value @($Gate.arguments)
    $environmentRows = @($Gate.environment | ForEach-Object {
            if ([string]$_.action -ceq "remove") { "-$([string]$_.name)" }
            else { "$([string]$_.name)=$([string]$_.value)" }
        })
    $environmentJson = Convert-ArrayToCompactJson -Value $environmentRows
    return "kind=$($Gate.kind)|command=$($Gate.command)|arguments=$argumentsJson|environment=$environmentJson"
}

function Write-DeterministicPlan {
    param(
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$Platform
    )

    Write-Output "core-profile-gates|plan=$($Manifest.plan_id)|version=$($Manifest.version)|profile=$($Manifest.profile)|platform=$Platform|execute=false|gates=$(@($Manifest.gates).Count)"
    foreach ($gate in @($Manifest.gates)) {
        $selected = @($gate.platforms) -ccontains $Platform
        $disposition = if ($selected) { "run" } else { "not-applicable" }
        $reason = if ($selected) { "selected:$Platform" } else { "platform:$Platform" }
        $platforms = @($gate.platforms) -join ','
        $cargo = ([bool]$gate.cargo_workspace).ToString().ToLowerInvariant()
        $commandDisplay = Get-CommandDisplay -Gate $gate
        Write-Output (
            "{0:D3}|{1}|{2}|platforms={3}|timeout_seconds={4}|cargo_workspace={5}|evidence={6}/<run-id>/{7}|{8}|reason={9}" -f
            [int]$gate.order, $disposition, [string]$gate.id, $platforms,
            [int]$gate.timeout_seconds, $cargo, [string]$Manifest.evidence.no_artifact_root,
            [string]$gate.evidence_path, $commandDisplay, $reason
        )
    }
}

function Write-JsonUtf8 {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    Write-EvidenceText -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $Path `
        -Text (Get-StableJsonText -Value $Value) -Owner $Owner
}

function Get-StableJsonText {
    param([Parameter(Mandatory = $true)]$Value)

    return (ConvertTo-Json -InputObject $Value -Depth 60) + "`n"
}

function Assert-ExactTextFile {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Expected,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if (-not (Test-EvidencePath -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $Path -Owner $Owner)) {
        throw "core-profile-gates: $Owner is missing: $Path"
    }
    [byte[]]$actual = Read-EvidenceBytes -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $Path -Owner $Owner
    [byte[]]$expected = $utf8.GetBytes($Expected)
    if (-not [Collections.StructuralComparisons]::StructuralEqualityComparer.Equals($actual, $expected)) {
        throw "core-profile-gates: $Owner bytes differ from immutable in-memory evidence"
    }
}

function Get-CargoMutexName {
    param(
        [Parameter(Mandatory = $true)][string]$Prefix,
        [Parameter(Mandatory = $true)][string]$RepoRoot
    )

    $identityRoot = [IO.Path]::GetFullPath($RepoRoot).Replace('\', '/')
    if ($IsWindows) { $identityRoot = $identityRoot.ToLowerInvariant() }
    $digest = Get-Sha256Hex -Bytes ($utf8.GetBytes("$Prefix|$identityRoot"))
    return "$Prefix-$($digest.Substring(0, 32))"
}

function Resolve-GateProcess {
    param(
        [Parameter(Mandatory = $true)]$Gate,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][object[]]$Tools
    )

    if ([string]$Gate.kind -ceq "powershell") {
        $pwsh = Get-ToolIdentityById -Tools $Tools -Id "pwsh"
        $scriptPath = Resolve-RepoRelativePath -Root $RepoRoot -Path ([string]$Gate.command) -Owner "gate $($Gate.id) command"
        $arguments = @("-NoLogo", "-NoProfile", "-NonInteractive", "-File", $scriptPath) +
            @($Gate.arguments | ForEach-Object { [string]$_ })
        return [pscustomobject]@{ executable = [string]$pwsh.path; arguments = $arguments; tool_id = "pwsh" }
    }
    $cargo = Get-ToolIdentityById -Tools $Tools -Id "cargo"
    return [pscustomobject]@{
        executable = [string]$cargo.path
        arguments = @($Gate.arguments | ForEach-Object { [string]$_ })
        tool_id = "cargo"
    }
}

function New-ChildEnvironment {
    param(
        [Parameter(Mandatory = $true)]$Gate,
        [Parameter(Mandatory = $true)][object[]]$Tools,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)][string]$PlanPath,
        [Parameter(Mandatory = $true)][string]$PlanSha256,
        [Parameter(Mandatory = $true)][string]$ManifestSha256,
        [Parameter(Mandatory = $true)][string]$ManifestPath,
        [Parameter(Mandatory = $true)][string]$RunId
    )

    $comparison = if ($IsWindows) { [StringComparer]::OrdinalIgnoreCase } else { [StringComparer]::Ordinal }
    $environment = [Collections.Generic.Dictionary[string, string]]::new($comparison)
    foreach ($entry in [Environment]::GetEnvironmentVariables().GetEnumerator()) {
        $name = [string]$entry.Key
        if ($name -cmatch '^OXVBA_CORE_GATE_' -or (Test-HostileInheritedEnvironmentName -Name $name)) { continue }
        $environment[$name] = [string]$entry.Value
    }
    foreach ($environmentEntry in @($Gate.environment)) {
        if ([string]$environmentEntry.action -ceq "remove") {
            [void]$environment.Remove([string]$environmentEntry.name)
        }
        else {
            $environment[[string]$environmentEntry.name] = [string]$environmentEntry.value
        }
    }
    $toolDirectories = @($Tools | ForEach-Object { Split-Path -Parent ([string]$_.path) } | Select-Object -Unique)
    $inheritedPath = if ($environment.ContainsKey("PATH")) { [string]$environment["PATH"] } else { "" }
    $pathSeparator = [IO.Path]::PathSeparator
    $environment["PATH"] = (@($toolDirectories) + @($inheritedPath -split [regex]::Escape([string]$pathSeparator) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Unique)) -join $pathSeparator
    $environment["OXVBA_CORE_GATE_RUN_ID"] = $RunId
    $environment["OXVBA_CORE_GATE_ID"] = [string]$Gate.id
    $environment["OXVBA_CORE_GATE_EVIDENCE_ROOT"] = $EvidenceRoot
    $environment["OXVBA_CORE_GATE_PLAN_PATH"] = $PlanPath
    $environment["OXVBA_CORE_GATE_PLAN_SHA256"] = $PlanSha256
    $environment["OXVBA_CORE_GATE_MANIFEST_SHA256"] = $ManifestSha256
    $environment["OXVBA_CORE_GATE_MANIFEST_PATH"] = $ManifestPath
    $environment["OXVBA_CORE_GATE_PWSH_PATH"] = [string](Get-ToolIdentityById -Tools $Tools -Id "pwsh").path
    return $environment
}

# Bounded window allowed for ordinary process-tree teardown after the direct
# child exits (rustup-proxy/toolchain chains exit out of order; job accounting
# of the final children can lag tens to hundreds of milliseconds on a loaded
# host). A descendant that survives beyond this window is judged by identity:
# a residual set made up only of manifest-declared ambient toolchain helpers
# (for example MSVC vctip.exe and its console host) is recorded and terminated,
# while any other surviving descendant fails closed with
# descendant-processes-remained-after-direct-exit.
$script:DescendantDrainMs = 3000

function Test-AllAmbientDescendants {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Residuals,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$AmbientNames
    )

    # Residuals are "pid:image" entries (Windows full image path, Linux comm).
    # Every residual image leaf must equal a manifest-declared ambient name.
    if ($Residuals.Count -eq 0 -or $AmbientNames.Count -eq 0) { return $false }
    foreach ($entry in $Residuals) {
        $text = [string]$entry
        $separator = $text.IndexOf(':')
        if ($separator -lt 0) { return $false }
        $image = $text.Substring($separator + 1)
        if ([string]::IsNullOrEmpty($image)) { return $false }
        $leaf = if ($image.Contains('\')) { $image.Split('\')[-1] } else { $image }
        $matched = $false
        foreach ($ambient in $AmbientNames) {
            if ([string]::Equals($leaf, [string]$ambient, [StringComparison]::OrdinalIgnoreCase)) { $matched = $true; break }
        }
        if (-not $matched) { return $false }
    }
    return $true
}

function Invoke-WindowsOwnedProcess {
    param(
        [Parameter(Mandatory = $true)]$ProcessShape,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)]$Environment,
        [Parameter(Mandatory = $true)][string]$StdoutPath,
        [Parameter(Mandatory = $true)][string]$StderrPath,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds,
        [Parameter(Mandatory = $true)][int]$CleanupReserveMs,
        [Parameter(Mandatory = $true)][object[]]$BoundInputs,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$AmbientDescendantNames
    )

    $totalMs = $TimeoutSeconds * 1000
    $reserveMs = [Math]::Min($CleanupReserveMs, [Math]::Max(100, [int]($totalMs / 2)))
    $executionCutoffMs = $totalMs - $reserveMs
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $job = $null
    $terminationReason = ""
    $treeCleanup = "complete"
    $exitCode = $null
    $ambientAccepted = @()
    $ownershipRootPid = $null
    try {
        Assert-EvidencePathConfined -RepoRoot $WorkingDirectory -EvidenceRoot $EvidenceRoot -Target $StdoutPath -Owner "Windows gate stdout"
        Assert-EvidencePathConfined -RepoRoot $WorkingDirectory -EvidenceRoot $EvidenceRoot -Target $StderrPath -Owner "Windows gate stderr"
        $job = [OxVbaCoreGateWindowsJob]::Start(
            [string]$ProcessShape.executable,
            [string[]]@($ProcessShape.arguments),
            $WorkingDirectory,
            $Environment,
            $StdoutPath,
            $StderrPath,
            [string[]]@($BoundInputs | ForEach-Object { [string]$_.path }),
            [string[]]@($BoundInputs | ForEach-Object { [string]$_.sha256 }))
        $ownershipRootPid = [int]$job.ProcessId
        $directExitObservedMs = $null
        while ($timer.ElapsedMilliseconds -lt $executionCutoffMs) {
            $directExited = $job.DirectExited
            $active = $job.ActiveProcesses
            if ($directExited -and $active -eq 0) {
                $exitCode = $job.ExitCode
                break
            }
            if ($directExited) {
                if ($null -eq $directExitObservedMs) { $directExitObservedMs = $timer.ElapsedMilliseconds }
                elseif (($timer.ElapsedMilliseconds - [int64]$directExitObservedMs) -ge $script:DescendantDrainMs) {
                    # Ordinary teardown did not drain inside the bounded window.
                    # Accept only manifest-declared ambient toolchain helpers
                    # (recorded and then terminated); anything else fails closed.
                    $residuals = @($job.GetMemberImageNames())
                    if ($residuals.Count -eq 0) { continue }
                    if (Test-AllAmbientDescendants -Residuals $residuals -AmbientNames $AmbientDescendantNames) {
                        $ambientAccepted = $residuals
                        $exitCode = $job.ExitCode
                        $job.Terminate(0)
                        while ($timer.ElapsedMilliseconds -lt $totalMs) {
                            if ($job.ActiveProcesses -eq 0) { break }
                            Start-Sleep -Milliseconds 10
                        }
                        if ($job.ActiveProcesses -ne 0) { $treeCleanup = "kill-on-close-forced-at-deadline" }
                        break
                    }
                    $exitCode = $job.ExitCode
                    $terminationReason = "descendant-processes-remained-after-direct-exit"
                    break
                }
            }
            Start-Sleep -Milliseconds 10
        }
        if ($null -eq $exitCode -and [string]::IsNullOrEmpty($terminationReason)) {
            $terminationReason = "total-deadline-exceeded"
        }
        if (-not [string]::IsNullOrEmpty($terminationReason)) {
            $job.Terminate(124)
            while ($timer.ElapsedMilliseconds -lt $totalMs) {
                if ($job.DirectExited -and $job.ActiveProcesses -eq 0) { break }
                Start-Sleep -Milliseconds 10
            }
            if (-not ($job.DirectExited -and $job.ActiveProcesses -eq 0)) {
                $treeCleanup = "kill-on-close-forced-at-deadline"
            }
        }
        elseif ($job.ActiveProcesses -ne 0 -and $ambientAccepted.Count -eq 0) {
            $terminationReason = "owned-process-tree-not-empty"
            $job.Terminate(125)
        }
        if ($job.TestSentinelWasSignaled) {
            $terminationReason = "ambient-inheritable-handle-leaked"
        }
    }
    finally {
        if ($null -ne $job) { $job.Dispose() }
        $timer.Stop()
    }
    $status = if (-not [string]::IsNullOrEmpty($terminationReason)) {
        if ($terminationReason -ceq "total-deadline-exceeded") { "timeout" } else { "failed" }
    }
    elseif ($exitCode -eq 0) { "passed" } else { "failed" }
    $reason = if (-not [string]::IsNullOrEmpty($terminationReason)) { $terminationReason } elseif ($exitCode -eq 0) { "completed" } else { "command exited with code $exitCode" }
    return [pscustomobject]@{
        status = $status
        reason = $reason
        exit_code = $exitCode
        duration_ms = [int64]$timer.ElapsedMilliseconds
        tree_cleanup = $treeCleanup
        transport = "job-object-v3:identity-bound-input-handles;startupinfoex-handle-list;suspended-assign-resume;kill-on-close;owned-file-stdout-stderr"
        containment = "windows-job-object-v2"
        libc_identity = "not-applicable"
        input_binding = "windows-retained-file-and-ancestor-handles"
        bound_input_count = @($BoundInputs).Count
        supervisor_ready = $true
        ownership_root_pid = $ownershipRootPid
        ownership_root_start_ticks = $null
        escaped_descendants_observed = $false
        ambient_descendants = @($ambientAccepted)
        total_deadline_ms = $totalMs
    }
}

function Invoke-LinuxOwnedProcess {
    param(
        [Parameter(Mandatory = $true)]$ProcessShape,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)]$Environment,
        [Parameter(Mandatory = $true)][string]$StdoutPath,
        [Parameter(Mandatory = $true)][string]$StderrPath,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds,
        [Parameter(Mandatory = $true)][int]$CleanupReserveMs,
        [Parameter(Mandatory = $true)]$SetsidIdentity,
        [Parameter(Mandatory = $true)]$BashIdentity,
        [Parameter(Mandatory = $true)]$SupervisorIdentity,
        [Parameter(Mandatory = $true)][object[]]$BoundInputs,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$AmbientDescendantNames
    )

    Write-EvidenceBytes -RepoRoot $WorkingDirectory -EvidenceRoot $EvidenceRoot -Path $StdoutPath `
        -Bytes ([byte[]]::new(0)) -Owner "Linux gate stdout initialization"
    Write-EvidenceBytes -RepoRoot $WorkingDirectory -EvidenceRoot $EvidenceRoot -Path $StderrPath `
        -Bytes ([byte[]]::new(0)) -Owner "Linux gate stderr initialization"
    $gateRoot = Split-Path -Parent $StdoutPath
    $readyPath = Join-Path $gateRoot "ownership.ready"
    $ackPath = Join-Path $gateRoot "ownership.ack"
    $nonce = [guid]::NewGuid().ToString("N")
    Assert-EvidencePathConfined -RepoRoot $WorkingDirectory -EvidenceRoot $EvidenceRoot -Target $readyPath -Owner "Linux ownership readiness"
    Assert-EvidencePathConfined -RepoRoot $WorkingDirectory -EvidenceRoot $EvidenceRoot -Target $ackPath -Owner "Linux ownership acknowledgement"
    $totalMs = $TimeoutSeconds * 1000
    $reserveMs = [Math]::Min($CleanupReserveMs, [Math]::Max(100, [int]($totalMs / 2)))
    $executionCutoffMs = $totalMs - $reserveMs
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $process = $null
    $ownedTree = [OxVbaCoreGatePosixOwnedTree]::new()
    $processStarted = $false
    $processGroup = 0
    $terminationReason = ""
    $treeCleanup = "complete"
    $exitCode = $null
    $ambientAccepted = @()
    $supervisorReady = $false
    $ownershipRootPid = $null
    $ownershipRootStartTicks = $null
    $escapedDescendantsObserved = $false
    [byte[]]$capturedStdout = [byte[]]::new(0)
    [byte[]]$capturedStderr = [byte[]]::new(0)
    $libcIdentity = [string][OxVbaCoreGatePosixChild]::RuntimeLibcIdentity
    try {
        if (Test-EvidencePath -RepoRoot $WorkingDirectory -EvidenceRoot $EvidenceRoot -Path $readyPath -Owner "Linux ownership readiness precondition") {
            throw "Linux ownership readiness path existed before fd-bound creation"
        }
        if (Test-EvidencePath -RepoRoot $WorkingDirectory -EvidenceRoot $EvidenceRoot -Path $ackPath -Owner "Linux ownership acknowledgement precondition") {
            throw "Linux ownership acknowledgement path existed before fd-bound creation"
        }
        $process = [OxVbaCoreGatePosixChild]::Start(
            [string]$SetsidIdentity.path,
            [string]$BashIdentity.path,
            [string]$SupervisorIdentity.path,
            [string]$ProcessShape.executable,
            [string[]]@($ProcessShape.arguments),
            $WorkingDirectory,
            $Environment,
            $readyPath,
            $ackPath,
            $nonce,
            $StdoutPath,
            $StderrPath,
            [string[]]@($BoundInputs | ForEach-Object { [string]$_.path }),
            [string[]]@($BoundInputs | ForEach-Object { [string]$_.sha256 }))
        $processStarted = $true
        if (-not ([string]$process.LibcIdentity -ceq $libcIdentity)) {
            throw "Linux pinned glibc identity changed across fd-bound launch"
        }
        $ownershipRootPid = [int]$process.ProcessId
        $ownershipRootStartTicks = [uint64]$ownedTree.ArmRoot($ownershipRootPid)
        $readyDeadlineMs = [Math]::Min(1500, $executionCutoffMs)
        $readyRecord = $null
        while ($timer.ElapsedMilliseconds -lt $readyDeadlineMs) {
            [byte[]]$candidateBytes = $process.ReadReadyBytes()
            if ($candidateBytes.Length -gt 0 -and $candidateBytes[$candidateBytes.Length - 1] -eq 10) {
                    $candidateText = $utf8.GetString($candidateBytes).TrimEnd("`r", "`n")
                    $candidateParts = @($candidateText -split '\|', -1)
                    $candidatePid = 0; $candidateGroup = 0; $candidateSession = 0; [uint64]$candidateTicks = 0
                    if ($candidateParts.Count -eq 5 -and [string]$candidateParts[0] -ceq $nonce -and
                        [int]::TryParse([string]$candidateParts[1], [ref]$candidatePid) -and
                        [int]::TryParse([string]$candidateParts[2], [ref]$candidateGroup) -and
                        [int]::TryParse([string]$candidateParts[3], [ref]$candidateSession) -and
                        [uint64]::TryParse([string]$candidateParts[4], [ref]$candidateTicks)) {
                        $readyRecord = [pscustomobject]@{
                            pid = $candidatePid; process_group = $candidateGroup
                            session = $candidateSession; start_ticks = $candidateTicks
                        }
                        break
                    }
            }
            if ($process.HasExited) { break }
            Start-Sleep -Milliseconds 5
        }
        if ($null -eq $readyRecord) {
            throw "Linux gate supervisor did not publish a complete valid ownership readiness record before execution"
        }

        $publishedRootPid = [int]$readyRecord.pid
        $processGroup = [int]$readyRecord.process_group
        $sessionId = [int]$readyRecord.session
        $publishedStartTicks = [uint64]$readyRecord.start_ticks
        if ($publishedRootPid -ne $ownershipRootPid -or $processGroup -ne $ownershipRootPid -or $sessionId -ne $ownershipRootPid) {
            throw "setsid/Bash supervisor identity did not establish the exact owned process group and session"
        }
        if ($ownershipRootStartTicks -ne $publishedStartTicks) {
            throw "Linux gate supervisor /proc start-time identity changed before containment acknowledgement"
        }
        $process.WriteAcknowledgement($utf8.GetBytes("$nonce`n"))
        $supervisorReady = $true

        $directExitObservedMs = $null
        while ($timer.ElapsedMilliseconds -lt $executionCutoffMs) {
            $directExited = $process.HasExited
            $ownedLive = [int]$ownedTree.LiveProcessCount
            if ($directExited -and $ownedLive -eq 0) {
                $exitCode = [int]$process.ExitCode
                break
            }
            if ($directExited -and $ownedLive -gt 0) {
                if ($null -eq $directExitObservedMs) { $directExitObservedMs = $timer.ElapsedMilliseconds }
                elseif (($timer.ElapsedMilliseconds - [int64]$directExitObservedMs) -ge $script:DescendantDrainMs) {
                    # Ordinary teardown did not drain inside the bounded window.
                    # Accept only manifest-declared ambient toolchain helpers
                    # (recorded and then terminated); anything else fails closed.
                    $residuals = @($ownedTree.GetLiveProcessNames())
                    if ($residuals.Count -eq 0) { continue }
                    if (Test-AllAmbientDescendants -Residuals $residuals -AmbientNames $AmbientDescendantNames) {
                        $ambientAccepted = $residuals
                        $exitCode = [int]$process.ExitCode
                        $ambientCleanupBudget = [Math]::Max(0, $totalMs - [int]$timer.ElapsedMilliseconds)
                        if (-not $ownedTree.TerminateAll($ambientCleanupBudget)) { $treeCleanup = "owned-tree-kill-incomplete-at-deadline" }
                        break
                    }
                    $exitCode = [int]$process.ExitCode
                    $terminationReason = "descendant-processes-remained-after-direct-exit"
                    break
                }
            }
            Start-Sleep -Milliseconds 10
        }
        if ($null -eq $exitCode -and [string]::IsNullOrEmpty($terminationReason)) { $terminationReason = "total-deadline-exceeded" }
        if (-not [string]::IsNullOrEmpty($terminationReason)) {
            $cleanupBudget = [Math]::Max(0, $totalMs - [int]$timer.ElapsedMilliseconds)
            if (-not $ownedTree.TerminateAll($cleanupBudget)) { $treeCleanup = "owned-tree-kill-incomplete-at-deadline" }
        }
        if (-not $process.HasExited -and $timer.ElapsedMilliseconds -lt $totalMs) {
            [void]$process.WaitForExit([Math]::Max(0, $totalMs - [int]$timer.ElapsedMilliseconds))
        }
        if ($process.HasExited) {
            [void]$ownedTree.LiveProcessCount
        }
        if (-not $process.HasExited -or $ownedTree.LiveProcessCount -ne 0 -or $ownedTree.RetainedPidFdCount -ne 0) {
            $treeCleanup = "owned-tree-kill-incomplete-at-deadline"
        }
    }
    finally {
        try {
            if ($processStarted) {
                $cleanupBudget = [Math]::Max(0, $totalMs - [int]$timer.ElapsedMilliseconds)
                if ((-not $process.HasExited -or $ownedTree.LiveProcessCount -gt 0) -and
                    -not $ownedTree.TerminateAll($cleanupBudget)) {
                    $treeCleanup = "owned-tree-kill-incomplete-at-deadline"
                }
                if (-not $process.HasExited -and $timer.ElapsedMilliseconds -lt $totalMs) {
                    [void]$process.WaitForExit([Math]::Max(0, $totalMs - [int]$timer.ElapsedMilliseconds))
                }
                if ($process.HasExited) {
                    [void]$ownedTree.LiveProcessCount
                }
                if (-not $process.HasExited -or $ownedTree.LiveProcessCount -ne 0 -or $ownedTree.RetainedPidFdCount -ne 0) {
                    $treeCleanup = "owned-tree-kill-incomplete-at-deadline"
                }
            }
            $escapedDescendantsObserved = [bool]$ownedTree.EscapedSessionObserved
            if ($null -ne $process) {
                [byte[]]$capturedStdout = $process.ReadStdoutBytes()
                [byte[]]$capturedStderr = $process.ReadStderrBytes()
            }
        }
        finally {
            $ownedTree.Dispose()
            if ($null -ne $process) { $process.Dispose() }
            $timer.Stop()
        }
    }
    $status = if (-not [string]::IsNullOrEmpty($terminationReason)) {
        if ($terminationReason -ceq "total-deadline-exceeded") { "timeout" } else { "failed" }
    }
    elseif ($exitCode -eq 0) { "passed" } else { "failed" }
    $reason = if (-not [string]::IsNullOrEmpty($terminationReason)) { $terminationReason } elseif ($exitCode -eq 0) { "completed" } else { "command exited with code $exitCode" }
    return [pscustomobject]@{
        status = $status
        reason = $reason
        exit_code = $exitCode
        duration_ms = [int64]$timer.ElapsedMilliseconds
        tree_cleanup = $treeCleanup
        transport = "setsid-fd-posix-spawn-pidfd-subreaper-v6:child-dup2-bound-inputs;no-ambient-parent-inheritance;pinned-glibc-x64-abi;direct-ready;builtin-ack-poll;parent-freeze;pidfd-kill;owned-file-stdout-stderr"
        containment = "linux-pidfd-subreaper-v1"
        libc_identity = $libcIdentity
        input_binding = "linux-retained-directory-and-file-descriptors"
        bound_input_count = @($BoundInputs).Count
        stdout_bytes = $capturedStdout
        stderr_bytes = $capturedStderr
        supervisor_ready = $supervisorReady
        ownership_root_pid = $ownershipRootPid
        ownership_root_start_ticks = $ownershipRootStartTicks
        escaped_descendants_observed = $escapedDescendantsObserved
        ambient_descendants = @($ambientAccepted)
        total_deadline_ms = $totalMs
    }
}

function Invoke-GateProcess {
    param(
        [Parameter(Mandatory = $true)]$Gate,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)][string]$PlanPath,
        [Parameter(Mandatory = $true)][string]$PlanSha256,
        [Parameter(Mandatory = $true)][string]$ManifestSha256,
        [Parameter(Mandatory = $true)][string]$ManifestPath,
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)][string]$MutexName,
        [Parameter(Mandatory = $true)][int]$MutexTimeoutSeconds,
        [Parameter(Mandatory = $true)][object[]]$Tools,
        [Parameter(Mandatory = $true)]$Supervision,
        [Parameter(Mandatory = $true)][object[]]$CommandIdentities
    )

    $gateRoot = Resolve-RepoRelativePath -Root $EvidenceRoot -Path ([string]$Gate.evidence_path) -Owner "gate $($Gate.id) evidence_path"
    New-EvidenceDirectory -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $gateRoot -Owner "gate $($Gate.id) evidence directory"
    $stdoutPath = Join-Path $gateRoot "stdout.log"
    $stderrPath = Join-Path $gateRoot "stderr.log"
    $resultPath = Join-Path $gateRoot "result.json"
    $lock = $null
    $lockAcquired = $false
    $lockRecoveredAbandoned = $false
    $lockWait = [Diagnostics.Stopwatch]::StartNew()
    try {
        if ([bool]$Gate.cargo_workspace) {
            $lock = [Threading.Mutex]::new($false, $MutexName)
            try {
                $lockAcquired = $lock.WaitOne($MutexTimeoutSeconds * 1000)
            }
            catch [Threading.AbandonedMutexException] {
                $lockAcquired = $true
                $lockRecoveredAbandoned = $true
            }
            if (-not $lockAcquired) {
                throw "core-profile-gates: timed out acquiring the workspace Cargo lock for gate '$($Gate.id)'"
            }
        }
        $lockWait.Stop()
        $processShape = Resolve-GateProcess -Gate $Gate -RepoRoot $RepoRoot -Tools $Tools
        $executorIdentity = Get-ToolIdentityById -Tools $Tools -Id ([string]$processShape.tool_id)
        $manifestRelative = [IO.Path]::GetRelativePath($RepoRoot, $ManifestPath).Replace('\', '/')
        $manifestIdentity = Get-CommandFileIdentityByPath -Commands $CommandIdentities -Path $manifestRelative
        $boundCandidates = @($executorIdentity, $manifestIdentity)
        if ([string]$Gate.kind -ceq "powershell") {
            $boundCandidates += Get-CommandFileIdentityByPath -Commands $CommandIdentities -Path ([string]$Gate.command)
        }
        $setsid = $null
        $bash = $null
        $supervisorIdentity = $null
        if ($IsLinux) {
            $setsid = Get-ToolIdentityById -Tools $Tools -Id "setsid"
            $bash = Get-ToolIdentityById -Tools $Tools -Id "bash"
            $supervisorIdentity = Get-CommandFileIdentityByPath -Commands $CommandIdentities -Path ([string]$Supervision.linux_supervisor_path)
            $boundCandidates += @($setsid, $bash, $supervisorIdentity)
        }
        $boundInputs = Get-UniqueBoundInputs -Identities $boundCandidates
        $childEnvironment = New-ChildEnvironment -Gate $Gate -Tools $Tools -EvidenceRoot $EvidenceRoot `
            -PlanPath $PlanPath -PlanSha256 $PlanSha256 -ManifestSha256 $ManifestSha256 `
            -ManifestPath $ManifestPath -RunId $RunId
        $start = [DateTimeOffset]::UtcNow
        if ($IsWindows) {
            $execution = Invoke-WindowsOwnedProcess -ProcessShape $processShape -WorkingDirectory $RepoRoot -EvidenceRoot $EvidenceRoot `
                -Environment $childEnvironment -StdoutPath $stdoutPath -StderrPath $stderrPath `
                -TimeoutSeconds ([int]$Gate.timeout_seconds) -CleanupReserveMs ([int]$Supervision.cleanup_reserve_ms) `
                -BoundInputs $boundInputs -AmbientDescendantNames ([string[]]$Supervision.ambient_descendant_names)
        }
        else {
            $execution = Invoke-LinuxOwnedProcess -ProcessShape $processShape -WorkingDirectory $RepoRoot -EvidenceRoot $EvidenceRoot `
                -Environment $childEnvironment -StdoutPath $stdoutPath -StderrPath $stderrPath `
                -TimeoutSeconds ([int]$Gate.timeout_seconds) -CleanupReserveMs ([int]$Supervision.cleanup_reserve_ms) `
                -SetsidIdentity $setsid -BashIdentity $bash -SupervisorIdentity $supervisorIdentity -BoundInputs $boundInputs `
                -AmbientDescendantNames ([string[]]$Supervision.ambient_descendant_names)
        }
        $finish = [DateTimeOffset]::UtcNow
        [byte[]]$stdoutBytes = Read-EvidenceBytes -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $stdoutPath -Owner "gate $($Gate.id) stdout"
        [byte[]]$stderrBytes = Read-EvidenceBytes -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $stderrPath -Owner "gate $($Gate.id) stderr"
        if ($IsLinux) {
            if (-not [Collections.StructuralComparisons]::StructuralEqualityComparer.Equals($stdoutBytes, [byte[]]$execution.stdout_bytes) -or
                -not [Collections.StructuralComparisons]::StructuralEqualityComparer.Equals($stderrBytes, [byte[]]$execution.stderr_bytes)) {
                throw "core-profile-gates: fd-bound gate output no longer matches its admitted evidence path"
            }
        }
        $gateEvidence = [ordered]@{
            order = [int]$Gate.order
            id = [string]$Gate.id
            status = [string]$execution.status
            reason = [string]$execution.reason
            exit_code = $execution.exit_code
            started_utc = $start.ToString("O")
            finished_utc = $finish.ToString("O")
            duration_ms = [int64]$execution.duration_ms
            cargo_lock_wait_ms = if ([bool]$Gate.cargo_workspace) { [int64]$lockWait.ElapsedMilliseconds } else { $null }
            cargo_lock_abandoned_recovered = $lockRecoveredAbandoned
            total_deadline_ms = [int]$execution.total_deadline_ms
            tree_cleanup = [string]$execution.tree_cleanup
            transport = [string]$execution.transport
            containment = [string]$execution.containment
            libc_identity = [string]$execution.libc_identity
            input_binding = [string]$execution.input_binding
            bound_input_count = [int]$execution.bound_input_count
            supervisor_ready = [bool]$execution.supervisor_ready
            ownership_root_pid = $execution.ownership_root_pid
            ownership_root_start_ticks = $execution.ownership_root_start_ticks
            escaped_descendants_observed = [bool]$execution.escaped_descendants_observed
            ambient_descendants = @($execution.ambient_descendants)
            evidence_path = [string]$Gate.evidence_path
            stdout_path = "$($Gate.evidence_path)/stdout.log"
            stderr_path = "$($Gate.evidence_path)/stderr.log"
            result_path = "$($Gate.evidence_path)/result.json"
            stdout_sha256 = Get-Sha256Hex -Bytes $stdoutBytes
            stderr_sha256 = Get-Sha256Hex -Bytes $stderrBytes
        }
        Write-JsonUtf8 -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $resultPath -Value $gateEvidence -Owner "gate $($Gate.id) result"
        $runResult = [ordered]@{}
        foreach ($key in $gateEvidence.Keys) { $runResult[$key] = $gateEvidence[$key] }
        $runResult["result_sha256"] = Get-EvidenceSha256 -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $resultPath -Owner "gate $($Gate.id) result hash"
        return [pscustomobject]$runResult
    }
    finally {
        if ($lockAcquired -and $null -ne $lock) {
            try { $lock.ReleaseMutex() } catch {}
        }
        if ($null -ne $lock) { $lock.Dispose() }
    }
}

function New-NonExecutedResult {
    param(
        [Parameter(Mandatory = $true)]$Gate,
        [Parameter(Mandatory = $true)][string]$Status,
        [Parameter(Mandatory = $true)][string]$Reason
    )

    return [pscustomobject][ordered]@{
        order = [int]$Gate.order
        id = [string]$Gate.id
        status = $Status
        reason = $Reason
        exit_code = $null
        started_utc = $null
        finished_utc = $null
        duration_ms = $null
        cargo_lock_wait_ms = $null
        cargo_lock_abandoned_recovered = $false
        total_deadline_ms = $null
        tree_cleanup = "not-started"
        transport = "none"
        containment = "none"
        libc_identity = "not-started"
        supervisor_ready = $false
        ownership_root_pid = $null
        ownership_root_start_ticks = $null
        escaped_descendants_observed = $false
        ambient_descendants = @()
        evidence_path = [string]$Gate.evidence_path
        stdout_path = ""
        stderr_path = ""
        result_path = ""
        stdout_sha256 = ""
        stderr_sha256 = ""
        result_sha256 = ""
    }
}

function New-RunManifestValue {
    param(
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$ManifestSha256,
        [Parameter(Mandatory = $true)][string]$PlanSha256,
        [Parameter(Mandatory = $true)]$Architecture,
        [Parameter(Mandatory = $true)]$Source,
        [Parameter(Mandatory = $true)][object[]]$Tools,
        [Parameter(Mandatory = $true)][object[]]$Commands,
        [Parameter(Mandatory = $true)]$Supervision,
        [Parameter(Mandatory = $true)][string]$Status,
        [AllowEmptyString()][string]$Failure,
        [Parameter(Mandatory = $true)][string]$StartedUtc,
        [AllowNull()][string]$FinishedUtc,
        [AllowNull()][string]$SummarySha256,
        [AllowEmptyCollection()][object[]]$Results
    )

    return [ordered]@{
        schema_id = "oxvba-core-profile-gate-run-v1"
        run_id = $RunId
        plan_id = [string]$Manifest.plan_id
        manifest_sha256 = $ManifestSha256
        plan_sha256 = $PlanSha256
        platform = [string]$Architecture.platform
        architecture = $Architecture
        source = $Source
        tools = @($Tools)
        commands = @($Commands)
        supervision = $Supervision
        mode = "no-artifacts"
        no_artifacts = $true
        status = $Status
        failure = $Failure
        started_utc = $StartedUtc
        finished_utc = $FinishedUtc
        summary_sha256 = $SummarySha256
        results = @($Results)
    }
}

function Get-SummaryText {
    param(
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)][string]$Platform,
        [Parameter(Mandatory = $true)][string]$Status,
        [AllowEmptyString()][string]$Failure,
        [AllowEmptyCollection()][object[]]$Results
    )

    $lines = @(
        "schema=oxvba-core-profile-gate-summary-v1",
        "run_id=$RunId",
        "platform=$Platform",
        "mode=no-artifacts",
        "status=$Status",
        "failure=$Failure"
    )
    foreach ($result in @($Results)) {
        $lines += ("{0:D3}|{1}|{2}|reason={3}|evidence={4}" -f
            [int]$result.order, [string]$result.status, [string]$result.id,
            ([string]$result.reason).Replace("`r", " ").Replace("`n", " "),
            [string]$result.evidence_path)
    }
    return ($lines -join "`n") + "`n"
}

function Assert-ExecutionEvidence {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$PlanPath,
        [Parameter(Mandatory = $true)][string]$RunManifestPath,
        [Parameter(Mandatory = $true)][string]$RunManifestDigestPath,
        [Parameter(Mandatory = $true)][string]$SummaryPath,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$ManifestSha256,
        [Parameter(Mandatory = $true)][string]$ManifestPath,
        [Parameter(Mandatory = $true)][string]$ExpectedPlanText,
        [Parameter(Mandatory = $true)][string]$ExpectedRunManifestText,
        [Parameter(Mandatory = $true)][string]$ExpectedSummaryText,
        [Parameter(Mandatory = $true)][object[]]$ExpectedResults,
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)][string]$Platform
    )

    Assert-ManifestUnchanged -Path $ManifestPath -ExpectedSha256 $ManifestSha256
    Assert-ExactTextFile -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $PlanPath -Expected $ExpectedPlanText -Owner "execution plan evidence"
    Assert-ExactTextFile -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $RunManifestPath -Expected $ExpectedRunManifestText -Owner "run manifest evidence"
    Assert-ExactTextFile -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $SummaryPath -Expected $ExpectedSummaryText -Owner "summary evidence"
    $runManifestSha256 = Get-EvidenceSha256 -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $RunManifestPath -Owner "run manifest evidence hash"
    $expectedDigestText = "$runManifestSha256  $([string]$Manifest.evidence.run_manifest_path)`n"
    Assert-ExactTextFile -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $RunManifestDigestPath -Expected $expectedDigestText -Owner "run manifest digest evidence"
    if ($ExpectedResults.Count -ne @($Manifest.gates).Count) {
        throw "core-profile-gates: immutable result count differs from the gate plan"
    }
    for ($index = 0; $index -lt $ExpectedResults.Count; $index++) {
        $result = $ExpectedResults[$index]
        $gate = @($Manifest.gates)[$index]
        if ([int64]$result.order -ne [int64]$gate.order -or [string]$result.id -cne [string]$gate.id) {
            throw "core-profile-gates: immutable result identity/order drifted at index $index"
        }
        $selected = @($gate.platforms) -ccontains $Platform
        $expectedContainment = if ($Platform -ceq "windows-x64") { "windows-job-object-v2" } else { "linux-pidfd-subreaper-v1" }
        if ($selected -and ([string]$result.status -cne "passed" -or $null -eq $result.exit_code -or
                [int]$result.exit_code -ne 0 -or [string]$result.tree_cleanup -cne "complete" -or
                -not [bool]$result.supervisor_ready -or $null -eq $result.ownership_root_pid -or
                [string]$result.containment -cne $expectedContainment)) {
            throw "core-profile-gates: selected gate '$($gate.id)' is not an exact passed/exit-0/clean-tree result"
        }
        if (-not $selected -and ([string]$result.status -cne "not-applicable" -or [string]$result.reason -cne "platform:$Platform")) {
            throw "core-profile-gates: nonselected gate '$($gate.id)' is not exact not-applicable evidence"
        }
        if ([string]$result.status -in @("passed", "failed", "timeout")) {
            foreach ($pair in @(
                @($result.stdout_path, $result.stdout_sha256),
                @($result.stderr_path, $result.stderr_sha256),
                @($result.result_path, $result.result_sha256)
            )) {
                $absolutePath = Resolve-RepoRelativePath -Root $EvidenceRoot -Path ([string]$pair[0]) -Owner "run result evidence path"
                if ((Get-EvidenceSha256 -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $absolutePath -Owner "run result evidence hash") -cne [string]$pair[1]) {
                    throw "core-profile-gates: content hash drifted for '$($pair[0])'"
                }
            }
        }
    }
    return $runManifestSha256
}

function Assert-InterimEvidence {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$PlanPath,
        [Parameter(Mandatory = $true)][string]$ExpectedPlanText,
        [Parameter(Mandatory = $true)][string]$RunManifestPath,
        [Parameter(Mandatory = $true)][string]$ExpectedRunManifestText,
        [Parameter(Mandatory = $true)][string]$SummaryPath,
        [Parameter(Mandatory = $true)][string]$RunManifestDigestPath,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [AllowEmptyCollection()][object[]]$Results
    )

    Assert-ExactTextFile -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $PlanPath -Expected $ExpectedPlanText -Owner "interim execution plan"
    Assert-ExactTextFile -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $RunManifestPath -Expected $ExpectedRunManifestText -Owner "interim run manifest"
    foreach ($unexpected in @($SummaryPath, $RunManifestDigestPath)) {
        if (Test-EvidencePath -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $unexpected -Owner "interim terminal evidence absence") {
            throw "core-profile-gates: child created terminal evidence before the runner finalized it: $unexpected"
        }
    }
    foreach ($result in @($Results)) {
        if ([string]$result.status -in @("passed", "failed", "timeout")) {
            foreach ($pair in @(
                @($result.stdout_path, $result.stdout_sha256),
                @($result.stderr_path, $result.stderr_sha256),
                @($result.result_path, $result.result_sha256)
            )) {
                $absolute = Resolve-RepoRelativePath -Root $EvidenceRoot -Path ([string]$pair[0]) -Owner "interim result evidence"
                if ((Get-EvidenceSha256 -RepoRoot $RepoRoot -EvidenceRoot $EvidenceRoot -Path $absolute -Owner "interim result evidence hash") -cne [string]$pair[1]) {
                    throw "core-profile-gates: child changed prior evidence bytes: $($pair[0])"
                }
            }
        }
    }
}

$repoRoot = if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    [IO.Path]::GetFullPath((Resolve-Path (Join-Path $PSScriptRoot "..")).Path)
}
else {
    [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $RepositoryRoot).Path)
}
$architecture = Get-ArchitectureIdentity
$platform = [string]$architecture.platform
$runnerAbs = [IO.Path]::GetFullPath($PSCommandPath)
$runnerRelative = [IO.Path]::GetRelativePath($repoRoot, $runnerAbs).Replace('\', '/')
if ($runnerRelative -eq ".." -or $runnerRelative.StartsWith("../", [StringComparison]::Ordinal)) {
    throw "core-profile-gates: gate runner must be inside the repository root"
}
Assert-NoReparsePath -Root $repoRoot -Target $repoRoot -Owner "repository root"
Assert-NoReparsePath -Root $repoRoot -Target $runnerAbs -Owner "gate runner"
$manifestAbs = Resolve-RepoRelativePath -Root $repoRoot -Path $ManifestPath -Owner "ManifestPath"
if (-not (Test-Path -LiteralPath $manifestAbs -PathType Leaf)) { throw "core-profile-gates: manifest is missing: $ManifestPath" }
Assert-NoReparsePath -Root $repoRoot -Target $manifestAbs -Owner "versioned manifest"
[byte[]]$manifestAdmissionBytes = [IO.File]::ReadAllBytes($manifestAbs)
$manifestAdmissionRawSha256 = Get-Sha256Hex -Bytes $manifestAdmissionBytes
$manifest = Read-StrictJson -Path $manifestAbs -Owner "manifest" -Bytes $manifestAdmissionBytes
Assert-ManifestJsonArrayShapes -Path $manifestAbs -Bytes $manifestAdmissionBytes
Assert-Manifest -Manifest $manifest -RepoRoot $repoRoot -Platform $platform
$manifestSha256 = Get-Sha256Hex -Bytes (Get-CanonicalTextBytes -Path $manifestAbs -Bytes $manifestAdmissionBytes)
Assert-ManifestUnchanged -Path $manifestAbs -ExpectedSha256 $manifestSha256

if ($List -or $DryRun) { Write-DeterministicPlan -Manifest $manifest -Platform $platform; return }
if ($Mode -ceq "ValidateManifest") {
    if (-not [string]::IsNullOrWhiteSpace($RunId)) { throw "core-profile-gates: RunId is only valid for Mode=NoArtifacts" }
    Write-Host "core-profile-gates: manifest ok (plan=$($manifest.plan_id) version=$($manifest.version) platform=$platform sha256=$manifestSha256 gates=$(@($manifest.gates).Count))"
    return
}
if ([string]::IsNullOrWhiteSpace($RunId) -or $RunId -cnotmatch '^[a-z0-9][a-z0-9._-]{0,63}$' -or $RunId -in @(".", "..")) {
    throw "core-profile-gates: Mode=NoArtifacts requires a bounded lowercase RunId"
}

$commandIdentities = Get-CommandFileIdentities -Manifest $manifest -RepoRoot $repoRoot -ManifestPath $manifestAbs -RunnerPath $runnerAbs
$requiredTrackedPaths = @($commandIdentities | ForEach-Object { [string]$_.path })
$nativeSource = Get-CommandFileIdentityByPath -Commands $commandIdentities -Path ([string]$manifest.supervision.native_source_path)
$linuxSupervisor = Get-CommandFileIdentityByPath -Commands $commandIdentities -Path ([string]$manifest.supervision.linux_supervisor_path)
$manifestRelative = [IO.Path]::GetRelativePath($repoRoot, $manifestAbs).Replace('\', '/')
$manifestIdentity = Get-CommandFileIdentityByPath -Commands $commandIdentities -Path $manifestRelative
if ([string]$manifestIdentity.sha256 -cne $manifestAdmissionRawSha256) {
    throw "core-profile-gates: versioned manifest path changed after its admitted bytes were decoded"
}
$toolCandidates = Get-ToolCandidates -RepoRoot $repoRoot -Manifest $manifest -Platform $platform
[byte[]]$nativeSourceAdmissionBytes = [IO.File]::ReadAllBytes([string]$nativeSource.absolute_path)
if ((Get-Sha256Hex -Bytes $nativeSourceAdmissionBytes) -cne [string]$nativeSource.sha256) {
    throw "core-profile-gates: native process supervisor changed before its admitted bytes could be compiled"
}
Add-Type -TypeDefinition ($utf8.GetString($nativeSourceAdmissionBytes))

$evidenceBase = Resolve-RepoRelativePath -Root $repoRoot -Path ([string]$manifest.evidence.no_artifact_root) -Owner "manifest.evidence.no_artifact_root"
$evidenceRoot = [IO.Path]::GetFullPath((Join-Path $evidenceBase $RunId))
$rootPrefix = $evidenceBase.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
$comparison = if ($IsWindows) { [StringComparison]::OrdinalIgnoreCase } else { [StringComparison]::Ordinal }
if (-not $evidenceRoot.StartsWith($rootPrefix, $comparison)) { throw "core-profile-gates: RunId escapes the no-artifact evidence root" }
Assert-NoReparseAncestor -Root $repoRoot -Target $evidenceRoot
if (Test-Path -LiteralPath $evidenceRoot) { throw "core-profile-gates: no-artifact evidence root already exists; refusing stale evidence: $evidenceRoot" }
[void](New-Item -ItemType Directory -Path $evidenceRoot)
Assert-EvidencePathConfined -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot -Target $evidenceRoot -Owner "new no-artifact evidence root"
$probeRoot = Join-Path $evidenceRoot "admission-probes"
New-EvidenceDirectory -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot -Path $probeRoot -Owner "admission probe directory"
$probeContext = [pscustomobject]@{
    evidence_root = $evidenceRoot
    probe_root = $probeRoot
    cleanup_reserve_ms = [int]$manifest.supervision.cleanup_reserve_ms
    ambient_descendant_names = @($manifest.supervision.ambient_descendant_names)
    sequence = 0
    tools = @($toolCandidates)
    native_source = [pscustomobject]@{
        path = [string]$nativeSource.absolute_path; sha256 = [string]$nativeSource.sha256; link_target = [string]$nativeSource.link_target
    }
    linux_supervisor = [pscustomobject]@{
        path = [string]$linuxSupervisor.absolute_path; sha256 = [string]$linuxSupervisor.sha256; link_target = [string]$linuxSupervisor.link_target
    }
}
$tools = Get-ToolIdentities -RepoRoot $repoRoot -Manifest $manifest -Platform $platform `
    -Candidates $toolCandidates -ProbeContext $probeContext
$gitTool = Get-ToolIdentityById -Tools $tools -Id "git"
$sourceIdentity = Get-SourceIdentity -Git $gitTool -RepoRoot $repoRoot -RequiredTrackedPaths $requiredTrackedPaths `
    -ProbeContext $probeContext
Assert-ExecutionInputs -Source $sourceIdentity -Git $gitTool -Tools $tools -Commands $commandIdentities `
    -RepoRoot $repoRoot -RequiredTrackedPaths $requiredTrackedPaths -ManifestPath $manifestAbs `
    -ManifestSha256 $manifestSha256 -ProbeContext $probeContext

$planPath = Join-Path $evidenceRoot ([string]$manifest.evidence.plan_path)
$runManifestPath = Join-Path $evidenceRoot ([string]$manifest.evidence.run_manifest_path)
$runManifestDigestPath = Join-Path $evidenceRoot ([string]$manifest.evidence.run_manifest_digest_path)
$summaryPath = Join-Path $evidenceRoot ([string]$manifest.evidence.summary_path)
$toolEvidence = @($tools | ForEach-Object { [ordered]@{ id = $_.id; path = $_.path; sha256 = $_.sha256; version = $_.version; link_target = $_.link_target } })
$commandEvidence = @($commandIdentities | ForEach-Object { [ordered]@{ path = $_.path; sha256 = $_.sha256 } })
$supervisionEvidence = [ordered]@{
    cleanup_reserve_ms = [int]$manifest.supervision.cleanup_reserve_ms
    ambient_descendant_names = @($manifest.supervision.ambient_descendant_names)
    transport = if ($IsWindows) { [string]$manifest.supervision.windows_transport } else { [string]$manifest.supervision.linux_transport }
    native_source_path = [string]$manifest.supervision.native_source_path
    native_source_sha256 = [string]$nativeSource.sha256
    linux_launcher_path = if ($IsLinux) { [string](Get-ToolIdentityById -Tools $tools -Id "setsid").path } else { "not-applicable" }
    linux_bash_path = if ($IsLinux) { [string](Get-ToolIdentityById -Tools $tools -Id "bash").path } else { "not-applicable" }
    linux_libc_identity = if ($IsLinux) { [string][OxVbaCoreGatePosixChild]::RuntimeLibcIdentity } else { "not-applicable" }
    linux_supervisor_path = [string]$manifest.supervision.linux_supervisor_path
}
$planRows = @()
foreach ($gate in @($manifest.gates)) {
    $selected = @($gate.platforms) -ccontains $platform
    $executorId = if ([string]$gate.kind -ceq "cargo") { "cargo" } else { "pwsh" }
    $executor = Get-ToolIdentityById -Tools $tools -Id $executorId
    $commandSha = if ([string]$gate.kind -ceq "cargo") { [string]$executor.sha256 } else { [string](Get-CommandFileIdentityByPath -Commands $commandIdentities -Path ([string]$gate.command)).sha256 }
    $commandDigestShape = [ordered]@{
        kind = [string]$gate.kind
        command = [string]$gate.command
        command_sha256 = $commandSha
        executor_path = [string]$executor.path
        executor_sha256 = [string]$executor.sha256
        arguments = @($gate.arguments | ForEach-Object { [string]$_ })
        environment = @($gate.environment | ForEach-Object { [ordered]@{ name = $_.name; action = $_.action; value = $_.value } })
    }
    $commandDigest = Get-Sha256Hex -Bytes ($utf8.GetBytes((Get-StableJsonText -Value $commandDigestShape)))
    $planRows += [ordered]@{
        order = [int]$gate.order; id = [string]$gate.id
        disposition = if ($selected) { "run" } else { "not-applicable" }
        reason = if ($selected) { "selected:$platform" } else { "platform:$platform" }
        platforms = @($gate.platforms); kind = [string]$gate.kind; command = [string]$gate.command
        command_sha256 = $commandSha; command_digest = $commandDigest
        executor_path = [string]$executor.path; executor_sha256 = [string]$executor.sha256
        arguments = @($gate.arguments)
        environment = @($gate.environment | ForEach-Object { [ordered]@{ name = $_.name; action = $_.action; value = $_.value } })
        timeout_seconds = [int]$gate.timeout_seconds; cargo_workspace = [bool]$gate.cargo_workspace
        evidence_path = [string]$gate.evidence_path
    }
}
$executionPlan = [ordered]@{
    schema_id = "oxvba-core-profile-execution-plan-v1"; plan_id = [string]$manifest.plan_id
    manifest_sha256 = $manifestSha256; version = [int]$manifest.version; profile = [string]$manifest.profile
    platform = $platform; architecture = $architecture; source = $sourceIdentity; tools = $toolEvidence
    commands_identity = $commandEvidence; supervision = $supervisionEvidence
    mode = "no-artifacts"; run_id = $RunId; evidence_root = "$($manifest.evidence.no_artifact_root)/$RunId"
    commands = $planRows
}
$expectedPlanText = Get-StableJsonText -Value $executionPlan
Write-EvidenceText -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot -Path $planPath -Text $expectedPlanText -Owner "execution plan evidence"
$planSha256 = Get-EvidenceSha256 -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot -Path $planPath -Owner "execution plan evidence hash"
$mutexName = Get-CargoMutexName -Prefix ([string]$manifest.cargo_lock.name_prefix) -RepoRoot $repoRoot
$runStarted = [DateTimeOffset]::UtcNow.ToString("O")
$results = @()
$executionFailure = $null
$initialRunManifest = New-RunManifestValue -RunId $RunId -Manifest $manifest -ManifestSha256 $manifestSha256 `
    -PlanSha256 $planSha256 -Architecture $architecture -Source $sourceIdentity -Tools $toolEvidence `
    -Commands $commandEvidence -Supervision $supervisionEvidence -Status "running" -Failure "" `
    -StartedUtc $runStarted -FinishedUtc $null -SummarySha256 $null -Results $results
$expectedInterimRunManifestText = Get-StableJsonText -Value $initialRunManifest
Write-EvidenceText -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot -Path $runManifestPath `
    -Text $expectedInterimRunManifestText -Owner "interim run manifest evidence"

foreach ($gate in @($manifest.gates)) {
    if ($null -ne $executionFailure) { $results += New-NonExecutedResult -Gate $gate -Status "not-run" -Reason "earlier gate failed"; continue }
    try {
        Assert-ExecutionInputs -Source $sourceIdentity -Git $gitTool -Tools $tools -Commands $commandIdentities `
            -RepoRoot $repoRoot -RequiredTrackedPaths $requiredTrackedPaths -ManifestPath $manifestAbs `
            -ManifestSha256 $manifestSha256 -ProbeContext $probeContext
        Assert-InterimEvidence -RepoRoot $repoRoot -PlanPath $planPath -ExpectedPlanText $expectedPlanText -RunManifestPath $runManifestPath `
            -ExpectedRunManifestText $expectedInterimRunManifestText -SummaryPath $summaryPath `
            -RunManifestDigestPath $runManifestDigestPath -EvidenceRoot $evidenceRoot -Results $results
    }
    catch { $executionFailure = $_.Exception.Message; $results += New-NonExecutedResult -Gate $gate -Status "not-run" -Reason $executionFailure; continue }
    if (@($gate.platforms) -cnotcontains $platform) { $results += New-NonExecutedResult -Gate $gate -Status "not-applicable" -Reason "platform:$platform"; continue }
    $result = $null
    try {
        Write-Host ("[core-profile] {0:D3} {1} (total_deadline={2}s cargo_lock={3})" -f [int]$gate.order, [string]$gate.id, [int]$gate.timeout_seconds, [bool]$gate.cargo_workspace)
        $result = Invoke-GateProcess -Gate $gate -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot `
            -PlanPath $planPath -PlanSha256 $planSha256 -ManifestSha256 $manifestSha256 -RunId $RunId `
            -ManifestPath $manifestAbs -MutexName $mutexName -MutexTimeoutSeconds ([int]$manifest.cargo_lock.acquire_timeout_seconds) `
            -Tools $tools -Supervision $manifest.supervision -CommandIdentities $commandIdentities
        $results += $result
        Assert-ExecutionInputs -Source $sourceIdentity -Git $gitTool -Tools $tools -Commands $commandIdentities `
            -RepoRoot $repoRoot -RequiredTrackedPaths $requiredTrackedPaths -ManifestPath $manifestAbs `
            -ManifestSha256 $manifestSha256 -ProbeContext $probeContext
        Assert-InterimEvidence -RepoRoot $repoRoot -PlanPath $planPath -ExpectedPlanText $expectedPlanText -RunManifestPath $runManifestPath `
            -ExpectedRunManifestText $expectedInterimRunManifestText -SummaryPath $summaryPath `
            -RunManifestDigestPath $runManifestDigestPath -EvidenceRoot $evidenceRoot -Results $results
        if ([string]$result.status -cne "passed") { $executionFailure = "gate '$($gate.id)' $($result.reason)" }
    }
    catch {
        $executionFailure = "gate '$($gate.id)' failed: $($_.Exception.Message)"
        if ($null -eq $result) { $results += New-NonExecutedResult -Gate $gate -Status "not-run" -Reason $executionFailure }
    }
}

$runFinished = [DateTimeOffset]::UtcNow.ToString("O")
$runStatus = if ($null -eq $executionFailure) { "passed" } else { "failed" }
$failureText = if ($null -eq $executionFailure) { "" } else { [string]$executionFailure }
if ($runStatus -ceq "passed") {
    try {
        Assert-ExecutionInputs -Source $sourceIdentity -Git $gitTool -Tools $tools -Commands $commandIdentities `
            -RepoRoot $repoRoot -RequiredTrackedPaths $requiredTrackedPaths -ManifestPath $manifestAbs `
            -ManifestSha256 $manifestSha256 -ProbeContext $probeContext
        Assert-InterimEvidence -RepoRoot $repoRoot -PlanPath $planPath -ExpectedPlanText $expectedPlanText -RunManifestPath $runManifestPath `
            -ExpectedRunManifestText $expectedInterimRunManifestText -SummaryPath $summaryPath `
            -RunManifestDigestPath $runManifestDigestPath -EvidenceRoot $evidenceRoot -Results $results
        for ($index = 0; $index -lt @($manifest.gates).Count; $index++) {
            $gate = @($manifest.gates)[$index]; $result = $results[$index]; $selected = @($gate.platforms) -ccontains $platform
            $expectedContainment = if ($IsWindows) { "windows-job-object-v2" } else { "linux-pidfd-subreaper-v1" }
            if ($selected -and ([string]$result.status -cne "passed" -or [int]$result.exit_code -ne 0 -or
                    [string]$result.tree_cleanup -cne "complete" -or -not [bool]$result.supervisor_ready -or
                    $null -eq $result.ownership_root_pid -or [string]$result.containment -cne $expectedContainment)) {
                throw "selected gate '$($gate.id)' lacks exact success"
            }
            if (-not $selected -and [string]$result.status -cne "not-applicable") { throw "nonselected gate '$($gate.id)' lacks exact not-applicable result" }
        }
    }
    catch { $runStatus = "failed"; $failureText = "terminal success reconstruction failed: $($_.Exception.Message)" }
}
$expectedSummaryText = Get-SummaryText -RunId $RunId -Platform $platform -Status $runStatus -Failure $failureText -Results $results
$summarySha256 = Get-Sha256Hex -Bytes ($utf8.GetBytes($expectedSummaryText))
$finalRunManifest = New-RunManifestValue -RunId $RunId -Manifest $manifest -ManifestSha256 $manifestSha256 `
    -PlanSha256 $planSha256 -Architecture $architecture -Source $sourceIdentity -Tools $toolEvidence `
    -Commands $commandEvidence -Supervision $supervisionEvidence -Status $runStatus -Failure $failureText `
    -StartedUtc $runStarted -FinishedUtc $runFinished -SummarySha256 $summarySha256 -Results $results
$expectedRunManifestText = Get-StableJsonText -Value $finalRunManifest
Write-EvidenceText -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot -Path $summaryPath -Text $expectedSummaryText -Owner "terminal summary evidence"
Write-EvidenceText -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot -Path $runManifestPath -Text $expectedRunManifestText -Owner "terminal run manifest evidence"
$runManifestSha256 = Get-EvidenceSha256 -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot -Path $runManifestPath -Owner "terminal run manifest hash"
Write-EvidenceText -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot -Path $runManifestDigestPath `
    -Text "$runManifestSha256  $($manifest.evidence.run_manifest_path)`n" -Owner "terminal run manifest digest evidence"

if ($runStatus -ceq "passed") {
    try {
        $validatedDigest = Assert-ExecutionEvidence -RepoRoot $repoRoot -PlanPath $planPath -RunManifestPath $runManifestPath `
            -RunManifestDigestPath $runManifestDigestPath -SummaryPath $summaryPath -EvidenceRoot $evidenceRoot `
            -Manifest $manifest -ManifestSha256 $manifestSha256 -ManifestPath $manifestAbs `
            -ExpectedPlanText $expectedPlanText -ExpectedRunManifestText $expectedRunManifestText `
            -ExpectedSummaryText $expectedSummaryText -ExpectedResults $results -RunId $RunId -Platform $platform
        Assert-ExecutionInputs -Source $sourceIdentity -Git $gitTool -Tools $tools -Commands $commandIdentities `
            -RepoRoot $repoRoot -RequiredTrackedPaths $requiredTrackedPaths -ManifestPath $manifestAbs `
            -ManifestSha256 $manifestSha256 -ProbeContext $probeContext
    }
    catch { $runStatus = "failed"; $failureText = "terminal evidence validation failed: $($_.Exception.Message)" }
}
if ($runStatus -cne "passed") {
    $expectedSummaryText = Get-SummaryText -RunId $RunId -Platform $platform -Status $runStatus -Failure $failureText -Results $results
    $summarySha256 = Get-Sha256Hex -Bytes ($utf8.GetBytes($expectedSummaryText))
    $finalRunManifest = New-RunManifestValue -RunId $RunId -Manifest $manifest -ManifestSha256 $manifestSha256 `
        -PlanSha256 $planSha256 -Architecture $architecture -Source $sourceIdentity -Tools $toolEvidence `
        -Commands $commandEvidence -Supervision $supervisionEvidence -Status $runStatus -Failure $failureText `
        -StartedUtc $runStarted -FinishedUtc $runFinished -SummarySha256 $summarySha256 -Results $results
    $expectedRunManifestText = Get-StableJsonText -Value $finalRunManifest
    Write-EvidenceText -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot -Path $summaryPath -Text $expectedSummaryText -Owner "failed terminal summary evidence"
    Write-EvidenceText -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot -Path $runManifestPath -Text $expectedRunManifestText -Owner "failed terminal run manifest evidence"
    $runManifestSha256 = Get-EvidenceSha256 -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot -Path $runManifestPath -Owner "failed terminal run manifest hash"
    Write-EvidenceText -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot -Path $runManifestDigestPath `
        -Text "$runManifestSha256  $($manifest.evidence.run_manifest_path)`n" -Owner "failed terminal run manifest digest evidence"
}
if ($runStatus -cne "passed") { throw "core-profile-gates: $failureText" }
Write-Host "core-profile-gates: ok (run_id=$RunId platform=$platform evidence=$evidenceRoot manifest_sha256=$manifestSha256 plan_sha256=$planSha256 run_manifest_sha256=$validatedDigest)"
