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
$script:bodyError = ''

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
        [IO.Directory]::Delete([IO.Path]::GetFullPath($Path), $false)
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
        $journal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $lease -JournalPath $JournalPath
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

function New-TestPreparedFileMutation {
    param(
        [Parameter(Mandatory = $true)][string]$JournalPath,
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][byte[]]$Bytes
    )

    $lease = Enter-WindowsOwnedJournalLease -JournalPath $JournalPath
    try {
        $journal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $lease -JournalPath $JournalPath
        $full = Assert-WindowsOwnedConfinedPath -Journal $journal -Path $Path -Owner 'synthetic prepared file'
        $before = Get-WindowsOwnedFileSnapshot -Path $full
        if ([bool]$before.exists) {
            throw 'synthetic prepared file requires an absent path'
        }
        $expected = [pscustomobject][ordered]@{
            exists = $true
            length = [long]$Bytes.Length
            sha256 = Get-WindowsOwnedSha256Bytes -Bytes $Bytes
        }
        $descriptor = [pscustomobject][ordered]@{
            path = $full
            mutation_mode = 'create-only'
            creation_disposition = 'pending'
            volume_serial_hex = ''
            file_id_hex = ''
        }
        $resourceId = Add-WindowsOwnedPreparedResource -JournalPath $JournalPath -Lease $lease -Kind file `
            -Descriptor $descriptor -Before $before -Expected $expected -Journal $journal
        return [pscustomobject]@{
            resource_id = $resourceId
            descriptor = $descriptor
            before = $before
            expected = $expected
            journal = $journal
            path = $full
            bytes = $Bytes
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

    $supportedOuter = Assert-WindowsOwnedSupportedLocalPath -Path $outer -Owner 'test local-volume admission'
    $supportedDrive = [IO.DriveInfo]::new([IO.Path]::GetPathRoot($supportedOuter))
    Assert-PolicyTrue -Condition ($supportedDrive.DriveType -eq [IO.DriveType]::Fixed -and
        $supportedDrive.DriveFormat -in @('NTFS', 'ReFS')) `
        -Message 'owned-resource acceptance executes on a supported local fixed NTFS/ReFS volume'
    foreach ($invalidPathCase in @(
        [pscustomobject]@{ name = 'drive-relative local path'; path = 'C:relative-owned-resource.txt'; pattern = 'drive-qualified|ADS/device/namespace/UNC' },
        [pscustomobject]@{ name = 'alternate data stream'; path = ([string]::Concat($outer, ':owned-stream')); pattern = 'ADS/device/namespace/UNC' },
        [pscustomobject]@{ name = 'UNC path'; path = '\\localhost\C$\oxvba-owned-resource.txt'; pattern = 'ADS/device/namespace/UNC' },
        [pscustomobject]@{ name = 'extended namespace path'; path = "\\?\$outer"; pattern = 'ADS/device/namespace/UNC' },
        [pscustomobject]@{ name = 'device namespace path'; path = '\\.\C:\oxvba-owned-resource.txt'; pattern = 'ADS/device/namespace/UNC' },
        [pscustomobject]@{ name = 'reserved device component'; path = (Join-Path $outer 'CON'); pattern = 'reserved Windows device' }
    )) {
        Expect-PolicyRejection -Name $invalidPathCase.name -MessagePattern $invalidPathCase.pattern -Action {
            Assert-WindowsOwnedSupportedLocalPath -Path ([string]$invalidPathCase.path) -Owner ([string]$invalidPathCase.name)
        }
    }

    Initialize-WindowsOwnedFileNative
    Assert-PolicyTrue -Condition ([OxVba.WindowsOwnedFileNative]::TestWriteProgressError($true, 0, 0) -eq 1117 -and
        [OxVba.WindowsOwnedFileNative]::TestWriteProgressError($false, 0, 0) -eq 1117) `
        -Message 'native journal writes fail closed when WriteFile reports zero progress or fails without a last error'
    $nativeHandleRoot = Join-Path $outer 'native-handle-publication-probe'
    $nativeTargetPath = Join-Path $nativeHandleRoot 'target.tmp'
    $nativeReplacementPath = Join-Path $nativeHandleRoot 'replacement.tmp'
    [void](New-Item -ItemType Directory -Path $nativeHandleRoot)
    $nativeTargetHandle = $null
    $nativeReplacementHandle = $null
    try {
        $nativeError = 0
        $nativeTargetHandle = [OxVba.WindowsOwnedFileNative]::CreateWriteThroughNew(
            $nativeTargetPath, ([Text.UTF8Encoding]::new($false).GetBytes('target')), [ref]$nativeError)
        Assert-PolicyTrue -Condition ($nativeError -eq 0 -and $null -ne $nativeTargetHandle -and -not $nativeTargetHandle.IsInvalid) `
            -Message 'native publication probe retains the exact created target handle'
        $nativeError = 0
        $nativeReplacementHandle = [OxVba.WindowsOwnedFileNative]::CreateWriteThroughNew(
            $nativeReplacementPath, ([Text.UTF8Encoding]::new($false).GetBytes('replacement')), [ref]$nativeError)
        Assert-PolicyTrue -Condition ($nativeError -eq 0 -and $null -ne $nativeReplacementHandle -and -not $nativeReplacementHandle.IsInvalid) `
            -Message 'native publication probe retains the exact created replacement handle'
        Expect-PolicyRejection -Name 'handle-bound temporary path swap' -Action {
            [IO.File]::Delete($nativeReplacementPath)
        }
        $nativePublishError = [OxVba.WindowsOwnedFileNative]::PublishReplace($nativeReplacementHandle, $nativeTargetPath)
        Assert-PolicyTrue -Condition ($nativePublishError -ne 0) `
            -Message 'handle-bound replacement cannot overwrite a destination whose exact handle denies delete sharing'
        Assert-PolicyTrue -Condition ([OxVba.WindowsOwnedFileNative]::DeleteOpened($nativeReplacementHandle) -eq 0 -and
            [OxVba.WindowsOwnedFileNative]::DeleteOpened($nativeTargetHandle) -eq 0) `
            -Message 'failed publication cleans both exact opened objects through retained handles'
        $nativeReplacementHandle.Dispose()
        $nativeTargetHandle.Dispose()
        Assert-PolicyTrue -Condition (-not (Test-Path -LiteralPath $nativeReplacementPath) -and
            -not (Test-Path -LiteralPath $nativeTargetPath)) `
            -Message 'handle-disposition cleanup leaves no path-selected temporary residue'
        Assert-PolicyTrue -Condition ([OxVba.WindowsOwnedFileNative]::DeleteOpened($nativeTargetHandle) -ne 0) `
            -Message 'native handle cleanup reports rather than swallows an invalid/closed-handle failure'
    }
    finally {
        foreach ($nativeHandle in @($nativeReplacementHandle, $nativeTargetHandle)) {
            if ($null -ne $nativeHandle -and -not $nativeHandle.IsClosed) {
                if (-not $nativeHandle.IsInvalid) { [void][OxVba.WindowsOwnedFileNative]::DeleteOpened($nativeHandle) }
                $nativeHandle.Dispose()
            }
        }
        foreach ($nativeProbePath in @($nativeReplacementPath, $nativeTargetPath)) {
            if (Test-Path -LiteralPath $nativeProbePath -PathType Leaf) { [IO.File]::Delete($nativeProbePath) }
        }
        Remove-ExactEmptyDirectory -Path $nativeHandleRoot
    }

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

    $leaseProbeJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer `
        -AllowedExecutablePaths @($executable)
    [void](Register-TestJournal -JournalPath $leaseProbeJournalPath)
    $leaseProbe = Enter-WindowsOwnedJournalLease -JournalPath $leaseProbeJournalPath
    try {
        $leaseBoundJournal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $leaseProbe -JournalPath $leaseProbeJournalPath
        $staleLeaseJournal = Read-WindowsOwnedResourceJournal -JournalPath $leaseProbeJournalPath
        Expect-PolicyRejection -Name 'stale reread object under live lease' -MessagePattern 'stale, modified, or unbound' -Action {
            Start-WindowsOwnedJournalMutation -Lease $leaseProbe -Journal $staleLeaseJournal
        }
        $leaseBoundJournal.allowed_executable_paths = @($leaseBoundJournal.allowed_executable_paths) + @('C:\immutable-allowlist-expansion.exe')
        Expect-PolicyRejection -Name 'immutable allowlist expansion under live lease' -MessagePattern 'stale, modified, or unbound' -Action {
            Start-WindowsOwnedJournalMutation -Lease $leaseProbe -Journal $leaseBoundJournal
        }
        $leaseBoundJournal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $leaseProbe -JournalPath $leaseProbeJournalPath
        Expect-PolicyRejection -Name 'journal publication without mutation ticket' -MessagePattern 'explicit validated mutation ticket' -Action {
            Write-WindowsOwnedResourceJournal -Journal $leaseBoundJournal -Lease $leaseProbe
        }
        $leaseHistoryBackup = [IO.File]::ReadAllBytes($leaseProbeJournalPath)
        $concurrentLeaseJournal = Read-WindowsOwnedResourceJournal -JournalPath $leaseProbeJournalPath
        $concurrentLeaseJournal.updated_utc = ([DateTime]::UtcNow.AddSeconds(1)).ToString('O')
        Write-TestJournalObject -JournalPath $leaseProbeJournalPath -Journal $concurrentLeaseJournal
        Expect-PolicyRejection -Name 'concurrent signed history under live lease' -MessagePattern 'concurrent or identity-changing journal history' -Action {
            Start-WindowsOwnedJournalMutation -Lease $leaseProbe -Journal $leaseBoundJournal
        }
        [IO.File]::WriteAllBytes($leaseProbeJournalPath, $leaseHistoryBackup)
        $leaseBoundJournal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $leaseProbe -JournalPath $leaseProbeJournalPath
        Assert-PolicyTrue -Condition ([object]::ReferenceEquals($leaseProbe.bound_journal, $leaseBoundJournal) -and
            [string]$leaseProbe.bound_journal_digest -ceq [string]$leaseBoundJournal.journal_digest) `
            -Message 'lease revalidation binds the exact immutable object and canonical history digest'
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $leaseProbe
    }
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $leaseProbeJournalPath)

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
    $fileResource = Get-WindowsOwnedRecordedResource -Journal $mainJournal -ResourceId $fileResourceId -Kind file -RequireActive
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
    Assert-PolicyTrue -Condition ([string]$fileResource.descriptor.creation_disposition -ceq 'created-owned' -and
        [string]$fileResource.descriptor.volume_serial_hex -match '^[0-9a-f]{16}$' -and
        [string]$fileResource.descriptor.file_id_hex -match '^[0-9a-f]{32}$') `
        -Message 'file cleanup identity records the exact volume and file ID captured from its creation handle'
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
    $syntheticRecordedStart = '2026-01-02T03:04:05.0000000Z'
    $syntheticReusedStart = '2026-01-02T03:04:06.0000000Z'
    $script:syntheticExecutableProbeCalls = 0
    $neverExecutable = {
        param([int]$IgnoredProcessId)
        $script:syntheticExecutableProbeCalls++
        throw 'executable query must not run before an exact creation-time match'
    }
    $reusedDecision = Resolve-WindowsOwnedProcessCleanupIdentity -ProcessId 1234 `
        -RecordedStartUtc $syntheticRecordedStart -RecordedExecutablePath $executable `
        -CreationQuery { param([int]$IgnoredProcessId) [pscustomobject][ordered]@{ state = 'observed'; start_utc = $syntheticReusedStart; error_code = 0 } } `
        -ExecutableQuery $neverExecutable
    Assert-PolicyTrue -Condition ($reusedDecision -ceq 'recorded-child-already-exited-or-pid-reused' -and
        $script:syntheticExecutableProbeCalls -eq 0) -Message 'PID reuse is decided from creation time before executable identity is queried'
    $missingDecision = Resolve-WindowsOwnedProcessCleanupIdentity -ProcessId 1234 `
        -RecordedStartUtc $syntheticRecordedStart -RecordedExecutablePath $executable `
        -CreationQuery { param([int]$IgnoredProcessId) [pscustomobject][ordered]@{ state = 'missing'; start_utc = ''; error_code = 87 } } `
        -ExecutableQuery $neverExecutable
    Assert-PolicyTrue -Condition ($missingDecision -ceq 'recorded-child-already-exited' -and
        $script:syntheticExecutableProbeCalls -eq 0) -Message 'missing PID is accepted as the recorded child already gone without executable lookup'
    Expect-PolicyRejection -Name 'unverifiable live creation time' -MessagePattern 'unverifiable creation-time' -Action {
        Resolve-WindowsOwnedProcessCleanupIdentity -ProcessId 1234 `
            -RecordedStartUtc $syntheticRecordedStart -RecordedExecutablePath $executable `
            -CreationQuery { param([int]$IgnoredProcessId) [pscustomobject][ordered]@{ state = 'unverifiable'; start_utc = ''; error_code = 5 } } `
            -ExecutableQuery $neverExecutable
    }
    Assert-PolicyTrue -Condition ($script:syntheticExecutableProbeCalls -eq 0) -Message 'unverifiable live creation time fails closed before executable lookup'
    $missingAfterMatch = Resolve-WindowsOwnedProcessCleanupIdentity -ProcessId 1234 `
        -RecordedStartUtc $syntheticRecordedStart -RecordedExecutablePath $executable `
        -CreationQuery { param([int]$IgnoredProcessId) [pscustomobject][ordered]@{ state = 'observed'; start_utc = $syntheticRecordedStart; error_code = 0 } } `
        -ExecutableQuery { param([int]$IgnoredProcessId) [pscustomobject][ordered]@{ state = 'missing'; path = ''; error_code = 87 } }
    Assert-PolicyTrue -Condition ($missingAfterMatch -ceq 'recorded-child-already-exited') -Message 'process exit after a matching creation-time query is harmless'
    Expect-PolicyRejection -Name 'matching process with wrong executable' -MessagePattern 'unexpected executable identity' -Action {
        Resolve-WindowsOwnedProcessCleanupIdentity -ProcessId 1234 `
            -RecordedStartUtc $syntheticRecordedStart -RecordedExecutablePath $executable `
            -CreationQuery { param([int]$IgnoredProcessId) [pscustomobject][ordered]@{ state = 'observed'; start_utc = $syntheticRecordedStart; error_code = 0 } } `
            -ExecutableQuery { param([int]$IgnoredProcessId) [pscustomobject][ordered]@{ state = 'observed'; path = 'C:\Windows\System32\not-owned.exe'; error_code = 0 } }
    }
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

    $fileCaseBytes = [Text.UTF8Encoding]::new($false).GetBytes("file-disposition-$testId")

    $preexistingFileJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer
    $preexistingFileJournal = Register-TestJournal -JournalPath $preexistingFileJournalPath
    $preexistingFilePath = Join-Path ([string]$preexistingFileJournal.run_root) 'preexisting-file.bin'
    [IO.File]::WriteAllBytes($preexistingFilePath, $fileCaseBytes)
    Expect-PolicyRejection -Name 'pre-existing file create-only refusal' -MessagePattern 'already exists' -Action {
        New-WindowsOwnedFile -JournalPath $preexistingFileJournalPath -Path $preexistingFilePath -Bytes $fileCaseBytes
    }
    Assert-PolicyTrue -Condition ((Get-WindowsOwnedFileSnapshot $preexistingFilePath).sha256 -ceq
        (Get-WindowsOwnedSha256Bytes $fileCaseBytes)) -Message 'pre-existing file is preserved without a prepared ownership record'
    [IO.File]::Delete($preexistingFilePath)
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $preexistingFileJournalPath)

    $beforeFileCreateJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer
    $beforeFileCreateJournal = Register-TestJournal -JournalPath $beforeFileCreateJournalPath
    $beforeFileCreatePath = Join-Path ([string]$beforeFileCreateJournal.run_root) 'before-create.bin'
    [void](New-TestPreparedFileMutation -JournalPath $beforeFileCreateJournalPath -Path $beforeFileCreatePath -Bytes $fileCaseBytes)
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $beforeFileCreateJournalPath)
    Assert-PolicyTrue -Condition (-not (Test-Path -LiteralPath $beforeFileCreatePath)) -Message 'pending file record before creation cleans as already absent without inferred ownership'

    $externalFileJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer
    $externalFileJournal = Register-TestJournal -JournalPath $externalFileJournalPath
    $externalFilePath = Join-Path ([string]$externalFileJournal.run_root) 'external-winner.bin'
    [void](New-TestPreparedFileMutation -JournalPath $externalFileJournalPath -Path $externalFilePath -Bytes $fileCaseBytes)
    [IO.File]::WriteAllBytes($externalFilePath, $fileCaseBytes)
    Expect-PolicyRejection -Name 'prepared file followed by external winner' -MessagePattern 'without a durable created-owned disposition|conflicts' -Action {
        Invoke-WindowsOwnedResourceCleanup -JournalPath $externalFileJournalPath
    }
    Assert-PolicyTrue -Condition ((Get-WindowsOwnedFileSnapshot $externalFilePath).sha256 -ceq
        (Get-WindowsOwnedSha256Bytes $fileCaseBytes)) -Message 'pending external-winner file is preserved as a cleanup conflict'
    [IO.File]::Delete($externalFilePath)
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $externalFileJournalPath)

    $beforeFileDispositionJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer
    $beforeFileDispositionJournal = Register-TestJournal -JournalPath $beforeFileDispositionJournalPath
    $beforeFileDispositionPath = Join-Path ([string]$beforeFileDispositionJournal.run_root) 'before-disposition.bin'
    [void](New-TestPreparedFileMutation -JournalPath $beforeFileDispositionJournalPath -Path $beforeFileDispositionPath -Bytes $fileCaseBytes)
    $beforeDispositionStream = [IO.FileStream]::new(
        $beforeFileDispositionPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write,
        [IO.FileShare]::None, 4096, [IO.FileOptions]::WriteThrough)
    try {
        $beforeDispositionStream.Write($fileCaseBytes, 0, $fileCaseBytes.Length)
        $beforeDispositionStream.Flush($true)
    }
    finally {
        $beforeDispositionStream.Dispose()
    }
    Expect-PolicyRejection -Name 'file create before disposition crash window' -MessagePattern 'without a durable created-owned disposition|conflicts' -Action {
        Invoke-WindowsOwnedResourceCleanup -JournalPath $beforeFileDispositionJournalPath
    }
    Assert-PolicyTrue -Condition (Test-Path -LiteralPath $beforeFileDispositionPath -PathType Leaf) -Message 'file created before durable disposition remains a blocking ambiguity'
    [IO.File]::Delete($beforeFileDispositionPath)
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $beforeFileDispositionJournalPath)

    $afterFileDispositionJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer
    $afterFileDispositionJournal = Register-TestJournal -JournalPath $afterFileDispositionJournalPath
    $afterFileDispositionPath = Join-Path ([string]$afterFileDispositionJournal.run_root) 'after-disposition.bin'
    $afterFilePrepared = New-TestPreparedFileMutation -JournalPath $afterFileDispositionJournalPath -Path $afterFileDispositionPath -Bytes $fileCaseBytes
    $afterDispositionLease = Enter-WindowsOwnedJournalLease -JournalPath $afterFileDispositionJournalPath
    try {
        $afterDispositionJournal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $afterDispositionLease -JournalPath $afterFileDispositionJournalPath
        $afterDispositionResource = Get-WindowsOwnedRecordedResource -Journal $afterDispositionJournal `
            -ResourceId $afterFilePrepared.resource_id -Kind file
        $afterDispositionDescriptor = ($afterDispositionResource.descriptor | ConvertTo-Json -Depth 8 -Compress) |
            ConvertFrom-Json -Depth 8 -DateKind String
        $afterDispositionStream = [IO.FileStream]::new(
            $afterFileDispositionPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write,
            [IO.FileShare]::None, 4096, [IO.FileOptions]::WriteThrough)
        try {
            $afterDispositionStream.Write($fileCaseBytes, 0, $fileCaseBytes.Length)
            $afterDispositionStream.Flush($true)
            $afterDispositionIdentity = Get-WindowsOwnedFileIdentityFromHandle -Handle $afterDispositionStream.SafeFileHandle `
                -Owner 'synthetic file after-disposition identity'
            $afterDispositionDescriptor.volume_serial_hex = [string]$afterDispositionIdentity.volume_serial_hex
            $afterDispositionDescriptor.file_id_hex = [string]$afterDispositionIdentity.file_id_hex
        }
        finally {
            $afterDispositionStream.Dispose()
        }
        $afterDispositionDescriptor.creation_disposition = 'created-owned'
        Set-WindowsOwnedPreparedResourceDescriptor -JournalPath $afterFileDispositionJournalPath -Lease $afterDispositionLease `
            -ResourceId $afterFilePrepared.resource_id -Descriptor $afterDispositionDescriptor `
            -Detail 'file-creation=created-owned' -Journal $afterDispositionJournal
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $afterDispositionLease
    }
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $afterFileDispositionJournalPath)
    Assert-PolicyTrue -Condition (-not (Test-Path -LiteralPath $afterFileDispositionPath)) -Message 'durable created-owned file disposition permits exact cleanup before activation'

    $replacementJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer
    $replacementJournal = Register-TestJournal -JournalPath $replacementJournalPath
    $replacementPath = Join-Path ([string]$replacementJournal.run_root) 'same-content-replacement.bin'
    [void](New-WindowsOwnedFile -JournalPath $replacementJournalPath -Path $replacementPath -Bytes $fileCaseBytes)
    $replacementJournal = Read-WindowsOwnedResourceJournal -JournalPath $replacementJournalPath
    $ownedReplacementRecord = @($replacementJournal.resources | Where-Object { [string]$_.kind -ceq 'file' })[0]
    $originalIdentity = "$($ownedReplacementRecord.descriptor.volume_serial_hex):$($ownedReplacementRecord.descriptor.file_id_hex)"
    $retainedOriginal = [IO.FileStream]::new(
        $replacementPath, [IO.FileMode]::Open, [IO.FileAccess]::Read,
        ([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete))
    try {
        [IO.File]::Delete($replacementPath)
        [IO.File]::WriteAllBytes($replacementPath, $fileCaseBytes)
        $replacementStream = [IO.FileStream]::new(
            $replacementPath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
        try {
            $replacementIdentity = Get-WindowsOwnedFileIdentityFromHandle -Handle $replacementStream.SafeFileHandle `
                -Owner 'synthetic same-content replacement identity'
        }
        finally {
            $replacementStream.Dispose()
        }
        Assert-PolicyTrue -Condition ("$($replacementIdentity.volume_serial_hex):$($replacementIdentity.file_id_hex)" -cne $originalIdentity) `
            -Message 'same-content replacement regression holds the deleted original open so file identity differs deterministically'
        Expect-PolicyRejection -Name 'same-content different-file replacement' -MessagePattern 'different volume/file identity|cleanup conflicts' -Action {
            Invoke-WindowsOwnedResourceCleanup -JournalPath $replacementJournalPath
        }
        Assert-PolicyTrue -Condition ((Get-WindowsOwnedFileSnapshot -Path $replacementPath).sha256 -ceq
            (Get-WindowsOwnedSha256Bytes -Bytes $fileCaseBytes)) `
            -Message 'same-content replacement is preserved when its stable file identity is not owned'
    }
    finally {
        $retainedOriginal.Dispose()
    }
    [IO.File]::Delete($replacementPath)
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $replacementJournalPath)

    $resumeJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer
    $resumeJournal = Register-TestJournal -JournalPath $resumeJournalPath
    $resumePresentPath = Join-Path ([string]$resumeJournal.run_root) 'resume-present.bin'
    $resumeAlreadyInvertedPath = Join-Path ([string]$resumeJournal.run_root) 'resume-already-inverted.bin'
    $resumePresentId = New-WindowsOwnedFile -JournalPath $resumeJournalPath -Path $resumePresentPath -Bytes $fileCaseBytes
    $resumeAlreadyInvertedId = New-WindowsOwnedFile -JournalPath $resumeJournalPath -Path $resumeAlreadyInvertedPath -Bytes $fileCaseBytes
    $resumeLease = Enter-WindowsOwnedJournalLease -JournalPath $resumeJournalPath
    try {
        $resumeJournal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $resumeLease -JournalPath $resumeJournalPath
        $resumeMutation = Start-WindowsOwnedJournalMutation -Lease $resumeLease -Journal $resumeJournal
        $resumeJournal.state = 'cleaning'
        Add-WindowsOwnedJournalEvent -Journal $resumeJournal -Event 'cleanup-started' -Detail 'owner-initiated'
        Write-WindowsOwnedResourceJournal -Journal $resumeJournal -Lease $resumeLease -Mutation $resumeMutation
    }
    finally {
        Exit-WindowsOwnedJournalLease -Lease $resumeLease
    }
    [IO.File]::Delete($resumeAlreadyInvertedPath)
    $resumeJournal = Invoke-WindowsOwnedResourceCleanup -JournalPath $resumeJournalPath
    $resumeStartEvents = @($resumeJournal.events | Where-Object { [string]$_.event -ceq 'cleanup-started' })
    $resumeAlreadyInvertedEvent = @($resumeJournal.events | Where-Object {
        [string]$_.event -ceq 'resource-cleaned' -and [string]$_.resource_id -ceq $resumeAlreadyInvertedId
    })[0]
    Assert-PolicyTrue -Condition ([string]$resumeJournal.state -ceq 'completed' -and $resumeStartEvents.Count -eq 1 -and
        [string]$resumeAlreadyInvertedEvent.detail -match 'action=already-before' -and
        -not (Test-Path -LiteralPath $resumePresentPath) -and -not (Test-Path -LiteralPath $resumeAlreadyInvertedPath)) `
        -Message 'cleanup resumes one durable cleaning cycle exactly and idempotently after a partial inverse'

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

    $registryReplacementParent = "HKCU\Software\OxVbaOwnedRegistryReplacement-$testId"
    $registryReplacementLeaf = "$registryReplacementParent\leaf"
    $registryReplacementSentinelName = "neighbor-$testId"
    Register-TestRegistryPath -Path $registryReplacementLeaf
    Register-TestRegistryPath -Path $registryReplacementParent
    $registryReplacementParentKey = Open-TestRegistryKey64 -Path $registryReplacementParent -Create
    try {
        $registryReplacementParentKey.SetValue($registryReplacementSentinelName, 'preserve-parent', [Microsoft.Win32.RegistryValueKind]::String)
        $registryReplacementParentKey.Flush()
    }
    finally {
        $registryReplacementParentKey.Dispose()
    }
    $registryReplacementJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer `
        -AllowedRegistryPaths @($registryReplacementLeaf)
    [void](Register-TestJournal -JournalPath $registryReplacementJournalPath)
    [void](Set-WindowsOwnedRegistryValue -JournalPath $registryReplacementJournalPath -Path $registryReplacementLeaf `
        -ValueName owned -Value 'owned-before-replacement')
    $registryReplacementOwnedKey = Open-TestRegistryKey64 -Path $registryReplacementLeaf -Writable
    try {
        foreach ($name in @($registryReplacementOwnedKey.GetValueNames())) {
            $registryReplacementOwnedKey.DeleteValue($name, $false)
        }
        $registryReplacementOwnedKey.Flush()
    }
    finally {
        $registryReplacementOwnedKey.Dispose()
    }
    Remove-TestRegistryKey64IfEmpty -Path $registryReplacementLeaf
    $registryReplacementExternalKey = Open-TestRegistryKey64 -Path $registryReplacementLeaf -Create
    $registryReplacementExternalKey.Dispose()
    Expect-PolicyRejection -Name 'same-path replacement registry key' -MessagePattern 'marker|ownership|conflicts' -Action {
        Invoke-WindowsOwnedResourceCleanup -JournalPath $registryReplacementJournalPath
    }
    $registryReplacementParentKey = Open-TestRegistryKey64 -Path $registryReplacementParent
    try {
        $registryReplacementParentPreserved = (Test-WindowsOwnedRegistryKeyExists -Path $registryReplacementLeaf) -and
            [string]$registryReplacementParentKey.GetValue($registryReplacementSentinelName, $null) -ceq 'preserve-parent'
    }
    finally {
        $registryReplacementParentKey.Dispose()
    }
    Assert-PolicyTrue -Condition $registryReplacementParentPreserved `
        -Message 'same-path registry replacement and neighboring parent sentinel survive missing-marker cleanup conflict'
    Remove-TestRegistryKey64IfEmpty -Path $registryReplacementLeaf
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $registryReplacementJournalPath)
    $registryReplacementParentKey = Open-TestRegistryKey64 -Path $registryReplacementParent -Writable
    try {
        $registryReplacementParentKey.DeleteValue($registryReplacementSentinelName, $false)
        $registryReplacementParentKey.Flush()
    }
    finally {
        $registryReplacementParentKey.Dispose()
    }
    Remove-TestRegistryKey64IfEmpty -Path $registryReplacementParent

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
    foreach ($caseName in @('before-create', 'external-empty', 'after-markers', 'actor-wins', 'value-rolled-back')) {
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

    $case = $registryCrashCases['after-markers']
    $prepared = New-TestPreparedRegistryMutation -JournalPath $case.journal -Path $case.leaf
    $caseLease = Enter-WindowsOwnedJournalLease -JournalPath $case.journal
    try {
        $caseJournal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $caseLease -JournalPath $case.journal
        foreach ($record in @($prepared.descriptor.key_ownership)) {
            $creation = New-WindowsOwnedRegistryKeyExact -JournalPath $case.journal -Lease $caseLease -Journal $caseJournal `
                -ResourceId $prepared.resource_id -Path ([string]$record.path) `
                -MarkerName ([string]$record.marker_name) -MarkerToken ([string]$record.marker_token)
            if ($creation -cne 'created-new') { throw 'synthetic marker crash key was not newly created' }
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
        $caseJournal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $caseLease -JournalPath $case.journal
        for ($keyIndex = 0; $keyIndex -lt @($prepared.descriptor.key_ownership).Count; $keyIndex++) {
            $record = $prepared.descriptor.key_ownership[$keyIndex]
            $creation = New-WindowsOwnedRegistryKeyExact -JournalPath $case.journal -Lease $caseLease -Journal $caseJournal `
                -ResourceId $prepared.resource_id -Path ([string]$record.path) `
                -MarkerName ([string]$record.marker_name) -MarkerToken ([string]$record.marker_token)
            if ($creation -cne 'opened-existing') { throw 'external actor did not win Registry64 creation' }
            $record.creation_disposition = 'opened-existing'
            Set-WindowsOwnedPreparedResourceDescriptor -JournalPath $case.journal -Lease $caseLease -ResourceId $prepared.resource_id `
                -Descriptor $prepared.descriptor -Detail "registry-key[$keyIndex]=opened-existing" -Journal $caseJournal
        }
        Set-WindowsOwnedRegistryValueRaw -JournalPath $case.journal -Lease $caseLease -Journal $caseJournal `
            -ResourceId $prepared.resource_id -Path $case.leaf -ValueName owned -Snapshot $prepared.expected
        Set-WindowsOwnedResourceActive -JournalPath $case.journal -Lease $caseLease -ResourceId $prepared.resource_id -Journal $caseJournal
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
        $valueRollbackJournal = Confirm-WindowsOwnedJournalLeaseRevalidated -Lease $caseLease -JournalPath $case.journal
        $valueRollbackResource = Get-WindowsOwnedRecordedResource -Journal $valueRollbackJournal -ResourceId $valueRollbackResourceId -Kind registry
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

    $descendantJournalPath = New-WindowsOwnedResourceJournal -RepositoryRoot $repository -TempRoot $outer `
        -AllowedExecutablePaths @($executable)
    $descendantJournal = Register-TestJournal -JournalPath $descendantJournalPath
    $descendantScriptPath = Join-Path ([string]$descendantJournal.run_root) 'parent-with-descendant.ps1'
    $descendantActivationPath = Join-Path ([string]$descendantJournal.run_root) 'parent-with-descendant.activation'
    $descendantScript = @'
param([string]$ActivationPath, [int]$SelfTimeoutSeconds, [string]$ExecutablePath)
$deadline = [DateTime]::UtcNow.AddSeconds($SelfTimeoutSeconds)
while ([DateTime]::UtcNow -lt $deadline -and -not (Test-Path -LiteralPath $ActivationPath)) { Start-Sleep -Milliseconds 10 }
if (-not (Test-Path -LiteralPath $ActivationPath)) { exit 41 }
$info = [Diagnostics.ProcessStartInfo]::new()
$info.FileName = $ExecutablePath
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
[void]$info.ArgumentList.Add('-NoProfile')
[void]$info.ArgumentList.Add('-NonInteractive')
[void]$info.ArgumentList.Add('-Command')
[void]$info.ArgumentList.Add('Start-Sleep -Seconds 8')
$descendant = [Diagnostics.Process]::Start($info)
$descendant.Dispose()
while ([DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 50 }
'@
    [void](New-WindowsOwnedFile -JournalPath $descendantJournalPath -Path $descendantScriptPath `
        -Bytes ([Text.UTF8Encoding]::new($false).GetBytes($descendantScript)))
    $descendantParentResourceId = Start-WindowsOwnedHarmlessChild -JournalPath $descendantJournalPath `
        -ExecutablePath $executable -ScriptPath $descendantScriptPath -ActivationPath $descendantActivationPath `
        -SelfTimeoutSeconds 30 -AdditionalArguments @('-ExecutablePath', $executable)
    $descendantJournal = Read-WindowsOwnedResourceJournal -JournalPath $descendantJournalPath
    $descendantParentResource = Get-WindowsOwnedRecordedResource -Journal $descendantJournal `
        -ResourceId $descendantParentResourceId -Kind process -RequireActive
    $descendantPid = 0
    $descendantStartUtc = ''
    $descendantDiscoveryDeadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        foreach ($candidate in @(Get-CimInstance -ClassName Win32_Process -Filter "ParentProcessId = $([int]$descendantParentResource.descriptor.pid)")) {
            $candidatePath = [string](Get-WindowsOwnedProcessExecutablePath -ProcessId ([int]$candidate.ProcessId))
            if (-not [string]::IsNullOrEmpty($candidatePath) -and
                (Test-WindowsOwnedExactPathEqual -Left $candidatePath -Right $executable)) {
                $descendantPid = [int]$candidate.ProcessId
                $descendantStartUtc = Get-WindowsOwnedProcessStartUtc -ProcessId $descendantPid
                break
            }
        }
        if ($descendantPid -eq 0) { Start-Sleep -Milliseconds 25 }
    } while ($descendantPid -eq 0 -and [DateTime]::UtcNow -lt $descendantDiscoveryDeadline)
    Assert-PolicyTrue -Condition ($descendantPid -gt 0 -and
        (Test-WindowsOwnedProcessIdentity -ProcessId $descendantPid -StartUtc $descendantStartUtc)) `
        -Message 'descendant sentinel is discovered with exact PID/start identity while its recorded parent is live'
    [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $descendantJournalPath)
    Assert-PolicyTrue -Condition (-not (Test-WindowsOwnedProcessIdentity `
            -ProcessId ([int]$descendantParentResource.descriptor.pid) -StartUtc ([string]$descendantParentResource.descriptor.process_start_utc)) -and
        (Test-WindowsOwnedProcessIdentity -ProcessId $descendantPid -StartUtc $descendantStartUtc)) `
        -Message 'exact process cleanup terminates only the recorded parent and leaves its unrecorded descendant untouched'
    $descendantExitDeadline = [DateTime]::UtcNow.AddSeconds(15)
    while ([DateTime]::UtcNow -lt $descendantExitDeadline -and
        (Test-WindowsOwnedProcessIdentity -ProcessId $descendantPid -StartUtc $descendantStartUtc)) {
        Start-Sleep -Milliseconds 25
    }
    Assert-PolicyTrue -Condition (-not (Test-WindowsOwnedProcessIdentity -ProcessId $descendantPid -StartUtc $descendantStartUtc)) `
        -Message 'unrecorded descendant sentinel exits only by its own bounded lifetime'

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
    [int]$WriterGateTimeoutSeconds,
    [string]$TargetPath,
    [string]$Payload
)
$deadline = [DateTime]::UtcNow.AddSeconds($SelfTimeoutSeconds)
while ([DateTime]::UtcNow -lt $deadline -and -not (Test-Path -LiteralPath $ActivationPath)) { Start-Sleep -Milliseconds 10 }
if (-not (Test-Path -LiteralPath $ActivationPath)) { exit 21 }
$writerGateDeadline = [DateTime]::UtcNow.AddSeconds($WriterGateTimeoutSeconds)
while ([DateTime]::UtcNow -lt $writerGateDeadline -and -not (Test-Path -LiteralPath $WriterGatePath)) { Start-Sleep -Milliseconds 10 }
if (-not (Test-Path -LiteralPath $WriterGatePath)) { exit 22 }
. $LibraryPath
[void](New-WindowsOwnedFile -JournalPath $JournalPath -Path $TargetPath -Bytes ([Text.UTF8Encoding]::new($false).GetBytes($Payload)))
exit 0
'@
    [void](New-WindowsOwnedFile -JournalPath $writerRaceJournalPath -Path $writerScriptPath -Bytes ([Text.UTF8Encoding]::new($false).GetBytes($writerScript)))
    $writerProcessIds = [Collections.Generic.List[string]]::new()
    $writerTargets = [Collections.Generic.List[string]]::new()
    $writerCount = 12
    for ($writerIndex = 0; $writerIndex -lt $writerCount; $writerIndex++) {
        $writerTarget = Join-Path ([string]$writerRaceJournal.run_root) ("writer-{0:D2}.txt" -f $writerIndex)
        $writerActivation = Join-Path ([string]$writerRaceJournal.run_root) ("writer-{0:D2}.activation" -f $writerIndex)
        $writerTargets.Add($writerTarget)
        $writerProcessIds.Add((Start-WindowsOwnedHarmlessChild -JournalPath $writerRaceJournalPath -ExecutablePath $executable `
            -ScriptPath $writerScriptPath -ActivationPath $writerActivation -SelfTimeoutSeconds 60 `
            -AdditionalArguments @('-LibraryPath', $libraryPath, '-JournalPath', $writerRaceJournalPath,
                '-WriterGatePath', $writerGatePath, '-WriterGateTimeoutSeconds', '600',
                '-TargetPath', $writerTarget, '-Payload', "writer-$writerIndex")))
    }
    [void](New-WindowsOwnedFile -JournalPath $writerRaceJournalPath -Path $writerGatePath -Bytes ([byte[]](1)))
    $writerDeadline = [DateTime]::UtcNow.AddSeconds(600)
    do {
        $writerRaceJournal = Read-WindowsOwnedResourceJournal -JournalPath $writerRaceJournalPath
        $liveWriters = @($writerProcessIds | ForEach-Object {
            $resource = Get-WindowsOwnedRecordedResource -Journal $writerRaceJournal -ResourceId $_ -Kind process
            if (Test-WindowsOwnedProcessIdentity -ProcessId ([int]$resource.descriptor.pid) -StartUtc ([string]$resource.descriptor.process_start_utc)) { $_ }
        })
        if ($liveWriters.Count -gt 0) { Start-Sleep -Milliseconds 50 }
    } while ($liveWriters.Count -gt 0 -and [DateTime]::UtcNow -lt $writerDeadline)
    Assert-PolicyTrue -Condition ($liveWriters.Count -eq 0) -Message "all $writerCount exact recorded writer children exit within their bounded contract"
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
    Assert-PolicyTrue -Condition ($writerFileResources.Count -eq $writerCount -and
        $uniqueWriterResourceCount -eq $writerCount -and $writerSequenceMatch -and $missingWriterTargetCount -eq 0) `
        -Message "$writerCount contending writers are journaled (records=$($writerFileResources.Count); unique=$uniqueWriterResourceCount; missing=$missingWriterTargetCount; sequences=$writerSequenceMatch)"
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

}
catch {
    $script:bodyError = "$($_.Exception.Message) [stack=$($_.ScriptStackTrace)]"
}
finally {
    $teardownErrors = [Collections.Generic.List[string]]::new()
    $completedJournals = [Collections.Generic.List[object]]::new()
    $recordedProcesses = [Collections.Generic.List[object]]::new()

    if ($null -ne $script:tamperBackup -and $null -ne $script:tamperJournal -and (Test-Path -LiteralPath $script:tamperJournal -PathType Leaf)) {
        try { [IO.File]::WriteAllBytes($script:tamperJournal, $script:tamperBackup) }
        catch { $teardownErrors.Add("tamper journal restore: $($_.Exception.Message)") }
    }
    if ($null -ne $script:junctionPath -and (Test-Path -LiteralPath $script:junctionPath)) {
        try { [IO.Directory]::Delete($script:junctionPath, $false) }
        catch { $teardownErrors.Add("junction cleanup: $($_.Exception.Message)") }
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
    catch { $teardownErrors.Add("conflict sentinel rollback: $($_.Exception.Message)") }

    foreach ($journalPath in @($script:journalPaths | Select-Object -Unique)) {
        if (-not (Test-Path -LiteralPath $journalPath -PathType Leaf)) { continue }
        $journal = $null
        try {
            $journal = Read-WindowsOwnedResourceJournal -JournalPath $journalPath
            if ([string]$journal.state -cne 'completed') {
                $cleanupError = $null
                for ($attempt = 1; $attempt -le 2; $attempt++) {
                    try {
                        if (Test-WindowsOwnedProcessIdentity -ProcessId ([int]$journal.owner_pid) -StartUtc ([string]$journal.owner_process_start_utc)) {
                            [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $journalPath)
                        }
                        else {
                            [void](Invoke-WindowsOwnedResourceCleanup -JournalPath $journalPath -RecoveryMode)
                        }
                        $cleanupError = $null
                        break
                    }
                    catch {
                        $cleanupError = $_.Exception.Message
                        if ($attempt -lt 2) { Start-Sleep -Milliseconds 100 }
                    }
                    $journal = Read-WindowsOwnedResourceJournal -JournalPath $journalPath
                }
                if ($null -ne $cleanupError) {
                    $teardownErrors.Add("journal cleanup '$journalPath': $cleanupError")
                }
            }
            $journal = Read-WindowsOwnedResourceJournal -JournalPath $journalPath
            foreach ($resource in @($journal.resources | Where-Object { [string]$_.kind -ceq 'process' -and [int]$_.descriptor.pid -gt 0 })) {
                $recordedProcesses.Add([pscustomobject]@{
                    pid = [int]$resource.descriptor.pid
                    start_utc = [string]$resource.descriptor.process_start_utc
                    resource_id = [string]$resource.resource_id
                })
            }
            if ([string]$journal.state -ceq 'completed') {
                $completedJournals.Add([pscustomobject]@{
                    journal_path = [IO.Path]::GetFullPath($journalPath)
                    run_root = [IO.Path]::GetFullPath([string]$journal.run_root)
                })
            }
            else {
                $teardownErrors.Add("journal '$journalPath' remains '$($journal.state)'; recovery root is preserved")
            }
        }
        catch { $teardownErrors.Add("journal validation '$journalPath': $($_.Exception.Message)") }
    }

    if ($teardownErrors.Count -eq 0) {
        foreach ($entry in $completedJournals) {
            try {
                $remaining = @(Get-ChildItem -LiteralPath ([string]$entry.run_root) -Force -ErrorAction Stop)
                if ($remaining.Count -ne 0) {
                    throw "completed run root contains $($remaining.Count) residual entries"
                }
            }
            catch { $teardownErrors.Add("completed run-root audit '$($entry.run_root)': $($_.Exception.Message)") }
        }
    }

    if ($teardownErrors.Count -eq 0) {
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
        catch { $teardownErrors.Add("main Registry64 fixture cleanup: $($_.Exception.Message)") }
        foreach ($extraRegistryPath in @($script:extraRegistryPaths | Select-Object -Unique | Sort-Object { $_.Length } -Descending)) {
            try { Remove-TestRegistryKey64IfEmpty -Path $extraRegistryPath }
            catch { $teardownErrors.Add("extra Registry64 cleanup '$extraRegistryPath': $($_.Exception.Message)") }
        }
        foreach ($ownedAbsentNamespace in @(
            [pscustomobject]@{ path = $registryNamespacePath; existed = $registryNamespaceExisted },
            [pscustomobject]@{ path = $absentNamespacePath; existed = $absentNamespaceExisted },
            [pscustomobject]@{ path = $conflictNamespacePath; existed = $conflictNamespaceExisted }
        )) {
            if (-not [bool]$ownedAbsentNamespace.existed) {
                try { Remove-TestRegistryKey64IfEmpty -Path ([string]$ownedAbsentNamespace.path) }
                catch { $teardownErrors.Add("Registry64 namespace cleanup '$($ownedAbsentNamespace.path)': $($_.Exception.Message)") }
            }
        }

        if (Test-Path -LiteralPath $fileSentinel -PathType Leaf) {
            try { [IO.File]::Delete($fileSentinel) }
            catch { $teardownErrors.Add("file sentinel cleanup: $($_.Exception.Message)") }
        }

        foreach ($entry in $completedJournals) {
            try {
                [IO.File]::Delete([string]$entry.journal_path)
                Remove-ExactEmptyDirectory -Path ([string]$entry.run_root)
            }
            catch { $teardownErrors.Add("completed journal/root cleanup '$($entry.journal_path)': $($_.Exception.Message)") }
        }
    }

    if ($teardownErrors.Count -eq 0) {
        try { Remove-ExactEmptyDirectory -Path $junctionTarget }
        catch { $teardownErrors.Add("junction target cleanup: $($_.Exception.Message)") }
        try { Remove-ExactEmptyDirectory -Path $runDirectory }
        catch { $teardownErrors.Add("run infrastructure cleanup: $($_.Exception.Message)") }
        try { Remove-ExactEmptyDirectory -Path $journalDirectory }
        catch { $teardownErrors.Add("journal infrastructure cleanup: $($_.Exception.Message)") }
        try { Remove-ExactEmptyDirectory -Path $outer }
        catch { $teardownErrors.Add("outer fixture cleanup: $($_.Exception.Message)") }
    }

    if ($teardownErrors.Count -eq 0 -and (Test-Path -LiteralPath $outer)) {
        $teardownErrors.Add("owned test root '$outer' remains after exact teardown")
    }
    if ($teardownErrors.Count -eq 0) {
        foreach ($identity in $recordedProcesses) {
            if (Test-WindowsOwnedProcessIdentity -ProcessId ([int]$identity.pid) -StartUtc ([string]$identity.start_utc)) {
                $teardownErrors.Add("recorded process '$($identity.resource_id)' remains live after teardown")
            }
        }
        if ((Get-WindowsOwnedRegistryValueSnapshot -Path $registryPath -ValueName $ownedValueName).exists -or
            (Get-WindowsOwnedRegistryValueSnapshot -Path $registryPath -ValueName $neighborValueName).exists) {
            $teardownErrors.Add('main Registry64 fixture values remain after teardown')
        }
        foreach ($extraRegistryPath in @($script:extraRegistryPaths | Select-Object -Unique)) {
            if (Test-WindowsOwnedRegistryKeyExists -Path $extraRegistryPath) {
                $teardownErrors.Add("extra Registry64 fixture key '$extraRegistryPath' remains after teardown")
            }
        }
        if ($script:WindowsOwnedActiveLeases.Count -ne 0) {
            $teardownErrors.Add("$($script:WindowsOwnedActiveLeases.Count) in-process journal leases remain after teardown")
        }
    }

    if (-not [string]::IsNullOrEmpty($script:bodyError) -or $teardownErrors.Count -ne 0) {
        throw "owned-resource acceptance failed without deleting recovery prerequisites: body='$script:bodyError'; teardown='$($teardownErrors -join ' | ')'"
    }
}

"PASS: Windows owned-resource policy ($script:assertionCount assertions; $script:rejectionCount fail-closed mutations; real HKCU/file/child; logical COM/UIA only; exact teardown verified)"
