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
    param([Parameter(Mandatory = $true)][byte[]]$Bytes)

    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Bytes)).ToLowerInvariant()
}

function Get-CanonicalTextBytes {
    param([Parameter(Mandatory = $true)][string]$Path)

    $text = $utf8.GetString([IO.File]::ReadAllBytes($Path)).Replace("`r`n", "`n")
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
        [Parameter(Mandatory = $true)][string]$Owner
    )

    [byte[]]$bytes = [IO.File]::ReadAllBytes($Path)
    [void]$utf8.GetString($bytes)
    $options = [Text.Json.JsonDocumentOptions]::new()
    $options.AllowTrailingCommas = $false
    $options.CommentHandling = [Text.Json.JsonCommentHandling]::Disallow
    $stream = [IO.MemoryStream]::new($bytes, $false)
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
        return $utf8.GetString($bytes) | ConvertFrom-Json -Depth 100
    }
    catch {
        throw "core-profile-gates: $Owner cannot be decoded: $($_.Exception.Message)"
    }
}

function Assert-ManifestJsonArrayShapes {
    param([Parameter(Mandatory = $true)][string]$Path)

    [byte[]]$bytes = [IO.File]::ReadAllBytes($Path)
    $options = [Text.Json.JsonDocumentOptions]::new()
    $options.AllowTrailingCommas = $false
    $options.CommentHandling = [Text.Json.JsonCommentHandling]::Disallow
    $document = [Text.Json.JsonDocument]::Parse([ReadOnlyMemory[byte]]$bytes, $options)
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

function Get-CurrentPlatformId {
    if ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne [Runtime.InteropServices.Architecture]::X64) {
        throw "core-profile-gates: only x64 execution is supported"
    }
    if ($IsWindows) { return "windows-x64" }
    if ($IsLinux) { return "linux-x64" }
    throw "core-profile-gates: unsupported operating system; expected Windows x64 or Linux x64"
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
        "cargo_lock", "gates"
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
        "no_artifact_root", "plan_path", "run_manifest_path", "summary_path"
    ) "manifest.evidence"
    Assert-ExactString $Manifest.evidence.no_artifact_root "temp/no-artifacts/core-profile-gates" "manifest.evidence.no_artifact_root"
    Assert-ExactString $Manifest.evidence.plan_path "plan.json" "manifest.evidence.plan_path"
    Assert-ExactString $Manifest.evidence.run_manifest_path "run-manifest.json" "manifest.evidence.run_manifest_path"
    Assert-ExactString $Manifest.evidence.summary_path "summary.txt" "manifest.evidence.summary_path"

    Assert-ExactKeys $Manifest.cargo_lock @("name_prefix", "acquire_timeout_seconds") "manifest.cargo_lock"
    Assert-ExactString $Manifest.cargo_lock.name_prefix "oxvba-core-profile-cargo-v1" "manifest.cargo_lock.name_prefix"
    if (($Manifest.cargo_lock.acquire_timeout_seconds -isnot [int] -and
            $Manifest.cargo_lock.acquire_timeout_seconds -isnot [long]) -or
        [int64]$Manifest.cargo_lock.acquire_timeout_seconds -lt 1 -or
        [int64]$Manifest.cargo_lock.acquire_timeout_seconds -gt 3600) {
        throw "core-profile-gates: manifest.cargo_lock.acquire_timeout_seconds must be an integer from 1 through 3600"
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
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )

    $json = ConvertTo-Json -InputObject $Value -Depth 40
    [IO.File]::WriteAllText($Path, $json + "`n", $utf8)
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
        [Parameter(Mandatory = $true)][string]$RepoRoot
    )

    if ([string]$Gate.kind -ceq "powershell") {
        $pwshPath = [Environment]::ProcessPath
        $pwshName = if ([string]::IsNullOrWhiteSpace($pwshPath)) { "" } else { [IO.Path]::GetFileNameWithoutExtension($pwshPath) }
        if ([string]::IsNullOrWhiteSpace($pwshPath) -or $pwshName -cne "pwsh" -or
            -not (Test-Path -LiteralPath $pwshPath -PathType Leaf)) {
            throw "core-profile-gates: the runner must execute under the PowerShell Core 'pwsh' host"
        }
        $scriptPath = Resolve-RepoRelativePath -Root $RepoRoot -Path ([string]$Gate.command) -Owner "gate $($Gate.id) command"
        $arguments = @("-NoLogo", "-NoProfile", "-NonInteractive", "-File", $scriptPath) +
            @($Gate.arguments | ForEach-Object { [string]$_ })
        return [pscustomobject]@{ executable = $pwshPath; arguments = $arguments }
    }
    $cargo = Get-Command cargo -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $cargo) {
        throw "core-profile-gates: required tool 'cargo' is unavailable"
    }
    return [pscustomobject]@{
        executable = [string]$cargo.Source
        arguments = @($Gate.arguments | ForEach-Object { [string]$_ })
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
        [Parameter(Mandatory = $true)][int]$MutexTimeoutSeconds
    )

    $gateRoot = Resolve-RepoRelativePath -Root $EvidenceRoot -Path ([string]$Gate.evidence_path) -Owner "gate $($Gate.id) evidence_path"
    [void](New-Item -ItemType Directory -Path $gateRoot)
    $stdoutPath = Join-Path $gateRoot "stdout.log"
    $stderrPath = Join-Path $gateRoot "stderr.log"
    $resultPath = Join-Path $gateRoot "result.json"
    $lock = $null
    $lockAcquired = $false
    $process = $null
    $lockWait = [Diagnostics.Stopwatch]::StartNew()
    try {
        if ([bool]$Gate.cargo_workspace) {
            $lock = [Threading.Mutex]::new($false, $MutexName)
            try {
                $lockAcquired = $lock.WaitOne($MutexTimeoutSeconds * 1000)
            }
            catch [Threading.AbandonedMutexException] {
                $lockAcquired = $true
            }
            if (-not $lockAcquired) {
                throw "core-profile-gates: timed out acquiring the workspace Cargo lock for gate '$($Gate.id)'"
            }
        }
        $lockWait.Stop()
        $processShape = Resolve-GateProcess -Gate $Gate -RepoRoot $RepoRoot
        $start = [DateTimeOffset]::UtcNow
        $stopwatch = [Diagnostics.Stopwatch]::StartNew()
        $process = [Diagnostics.Process]::new()
        $startInfo = [Diagnostics.ProcessStartInfo]::new()
        $startInfo.FileName = [string]$processShape.executable
        $startInfo.WorkingDirectory = $RepoRoot
        $startInfo.UseShellExecute = $false
        $startInfo.RedirectStandardOutput = $true
        $startInfo.RedirectStandardError = $true
        $startInfo.StandardOutputEncoding = $utf8
        $startInfo.StandardErrorEncoding = $utf8
        foreach ($argument in @($processShape.arguments)) {
            [void]$startInfo.ArgumentList.Add([string]$argument)
        }
        foreach ($environmentName in @($startInfo.Environment.Keys)) {
            if (Test-HostileInheritedEnvironmentName -Name ([string]$environmentName)) {
                [void]$startInfo.Environment.Remove([string]$environmentName)
            }
        }
        foreach ($environmentEntry in @($Gate.environment)) {
            if ([string]$environmentEntry.action -ceq "remove") {
                [void]$startInfo.Environment.Remove([string]$environmentEntry.name)
            }
            else {
                $startInfo.Environment[[string]$environmentEntry.name] = [string]$environmentEntry.value
            }
        }
        $startInfo.Environment["OXVBA_CORE_GATE_RUN_ID"] = $RunId
        $startInfo.Environment["OXVBA_CORE_GATE_ID"] = [string]$Gate.id
        $startInfo.Environment["OXVBA_CORE_GATE_EVIDENCE_ROOT"] = $EvidenceRoot
        $startInfo.Environment["OXVBA_CORE_GATE_PLAN_PATH"] = $PlanPath
        $startInfo.Environment["OXVBA_CORE_GATE_PLAN_SHA256"] = $PlanSha256
        $startInfo.Environment["OXVBA_CORE_GATE_MANIFEST_SHA256"] = $ManifestSha256
        $startInfo.Environment["OXVBA_CORE_GATE_MANIFEST_PATH"] = $ManifestPath
        $process.StartInfo = $startInfo
        if (-not $process.Start()) {
            throw "core-profile-gates: failed to start gate '$($Gate.id)'"
        }
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $timedOut = $false
        $cancellation = [Threading.CancellationTokenSource]::new([TimeSpan]::FromSeconds([int]$Gate.timeout_seconds))
        try {
            [void]$process.WaitForExitAsync($cancellation.Token).GetAwaiter().GetResult()
        }
        catch [OperationCanceledException] {
            $timedOut = $true
            try { $process.Kill($true) } catch {}
            try { $process.WaitForExit() } catch {}
        }
        finally {
            $cancellation.Dispose()
        }
        $stdout = $stdoutTask.GetAwaiter().GetResult()
        $stderr = $stderrTask.GetAwaiter().GetResult()
        [IO.File]::WriteAllText($stdoutPath, $stdout, $utf8)
        [IO.File]::WriteAllText($stderrPath, $stderr, $utf8)
        $stopwatch.Stop()
        $finish = [DateTimeOffset]::UtcNow
        $exitCode = if ($timedOut) { $null } else { [int]$process.ExitCode }
        $status = if ($timedOut) { "timeout" } elseif ($exitCode -eq 0) { "passed" } else { "failed" }
        $reason = if ($timedOut) {
            "command exceeded $([int]$Gate.timeout_seconds) seconds"
        }
        elseif ($exitCode -ne 0) {
            "command exited with code $exitCode"
        }
        else { "completed" }
        $result = [ordered]@{
            order = [int]$Gate.order
            id = [string]$Gate.id
            status = $status
            reason = $reason
            exit_code = $exitCode
            started_utc = $start.ToString("O")
            finished_utc = $finish.ToString("O")
            duration_ms = [int64]$stopwatch.ElapsedMilliseconds
            cargo_lock_wait_ms = if ([bool]$Gate.cargo_workspace) { [int64]$lockWait.ElapsedMilliseconds } else { $null }
            evidence_path = [string]$Gate.evidence_path
            stdout_path = "$($Gate.evidence_path)/stdout.log"
            stderr_path = "$($Gate.evidence_path)/stderr.log"
            result_path = "$($Gate.evidence_path)/result.json"
        }
        Write-JsonUtf8 -Path $resultPath -Value $result
        $process.Dispose()
        $process = $null
        return [pscustomobject]$result
    }
    finally {
        if ($lockAcquired -and $null -ne $lock) {
            try { $lock.ReleaseMutex() } catch {}
        }
        if ($null -ne $lock) { $lock.Dispose() }
        if ($null -ne $process) { $process.Dispose() }
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
        evidence_path = [string]$Gate.evidence_path
        stdout_path = ""
        stderr_path = ""
        result_path = ""
    }
}

