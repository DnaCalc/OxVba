Set-StrictMode -Version Latest

$script:WindowsOwnedJournalSchema = "oxvba-windows-owned-resource-journal-v1"
$script:WindowsOwnedRunIdPattern = '^oxvba-[0-9]{8}T[0-9]{6}Z-[0-9a-f]{32}$'
$script:WindowsOwnedPolicyRepositoryRoot = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot)).TrimEnd(
    [IO.Path]::DirectorySeparatorChar,
    [IO.Path]::AltDirectorySeparatorChar)
$script:WindowsOwnedRegistryView = 'Registry64'
$script:WindowsOwnedActiveLeases = [Collections.Concurrent.ConcurrentDictionary[string, object]]::new([StringComparer]::Ordinal)

function Initialize-WindowsOwnedRegistryNative {
    if (-not ('OxVba.WindowsRegistryNative' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

namespace OxVba {
    public static class WindowsRegistryNative {
        private static readonly IntPtr HKEY_CURRENT_USER = new IntPtr(unchecked((int)0x80000001));
        private const int KEY_ALL_ACCESS = 0xF003F;
        private const int KEY_WOW64_64KEY = 0x0100;

        [DllImport("advapi32.dll", CharSet = CharSet.Unicode, EntryPoint = "RegCreateKeyExW")]
        private static extern int RegCreateKeyEx(
            IntPtr hKey, string subKey, int reserved, string keyClass, int options,
            int samDesired, IntPtr securityAttributes, out IntPtr result, out int disposition);

        [DllImport("advapi32.dll", CharSet = CharSet.Unicode, EntryPoint = "RegDeleteKeyExW")]
        private static extern int RegDeleteKeyEx(IntPtr hKey, string subKey, int samDesired, int reserved);

        [DllImport("advapi32.dll")]
        private static extern int RegCloseKey(IntPtr hKey);

        public static int CreateCurrentUserKey64(string subKey, out int disposition) {
            IntPtr result;
            int error = RegCreateKeyEx(HKEY_CURRENT_USER, subKey, 0, null, 0,
                KEY_ALL_ACCESS | KEY_WOW64_64KEY, IntPtr.Zero, out result, out disposition);
            if (result != IntPtr.Zero) RegCloseKey(result);
            return error;
        }

        public static int DeleteCurrentUserKey64(string subKey) {
            return RegDeleteKeyEx(HKEY_CURRENT_USER, subKey, KEY_WOW64_64KEY, 0);
        }
    }
}
'@
    }
}

function Assert-WindowsOwnedX64Windows {
    if (-not [Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([Runtime.InteropServices.OSPlatform]::Windows) -or
        -not [Environment]::Is64BitOperatingSystem -or -not [Environment]::Is64BitProcess) {
        throw 'owned Windows resource policy requires a 64-bit process on 64-bit Windows'
    }
}

function Open-WindowsOwnedRegistry64Base {
    Assert-WindowsOwnedX64Windows
    return [Microsoft.Win32.RegistryKey]::OpenBaseKey(
        [Microsoft.Win32.RegistryHive]::CurrentUser,
        [Microsoft.Win32.RegistryView]::Registry64)
}

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

function Get-WindowsOwnedJournalLeaseName {
    param([Parameter(Mandatory = $true)][string]$JournalPath)

    $canonical = [IO.Path]::GetFullPath($JournalPath).ToLowerInvariant()
    $digest = (Get-WindowsOwnedSha256Text -Text "journal-transaction-v1|$canonical").Substring('sha256:'.Length)
    return "Local\OxVba.WindowsOwnedJournal.$digest"
}

function Enter-WindowsOwnedJournalLease {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [ValidateRange(100, 120000)][int]$TimeoutMilliseconds = 120000
    )

    $path = [IO.Path]::GetFullPath($JournalPath)
    $name = Get-WindowsOwnedJournalLeaseName -JournalPath $path
    $createdNew = $false
    $mutex = [Threading.Mutex]::new($false, $name, [ref]$createdNew)
    $acquired = $false
    $abandoned = $false
    try {
        try {
            $acquired = $mutex.WaitOne($TimeoutMilliseconds)
        }
        catch [Threading.AbandonedMutexException] {
            $acquired = $true
            $abandoned = $true
        }
        if (-not $acquired) {
            throw "timed out acquiring owned-resource journal transaction lease '$name'"
        }
        $lease = [pscustomobject]@{
            token_id = [Guid]::NewGuid().ToString('N')
            journal_path = $path
            lease_name = $name
            mutex = $mutex
            acquired = $true
            abandoned = $abandoned
            revalidated = -not $abandoned
            owner_pid = $PID
            owner_thread_id = [Threading.Thread]::CurrentThread.ManagedThreadId
        }
        if (-not $script:WindowsOwnedActiveLeases.TryAdd([string]$lease.token_id, $lease)) {
            throw 'failed to register owned-resource journal transaction lease token'
        }
        return $lease
    }
    catch {
        if ($acquired) { try { $mutex.ReleaseMutex() } catch { } }
        $mutex.Dispose()
        throw
    }
}

function Assert-WindowsOwnedJournalLease {
    param(
        [Parameter(Mandatory = $true)]$Lease,
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [switch]$AllowPendingRevalidation
    )

    $registered = $null
    $registeredExact = $null -ne $Lease -and
        $script:WindowsOwnedActiveLeases.TryGetValue([string]$Lease.token_id, [ref]$registered) -and
        [object]::ReferenceEquals($registered, $Lease)
    if (-not $registeredExact -or -not [bool]$Lease.acquired -or [int]$Lease.owner_pid -ne $PID -or
        [int]$Lease.owner_thread_id -ne [Threading.Thread]::CurrentThread.ManagedThreadId -or
        -not (Test-WindowsOwnedExactPathEqual -Left ([string]$Lease.journal_path) -Right ([IO.Path]::GetFullPath($JournalPath))) -or
        [string]$Lease.lease_name -cne (Get-WindowsOwnedJournalLeaseName -JournalPath $JournalPath) -or
        (-not $AllowPendingRevalidation -and -not [bool]$Lease.revalidated)) {
        throw "owned-resource journal mutation requires its exact live transaction lease"
    }
}

function Confirm-WindowsOwnedJournalLeaseRevalidated {
    param(
        [Parameter(Mandatory = $true)]$Lease,
        [Parameter(Mandatory = $true)][string]$JournalPath
    )

    Assert-WindowsOwnedJournalLease -Lease $Lease -JournalPath $JournalPath -AllowPendingRevalidation
    $journal = Read-WindowsOwnedResourceJournal -JournalPath $JournalPath
    $Lease.revalidated = $true
    return $journal
}

function Confirm-WindowsOwnedNewJournalLeaseRevalidated {
    param(
        [Parameter(Mandatory = $true)]$Lease,
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$TempRoot,
        [Parameter(Mandatory = $true)][string]$JournalDirectory,
        [Parameter(Mandatory = $true)][string]$RunDirectory,
        [Parameter(Mandatory = $true)][string]$RunRoot
    )

    Assert-WindowsOwnedJournalLease -Lease $Lease -JournalPath $JournalPath -AllowPendingRevalidation
    [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $RepositoryRoot -Owner 'owned-resource repository root' -RequireContainer)
    [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $TempRoot -Owner 'owned-resource temp root' -RequireContainer)
    foreach ($infrastructurePath in @($JournalDirectory, $RunDirectory)) {
        if (Test-Path -LiteralPath $infrastructurePath) {
            [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $infrastructurePath -Owner 'owned-resource infrastructure root' -RequireContainer)
        }
    }
    if ((Test-Path -LiteralPath $JournalPath) -or (Test-Path -LiteralPath $RunRoot)) {
        throw "owned-resource run '$([IO.Path]::GetFileNameWithoutExtension($JournalPath))' collides with an existing journal/root"
    }

    # An abandoned lease has no prior journal to authenticate on this creation
    # path. Exact absence of both immutable run identities, after validating all
    # existing parents, is its fail-closed revalidation condition.
    $Lease.revalidated = $true
    Assert-WindowsOwnedJournalLease -Lease $Lease -JournalPath $JournalPath
}

function Exit-WindowsOwnedJournalLease {
    param([Parameter(Mandatory = $true)]$Lease)

    $registered = $null
    $registeredExact = $null -ne $Lease -and
        $script:WindowsOwnedActiveLeases.TryGetValue([string]$Lease.token_id, [ref]$registered) -and
        [object]::ReferenceEquals($registered, $Lease)
    if (-not $registeredExact -or -not [bool]$Lease.acquired -or [int]$Lease.owner_pid -ne $PID -or
        [int]$Lease.owner_thread_id -ne [Threading.Thread]::CurrentThread.ManagedThreadId) {
        throw 'owned-resource journal lease can only be released by its acquiring process/thread'
    }
    $Lease.mutex.ReleaseMutex()
    $Lease.acquired = $false
    $Lease.mutex.Dispose()
    $removed = $null
    if (-not $script:WindowsOwnedActiveLeases.TryRemove([string]$Lease.token_id, [ref]$removed) -or
        -not [object]::ReferenceEquals($removed, $Lease)) {
        throw 'owned-resource journal lease token registry was inconsistent during release'
    }
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

function Assert-WindowsOwnedPathComponentsNoReparse {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner,
        [switch]$RequireContainer
    )

    $full = [IO.Path]::GetFullPath($Path)
    if ($RequireContainer -and -not [IO.Directory]::Exists($full)) {
        throw "$Owner '$full' must be an existing directory"
    }
    $volume = [IO.Path]::GetPathRoot($full).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    $current = $volume + [IO.Path]::DirectorySeparatorChar
    $relative = $full.Substring($current.Length)
    foreach ($part in @($relative -split '[\\/]')) {
        if ([string]::IsNullOrEmpty($part)) { continue }
        $current = Join-Path $current $part
        if (-not [IO.Directory]::Exists($current) -and -not [IO.File]::Exists($current)) { break }
        $attributes = [IO.File]::GetAttributes($current)
        if (($attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "$Owner crosses reparse point '$current'"
        }
    }
    return $full
}

function Assert-WindowsOwnedNoReparseTraversal {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$CandidatePath,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $base = [IO.Path]::GetFullPath($BasePath).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    $candidate = [IO.Path]::GetFullPath($CandidatePath)
    [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $base -Owner $Owner -RequireContainer)
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
        if (-not [IO.Directory]::Exists($current) -and -not [IO.File]::Exists($current)) {
            continue
        }
        $attributes = [IO.File]::GetAttributes($current)
        if (($attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
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

function ConvertTo-WindowsOwnedRegistryAncestorPath {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [string]$Owner = 'registry ancestor path'
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
    if ($normalized -notmatch '^HKCU\\Software\\[^\\]+(?:\\.*)?$' -or
        $normalized -in @('HKCU\Software\Classes', 'HKCU\Software\Classes\CLSID', 'HKCU\Software\Classes\TypeLib', 'HKCU\Software\Classes\Interface', 'HKCU\Software\Classes\AppID')) {
        throw "$Owner must be an exact path below HKCU\Software, not a hive/category root"
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

function Get-WindowsOwnedJsonProperty {
    param(
        [Parameter(Mandatory = $true)][Text.Json.JsonElement]$Element,
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    [Text.Json.JsonElement]$value = [Text.Json.JsonElement]::new()
    if ($Element.ValueKind -ne [Text.Json.JsonValueKind]::Object -or -not $Element.TryGetProperty($Name, [ref]$value)) {
        throw "$Owner is missing exact JSON property '$Name'"
    }
    return $value
}

function Assert-WindowsOwnedJsonObjectShape {
    param(
        [Parameter(Mandatory = $true)][Text.Json.JsonElement]$Element,
        [Parameter(Mandatory = $true)][Collections.IDictionary]$Schema,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ($Element.ValueKind -ne [Text.Json.JsonValueKind]::Object) {
        throw "$Owner must be a JSON object"
    }
    $actual = @($Element.EnumerateObject() | ForEach-Object { $_.Name })
    if (-not (Test-WindowsOwnedStringSetEqual -Left $actual -Right @($Schema.Keys))) {
        throw "$Owner must use the exact case-sensitive JSON property schema"
    }
    foreach ($name in $Schema.Keys) {
        $value = Get-WindowsOwnedJsonProperty -Element $Element -Name $name -Owner $Owner
        $expected = [string]$Schema[$name]
        $valid = switch ($expected) {
            'string' { $value.ValueKind -eq [Text.Json.JsonValueKind]::String; break }
            'int32' {
                $number = 0
                $value.ValueKind -eq [Text.Json.JsonValueKind]::Number -and $value.TryGetInt32([ref]$number); break
            }
            'int64' {
                [long]$number = 0
                $value.ValueKind -eq [Text.Json.JsonValueKind]::Number -and $value.TryGetInt64([ref]$number); break
            }
            'bool' { $value.ValueKind -in @([Text.Json.JsonValueKind]::True, [Text.Json.JsonValueKind]::False); break }
            'array' { $value.ValueKind -eq [Text.Json.JsonValueKind]::Array; break }
            'object' { $value.ValueKind -eq [Text.Json.JsonValueKind]::Object; break }
            default { throw "unsupported raw JSON schema kind '$expected'" }
        }
        if (-not $valid) {
            throw "$Owner property '$name' must be JSON $expected without coercion"
        }
    }
}

function Get-WindowsOwnedJsonInt32 {
    param([Text.Json.JsonElement]$Element, [string]$Name, [string]$Owner, [int]$Minimum = [int]::MinValue, [int]$Maximum = [int]::MaxValue)

    $property = Get-WindowsOwnedJsonProperty -Element $Element -Name $Name -Owner $Owner
    $value = 0
    if ($property.ValueKind -ne [Text.Json.JsonValueKind]::Number -or -not $property.TryGetInt32([ref]$value) -or $value -lt $Minimum -or $value -gt $Maximum) {
        throw "$Owner property '$Name' is outside its exact JSON int32 range"
    }
    return $value
}

function Get-WindowsOwnedJsonInt64 {
    param([Text.Json.JsonElement]$Element, [string]$Name, [string]$Owner, [long]$Minimum = [long]::MinValue, [long]$Maximum = [long]::MaxValue)

    $property = Get-WindowsOwnedJsonProperty -Element $Element -Name $Name -Owner $Owner
    [long]$value = 0
    if ($property.ValueKind -ne [Text.Json.JsonValueKind]::Number -or -not $property.TryGetInt64([ref]$value) -or $value -lt $Minimum -or $value -gt $Maximum) {
        throw "$Owner property '$Name' is outside its exact JSON int64 range"
    }
    return $value
}

function Assert-WindowsOwnedRawSnapshotJson {
    param(
        [Text.Json.JsonElement]$Element,
        [Parameter(Mandatory = $true)][ValidateSet('file', 'registry', 'process', 'apartment', 'callback', 'connection', 'dialog')][string]$Kind,
        [Parameter(Mandatory = $true)][ValidateSet('before', 'expected')][string]$Phase,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    switch ($Kind) {
        'file' {
            Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{ exists = 'bool'; length = 'int64'; sha256 = 'string' }) -Owner $Owner
            [void](Get-WindowsOwnedJsonInt64 -Element $Element -Name length -Owner $Owner -Minimum 0)
        }
        'registry' {
            Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{ key_exists = 'bool'; exists = 'bool'; kind = 'string'; data_base64 = 'string' }) -Owner $Owner
        }
        'process' {
            $name = if ($Phase -eq 'before') { 'exists' } else { 'recorded' }
            Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{ $name = 'bool' }) -Owner $Owner
        }
        'apartment' { Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{ registered = 'bool' }) -Owner $Owner }
        'callback' { Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{ registered = 'bool' }) -Owner $Owner }
        'connection' { Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{ advised = 'bool' }) -Owner $Owner }
        'dialog' { Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{ registered = 'bool' }) -Owner $Owner }
    }
}

function Assert-WindowsOwnedRawDescriptorJson {
    param(
        [Text.Json.JsonElement]$Element,
        [Parameter(Mandatory = $true)][string]$Kind,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    switch ($Kind) {
        'file' {
            Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{ path = 'string'; mutation_mode = 'string' }) -Owner $Owner
        }
        'registry' {
            Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{
                path = 'string'; value_name = 'string'; mutation_mode = 'string'; registry_view = 'string';
                existing_ancestor_path = 'string'; key_ownership = 'array'
            }) -Owner $Owner
            $records = Get-WindowsOwnedJsonProperty -Element $Element -Name key_ownership -Owner $Owner
            foreach ($record in $records.EnumerateArray()) {
                Assert-WindowsOwnedJsonObjectShape -Element $record -Schema ([ordered]@{
                    path = 'string'; creation_disposition = 'string'; marker_name = 'string'; marker_token = 'string'
                }) -Owner "$Owner key ownership"
            }
        }
        'process' {
            Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{
                executable_path = 'string'; pid = 'int32'; process_start_utc = 'string'; arguments_sha256 = 'string';
                activation_path = 'string'; parent_pid = 'int32'; harmless_child = 'bool'; self_timeout_seconds = 'int32'
            }) -Owner $Owner
            [void](Get-WindowsOwnedJsonInt32 -Element $Element -Name pid -Owner $Owner -Minimum 0)
            [void](Get-WindowsOwnedJsonInt32 -Element $Element -Name parent_pid -Owner $Owner -Minimum 1)
            [void](Get-WindowsOwnedJsonInt32 -Element $Element -Name self_timeout_seconds -Owner $Owner -Minimum 1 -Maximum 60)
        }
        'apartment' {
            Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{
                process_id = 'int32'; thread_id = 'int32'; model = 'string'; com_initialization = 'string'; reentry_policy = 'string';
                message_pump = 'string'; max_reentry_depth = 'int32'
            }) -Owner $Owner
            [void](Get-WindowsOwnedJsonInt32 -Element $Element -Name process_id -Owner $Owner -Minimum 1)
            [void](Get-WindowsOwnedJsonInt32 -Element $Element -Name thread_id -Owner $Owner -Minimum 1)
            [void](Get-WindowsOwnedJsonInt32 -Element $Element -Name max_reentry_depth -Owner $Owner -Minimum 0 -Maximum 16)
        }
        'callback' {
            Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{
                apartment_resource_id = 'string'; session_id = 'string'; thunk_id = 'string'; owning_thread_id = 'int32';
                retention = 'string'; wrong_thread_policy = 'string'; stale_policy = 'string'
            }) -Owner $Owner
            [void](Get-WindowsOwnedJsonInt32 -Element $Element -Name owning_thread_id -Owner $Owner -Minimum 1)
        }
        'connection' {
            Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{
                apartment_resource_id = 'string'; callback_resource_id = 'string'; source_identity = 'string'; sink_identity = 'string';
                connection_point_iid = 'string'; cookie = 'int32'; writeback_policy = 'string'
            }) -Owner $Owner
            [void](Get-WindowsOwnedJsonInt32 -Element $Element -Name cookie -Owner $Owner -Minimum 1)
        }
        'dialog' {
            Assert-WindowsOwnedJsonObjectShape -Element $Element -Schema ([ordered]@{
                process_resource_id = 'string'; process_id = 'int32'; process_start_utc = 'string'; uia_runtime_id = 'string';
                native_window_handle = 'int64'; title_sha256 = 'string'; allowed_action = 'string'
            }) -Owner $Owner
            [void](Get-WindowsOwnedJsonInt32 -Element $Element -Name process_id -Owner $Owner -Minimum 1)
            [void](Get-WindowsOwnedJsonInt64 -Element $Element -Name native_window_handle -Owner $Owner -Minimum 1)
        }
        default { throw "$Owner has unsupported exact resource kind '$Kind'" }
    }
}

