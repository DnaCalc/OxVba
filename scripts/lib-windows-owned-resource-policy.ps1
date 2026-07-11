Set-StrictMode -Version Latest

$script:WindowsOwnedJournalSchema = "oxvba-windows-owned-resource-journal-v1"
$script:WindowsOwnedRunIdPattern = '^oxvba-[0-9]{8}T[0-9]{6}Z-[0-9a-f]{32}$'
$script:WindowsOwnedPolicyRepositoryRoot = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot)).TrimEnd(
    [IO.Path]::DirectorySeparatorChar,
    [IO.Path]::AltDirectorySeparatorChar)

function Get-WindowsOwnedSha256Bytes {
    param([Parameter(Mandatory = $true)][byte[]]$Bytes)

    $digest = [Security.Cryptography.SHA256]::HashData($Bytes)
    return "sha256:$([Convert]::ToHexString($digest).ToLowerInvariant())"
}

function Get-WindowsOwnedSha256Text {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)

    return Get-WindowsOwnedSha256Bytes -Bytes ([Text.UTF8Encoding]::new($false).GetBytes($Text))
}

function Get-WindowsOwnedUtcText {
    return [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ss.fffffffZ", [Globalization.CultureInfo]::InvariantCulture)
}

function New-WindowsOwnedRunId {
    return "oxvba-$([DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ', [Globalization.CultureInfo]::InvariantCulture))-$([Guid]::NewGuid().ToString('N'))"
}

function Test-WindowsOwnedPathWithin {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$CandidatePath
    )

    $base = [IO.Path]::GetFullPath($BasePath).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    $candidate = [IO.Path]::GetFullPath($CandidatePath)
    $comparison = if ([Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([Runtime.InteropServices.OSPlatform]::Windows)) {
        [StringComparison]::OrdinalIgnoreCase
    }
    else {
        [StringComparison]::Ordinal
    }
    return $candidate.Equals($base, $comparison) -or
        $candidate.StartsWith($base + [IO.Path]::DirectorySeparatorChar, $comparison)
}

function Assert-WindowsOwnedNoReparseTraversal {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$CandidatePath,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $base = [IO.Path]::GetFullPath($BasePath).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    $candidate = [IO.Path]::GetFullPath($CandidatePath)
    if (-not (Test-WindowsOwnedPathWithin -BasePath $base -CandidatePath $candidate)) {
        throw "$Owner escapes its controlled root '$base'"
    }
    $relative = [IO.Path]::GetRelativePath($base, $candidate)
    $current = $base
    foreach ($part in @($relative -split '[\\/]')) {
        if ([string]::IsNullOrWhiteSpace($part) -or $part -eq ".") {
            continue
        }
        $current = Join-Path $current $part
        if (-not (Test-Path -LiteralPath $current)) {
            continue
        }
        $item = Get-Item -LiteralPath $current -Force
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "$Owner crosses reparse point '$current'"
        }
    }
}

