[CmdletBinding()]
param(
    [string]$RepositoryRoot = (Split-Path -Parent $PSScriptRoot)
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repository = [IO.Path]::GetFullPath($RepositoryRoot)
$libraryPath = Join-Path $PSScriptRoot 'lib-windows-owned-resource-policy.ps1'
. $libraryPath

if (-not [Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([Runtime.InteropServices.OSPlatform]::Windows)) {
    throw 'WIN-0 owned-resource mutation evidence must run on Windows'
}

$script:assertionCount = 0
$script:rejectionCount = 0
$script:journalPaths = [Collections.Generic.List[string]]::new()
$script:runRoots = [Collections.Generic.List[string]]::new()
$script:tamperBackup = $null
$script:tamperJournal = $null
$script:junctionPath = $null
$script:extraRegistryPaths = [Collections.Generic.List[string]]::new()

function Assert-PolicyTrue {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )

    if (-not $Condition) {
        throw "ASSERTION FAILED: $Message"
    }
    $script:assertionCount++
}

function Assert-PolicyEqual {
    param(
        $Actual,
        $Expected,
        [Parameter(Mandatory = $true)][string]$Message
    )

    Assert-PolicyTrue -Condition (($Actual | ConvertTo-Json -Depth 16 -Compress) -ceq ($Expected | ConvertTo-Json -Depth 16 -Compress)) -Message $Message
}

function Expect-PolicyRejection {
    param(
        [Parameter(Mandatory = $true)][scriptblock]$Action,
        [Parameter(Mandatory = $true)][string]$Name,
        [string]$MessagePattern = ''
    )

    $rejected = $false
    try {
        & $Action
    }
    catch {
        if (-not [string]::IsNullOrEmpty($MessagePattern) -and $_.Exception.Message -notmatch $MessagePattern) {
            throw "policy rejection '$Name' used unexpected diagnostic: $($_.Exception.Message)"
        }
        $rejected = $true
    }
    if (-not $rejected) {
        throw "policy rejection '$Name' unexpectedly succeeded"
    }
    $script:rejectionCount++
}

function Register-TestJournal {
    param([Parameter(Mandatory = $true)][string]$JournalPath)

    $full = [IO.Path]::GetFullPath($JournalPath)
    if (-not $script:journalPaths.Contains($full)) {
        $script:journalPaths.Add($full)
    }
    $journal = Read-WindowsOwnedResourceJournal -JournalPath $full
    $root = [IO.Path]::GetFullPath([string]$journal.run_root)
    if (-not $script:runRoots.Contains($root)) {
        $script:runRoots.Add($root)
    }
    return $journal
}

function Remove-ExactEmptyDirectory {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (Test-Path -LiteralPath $Path -PathType Container) {
        try { [IO.Directory]::Delete([IO.Path]::GetFullPath($Path), $false) } catch { }
    }
}

function Open-TestRegistryKey64 {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [switch]$Writable,
        [switch]$Create
    )

    $subKey = Get-WindowsOwnedRegistrySubKey -Path $Path -AllowAncestor
    $base = Open-WindowsOwnedRegistry64Base
    try {
        if ($Create) {
            return $base.CreateSubKey($subKey, $true)
        }
        return $base.OpenSubKey($subKey, [bool]$Writable)
    }
    finally {
        $base.Dispose()
    }
}

function Remove-TestRegistryKey64IfEmpty {
    param([Parameter(Mandatory = $true)][string]$Path)

    $subKey = Get-WindowsOwnedRegistrySubKey -Path $Path -AllowAncestor
    $base = Open-WindowsOwnedRegistry64Base
    try {
        $key = $base.OpenSubKey($subKey, $false)
        if ($null -eq $key) { return }
        try { $empty = $key.ValueCount -eq 0 -and $key.SubKeyCount -eq 0 } finally { $key.Dispose() }
        if ($empty) { $base.DeleteSubKey($subKey, $false) }
    }
    finally {
        $base.Dispose()
    }
}

function Write-TestJournalObject {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)]$Journal
    )

    $Journal.journal_digest = Get-WindowsOwnedJournalDigest -Journal $Journal
    [IO.File]::WriteAllText(
        $JournalPath,
        (($Journal | ConvertTo-Json -Depth 32) + "`n"),
        [Text.UTF8Encoding]::new($false))
}