function Assert-WindowsOwnedRawJournalJson {
    param(
        [Parameter(Mandatory = $true)][byte[]]$Bytes,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    Assert-WindowsOwnedJsonNoDuplicates -Bytes $Bytes -Owner $Owner
    try {
        $text = [Text.UTF8Encoding]::new($false, $true).GetString($Bytes)
        $document = [Text.Json.JsonDocument]::Parse($text)
    }
    catch {
        throw "$Owner is not strict UTF-8 JSON"
    }
    try {
        $root = $document.RootElement
        Assert-WindowsOwnedJsonObjectShape -Element $root -Schema ([ordered]@{
            schema_id = 'string'; schema_version = 'int32'; run_id = 'string'; created_utc = 'string'; updated_utc = 'string';
            owner_pid = 'int32'; owner_process_start_utc = 'string'; repository_root = 'string'; temp_root = 'string';
            run_root = 'string'; journal_path = 'string'; registry_view = 'string'; allowed_registry_paths = 'array';
            allowed_executable_paths = 'array'; orchestrator_apartment = 'object'; reentry_policy = 'string'; state = 'string';
            next_resource_sequence = 'int32'; next_event_sequence = 'int32'; resources = 'array'; events = 'array'; journal_digest = 'string'
        }) -Owner $Owner
        [void](Get-WindowsOwnedJsonInt32 -Element $root -Name schema_version -Owner $Owner -Minimum 1 -Maximum 1)
        [void](Get-WindowsOwnedJsonInt32 -Element $root -Name owner_pid -Owner $Owner -Minimum 1)
        [void](Get-WindowsOwnedJsonInt32 -Element $root -Name next_resource_sequence -Owner $Owner -Minimum 1)
        [void](Get-WindowsOwnedJsonInt32 -Element $root -Name next_event_sequence -Owner $Owner -Minimum 1)
        foreach ($arrayName in @('allowed_registry_paths', 'allowed_executable_paths')) {
            $array = Get-WindowsOwnedJsonProperty -Element $root -Name $arrayName -Owner $Owner
            foreach ($item in $array.EnumerateArray()) {
                if ($item.ValueKind -ne [Text.Json.JsonValueKind]::String) {
                    throw "$Owner '$arrayName' must contain only JSON strings"
                }
            }
        }
        $apartment = Get-WindowsOwnedJsonProperty -Element $root -Name orchestrator_apartment -Owner $Owner
        Assert-WindowsOwnedJsonObjectShape -Element $apartment -Schema ([ordered]@{ process_id = 'int32'; thread_id = 'int32'; model = 'string' }) -Owner "$Owner orchestrator apartment"
        [void](Get-WindowsOwnedJsonInt32 -Element $apartment -Name process_id -Owner "$Owner orchestrator apartment" -Minimum 1)
        [void](Get-WindowsOwnedJsonInt32 -Element $apartment -Name thread_id -Owner "$Owner orchestrator apartment" -Minimum 1)

        $resources = Get-WindowsOwnedJsonProperty -Element $root -Name resources -Owner $Owner
        foreach ($resource in $resources.EnumerateArray()) {
            Assert-WindowsOwnedJsonObjectShape -Element $resource -Schema ([ordered]@{
                sequence = 'int32'; resource_id = 'string'; kind = 'string'; state = 'string'; prepared_utc = 'string';
                active_utc = 'string'; cleaned_utc = 'string'; descriptor = 'object'; before = 'object'; expected = 'object'
            }) -Owner "$Owner resource"
            [void](Get-WindowsOwnedJsonInt32 -Element $resource -Name sequence -Owner "$Owner resource" -Minimum 1)
            $kind = (Get-WindowsOwnedJsonProperty -Element $resource -Name kind -Owner "$Owner resource").GetString()
            $descriptor = Get-WindowsOwnedJsonProperty -Element $resource -Name descriptor -Owner "$Owner resource"
            Assert-WindowsOwnedRawDescriptorJson -Element $descriptor -Kind $kind -Owner "$Owner $kind descriptor"
            Assert-WindowsOwnedRawSnapshotJson -Element (Get-WindowsOwnedJsonProperty -Element $resource -Name before -Owner "$Owner resource") -Kind $kind -Phase before -Owner "$Owner $kind before"
            Assert-WindowsOwnedRawSnapshotJson -Element (Get-WindowsOwnedJsonProperty -Element $resource -Name expected -Owner "$Owner resource") -Kind $kind -Phase expected -Owner "$Owner $kind expected"
        }

        $events = Get-WindowsOwnedJsonProperty -Element $root -Name events -Owner $Owner
        foreach ($event in $events.EnumerateArray()) {
            Assert-WindowsOwnedJsonObjectShape -Element $event -Schema ([ordered]@{
                sequence = 'int32'; timestamp_utc = 'string'; event = 'string'; resource_id = 'string'; detail = 'string'
            }) -Owner "$Owner event"
            [void](Get-WindowsOwnedJsonInt32 -Element $event -Name sequence -Owner "$Owner event" -Minimum 1)
        }
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
        registry_view = [string]$Journal.registry_view
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
    param(
        [Parameter(Mandatory = $true)]$Journal,
        [Parameter(Mandatory = $true)]$Lease
    )

    Assert-WindowsOwnedJournalLease -Lease $Lease -JournalPath ([string]$Journal.journal_path)
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
    [void](Assert-WindowsOwnedPathComponentsNoReparse -Path ([string]$Journal.repository_root) -Owner 'owned-resource journal repository root' -RequireContainer)
    [void](Assert-WindowsOwnedPathComponentsNoReparse -Path ([string]$Journal.temp_root) -Owner 'owned-resource journal temp root' -RequireContainer)
    [void](Assert-WindowsOwnedPathComponentsNoReparse -Path ([string]$Journal.run_root) -Owner 'owned-resource journal run root' -RequireContainer)
    [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $parent -Owner 'owned-resource journal infrastructure' -RequireContainer)
    if (Test-Path -LiteralPath $path) {
        [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $path -Owner 'owned-resource journal file')
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
        [void](Assert-WindowsOwnedPathComponentsNoReparse -Path ([string]$Journal.repository_root) -Owner 'owned-resource journal repository root' -RequireContainer)
        [void](Assert-WindowsOwnedPathComponentsNoReparse -Path ([string]$Journal.temp_root) -Owner 'owned-resource journal temp root' -RequireContainer)
        [void](Assert-WindowsOwnedPathComponentsNoReparse -Path ([string]$Journal.run_root) -Owner 'owned-resource journal run root' -RequireContainer)
        [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $parent -Owner 'owned-resource journal operation boundary' -RequireContainer)
        [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $temporary -Owner 'owned-resource journal temporary file')
        if (Test-Path -LiteralPath $path) {
            [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $path -Owner 'owned-resource journal destination file')
        }
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
    [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $path -Owner "owned-resource journal '$path'")
    $bytes = [IO.File]::ReadAllBytes($path)
    Assert-WindowsOwnedRawJournalJson -Bytes $bytes -Owner "owned-resource journal '$path'"
    try {
        $journal = [Text.UTF8Encoding]::new($false, $true).GetString($bytes) | ConvertFrom-Json -Depth 32 -DateKind String
    }
    catch {
        throw "owned-resource journal '$path' is not strict UTF-8 JSON"
    }
    Assert-WindowsOwnedExactProperties -Value $journal -Expected @(
        'schema_id', 'schema_version', 'run_id', 'created_utc', 'updated_utc',
        'owner_pid', 'owner_process_start_utc', 'repository_root', 'temp_root',
        'run_root', 'journal_path', 'registry_view', 'allowed_registry_paths',
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
        [string]$journal.registry_view -cne $script:WindowsOwnedRegistryView -or
        [string]$journal.reentry_policy -notin @('reject', 'same-apartment-synchronous', 'declared-nested') -or
        [string]$journal.journal_digest -notmatch '^sha256:[0-9a-f]{64}$' -or
        [string]$journal.journal_digest -cne (Get-WindowsOwnedJournalDigest -Journal $journal)) {
        throw "owned-resource journal '$path' has invalid identity or digest"
    }
    [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $repositoryRoot -Owner "owned-resource journal '$path' repository root" -RequireContainer)
    [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $tempRoot -Owner "owned-resource journal '$path' temp root" -RequireContainer)
    [void](Assert-WindowsOwnedPathComponentsNoReparse -Path ([string]$journal.run_root) -Owner "owned-resource journal '$path' run root" -RequireContainer)
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
            (-not [string]::IsNullOrEmpty([string]$resource.active_utc) -and [string]$resource.active_utc -notmatch '^\d{4}-\d{2}-\d{2}T') -or
            (-not [string]::IsNullOrEmpty([string]$resource.cleaned_utc) -and [string]$resource.cleaned_utc -notmatch '^\d{4}-\d{2}-\d{2}T') -or
            ([string]$resource.state -ceq 'prepared' -and
                (-not [string]::IsNullOrEmpty([string]$resource.active_utc) -or -not [string]::IsNullOrEmpty([string]$resource.cleaned_utc))) -or
            ([string]$resource.state -ceq 'active' -and
                ([string]$resource.active_utc -notmatch '^\d{4}-\d{2}-\d{2}T' -or -not [string]::IsNullOrEmpty([string]$resource.cleaned_utc))) -or
            ([string]$resource.state -ceq 'conflict' -and -not [string]::IsNullOrEmpty([string]$resource.cleaned_utc)) -or
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
    Assert-WindowsOwnedJournalLifecycle -Journal $journal
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

    Assert-WindowsOwnedX64Windows
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
    [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $repositoryRootFull -Owner 'owned-resource repository root' -RequireContainer)
    [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $tempRootFull -Owner 'owned-resource temp root' -RequireContainer)
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
    $journalPath = Join-Path $journalDirectory "$RunId.json"
    $runRoot = Join-Path $runDirectory $RunId
    $lease = Enter-WindowsOwnedJournalLease -JournalPath $journalPath
    try {
        Confirm-WindowsOwnedNewJournalLeaseRevalidated -Lease $lease -JournalPath $journalPath `
            -RepositoryRoot $repositoryRootFull -TempRoot $tempRootFull -JournalDirectory $journalDirectory `
            -RunDirectory $runDirectory -RunRoot $runRoot
        [void](New-Item -ItemType Directory -Path $journalDirectory -Force)
        [void](New-Item -ItemType Directory -Path $runDirectory -Force)
        [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $journalDirectory -Owner 'owned-resource journal infrastructure' -RequireContainer)
        [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $runDirectory -Owner 'owned-resource run infrastructure' -RequireContainer)
        if ((Test-Path -LiteralPath $journalPath) -or (Test-Path -LiteralPath $runRoot)) {
            throw "owned-resource run '$RunId' collides with an existing journal/root"
        }
        [void](New-Item -ItemType Directory -Path $runRoot)
        [void](Assert-WindowsOwnedPathComponentsNoReparse -Path $runRoot -Owner 'owned-resource run root' -RequireContainer)
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
            registry_view = $script:WindowsOwnedRegistryView
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
        Write-WindowsOwnedResourceJournal -Journal $journal -Lease $lease
        return [IO.Path]::GetFullPath($journalPath)
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $lease
    }
}

function Add-WindowsOwnedPreparedResource {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)]$Lease,
        [Parameter(Mandatory = $true)][ValidateSet('file', 'registry', 'process', 'apartment', 'callback', 'connection', 'dialog')][string]$Kind,
        [Parameter(Mandatory = $true)]$Descriptor,
        [Parameter(Mandatory = $true)]$Before,
        [Parameter(Mandatory = $true)]$Expected,
        $Journal = $null
    )

    Assert-WindowsOwnedJournalLease -Lease $Lease -JournalPath $JournalPath
    $journal = if ($null -eq $Journal) { Read-WindowsOwnedResourceJournal -JournalPath $JournalPath } else { $Journal }
    if (-not (Test-WindowsOwnedExactPathEqual -Left ([string]$journal.journal_path) -Right $JournalPath)) {
        throw 'prepared resource journal object does not match the transaction lease path'
    }
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
    Write-WindowsOwnedResourceJournal -Journal $journal -Lease $Lease
    return $resourceId
}