function Assert-WindowsOwnedConfinedPath {
    param(
        [Parameter(Mandatory = $true)]$Journal,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ([string]::IsNullOrWhiteSpace($Path) -or $Path.IndexOfAny([char[]]'*?[]') -ge 0) {
        throw "$Owner must be one exact non-wildcard path"
    }
    $candidate = [IO.Path]::GetFullPath($Path)
    $roots = @([string]$Journal.repository_root, [string]$Journal.temp_root)
    $matchedRoot = $null
    foreach ($root in $roots) {
        if (Test-WindowsOwnedPathWithin -BasePath $root -CandidatePath $candidate) {
            $matchedRoot = [IO.Path]::GetFullPath($root)
            break
        }
    }
    if ($null -eq $matchedRoot -or
        $candidate -eq [IO.Path]::GetFullPath([string]$Journal.repository_root) -or
        $candidate -eq [IO.Path]::GetFullPath([string]$Journal.temp_root)) {
        throw "$Owner is outside the exact repository/temp confinement or names a controlled root"
    }
    Assert-WindowsOwnedNoReparseTraversal -BasePath $matchedRoot -CandidatePath $candidate -Owner $Owner
    return $candidate
}

function ConvertTo-WindowsOwnedRegistryPath {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [string]$Owner = "registry path"
    )

    if ([string]::IsNullOrWhiteSpace($Path) -or $Path.IndexOfAny([char[]]'*?[]') -ge 0 -or
        $Path -match '(?:^|[\\/])\.\.(?:[\\/]|$)') {
        throw "$Owner must be one exact non-wildcard HKCU path"
    }
    $normalized = $Path.Trim().Replace('/', '\').Replace('HKCU:\', 'HKCU\').Replace('HKEY_CURRENT_USER\', 'HKCU\')
    while ($normalized.Contains('\\')) {
        $normalized = $normalized.Replace('\\', '\')
    }
    $normalized = $normalized.TrimEnd('\')
    if ($normalized -notmatch '^HKCU\\Software\\[^\\]+\\[^\\]+(?:\\.*)?$' -or
        $normalized -in @('HKCU\Software\Classes\CLSID', 'HKCU\Software\Classes\TypeLib', 'HKCU\Software\Classes\Interface', 'HKCU\Software\Classes\AppID')) {
        throw "$Owner must be an exact HKCU leaf allowlist, not a hive/category root"
    }
    return $normalized
}

function Test-WindowsOwnedStringSetEqual {
    param([string[]]$Left, [string[]]$Right)

    $a = @($Left | Sort-Object -Unique -CaseSensitive)
    $b = @($Right | Sort-Object -Unique -CaseSensitive)
    return ($a -join "`n") -ceq ($b -join "`n")
}

function Assert-WindowsOwnedJsonNoDuplicates {
    param(
        [Parameter(Mandatory = $true)][byte[]]$Bytes,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    try {
        $text = [Text.UTF8Encoding]::new($false, $true).GetString($Bytes)
        $document = [Text.Json.JsonDocument]::Parse($text)
    }
    catch {
        throw "$Owner is not strict JSON"
    }
    try {
        $walk = $null
        $walk = {
            param([Text.Json.JsonElement]$Element, [string]$JsonPath)
            if ($Element.ValueKind -eq [Text.Json.JsonValueKind]::Object) {
                $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
                foreach ($property in $Element.EnumerateObject()) {
                    if (-not $names.Add($property.Name)) {
                        throw "$Owner contains duplicate JSON property '$($property.Name)' at '$JsonPath'"
                    }
                    & $walk $property.Value "$JsonPath.$($property.Name)"
                }
            }
            elseif ($Element.ValueKind -eq [Text.Json.JsonValueKind]::Array) {
                $index = 0
                foreach ($item in $Element.EnumerateArray()) {
                    & $walk $item "$JsonPath[$index]"
                    $index++
                }
            }
        }
        & $walk $document.RootElement '$'
    }
    finally {
        $document.Dispose()
    }
}

function Assert-WindowsOwnedExactProperties {
    param(
        [Parameter(Mandatory = $true)]$Value,
        [Parameter(Mandatory = $true)][string[]]$Expected,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ($null -eq $Value -or $Value -isnot [psobject]) {
        throw "$Owner must be a JSON object"
    }
    $actual = @($Value.PSObject.Properties.Name)
    if (-not (Test-WindowsOwnedStringSetEqual -Left $actual -Right $Expected)) {
        throw "$Owner must use the exact case-sensitive property schema"
    }
}

function Get-WindowsOwnedJournalDigest {
    param([Parameter(Mandatory = $true)]$Journal)

    $payload = [pscustomobject][ordered]@{
        schema_id = [string]$Journal.schema_id
        schema_version = [int]$Journal.schema_version
        run_id = [string]$Journal.run_id
        created_utc = [string]$Journal.created_utc
        updated_utc = [string]$Journal.updated_utc
        owner_pid = [int]$Journal.owner_pid
        owner_process_start_utc = [string]$Journal.owner_process_start_utc
        repository_root = [string]$Journal.repository_root
        temp_root = [string]$Journal.temp_root
        run_root = [string]$Journal.run_root
        journal_path = [string]$Journal.journal_path
        allowed_registry_paths = @($Journal.allowed_registry_paths)
        allowed_executable_paths = @($Journal.allowed_executable_paths)
        orchestrator_apartment = $Journal.orchestrator_apartment
        reentry_policy = [string]$Journal.reentry_policy
        state = [string]$Journal.state
        next_resource_sequence = [int]$Journal.next_resource_sequence
        next_event_sequence = [int]$Journal.next_event_sequence
        resources = @($Journal.resources)
        events = @($Journal.events)
    }
    # Normalize live CLR collection/value shapes through the same strict JSON
    # representation used after a journal is reloaded.
    $canonical = ($payload | ConvertTo-Json -Depth 32 -Compress) |
        ConvertFrom-Json -Depth 32 -DateKind String |
        ConvertTo-Json -Depth 32 -Compress
    return Get-WindowsOwnedSha256Text -Text $canonical
}

function Write-WindowsOwnedResourceJournal {
    param([Parameter(Mandatory = $true)]$Journal)

    $Journal.updated_utc = Get-WindowsOwnedUtcText
    # Persist and hash the same JSON-normalized object shape. PowerShell can
    # otherwise serialize a live generic list differently from its reloaded
    # Object[] representation even when the JSON values are equivalent.
    $normalized = ($Journal | ConvertTo-Json -Depth 32 -Compress) | ConvertFrom-Json -Depth 32 -DateKind String
    $normalized.journal_digest = Get-WindowsOwnedJournalDigest -Journal $normalized
    $Journal.journal_digest = [string]$normalized.journal_digest
    $path = [IO.Path]::GetFullPath([string]$Journal.journal_path)
    $parent = Split-Path -Parent $path
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        throw "owned-resource journal parent '$parent' does not exist"
    }
    $text = ($normalized | ConvertTo-Json -Depth 32) + "`n"
    $bytes = [Text.UTF8Encoding]::new($false).GetBytes($text)
    $temporary = "$path.write-$PID-$([Guid]::NewGuid().ToString('N'))"
    $stream = [IO.FileStream]::new(
        $temporary,
        [IO.FileMode]::CreateNew,
        [IO.FileAccess]::Write,
        [IO.FileShare]::None,
        4096,
        [IO.FileOptions]::WriteThrough)
    try {
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
    }
    finally {
        $stream.Dispose()
    }
    try {
        [IO.File]::Move($temporary, $path, $true)
    }
    finally {
        if (Test-Path -LiteralPath $temporary) {
            Remove-Item -LiteralPath $temporary -Force
        }
    }
}

function Read-WindowsOwnedResourceJournal {
    param([Parameter(Mandatory = $true)][string]$JournalPath)

    $path = [IO.Path]::GetFullPath($JournalPath)
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "owned-resource journal '$path' does not exist"
    }
    $bytes = [IO.File]::ReadAllBytes($path)
    Assert-WindowsOwnedJsonNoDuplicates -Bytes $bytes -Owner "owned-resource journal '$path'"
    try {
        $journal = [Text.UTF8Encoding]::new($false, $true).GetString($bytes) | ConvertFrom-Json -Depth 32 -DateKind String
    }
    catch {
        throw "owned-resource journal '$path' is not strict UTF-8 JSON"
    }
    Assert-WindowsOwnedExactProperties -Value $journal -Expected @(
        'schema_id', 'schema_version', 'run_id', 'created_utc', 'updated_utc',
        'owner_pid', 'owner_process_start_utc', 'repository_root', 'temp_root',
        'run_root', 'journal_path', 'allowed_registry_paths',
        'allowed_executable_paths', 'orchestrator_apartment', 'reentry_policy',
        'state', 'next_resource_sequence', 'next_event_sequence', 'resources',
        'events', 'journal_digest'
    ) -Owner "owned-resource journal '$path'"
    Assert-WindowsOwnedExactProperties -Value $journal.orchestrator_apartment -Expected @(
        'process_id', 'thread_id', 'model'
    ) -Owner "owned-resource journal '$path' orchestrator apartment"
    $repositoryRoot = [IO.Path]::GetFullPath([string]$journal.repository_root).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    $tempRoot = [IO.Path]::GetFullPath([string]$journal.temp_root).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    $expectedRunRoot = [IO.Path]::GetFullPath((Join-Path (Join-Path $tempRoot 'oxvba-owned-resource-runs') ([string]$journal.run_id)))
    $expectedJournalPath = [IO.Path]::GetFullPath((Join-Path (Join-Path $tempRoot 'oxvba-owned-resource-journals') "$($journal.run_id).json"))
    if ([string]$journal.schema_id -cne $script:WindowsOwnedJournalSchema -or
        [int]$journal.schema_version -ne 1 -or
        [string]$journal.run_id -notmatch $script:WindowsOwnedRunIdPattern -or
        -not [IO.Path]::IsPathFullyQualified([string]$journal.repository_root) -or
        -not [IO.Path]::IsPathFullyQualified([string]$journal.temp_root) -or
        -not (Test-WindowsOwnedExactPathEqual -Left ([string]$journal.repository_root) -Right $repositoryRoot) -or
        -not (Test-WindowsOwnedExactPathEqual -Left $repositoryRoot -Right $script:WindowsOwnedPolicyRepositoryRoot) -or
        -not (Test-WindowsOwnedExactPathEqual -Left ([string]$journal.temp_root) -Right $tempRoot) -or
        -not (Test-WindowsOwnedExactPathEqual -Left ([string]$journal.run_root) -Right $expectedRunRoot) -or
        -not (Test-WindowsOwnedExactPathEqual -Left ([string]$journal.journal_path) -Right $expectedJournalPath) -or
        -not (Test-WindowsOwnedExactPathEqual -Left $path -Right $expectedJournalPath) -or
        [int]$journal.owner_pid -le 0 -or
        [string]$journal.owner_process_start_utc -notmatch '^\d{4}-\d{2}-\d{2}T' -or
        [int]$journal.orchestrator_apartment.process_id -ne [int]$journal.owner_pid -or
        [int]$journal.orchestrator_apartment.thread_id -le 0 -or
        [string]$journal.orchestrator_apartment.model -notin @('STA', 'MTA', 'none') -or
        [string]$journal.reentry_policy -notin @('reject', 'same-apartment-synchronous', 'declared-nested') -or
        [string]$journal.journal_digest -notmatch '^sha256:[0-9a-f]{64}$' -or
        [string]$journal.journal_digest -cne (Get-WindowsOwnedJournalDigest -Journal $journal)) {
        throw "owned-resource journal '$path' has invalid identity or digest"
    }
    $registryPaths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($registryPath in @($journal.allowed_registry_paths)) {
        $normalized = ConvertTo-WindowsOwnedRegistryPath -Path ([string]$registryPath) -Owner "owned-resource journal '$path' registry allowlist"
        if ([string]$registryPath -cne $normalized -or -not $registryPaths.Add($normalized)) {
            throw "owned-resource journal '$path' has a noncanonical or duplicate registry allowlist entry"
        }
    }
    $executablePaths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($executablePath in @($journal.allowed_executable_paths)) {
        if ([string]::IsNullOrWhiteSpace([string]$executablePath) -or
            ([string]$executablePath).IndexOfAny([char[]]'*?[]') -ge 0 -or
            -not [IO.Path]::IsPathFullyQualified([string]$executablePath) -or
            -not (Test-WindowsOwnedExactPathEqual -Left ([string]$executablePath) -Right ([IO.Path]::GetFullPath([string]$executablePath))) -or
            -not $executablePaths.Add([string]$executablePath)) {
            throw "owned-resource journal '$path' has a noncanonical or duplicate executable allowlist entry"
        }
    }
    if ([string]$journal.state -notin @('active', 'cleaning', 'cleanup-conflict', 'completed')) {
        throw "owned-resource journal '$path' has invalid state '$($journal.state)'"
    }
    if ([int]$journal.next_resource_sequence -ne @($journal.resources).Count + 1 -or
        [int]$journal.next_event_sequence -ne @($journal.events).Count + 1) {
        throw "owned-resource journal '$path' sequence counters are inconsistent"
    }
    $expectedSequence = 1
    $resourceIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($resource in @($journal.resources)) {
        Assert-WindowsOwnedExactProperties -Value $resource -Expected @(
            'sequence', 'resource_id', 'kind', 'state', 'prepared_utc',
            'active_utc', 'cleaned_utc', 'descriptor', 'before', 'expected'
        ) -Owner "owned-resource journal '$path' resource"
        if ([int]$resource.sequence -ne $expectedSequence -or
            [string]$resource.resource_id -notmatch '^[a-z]+-[0-9a-f]{32}$' -or
            -not $resourceIds.Add([string]$resource.resource_id) -or
            [string]$resource.kind -notin @('file', 'registry', 'process', 'apartment', 'callback', 'connection', 'dialog') -or
            [string]$resource.state -notin @('prepared', 'active', 'cleaned', 'conflict')) {
            throw "owned-resource journal '$path' contains an invalid resource record"
        }
        if ([string]$resource.prepared_utc -notmatch '^\d{4}-\d{2}-\d{2}T' -or
            ([string]$resource.state -ceq 'prepared' -and
                (-not [string]::IsNullOrEmpty([string]$resource.active_utc) -or -not [string]::IsNullOrEmpty([string]$resource.cleaned_utc))) -or
            ([string]$resource.state -ceq 'active' -and
                ([string]$resource.active_utc -notmatch '^\d{4}-\d{2}-\d{2}T' -or -not [string]::IsNullOrEmpty([string]$resource.cleaned_utc))) -or
            ([string]$resource.state -ceq 'cleaned' -and [string]$resource.cleaned_utc -notmatch '^\d{4}-\d{2}-\d{2}T')) {
            throw "owned-resource journal '$path' contains inconsistent resource transition timestamps"
        }
        Assert-WindowsOwnedResourceDescriptor -Journal $journal -Resource $resource
        $expectedSequence++
    }
    $expectedEventSequence = 1
    foreach ($event in @($journal.events)) {
        Assert-WindowsOwnedExactProperties -Value $event -Expected @(
            'sequence', 'timestamp_utc', 'event', 'resource_id', 'detail'
        ) -Owner "owned-resource journal '$path' event"
        if ([int]$event.sequence -ne $expectedEventSequence) {
            throw "owned-resource journal '$path' contains an invalid event sequence"
        }
        $expectedEventSequence++
    }
    return $journal
}

function Get-WindowsOwnedProcessStartUtc {
    param([Parameter(Mandatory = $true)][int]$ProcessId)

    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if ($null -eq $process) {
        return $null
    }
    try {
        return $process.StartTime.ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffffffZ", [Globalization.CultureInfo]::InvariantCulture)
    }
    catch {
        return $null
    }
}

function Test-WindowsOwnedProcessIdentity {
    param(
        [Parameter(Mandatory = $true)][int]$ProcessId,
        [Parameter(Mandatory = $true)][string]$StartUtc
    )

    $actual = Get-WindowsOwnedProcessStartUtc -ProcessId $ProcessId
    return $null -ne $actual -and $actual -ceq $StartUtc
}

function Add-WindowsOwnedJournalEvent {
    param(
        [Parameter(Mandatory = $true)]$Journal,
        [Parameter(Mandatory = $true)][string]$Event,
        [AllowEmptyString()][string]$ResourceId = "",
        [AllowEmptyString()][string]$Detail = ""
    )

    $entry = [pscustomobject][ordered]@{
        sequence = [int]$Journal.next_event_sequence
        timestamp_utc = Get-WindowsOwnedUtcText
        event = $Event
        resource_id = $ResourceId
        detail = $Detail
    }
    $Journal.events = @($Journal.events) + @($entry)
    $Journal.next_event_sequence = [int]$Journal.next_event_sequence + 1
}

function Assert-WindowsOwnedJournalWriter {
    param([Parameter(Mandatory = $true)]$Journal)

    $authorized = [int]$Journal.owner_pid -eq $PID -and
        (Test-WindowsOwnedProcessIdentity -ProcessId $PID -StartUtc ([string]$Journal.owner_process_start_utc))
    if (-not $authorized) {
        foreach ($resource in @($Journal.resources | Where-Object { $_.kind -eq 'process' -and $_.state -eq 'active' })) {
            if ([int]$resource.descriptor.pid -eq $PID -and
                (Test-WindowsOwnedProcessIdentity -ProcessId $PID -StartUtc ([string]$resource.descriptor.process_start_utc))) {
                $authorized = $true
                break
            }
        }
    }
    if (-not $authorized) {
        throw "owned-resource journal '$($Journal.journal_path)' can only be mutated by its exact owner or a recorded live child"
    }
}

function New-WindowsOwnedResourceJournal {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$TempRoot,
        [string[]]$AllowedRegistryPaths = @(),
        [string[]]$AllowedExecutablePaths = @(),
        [ValidateSet('STA', 'MTA', 'none')][string]$OrchestratorApartment = 'none',
        [ValidateSet('reject', 'same-apartment-synchronous', 'declared-nested')][string]$ReentryPolicy = 'reject',
        [string]$RunId = "",
        [int]$OwnerPid = $PID
    )

    if ([string]::IsNullOrWhiteSpace($RunId)) {
        $RunId = New-WindowsOwnedRunId
    }
    if ($RunId -notmatch $script:WindowsOwnedRunIdPattern) {
        throw "owned-resource run ID '$RunId' is not immutable and unique-formatted"
    }
    $repositoryRootFull = [IO.Path]::GetFullPath($RepositoryRoot).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    $tempRootFull = [IO.Path]::GetFullPath($TempRoot).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    if (-not (Test-Path -LiteralPath $repositoryRootFull -PathType Container) -or
        -not (Test-Path -LiteralPath $tempRootFull -PathType Container)) {
        throw "owned-resource repository and temp roots must already exist"
    }
    if (-not (Test-WindowsOwnedExactPathEqual -Left $repositoryRootFull -Right $script:WindowsOwnedPolicyRepositoryRoot)) {
        throw "owned-resource repository root must match the policy helper's exact repository"
    }
    $ownerStart = Get-WindowsOwnedProcessStartUtc -ProcessId $OwnerPid
    if ($null -eq $ownerStart) {
        throw "owned-resource journal owner PID '$OwnerPid' is not a live process"
    }

    $registryPaths = [Collections.Generic.List[string]]::new()
    $seenRegistry = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($path in $AllowedRegistryPaths) {
        $normalized = ConvertTo-WindowsOwnedRegistryPath -Path $path -Owner "owned-resource registry allowlist"
        if (-not $seenRegistry.Add($normalized)) {
            throw "owned-resource registry allowlist contains duplicate '$normalized'"
        }
        $registryPaths.Add($normalized)
    }
    $executables = [Collections.Generic.List[string]]::new()
    $seenExecutables = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($path in $AllowedExecutablePaths) {
        if ([string]::IsNullOrWhiteSpace($path) -or $path.IndexOfAny([char[]]'*?[]') -ge 0) {
            throw "owned-resource executable allowlist must contain exact paths"
        }
        $full = [IO.Path]::GetFullPath($path)
        if (-not (Test-Path -LiteralPath $full -PathType Leaf) -or -not $seenExecutables.Add($full)) {
            throw "owned-resource executable allowlist path '$path' is missing or duplicate"
        }
        $executables.Add($full)
    }

    $journalDirectory = Join-Path $tempRootFull "oxvba-owned-resource-journals"
    $runDirectory = Join-Path $tempRootFull "oxvba-owned-resource-runs"
    [void](New-Item -ItemType Directory -Path $journalDirectory -Force)
    [void](New-Item -ItemType Directory -Path $runDirectory -Force)
    $journalPath = Join-Path $journalDirectory "$RunId.json"
    $runRoot = Join-Path $runDirectory $RunId
    if ((Test-Path -LiteralPath $journalPath) -or (Test-Path -LiteralPath $runRoot)) {
        throw "owned-resource run '$RunId' collides with an existing journal/root"
    }
    [void](New-Item -ItemType Directory -Path $runRoot)
    $now = Get-WindowsOwnedUtcText
    $journal = [pscustomobject][ordered]@{
        schema_id = $script:WindowsOwnedJournalSchema
        schema_version = 1
        run_id = $RunId
        created_utc = $now
        updated_utc = $now
        owner_pid = $OwnerPid
        owner_process_start_utc = $ownerStart
        repository_root = $repositoryRootFull
        temp_root = $tempRootFull
        run_root = [IO.Path]::GetFullPath($runRoot)
        journal_path = [IO.Path]::GetFullPath($journalPath)
        allowed_registry_paths = @($registryPaths)
        allowed_executable_paths = @($executables)
        orchestrator_apartment = [pscustomobject][ordered]@{
            process_id = $OwnerPid
            thread_id = [Threading.Thread]::CurrentThread.ManagedThreadId
            model = $OrchestratorApartment
        }
        reentry_policy = $ReentryPolicy
        state = 'active'
        next_resource_sequence = 1
        next_event_sequence = 1
        resources = @()
        events = @()
        journal_digest = 'sha256:' + ('0' * 64)
    }
    Add-WindowsOwnedJournalEvent -Journal $journal -Event 'journal-created' -Detail "support-only; capability-credit=none"
    Write-WindowsOwnedResourceJournal -Journal $journal
    return [IO.Path]::GetFullPath($journalPath)
}

function Add-WindowsOwnedPreparedResource {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][ValidateSet('file', 'registry', 'process', 'apartment', 'callback', 'connection', 'dialog')][string]$Kind,
        [Parameter(Mandatory = $true)]$Descriptor,
        [Parameter(Mandatory = $true)]$Before,
        [Parameter(Mandatory = $true)]$Expected
    )

    $journal = Read-WindowsOwnedResourceJournal -JournalPath $JournalPath
    Assert-WindowsOwnedJournalWriter -Journal $journal
    if ([string]$journal.state -ne 'active') {
        throw "owned-resource journal '$JournalPath' cannot acquire resources in state '$($journal.state)'"
    }
    $resourceId = "$Kind-$([Guid]::NewGuid().ToString('N'))"
    $resource = [pscustomobject][ordered]@{
        sequence = [int]$journal.next_resource_sequence
        resource_id = $resourceId
        kind = $Kind
        state = 'prepared'
        prepared_utc = Get-WindowsOwnedUtcText
        active_utc = ''
        cleaned_utc = ''
        descriptor = $Descriptor
        before = $Before
        expected = $Expected
    }
    Assert-WindowsOwnedResourceDescriptor -Journal $journal -Resource $resource
    $journal.resources = @($journal.resources) + @($resource)
    $journal.next_resource_sequence = [int]$journal.next_resource_sequence + 1
    Add-WindowsOwnedJournalEvent -Journal $journal -Event 'resource-prepared' -ResourceId $resourceId -Detail $Kind
    Write-WindowsOwnedResourceJournal -Journal $journal
    return $resourceId
}