function Write-RunManifest {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$ManifestSha256,
        [Parameter(Mandatory = $true)][string]$PlanSha256,
        [Parameter(Mandatory = $true)][string]$Platform,
        [Parameter(Mandatory = $true)][string]$Status,
        [AllowEmptyString()][string]$Failure,
        [Parameter(Mandatory = $true)][string]$StartedUtc,
        [AllowNull()][string]$FinishedUtc,
        [AllowEmptyCollection()][object[]]$Results
    )

    $runManifest = [ordered]@{
        schema_id = "oxvba-core-profile-gate-run-v1"
        run_id = $RunId
        plan_id = [string]$Manifest.plan_id
        manifest_sha256 = $ManifestSha256
        plan_sha256 = $PlanSha256
        platform = $Platform
        mode = "no-artifacts"
        no_artifacts = $true
        status = $Status
        failure = $Failure
        started_utc = $StartedUtc
        finished_utc = $FinishedUtc
        results = @($Results)
    }
    Write-JsonUtf8 -Path $Path -Value $runManifest
}

function Write-Summary {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
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
    [IO.File]::WriteAllText($Path, ($lines -join "`n") + "`n", $utf8)
}

function Assert-ExecutionEvidence {
    param(
        [Parameter(Mandatory = $true)][string]$PlanPath,
        [Parameter(Mandatory = $true)][string]$RunManifestPath,
        [Parameter(Mandatory = $true)][string]$SummaryPath,
        [Parameter(Mandatory = $true)][string]$EvidenceRoot,
        [Parameter(Mandatory = $true)]$Manifest,
        [Parameter(Mandatory = $true)][string]$ManifestSha256,
        [Parameter(Mandatory = $true)][string]$ManifestPath,
        [Parameter(Mandatory = $true)][string]$ExpectedPlanSha256,
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)][string]$Platform
    )

    Assert-ManifestUnchanged -Path $ManifestPath -ExpectedSha256 $ManifestSha256
    foreach ($path in @($PlanPath, $RunManifestPath, $SummaryPath)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "core-profile-gates: required evidence file is missing: $path"
        }
    }
    $actualPlanSha256 = Get-Sha256Hex -Bytes (Get-CanonicalTextBytes -Path $PlanPath)
    if ($actualPlanSha256 -cne $ExpectedPlanSha256) {
        throw "core-profile-gates: execution plan evidence changed during the run"
    }
    $plan = Read-StrictJson -Path $PlanPath -Owner "execution plan evidence"
    Assert-ExactKeys $plan @(
        "schema_id", "plan_id", "manifest_sha256", "version", "profile", "platform", "mode",
        "run_id", "evidence_root", "commands"
    ) "execution plan evidence"
    Assert-ExactString $plan.schema_id "oxvba-core-profile-execution-plan-v1" "execution plan evidence.schema_id"
    Assert-ExactString $plan.plan_id ([string]$Manifest.plan_id) "execution plan evidence.plan_id"
    Assert-ExactString $plan.manifest_sha256 $ManifestSha256 "execution plan evidence.manifest_sha256"
    Assert-ExactString $plan.platform $Platform "execution plan evidence.platform"
    Assert-ExactString $plan.mode "no-artifacts" "execution plan evidence.mode"
    Assert-ExactString $plan.run_id $RunId "execution plan evidence.run_id"
    if ([int64]$plan.version -ne [int64]$Manifest.version) {
        throw "core-profile-gates: execution plan evidence.version drifted"
    }
    Assert-ExactString $plan.profile ([string]$Manifest.profile) "execution plan evidence.profile"
    Assert-ExactString $plan.evidence_root "$($Manifest.evidence.no_artifact_root)/$RunId" "execution plan evidence.evidence_root"
    $planCommands = @($plan.commands)
    if ($planCommands.Count -ne @($Manifest.gates).Count) {
        throw "core-profile-gates: execution plan evidence command count drifted"
    }
    for ($index = 0; $index -lt $planCommands.Count; $index++) {
        $planCommand = $planCommands[$index]
        $gate = @($Manifest.gates)[$index]
        Assert-ExactKeys $planCommand @(
            "order", "id", "disposition", "reason", "platforms", "kind", "command", "arguments",
            "environment", "timeout_seconds", "cargo_workspace", "evidence_path"
        ) "execution plan evidence.commands[$index]"
        $selected = @($gate.platforms) -ccontains $Platform
        $expectedDisposition = if ($selected) { "run" } else { "not-applicable" }
        $expectedReason = if ($selected) { "selected:$Platform" } else { "platform:$Platform" }
        foreach ($field in @("order", "id", "kind", "command", "timeout_seconds", "cargo_workspace", "evidence_path")) {
            if ([string]$planCommand.$field -cne [string]$gate.$field) {
                throw "core-profile-gates: execution plan evidence command field '$field' drifted at index $index"
            }
        }
        Assert-ExactString $planCommand.disposition $expectedDisposition "execution plan evidence.commands[$index].disposition"
        Assert-ExactString $planCommand.reason $expectedReason "execution plan evidence.commands[$index].reason"
        if ((@($planCommand.platforms) -join '|') -cne (@($gate.platforms) -join '|') -or
            (@($planCommand.arguments) -join "`u{1f}") -cne (@($gate.arguments) -join "`u{1f}")) {
            throw "core-profile-gates: execution plan evidence command platforms/arguments drifted at index $index"
        }
        $planEnvironment = @($planCommand.environment)
        $gateEnvironment = @($gate.environment)
        if ($planEnvironment.Count -ne $gateEnvironment.Count) {
            throw "core-profile-gates: execution plan evidence command environment count drifted at index $index"
        }
        for ($environmentIndex = 0; $environmentIndex -lt $planEnvironment.Count; $environmentIndex++) {
            Assert-ExactKeys $planEnvironment[$environmentIndex] @("name", "action", "value") "execution plan evidence.commands[$index].environment[$environmentIndex]"
            foreach ($field in @("name", "action", "value")) {
                if ([string]$planEnvironment[$environmentIndex].$field -cne [string]$gateEnvironment[$environmentIndex].$field) {
                    throw "core-profile-gates: execution plan evidence command environment field '$field' drifted at index $index"
                }
            }
        }
    }

    $runManifest = Read-StrictJson -Path $RunManifestPath -Owner "run manifest evidence"
    Assert-ExactKeys $runManifest @(
        "schema_id", "run_id", "plan_id", "manifest_sha256", "plan_sha256", "platform",
        "mode", "no_artifacts", "status", "failure", "started_utc", "finished_utc", "results"
    ) "run manifest evidence"
    Assert-ExactString $runManifest.schema_id "oxvba-core-profile-gate-run-v1" "run manifest evidence.schema_id"
    Assert-ExactString $runManifest.run_id $RunId "run manifest evidence.run_id"
    Assert-ExactString $runManifest.plan_id ([string]$Manifest.plan_id) "run manifest evidence.plan_id"
    Assert-ExactString $runManifest.manifest_sha256 $ManifestSha256 "run manifest evidence.manifest_sha256"
    Assert-ExactString $runManifest.plan_sha256 $ExpectedPlanSha256 "run manifest evidence.plan_sha256"
    Assert-ExactString $runManifest.platform $Platform "run manifest evidence.platform"
    Assert-ExactString $runManifest.mode "no-artifacts" "run manifest evidence.mode"
    if ($runManifest.no_artifacts -isnot [bool] -or -not [bool]$runManifest.no_artifacts) {
        throw "core-profile-gates: run manifest evidence.no_artifacts must be true"
    }
    if ([string]$runManifest.status -cnotin @("passed", "failed")) {
        throw "core-profile-gates: run manifest evidence.status must be passed or failed"
    }
    $results = @($runManifest.results)
    if ($results.Count -ne @($Manifest.gates).Count) {
        throw "core-profile-gates: run manifest evidence result count drifted"
    }
    for ($index = 0; $index -lt $results.Count; $index++) {
        $result = $results[$index]
        $gate = @($Manifest.gates)[$index]
        Assert-ExactKeys $result @(
            "order", "id", "status", "reason", "exit_code", "started_utc", "finished_utc",
            "duration_ms", "cargo_lock_wait_ms", "evidence_path", "stdout_path", "stderr_path",
            "result_path"
        ) "run manifest evidence.results[$index]"
        if ([int64]$result.order -ne [int64]$gate.order -or [string]$result.id -cne [string]$gate.id) {
            throw "core-profile-gates: run manifest evidence result identity/order drifted at index $index"
        }
        if ([string]$result.status -cnotin @("passed", "failed", "timeout", "not-applicable", "not-run")) {
            throw "core-profile-gates: run manifest evidence result '$($result.id)' has invalid status '$($result.status)'"
        }
        if ([string]$result.evidence_path -cne [string]$gate.evidence_path) {
            throw "core-profile-gates: run manifest evidence path drifted for '$($result.id)'"
        }
        if ([string]$result.status -in @("passed", "failed", "timeout")) {
            foreach ($relativePath in @($result.stdout_path, $result.stderr_path, $result.result_path)) {
                $absolutePath = Resolve-RepoRelativePath -Root $EvidenceRoot -Path ([string]$relativePath) -Owner "run manifest result path"
                if (-not (Test-Path -LiteralPath $absolutePath -PathType Leaf)) {
                    throw "core-profile-gates: executed gate evidence is missing: $relativePath"
                }
            }
            $gateResultPath = Resolve-RepoRelativePath -Root $EvidenceRoot -Path ([string]$result.result_path) -Owner "gate result evidence"
            $gateResult = Read-StrictJson -Path $gateResultPath -Owner "gate result evidence"
            Assert-ExactKeys $gateResult @(
                "order", "id", "status", "reason", "exit_code", "started_utc", "finished_utc",
                "duration_ms", "cargo_lock_wait_ms", "evidence_path", "stdout_path", "stderr_path",
                "result_path"
            ) "gate result evidence"
            foreach ($field in @(
                "order", "id", "status", "reason", "exit_code", "started_utc", "finished_utc",
                "duration_ms", "cargo_lock_wait_ms", "evidence_path", "stdout_path", "stderr_path",
                "result_path"
            )) {
                if ([string]$gateResult.$field -cne [string]$result.$field) {
                    throw "core-profile-gates: gate result evidence field '$field' differs from the run manifest for '$($result.id)'"
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
$platform = Get-CurrentPlatformId
$manifestAbs = Resolve-RepoRelativePath -Root $repoRoot -Path $ManifestPath -Owner "ManifestPath"
if (-not (Test-Path -LiteralPath $manifestAbs -PathType Leaf)) {
    throw "core-profile-gates: manifest is missing: $ManifestPath"
}
$manifest = Read-StrictJson -Path $manifestAbs -Owner "manifest"
Assert-ManifestJsonArrayShapes -Path $manifestAbs
Assert-Manifest -Manifest $manifest -RepoRoot $repoRoot -Platform $platform
$manifestSha256 = Get-Sha256Hex -Bytes (Get-CanonicalTextBytes -Path $manifestAbs)
Assert-ManifestUnchanged -Path $manifestAbs -ExpectedSha256 $manifestSha256

if ($List -or $DryRun) {
    Write-DeterministicPlan -Manifest $manifest -Platform $platform
    return
}

if ($Mode -ceq "ValidateManifest") {
    if (-not [string]::IsNullOrWhiteSpace($RunId)) {
        throw "core-profile-gates: RunId is only valid for Mode=NoArtifacts"
    }
    Write-Host "core-profile-gates: manifest ok (plan=$($manifest.plan_id) version=$($manifest.version) platform=$platform sha256=$manifestSha256 gates=$(@($manifest.gates).Count))"
    return
}

if ([string]::IsNullOrWhiteSpace($RunId) -or $RunId -cnotmatch '^[a-z0-9][a-z0-9._-]{0,63}$' -or
    $RunId -in @(".", "..")) {
    throw "core-profile-gates: Mode=NoArtifacts requires a bounded lowercase RunId"
}
$evidenceBase = Resolve-RepoRelativePath -Root $repoRoot -Path ([string]$manifest.evidence.no_artifact_root) -Owner "manifest.evidence.no_artifact_root"
$evidenceRoot = [IO.Path]::GetFullPath((Join-Path $evidenceBase $RunId))
$rootPrefix = $evidenceBase.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
$comparison = if ($IsWindows) { [StringComparison]::OrdinalIgnoreCase } else { [StringComparison]::Ordinal }
if (-not $evidenceRoot.StartsWith($rootPrefix, $comparison)) {
    throw "core-profile-gates: RunId escapes the no-artifact evidence root"
}
Assert-NoReparseAncestor -Root $repoRoot -Target $evidenceRoot
if (Test-Path -LiteralPath $evidenceRoot) {
    throw "core-profile-gates: no-artifact evidence root already exists; refusing stale evidence: $evidenceRoot"
}
[void](New-Item -ItemType Directory -Path $evidenceRoot)

$planPath = Join-Path $evidenceRoot ([string]$manifest.evidence.plan_path)
$runManifestPath = Join-Path $evidenceRoot ([string]$manifest.evidence.run_manifest_path)
$summaryPath = Join-Path $evidenceRoot ([string]$manifest.evidence.summary_path)
$planRows = @()
foreach ($gate in @($manifest.gates)) {
    $selected = @($gate.platforms) -ccontains $platform
    $environmentRows = @($gate.environment | ForEach-Object {
            [ordered]@{ name = [string]$_.name; action = [string]$_.action; value = [string]$_.value }
        })
    $planRows += [ordered]@{
        order = [int]$gate.order
        id = [string]$gate.id
        disposition = if ($selected) { "run" } else { "not-applicable" }
        reason = if ($selected) { "selected:$platform" } else { "platform:$platform" }
        platforms = @($gate.platforms)
        kind = [string]$gate.kind
        command = [string]$gate.command
        arguments = @($gate.arguments)
        environment = $environmentRows
        timeout_seconds = [int]$gate.timeout_seconds
        cargo_workspace = [bool]$gate.cargo_workspace
        evidence_path = [string]$gate.evidence_path
    }
}
$executionPlan = [ordered]@{
    schema_id = "oxvba-core-profile-execution-plan-v1"
    plan_id = [string]$manifest.plan_id
    manifest_sha256 = $manifestSha256
    version = [int]$manifest.version
    profile = [string]$manifest.profile
    platform = $platform
    mode = "no-artifacts"
    run_id = $RunId
    evidence_root = "$($manifest.evidence.no_artifact_root)/$RunId"
    commands = $planRows
}
Write-JsonUtf8 -Path $planPath -Value $executionPlan
$planSha256 = Get-Sha256Hex -Bytes (Get-CanonicalTextBytes -Path $planPath)
$mutexName = Get-CargoMutexName -Prefix ([string]$manifest.cargo_lock.name_prefix) -RepoRoot $repoRoot
$runStarted = [DateTimeOffset]::UtcNow.ToString("O")
$results = @()
$executionFailure = $null

Write-RunManifest -Path $runManifestPath -RunId $RunId -Manifest $manifest `
    -ManifestSha256 $manifestSha256 -PlanSha256 $planSha256 -Platform $platform -Status "running" `
    -Failure "" -StartedUtc $runStarted -FinishedUtc $null -Results $results

foreach ($gate in @($manifest.gates)) {
    if ($null -ne $executionFailure) {
        $results += New-NonExecutedResult -Gate $gate -Status "not-run" -Reason "earlier gate failed"
        continue
    }
    try {
        Assert-ManifestUnchanged -Path $manifestAbs -ExpectedSha256 $manifestSha256
    }
    catch {
        $executionFailure = $_.Exception.Message
        $results += New-NonExecutedResult -Gate $gate -Status "not-run" -Reason $executionFailure
        continue
    }
    if (@($gate.platforms) -cnotcontains $platform) {
        $results += New-NonExecutedResult -Gate $gate -Status "not-applicable" -Reason "platform:$platform"
        continue
    }
    $result = $null
    try {
        Write-Host ("[core-profile] {0:D3} {1} (timeout={2}s cargo_lock={3})" -f
            [int]$gate.order, [string]$gate.id, [int]$gate.timeout_seconds, [bool]$gate.cargo_workspace)
        $result = Invoke-GateProcess -Gate $gate -RepoRoot $repoRoot -EvidenceRoot $evidenceRoot `
            -PlanPath $planPath -PlanSha256 $planSha256 -ManifestSha256 $manifestSha256 -RunId $RunId `
            -ManifestPath $manifestAbs -MutexName $mutexName `
            -MutexTimeoutSeconds ([int]$manifest.cargo_lock.acquire_timeout_seconds)
        $results += $result
        Assert-ManifestUnchanged -Path $manifestAbs -ExpectedSha256 $manifestSha256
        if ([string]$result.status -cne "passed") {
            $executionFailure = "gate '$($gate.id)' $($result.reason)"
        }
    }
    catch {
        $executionFailure = "gate '$($gate.id)' failed: $($_.Exception.Message)"
        if ($null -eq $result) {
            $results += New-NonExecutedResult -Gate $gate -Status "not-run" -Reason $executionFailure
        }
    }
}

$runFinished = [DateTimeOffset]::UtcNow.ToString("O")
$runStatus = if ($null -eq $executionFailure) { "passed" } else { "failed" }
$failureText = if ($null -eq $executionFailure) { "" } else { [string]$executionFailure }
Write-RunManifest -Path $runManifestPath -RunId $RunId -Manifest $manifest `
    -ManifestSha256 $manifestSha256 -PlanSha256 $planSha256 -Platform $platform -Status $runStatus `
    -Failure $failureText -StartedUtc $runStarted -FinishedUtc $runFinished -Results $results
Write-Summary -Path $summaryPath -RunId $RunId -Platform $platform -Status $runStatus `
    -Failure $failureText -Results $results

try {
    Assert-ExecutionEvidence -PlanPath $planPath -RunManifestPath $runManifestPath -SummaryPath $summaryPath `
        -EvidenceRoot $evidenceRoot -Manifest $manifest -ManifestSha256 $manifestSha256 -ManifestPath $manifestAbs `
        -ExpectedPlanSha256 $planSha256 -RunId $RunId -Platform $platform
}
catch {
    $evidenceFailure = "evidence validation failed: $($_.Exception.Message)"
    Write-RunManifest -Path $runManifestPath -RunId $RunId -Manifest $manifest `
        -ManifestSha256 $manifestSha256 -PlanSha256 $planSha256 -Platform $platform -Status "failed" `
        -Failure $evidenceFailure -StartedUtc $runStarted -FinishedUtc ([DateTimeOffset]::UtcNow.ToString("O")) `
        -Results $results
    Write-Summary -Path $summaryPath -RunId $RunId -Platform $platform -Status "failed" `
        -Failure $evidenceFailure -Results $results
    throw "core-profile-gates: $evidenceFailure"
}

if ($null -ne $executionFailure) {
    throw "core-profile-gates: $executionFailure"
}
Assert-ManifestUnchanged -Path $manifestAbs -ExpectedSha256 $manifestSha256
Write-Host "core-profile-gates: ok (run_id=$RunId platform=$platform evidence=$evidenceRoot manifest_sha256=$manifestSha256 plan_sha256=$planSha256)"