function Set-WindowsOwnedResourceActive {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)]$Lease,
        [Parameter(Mandatory = $true)][string]$ResourceId,
        $Descriptor = $null,
        $Journal = $null
    )

    Assert-WindowsOwnedJournalLease -Lease $Lease -JournalPath $JournalPath
    $journal = if ($null -eq $Journal) { Read-WindowsOwnedResourceJournal -JournalPath $JournalPath } else { $Journal }
    if (-not (Test-WindowsOwnedExactPathEqual -Left ([string]$journal.journal_path) -Right $JournalPath)) {
        throw 'active resource journal object does not match the transaction lease path'
    }
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
    Write-WindowsOwnedResourceJournal -Journal $journal -Lease $Lease
}

function Set-WindowsOwnedPreparedResourceDescriptor {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)]$Lease,
        [Parameter(Mandatory = $true)][string]$ResourceId,
        [Parameter(Mandatory = $true)]$Descriptor,
        [Parameter(Mandatory = $true)][string]$Detail,
        $Journal = $null
    )

    Assert-WindowsOwnedJournalLease -Lease $Lease -JournalPath $JournalPath
    $journal = if ($null -eq $Journal) { Read-WindowsOwnedResourceJournal -JournalPath $JournalPath } else { $Journal }
    if (-not (Test-WindowsOwnedExactPathEqual -Left ([string]$journal.journal_path) -Right $JournalPath)) {
        throw 'prepared-update journal object does not match the transaction lease path'
    }
    Assert-WindowsOwnedJournalWriter -Journal $journal
    $matches = @($journal.resources | Where-Object { [string]$_.resource_id -ceq $ResourceId })
    if ($matches.Count -ne 1 -or [string]$matches[0].state -ne 'prepared') {
        throw "owned-resource '$ResourceId' is not one prepared journal resource"
    }
    $matches[0].descriptor = $Descriptor
    Assert-WindowsOwnedResourceDescriptor -Journal $journal -Resource $matches[0]
    Add-WindowsOwnedJournalEvent -Journal $journal -Event 'resource-prepared-updated' -ResourceId $ResourceId -Detail $Detail
    Write-WindowsOwnedResourceJournal -Journal $journal -Lease $Lease
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
            Assert-WindowsOwnedExactProperties -Value $Resource.descriptor -Expected @(
                'path', 'value_name', 'mutation_mode', 'registry_view', 'existing_ancestor_path', 'key_ownership'
            ) -Owner "$owner registry descriptor"
            $path = ConvertTo-WindowsOwnedRegistryPath -Path ([string]$Resource.descriptor.path) -Owner "$owner registry path"
            Assert-WindowsOwnedExactIdentityText -Value ([string]$Resource.descriptor.value_name) -Owner "$owner registry value"
            if ([string]$Resource.descriptor.mutation_mode -cne 'exact-value' -or
                [string]$Resource.descriptor.registry_view -cne $script:WindowsOwnedRegistryView -or
                [string]$Journal.registry_view -cne $script:WindowsOwnedRegistryView -or
                -not (@($Journal.allowed_registry_paths) | Where-Object { [string]$_ -ieq $path })) {
                throw "$owner registry path/view is not one exact HKCU Registry64 allowlist entry"
            }
            Assert-WindowsOwnedSnapshotSchema -Snapshot $Resource.before -Owner "$owner registry before"
            Assert-WindowsOwnedSnapshotSchema -Snapshot $Resource.expected -Owner "$owner registry expected"
            if (-not [bool]$Resource.expected.exists) {
                throw "$owner registry mutation must journal one exact resulting value"
            }
            $existingAncestor = if ([string]$Resource.descriptor.existing_ancestor_path -ceq 'HKCU\Software') {
                'HKCU\Software'
            }
            else {
                ConvertTo-WindowsOwnedRegistryAncestorPath -Path ([string]$Resource.descriptor.existing_ancestor_path) -Owner "$owner existing registry ancestor"
            }
            if (-not ($path -ieq $existingAncestor -or $path.StartsWith($existingAncestor + '\', [StringComparison]::OrdinalIgnoreCase))) {
                throw "$owner existing registry ancestor must be an exact prefix of the owned leaf"
            }
            $previousAncestor = $existingAncestor
            $seenAncestors = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
            $seenPendingDisposition = $false
            foreach ($keyRecord in @($Resource.descriptor.key_ownership)) {
                Assert-WindowsOwnedExactProperties -Value $keyRecord -Expected @(
                    'path', 'creation_disposition', 'marker_name', 'marker_token'
                ) -Owner "$owner registry key ownership"
                $ancestorPath = ConvertTo-WindowsOwnedRegistryAncestorPath -Path ([string]$keyRecord.path) -Owner "$owner registry key ownership path"
                $expectedParent = $ancestorPath.Substring(0, $ancestorPath.LastIndexOf('\'))
                $markerName = [string]$keyRecord.marker_name
                if ([string]$keyRecord.path -cne $ancestorPath -or
                    -not $seenAncestors.Add($ancestorPath) -or
                    $expectedParent -ine $previousAncestor -or
                    -not ($path -ieq $ancestorPath -or $path.StartsWith($ancestorPath + '\', [StringComparison]::OrdinalIgnoreCase)) -or
                    [string]$keyRecord.creation_disposition -notin @('pending', 'created-owned', 'opened-existing') -or
                    $markerName -notmatch '^__OxVbaOwnedKey_[0-9a-f]{32}$' -or
                    [string]$keyRecord.marker_token -cne "oxvba-key-token-v1:$($markerName.Substring('__OxVbaOwnedKey_'.Length))") {
                    throw "$owner registry key ownership must be canonical, token-bound, unique and shallow-to-deep"
                }
                if ([string]$keyRecord.creation_disposition -ceq 'pending') {
                    $seenPendingDisposition = $true
                }
                elseif ($seenPendingDisposition) {
                    throw "$owner registry key creation outcomes must be one durable shallow-to-deep prefix"
                }
                $previousAncestor = $ancestorPath
            }
            $keyCount = @($Resource.descriptor.key_ownership).Count
            if (([bool]$Resource.before.key_exists -and ($keyCount -ne 0 -or $existingAncestor -ine $path)) -or
                (-not [bool]$Resource.before.key_exists -and ($keyCount -eq 0 -or $previousAncestor -ine $path)) -or
                ([string]$Resource.state -ceq 'active' -and @($Resource.descriptor.key_ownership | Where-Object { [string]$_.creation_disposition -ceq 'pending' }).Count -ne 0)) {
                throw "$owner must record exact Registry64 key creation outcomes and proof tokens"
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
            if ($apartment.Count -ne 1 -or [int]$apartment[0].sequence -ge [int]$Resource.sequence -or
                [int]$Resource.descriptor.owning_thread_id -ne [int]$apartment[0].descriptor.thread_id -or
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
            if ($apartment.Count -ne 1 -or $callback.Count -ne 1 -or
                [int]$apartment[0].sequence -ge [int]$Resource.sequence -or [int]$callback[0].sequence -ge [int]$Resource.sequence -or
                [string]$callback[0].descriptor.apartment_resource_id -cne [string]$Resource.descriptor.apartment_resource_id -or
                [int64]$Resource.descriptor.cookie -le 0 -or
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
            if ($process.Count -ne 1 -or [int]$process[0].sequence -ge [int]$Resource.sequence -or
                [int]$Resource.descriptor.process_id -ne [int]$process[0].descriptor.pid -or
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

function Assert-WindowsOwnedJournalLifecycle {
    param([Parameter(Mandatory = $true)]$Journal)

    $resources = @{}
    $lifecycle = @{}
    foreach ($resource in @($Journal.resources)) {
        $id = [string]$resource.resource_id
        $resources[$id] = $resource
        $lifecycle[$id] = [pscustomobject]@{ prepared = 0; updated = 0; active = 0; cleaned = 0; conflicts = 0 }
    }
    $allowed = @(
        'journal-created', 'resource-prepared', 'resource-prepared-updated', 'resource-active',
        'cleanup-started', 'resource-cleaned', 'cleanup-conflict', 'cleanup-incomplete', 'cleanup-completed'
    )
    $journalCreated = 0
    $cleanupStarted = 0
    $cleanupCompleted = 0
    $cleanupIncomplete = 0
    $cleanupEverStarted = $false
    $cleanupCycleOpen = $false
    $lastCleanupResourceSequence = [int]::MaxValue
    $cycleOutcomes = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($event in @($Journal.events)) {
        $name = [string]$event.event
        $resourceId = [string]$event.resource_id
        if ($name -notin $allowed -or [string]$event.timestamp_utc -notmatch '^\d{4}-\d{2}-\d{2}T') {
            throw "owned-resource journal contains an unknown or malformed lifecycle event '$name'"
        }
        if ($cleanupCompleted -gt 0) {
            throw "owned-resource journal contains lifecycle events after terminal cleanup completion"
        }
        if ($name -in @('journal-created', 'cleanup-started', 'cleanup-incomplete', 'cleanup-completed')) {
            if (-not [string]::IsNullOrEmpty($resourceId)) {
                throw "owned-resource journal event '$name' must not claim a resource ID"
            }
            switch ($name) {
                'journal-created' {
                    $journalCreated++
                    if ([int]$event.sequence -ne 1 -or $cleanupEverStarted -or
                        [string]$event.detail -cne 'support-only; capability-credit=none') {
                        throw 'owned-resource journal creation event is not exact'
                    }
                }
                'cleanup-started' {
                    if ($journalCreated -ne 1 -or $cleanupCycleOpen -or
                        [string]$event.detail -notin @('owner-initiated', 'stale-owner-exact-mismatch')) {
                        throw 'owned-resource cleanup cycle start is out of order or malformed'
                    }
                    $cleanupStarted++
                    $cleanupEverStarted = $true
                    $cleanupCycleOpen = $true
                    $lastCleanupResourceSequence = [int]::MaxValue
                    $cycleOutcomes.Clear()
                }
                'cleanup-incomplete' {
                    if (-not $cleanupCycleOpen -or [string]::IsNullOrWhiteSpace([string]$event.detail)) {
                        throw 'owned-resource incomplete cleanup event has no open cycle or conflict detail'
                    }
                    $cleanupIncomplete++
                    $cleanupCycleOpen = $false
                }
                'cleanup-completed' {
                    if (-not $cleanupCycleOpen -or [string]$event.detail -cne 'reverse-order; idempotent; zero-unrelated-mutation') {
                        throw 'owned-resource completed cleanup event has no exact open cycle'
                    }
                    $cleanupCompleted++
                    $cleanupCycleOpen = $false
                }
            }
            continue
        }
        if (-not $resources.ContainsKey($resourceId)) {
            throw "owned-resource journal event '$name' references unknown resource '$resourceId'"
        }
        $resource = $resources[$resourceId]
        $facts = $lifecycle[$resourceId]
        switch ($name) {
            'resource-prepared' {
                if ($cleanupEverStarted -or $facts.prepared -ne 0 -or [string]$event.detail -cne [string]$resource.kind) {
                    throw "owned-resource '$resourceId' prepared event kind is inconsistent"
                }
                $facts.prepared++
            }
            'resource-prepared-updated' {
                if ($cleanupEverStarted -or [string]$resource.kind -cne 'registry' -or
                    $facts.prepared -ne 1 -or $facts.active -ne 0) {
                    throw "owned-resource '$resourceId' has an invalid prepared-update event"
                }
                $updateIndex = [int]$facts.updated
                $keyOwnership = @($resource.descriptor.key_ownership)
                if ($updateIndex -ge $keyOwnership.Count -or
                    [string]$keyOwnership[$updateIndex].creation_disposition -ceq 'pending' -or
                    [string]$event.detail -cne "registry-key[$updateIndex]=$([string]$keyOwnership[$updateIndex].creation_disposition)") {
                    throw "owned-resource '$resourceId' prepared-update event does not match its durable Registry64 disposition prefix"
                }
                $facts.updated++
            }
            'resource-active' {
                if ($cleanupEverStarted -or $facts.prepared -ne 1 -or $facts.active -ne 0 -or
                    [string]$event.detail -cne [string]$resource.kind) {
                    throw "owned-resource '$resourceId' active event does not follow exact preparation"
                }
                $facts.active++
            }
            'resource-cleaned' {
                if (-not $cleanupCycleOpen -or $facts.prepared -ne 1 -or $facts.cleaned -ne 0 -or
                    -not $cycleOutcomes.Add($resourceId) -or
                    [int]$resource.sequence -ge $lastCleanupResourceSequence -or
                    [string]$event.detail -notmatch "^sequence=$([int]$resource.sequence);action=.+") {
                    throw "owned-resource '$resourceId' cleanup event is not in reverse acquisition order"
                }
                $facts.cleaned++
                $lastCleanupResourceSequence = [int]$resource.sequence
            }
            'cleanup-conflict' {
                if (-not $cleanupCycleOpen -or $facts.prepared -ne 1 -or $facts.cleaned -ne 0 -or
                    -not $cycleOutcomes.Add($resourceId) -or
                    [int]$resource.sequence -ge $lastCleanupResourceSequence -or
                    [string]::IsNullOrWhiteSpace([string]$event.detail)) {
                    throw "owned-resource '$resourceId' conflict is not in one reverse-order cleanup cycle"
                }
                $facts.conflicts++
                $lastCleanupResourceSequence = [int]$resource.sequence
            }
        }
    }
    if ($journalCreated -ne 1) {
        throw 'owned-resource journal must contain exactly one creation event'
    }
    foreach ($resource in @($Journal.resources)) {
        $facts = $lifecycle[[string]$resource.resource_id]
        $hasActiveTimestamp = -not [string]::IsNullOrEmpty([string]$resource.active_utc)
        if ($facts.prepared -ne 1 -or ($hasActiveTimestamp -and $facts.active -ne 1) -or (-not $hasActiveTimestamp -and $facts.active -ne 0) -or
            ([string]$resource.state -ceq 'cleaned' -and $facts.cleaned -ne 1) -or
            ([string]$resource.state -ceq 'conflict' -and $facts.conflicts -lt 1)) {
            throw "owned-resource '$($resource.resource_id)' lifecycle events do not match its terminal state"
        }
        if ([string]$resource.kind -ceq 'registry' -and $facts.updated -ne
            @($resource.descriptor.key_ownership | Where-Object { [string]$_.creation_disposition -cne 'pending' }).Count) {
            throw "owned-resource '$($resource.resource_id)' Registry64 disposition events do not match its descriptor"
        }
    }
    $resourceStates = @($Journal.resources | ForEach-Object { [string]$_.state })
    $rootLifecycleInvalid = switch ([string]$Journal.state) {
        'active' {
            $cleanupStarted -ne 0 -or $cleanupIncomplete -ne 0 -or $cleanupCompleted -ne 0 -or $cleanupCycleOpen -or
                @($resourceStates | Where-Object { $_ -notin @('prepared', 'active') }).Count -ne 0
            break
        }
        'cleaning' {
            -not $cleanupCycleOpen -or $cleanupCompleted -ne 0 -or $cleanupStarted -ne ($cleanupIncomplete + 1)
            break
        }
        'cleanup-conflict' {
            $cleanupCycleOpen -or $cleanupCompleted -ne 0 -or $cleanupIncomplete -lt 1 -or
                $cleanupStarted -ne $cleanupIncomplete -or @($resourceStates | Where-Object { $_ -ceq 'conflict' }).Count -eq 0
            break
        }
        'completed' {
            $cleanupCycleOpen -or $cleanupCompleted -ne 1 -or $cleanupStarted -ne ($cleanupIncomplete + 1) -or
                @($resourceStates | Where-Object { $_ -cne 'cleaned' }).Count -ne 0
            break
        }
        default { $true }
    }
    if ($rootLifecycleInvalid) {
        throw "owned-resource journal root state does not match its cleanup lifecycle events"
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
        [Parameter(Mandatory = $true)][byte[]]$Bytes,
        $Lease = $null,
        $Journal = $null
    )

    $ownsLease = $null -eq $Lease
    if ($ownsLease) {
        $Lease = Enter-WindowsOwnedJournalLease -JournalPath $JournalPath
    }
    try {
        Assert-WindowsOwnedJournalLease -Lease $Lease -JournalPath $JournalPath
        $journal = if ($null -eq $Journal) { Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $Lease -JournalPath $JournalPath } else { $Journal }
        if (-not (Test-WindowsOwnedExactPathEqual -Left ([string]$journal.journal_path) -Right $JournalPath)) {
            throw 'owned file journal object does not match the transaction lease path'
        }
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
        $resourceId = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Lease $Lease -Kind file -Descriptor $descriptor -Before $before -Expected $expected -Journal $journal
        $confirmedPath = Assert-WindowsOwnedConfinedPath -Journal $journal -Path $full -Owner 'owned file operation boundary'
        if (-not (Test-WindowsOwnedExactPathEqual -Left $confirmedPath -Right $full)) {
            throw 'owned file path changed across its prepared mutation boundary'
        }
        $stream = [IO.FileStream]::new($full, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None, 4096, [IO.FileOptions]::WriteThrough)
        try {
            $stream.Write($Bytes, 0, $Bytes.Length)
            $stream.Flush($true)
        }
        finally {
            $stream.Dispose()
        }
        Set-WindowsOwnedResourceActive -JournalPath $JournalPath -Lease $Lease -ResourceId $resourceId -Journal $journal
        return $resourceId
    }
    finally {
        if ($ownsLease) {
            Exit-WindowsOwnedJournalLease -Lease $Lease
        }
    }
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
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [switch]$AllowAncestor
    )

    $normalized = if ($AllowAncestor) {
        ConvertTo-WindowsOwnedRegistryAncestorPath -Path $Path
    }
    else {
        ConvertTo-WindowsOwnedRegistryPath -Path $Path
    }
    return $normalized.Substring('HKCU\'.Length)
}

function Test-WindowsOwnedRegistryKeyExists {
    param([Parameter(Mandatory = $true)][string]$Path)

    $subKey = Get-WindowsOwnedRegistrySubKey -Path $Path -AllowAncestor
    $base = Open-WindowsOwnedRegistry64Base
    try {
        $key = $base.OpenSubKey($subKey, $false)
        if ($null -eq $key) {
            return $false
        }
        $key.Dispose()
        return $true
    }
    finally {
        $base.Dispose()
    }
}

function Get-WindowsOwnedRegistryAncestorPlan {
    param([Parameter(Mandatory = $true)][string]$Path)

    $leaf = ConvertTo-WindowsOwnedRegistryPath -Path $Path
    $relative = $leaf.Substring('HKCU\Software\'.Length)
    $parts = @($relative -split '\\')
    $current = 'HKCU\Software'
    $existing = $current
    $seenAbsent = $false
    $absent = [Collections.Generic.List[string]]::new()
    foreach ($part in $parts) {
        $current = "$current\$part"
        if (Test-WindowsOwnedRegistryKeyExists -Path $current) {
            if ($seenAbsent) {
                throw "registry ancestor plan changed while it was being captured at '$current'"
            }
            $existing = $current
        }
        else {
            $seenAbsent = $true
            $absent.Add($current)
        }
    }
    return [pscustomobject][ordered]@{
        existing_ancestor_path = $existing
        absent_ancestor_paths = @($absent)
    }
}

function New-WindowsOwnedRegistryKeyOwnershipPlan {
    param([Parameter(Mandatory = $true)][string]$Path)

    $plan = Get-WindowsOwnedRegistryAncestorPlan -Path $Path
    $records = [Collections.Generic.List[object]]::new()
    foreach ($absentPath in @($plan.absent_ancestor_paths)) {
        $markerId = [Guid]::NewGuid().ToString('N')
        $records.Add([pscustomobject][ordered]@{
            path = [string]$absentPath
            creation_disposition = 'pending'
            marker_name = "__OxVbaOwnedKey_$markerId"
            marker_token = "oxvba-key-token-v1:$markerId"
        })
    }
    return [pscustomobject][ordered]@{
        existing_ancestor_path = [string]$plan.existing_ancestor_path
        key_ownership = @($records)
    }
}

function Assert-WindowsOwnedRegistryMutationBinding {
    param(
        [Parameter(Mandatory = $true)]$Journal,
        [Parameter(Mandatory = $true)][string]$ResourceId,
        [Parameter(Mandatory = $true)][string]$Path,
        [string]$MarkerName = '',
        [string]$MarkerToken = '',
        [string]$ValueName = '',
        $Snapshot = $null
    )

    $resource = Get-WindowsOwnedRecordedResource -Journal $Journal -ResourceId $ResourceId -Kind registry
    if ([string]$Journal.registry_view -cne $script:WindowsOwnedRegistryView) {
        throw 'registry mutation binding requires exact Registry64 journal view'
    }
    if (-not [string]::IsNullOrEmpty($ValueName)) {
        if ([string]$resource.descriptor.path -ine (ConvertTo-WindowsOwnedRegistryPath -Path $Path) -or
            [string]$resource.descriptor.value_name -ine $ValueName -or
            ($null -ne $Snapshot -and -not (Test-WindowsOwnedObjectEqual -Left $Snapshot -Right $resource.before) -and
                -not (Test-WindowsOwnedObjectEqual -Left $Snapshot -Right $resource.expected))) {
            throw "registry value mutation is not bound to exact journal resource '$ResourceId'"
        }
        return $resource
    }
    $normalized = ConvertTo-WindowsOwnedRegistryAncestorPath -Path $Path
    $records = @($resource.descriptor.key_ownership | Where-Object {
        [string]$_.path -ieq $normalized -and
        ([string]::IsNullOrEmpty($MarkerName) -or ([string]$_.marker_name -ceq $MarkerName -and [string]$_.marker_token -ceq $MarkerToken))
    })
    if ($records.Count -ne 1) {
        throw "registry key mutation is not bound to exact journal resource '$ResourceId'"
    }
    return $resource
}

function New-WindowsOwnedRegistryKeyExact {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)]$Lease,
        [Parameter(Mandatory = $true)]$Journal,
        [Parameter(Mandatory = $true)][string]$ResourceId,
        [Parameter(Mandatory = $true)][string]$Path
    )

    Assert-WindowsOwnedJournalLease -Lease $Lease -JournalPath $JournalPath
    [void](Assert-WindowsOwnedRegistryMutationBinding -Journal $Journal -ResourceId $ResourceId -Path $Path)
    Assert-WindowsOwnedX64Windows
    Initialize-WindowsOwnedRegistryNative
    $normalized = ConvertTo-WindowsOwnedRegistryAncestorPath -Path $Path -Owner 'owned Registry64 key creation'
    $subKey = Get-WindowsOwnedRegistrySubKey -Path $normalized -AllowAncestor
    $disposition = 0
    $errorCode = [OxVba.WindowsRegistryNative]::CreateCurrentUserKey64($subKey, [ref]$disposition)
    if ($errorCode -ne 0 -or $disposition -notin @(1, 2)) {
        throw "RegCreateKeyExW Registry64 failed for '$normalized' (error=$errorCode disposition=$disposition)"
    }
    return $(if ($disposition -eq 1) { 'created-new' } else { 'opened-existing' })
}

function Set-WindowsOwnedRegistryKeyMarker {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)]$Lease,
        [Parameter(Mandatory = $true)]$Journal,
        [Parameter(Mandatory = $true)][string]$ResourceId,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$MarkerName,
        [Parameter(Mandatory = $true)][string]$MarkerToken
    )

    Assert-WindowsOwnedJournalLease -Lease $Lease -JournalPath $JournalPath
    [void](Assert-WindowsOwnedRegistryMutationBinding -Journal $Journal -ResourceId $ResourceId -Path $Path -MarkerName $MarkerName -MarkerToken $MarkerToken)
    Assert-WindowsOwnedExactIdentityText -Value $MarkerName -Owner 'owned registry marker name'
    Assert-WindowsOwnedExactIdentityText -Value $MarkerToken -Owner 'owned registry marker token'
    $subKey = Get-WindowsOwnedRegistrySubKey -Path $Path -AllowAncestor
    $base = Open-WindowsOwnedRegistry64Base
    try {
        $key = $base.OpenSubKey($subKey, $true)
        if ($null -eq $key) {
            throw "new Registry64 key '$Path' disappeared before its ownership marker"
        }
        try {
            if (@($key.GetValueNames() | Where-Object { $_ -ieq $MarkerName }).Count -ne 0) {
                throw "new Registry64 key '$Path' already contains ownership marker '$MarkerName'"
            }
            $key.SetValue($MarkerName, $MarkerToken, [Microsoft.Win32.RegistryValueKind]::String)
            $key.Flush()
        }
        finally {
            $key.Dispose()
        }
    }
    finally {
        $base.Dispose()
    }
}

function Get-WindowsOwnedRegistryKeyProof {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$MarkerName
    )

    $subKey = Get-WindowsOwnedRegistrySubKey -Path $Path -AllowAncestor
    $base = Open-WindowsOwnedRegistry64Base
    try {
        $key = $base.OpenSubKey($subKey, $false)
        if ($null -eq $key) {
            return [pscustomobject][ordered]@{ key_exists = $false; marker_exists = $false; marker_kind = ''; marker_value = ''; value_names = @(); subkey_names = @() }
        }
        try {
            $valueNames = @($key.GetValueNames())
            $markerExists = @($valueNames | Where-Object { $_ -ieq $MarkerName }).Count -eq 1
            $markerKind = ''
            $markerValue = ''
            if ($markerExists) {
                $markerKind = $key.GetValueKind($MarkerName).ToString()
                $markerValue = [string]$key.GetValue($MarkerName, '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            }
            return [pscustomobject][ordered]@{
                key_exists = $true
                marker_exists = $markerExists
                marker_kind = $markerKind
                marker_value = $markerValue
                value_names = $valueNames
                subkey_names = @($key.GetSubKeyNames())
            }
        }
        finally {
            $key.Dispose()
        }
    }
    finally {
        $base.Dispose()
    }
}

function Get-WindowsOwnedRegistryValueSnapshot {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ValueName
    )

    Assert-WindowsOwnedX64Windows
    $subKey = Get-WindowsOwnedRegistrySubKey -Path $Path
    $base = Open-WindowsOwnedRegistry64Base
    try {
        $key = $base.OpenSubKey($subKey, $false)
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
    finally {
        $base.Dispose()
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
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)]$Lease,
        [Parameter(Mandatory = $true)]$Journal,
        [Parameter(Mandatory = $true)][string]$ResourceId,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ValueName,
        [Parameter(Mandatory = $true)]$Snapshot
    )

    Assert-WindowsOwnedJournalLease -Lease $Lease -JournalPath $JournalPath
    [void](Assert-WindowsOwnedRegistryMutationBinding -Journal $Journal -ResourceId $ResourceId -Path $Path -ValueName $ValueName -Snapshot $Snapshot)
    Assert-WindowsOwnedX64Windows
    $subKey = Get-WindowsOwnedRegistrySubKey -Path $Path
    $base = Open-WindowsOwnedRegistry64Base
    try {
        if ([bool]$Snapshot.exists) {
            $kind = [Microsoft.Win32.RegistryValueKind]([Enum]::Parse([Microsoft.Win32.RegistryValueKind], [string]$Snapshot.kind, $false))
            $value = ConvertFrom-WindowsOwnedRegistryData -DataBase64 ([string]$Snapshot.data_base64) -Kind $kind
            $key = $base.OpenSubKey($subKey, $true)
            if ($null -eq $key) {
                throw "exact Registry64 key '$Path' is missing; value mutation refuses implicit creation"
            }
            try {
                $key.SetValue($ValueName, $value, $kind)
                $key.Flush()
            }
            finally {
                $key.Dispose()
            }
            return
        }
        $key = $base.OpenSubKey($subKey, $true)
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
    finally {
        $base.Dispose()
    }
}

function Remove-WindowsOwnedProvenRegistryKeys {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)]$Lease,
        [Parameter(Mandatory = $true)]$Journal,
        [Parameter(Mandatory = $true)][string]$ResourceId,
        [object[]]$KeyOwnership = @()
    )

    Assert-WindowsOwnedJournalLease -Lease $Lease -JournalPath $JournalPath
    Initialize-WindowsOwnedRegistryNative
    $ordered = @($KeyOwnership)
    [Array]::Reverse($ordered)
    $removed = [Collections.Generic.List[string]]::new()
    foreach ($record in $ordered) {
        [void](Assert-WindowsOwnedRegistryMutationBinding -Journal $Journal -ResourceId $ResourceId -Path ([string]$record.path) `
            -MarkerName ([string]$record.marker_name) -MarkerToken ([string]$record.marker_token))
        $normalized = ConvertTo-WindowsOwnedRegistryAncestorPath -Path ([string]$record.path) -Owner 'owned registry key cleanup'
        $subKey = Get-WindowsOwnedRegistrySubKey -Path $normalized -AllowAncestor
        $proof = Get-WindowsOwnedRegistryKeyProof -Path $normalized -MarkerName ([string]$record.marker_name)
        if (-not [bool]$proof.key_exists) {
            continue
        }
        if ([string]$record.creation_disposition -ceq 'opened-existing') {
            continue
        }
        if (-not [bool]$proof.marker_exists) {
            throw "Registry64 key '$normalized' exists without its exact ownership marker; create-before-marker ownership is unprovable"
        }
        if ([string]$proof.marker_kind -cne 'String' -or [string]$proof.marker_value -cne [string]$record.marker_token) {
            throw "Registry64 key '$normalized' has a mismatched ownership marker; exact cleanup refuses deletion"
        }
        if (@($proof.value_names).Count -ne 1 -or [string]$proof.value_names[0] -ine [string]$record.marker_name -or
            @($proof.subkey_names).Count -ne 0) {
            throw "owned Registry64 key '$normalized' is now populated beyond its exact marker; exact cleanup refuses deletion"
        }
        $errorCode = [OxVba.WindowsRegistryNative]::DeleteCurrentUserKey64($subKey)
        if ($errorCode -notin @(0, 2)) {
            throw "RegDeleteKeyExW Registry64 failed for proven owned key '$normalized' (error=$errorCode)"
        }
        $removed.Add($normalized)
    }
    return @($removed)
}

function Set-WindowsOwnedRegistryValue {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ValueName,
        [Parameter(Mandatory = $true)]$Value,
        [Microsoft.Win32.RegistryValueKind]$Kind = [Microsoft.Win32.RegistryValueKind]::String
    )

    $lease = Enter-WindowsOwnedJournalLease -JournalPath $JournalPath
    try {
        $journal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $lease -JournalPath $JournalPath
        Assert-WindowsOwnedJournalWriter -Journal $journal
        if ([string]$journal.registry_view -cne $script:WindowsOwnedRegistryView) {
            throw "registry mutation requires exact journal view '$script:WindowsOwnedRegistryView'"
        }
        $normalized = ConvertTo-WindowsOwnedRegistryPath -Path $Path -Owner 'owned registry mutation'
        Assert-WindowsOwnedExactIdentityText -Value $ValueName -Owner 'owned registry value name'
        if (-not (@($journal.allowed_registry_paths) | Where-Object { [string]$_ -ieq $normalized })) {
            throw "registry path '$normalized' is not an exact journal allowlist entry"
        }
        $before = Get-WindowsOwnedRegistryValueSnapshot -Path $normalized -ValueName $ValueName
        $expected = New-WindowsOwnedRegistryValueSnapshot -Value $Value -Kind $Kind -KeyExists $true
        $ownershipPlan = New-WindowsOwnedRegistryKeyOwnershipPlan -Path $normalized
        $descriptor = [pscustomobject][ordered]@{
            path = $normalized
            value_name = $ValueName
            mutation_mode = 'exact-value'
            registry_view = $script:WindowsOwnedRegistryView
            existing_ancestor_path = [string]$ownershipPlan.existing_ancestor_path
            key_ownership = @($ownershipPlan.key_ownership)
        }
        $resourceId = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Lease $lease -Kind registry -Descriptor $descriptor -Before $before -Expected $expected -Journal $journal
        for ($index = 0; $index -lt @($descriptor.key_ownership).Count; $index++) {
            $record = $descriptor.key_ownership[$index]
            $disposition = New-WindowsOwnedRegistryKeyExact -JournalPath $JournalPath -Lease $lease -Journal $journal -ResourceId $resourceId -Path ([string]$record.path)
            if ($disposition -ceq 'created-new') {
                Set-WindowsOwnedRegistryKeyMarker -JournalPath $JournalPath -Lease $lease -Journal $journal -ResourceId $resourceId `
                    -Path ([string]$record.path) -MarkerName ([string]$record.marker_name) -MarkerToken ([string]$record.marker_token)
                $record.creation_disposition = 'created-owned'
            }
            else {
                $record.creation_disposition = 'opened-existing'
            }
            Set-WindowsOwnedPreparedResourceDescriptor -JournalPath $JournalPath -Lease $lease -ResourceId $resourceId `
                -Descriptor $descriptor -Detail "registry-key[$index]=$($record.creation_disposition)" -Journal $journal
        }
        Set-WindowsOwnedRegistryValueRaw -JournalPath $JournalPath -Lease $lease -Journal $journal -ResourceId $resourceId `
            -Path $normalized -ValueName $ValueName -Snapshot $expected
        Set-WindowsOwnedResourceActive -JournalPath $JournalPath -Lease $lease -ResourceId $resourceId -Journal $journal
        return $resourceId
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $lease
    }
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

    $lease = Enter-WindowsOwnedJournalLease -JournalPath $JournalPath
    try {
        $journal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $lease -JournalPath $JournalPath
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
        $resourceId = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Lease $lease -Kind process -Descriptor $descriptor -Before $before -Expected $expected -Journal $journal

        $confirmedScript = Assert-WindowsOwnedConfinedPath -Journal $journal -Path $script -Owner 'owned child script operation boundary'
        $confirmedActivation = Assert-WindowsOwnedConfinedPath -Journal $journal -Path $activation -Owner 'owned child activation operation boundary'
        $scriptActual = Get-WindowsOwnedFileSnapshot -Path $confirmedScript
        if (-not (Test-WindowsOwnedExactPathEqual -Left $confirmedScript -Right $script) -or
            -not (Test-WindowsOwnedExactPathEqual -Left $confirmedActivation -Right $activation) -or
            (Test-Path -LiteralPath $confirmedActivation) -or
            -not (Test-WindowsOwnedObjectEqual -Left $scriptActual -Right $scriptResource[0].expected)) {
            throw 'owned child script/activation changed across its prepared process boundary'
        }

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
            Set-WindowsOwnedResourceActive -JournalPath $JournalPath -Lease $lease -ResourceId $resourceId -Descriptor $descriptor -Journal $journal
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
        [void](New-WindowsOwnedFile -JournalPath $JournalPath -Path $activation -Bytes ([Text.UTF8Encoding]::new($false).GetBytes($resourceId)) -Lease $lease -Journal $journal)
        return $resourceId
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $lease
    }
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

    $lease = Enter-WindowsOwnedJournalLease -JournalPath $JournalPath
    try {
        $journal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $lease -JournalPath $JournalPath
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
        $id = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Lease $lease -Kind apartment -Descriptor $descriptor `
            -Before ([pscustomobject][ordered]@{ registered = $false }) -Expected ([pscustomobject][ordered]@{ registered = $true }) -Journal $journal
        Set-WindowsOwnedResourceActive -JournalPath $JournalPath -Lease $lease -ResourceId $id -Journal $journal
        return $id
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $lease
    }
}

function Register-WindowsOwnedCallback {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][string]$ApartmentResourceId,
        [Parameter(Mandatory = $true)][string]$SessionId,
        [Parameter(Mandatory = $true)][string]$ThunkId
    )

    $lease = Enter-WindowsOwnedJournalLease -JournalPath $JournalPath
    try {
        $journal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $lease -JournalPath $JournalPath
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
        $id = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Lease $lease -Kind callback -Descriptor $descriptor `
            -Before ([pscustomobject][ordered]@{ registered = $false }) -Expected ([pscustomobject][ordered]@{ registered = $true }) -Journal $journal
        Set-WindowsOwnedResourceActive -JournalPath $JournalPath -Lease $lease -ResourceId $id -Journal $journal
        return $id
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $lease
    }
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

    $lease = Enter-WindowsOwnedJournalLease -JournalPath $JournalPath
    try {
        $journal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $lease -JournalPath $JournalPath
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
        $id = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Lease $lease -Kind connection -Descriptor $descriptor `
            -Before ([pscustomobject][ordered]@{ advised = $false }) -Expected ([pscustomobject][ordered]@{ advised = $true }) -Journal $journal
        Set-WindowsOwnedResourceActive -JournalPath $JournalPath -Lease $lease -ResourceId $id -Journal $journal
        return $id
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $lease
    }
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

    $lease = Enter-WindowsOwnedJournalLease -JournalPath $JournalPath
    try {
        $journal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $lease -JournalPath $JournalPath
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
        $id = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Lease $lease -Kind dialog -Descriptor $descriptor `
            -Before ([pscustomobject][ordered]@{ registered = $false }) -Expected ([pscustomobject][ordered]@{ registered = $true }) -Journal $journal
        Set-WindowsOwnedResourceActive -JournalPath $JournalPath -Lease $lease -ResourceId $id -Journal $journal
        return $id
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $lease
    }
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
        [Parameter(Mandatory = $true)]$Resource,
        [Parameter(Mandatory = $true)]$Lease
    )

    Assert-WindowsOwnedJournalLease -Lease $Lease -JournalPath ([string]$Journal.journal_path)
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
            $confirmedPath = Assert-WindowsOwnedConfinedPath -Journal $Journal -Path $path -Owner 'owned file cleanup operation boundary'
            if (-not (Test-WindowsOwnedExactPathEqual -Left $confirmedPath -Right $path)) {
                throw "owned file '$path' changed across its cleanup boundary"
            }
            [IO.File]::Delete($confirmedPath)
            return 'delete-exact-file'
        }
        'registry' {
            $path = ConvertTo-WindowsOwnedRegistryPath -Path ([string]$Resource.descriptor.path) -Owner 'owned registry cleanup'
            $name = [string]$Resource.descriptor.value_name
            $actual = Get-WindowsOwnedRegistryValueSnapshot -Path $path -ValueName $name
            $keyOwnership = @($Resource.descriptor.key_ownership)
            if (Test-WindowsOwnedObjectEqual -Left $actual -Right $Resource.before) {
                [void](Remove-WindowsOwnedProvenRegistryKeys -JournalPath ([string]$Journal.journal_path) -Lease $Lease -Journal $Journal `
                    -ResourceId ([string]$Resource.resource_id) -KeyOwnership $keyOwnership)
                return 'already-before-and-remove-token-proven-Registry64-keys'
            }
            if (Test-WindowsOwnedObjectEqual -Left $actual -Right $Resource.expected) {
                Set-WindowsOwnedRegistryValueRaw -JournalPath ([string]$Journal.journal_path) -Lease $Lease -Journal $Journal `
                    -ResourceId ([string]$Resource.resource_id) -Path $path -ValueName $name -Snapshot $Resource.before
            }
            elseif (-not [bool]$actual.exists -and -not [bool]$Resource.before.exists -and $keyOwnership.Count -gt 0) {
                # Crash window before value mutation, or after its inverse but
                # before token-proven key deletion.
            }
            else {
                throw "owned registry value '$path::$name' drifted from both its before and expected snapshots"
            }
            [void](Remove-WindowsOwnedProvenRegistryKeys -JournalPath ([string]$Journal.journal_path) -Lease $Lease -Journal $Journal `
                -ResourceId ([string]$Resource.resource_id) -KeyOwnership $keyOwnership)
            $restored = Get-WindowsOwnedRegistryValueSnapshot -Path $path -ValueName $name
            $openedExisting = @($keyOwnership | Where-Object { [string]$_.creation_disposition -ceq 'opened-existing' }).Count -gt 0
            $restoredExact = Test-WindowsOwnedObjectEqual -Left $restored -Right $Resource.before
            $preservedExternalLeaf = -not [bool]$Resource.before.exists -and -not [bool]$restored.exists -and $openedExisting
            if (-not $restoredExact -and -not $preservedExternalLeaf) {
                throw "owned registry value '$path::$name' did not reach its exact before snapshot"
            }
            return 'restore-exact-Registry64-value-and-token-proven-keys'
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

    $lease = Enter-WindowsOwnedJournalLease -JournalPath $JournalPath
    try {
        $journal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $lease -JournalPath $JournalPath
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
        Write-WindowsOwnedResourceJournal -Journal $journal -Lease $lease

        $conflicts = [Collections.Generic.List[string]]::new()
        $ordered = @($journal.resources | Sort-Object -Property @{ Expression = { [int]$_.sequence }; Descending = $true })
        foreach ($resource in $ordered) {
            if ([string]$resource.state -ceq 'cleaned') {
                continue
            }
            try {
                $action = Invoke-WindowsOwnedSingleResourceCleanup -Journal $journal -Resource $resource -Lease $lease
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
            Write-WindowsOwnedResourceJournal -Journal $journal -Lease $lease
        }

        if ($conflicts.Count -gt 0) {
            $journal.state = 'cleanup-conflict'
            Add-WindowsOwnedJournalEvent -Journal $journal -Event 'cleanup-incomplete' -Detail ($conflicts -join ' | ')
            Write-WindowsOwnedResourceJournal -Journal $journal -Lease $lease
            throw "owned-resource cleanup stopped at exact-resource conflicts: $($conflicts -join ' | ')"
        }
        $journal.state = 'completed'
        Add-WindowsOwnedJournalEvent -Journal $journal -Event 'cleanup-completed' -Detail 'reverse-order; idempotent; zero-unrelated-mutation'
        Write-WindowsOwnedResourceJournal -Journal $journal -Lease $lease
        return $journal
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $lease
    }
}