function Set-WindowsOwnedResourceActive {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][string]$ResourceId,
        $Descriptor = $null
    )

    $journal = Read-WindowsOwnedResourceJournal -JournalPath $JournalPath
    Assert-WindowsOwnedJournalWriter -Journal $journal
    $matches = @($journal.resources | Where-Object { [string]$_.resource_id -ceq $ResourceId })
    if ($matches.Count -ne 1 -or [string]$matches[0].state -ne 'prepared') {
        throw "owned-resource '$ResourceId' is not one prepared journal resource"
    }
    if ($null -ne $Descriptor) {
        $matches[0].descriptor = $Descriptor
    }
    $matches[0].state = 'active'
    $matches[0].active_utc = Get-WindowsOwnedUtcText
    Assert-WindowsOwnedResourceDescriptor -Journal $journal -Resource $matches[0]
    Add-WindowsOwnedJournalEvent -Journal $journal -Event 'resource-active' -ResourceId $ResourceId -Detail ([string]$matches[0].kind)
    Write-WindowsOwnedResourceJournal -Journal $journal
}

function Test-WindowsOwnedExactPathEqual {
    param(
        [Parameter(Mandatory = $true)][string]$Left,
        [Parameter(Mandatory = $true)][string]$Right
    )

    $comparison = if ([Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([Runtime.InteropServices.OSPlatform]::Windows)) {
        [StringComparison]::OrdinalIgnoreCase
    }
    else {
        [StringComparison]::Ordinal
    }
    return [IO.Path]::GetFullPath($Left).Equals([IO.Path]::GetFullPath($Right), $comparison)
}

function Assert-WindowsOwnedExactIdentityText {
    param(
        [AllowEmptyString()][string]$Value,
        [Parameter(Mandatory = $true)][string]$Owner,
        [switch]$AllowEmpty
    )

    if ((-not $AllowEmpty -and [string]::IsNullOrWhiteSpace($Value)) -or
        $Value.IndexOfAny([char[]]'*?[]') -ge 0 -or
        $Value -match '(?i)^(all|any|global|by-name|recursive|subtree|window-class)$') {
        throw "$Owner must be one exact, non-wildcard identity"
    }
}

function Assert-WindowsOwnedSnapshotSchema {
    param(
        [Parameter(Mandatory = $true)]$Snapshot,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    Assert-WindowsOwnedExactProperties -Value $Snapshot -Expected @('key_exists', 'exists', 'kind', 'data_base64') -Owner $Owner
    if ([string]$Snapshot.exists -notin @('True', 'False') -or
        [string]$Snapshot.key_exists -notin @('True', 'False') -or
        ([bool]$Snapshot.exists -and [string]$Snapshot.kind -notin @('String', 'ExpandString', 'Binary', 'DWord', 'QWord', 'MultiString')) -or
        (-not [bool]$Snapshot.exists -and (-not [string]::IsNullOrEmpty([string]$Snapshot.kind) -or -not [string]::IsNullOrEmpty([string]$Snapshot.data_base64)))) {
        throw "$Owner has an invalid exact registry-value snapshot"
    }
    if ([bool]$Snapshot.exists) {
        try {
            [void][Convert]::FromBase64String([string]$Snapshot.data_base64)
        }
        catch {
            throw "$Owner has invalid base64 registry data"
        }
    }
}

function Assert-WindowsOwnedResourceDescriptor {
    param(
        [Parameter(Mandatory = $true)]$Journal,
        [Parameter(Mandatory = $true)]$Resource
    )

    $owner = "owned-resource '$($Resource.resource_id)'"
    $kind = [string]$Resource.kind
    switch ($kind) {
        'file' {
            Assert-WindowsOwnedExactProperties -Value $Resource.descriptor -Expected @('path', 'mutation_mode') -Owner "$owner file descriptor"
            $path = Assert-WindowsOwnedConfinedPath -Journal $Journal -Path ([string]$Resource.descriptor.path) -Owner "$owner file"
            if ([string]$Resource.descriptor.mutation_mode -cne 'create-only' -or
                -not (Test-WindowsOwnedExactPathEqual -Left $path -Right ([string]$Resource.descriptor.path))) {
                throw "$owner file policy permits only one canonical create-only path"
            }
            foreach ($pair in @(@($Resource.before, 'before'), @($Resource.expected, 'expected'))) {
                Assert-WindowsOwnedExactProperties -Value $pair[0] -Expected @('exists', 'length', 'sha256') -Owner "$owner file $($pair[1])"
                if ([string]$pair[0].exists -notin @('True', 'False') -or [long]$pair[0].length -lt 0 -or
                    ([bool]$pair[0].exists -and [string]$pair[0].sha256 -notmatch '^sha256:[0-9a-f]{64}$') -or
                    (-not [bool]$pair[0].exists -and -not [string]::IsNullOrEmpty([string]$pair[0].sha256))) {
                    throw "$owner file $($pair[1]) snapshot is invalid"
                }
            }
            if ([bool]$Resource.before.exists -or -not [bool]$Resource.expected.exists) {
                throw "$owner file must describe an absent-to-exact create-only mutation"
            }
        }
        'registry' {
            Assert-WindowsOwnedExactProperties -Value $Resource.descriptor -Expected @('path', 'value_name', 'mutation_mode') -Owner "$owner registry descriptor"
            $path = ConvertTo-WindowsOwnedRegistryPath -Path ([string]$Resource.descriptor.path) -Owner "$owner registry path"
            Assert-WindowsOwnedExactIdentityText -Value ([string]$Resource.descriptor.value_name) -Owner "$owner registry value"
            if ([string]$Resource.descriptor.mutation_mode -cne 'exact-value' -or
                -not (@($Journal.allowed_registry_paths) | Where-Object { [string]$_ -ieq $path })) {
                throw "$owner registry path is not one exact HKCU allowlist entry"
            }
            Assert-WindowsOwnedSnapshotSchema -Snapshot $Resource.before -Owner "$owner registry before"
            Assert-WindowsOwnedSnapshotSchema -Snapshot $Resource.expected -Owner "$owner registry expected"
            if (-not [bool]$Resource.expected.exists) {
                throw "$owner registry mutation must journal one exact resulting value"
            }
        }
        'process' {
            Assert-WindowsOwnedExactProperties -Value $Resource.descriptor -Expected @(
                'executable_path', 'pid', 'process_start_utc', 'arguments_sha256',
                'activation_path', 'parent_pid', 'harmless_child', 'self_timeout_seconds'
            ) -Owner "$owner process descriptor"
            Assert-WindowsOwnedExactProperties -Value $Resource.before -Expected @('exists') -Owner "$owner process before"
            Assert-WindowsOwnedExactProperties -Value $Resource.expected -Expected @('recorded') -Owner "$owner process expected"
            $executable = [IO.Path]::GetFullPath([string]$Resource.descriptor.executable_path)
            $activation = Assert-WindowsOwnedConfinedPath -Journal $Journal -Path ([string]$Resource.descriptor.activation_path) -Owner "$owner process activation"
            if (-not (@($Journal.allowed_executable_paths) | Where-Object { Test-WindowsOwnedExactPathEqual -Left ([string]$_) -Right $executable }) -or
                [string]$Resource.descriptor.arguments_sha256 -notmatch '^sha256:[0-9a-f]{64}$' -or
                [int]$Resource.descriptor.parent_pid -le 0 -or
                -not [bool]$Resource.descriptor.harmless_child -or
                [int]$Resource.descriptor.self_timeout_seconds -lt 1 -or [int]$Resource.descriptor.self_timeout_seconds -gt 60 -or
                [bool]$Resource.before.exists -or -not [bool]$Resource.expected.recorded -or
                -not (Test-WindowsOwnedExactPathEqual -Left $activation -Right ([string]$Resource.descriptor.activation_path))) {
                throw "$owner process contract is not exact, allowlisted, harmless, and self-expiring"
            }
            if ([string]$Resource.state -eq 'prepared') {
                if ([int]$Resource.descriptor.pid -ne 0 -or -not [string]::IsNullOrEmpty([string]$Resource.descriptor.process_start_utc)) {
                    throw "$owner prepared process must remain inert and unassigned"
                }
            }
            elseif ([int]$Resource.descriptor.pid -eq 0 -and [string]$Resource.state -eq 'cleaned' -and
                [string]::IsNullOrEmpty([string]$Resource.active_utc) -and [string]::IsNullOrEmpty([string]$Resource.descriptor.process_start_utc)) {
                # A crash-safe prepared record can be cleaned without ever
                # assigning or activating a child PID.
            }
            elseif ([int]$Resource.descriptor.pid -le 0 -or [string]$Resource.descriptor.process_start_utc -notmatch '^\d{4}-\d{2}-\d{2}T') {
                throw "$owner active/terminal process must retain its exact PID/start identity"
            }
        }
        'apartment' {
            Assert-WindowsOwnedExactProperties -Value $Resource.descriptor -Expected @(
                'process_id', 'thread_id', 'model', 'com_initialization', 'reentry_policy', 'message_pump', 'max_reentry_depth'
            ) -Owner "$owner apartment descriptor"
            Assert-WindowsOwnedExactProperties -Value $Resource.before -Expected @('registered') -Owner "$owner apartment before"
            Assert-WindowsOwnedExactProperties -Value $Resource.expected -Expected @('registered') -Owner "$owner apartment expected"
            if ([int]$Resource.descriptor.process_id -ne [int]$Journal.owner_pid -or [int]$Resource.descriptor.thread_id -le 0 -or
                [string]$Resource.descriptor.model -notin @('STA', 'MTA', 'none') -or
                [string]$Resource.descriptor.com_initialization -notin @('logical-only-no-com', 'CoInitializeEx-owned', 'caller-owned') -or
                [string]$Resource.descriptor.reentry_policy -notin @('reject', 'same-apartment-synchronous', 'declared-nested') -or
                [string]$Resource.descriptor.message_pump -notin @('none', 'owned-loop', 'caller-loop') -or
                [int]$Resource.descriptor.max_reentry_depth -lt 0 -or [int]$Resource.descriptor.max_reentry_depth -gt 16 -or
                [bool]$Resource.before.registered -or -not [bool]$Resource.expected.registered) {
                throw "$owner apartment lifecycle declaration is incomplete"
            }
        }
        'callback' {
            Assert-WindowsOwnedExactProperties -Value $Resource.descriptor -Expected @(
                'apartment_resource_id', 'session_id', 'thunk_id', 'owning_thread_id',
                'retention', 'wrong_thread_policy', 'stale_policy'
            ) -Owner "$owner callback descriptor"
            Assert-WindowsOwnedExactProperties -Value $Resource.before -Expected @('registered') -Owner "$owner callback before"
            Assert-WindowsOwnedExactProperties -Value $Resource.expected -Expected @('registered') -Owner "$owner callback expected"
            foreach ($name in @('apartment_resource_id', 'session_id', 'thunk_id')) {
                Assert-WindowsOwnedExactIdentityText -Value ([string]$Resource.descriptor.$name) -Owner "$owner callback $name"
            }
            $apartment = @($Journal.resources | Where-Object { [string]$_.resource_id -ceq [string]$Resource.descriptor.apartment_resource_id -and [string]$_.kind -ceq 'apartment' })
            if ($apartment.Count -ne 1 -or [int]$Resource.descriptor.owning_thread_id -ne [int]$apartment[0].descriptor.thread_id -or
                [string]$Resource.descriptor.retention -cne 'strong-until-unregistered' -or
                [string]$Resource.descriptor.wrong_thread_policy -cne 'reject' -or
                [string]$Resource.descriptor.stale_policy -cne 'reject-after-retire' -or
                [bool]$Resource.before.registered -or -not [bool]$Resource.expected.registered) {
                throw "$owner callback lifetime/apartment declaration is invalid"
            }
        }
        'connection' {
            Assert-WindowsOwnedExactProperties -Value $Resource.descriptor -Expected @(
                'apartment_resource_id', 'callback_resource_id', 'source_identity', 'sink_identity',
                'connection_point_iid', 'cookie', 'writeback_policy'
            ) -Owner "$owner connection descriptor"
            Assert-WindowsOwnedExactProperties -Value $Resource.before -Expected @('advised') -Owner "$owner connection before"
            Assert-WindowsOwnedExactProperties -Value $Resource.expected -Expected @('advised') -Owner "$owner connection expected"
            foreach ($name in @('apartment_resource_id', 'callback_resource_id', 'source_identity', 'sink_identity', 'connection_point_iid')) {
                Assert-WindowsOwnedExactIdentityText -Value ([string]$Resource.descriptor.$name) -Owner "$owner connection $name"
            }
            $apartment = @($Journal.resources | Where-Object { [string]$_.resource_id -ceq [string]$Resource.descriptor.apartment_resource_id -and [string]$_.kind -ceq 'apartment' })
            $callback = @($Journal.resources | Where-Object { [string]$_.resource_id -ceq [string]$Resource.descriptor.callback_resource_id -and [string]$_.kind -ceq 'callback' })
            if ($apartment.Count -ne 1 -or $callback.Count -ne 1 -or [int64]$Resource.descriptor.cookie -le 0 -or
                [string]$Resource.descriptor.writeback_policy -notin @('copy-in-copy-out', 'none') -or
                [bool]$Resource.before.advised -or -not [bool]$Resource.expected.advised) {
                throw "$owner connection lifetime declaration is invalid"
            }
        }
        'dialog' {
            Assert-WindowsOwnedExactProperties -Value $Resource.descriptor -Expected @(
                'process_resource_id', 'process_id', 'process_start_utc', 'uia_runtime_id',
                'native_window_handle', 'title_sha256', 'allowed_action'
            ) -Owner "$owner dialog descriptor"
            Assert-WindowsOwnedExactProperties -Value $Resource.before -Expected @('registered') -Owner "$owner dialog before"
            Assert-WindowsOwnedExactProperties -Value $Resource.expected -Expected @('registered') -Owner "$owner dialog expected"
            Assert-WindowsOwnedExactIdentityText -Value ([string]$Resource.descriptor.process_resource_id) -Owner "$owner dialog process"
            Assert-WindowsOwnedExactIdentityText -Value ([string]$Resource.descriptor.uia_runtime_id) -Owner "$owner dialog UIA runtime ID"
            $process = @($Journal.resources | Where-Object { [string]$_.resource_id -ceq [string]$Resource.descriptor.process_resource_id -and [string]$_.kind -ceq 'process' })
            if ($process.Count -ne 1 -or [int]$Resource.descriptor.process_id -ne [int]$process[0].descriptor.pid -or
                [string]$Resource.descriptor.process_start_utc -cne [string]$process[0].descriptor.process_start_utc -or
                [int64]$Resource.descriptor.native_window_handle -le 0 -or
                [string]$Resource.descriptor.title_sha256 -notmatch '^sha256:[0-9a-f]{64}$' -or
                [string]$Resource.descriptor.allowed_action -notin @('observe-only', 'dismiss-exact') -or
                [bool]$Resource.before.registered -or -not [bool]$Resource.expected.registered) {
                throw "$owner dialog is not process-scoped to one exact recorded UIA identity"
            }
        }
        default {
            throw "$owner has unsupported resource kind '$kind'"
        }
    }
}

function Assert-WindowsOwnedCleanupIntent {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][ValidateSet('file', 'registry', 'process', 'dialog')][string]$Kind,
        [Parameter(Mandatory = $true)][string]$SelectorMode,
        [Parameter(Mandatory = $true)][string]$ResourceId,
        [Parameter(Mandatory = $true)][string]$Selector
    )

    $required = @{
        file = 'exact-recorded-file'
        registry = 'exact-recorded-value'
        process = 'exact-recorded-pid-start'
        dialog = 'exact-recorded-process-uia'
    }[$Kind]
    Assert-WindowsOwnedExactIdentityText -Value $ResourceId -Owner "$Kind cleanup resource ID"
    Assert-WindowsOwnedExactIdentityText -Value $Selector -Owner "$Kind cleanup selector"
    if ($SelectorMode -cne $required) {
        throw "$Kind cleanup rejects blanket/by-name/recursive selectors; expected '$required'"
    }
    $journal = Read-WindowsOwnedResourceJournal -JournalPath $JournalPath
    $resource = Get-WindowsOwnedRecordedResource -Journal $journal -ResourceId $ResourceId -Kind $Kind
    $expectedSelector = switch ($Kind) {
        'file' { [IO.Path]::GetFullPath([string]$resource.descriptor.path); break }
        'registry' { "$(ConvertTo-WindowsOwnedRegistryPath -Path ([string]$resource.descriptor.path))::$([string]$resource.descriptor.value_name)"; break }
        'process' { "pid=$([int]$resource.descriptor.pid);start=$([string]$resource.descriptor.process_start_utc)"; break }
        'dialog' {
            "pid=$([int]$resource.descriptor.process_id);start=$([string]$resource.descriptor.process_start_utc);uia=$([string]$resource.descriptor.uia_runtime_id);hwnd=$([int64]$resource.descriptor.native_window_handle)"; break
        }
    }
    $matches = if ($Kind -eq 'file') {
        Test-WindowsOwnedExactPathEqual -Left $Selector -Right $expectedSelector
    }
    elseif ($Kind -eq 'registry') {
        $Selector -ieq $expectedSelector
    }
    else {
        $Selector -ceq $expectedSelector
    }
    if (-not $matches) {
        throw "$Kind cleanup selector does not match the exact recorded resource identity"
    }
    return $true
}