function New-TestPreparedRegistryMutation {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][string]$Path,
        [string]$ValueName = 'owned',
        [string]$Value = 'owned-value'
    )

    $lease = Enter-WindowsOwnedJournalLease -JournalPath $JournalPath
    try {
        $journal = Read-WindowsOwnedResourceJournal -JournalPath $JournalPath
        $before = Get-WindowsOwnedRegistryValueSnapshot -Path $Path -ValueName $ValueName
        $expected = New-WindowsOwnedRegistryValueSnapshot -Value $Value -Kind ([Microsoft.Win32.RegistryValueKind]::String) -KeyExists $true
        $plan = New-WindowsOwnedRegistryKeyOwnershipPlan -Path $Path
        $descriptor = [pscustomobject][ordered]@{
            path = $Path
            value_name = $ValueName
            mutation_mode = 'exact-value'
            registry_view = 'Registry64'
            existing_ancestor_path = [string]$plan.existing_ancestor_path
            key_ownership = @($plan.key_ownership)
        }
        $resourceId = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Lease $lease -Kind registry `
            -Descriptor $descriptor -Before $before -Expected $expected -Journal $journal
        return [pscustomobject]@{
            resource_id = $resourceId
            descriptor = $descriptor
            before = $before
            expected = $expected
            journal = $journal
        }
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $lease
    }
}

function Register-TestRegistryPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not $script:extraRegistryPaths.Contains($Path)) {
        $script:extraRegistryPaths.Add($Path)
    }
}

$testId = [Guid]::NewGuid().ToString('N')
$outer = Join-Path ([IO.Path]::GetTempPath()) "oxvba-owned-policy-test-$testId"
$journalDirectory = Join-Path $outer 'oxvba-owned-resource-journals'
$runDirectory = Join-Path $outer 'oxvba-owned-resource-runs'
$fileSentinel = Join-Path $outer 'neighbor-sentinel.txt'
$junctionTarget = Join-Path $outer 'junction-target'
$registryNamespacePath = 'HKCU\Software\OxVbaOwnedResourcePolicy'
$registryPath = "$registryNamespacePath\$testId"
$absentNamespacePath = "HKCU\Software\OxVbaOwnedAbsent-$testId"
$absentRegistryPath = "$absentNamespacePath\leaf"
$conflictNamespacePath = "HKCU\Software\OxVbaOwnedConflict-$testId"
$conflictRegistryPath = "$conflictNamespacePath\leaf"
$conflictAncestorValueName = "ancestor-neighbor-$testId"
$ownedValueName = "owned-$testId"
$neighborValueName = "neighbor-$testId"
$neighborValue = "neighbor-value-$testId"
$executable = [IO.Path]::GetFullPath((Get-Command pwsh -ErrorAction Stop).Source)
$currentProcessStart = Get-WindowsOwnedProcessStartUtc -ProcessId $PID
$logicalSentinel = [pscustomobject][ordered]@{ identity = "logical-$testId"; state = 'unchanged'; version = 1 }
$logicalSentinelDigest = Get-WindowsOwnedSha256Text -Text ($logicalSentinel | ConvertTo-Json -Compress)
$registryNamespaceExisted = Test-WindowsOwnedRegistryKeyExists -Path $registryNamespacePath
$absentNamespaceExisted = Test-WindowsOwnedRegistryKeyExists -Path $absentNamespacePath
$conflictNamespaceExisted = Test-WindowsOwnedRegistryKeyExists -Path $conflictNamespacePath
if ($absentNamespaceExisted -or $conflictNamespaceExisted) {
    throw 'synthetic registry namespace GUID collision'
}

[void](New-Item -ItemType Directory -Path $outer)
[void](New-Item -ItemType Directory -Path $junctionTarget)
[IO.File]::WriteAllText($fileSentinel, "neighbor-file-$testId", [Text.UTF8Encoding]::new($false))
$fileSentinelDigest = (Get-FileHash -LiteralPath $fileSentinel -Algorithm SHA256).Hash
$sentinelKey = Open-TestRegistryKey64 -Path $registryPath -Create
try {
    $sentinelKey.SetValue($neighborValueName, $neighborValue, [Microsoft.Win32.RegistryValueKind]::String)
    $sentinelKey.Flush()
}
finally {
    $sentinelKey.Dispose()
}
$registrySentinel = Get-WindowsOwnedRegistryValueSnapshot -Path $registryPath -ValueName $neighborValueName

try {
    $runIdA = New-WindowsOwnedRunId
    $runIdB = New-WindowsOwnedRunId
    Assert-PolicyTrue -Condition ($runIdA -match $script:WindowsOwnedRunIdPattern -and $runIdB -match $script:WindowsOwnedRunIdPattern -and $runIdA -cne $runIdB) -Message 'run IDs are unique and immutable-formatted'

    $repoRootJunction = Join-Path $outer 'repo-root-junction'
    [void](New-Item -ItemType Junction -Path $repoRootJunction -Target $repository)
    Expect-PolicyRejection -Name 'repository root junction' -MessagePattern 'reparse point' -Action {
        New-WindowsOwnedResourceJournal -RepositoryRoot $repoRootJunction -TempRoot $outer
    }
    [IO.Directory]::Delete($repoRootJunction, $false)

    $tempRootJunction = Join-Path $outer 'temp-root-junction'
    [void](New-Item -ItemType Junction -Path $tempRootJunction -Target $junctionTarget)
    Expect-PolicyRejection -Name 'temp root junction' -MessagePattern 'reparse point' -Action {
        New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $tempRootJunction
    }
    [IO.Directory]::Delete($tempRootJunction, $false)

    $infrastructureTemp = Join-Path $outer 'infrastructure-junction-probe'
    [void](New-Item -ItemType Directory -Path $infrastructureTemp)
    $infrastructureJunction = Join-Path $infrastructureTemp 'oxvba-owned-resource-journals'
    [void](New-Item -ItemType Junction -Path $infrastructureJunction -Target $junctionTarget)
    Expect-PolicyRejection -Name 'journal infrastructure junction' -MessagePattern 'reparse point' -Action {
        New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $infrastructureTemp
    }
    [IO.Directory]::Delete($infrastructureJunction, $false)
    [IO.Directory]::Delete($infrastructureTemp, $false)

    $boundaryJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer
    $boundaryJournal = Register-TestJournal -JournalPath $boundaryJournalPath
    [IO.Directory]::Delete([string]$boundaryJournal.run_root, $false)
    [void](New-Item -ItemType Junction -Path ([string]$boundaryJournal.run_root) -Target $junctionTarget)
    Expect-PolicyRejection -Name 'run-root operation-boundary junction' -MessagePattern 'reparse point' -Action {
        Read-WindowsOwnedResourceJournal -JournalPath $boundaryJournalPath
    }
    [IO.Directory]::Delete([string]$boundaryJournal.run_root, $false)
    [void](New-Item -ItemType Directory -Path ([string]$boundaryJournal.run_root))
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $boundaryJournalPath)

    $mainJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer `
        -AllowedRegistryPaths @($registryPath) -AllowedExecutablePaths @($executable) `
        -OrchestratorApartment STA -ReentryPolicy reject -RunId $runIdA
    $mainJournal = Register-TestJournal -JournalPath $mainJournalPath
    $fakeLease = [pscustomobject]@{ token_id = [Guid]::NewGuid().ToString('N'); acquired = $true; revalidated = $true; owner_pid = $PID; owner_thread_id = [Threading.Thread]::CurrentThread.ManagedThreadId; journal_path = $mainJournalPath; lease_name = (Get-WindowsOwnedJournalLeaseName $mainJournalPath) }
    Expect-PolicyRejection -Name 'unlocked journal mutation' -MessagePattern 'transaction lease' -Action {
        Write-WindowsOwnedResourceJournal -Journal $mainJournal -Lease $fakeLease
    }

    Expect-PolicyRejection -Name 'duplicate run ID collision' -MessagePattern 'collides' -Action {
        New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer `
            -AllowedRegistryPaths @($registryPath) -AllowedExecutablePaths @($executable) `
            -OrchestratorApartment STA -ReentryPolicy reject -RunId $runIdA
    }
    Expect-PolicyRejection -Name 'recovery refuses a live exact owner' -MessagePattern 'refuses live exact owner' -Action {
        Invoke-WindowsOwnedResourceCleanup -JournalPath $mainJournalPath -RecoveryMode
    }
    Expect-PolicyRejection -Name 'escaped file path' -MessagePattern 'outside|escapes' -Action {
        New-WindowsOwnedFile -JournalPath $mainJournalPath -Path (Join-Path (Split-Path -Parent $outer) "escaped-$testId.txt") -Bytes ([byte[]](1))
    }
    Expect-PolicyRejection -Name 'wildcard file path' -MessagePattern 'non-wildcard' -Action {
        New-WindowsOwnedFile -JournalPath $mainJournalPath -Path (Join-Path $mainJournal.run_root '*.txt') -Bytes ([byte[]](1))
    }
    Expect-PolicyRejection -Name 'controlled root deletion target' -MessagePattern 'controlled root' -Action {
        New-WindowsOwnedFile -JournalPath $mainJournalPath -Path $outer -Bytes ([byte[]](1))
    }

    $script:junctionPath = Join-Path ([string]$mainJournal.run_root) 'escape-junction'
    [void](New-Item -ItemType Junction -Path $script:junctionPath -Target $junctionTarget)
    Expect-PolicyRejection -Name 'reparse traversal' -MessagePattern 'reparse point' -Action {
        New-WindowsOwnedFile -JournalPath $mainJournalPath -Path (Join-Path $script:junctionPath 'escaped.txt') -Bytes ([byte[]](1))
    }
    [IO.Directory]::Delete($script:junctionPath, $false)
    $script:junctionPath = $null

    Expect-PolicyRejection -Name 'non-HKCU registry path' -MessagePattern 'HKCU' -Action {
        ConvertTo-WindowsOwnedRegistryPath -Path 'HKLM\Software\OxVba\Owned'
    }
    Expect-PolicyRejection -Name 'broad registry category' -MessagePattern 'leaf allowlist' -Action {
        ConvertTo-WindowsOwnedRegistryPath -Path 'HKCU\Software\Classes\CLSID'
    }
    Expect-PolicyRejection -Name 'non-allowlisted registry key' -MessagePattern 'not an exact journal allowlist' -Action {
        Set-WindowsOwnedRegistryValue -JournalPath $mainJournalPath -Path "HKCU\Software\OxVbaOwnedResourcePolicy\other-$testId" -ValueName x -Value x
    }
    Expect-PolicyRejection -Name 'wildcard registry value' -MessagePattern 'exact, non-wildcard' -Action {
        Set-WindowsOwnedRegistryValue -JournalPath $mainJournalPath -Path $registryPath -ValueName '*' -Value x
    }

    $payloadPath = Join-Path ([string]$mainJournal.run_root) 'owned-payload.bin'
    $payloadBytes = [Text.UTF8Encoding]::new($false).GetBytes("owned-payload-$testId")
    $fileResourceId = New-WindowsOwnedFile -JournalPath $mainJournalPath -Path $payloadPath -Bytes $payloadBytes
    $registryResourceId = Set-WindowsOwnedRegistryValue -JournalPath $mainJournalPath -Path $registryPath -ValueName $ownedValueName -Value "owned-value-$testId"

    $loopChildPath = Join-Path ([string]$mainJournal.run_root) 'harmless-loop-child.ps1'
    $activationPath = Join-Path ([string]$mainJournal.run_root) 'harmless-loop.activation'
    $loopChild = @'
param([string]$ActivationPath, [int]$SelfTimeoutSeconds)
$deadline = [DateTime]::UtcNow.AddSeconds($SelfTimeoutSeconds)
while ([DateTime]::UtcNow -lt $deadline -and -not (Test-Path -LiteralPath $ActivationPath)) {
    Start-Sleep -Milliseconds 25
}
if (-not (Test-Path -LiteralPath $ActivationPath)) { exit 12 }
while ([DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 50 }
'@
    [void](New-WindowsOwnedFile -JournalPath $mainJournalPath -Path $loopChildPath -Bytes ([Text.UTF8Encoding]::new($false).GetBytes($loopChild)))
    $processResourceId = Start-WindowsOwnedHarmlessChild -JournalPath $mainJournalPath -ExecutablePath $executable `
        -ScriptPath $loopChildPath -ActivationPath $activationPath -SelfTimeoutSeconds 60
    $apartmentResourceId = Register-WindowsOwnedApartment -JournalPath $mainJournalPath -Model STA `
        -ComInitialization logical-only-no-com -ReentryPolicy reject -MessagePump none -MaxReentryDepth 0
    $callbackResourceId = Register-WindowsOwnedCallback -JournalPath $mainJournalPath -ApartmentResourceId $apartmentResourceId `
        -SessionId "session-$testId" -ThunkId "thunk-$testId"
    $connectionResourceId = Register-WindowsOwnedConnection -JournalPath $mainJournalPath -ApartmentResourceId $apartmentResourceId `
        -CallbackResourceId $callbackResourceId -SourceIdentity "source-$testId" -SinkIdentity "sink-$testId" `
        -ConnectionPointIid '{00020400-0000-0000-C000-000000000046}' -Cookie 17 -WritebackPolicy copy-in-copy-out
    $dialogResourceId = Register-WindowsOwnedDialogRepresentation -JournalPath $mainJournalPath -ProcessResourceId $processResourceId `
        -UiaRuntimeId "uia-runtime-$testId" -NativeWindowHandle 424242 -Title 'logical-only synthetic dialog' -AllowedAction dismiss-exact
    $laterApartmentResourceId = Register-WindowsOwnedApartment -JournalPath $mainJournalPath -Model STA `
        -ComInitialization logical-only-no-com -ReentryPolicy reject -MessagePump none -MaxReentryDepth 0

    $mainJournal = Read-WindowsOwnedResourceJournal -JournalPath $mainJournalPath
    $processResource = Get-WindowsOwnedRecordedResource -Journal $mainJournal -ResourceId $processResourceId -Kind process -RequireActive
    $dialogResource = Get-WindowsOwnedRecordedResource -Journal $mainJournal -ResourceId $dialogResourceId -Kind dialog -RequireActive
    foreach ($resourceId in @($fileResourceId, $registryResourceId, $processResourceId)) {
        $preparedEvent = @($mainJournal.events | Where-Object { [string]$_.resource_id -ceq $resourceId -and [string]$_.event -ceq 'resource-prepared' })[0]
        $activeEvent = @($mainJournal.events | Where-Object { [string]$_.resource_id -ceq $resourceId -and [string]$_.event -ceq 'resource-active' })[0]
        Assert-PolicyTrue -Condition ([int]$preparedEvent.sequence -lt [int]$activeEvent.sequence) -Message "resource $resourceId has a durable prepared event before activation"
    }
    Assert-PolicyTrue -Condition ((Test-WindowsOwnedExactPathEqual -Left ([string]$processResource.descriptor.executable_path) -Right $executable) -and
        [int]$processResource.descriptor.pid -gt 0 -and -not [string]::IsNullOrEmpty([string]$processResource.descriptor.process_start_utc)) `
        -Message 'child cleanup identity records exact executable path, PID, and start time'
    $activationResource = @($mainJournal.resources | Where-Object { [string]$_.kind -ceq 'file' -and (Test-WindowsOwnedExactPathEqual -Left ([string]$_.descriptor.path) -Right $activationPath) })[0]
    Assert-PolicyTrue -Condition ([int]$processResource.sequence -lt [int]$activationResource.sequence) -Message 'child PID/start is durable before the activation token exists'
    Assert-PolicyTrue -Condition (Assert-WindowsOwnedCleanupIntent -JournalPath $mainJournalPath -Kind file -SelectorMode exact-recorded-file -ResourceId $fileResourceId -Selector $payloadPath) -Message 'exact file cleanup identity is accepted'
    Assert-PolicyTrue -Condition (Assert-WindowsOwnedCleanupIntent -JournalPath $mainJournalPath -Kind registry -SelectorMode exact-recorded-value -ResourceId $registryResourceId -Selector "$registryPath::$ownedValueName") -Message 'exact registry cleanup identity is accepted'
    Assert-PolicyTrue -Condition (Assert-WindowsOwnedCleanupIntent -JournalPath $mainJournalPath -Kind process -SelectorMode exact-recorded-pid-start -ResourceId $processResourceId -Selector "pid=$($processResource.descriptor.pid);start=$($processResource.descriptor.process_start_utc)") -Message 'exact process cleanup identity is accepted'
    Assert-PolicyTrue -Condition (Assert-WindowsOwnedCleanupIntent -JournalPath $mainJournalPath -Kind dialog -SelectorMode exact-recorded-process-uia -ResourceId $dialogResourceId -Selector "pid=$($dialogResource.descriptor.process_id);start=$($dialogResource.descriptor.process_start_utc);uia=$($dialogResource.descriptor.uia_runtime_id);hwnd=$($dialogResource.descriptor.native_window_handle)") -Message 'exact dialog cleanup identity is accepted'

    Expect-PolicyRejection -Name 'blanket process cleanup' -MessagePattern 'rejects blanket' -Action {
        Assert-WindowsOwnedCleanupIntent -JournalPath $mainJournalPath -Kind process -SelectorMode by-name -ResourceId $processResourceId -Selector pwsh
    }
    Expect-PolicyRejection -Name 'window-class dialog cleanup' -MessagePattern 'exact identity|rejects blanket' -Action {
        Assert-WindowsOwnedCleanupIntent -JournalPath $mainJournalPath -Kind dialog -SelectorMode window-class -ResourceId $dialogResourceId -Selector '#32770'
    }
    Expect-PolicyRejection -Name 'recursive registry cleanup' -MessagePattern 'exact identity|rejects blanket' -Action {
        Assert-WindowsOwnedCleanupIntent -JournalPath $mainJournalPath -Kind registry -SelectorMode subtree -ResourceId $registryResourceId -Selector $registryPath
    }
    Expect-PolicyRejection -Name 'recursive file cleanup' -MessagePattern 'exact identity|rejects blanket' -Action {
        Assert-WindowsOwnedCleanupIntent -JournalPath $mainJournalPath -Kind file -SelectorMode recursive -ResourceId $fileResourceId -Selector $outer
    }
    Expect-PolicyRejection -Name 'unrecorded PID resource' -MessagePattern 'not one exact' -Action {
        Assert-WindowsOwnedCleanupIntent -JournalPath $mainJournalPath -Kind process -SelectorMode exact-recorded-pid-start `
            -ResourceId "process-$([Guid]::NewGuid().ToString('N'))" -Selector "pid=$PID;start=$currentProcessStart"
    }
    Expect-PolicyRejection -Name 'mismatched recorded PID/start selector' -MessagePattern 'does not match' -Action {
        Assert-WindowsOwnedCleanupIntent -JournalPath $mainJournalPath -Kind process -SelectorMode exact-recorded-pid-start `
            -ResourceId $processResourceId -Selector "pid=$PID;start=$currentProcessStart"
    }
    Expect-PolicyRejection -Name 'apartment model ambiguity' -MessagePattern 'must match' -Action {
        Register-WindowsOwnedApartment -JournalPath $mainJournalPath -Model MTA -ComInitialization logical-only-no-com -ReentryPolicy reject
    }
    Expect-PolicyRejection -Name 'callback missing apartment' -MessagePattern 'not one exact' -Action {
        Register-WindowsOwnedCallback -JournalPath $mainJournalPath -ApartmentResourceId "apartment-$([Guid]::NewGuid().ToString('N'))" -SessionId session -ThunkId thunk
    }
    Expect-PolicyRejection -Name 'callback wildcard identity' -MessagePattern 'exact, non-wildcard' -Action {
        Register-WindowsOwnedCallback -JournalPath $mainJournalPath -ApartmentResourceId $apartmentResourceId -SessionId '*' -ThunkId thunk
    }
    Expect-PolicyRejection -Name 'connection missing callback' -MessagePattern 'not one exact' -Action {
        Register-WindowsOwnedConnection -JournalPath $mainJournalPath -ApartmentResourceId $apartmentResourceId `
            -CallbackResourceId "callback-$([Guid]::NewGuid().ToString('N'))" -SourceIdentity source -SinkIdentity sink `
            -ConnectionPointIid '{00020400-0000-0000-C000-000000000046}' -Cookie 1
    }
    Expect-PolicyRejection -Name 'connection missing cookie' -MessagePattern 'ValidateRange|greater than or equal to 1' -Action {
        Register-WindowsOwnedConnection -JournalPath $mainJournalPath -ApartmentResourceId $apartmentResourceId `
            -CallbackResourceId $callbackResourceId -SourceIdentity source -SinkIdentity sink `
            -ConnectionPointIid '{00020400-0000-0000-C000-000000000046}' -Cookie 0
    }
    Expect-PolicyRejection -Name 'dialog unrecorded process' -MessagePattern 'not one exact' -Action {
        Register-WindowsOwnedDialogRepresentation -JournalPath $mainJournalPath `
            -ProcessResourceId "process-$([Guid]::NewGuid().ToString('N'))" -UiaRuntimeId uia -NativeWindowHandle 1 -Title title
    }
    Expect-PolicyRejection -Name 'dialog wildcard UIA identity' -MessagePattern 'exact, non-wildcard' -Action {
        Register-WindowsOwnedDialogRepresentation -JournalPath $mainJournalPath `
            -ProcessResourceId $processResourceId -UiaRuntimeId '*' -NativeWindowHandle 1 -Title title
    }

    Assert-PolicyTrue -Condition (Test-WindowsOwnedProcessIdentity -ProcessId ([int]$processResource.descriptor.pid) -StartUtc ([string]$processResource.descriptor.process_start_utc)) -Message 'only the exact journaled harmless child is live'
    Assert-PolicyTrue -Condition ((Get-WindowsOwnedRegistryValueSnapshot -Path $registryPath -ValueName $ownedValueName).exists) -Message 'real owned HKCU value exists before rollback'
    Assert-PolicyTrue -Condition (Test-Path -LiteralPath $payloadPath -PathType Leaf) -Message 'real confined file exists before rollback'

    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $mainJournalPath)
    $mainJournal = Read-WindowsOwnedResourceJournal -JournalPath $mainJournalPath
    Assert-PolicyTrue -Condition ([string]$mainJournal.state -ceq 'completed' -and @($mainJournal.resources | Where-Object { [string]$_.state -cne 'cleaned' }).Count -eq 0) -Message 'all main resources reached terminal cleaned state'
    $cleanupSequences = @($mainJournal.events | Where-Object { [string]$_.event -ceq 'resource-cleaned' } | ForEach-Object {
        if ([string]$_.detail -notmatch '^sequence=(\d+);action=') { throw 'cleanup event omitted exact resource sequence' }
        [int]$Matches[1]
    })
    $expectedSequences = @($mainJournal.resources | Sort-Object { [int]$_.sequence } -Descending | ForEach-Object { [int]$_.sequence })
    Assert-PolicyEqual -Actual $cleanupSequences -Expected $expectedSequences -Message 'cleanup is strict reverse acquisition order'
    $connectionCleanupEvent = @($mainJournal.events | Where-Object { [string]$_.resource_id -ceq $connectionResourceId -and [string]$_.event -ceq 'resource-cleaned' })[0]
    $callbackCleanupEvent = @($mainJournal.events | Where-Object { [string]$_.resource_id -ceq $callbackResourceId -and [string]$_.event -ceq 'resource-cleaned' })[0]
    Assert-PolicyTrue -Condition ([int]$connectionCleanupEvent.sequence -lt [int]$callbackCleanupEvent.sequence) -Message 'Unadvise is recorded before callback retirement'
    Assert-PolicyTrue -Condition (-not (Test-Path -LiteralPath $payloadPath)) -Message 'owned confined file was removed'
    Assert-PolicyTrue -Condition (-not (Get-WindowsOwnedRegistryValueSnapshot -Path $registryPath -ValueName $ownedValueName).exists) -Message 'owned HKCU value was removed'
    Assert-PolicyTrue -Condition (-not (Test-WindowsOwnedProcessIdentity -ProcessId ([int]$processResource.descriptor.pid) -StartUtc ([string]$processResource.descriptor.process_start_utc))) -Message 'owned child exact PID/start is no longer live'
    Assert-PolicyEqual -Actual (Get-WindowsOwnedRegistryValueSnapshot -Path $registryPath -ValueName $neighborValueName) -Expected $registrySentinel -Message 'neighbor registry sentinel is byte-for-byte unchanged'
    Assert-PolicyTrue -Condition (Test-WindowsOwnedRegistryKeyExists -Path $registryNamespacePath) -Message 'registry ancestor that pre-existed the journal mutation is preserved'
    Assert-PolicyTrue -Condition ((Get-FileHash -LiteralPath $fileSentinel -Algorithm SHA256).Hash -ceq $fileSentinelDigest) -Message 'neighbor file sentinel is unchanged'
    Assert-PolicyTrue -Condition (Test-WindowsOwnedProcessIdentity -ProcessId $PID -StartUtc $currentProcessStart) -Message 'unowned process sentinel is unchanged and live'
    Assert-PolicyTrue -Condition ((Get-WindowsOwnedSha256Text -Text ($logicalSentinel | ConvertTo-Json -Compress)) -ceq $logicalSentinelDigest) -Message 'logical sentinel is unchanged'
    $mainJournalHash = (Get-FileHash -LiteralPath $mainJournalPath -Algorithm SHA256).Hash
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $mainJournalPath)
    Assert-PolicyTrue -Condition ((Get-FileHash -LiteralPath $mainJournalPath -Algorithm SHA256).Hash -ceq $mainJournalHash) -Message 'second cleanup is byte-idempotent'

    $mainJournalBackup = [IO.File]::ReadAllBytes($mainJournalPath)
    try {
        $typedTamper = [Text.UTF8Encoding]::new($false, $true).GetString($mainJournalBackup) | ConvertFrom-Json -Depth 32 -DateKind String
        (@($typedTamper.resources | Where-Object { [string]$_.resource_id -ceq $processResourceId })[0]).descriptor.harmless_child = 'true'
        Write-TestJournalObject -JournalPath $mainJournalPath -Journal $typedTamper
        Expect-PolicyRejection -Name 'string boolean journal coercion' -MessagePattern 'JSON bool' -Action { Read-WindowsOwnedResourceJournal $mainJournalPath }
        [IO.File]::WriteAllBytes($mainJournalPath, $mainJournalBackup)

        $typedTamper = [Text.UTF8Encoding]::new($false, $true).GetString($mainJournalBackup) | ConvertFrom-Json -Depth 32 -DateKind String
        $typedTamper.owner_pid = [string]$typedTamper.owner_pid
        Write-TestJournalObject -JournalPath $mainJournalPath -Journal $typedTamper
        Expect-PolicyRejection -Name 'string number journal coercion' -MessagePattern 'JSON int32' -Action { Read-WindowsOwnedResourceJournal $mainJournalPath }
        [IO.File]::WriteAllBytes($mainJournalPath, $mainJournalBackup)

        $typedTamper = [Text.UTF8Encoding]::new($false, $true).GetString($mainJournalBackup) | ConvertFrom-Json -Depth 32 -DateKind String
        $typedTamper.allowed_registry_paths = 'scalar-not-array'
        Write-TestJournalObject -JournalPath $mainJournalPath -Journal $typedTamper
        Expect-PolicyRejection -Name 'scalar journal array' -MessagePattern 'JSON array' -Action { Read-WindowsOwnedResourceJournal $mainJournalPath }
        [IO.File]::WriteAllBytes($mainJournalPath, $mainJournalBackup)

        $caseDriftText = [Text.UTF8Encoding]::new($false, $true).GetString($mainJournalBackup).Replace('"registry_view":', '"Registry_View":')
        [IO.File]::WriteAllText($mainJournalPath, $caseDriftText, [Text.UTF8Encoding]::new($false))
        Expect-PolicyRejection -Name 'journal property case drift' -MessagePattern 'case-sensitive JSON property schema' -Action { Read-WindowsOwnedResourceJournal $mainJournalPath }
        [IO.File]::WriteAllBytes($mainJournalPath, $mainJournalBackup)

        $typedTamper = [Text.UTF8Encoding]::new($false, $true).GetString($mainJournalBackup) | ConvertFrom-Json -Depth 32 -DateKind String
        $typedTamper.registry_view = 'registry64'
        Write-TestJournalObject -JournalPath $mainJournalPath -Journal $typedTamper
        Expect-PolicyRejection -Name 'Registry64 view case drift' -MessagePattern 'invalid identity or digest' -Action { Read-WindowsOwnedResourceJournal $mainJournalPath }
        [IO.File]::WriteAllBytes($mainJournalPath, $mainJournalBackup)

        $typedTamper = [Text.UTF8Encoding]::new($false, $true).GetString($mainJournalBackup) | ConvertFrom-Json -Depth 32 -DateKind String
        $typedTamper | Add-Member -NotePropertyName unexpected_root_property -NotePropertyValue 'forbidden'
        Write-TestJournalObject -JournalPath $mainJournalPath -Journal $typedTamper
        Expect-PolicyRejection -Name 'unknown journal property' -MessagePattern 'exact case-sensitive JSON property schema' -Action { Read-WindowsOwnedResourceJournal $mainJournalPath }
        [IO.File]::WriteAllBytes($mainJournalPath, $mainJournalBackup)

        $typedTamper = [Text.UTF8Encoding]::new($false, $true).GetString($mainJournalBackup) | ConvertFrom-Json -Depth 32 -DateKind String
        $callbackTamper = @($typedTamper.resources | Where-Object { [string]$_.resource_id -ceq $callbackResourceId })[0]
        $callbackTamper.descriptor.apartment_resource_id = $laterApartmentResourceId
        Write-TestJournalObject -JournalPath $mainJournalPath -Journal $typedTamper
        Expect-PolicyRejection -Name 'later apartment dependency' -MessagePattern 'lifetime/apartment declaration' -Action { Read-WindowsOwnedResourceJournal $mainJournalPath }
        [IO.File]::WriteAllBytes($mainJournalPath, $mainJournalBackup)

        $typedTamper = [Text.UTF8Encoding]::new($false, $true).GetString($mainJournalBackup) | ConvertFrom-Json -Depth 32 -DateKind String
        $typedTamper.events = @($typedTamper.events | Where-Object {
            -not ([string]$_.event -ceq 'resource-active' -and [string]$_.resource_id -ceq $processResourceId)
        })
        for ($eventIndex = 0; $eventIndex -lt @($typedTamper.events).Count; $eventIndex++) { $typedTamper.events[$eventIndex].sequence = $eventIndex + 1 }
        $typedTamper.next_event_sequence = @($typedTamper.events).Count + 1
        Write-TestJournalObject -JournalPath $mainJournalPath -Journal $typedTamper
        Expect-PolicyRejection -Name 'missing lifecycle event' -MessagePattern 'lifecycle events' -Action { Read-WindowsOwnedResourceJournal $mainJournalPath }
        [IO.File]::WriteAllBytes($mainJournalPath, $mainJournalBackup)

        $typedTamper = [Text.UTF8Encoding]::new($false, $true).GetString($mainJournalBackup) | ConvertFrom-Json -Depth 32 -DateKind String
        $cleanedEvents = @($typedTamper.events | Where-Object { [string]$_.event -ceq 'resource-cleaned' })
        $firstResourceId = [string]$cleanedEvents[0].resource_id
        $firstDetail = [string]$cleanedEvents[0].detail
        $cleanedEvents[0].resource_id = [string]$cleanedEvents[-1].resource_id
        $cleanedEvents[0].detail = [string]$cleanedEvents[-1].detail
        $cleanedEvents[-1].resource_id = $firstResourceId
        $cleanedEvents[-1].detail = $firstDetail
        Write-TestJournalObject -JournalPath $mainJournalPath -Journal $typedTamper
        Expect-PolicyRejection -Name 'forward cleanup lifecycle ordering' -MessagePattern 'reverse acquisition order' -Action { Read-WindowsOwnedResourceJournal $mainJournalPath }
        [IO.File]::WriteAllBytes($mainJournalPath, $mainJournalBackup)

        $typedTamper = [Text.UTF8Encoding]::new($false, $true).GetString($mainJournalBackup) | ConvertFrom-Json -Depth 32 -DateKind String
        $typedTamper.state = 'active'
        Write-TestJournalObject -JournalPath $mainJournalPath -Journal $typedTamper
        Expect-PolicyRejection -Name 'root state lifecycle rollback' -MessagePattern 'root state does not match' -Action { Read-WindowsOwnedResourceJournal $mainJournalPath }
    }
    finally {
        [IO.File]::WriteAllBytes($mainJournalPath, $mainJournalBackup)
    }

    $tamperJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer
    [void](Register-TestJournal -JournalPath $tamperJournalPath)
    $script:tamperJournal = $tamperJournalPath
    $script:tamperBackup = [IO.File]::ReadAllBytes($tamperJournalPath)
    $tamperedText = [Text.UTF8Encoding]::new($false, $true).GetString($script:tamperBackup).Replace('"reentry_policy": "reject"', '"reentry_policy": "declared-nested"')
    [IO.File]::WriteAllText($tamperJournalPath, $tamperedText, [Text.UTF8Encoding]::new($false))
    Expect-PolicyRejection -Name 'tampered journal digest' -MessagePattern 'invalid identity or digest' -Action {
        Read-WindowsOwnedResourceJournal -JournalPath $tamperJournalPath
    }
    [IO.File]::WriteAllBytes($tamperJournalPath, $script:tamperBackup)
    $resignedRootJournal = [Text.UTF8Encoding]::new($false, $true).GetString($script:tamperBackup) | ConvertFrom-Json -Depth 32 -DateKind String
    $resignedRootJournal.repository_root = [IO.Path]::GetPathRoot($repository)
    $resignedRootJournal.journal_digest = Get-WindowsOwnedJournalDigest -Journal $resignedRootJournal
    [IO.File]::WriteAllText($tamperJournalPath, (($resignedRootJournal | ConvertTo-Json -Depth 32) + "`n"), [Text.UTF8Encoding]::new($false))
    Expect-PolicyRejection -Name 'digest-valid repository-root tamper' -MessagePattern 'invalid identity or digest' -Action {
        Read-WindowsOwnedResourceJournal -JournalPath $tamperJournalPath
    }
    [IO.File]::WriteAllBytes($tamperJournalPath, $script:tamperBackup)
    $originalTamperText = [Text.UTF8Encoding]::new($false, $true).GetString($script:tamperBackup)
    $schemaLine = '  "schema_id": "oxvba-windows-owned-resource-journal-v1",'
    $duplicateText = $originalTamperText.Replace($schemaLine, "$schemaLine`n$schemaLine")
    [IO.File]::WriteAllText($tamperJournalPath, $duplicateText, [Text.UTF8Encoding]::new($false))
    Expect-PolicyRejection -Name 'duplicate JSON property' -MessagePattern 'duplicate JSON property' -Action {
        Read-WindowsOwnedResourceJournal -JournalPath $tamperJournalPath
    }
    [IO.File]::WriteAllBytes($tamperJournalPath, $script:tamperBackup)
    $script:tamperBackup = $null
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $tamperJournalPath)

    $driftJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer
    $driftJournal = Register-TestJournal -JournalPath $driftJournalPath
    $driftPath = Join-Path ([string]$driftJournal.run_root) 'drift-probe.bin'
    $driftExpectedBytes = [Text.UTF8Encoding]::new($false).GetBytes('owned-expected')
    [void](New-WindowsOwnedFile -JournalPath $driftJournalPath -Path $driftPath -Bytes $driftExpectedBytes)
    [IO.File]::WriteAllBytes($driftPath, [Text.UTF8Encoding]::new($false).GetBytes('unrelated-drift'))
    Expect-PolicyRejection -Name 'changed owned file conflict' -MessagePattern 'drifted|conflicts' -Action {
        Invoke-WindowsOwnedResourceCleanup -JournalPath $driftJournalPath
    }
    Assert-PolicyTrue -Condition ([Text.UTF8Encoding]::new($false, $true).GetString([IO.File]::ReadAllBytes($driftPath)) -ceq 'unrelated-drift') -Message 'conflict-safe cleanup leaves changed data untouched'
    Assert-PolicyEqual -Actual (Get-WindowsOwnedRegistryValueSnapshot -Path $registryPath -ValueName $neighborValueName) -Expected $registrySentinel -Message 'conflict cleanup does not drift neighbor registry state'
    [IO.File]::WriteAllBytes($driftPath, $driftExpectedBytes)
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $driftJournalPath)
    Assert-PolicyTrue -Condition (-not (Test-Path -LiteralPath $driftPath)) -Message 'conflict is recoverable after exact expected state is restored'

    $absentAncestorJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer `
        -AllowedRegistryPaths @($absentRegistryPath)
    [void](Register-TestJournal -JournalPath $absentAncestorJournalPath)
    $absentRegistryResourceId = Set-WindowsOwnedRegistryValue -JournalPath $absentAncestorJournalPath `
        -Path $absentRegistryPath -ValueName owned -Value 'absent-ancestor-owned'
    $absentAncestorJournal = Read-WindowsOwnedResourceJournal -JournalPath $absentAncestorJournalPath
    $absentRegistryResource = Get-WindowsOwnedRecordedResource -Journal $absentAncestorJournal -ResourceId $absentRegistryResourceId -Kind registry
    Assert-PolicyTrue -Condition ([string]$absentRegistryResource.descriptor.existing_ancestor_path -ceq 'HKCU\Software' -and
        [string]$absentRegistryResource.descriptor.registry_view -ceq 'Registry64' -and
        (@($absentRegistryResource.descriptor.key_ownership | ForEach-Object { [string]$_.path }) -join '|') -ceq "$absentNamespacePath|$absentRegistryPath" -and
        @($absentRegistryResource.descriptor.key_ownership | Where-Object { [string]$_.creation_disposition -cne 'created-owned' }).Count -eq 0) `
        -Message 'prepared Registry64 record captures exact disposition and token proof for every created key'
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $absentAncestorJournalPath)
    Assert-PolicyTrue -Condition (-not (Test-WindowsOwnedRegistryKeyExists -Path $absentNamespacePath)) -Message 'exact absent registry namespace and leaf are removed deepest-first when empty'

    $conflictAncestorJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer `
        -AllowedRegistryPaths @($conflictRegistryPath)
    [void](Register-TestJournal -JournalPath $conflictAncestorJournalPath)
    [void](Set-WindowsOwnedRegistryValue -JournalPath $conflictAncestorJournalPath `
        -Path $conflictRegistryPath -ValueName owned -Value 'ancestor-conflict-owned')
    $conflictAncestorKey = Open-TestRegistryKey64 -Path $conflictNamespacePath -Writable
    try {
        $conflictAncestorKey.SetValue($conflictAncestorValueName, 'preserve-me', [Microsoft.Win32.RegistryValueKind]::String)
        $conflictAncestorKey.Flush()
    }
    finally {
        $conflictAncestorKey.Dispose()
    }
    Expect-PolicyRejection -Name 'newly populated created registry ancestor' -MessagePattern 'now populated|conflicts' -Action {
        Invoke-WindowsOwnedResourceCleanup -JournalPath $conflictAncestorJournalPath
    }
    $conflictAncestorKey = Open-TestRegistryKey64 -Path $conflictNamespacePath
    try {
        $conflictAncestorPreserved = $null -ne $conflictAncestorKey -and
            [string]$conflictAncestorKey.GetValue($conflictAncestorValueName, $null) -ceq 'preserve-me'
    }
    finally {
        if ($null -ne $conflictAncestorKey) { $conflictAncestorKey.Dispose() }
    }
    Assert-PolicyTrue -Condition $conflictAncestorPreserved -Message 'populated recorded ancestor is preserved and reported as a cleanup conflict'
    $conflictAncestorKey = Open-TestRegistryKey64 -Path $conflictNamespacePath -Writable
    try {
        $conflictAncestorKey.DeleteValue($conflictAncestorValueName, $false)
        $conflictAncestorKey.Flush()
    }
    finally {
        $conflictAncestorKey.Dispose()
    }
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $conflictAncestorJournalPath)
    Assert-PolicyTrue -Condition (-not (Test-WindowsOwnedRegistryKeyExists -Path $conflictNamespacePath)) -Message 'ancestor conflict retry removes only the now-empty recorded ancestor chain'

    $registryCrashCases = @{}
    foreach ($caseName in @('before-create', 'external-empty', 'before-marker', 'after-markers', 'actor-wins', 'value-rolled-back')) {
        $namespace = "HKCU\Software\OxVbaOwned-$caseName-$testId"
        $leaf = "$namespace\leaf"
        Register-TestRegistryPath -Path $leaf
        Register-TestRegistryPath -Path $namespace
        $caseJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer -AllowedRegistryPaths @($leaf)
        [void](Register-TestJournal -JournalPath $caseJournalPath)
        $registryCrashCases[$caseName] = [pscustomobject]@{ namespace = $namespace; leaf = $leaf; journal = $caseJournalPath }
    }

    $case = $registryCrashCases['before-create']
    [void](New-TestPreparedRegistryMutation -JournalPath $case.journal -Path $case.leaf)
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $case.journal)
    Assert-PolicyTrue -Condition (-not (Test-WindowsOwnedRegistryKeyExists -Path $case.namespace)) -Message 'crash before Registry64 key creation leaves no inferred ownership or residue'

    $case = $registryCrashCases['external-empty']
    [void](New-TestPreparedRegistryMutation -JournalPath $case.journal -Path $case.leaf)
    $externalKey = Open-TestRegistryKey64 -Path $case.leaf -Create
    $externalKey.Dispose()
    Expect-PolicyRejection -Name 'prepared record followed by external empty key' -MessagePattern 'ownership is unprovable|conflicts' -Action {
        Invoke-WindowsOwnedResourceCleanup -JournalPath $case.journal
    }
    Assert-PolicyTrue -Condition (Test-WindowsOwnedRegistryKeyExists -Path $case.leaf) -Message 'external empty key is preserved without an exact marker token'
    Remove-TestRegistryKey64IfEmpty -Path $case.leaf
    Remove-TestRegistryKey64IfEmpty -Path $case.namespace
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $case.journal)

    $case = $registryCrashCases['before-marker']
    $prepared = New-TestPreparedRegistryMutation -JournalPath $case.journal -Path $case.leaf
    $caseLease = Enter-WindowsOwnedJournalLease -JournalPath $case.journal
    try {
        $creation = New-WindowsOwnedRegistryKeyExact -JournalPath $case.journal -Lease $caseLease -Journal $prepared.journal `
            -ResourceId $prepared.resource_id -Path ([string]$prepared.descriptor.key_ownership[0].path)
        Assert-PolicyTrue -Condition ($creation -ceq 'created-new') -Message 'Win32 disposition reports the exact create-before-marker window'
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $caseLease
    }
    Expect-PolicyRejection -Name 'create-before-marker crash window' -MessagePattern 'ownership is unprovable|conflicts' -Action {
        Invoke-WindowsOwnedResourceCleanup -JournalPath $case.journal
    }
    Assert-PolicyTrue -Condition (Test-WindowsOwnedRegistryKeyExists -Path $case.namespace) -Message 'unmarked created key is preserved as a blocking ambiguity'
    Remove-TestRegistryKey64IfEmpty -Path $case.namespace
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $case.journal)

    $case = $registryCrashCases['after-markers']
    $prepared = New-TestPreparedRegistryMutation -JournalPath $case.journal -Path $case.leaf
    $caseLease = Enter-WindowsOwnedJournalLease -JournalPath $case.journal
    try {
        foreach ($record in @($prepared.descriptor.key_ownership)) {
            $creation = New-WindowsOwnedRegistryKeyExact -JournalPath $case.journal -Lease $caseLease -Journal $prepared.journal `
                -ResourceId $prepared.resource_id -Path ([string]$record.path)
            if ($creation -cne 'created-new') { throw 'synthetic marker crash key was not newly created' }
            Set-WindowsOwnedRegistryKeyMarker -JournalPath $case.journal -Lease $caseLease -Journal $prepared.journal `
                -ResourceId $prepared.resource_id -Path ([string]$record.path) `
                -MarkerName ([string]$record.marker_name) -MarkerToken ([string]$record.marker_token)
        }
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $caseLease
    }
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $case.journal)
    Assert-PolicyTrue -Condition (-not (Test-WindowsOwnedRegistryKeyExists -Path $case.namespace)) -Message 'pending records with exact markers prove and remove every crash-created key'

    $case = $registryCrashCases['actor-wins']
    $prepared = New-TestPreparedRegistryMutation -JournalPath $case.journal -Path $case.leaf
    $externalKey = Open-TestRegistryKey64 -Path $case.leaf -Create
    $externalKey.Dispose()
    $caseLease = Enter-WindowsOwnedJournalLease -JournalPath $case.journal
    try {
        for ($keyIndex = 0; $keyIndex -lt @($prepared.descriptor.key_ownership).Count; $keyIndex++) {
            $record = $prepared.descriptor.key_ownership[$keyIndex]
            $creation = New-WindowsOwnedRegistryKeyExact -JournalPath $case.journal -Lease $caseLease -Journal $prepared.journal `
                -ResourceId $prepared.resource_id -Path ([string]$record.path)
            if ($creation -cne 'opened-existing') { throw 'external actor did not win Registry64 creation' }
            $record.creation_disposition = 'opened-existing'
            Set-WindowsOwnedPreparedResourceDescriptor -JournalPath $case.journal -Lease $caseLease -ResourceId $prepared.resource_id `
                -Descriptor $prepared.descriptor -Detail "registry-key[$keyIndex]=opened-existing"
        }
        Set-WindowsOwnedRegistryValueRaw -JournalPath $case.journal -Lease $caseLease -Journal $prepared.journal `
            -ResourceId $prepared.resource_id -Path $case.leaf -ValueName owned -Snapshot $prepared.expected
        Set-WindowsOwnedResourceActive -JournalPath $case.journal -Lease $caseLease -ResourceId $prepared.resource_id
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $caseLease
    }
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $case.journal)
    Assert-PolicyTrue -Condition ((Test-WindowsOwnedRegistryKeyExists -Path $case.leaf) -and
        -not (Get-WindowsOwnedRegistryValueSnapshot -Path $case.leaf -ValueName owned).exists) -Message 'opened-existing actor-owned keys survive exact value rollback'
    Remove-TestRegistryKey64IfEmpty -Path $case.leaf
    Remove-TestRegistryKey64IfEmpty -Path $case.namespace

    $case = $registryCrashCases['value-rolled-back']
    $valueRollbackResourceId = Set-WindowsOwnedRegistryValue -JournalPath $case.journal -Path $case.leaf -ValueName owned -Value owned-value
    $valueRollbackJournal = Read-WindowsOwnedResourceJournal -JournalPath $case.journal
    $valueRollbackResource = Get-WindowsOwnedRecordedResource -Journal $valueRollbackJournal -ResourceId $valueRollbackResourceId -Kind registry
    $caseLease = Enter-WindowsOwnedJournalLease -JournalPath $case.journal
    try {
        Set-WindowsOwnedRegistryValueRaw -JournalPath $case.journal -Lease $caseLease -Journal $valueRollbackJournal `
            -ResourceId $valueRollbackResourceId -Path $case.leaf -ValueName owned -Snapshot $valueRollbackResource.before
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $caseLease
    }
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $case.journal)
    Assert-PolicyTrue -Condition (-not (Test-WindowsOwnedRegistryKeyExists -Path $case.namespace)) -Message 'cleanup resumes after value rollback while ownership markers remain intact'

    $staleParentPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer `
        -AllowedExecutablePaths @($executable) -OrchestratorApartment STA -ReentryPolicy reject
    $staleParent = Register-TestJournal -JournalPath $staleParentPath
    $neverActivatedPath = Join-Path ([string]$staleParent.run_root) 'never-activated.token'
    $neverActivatedDescriptor = [pscustomobject][ordered]@{
        executable_path = $executable
        pid = 0
        process_start_utc = ''
        arguments_sha256 = Get-WindowsOwnedSha256Text -Text 'prepared-before-process-start'
        activation_path = $neverActivatedPath
        parent_pid = $PID
        harmless_child = $true
        self_timeout_seconds = 5
    }
    $staleParentLease = Enter-WindowsOwnedJournalLease -JournalPath $staleParentPath
    try {
        $neverActivatedResourceId = Add-WindowsOwnedPreparedResource -JournalPath $staleParentPath -Lease $staleParentLease -Kind process `
            -Descriptor $neverActivatedDescriptor -Before ([pscustomobject][ordered]@{ exists = $false }) `
            -Expected ([pscustomobject][ordered]@{ recorded = $true })
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $staleParentLease
    }
    $staleChildScriptPath = Join-Path ([string]$staleParent.run_root) 'stale-owner-child.ps1'
    $staleActivationPath = Join-Path ([string]$staleParent.run_root) 'stale-owner.activation'
    $nestedRunId = New-WindowsOwnedRunId
    $staleChildScript = @'
param(
    [string]$ActivationPath,
    [int]$SelfTimeoutSeconds,
    [string]$LibraryPath,
    [string]$RepositoryRoot,
    [string]$TempRoot,
    [string]$NestedRunId
)
$deadline = [DateTime]::UtcNow.AddSeconds($SelfTimeoutSeconds)
while ([DateTime]::UtcNow -lt $deadline -and -not (Test-Path -LiteralPath $ActivationPath)) {
    Start-Sleep -Milliseconds 25
}
if (-not (Test-Path -LiteralPath $ActivationPath)) { exit 14 }
. $LibraryPath
$journalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $RepositoryRoot -TempRoot $TempRoot -RunId $NestedRunId
$journal = Read-WindowsOwnedResourceJournal -JournalPath $journalPath
$staleFile = Join-Path ([string]$journal.run_root) 'stale-owned-file.txt'
[void](New-WindowsOwnedFile -JournalPath $journalPath -Path $staleFile -Bytes ([Text.UTF8Encoding]::new($false).GetBytes('stale-owned')))
exit 0
'@
    [void](New-WindowsOwnedFile -JournalPath $staleParentPath -Path $staleChildScriptPath -Bytes ([Text.UTF8Encoding]::new($false).GetBytes($staleChildScript)))
    $staleProcessId = Start-WindowsOwnedHarmlessChild -JournalPath $staleParentPath -ExecutablePath $executable `
        -ScriptPath $staleChildScriptPath -ActivationPath $staleActivationPath -SelfTimeoutSeconds 30 `
        -AdditionalArguments @('-LibraryPath', $libraryPath, '-RepositoryRoot', $repository, '-TempRoot', $outer, '-NestedRunId', $nestedRunId)
    $staleParent = Read-WindowsOwnedResourceJournal -JournalPath $staleParentPath
    $staleProcess = Get-WindowsOwnedRecordedResource -Journal $staleParent -ResourceId $staleProcessId -Kind process
    $waitDeadline = [DateTime]::UtcNow.AddSeconds(20)
    while ([DateTime]::UtcNow -lt $waitDeadline -and (Test-WindowsOwnedProcessIdentity -ProcessId ([int]$staleProcess.descriptor.pid) -StartUtc ([string]$staleProcess.descriptor.process_start_utc))) {
        Start-Sleep -Milliseconds 50
    }
    Assert-PolicyTrue -Condition (-not (Test-WindowsOwnedProcessIdentity -ProcessId ([int]$staleProcess.descriptor.pid) -StartUtc ([string]$staleProcess.descriptor.process_start_utc))) -Message 'synthetic crash owner exited without cleanup'
    $nestedJournalPath = Join-Path $journalDirectory "$nestedRunId.json"
    Assert-PolicyTrue -Condition (Test-Path -LiteralPath $nestedJournalPath -PathType Leaf) -Message 'stale journal remains durably discoverable by unique run ID'
    $nestedJournal = Register-TestJournal -JournalPath $nestedJournalPath
    $staleFilePath = Join-Path ([string]$nestedJournal.run_root) 'stale-owned-file.txt'
    Assert-PolicyTrue -Condition (Test-Path -LiteralPath $staleFilePath -PathType Leaf) -Message 'stale exact owned file exists before recovery'
    Expect-PolicyRejection -Name 'non-owner normal cleanup of stale journal' -MessagePattern 'exact owner|recorded live child' -Action {
        Invoke-WindowsOwnedResourceCleanup -JournalPath $nestedJournalPath
    }
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $nestedJournalPath -RecoveryMode)
    Assert-PolicyTrue -Condition (-not (Test-Path -LiteralPath $staleFilePath)) -Message 'stale recovery removes only the exact journaled file'
    $nestedHash = (Get-FileHash -LiteralPath $nestedJournalPath -Algorithm SHA256).Hash
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $nestedJournalPath -RecoveryMode)
    Assert-PolicyTrue -Condition ((Get-FileHash -LiteralPath $nestedJournalPath -Algorithm SHA256).Hash -ceq $nestedHash) -Message 'stale recovery is byte-idempotent'
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $staleParentPath)
    $cleanedStaleParent = Read-WindowsOwnedResourceJournal -JournalPath $staleParentPath
    $neverActivatedResource = Get-WindowsOwnedRecordedResource -Journal $cleanedStaleParent -ResourceId $neverActivatedResourceId -Kind process
    Assert-PolicyTrue -Condition ([string]$neverActivatedResource.state -ceq 'cleaned' -and [int]$neverActivatedResource.descriptor.pid -eq 0 -and
        [string]::IsNullOrEmpty([string]$neverActivatedResource.active_utc)) -Message 'prepared process record that never received a PID cleans safely without discovery'

    $writerRaceJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer `
        -AllowedExecutablePaths @($executable) -OrchestratorApartment STA -ReentryPolicy reject
    $writerRaceJournal = Register-TestJournal -JournalPath $writerRaceJournalPath
    $writerScriptPath = Join-Path ([string]$writerRaceJournal.run_root) 'journal-writer-child.ps1'
    $writerGatePath = Join-Path ([string]$writerRaceJournal.run_root) 'writers.gate'
    $writerScript = @'
param(
    [string]$ActivationPath,
    [int]$SelfTimeoutSeconds,
    [string]$LibraryPath,
    [string]$JournalPath,
    [string]$WriterGatePath,
    [string]$TargetPath,
    [string]$Payload
)
$deadline = [DateTime]::UtcNow.AddSeconds($SelfTimeoutSeconds)
while ([DateTime]::UtcNow -lt $deadline -and -not (Test-Path -LiteralPath $ActivationPath)) { Start-Sleep -Milliseconds 10 }
while ([DateTime]::UtcNow -lt $deadline -and -not (Test-Path -LiteralPath $WriterGatePath)) { Start-Sleep -Milliseconds 10 }
if (-not (Test-Path -LiteralPath $ActivationPath) -or -not (Test-Path -LiteralPath $WriterGatePath)) { exit 21 }
. $LibraryPath
[void](New-WindowsOwnedFile -JournalPath $JournalPath -Path $TargetPath -Bytes ([Text.UTF8Encoding]::new($false).GetBytes($Payload)))
exit 0
'@
    [void](New-WindowsOwnedFile -JournalPath $writerRaceJournalPath -Path $writerScriptPath -Bytes ([Text.UTF8Encoding]::new($false).GetBytes($writerScript)))
    $writerProcessIds = [Collections.Generic.List[string]]::new()
    $writerTargets = [Collections.Generic.List[string]]::new()
    for ($writerIndex = 0; $writerIndex -lt 24; $writerIndex++) {
        $writerTarget = Join-Path ([string]$writerRaceJournal.run_root) ("writer-{0:D2}.txt" -f $writerIndex)
        $writerActivation = Join-Path ([string]$writerRaceJournal.run_root) ("writer-{0:D2}.activation" -f $writerIndex)
        $writerTargets.Add($writerTarget)
        $writerProcessIds.Add((Start-WindowsOwnedHarmlessChild -JournalPath $writerRaceJournalPath -ExecutablePath $executable `
            -ScriptPath $writerScriptPath -ActivationPath $writerActivation -SelfTimeoutSeconds 60 `
            -AdditionalArguments @('-LibraryPath', $libraryPath, '-JournalPath', $writerRaceJournalPath,
                '-WriterGatePath', $writerGatePath, '-TargetPath', $writerTarget, '-Payload', "writer-$writerIndex")))
    }
    [void](New-WindowsOwnedFile -JournalPath $writerRaceJournalPath -Path $writerGatePath -Bytes ([byte[]](1)))
    $writerDeadline = [DateTime]::UtcNow.AddSeconds(180)
    do {
        $writerRaceJournal = Read-WindowsOwnedResourceJournal -JournalPath $writerRaceJournalPath
        $liveWriters = @($writerProcessIds | ForEach-Object {
            $resource = Get-WindowsOwnedRecordedResource -Journal $writerRaceJournal -ResourceId $_ -Kind process
            if (Test-WindowsOwnedProcessIdentity -ProcessId ([int]$resource.descriptor.pid) -StartUtc ([string]$resource.descriptor.process_start_utc)) { $_ }
        })
        if ($liveWriters.Count -gt 0) { Start-Sleep -Milliseconds 50 }
    } while ($liveWriters.Count -gt 0 -and [DateTime]::UtcNow -lt $writerDeadline)
    Assert-PolicyTrue -Condition ($liveWriters.Count -eq 0) -Message 'all 24 exact recorded writer children exit within their bounded contract'
    $writerRaceJournal = Read-WindowsOwnedResourceJournal -JournalPath $writerRaceJournalPath
    $writerFileResources = @($writerRaceJournal.resources | Where-Object {
        if ([string]$_.kind -cne 'file') { return $false }
        $resourcePath = [string]$_.descriptor.path
        return @($writerTargets | Where-Object { Test-WindowsOwnedExactPathEqual -Left $_ -Right $resourcePath }).Count -gt 0
    })
    $writerSequences = @($writerRaceJournal.resources | ForEach-Object { [int]$_.sequence })
    $expectedWriterSequences = @(1..@($writerRaceJournal.resources).Count)
    $uniqueWriterResourceCount = @($writerFileResources | Select-Object -ExpandProperty resource_id -Unique).Count
    $missingWriterTargetCount = @($writerTargets | Where-Object { -not (Test-Path -LiteralPath $_ -PathType Leaf) }).Count
    $writerSequenceMatch = ($writerSequences -join ',') -ceq ($expectedWriterSequences -join ',')
    Assert-PolicyTrue -Condition ($writerFileResources.Count -eq 24 -and
        $uniqueWriterResourceCount -eq 24 -and $writerSequenceMatch -and $missingWriterTargetCount -eq 0) `
        -Message "24 contending writers are journaled (records=$($writerFileResources.Count); unique=$uniqueWriterResourceCount; missing=$missingWriterTargetCount; sequences=$writerSequenceMatch)"
    $journalWriteTemps = @(Get-ChildItem -LiteralPath (Split-Path -Parent $writerRaceJournalPath) -File -Filter "$(Split-Path -Leaf $writerRaceJournalPath).write-*")
    Assert-PolicyTrue -Condition ($journalWriteTemps.Count -eq 0) -Message 'serialized writers leave no lost atomic-move temporary files'

    $cleanupRaceGate = Join-Path ([string]$writerRaceJournal.run_root) 'cleanup-race.gate'
    $cleanupRaceTarget = Join-Path ([string]$writerRaceJournal.run_root) 'cleanup-race-target.txt'
    $cleanupRaceActivation = Join-Path ([string]$writerRaceJournal.run_root) 'cleanup-race.activation'
    $cleanupRaceProcessId = Start-WindowsOwnedHarmlessChild -JournalPath $writerRaceJournalPath -ExecutablePath $executable `
        -ScriptPath $writerScriptPath -ActivationPath $cleanupRaceActivation -SelfTimeoutSeconds 60 `
        -AdditionalArguments @('-LibraryPath', $libraryPath, '-JournalPath', $writerRaceJournalPath,
            '-WriterGatePath', $cleanupRaceGate, '-TargetPath', $cleanupRaceTarget, '-Payload', 'must-not-appear')
    $outerLease = Enter-WindowsOwnedJournalLease -JournalPath $writerRaceJournalPath
    try {
        [void](New-WindowsOwnedFile -JournalPath $writerRaceJournalPath -Path $cleanupRaceGate -Bytes ([byte[]](1)) -Lease $outerLease)
        Start-Sleep -Milliseconds 250
        Assert-PolicyTrue -Condition (-not (Test-Path -LiteralPath $cleanupRaceTarget)) -Message 'writer blocks behind the same whole-transaction lease before cleanup'
        [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $writerRaceJournalPath)
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $outerLease
    }
    $writerRaceJournal = Read-WindowsOwnedResourceJournal -JournalPath $writerRaceJournalPath
    $cleanupRaceProcess = Get-WindowsOwnedRecordedResource -Journal $writerRaceJournal -ResourceId $cleanupRaceProcessId -Kind process
    Assert-PolicyTrue -Condition ([string]$writerRaceJournal.state -ceq 'completed' -and
        -not (Test-Path -LiteralPath $cleanupRaceTarget) -and
        @($writerRaceJournal.resources | Where-Object { [string]$_.kind -ceq 'file' -and (Test-WindowsOwnedExactPathEqual -Left ([string]$_.descriptor.path) -Right $cleanupRaceTarget) }).Count -eq 0 -and
        -not (Test-WindowsOwnedProcessIdentity -ProcessId ([int]$cleanupRaceProcess.descriptor.pid) -StartUtc ([string]$cleanupRaceProcess.descriptor.process_start_utc))) `
        -Message 'cleanup holds the full lease, stops the recorded racing child, and leaves no unjournaled mutation'

    $abandonedJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer `
        -AllowedExecutablePaths @($executable) -OrchestratorApartment STA -ReentryPolicy reject
    $abandonedJournal = Register-TestJournal -JournalPath $abandonedJournalPath
    $abandonScriptPath = Join-Path ([string]$abandonedJournal.run_root) 'abandon-journal-mutex.ps1'
    $abandonActivationPath = Join-Path ([string]$abandonedJournal.run_root) 'abandon.activation'
    $abandonScript = @'