function Get-WindowsOwnedFileSnapshot {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return [pscustomobject][ordered]@{ exists = $false; length = 0L; sha256 = '' }
    }
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "owned file '$Path' is not a regular file"
    }
    $bytes = [IO.File]::ReadAllBytes($Path)
    return [pscustomobject][ordered]@{
        exists = $true
        length = [long]$bytes.Length
        sha256 = Get-WindowsOwnedSha256Bytes -Bytes $bytes
    }
}

function Test-WindowsOwnedObjectEqual {
    param($Left, $Right)

    return ($Left | ConvertTo-Json -Depth 16 -Compress) -ceq ($Right | ConvertTo-Json -Depth 16 -Compress)
}

function New-WindowsOwnedFile {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][byte[]]$Bytes
    )

    $journal = Read-WindowsOwnedResourceJournal -JournalPath $JournalPath
    Assert-WindowsOwnedJournalWriter -Journal $journal
    $full = Assert-WindowsOwnedConfinedPath -Journal $journal -Path $Path -Owner 'owned file creation'
    $parent = Split-Path -Parent $full
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        throw "owned file parent '$parent' must already exist"
    }
    $before = Get-WindowsOwnedFileSnapshot -Path $full
    if ([bool]$before.exists) {
        throw "owned file '$full' already exists; create-only policy refuses overwrite"
    }
    $expected = [pscustomobject][ordered]@{
        exists = $true
        length = [long]$Bytes.Length
        sha256 = Get-WindowsOwnedSha256Bytes -Bytes $Bytes
    }
    $descriptor = [pscustomobject][ordered]@{ path = $full; mutation_mode = 'create-only' }
    $resourceId = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Kind file -Descriptor $descriptor -Before $before -Expected $expected
    $stream = [IO.FileStream]::new($full, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None, 4096, [IO.FileOptions]::WriteThrough)
    try {
        $stream.Write($Bytes, 0, $Bytes.Length)
        $stream.Flush($true)
    }
    finally {
        $stream.Dispose()
    }
    Set-WindowsOwnedResourceActive -JournalPath $JournalPath -ResourceId $resourceId
    return $resourceId
}

function ConvertTo-WindowsOwnedRegistryData {
    param(
        [Parameter(Mandatory = $true)]$Value,
        [Parameter(Mandatory = $true)][Microsoft.Win32.RegistryValueKind]$Kind
    )

    $bytes = switch ($Kind) {
        ([Microsoft.Win32.RegistryValueKind]::String) { [Text.UTF8Encoding]::new($false).GetBytes([string]$Value); break }
        ([Microsoft.Win32.RegistryValueKind]::ExpandString) { [Text.UTF8Encoding]::new($false).GetBytes([string]$Value); break }
        ([Microsoft.Win32.RegistryValueKind]::Binary) { [byte[]]$Value; break }
        ([Microsoft.Win32.RegistryValueKind]::DWord) { [BitConverter]::GetBytes([int]$Value); break }
        ([Microsoft.Win32.RegistryValueKind]::QWord) { [BitConverter]::GetBytes([long]$Value); break }
        ([Microsoft.Win32.RegistryValueKind]::MultiString) {
            [Text.UTF8Encoding]::new($false).GetBytes((@([string[]]$Value) | ConvertTo-Json -Compress)); break
        }
        default { throw "registry value kind '$Kind' is not supported by the exact journal codec" }
    }
    return [Convert]::ToBase64String($bytes)
}