param([string]$ActivationPath, [int]$SelfTimeoutSeconds, [string]$LibraryPath, [string]$JournalPath)
$deadline = [DateTime]::UtcNow.AddSeconds($SelfTimeoutSeconds)
while ([DateTime]::UtcNow -lt $deadline -and -not (Test-Path -LiteralPath $ActivationPath)) { Start-Sleep -Milliseconds 10 }
if (-not (Test-Path -LiteralPath $ActivationPath)) { exit 31 }
. $LibraryPath
$lease = Enter-WindowsOwnedJournalLease -JournalPath $JournalPath
[Environment]::Exit(0)
'@
    [void](New-WindowsOwnedFile -JournalPath $abandonedJournalPath -Path $abandonScriptPath -Bytes ([Text.UTF8Encoding]::new($false).GetBytes($abandonScript)))
    $observerMutex = [Threading.Mutex]::new($false, (Get-WindowsOwnedJournalLeaseName -JournalPath $abandonedJournalPath))
    try {
        $abandonProcessId = Start-WindowsOwnedHarmlessChild -JournalPath $abandonedJournalPath -ExecutablePath $executable `
            -ScriptPath $abandonScriptPath -ActivationPath $abandonActivationPath -SelfTimeoutSeconds 30 `
            -AdditionalArguments @('-LibraryPath', $libraryPath, '-JournalPath', $abandonedJournalPath)
        $abandonedJournal = Read-WindowsOwnedResourceJournal -JournalPath $abandonedJournalPath
        $abandonProcess = Get-WindowsOwnedRecordedResource -Journal $abandonedJournal -ResourceId $abandonProcessId -Kind process
        $abandonDeadline = [DateTime]::UtcNow.AddSeconds(20)
        while ([DateTime]::UtcNow -lt $abandonDeadline -and
            (Test-WindowsOwnedProcessIdentity -ProcessId ([int]$abandonProcess.descriptor.pid) -StartUtc ([string]$abandonProcess.descriptor.process_start_utc))) {
            Start-Sleep -Milliseconds 25
        }
        $recoveryLease = Enter-WindowsOwnedJournalLease -JournalPath $abandonedJournalPath
        try {
            Assert-PolicyTrue -Condition ([bool]$recoveryLease.abandoned) -Message 'abandoned named mutex acquisition is detected explicitly'
            Expect-PolicyRejection -Name 'abandoned lease mutation before revalidation' -MessagePattern 'transaction lease' -Action {
                Write-WindowsOwnedResourceJournal -Journal $abandonedJournal -Lease $recoveryLease
            }
            $revalidatedAfterAbandon = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $recoveryLease -JournalPath $abandonedJournalPath
            Assert-PolicyTrue -Condition ([string]$revalidatedAfterAbandon.journal_digest -ceq (Get-WindowsOwnedJournalDigest $revalidatedAfterAbandon)) -Message 'abandoned lease is revalidated before any further mutation'
        }
        finally {
            Exit-WindowsOwnedJournalLease -Lease $recoveryLease
        }
    }
    finally {
        $observerMutex.Dispose()
    }
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $abandonedJournalPath)

    Assert-PolicyEqual -Actual (Get-WindowsOwnedRegistryValueSnapshot -Path $registryPath -ValueName $neighborValueName) -Expected $registrySentinel -Message 'final neighbor registry sentinel has zero drift'
    Assert-PolicyTrue -Condition ((Get-FileHash -LiteralPath $fileSentinel -Algorithm SHA256).Hash -ceq $fileSentinelDigest) -Message 'final neighbor file sentinel has zero drift'
    Assert-PolicyTrue -Condition (Test-WindowsOwnedProcessIdentity -ProcessId $PID -StartUtc $currentProcessStart) -Message 'final unowned process sentinel has zero drift'
    Assert-PolicyTrue -Condition ((Get-WindowsOwnedSha256Text -Text ($logicalSentinel | ConvertTo-Json -Compress)) -ceq $logicalSentinelDigest) -Message 'final logical sentinel has zero drift'
    Assert-PolicyTrue -Condition ($script:WindowsOwnedActiveLeases.Count -eq 0) -Message 'all per-process transaction lease tokens are released after normal, race, conflict, and recovery paths'

    "PASS: Windows owned-resource policy ($script:assertionCount assertions; $script:rejectionCount fail-closed mutations; real HKCU/file/child; logical COM/UIA only)"
}
finally {
    if ($null -ne $script:tamperBackup -and $null -ne $script:tamperJournal -and (Test-Path -LiteralPath $script:tamperJournal -PathType Leaf)) {
        try { [IO.File]::WriteAllBytes($script:tamperJournal, $script:tamperBackup) } catch { }
    }
    if ($null -ne $script:junctionPath -and (Test-Path -LiteralPath $script:junctionPath)) {
        try { [IO.Directory]::Delete($script:junctionPath, $false) } catch { }
    }
    try {
        $conflictKey = Open-TestRegistryKey64 -Path $conflictNamespacePath -Writable
        if ($null -ne $conflictKey) {
            try {
                $conflictKey.DeleteValue($conflictAncestorValueName, $false)
                $conflictKey.Flush()
            }
            finally {
                $conflictKey.Dispose()
            }
        }
    }
    catch { }
    foreach ($journalPath in @($script:journalPaths | Select-Object -Unique)) {
        if (-not (Test-Path -LiteralPath $journalPath -PathType Leaf)) { continue }
        try {
            $journal = Read-WindowsOwnedResourceJournal -JournalPath $journalPath
            if ([string]$journal.state -cne 'completed') {
                if (Test-WindowsOwnedProcessIdentity -ProcessId ([int]$journal.owner_pid) -StartUtc ([string]$journal.owner_process_start_utc)) {
                    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $journalPath)
                }
                else {
                    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $journalPath -RecoveryMode)
                }
            }
        }
        catch { }
    }

    try {
        $key = Open-TestRegistryKey64 -Path $registryPath -Writable
        if ($null -ne $key) {
            try {
                $key.DeleteValue($ownedValueName, $false)
                $key.DeleteValue($neighborValueName, $false)
                $key.Flush()
                $empty = $key.ValueCount -eq 0 -and $key.SubKeyCount -eq 0
            }
            finally {
                $key.Dispose()
            }
            if ($empty) {
                Remove-TestRegistryKey64IfEmpty -Path $registryPath
            }
        }
    }
    catch { }
    foreach ($extraRegistryPath in @($script:extraRegistryPaths | Select-Object -Unique | Sort-Object { $_.Length } -Descending)) {
        try { Remove-TestRegistryKey64IfEmpty -Path $extraRegistryPath } catch { }
    }
    foreach ($ownedAbsentNamespace in @(
        [pscustomobject]@{ path = $registryNamespacePath; existed = $registryNamespaceExisted },
        [pscustomobject]@{ path = $absentNamespacePath; existed = $absentNamespaceExisted },
        [pscustomobject]@{ path = $conflictNamespacePath; existed = $conflictNamespaceExisted }
    )) {
        if (-not [bool]$ownedAbsentNamespace.existed) {
            try { Remove-TestRegistryKey64IfEmpty -Path ([string]$ownedAbsentNamespace.path) } catch { }
        }
    }

    if (Test-Path -LiteralPath $fileSentinel -PathType Leaf) {
        [IO.File]::Delete($fileSentinel)
    }
    foreach ($journalPath in @($script:journalPaths | Select-Object -Unique)) {
        if (Test-Path -LiteralPath $journalPath -PathType Leaf) {
            try {
                $journal = Read-WindowsOwnedResourceJournal -JournalPath $journalPath
                if ([string]$journal.state -ceq 'completed') { [IO.File]::Delete($journalPath) }
            }
            catch { }
        }
    }
    foreach ($root in @($script:runRoots | Select-Object -Unique | Sort-Object { $_.Length } -Descending)) {
        Remove-ExactEmptyDirectory -Path $root
    }
    Remove-ExactEmptyDirectory -Path $junctionTarget
    Remove-ExactEmptyDirectory -Path $runDirectory
    Remove-ExactEmptyDirectory -Path $journalDirectory
    Remove-ExactEmptyDirectory -Path $outer
}