function ConvertFrom-WindowsOwnedRegistryData {
    param(
        [Parameter(Mandatory = $true)][string]$DataBase64,
        [Parameter(Mandatory = $true)][Microsoft.Win32.RegistryValueKind]$Kind
    )

    $bytes = [Convert]::FromBase64String($DataBase64)
    switch ($Kind) {
        ([Microsoft.Win32.RegistryValueKind]::String) { return [Text.UTF8Encoding]::new($false, $true).GetString($bytes) }
        ([Microsoft.Win32.RegistryValueKind]::ExpandString) { return [Text.UTF8Encoding]::new($false, $true).GetString($bytes) }
        ([Microsoft.Win32.RegistryValueKind]::Binary) { return $bytes }
        ([Microsoft.Win32.RegistryValueKind]::DWord) {
            if ($bytes.Length -ne 4) { throw 'DWord registry snapshot must contain four bytes' }
            return [BitConverter]::ToInt32($bytes, 0)
        }
        ([Microsoft.Win32.RegistryValueKind]::QWord) {
            if ($bytes.Length -ne 8) { throw 'QWord registry snapshot must contain eight bytes' }
            return [BitConverter]::ToInt64($bytes, 0)
        }
        ([Microsoft.Win32.RegistryValueKind]::MultiString) {
            $value = [Text.UTF8Encoding]::new($false, $true).GetString($bytes) | ConvertFrom-Json
            return [string[]]@($value)
        }
        default { throw "registry value kind '$Kind' is not supported by the exact journal codec" }
    }
}

function Get-WindowsOwnedRegistrySubKey {
    param([Parameter(Mandatory = $true)][string]$Path)

    $normalized = ConvertTo-WindowsOwnedRegistryPath -Path $Path
    return $normalized.Substring('HKCU\'.Length)
}

function Get-WindowsOwnedRegistryValueSnapshot {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ValueName
    )

    if (-not [Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([Runtime.InteropServices.OSPlatform]::Windows)) {
        throw 'the owned registry policy helper is Windows-only'
    }
    $subKey = Get-WindowsOwnedRegistrySubKey -Path $Path
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($subKey, $false)
    if ($null -eq $key) {
        return [pscustomobject][ordered]@{ key_exists = $false; exists = $false; kind = ''; data_base64 = '' }
    }
    try {
        $exists = @($key.GetValueNames() | Where-Object { $_ -ieq $ValueName }).Count -eq 1
        if (-not $exists) {
            return [pscustomobject][ordered]@{ key_exists = $true; exists = $false; kind = ''; data_base64 = '' }
        }
        $kind = $key.GetValueKind($ValueName)
        $value = $key.GetValue($ValueName, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        return [pscustomobject][ordered]@{
            key_exists = $true
            exists = $true
            kind = $kind.ToString()
            data_base64 = ConvertTo-WindowsOwnedRegistryData -Value $value -Kind $kind
        }
    }
    finally {
        $key.Dispose()
    }
}

function New-WindowsOwnedRegistryValueSnapshot {
    param(
        [Parameter(Mandatory = $true)]$Value,
        [Parameter(Mandatory = $true)][Microsoft.Win32.RegistryValueKind]$Kind,
        [bool]$KeyExists = $true
    )

    return [pscustomobject][ordered]@{
        key_exists = $KeyExists
        exists = $true
        kind = $Kind.ToString()
        data_base64 = ConvertTo-WindowsOwnedRegistryData -Value $Value -Kind $Kind
    }
}

function Set-WindowsOwnedRegistryValueRaw {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ValueName,
        [Parameter(Mandatory = $true)]$Snapshot
    )

    $subKey = Get-WindowsOwnedRegistrySubKey -Path $Path
    if ([bool]$Snapshot.exists) {
        $kind = [Microsoft.Win32.RegistryValueKind]([Enum]::Parse([Microsoft.Win32.RegistryValueKind], [string]$Snapshot.kind, $false))
        $value = ConvertFrom-WindowsOwnedRegistryData -DataBase64 ([string]$Snapshot.data_base64) -Kind $kind
        $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($subKey, $true)
        try {
            $key.SetValue($ValueName, $value, $kind)
            $key.Flush()
        }
        finally {
            $key.Dispose()
        }
        return
    }
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($subKey, $true)
    if ($null -ne $key) {
        try {
            $key.DeleteValue($ValueName, $false)
            $key.Flush()
        }
        finally {
            $key.Dispose()
        }
    }
}

function Remove-WindowsOwnedEmptyRegistryKeyIfCreated {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Before
    )

    if ([bool]$Before.key_exists) {
        return
    }
    $subKey = Get-WindowsOwnedRegistrySubKey -Path $Path
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($subKey, $false)
    if ($null -eq $key) {
        return
    }
    try {
        $empty = $key.ValueCount -eq 0 -and $key.SubKeyCount -eq 0
    }
    finally {
        $key.Dispose()
    }
    if ($empty) {
        [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKey($subKey, $false)
    }
}

function Set-WindowsOwnedRegistryValue {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ValueName,
        [Parameter(Mandatory = $true)]$Value,
        [Microsoft.Win32.RegistryValueKind]$Kind = [Microsoft.Win32.RegistryValueKind]::String
    )

    $journal = Read-WindowsOwnedResourceJournal -JournalPath $JournalPath
    Assert-WindowsOwnedJournalWriter -Journal $journal
    $normalized = ConvertTo-WindowsOwnedRegistryPath -Path $Path -Owner 'owned registry mutation'
    Assert-WindowsOwnedExactIdentityText -Value $ValueName -Owner 'owned registry value name'
    if (-not (@($journal.allowed_registry_paths) | Where-Object { [string]$_ -ieq $normalized })) {
        throw "registry path '$normalized' is not an exact journal allowlist entry"
    }
    $before = Get-WindowsOwnedRegistryValueSnapshot -Path $normalized -ValueName $ValueName
    $expected = New-WindowsOwnedRegistryValueSnapshot -Value $Value -Kind $Kind -KeyExists $true
    $descriptor = [pscustomobject][ordered]@{ path = $normalized; value_name = $ValueName; mutation_mode = 'exact-value' }
    $resourceId = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Kind registry -Descriptor $descriptor -Before $before -Expected $expected
    Set-WindowsOwnedRegistryValueRaw -Path $normalized -ValueName $ValueName -Snapshot $expected
    Set-WindowsOwnedResourceActive -JournalPath $JournalPath -ResourceId $resourceId
    return $resourceId
}

function Get-WindowsOwnedRecordedResource {
    param(
        [Parameter(Mandatory = $true)]$Journal,
        [Parameter(Mandatory = $true)][string]$ResourceId,
        [Parameter(Mandatory = $true)][string]$Kind,
        [switch]$RequireActive
    )

    $matches = @($Journal.resources | Where-Object { [string]$_.resource_id -ceq $ResourceId -and [string]$_.kind -ceq $Kind })
    if ($matches.Count -ne 1 -or ($RequireActive -and [string]$matches[0].state -ne 'active')) {
        throw "owned $Kind resource '$ResourceId' is not one exact active journal record"
    }
    return $matches[0]
}

function Start-WindowsOwnedHarmlessChild {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][string]$ExecutablePath,
        [Parameter(Mandatory = $true)][string]$ScriptPath,
        [Parameter(Mandatory = $true)][string]$ActivationPath,
        [string[]]$AdditionalArguments = @(),
        [ValidateRange(1, 60)][int]$SelfTimeoutSeconds = 30
    )

    $journal = Read-WindowsOwnedResourceJournal -JournalPath $JournalPath
    Assert-WindowsOwnedJournalWriter -Journal $journal
    $executable = [IO.Path]::GetFullPath($ExecutablePath)
    if (-not (@($journal.allowed_executable_paths) | Where-Object { Test-WindowsOwnedExactPathEqual -Left ([string]$_) -Right $executable })) {
        throw "child executable '$executable' is not an exact journal allowlist entry"
    }
    $script = Assert-WindowsOwnedConfinedPath -Journal $journal -Path $ScriptPath -Owner 'owned child script'
    $activation = Assert-WindowsOwnedConfinedPath -Journal $journal -Path $ActivationPath -Owner 'owned child activation'
    if (-not (Test-Path -LiteralPath $script -PathType Leaf) -or (Test-Path -LiteralPath $activation)) {
        throw 'owned child requires one existing confined script and one absent confined activation path'
    }
    $scriptResource = @($journal.resources | Where-Object {
        [string]$_.kind -ceq 'file' -and [string]$_.state -ceq 'active' -and
        (Test-WindowsOwnedExactPathEqual -Left ([string]$_.descriptor.path) -Right $script)
    })
    if ($scriptResource.Count -ne 1) {
        throw 'owned child script must itself be one exact active journaled file'
    }
    foreach ($argument in $AdditionalArguments) {
        if ($null -eq $argument -or [string]$argument -match '[\x00\r\n]') {
            throw 'owned child arguments must be explicit scalar values'
        }
    }
    $arguments = @('-NoProfile', '-NonInteractive', '-File', $script, '-ActivationPath', $activation, '-SelfTimeoutSeconds', [string]$SelfTimeoutSeconds) + @($AdditionalArguments)
    $descriptor = [pscustomobject][ordered]@{
        executable_path = $executable
        pid = 0
        process_start_utc = ''
        arguments_sha256 = Get-WindowsOwnedSha256Text -Text ($arguments | ConvertTo-Json -Compress)
        activation_path = $activation
        parent_pid = $PID
        harmless_child = $true
        self_timeout_seconds = $SelfTimeoutSeconds
    }
    $before = [pscustomobject][ordered]@{ exists = $false }
    $expected = [pscustomobject][ordered]@{ recorded = $true }
    $resourceId = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Kind process -Descriptor $descriptor -Before $before -Expected $expected

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $executable
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    foreach ($argument in $arguments) {
        [void]$startInfo.ArgumentList.Add([string]$argument)
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    try {
        if (-not $process.Start()) {
            throw 'owned harmless child did not start'
        }
        $descriptor.pid = $process.Id
        $descriptor.process_start_utc = $process.StartTime.ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ss.fffffffZ', [Globalization.CultureInfo]::InvariantCulture)
        Set-WindowsOwnedResourceActive -JournalPath $JournalPath -ResourceId $resourceId -Descriptor $descriptor
    }
    catch {
        if ($null -ne $process -and -not $process.HasExited) {
            try { $process.Kill($true); $process.WaitForExit(5000) } catch { }
        }
        throw
    }
    finally {
        $process.Dispose()
    }
    [void](New-WindowsOwnedFile -JournalPath $JournalPath -Path $activation -Bytes ([Text.UTF8Encoding]::new($false).GetBytes($resourceId)))
    return $resourceId
}

function Register-WindowsOwnedApartment {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [ValidateSet('STA', 'MTA', 'none')][string]$Model = 'none',
        [ValidateSet('logical-only-no-com', 'CoInitializeEx-owned', 'caller-owned')][string]$ComInitialization = 'logical-only-no-com',
        [ValidateSet('reject', 'same-apartment-synchronous', 'declared-nested')][string]$ReentryPolicy = 'reject',
        [ValidateSet('none', 'owned-loop', 'caller-loop')][string]$MessagePump = 'none',
        [ValidateRange(0, 16)][int]$MaxReentryDepth = 0
    )

    $journal = Read-WindowsOwnedResourceJournal -JournalPath $JournalPath
    Assert-WindowsOwnedJournalWriter -Journal $journal
    if ($Model -cne [string]$journal.orchestrator_apartment.model -or $ReentryPolicy -cne [string]$journal.reentry_policy) {
        throw 'owned apartment registration must match the journal orchestrator apartment/reentry declaration'
    }
    $descriptor = [pscustomobject][ordered]@{
        process_id = [int]$journal.owner_pid
        thread_id = [int]$journal.orchestrator_apartment.thread_id
        model = $Model
        com_initialization = $ComInitialization
        reentry_policy = $ReentryPolicy
        message_pump = $MessagePump
        max_reentry_depth = $MaxReentryDepth
    }
    $id = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Kind apartment -Descriptor $descriptor `
        -Before ([pscustomobject][ordered]@{ registered = $false }) -Expected ([pscustomobject][ordered]@{ registered = $true })
    Set-WindowsOwnedResourceActive -JournalPath $JournalPath -ResourceId $id
    return $id
}

function Register-WindowsOwnedCallback {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][string]$ApartmentResourceId,
        [Parameter(Mandatory = $true)][string]$SessionId,
        [Parameter(Mandatory = $true)][string]$ThunkId
    )

    $journal = Read-WindowsOwnedResourceJournal -JournalPath $JournalPath
    Assert-WindowsOwnedJournalWriter -Journal $journal
    $apartment = Get-WindowsOwnedRecordedResource -Journal $journal -ResourceId $ApartmentResourceId -Kind apartment -RequireActive
    $descriptor = [pscustomobject][ordered]@{
        apartment_resource_id = $ApartmentResourceId
        session_id = $SessionId
        thunk_id = $ThunkId
        owning_thread_id = [int]$apartment.descriptor.thread_id
        retention = 'strong-until-unregistered'
        wrong_thread_policy = 'reject'
        stale_policy = 'reject-after-retire'
    }
    $id = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Kind callback -Descriptor $descriptor `
        -Before ([pscustomobject][ordered]@{ registered = $false }) -Expected ([pscustomobject][ordered]@{ registered = $true })
    Set-WindowsOwnedResourceActive -JournalPath $JournalPath -ResourceId $id
    return $id
}

function Register-WindowsOwnedConnection {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][string]$ApartmentResourceId,
        [Parameter(Mandatory = $true)][string]$CallbackResourceId,
        [Parameter(Mandatory = $true)][string]$SourceIdentity,
        [Parameter(Mandatory = $true)][string]$SinkIdentity,
        [Parameter(Mandatory = $true)][string]$ConnectionPointIid,
        [ValidateRange(1, [int]::MaxValue)][int]$Cookie,
        [ValidateSet('copy-in-copy-out', 'none')][string]$WritebackPolicy = 'none'
    )

    $journal = Read-WindowsOwnedResourceJournal -JournalPath $JournalPath
    Assert-WindowsOwnedJournalWriter -Journal $journal
    [void](Get-WindowsOwnedRecordedResource -Journal $journal -ResourceId $ApartmentResourceId -Kind apartment -RequireActive)
    $callback = Get-WindowsOwnedRecordedResource -Journal $journal -ResourceId $CallbackResourceId -Kind callback -RequireActive
    if ([string]$callback.descriptor.apartment_resource_id -cne $ApartmentResourceId) {
        throw 'owned connection callback must belong to the declared apartment'
    }
    $descriptor = [pscustomobject][ordered]@{
        apartment_resource_id = $ApartmentResourceId
        callback_resource_id = $CallbackResourceId
        source_identity = $SourceIdentity
        sink_identity = $SinkIdentity
        connection_point_iid = $ConnectionPointIid
        cookie = $Cookie
        writeback_policy = $WritebackPolicy
    }
    $id = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Kind connection -Descriptor $descriptor `
        -Before ([pscustomobject][ordered]@{ advised = $false }) -Expected ([pscustomobject][ordered]@{ advised = $true })
    Set-WindowsOwnedResourceActive -JournalPath $JournalPath -ResourceId $id
    return $id
}

function Register-WindowsOwnedDialogRepresentation {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][string]$ProcessResourceId,
        [Parameter(Mandatory = $true)][string]$UiaRuntimeId,
        [ValidateRange(1, [long]::MaxValue)][long]$NativeWindowHandle,
        [Parameter(Mandatory = $true)][string]$Title,
        [ValidateSet('observe-only', 'dismiss-exact')][string]$AllowedAction = 'observe-only'
    )

    $journal = Read-WindowsOwnedResourceJournal -JournalPath $JournalPath
    Assert-WindowsOwnedJournalWriter -Journal $journal
    $process = Get-WindowsOwnedRecordedResource -Journal $journal -ResourceId $ProcessResourceId -Kind process -RequireActive
    $descriptor = [pscustomobject][ordered]@{
        process_resource_id = $ProcessResourceId
        process_id = [int]$process.descriptor.pid
        process_start_utc = [string]$process.descriptor.process_start_utc
        uia_runtime_id = $UiaRuntimeId
        native_window_handle = $NativeWindowHandle
        title_sha256 = Get-WindowsOwnedSha256Text -Text $Title
        allowed_action = $AllowedAction
    }
    $id = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Kind dialog -Descriptor $descriptor `
        -Before ([pscustomobject][ordered]@{ registered = $false }) -Expected ([pscustomobject][ordered]@{ registered = $true })
    Set-WindowsOwnedResourceActive -JournalPath $JournalPath -ResourceId $id
    return $id
}

function Get-WindowsOwnedProcessExecutablePath {
    param([Parameter(Mandatory = $true)][int]$ProcessId)

    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if ($null -eq $process) {
        return $null
    }
    try {
        return [IO.Path]::GetFullPath([string]$process.Path)
    }
    catch {
        return $null
    }
    finally {
        $process.Dispose()
    }
}

function Invoke-WindowsOwnedSingleResourceCleanup {
    param(
        [Parameter(Mandatory = $true)]$Journal,
        [Parameter(Mandatory = $true)]$Resource
    )

    switch ([string]$Resource.kind) {
        'file' {
            $path = Assert-WindowsOwnedConfinedPath -Journal $Journal -Path ([string]$Resource.descriptor.path) -Owner 'owned file cleanup'
            $actual = Get-WindowsOwnedFileSnapshot -Path $path
            if (Test-WindowsOwnedObjectEqual -Left $actual -Right $Resource.before) {
                return 'already-before'
            }
            if (-not (Test-WindowsOwnedObjectEqual -Left $actual -Right $Resource.expected)) {
                throw "owned file '$path' drifted from both its before and expected snapshots"
            }
            [IO.File]::Delete($path)
            return 'delete-exact-file'
        }
        'registry' {
            $path = ConvertTo-WindowsOwnedRegistryPath -Path ([string]$Resource.descriptor.path) -Owner 'owned registry cleanup'
            $name = [string]$Resource.descriptor.value_name
            $actual = Get-WindowsOwnedRegistryValueSnapshot -Path $path -ValueName $name
            if (Test-WindowsOwnedObjectEqual -Left $actual -Right $Resource.before) {
                return 'already-before'
            }
            if (-not (Test-WindowsOwnedObjectEqual -Left $actual -Right $Resource.expected)) {
                throw "owned registry value '$path::$name' drifted from both its before and expected snapshots"
            }
            Set-WindowsOwnedRegistryValueRaw -Path $path -ValueName $name -Snapshot $Resource.before
            Remove-WindowsOwnedEmptyRegistryKeyIfCreated -Path $path -Before $Resource.before
            return 'restore-exact-registry-value'
        }
        'process' {
            $pidValue = [int]$Resource.descriptor.pid
            if ($pidValue -eq 0) {
                return 'prepared-child-never-activated'
            }
            $start = [string]$Resource.descriptor.process_start_utc
            $process = Get-Process -Id $pidValue -ErrorAction SilentlyContinue
            if ($null -eq $process) {
                return 'recorded-child-already-exited'
            }
            try {
                try {
                    $actualStart = $process.StartTime.ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ss.fffffffZ', [Globalization.CultureInfo]::InvariantCulture)
                    $actualExecutable = [IO.Path]::GetFullPath([string]$process.Path)
                }
                catch {
                    if ($process.HasExited) {
                        return 'recorded-child-already-exited'
                    }
                    throw "owned child PID '$pidValue' has an unverifiable start/executable identity"
                }
                if ($actualStart -cne $start) {
                    return 'recorded-child-already-exited-or-pid-reused'
                }
                if (-not (Test-WindowsOwnedExactPathEqual -Left $actualExecutable -Right ([string]$Resource.descriptor.executable_path))) {
                    throw "owned child PID '$pidValue' has an unexpected executable identity"
                }
                if (-not $process.HasExited) {
                    $process.Kill($true)
                    if (-not $process.WaitForExit(10000)) {
                        throw "owned child PID '$pidValue' did not exit after exact-PID cleanup"
                    }
                }
            }
            finally {
                $process.Dispose()
            }
            return 'stop-exact-pid-start-executable'
        }
        'dialog' { return 'retire-exact-process-uia-representation' }
        'connection' { return 'unadvise-exact-cookie-before-callback-retire' }
        'callback' { return 'retire-callback-after-unadvise' }
        'apartment' { return 'retire-apartment-after-callbacks' }
        default { throw "unsupported owned cleanup kind '$($Resource.kind)'" }
    }
}

function Invoke-WindowsOwnedResourceCleanup {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [switch]$RecoveryMode
    )

    $journal = Read-WindowsOwnedResourceJournal -JournalPath $JournalPath
    if ([string]$journal.state -ceq 'completed') {
        return $journal
    }
    $ownerLive = Test-WindowsOwnedProcessIdentity -ProcessId ([int]$journal.owner_pid) -StartUtc ([string]$journal.owner_process_start_utc)
    if ($RecoveryMode) {
        if ($ownerLive) {
            throw "stale recovery refuses live exact owner PID '$($journal.owner_pid)'"
        }
    }
    else {
        Assert-WindowsOwnedJournalWriter -Journal $journal
    }

    $journal.state = 'cleaning'
    Add-WindowsOwnedJournalEvent -Journal $journal -Event 'cleanup-started' -Detail $(if ($RecoveryMode) { 'stale-owner-exact-mismatch' } else { 'owner-initiated' })
    Write-WindowsOwnedResourceJournal -Journal $journal

    $conflicts = [Collections.Generic.List[string]]::new()
    $ordered = @($journal.resources | Sort-Object -Property @{ Expression = { [int]$_.sequence }; Descending = $true })
    foreach ($resource in $ordered) {
        if ([string]$resource.state -ceq 'cleaned') {
            continue
        }
        try {
            $action = Invoke-WindowsOwnedSingleResourceCleanup -Journal $journal -Resource $resource
            $resource.state = 'cleaned'
            $resource.cleaned_utc = Get-WindowsOwnedUtcText
            Add-WindowsOwnedJournalEvent -Journal $journal -Event 'resource-cleaned' -ResourceId ([string]$resource.resource_id) -Detail "sequence=$($resource.sequence);action=$action"
        }
        catch {
            $resource.state = 'conflict'
            $message = $_.Exception.Message
            $conflicts.Add("$($resource.resource_id): $message")
            Add-WindowsOwnedJournalEvent -Journal $journal -Event 'cleanup-conflict' -ResourceId ([string]$resource.resource_id) -Detail $message
        }
        Write-WindowsOwnedResourceJournal -Journal $journal
    }

    if ($conflicts.Count -gt 0) {
        $journal.state = 'cleanup-conflict'
        Add-WindowsOwnedJournalEvent -Journal $journal -Event 'cleanup-incomplete' -Detail ($conflicts -join ' | ')
        Write-WindowsOwnedResourceJournal -Journal $journal
        throw "owned-resource cleanup stopped at exact-resource conflicts: $($conflicts -join ' | ')"
    }
    $journal.state = 'completed'
    Add-WindowsOwnedJournalEvent -Journal $journal -Event 'cleanup-completed' -Detail 'reverse-order; idempotent; zero-unrelated-mutation'
    Write-WindowsOwnedResourceJournal -Journal $journal
    return $journal
}
