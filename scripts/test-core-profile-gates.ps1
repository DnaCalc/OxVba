param(
    [string]$RepositoryRoot = "",
    [ValidateSet("All", "Core", "Extended", "Concurrency")]
    [string]$Phase = "All"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$utf8 = [Text.UTF8Encoding]::new($false, $true)
$repoRoot = if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    [IO.Path]::GetFullPath((Resolve-Path (Join-Path $PSScriptRoot "..")).Path)
}
else {
    [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $RepositoryRoot).Path)
}
$pwsh = [Environment]::ProcessPath
$git = (Get-Command git -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
$cargo = (Get-Command cargo -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
$platform = if ($IsWindows) { "windows-x64" } elseif ($IsLinux) { "linux-x64" } else { throw "unsupported test platform" }
$oppositePlatform = if ($IsWindows) { "linux-x64" } else { "windows-x64" }
$expectedTransport = if ($IsWindows) {
    "job-object-v3:identity-bound-input-handles;startupinfoex-handle-list;suspended-assign-resume;kill-on-close;owned-file-stdout-stderr"
}
else {
    "setsid-fd-posix-spawn-pidfd-subreaper-v6:child-dup2-bound-inputs;no-ambient-parent-inheritance;pinned-glibc-x64-abi;direct-ready;builtin-ack-poll;parent-freeze;pidfd-kill;owned-file-stdout-stderr"
}
$expectedContainment = if ($IsWindows) { "windows-job-object-v2" } else { "linux-pidfd-subreaper-v1" }

function Assert-True {
    param([Parameter(Mandatory = $true)][bool]$Condition, [Parameter(Mandatory = $true)][string]$Message)
    if (-not $Condition) { throw "core profile gate test: $Message" }
}

function Assert-Equal {
    param([AllowNull()]$Actual, [AllowNull()]$Expected, [Parameter(Mandatory = $true)][string]$Message)
    if ([string]$Actual -cne [string]$Expected) {
        throw "core profile gate test: $Message; expected '$Expected', found '$Actual'"
    }
}

function Assert-Matches {
    param([AllowNull()][string]$Actual, [Parameter(Mandatory = $true)][string]$Pattern, [Parameter(Mandatory = $true)][string]$Message)
    if ([string]$Actual -notmatch $Pattern) { throw "core profile gate test: $Message; output was:`n$Actual" }
}

function Assert-NoSuccessOutput {
    param([Parameter(Mandatory = $true)]$Result, [Parameter(Mandatory = $true)][string]$Owner)
    Assert-True ($Result.exit_code -ne 0) "$Owner unexpectedly succeeded"
    Assert-True ("$($Result.stdout)`n$($Result.stderr)" -notmatch 'core-profile-gates: ok') "$Owner printed the terminal success marker"
}

function Write-Utf8Text {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    $parent = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) { [void](New-Item -ItemType Directory -Path $parent -Force) }
    [IO.File]::WriteAllText($Path, $Text, $utf8)
}

function Write-TestJson {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)]$Value)
    Write-Utf8Text -Path $Path -Text ((ConvertTo-Json -InputObject $Value -Depth 40) + "`n")
}

function Get-FileSha256 {
    param([Parameter(Mandatory = $true)][string]$Path)
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return [Convert]::ToHexString($sha.ComputeHash([IO.File]::ReadAllBytes($Path))).ToLowerInvariant() }
    finally { $sha.Dispose() }
}

function Get-TestCargoMutexName {
    param([Parameter(Mandatory = $true)][string]$Root)
    $identityRoot = [IO.Path]::GetFullPath($Root).Replace('\', '/')
    if ($IsWindows) { $identityRoot = $identityRoot.ToLowerInvariant() }
    $digest = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData(
            $utf8.GetBytes("oxvba-core-profile-cargo-v1|$identityRoot"))).ToLowerInvariant()
    return "oxvba-core-profile-cargo-v1-$($digest.Substring(0, 32))"
}

function Assert-ProcessGone {
    param([Parameter(Mandatory = $true)][int]$ProcessId, [Parameter(Mandatory = $true)][string]$Owner)
    for ($attempt = 0; $attempt -lt 80; $attempt++) {
        try { [void][Diagnostics.Process]::GetProcessById($ProcessId); Start-Sleep -Milliseconds 50 }
        catch [ArgumentException] { return }
    }
    throw "core profile gate test: $Owner process $ProcessId remained after cleanup"
}

function Wait-TestFile {
    param([Parameter(Mandatory = $true)][string]$Path, [int]$TimeoutSeconds = 15)
    $timer = [Diagnostics.Stopwatch]::StartNew()
    while ($timer.ElapsedMilliseconds -lt ($TimeoutSeconds * 1000)) {
        if (Test-Path -LiteralPath $Path -PathType Leaf) { return }
        Start-Sleep -Milliseconds 5
    }
    throw "core profile gate test: timed out waiting for controlled file '$Path'"
}

function Invoke-FixtureGit {
    param([Parameter(Mandatory = $true)][string]$Root, [Parameter(Mandatory = $true)][string[]]$Arguments)
    $output = @(& $git -C $Root @Arguments 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "fixture Git failed ($LASTEXITCODE): git $($Arguments -join ' ')`n$($output -join "`n")" }
    return $output
}

function Initialize-FixtureGit {
    param([Parameter(Mandatory = $true)][string]$Root)
    [void](Invoke-FixtureGit -Root $Root -Arguments @("init", "--quiet"))
    [void](Invoke-FixtureGit -Root $Root -Arguments @("config", "user.email", "core-gate-fixture@invalid.example"))
    [void](Invoke-FixtureGit -Root $Root -Arguments @("config", "user.name", "Core Gate Fixture"))
    [void](Invoke-FixtureGit -Root $Root -Arguments @("config", "core.autocrlf", "false"))
    if ($IsLinux) {
        & /usr/bin/chmod +x (Join-Path $Root "scripts/core-gate-linux-supervisor.sh")
        if ($LASTEXITCODE -ne 0) { throw "could not make Linux supervisor executable" }
    }
    [void](Invoke-FixtureGit -Root $Root -Arguments @("add", "--all"))
    [void](Invoke-FixtureGit -Root $Root -Arguments @("update-index", "--chmod=+x", "scripts/core-gate-linux-supervisor.sh"))
    [void](Invoke-FixtureGit -Root $Root -Arguments @("commit", "--quiet", "-m", "controlled fixture"))
    $status = (Invoke-FixtureGit -Root $Root -Arguments @("status", "--porcelain=v1", "--untracked-files=all")) -join "`n"
    Assert-Equal $status "" "new fixture is not clean"
}

function New-TestGate {
    param(
        [Parameter(Mandatory = $true)][int]$Order,
        [Parameter(Mandatory = $true)][string]$Id,
        [string]$Kind = "powershell",
        [string]$Command = "scripts/pass.ps1",
        [object[]]$Arguments = @(),
        [object[]]$Environment = @(),
        [string[]]$Platforms = @("windows-x64", "linux-x64"),
        [int]$TimeoutSeconds = 10,
        [bool]$CargoWorkspace = $false
    )
    return [pscustomobject][ordered]@{
        order = $Order; id = $Id; description = "Controlled test gate $Id"; platforms = @($Platforms)
        kind = $Kind; command = $Command; arguments = @($Arguments); environment = @($Environment)
        timeout_seconds = $TimeoutSeconds; cargo_workspace = $CargoWorkspace
        evidence_path = "commands/{0:D3}-{1}" -f $Order, $Id
    }
}

function New-TestManifest {
    param([Parameter(Mandatory = $true)][object[]]$Gates, [int]$CargoLockSeconds = 10, [string[]]$AmbientDescendantNames = @())
    return [pscustomobject][ordered]@{
        schema_id = "oxvba-core-profile-gate-plan-v1"; plan_id = "core-profile-portable-gates-v1"
        version = 1; profile = "core"; supported_platforms = @("windows-x64", "linux-x64")
        evidence = [pscustomobject][ordered]@{
            no_artifact_root = "temp/no-artifacts/core-profile-gates"; plan_path = "plan.json"
            run_manifest_path = "run-manifest.json"; run_manifest_digest_path = "run-manifest.sha256"
            summary_path = "summary.txt"
        }
        cargo_lock = [pscustomobject][ordered]@{
            name_prefix = "oxvba-core-profile-cargo-v1"; acquire_timeout_seconds = $CargoLockSeconds
        }
        supervision = [pscustomobject][ordered]@{
            cleanup_reserve_ms = 500
            ambient_descendant_names = @($AmbientDescendantNames)
            native_source_path = "scripts/core-gate-process-supervisor.cs"
            windows_transport = "job-object-v3:identity-bound-input-handles;startupinfoex-handle-list;suspended-assign-resume;kill-on-close;owned-file-stdout-stderr"
            linux_launcher_path = "/usr/bin/setsid"
            linux_bash_path = "/usr/bin/bash"
            linux_supervisor_path = "scripts/core-gate-linux-supervisor.sh"
            linux_transport = "setsid-fd-posix-spawn-pidfd-subreaper-v6:child-dup2-bound-inputs;no-ambient-parent-inheritance;pinned-glibc-x64-abi;direct-ready;builtin-ack-poll;parent-freeze;pidfd-kill;owned-file-stdout-stderr"
        }
        gates = @($Gates)
    }
}

function New-FixtureRoot {
    param([Parameter(Mandatory = $true)][string]$Name)
    $root = Join-Path $tempBase $Name
    [void](New-Item -ItemType Directory -Path (Join-Path $root "scripts") -Force)
    [void](New-Item -ItemType Directory -Path (Join-Path $root "ci/core-profile") -Force)
    foreach ($relative in @("run-core-profile-gates.ps1", "core-gate-process-supervisor.cs", "core-gate-linux-supervisor.sh")) {
        [IO.File]::Copy((Join-Path $repoRoot "scripts/$relative"), (Join-Path $root "scripts/$relative"), $false)
    }
    Write-Utf8Text -Path (Join-Path $root ".gitignore") -Text "temp/`ntest-output/`n"
    Write-Utf8Text -Path (Join-Path $root "scripts/pass.ps1") -Text @'
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
foreach ($name in @(
    "OXVBA_CORE_GATE_RUN_ID", "OXVBA_CORE_GATE_ID", "OXVBA_CORE_GATE_EVIDENCE_ROOT",
    "OXVBA_CORE_GATE_PLAN_PATH", "OXVBA_CORE_GATE_PLAN_SHA256", "OXVBA_CORE_GATE_MANIFEST_SHA256",
    "OXVBA_CORE_GATE_MANIFEST_PATH", "OXVBA_CORE_GATE_PWSH_PATH"
)) {
    if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($name))) { throw "missing propagated gate variable $name" }
}
if (-not (Test-Path -LiteralPath $env:OXVBA_CORE_GATE_PLAN_PATH -PathType Leaf)) { throw "propagated plan path is missing" }
foreach ($name in @("OXVBA_BLESS_JIT_SCOPE", "OXVBA_BLESS_GOLDEN", "OXVBA_SNAPSHOT_UPDATE", "INSTA_UPDATE")) {
    if ($null -ne [Environment]::GetEnvironmentVariable($name)) { throw "hostile inherited environment reached child: $name" }
}
Write-Output "propagated=$env:OXVBA_CORE_GATE_RUN_ID/$env:OXVBA_CORE_GATE_ID"
'@
    return $root
}

function Write-TestManifest {
    param([Parameter(Mandatory = $true)][string]$Root, [Parameter(Mandatory = $true)]$Manifest)
    Write-TestJson -Path (Join-Path $Root "ci/core-profile/gates-v1.json") -Value $Manifest
}

function New-RunnerProcess {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [hashtable]$Environment = @{}
    )
    $runner = Join-Path $Root "scripts/run-core-profile-gates.ps1"
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $pwsh; $startInfo.WorkingDirectory = $Root; $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true; $startInfo.RedirectStandardError = $true
    $startInfo.StandardOutputEncoding = $utf8; $startInfo.StandardErrorEncoding = $utf8
    foreach ($argument in @("-NoLogo", "-NoProfile", "-NonInteractive", "-File", $runner) + $Arguments) {
        [void]$startInfo.ArgumentList.Add([string]$argument)
    }
    foreach ($name in $Environment.Keys) { $startInfo.Environment[[string]$name] = [string]$Environment[$name] }
    $process = [Diagnostics.Process]::new(); $process.StartInfo = $startInfo
    if (-not $process.Start()) { throw "could not start runner test process" }
    return [pscustomobject]@{
        process = $process; stdout_task = $process.StandardOutput.ReadToEndAsync()
        stderr_task = $process.StandardError.ReadToEndAsync(); stopwatch = [Diagnostics.Stopwatch]::StartNew()
    }
}

function Complete-RunnerProcess {
    param([Parameter(Mandatory = $true)]$Handle, [int]$TimeoutSeconds = 120)
    if (-not $Handle.process.WaitForExit($TimeoutSeconds * 1000)) {
        try { $Handle.process.Kill($true) } catch {}; try { $Handle.process.WaitForExit() } catch {}
        throw "core profile gate test: runner process exceeded the test bound of $TimeoutSeconds seconds"
    }
    $Handle.process.WaitForExit(); $Handle.stopwatch.Stop()
    $result = [pscustomobject]@{
        exit_code = [int]$Handle.process.ExitCode; stdout = [string]$Handle.stdout_task.GetAwaiter().GetResult()
        stderr = [string]$Handle.stderr_task.GetAwaiter().GetResult(); duration_ms = [int64]$Handle.stopwatch.ElapsedMilliseconds
    }
    $Handle.process.Dispose(); return $result
}

function Invoke-Runner {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [hashtable]$Environment = @{},
        [int]$TimeoutSeconds = 120
    )
    return Complete-RunnerProcess -Handle (New-RunnerProcess -Root $Root -Arguments $Arguments -Environment $Environment) -TimeoutSeconds $TimeoutSeconds
}

function Get-RunRoot {
    param([Parameter(Mandatory = $true)][string]$Root, [Parameter(Mandatory = $true)][string]$RunId)
    return Join-Path $Root "temp/no-artifacts/core-profile-gates/$RunId"
}

function Get-RunManifest {
    param([Parameter(Mandatory = $true)][string]$Root, [Parameter(Mandatory = $true)][string]$RunId)
    return Get-Content -LiteralPath (Join-Path (Get-RunRoot -Root $Root -RunId $RunId) "run-manifest.json") -Raw | ConvertFrom-Json -Depth 40
}

function Assert-ManifestFailure {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Pattern,
        [Parameter(Mandatory = $true)][scriptblock]$Mutate
    )
    $root = New-FixtureRoot -Name "invalid-$Name"
    $manifest = New-TestManifest -Gates @((New-TestGate -Order 1 -Id "pass"))
    & $Mutate $manifest $root
    if (-not (Test-Path -LiteralPath (Join-Path $root "ci/core-profile/gates-v1.json") -PathType Leaf)) { Write-TestManifest -Root $root -Manifest $manifest }
    $result = Invoke-Runner -Root $root -Arguments @("-RepositoryRoot", $root, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "ValidateManifest")
    Assert-NoSuccessOutput -Result $result -Owner "mutated manifest '$Name'"
    Assert-Matches "$($result.stdout)`n$($result.stderr)" $Pattern "mutated manifest '$Name' failed for the wrong reason"
    Write-Host "core-profile-gates mutation: ok ($Name)"
}

function Invoke-TamperCase {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Script,
        [bool]$AfterPassedGate = $false,
        [string]$FailurePattern = 'evidence|source|identity'
    )
    $root = New-FixtureRoot -Name "tamper-$Name"
    Write-Utf8Text -Path (Join-Path $root "scripts/tamper.ps1") -Text $Script
    $gates = if ($AfterPassedGate) {
        @((New-TestGate -Order 1 -Id "prior-pass"), (New-TestGate -Order 2 -Id "tamper" -Command "scripts/tamper.ps1"))
    }
    else { @((New-TestGate -Order 1 -Id "tamper" -Command "scripts/tamper.ps1")) }
    Write-TestManifest -Root $root -Manifest (New-TestManifest -Gates $gates)
    Initialize-FixtureGit -Root $root
    $result = Invoke-Runner -Root $root -Arguments @("-RepositoryRoot", $root, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", $Name)
    Assert-NoSuccessOutput -Result $result -Owner "tamper case '$Name'"
    $run = Get-RunManifest -Root $root -RunId $Name
    Assert-Equal $run.status "failed" "tamper case '$Name' did not finalize as failed"
    Assert-Matches $run.failure $FailurePattern "tamper case '$Name' failure was not explicit"
    Write-Host "core-profile-gates immutable evidence tamper: ok ($Name)"
}

$systemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
$tempBase = Join-Path $systemTemp ("oxvba-core-profile-gates-" + [guid]::NewGuid().ToString("N"))
[void](New-Item -ItemType Directory -Path $tempBase)

try {
    if ($Phase -in @("All", "Core")) {
    $listArguments = @("-RepositoryRoot", $repoRoot, "-List")
    $listOne = Invoke-Runner -Root $repoRoot -Arguments $listArguments
    $listTwo = Invoke-Runner -Root $repoRoot -Arguments $listArguments
    $dryOne = Invoke-Runner -Root $repoRoot -Arguments @("-RepositoryRoot", $repoRoot, "-DryRun")
    $dryTwo = Invoke-Runner -Root $repoRoot -Arguments @("-RepositoryRoot", $repoRoot, "-DryRun")
    Assert-Equal $listOne.exit_code 0 "canonical list failed"
    Assert-Equal $listOne.stdout $listTwo.stdout "canonical list output is not deterministic"
    Assert-Equal $dryOne.stdout $dryTwo.stdout "canonical dry-run output is not deterministic"
    Assert-Equal $listOne.stdout $dryOne.stdout "list and dry-run do not project one canonical plan"
    Assert-Matches $listOne.stdout '(?m)^001\|(run|not-applicable)\|linux-runtime-environment\|' "Linux platform lane is not explicit"
    Assert-Matches $listOne.stdout '(?m)^002\|(run|not-applicable)\|windows-environment-ledger\|' "Windows platform lane is not explicit"
    Assert-True ($listOne.stdout -notmatch '(?i)(bless|snapshot[-_ ]*(update|accept)|(?:update|accept)[-_ ]*snapshot)') "canonical plan exposes a snapshot mutation path"
    $manifestOnly = Invoke-Runner -Root $repoRoot -Arguments @("-RepositoryRoot", $repoRoot, "-Mode", "ValidateManifest")
    Assert-Equal $manifestOnly.exit_code 0 "canonical manifest validation failed"
    Assert-Matches $manifestOnly.stdout 'manifest ok' "manifest validation did not report its distinct mode"
    $wrongArchitecture = Invoke-Runner -Root $repoRoot -Arguments @("-RepositoryRoot", $repoRoot, "-Mode", "ValidateManifest") -Environment @{ OXVBA_CORE_GATE_TEST_FORCE_PROCESS_ARCH = "x86" }
    Assert-NoSuccessOutput -Result $wrongArchitecture -Owner "injected x86 process"
    Assert-Matches "$($wrongArchitecture.stdout)`n$($wrongArchitecture.stderr)" 'OSArchitecture=x64, ProcessArchitecture=x64 and Is64BitProcess=true' "x86 injection failed for the wrong reason"
    $runnerSource = [IO.File]::ReadAllText((Join-Path $repoRoot "scripts/run-core-profile-gates.ps1"), $utf8)
    Assert-True ($runnerSource -notmatch 'SignalGroup|GroupExists|\.Kill\(') "gate runner retains a numeric PID/PGID or Process.Kill authority path"
    $boundedCaptureMatch = [regex]::Match($runnerSource, '(?s)function Invoke-BoundedCapture \{.*?\n\}\n\nfunction Resolve-ExactApplicationIdentity \{')
    Assert-True $boundedCaptureMatch.Success "owned tool-probe implementation could not be isolated for authority audit"
    Assert-Matches $boundedCaptureMatch.Value 'Invoke-WindowsOwnedProcess' "tool probes do not use Windows Job containment"
    Assert-Matches $boundedCaptureMatch.Value 'Invoke-LinuxOwnedProcess' "tool probes do not use Linux pidfd containment"
    foreach ($sealedUtility in @('setsid', 'bash')) {
        Assert-Matches $boundedCaptureMatch.Value "Get-ToolIdentityById -Tools \`$ProbeContext\.tools -Id `"$sealedUtility`"" "tool probes do not revalidate exact Linux $sealedUtility identity"
    }
    $candidateIndex = $runnerSource.IndexOf('$toolCandidates = Get-ToolCandidates', [StringComparison]::Ordinal)
    $addTypeIndex = $runnerSource.IndexOf('Add-Type -TypeDefinition', [StringComparison]::Ordinal)
    $versionIndex = $runnerSource.IndexOf('$tools = Get-ToolIdentities', [StringComparison]::Ordinal)
    Assert-True ($candidateIndex -ge 0 -and $candidateIndex -lt $addTypeIndex -and $addTypeIndex -lt $versionIndex) "tool path/hash resolution, supervisor load, and contained probing are not ordered fail-closed"
    $nativeSource = [IO.File]::ReadAllText((Join-Path $repoRoot "scripts/core-gate-process-supervisor.cs"), $utf8)
    Assert-Matches $nativeSource 'pidfd_open' "Linux containment does not retain pidfd authority"
    Assert-Matches $nativeSource 'pidfd_send_signal' "Linux containment does not signal through pidfd authority"
    Assert-True ($nativeSource -notmatch 'extern int kill\(|SignalGroup|GroupExists') "native Linux containment retains numeric PID/PGID signaling"
    Assert-Matches $nativeSource 'ArmRootWithForcedConfirmationFailureForTest' "Linux containment lacks a post-pidfd confirmation-failure proof hook"
    Assert-Matches $nativeSource 'windows-retained-file-and-ancestor-handles|BindAncestorDirectories' "Windows launch does not retain admitted file and ancestor handles"
    Assert-Matches $nativeSource 'posix_spawn\(' "Linux launch does not enter the fd-bound executable through posix_spawn"
    Assert-Matches $nativeSource 'posix_spawn_file_actions_adddup2' "Linux launch does not create child-only descriptor authority through posix_spawn file actions"
    Assert-Matches $nativeSource 'RequireCloseOnExec' "Linux launch does not enforce CLOEXEC on admitted parent descriptors"
    Assert-Matches $nativeSource 'OXVBA_CORE_GATE_TEST_PROBE_UNRELATED_INHERITANCE' "Linux launch lacks the unrelated concurrent-child inheritance sentinel"
    Assert-Matches $nativeSource 'gnu_get_libc_version' "Linux file-actions ABI is not guarded by a fail-closed glibc identity check"
    Assert-True ($nativeSource -notmatch 'SetCloseOnExec|F_SETFD') "Linux launch still has a global parent descriptor-inheritance window"
    Assert-Matches $nativeSource 'memfd_create' "Linux launch does not snapshot admitted bytes into sealed descriptors"
    Assert-Matches $nativeSource 'FSealWrite' "Linux launch does not make admitted byte snapshots immutable"
    Assert-Matches $nativeSource 'openat2' "Linux launch does not bind repository inputs below retained directory descriptors"
    $armRootStart = $nativeSource.IndexOf('private ulong ArmRootCore', [StringComparison]::Ordinal)
    $armRootEnd = $nativeSource.IndexOf('public bool TerminateAll', $armRootStart, [StringComparison]::Ordinal)
    Assert-True ($armRootStart -ge 0 -and $armRootEnd -gt $armRootStart) "Linux root-confirmation implementation could not be isolated"
    $armRootSource = $nativeSource.Substring($armRootStart, $armRootEnd - $armRootStart)
    $rootProcRead = $armRootSource.IndexOf('ProcRecord root = ReadProcRecord', [StringComparison]::Ordinal)
    $rootPidFdLiveness = $armRootSource.IndexOf('SignalPidFd(retained.PidFd, OxVbaCoreGatePosix.SignalZero)', [StringComparison]::Ordinal)
    $rootConfirmation = $armRootSource.IndexOf('_rootConfirmed = true', [StringComparison]::Ordinal)
    Assert-True ($rootProcRead -ge 0 -and $rootPidFdLiveness -gt $rootProcRead -and $rootConfirmation -gt $rootPidFdLiveness) "Linux root confirmation does not revalidate the retained pidfd after /proc inspection"
    $linuxSupervisorSource = [IO.File]::ReadAllText((Join-Path $repoRoot "scripts/core-gate-linux-supervisor.sh"), $utf8)
    Assert-True ($linuxSupervisorSource -notmatch '/usr/bin/(?:mv|sleep)') "Linux supervisor can create an external pre-ack helper child"
    Assert-Matches $linuxSupervisorSource 'EPOCHREALTIME' "Linux supervisor does not use the child-free bounded acknowledgement poll"
    if ($IsLinux) {
        Add-Type -Path (Join-Path $repoRoot "scripts/core-gate-process-supervisor.cs")
        $sentinelStart = [Diagnostics.ProcessStartInfo]::new("/usr/bin/sleep")
        $sentinelStart.UseShellExecute = $false
        [void]$sentinelStart.ArgumentList.Add("30")
        $confirmationSentinel = [Diagnostics.Process]::Start($sentinelStart)
        $pendingTree = [OxVbaCoreGatePosixOwnedTree]::new()
        $pendingRoot = $null
        $markerPath = Join-Path $tempBase "forced-confirmation-gate-ran"
        try {
            $pendingStart = [Diagnostics.ProcessStartInfo]::new("/usr/bin/setsid")
            $pendingStart.UseShellExecute = $false
            foreach ($argument in @("/usr/bin/bash", (Join-Path $repoRoot "scripts/core-gate-linux-supervisor.sh"),
                    (Join-Path $tempBase "forced-confirmation.ready"), (Join-Path $tempBase "forced-confirmation.ack"),
                    "forced-confirmation", (Join-Path $tempBase "forced-confirmation.stdout"),
                    (Join-Path $tempBase "forced-confirmation.stderr"), "/usr/bin/touch", $markerPath)) {
                [void]$pendingStart.ArgumentList.Add($argument)
            }
            $pendingRoot = [Diagnostics.Process]::Start($pendingStart)
            $confirmationFailure = $null
            try { [void]$pendingTree.ArmRootWithForcedConfirmationFailureForTest($pendingRoot.Id) }
            catch { $confirmationFailure = $_.Exception }
            Assert-True ($null -ne $confirmationFailure) "forced root confirmation failure did not fail"
            Assert-Matches $confirmationFailure.Message 'after pidfd retention' "forced root confirmation failure occurred before exact retention"
            Assert-Equal $pendingTree.RetainedPidFdCount 1 "unconfirmed root pidfd was not retained for abort"
            Assert-True $pendingTree.TerminateAll(5000) "unconfirmed exact root could not be aborted"
            Assert-True $pendingRoot.WaitForExit(5000) "unconfirmed exact root was not reaped"
            [void]$pendingTree.LiveProcessCount
            Assert-Equal $pendingTree.RetainedPidFdCount 0 "unconfirmed root pidfd remained after reap"
            Assert-True (-not (Test-Path -LiteralPath $markerPath)) "gate executable ran before root confirmation acknowledgement"
            Assert-True (-not $confirmationSentinel.HasExited) "unconfirmed-root abort terminated an unrelated sentinel"
        }
        finally {
            if ($null -ne $pendingRoot) {
                if (-not $pendingRoot.HasExited) { [void]$pendingTree.TerminateAll(5000); [void]$pendingRoot.WaitForExit(5000) }
                $pendingRoot.Dispose()
            }
            $pendingTree.Dispose()
            if (-not $confirmationSentinel.HasExited) { $confirmationSentinel.Kill($true); [void]$confirmationSentinel.WaitForExit(5000) }
            $confirmationSentinel.Dispose()
        }
        Write-Host "core-profile-gates unconfirmed-root pidfd abort: ok"

        $postSpawnSentinel = [Diagnostics.Process]::Start($sentinelStart)
        $postSpawnScript = Join-Path $tempBase "forced-post-spawn-gate.sh"
        $postSpawnMarker = Join-Path $tempBase "forced-post-spawn-gate-ran"
        $postSpawnPidPath = Join-Path $tempBase "forced-post-spawn.pid"
        $postSpawnReady = Join-Path $tempBase "forced-post-spawn.ready"
        $postSpawnAck = Join-Path $tempBase "forced-post-spawn.ack"
        $postSpawnStdout = Join-Path $tempBase "forced-post-spawn.stdout"
        $postSpawnStderr = Join-Path $tempBase "forced-post-spawn.stderr"
        Write-Utf8Text -Path $postSpawnScript -Text "printf ran > '$postSpawnMarker'`n"
        [IO.File]::WriteAllBytes($postSpawnStdout, [byte[]]::new(0))
        [IO.File]::WriteAllBytes($postSpawnStderr, [byte[]]::new(0))
        $postSpawnSupervisor = Join-Path $repoRoot "scripts/core-gate-linux-supervisor.sh"
        [string[]]$postSpawnInputs = @("/usr/bin/setsid", "/usr/bin/bash", $postSpawnSupervisor, $postSpawnScript)
        [string[]]$postSpawnHashes = @($postSpawnInputs | ForEach-Object { Get-FileSha256 -Path $_ })
        $postSpawnEnvironment = [Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
        foreach ($entry in [Environment]::GetEnvironmentVariables().GetEnumerator()) {
            $postSpawnEnvironment[[string]$entry.Key] = [string]$entry.Value
        }
        # Warm lazy exception/stack-trace assemblies before taking the exact
        # descriptor baseline used by the forced failure path.
        try { throw [InvalidOperationException]::new("descriptor-baseline-warmup") }
        catch { [void]$_.Exception.ToString() }
        $postSpawnBaselineFds = [OxVbaCoreGatePosixChild]::CountOpenDescriptorsForTest()
        try {
            [Environment]::SetEnvironmentVariable("OXVBA_CORE_GATE_TEST_FORCE_POST_SPAWN_FAILURE", "1")
            [Environment]::SetEnvironmentVariable("OXVBA_CORE_GATE_TEST_FORCED_POST_SPAWN_PID_PATH", $postSpawnPidPath)
            [Environment]::SetEnvironmentVariable("OXVBA_CORE_GATE_TEST_PROBE_UNRELATED_INHERITANCE", "1")
            $postSpawnFailure = $null
            try {
                [void][OxVbaCoreGatePosixChild]::Start(
                    "/usr/bin/setsid", "/usr/bin/bash", $postSpawnSupervisor, "/usr/bin/bash",
                    [string[]]@($postSpawnScript), $repoRoot, $postSpawnEnvironment,
                    $postSpawnReady, $postSpawnAck, "forced-post-spawn", $postSpawnStdout, $postSpawnStderr,
                    $postSpawnInputs, $postSpawnHashes)
            }
            catch { $postSpawnFailure = $_.Exception }
            Assert-True ($null -ne $postSpawnFailure) "forced post-spawn launch failure did not fail"
            Assert-Matches "$postSpawnFailure" 'after pidfd retention' "forced post-spawn failure occurred before exact pidfd ownership"
            Assert-True (Test-Path -LiteralPath $postSpawnPidPath -PathType Leaf) "forced post-spawn child did not publish its exact pid"
            $postSpawnPid = [int][IO.File]::ReadAllText($postSpawnPidPath, $utf8)
            Assert-ProcessGone -ProcessId $postSpawnPid -Owner "forced post-spawn exact child"
            Assert-True (-not (Test-Path -LiteralPath $postSpawnMarker)) "gate ran after forced post-spawn launch failure"
            Assert-True (-not (Test-Path -LiteralPath $postSpawnReady)) "ready descriptor path remained after forced post-spawn cleanup"
            Assert-True (-not (Test-Path -LiteralPath $postSpawnAck)) "ack descriptor path remained after forced post-spawn cleanup"
            Assert-Equal ([OxVbaCoreGatePosixChild]::CountOpenDescriptorsForTest()) $postSpawnBaselineFds "forced post-spawn cleanup leaked parent descriptors"
            Assert-True (-not $postSpawnSentinel.HasExited) "forced post-spawn cleanup terminated an unrelated sentinel"
        }
        finally {
            [Environment]::SetEnvironmentVariable("OXVBA_CORE_GATE_TEST_FORCE_POST_SPAWN_FAILURE", $null)
            [Environment]::SetEnvironmentVariable("OXVBA_CORE_GATE_TEST_FORCED_POST_SPAWN_PID_PATH", $null)
            [Environment]::SetEnvironmentVariable("OXVBA_CORE_GATE_TEST_PROBE_UNRELATED_INHERITANCE", $null)
            if (-not $postSpawnSentinel.HasExited) { $postSpawnSentinel.Kill($true); [void]$postSpawnSentinel.WaitForExit(5000) }
            $postSpawnSentinel.Dispose()
        }
        Write-Host "core-profile-gates child-only fd actions, concurrent inheritance sentinel, and post-spawn abort: ok"
    }
    Write-Host "core-profile-gates deterministic plan and x64 architecture gate: ok"

    $positiveRoot = New-FixtureRoot -Name "positive"
    $positiveManifest = New-TestManifest -Gates @(
        (New-TestGate -Order 1 -Id "propagation"),
        (New-TestGate -Order 2 -Id "serialized-pass" -CargoWorkspace $true),
        (New-TestGate -Order 3 -Id "nonselected" -Platforms @($oppositePlatform))
    )
    Write-TestManifest -Root $positiveRoot -Manifest $positiveManifest
    Initialize-FixtureGit -Root $positiveRoot
    $positive = Invoke-Runner -Root $positiveRoot -Arguments @("-RepositoryRoot", $positiveRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "positive") -Environment @{
        OXVBA_BLESS_JIT_SCOPE = "1"; OXVBA_BLESS_GOLDEN = "1"; OXVBA_SNAPSHOT_UPDATE = "1"; INSTA_UPDATE = "always"
    }
    Assert-Equal $positive.exit_code 0 "positive no-artifact run failed; stdout=$($positive.stdout); stderr=$($positive.stderr)"
    Assert-Matches $positive.stdout 'core-profile-gates: ok' "positive run did not print the terminal success marker"
    $positiveRunRoot = Get-RunRoot -Root $positiveRoot -RunId "positive"
    $positiveRun = Get-RunManifest -Root $positiveRoot -RunId "positive"
    $positivePlan = Get-Content -LiteralPath (Join-Path $positiveRunRoot "plan.json") -Raw | ConvertFrom-Json -Depth 40
    Assert-Equal $positiveRun.status "passed" "positive run manifest did not pass"
    Assert-Equal $positiveRun.failure "" "positive run retained a failure"
    Assert-Equal @($positiveRun.results).Count 3 "positive run result count drifted"
    Assert-Equal $positiveRun.results[0].status "passed" "first selected result did not pass"
    Assert-Equal $positiveRun.results[1].status "passed" "second selected result did not pass"
    Assert-Equal $positiveRun.results[2].status "not-applicable" "nonselected result was not N/A"
    Assert-Equal $positiveRun.results[2].reason "platform:$platform" "nonselected result reason drifted"
    foreach ($architecture in @($positivePlan.architecture, $positiveRun.architecture)) {
        Assert-Equal $architecture.os_architecture "x64" "OS architecture was not recorded as x64"
        Assert-Equal $architecture.process_architecture "x64" "process architecture was not recorded as x64"
        Assert-Equal $architecture.is_64_bit_process $true "64-bit process state was not recorded"
    }
    if ($IsLinux) {
        Assert-Matches $positivePlan.supervision.linux_libc_identity '^glibc-[0-9]+\.[0-9]+-x64$' "pinned glibc x64 identity was not recorded in the plan"
        Assert-Equal $positiveRun.results[0].libc_identity $positivePlan.supervision.linux_libc_identity "gate glibc identity drifted from the admitted plan"
    }
    else { Assert-Equal $positiveRun.results[0].libc_identity "not-applicable" "Windows gate recorded a libc identity" }
    Assert-Equal $positiveRun.source.status "clean" "source identity is not clean"
    Assert-Matches $positiveRun.source.head '^[0-9a-f]{40,64}$' "source HEAD is not recorded"
    Assert-Matches $positiveRun.source.tree '^[0-9a-f]{40,64}$' "source tree is not recorded"
    Assert-True (@($positiveRun.tools).Count -ge 3) "tool identities are incomplete"
    if ($IsLinux) {
        $bashTools = @($positiveRun.tools | Where-Object { $_.id -ceq "bash" })
        Assert-Equal $bashTools.Count 1 "exact Bash identity is not recorded"
        Assert-Equal $bashTools[0].path "/usr/bin/bash" "recorded Bash path drifted"
        Assert-Matches $bashTools[0].sha256 '^[0-9a-f]{64}$' "recorded Bash digest is absent"
    }
    Assert-True (@($positiveRun.commands).Count -ge 5) "command/source identities are incomplete"
    Assert-Equal $positiveRun.supervision.transport $expectedTransport "run supervision transport drifted"
    foreach ($command in @($positivePlan.commands)) { Assert-Matches $command.command_digest '^[0-9a-f]{64}$' "command digest is absent" }
    foreach ($resultRow in @($positiveRun.results | Where-Object { $_.status -eq "passed" })) {
        Assert-Equal $resultRow.exit_code 0 "selected result exit code drifted"
        Assert-Equal $resultRow.tree_cleanup "complete" "selected result tree was not empty"
        Assert-Equal $resultRow.transport $expectedTransport "selected result transport drifted"
        Assert-Equal $resultRow.containment $expectedContainment "selected result containment drifted"
        Assert-Equal $resultRow.supervisor_ready $true "selected result lacks ownership readiness"
        Assert-True ($null -ne $resultRow.ownership_root_pid) "selected result lacks an ownership root pid"
        if ($IsLinux) { Assert-True ($null -ne $resultRow.ownership_root_start_ticks) "Linux result lacks a /proc start-time identity" }
        foreach ($pair in @(@($resultRow.stdout_path, $resultRow.stdout_sha256), @($resultRow.stderr_path, $resultRow.stderr_sha256), @($resultRow.result_path, $resultRow.result_sha256))) {
            Assert-Equal (Get-FileSha256 -Path (Join-Path $positiveRunRoot ([string]$pair[0]))) ([string]$pair[1]) "result content hash drifted"
        }
    }
    $summaryPath = Join-Path $positiveRunRoot "summary.txt"
    Assert-Equal (Get-FileSha256 -Path $summaryPath) $positiveRun.summary_sha256 "summary hash drifted"
    $runManifestPath = Join-Path $positiveRunRoot "run-manifest.json"
    $expectedDigest = "$(Get-FileSha256 -Path $runManifestPath)  run-manifest.json`n"
    Assert-Equal ([IO.File]::ReadAllText((Join-Path $positiveRunRoot "run-manifest.sha256"), $utf8)) $expectedDigest "run manifest digest file drifted"
    $propagationOutput = Get-Content -LiteralPath (Join-Path $positiveRunRoot "commands/001-propagation/stdout.log") -Raw
    Assert-Matches $propagationOutput 'propagated=positive/propagation' "evidence environment was not propagated"
    $stale = Invoke-Runner -Root $positiveRoot -Arguments @("-RepositoryRoot", $positiveRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "positive")
    Assert-NoSuccessOutput -Result $stale -Owner "stale evidence reuse"
    Assert-Matches "$($stale.stdout)`n$($stale.stderr)" 'refusing stale evidence' "stale evidence failed for the wrong reason"
    Write-Host "core-profile-gates exact success evidence: ok"

    $fastExitRoot = New-FixtureRoot -Name "fast-exit"
    Write-Utf8Text -Path (Join-Path $fastExitRoot "scripts/fast-exit.ps1") -Text "exit 0`n"
    Write-TestManifest -Root $fastExitRoot -Manifest (New-TestManifest -Gates @(
            (New-TestGate -Order 1 -Id "fast-exit" -Command "scripts/fast-exit.ps1" -TimeoutSeconds 3)))
    Initialize-FixtureGit -Root $fastExitRoot
    $fastExit = Invoke-Runner -Root $fastExitRoot -Arguments @("-RepositoryRoot", $fastExitRoot,
        "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "fast-exit")
    Assert-Equal $fastExit.exit_code 0 "fast-exit ownership handshake failed: $($fastExit.stderr)"
    $fastExitRun = Get-RunManifest -Root $fastExitRoot -RunId "fast-exit"
    Assert-Equal $fastExitRun.results[0].status "passed" "fast-exit gate did not pass"
    Assert-Equal $fastExitRun.results[0].supervisor_ready $true "fast-exit gate ran before ownership readiness"
    Assert-Equal $fastExitRun.results[0].tree_cleanup "complete" "fast-exit gate left process residue"
    Assert-Equal $fastExitRun.results[0].containment $expectedContainment "fast-exit containment drifted"
    Write-Host "core-profile-gates fast-exit ownership handshake: ok"

    $failureRoot = New-FixtureRoot -Name "failure"
    Write-Utf8Text -Path (Join-Path $failureRoot "scripts/fail.ps1") -Text "Write-Error 'controlled failure'`nexit 7`n"
    Write-TestManifest -Root $failureRoot -Manifest (New-TestManifest -Gates @(
        (New-TestGate -Order 1 -Id "controlled-failure" -Command "scripts/fail.ps1"),
        (New-TestGate -Order 2 -Id "must-not-run")
    ))
    Initialize-FixtureGit -Root $failureRoot
    $failure = Invoke-Runner -Root $failureRoot -Arguments @("-RepositoryRoot", $failureRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "failure")
    Assert-NoSuccessOutput -Result $failure -Owner "nonzero command"
    $failureRun = Get-RunManifest -Root $failureRoot -RunId "failure"
    Assert-Equal $failureRun.status "failed" "failure run manifest did not fail"
    Assert-Equal $failureRun.results[0].exit_code 7 "command exit code was not preserved"
    Assert-Equal $failureRun.results[1].status "not-run" "later command ran after failure"
    Write-Host "core-profile-gates command failure: ok"

    $timeoutRoot = New-FixtureRoot -Name "timeout"
    Write-Utf8Text -Path (Join-Path $timeoutRoot "scripts/sleep.ps1") -Text "Start-Sleep -Seconds 30`nWrite-Output 'late'`n"
    Write-TestManifest -Root $timeoutRoot -Manifest (New-TestManifest -Gates @((New-TestGate -Order 1 -Id "controlled-timeout" -Command "scripts/sleep.ps1" -TimeoutSeconds 1)))
    Initialize-FixtureGit -Root $timeoutRoot
    $timeout = Invoke-Runner -Root $timeoutRoot -Arguments @("-RepositoryRoot", $timeoutRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "timeout") -TimeoutSeconds 45
    Assert-NoSuccessOutput -Result $timeout -Owner "timed-out command"
    $timeoutRun = Get-RunManifest -Root $timeoutRoot -RunId "timeout"
    Assert-Equal $timeoutRun.results[0].status "timeout" "timeout status was not preserved"
    Assert-True ([int64]$timeoutRun.results[0].duration_ms -le 1500) "command timeout did not bound the owned process tree"
    Assert-Equal $timeoutRun.results[0].tree_cleanup "complete" "timeout did not empty the process tree"
    Assert-Equal $timeoutRun.results[0].transport $expectedTransport "timeout transport was not recorded"
    Assert-Equal $timeoutRun.results[0].total_deadline_ms 1000 "timeout total deadline drifted"
    Write-Host "core-profile-gates total deadline: ok"

    $descendantRoot = New-FixtureRoot -Name "descendant"
    $pidPath = Join-Path $descendantRoot "test-output/grandchild.pid"
    Write-Utf8Text -Path (Join-Path $descendantRoot "scripts/grandchild.ps1") -Text @'
[void](New-Item -ItemType Directory -Path (Split-Path -Parent $env:OXVBA_CORE_GATE_TEST_PID_PATH) -Force)
[IO.File]::WriteAllText($env:OXVBA_CORE_GATE_TEST_PID_PATH, [string]$PID, [Text.UTF8Encoding]::new($false))
Start-Sleep -Seconds 30
'@
    Write-Utf8Text -Path (Join-Path $descendantRoot "scripts/spawn-grandchild.ps1") -Text @'
$startInfo = [Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName = $env:OXVBA_CORE_GATE_PWSH_PATH
$startInfo.UseShellExecute = $false
foreach ($argument in @("-NoLogo", "-NoProfile", "-NonInteractive", "-File", (Join-Path $PSScriptRoot "grandchild.ps1"))) { [void]$startInfo.ArgumentList.Add($argument) }
$child = [Diagnostics.Process]::Start($startInfo)
$deadline = [DateTime]::UtcNow.AddSeconds(2)
while (-not (Test-Path -LiteralPath $env:OXVBA_CORE_GATE_TEST_PID_PATH) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 10 }
if (-not (Test-Path -LiteralPath $env:OXVBA_CORE_GATE_TEST_PID_PATH)) { throw "grandchild did not publish its pid" }
Start-Sleep -Milliseconds 1200
Write-Output "direct-parent-exiting child=$($child.Id)"
'@
    $pidEnvironment = @([pscustomobject]@{ name = "OXVBA_CORE_GATE_TEST_PID_PATH"; action = "set"; value = $pidPath })
    Write-TestManifest -Root $descendantRoot -Manifest (New-TestManifest -Gates @((New-TestGate -Order 1 -Id "descendant" -Command "scripts/spawn-grandchild.ps1" -Environment $pidEnvironment -TimeoutSeconds 10)))
    Initialize-FixtureGit -Root $descendantRoot
    $descendant = Invoke-Runner -Root $descendantRoot -Arguments @("-RepositoryRoot", $descendantRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "descendant") -TimeoutSeconds 40
    Assert-NoSuccessOutput -Result $descendant -Owner "direct exit with descendant"
    $descendantRun = Get-RunManifest -Root $descendantRoot -RunId "descendant"
    Assert-Equal $descendantRun.results[0].status "failed" "descendant result did not fail"
    Assert-Equal $descendantRun.results[0].reason "descendant-processes-remained-after-direct-exit" "descendant failure reason drifted"
    Assert-Equal $descendantRun.results[0].tree_cleanup "complete" "descendant tree did not empty"
    Assert-Equal $descendantRun.results[0].transport $expectedTransport "descendant transport was not recorded"
    Assert-True ([int64]$descendantRun.results[0].duration_ms -le 9000) "descendant cleanup exceeded its owned-tree deadline (parent sleep plus bounded drain window plus admission/cleanup overhead)"
    Assert-True (Test-Path -LiteralPath $pidPath -PathType Leaf) "grandchild pid was not recorded"
    $grandchildPid = [int][IO.File]::ReadAllText($pidPath, $utf8)
    $grandchildGone = $false
    for ($attempt = 0; $attempt -lt 40; $attempt++) {
        try { [void][Diagnostics.Process]::GetProcessById($grandchildPid); Start-Sleep -Milliseconds 50 }
        catch [ArgumentException] { $grandchildGone = $true; break }
    }
    Assert-True $grandchildGone "owned grandchild process remained after runner completion"
    Write-Host "core-profile-gates complete process-tree ownership: ok"

    $drainRoot = New-FixtureRoot -Name "descendant-drain"
    $drainPidPath = Join-Path $drainRoot "test-output/drain-grandchild.pid"
    Write-Utf8Text -Path (Join-Path $drainRoot "scripts/drain-grandchild.ps1") -Text @'
[void](New-Item -ItemType Directory -Path (Split-Path -Parent $env:OXVBA_CORE_GATE_TEST_PID_PATH) -Force)
[IO.File]::WriteAllText($env:OXVBA_CORE_GATE_TEST_PID_PATH, [string]$PID, [Text.UTF8Encoding]::new($false))
Start-Sleep -Milliseconds 500
'@
    Write-Utf8Text -Path (Join-Path $drainRoot "scripts/spawn-drain-grandchild.ps1") -Text @'
$startInfo = [Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName = $env:OXVBA_CORE_GATE_PWSH_PATH
$startInfo.UseShellExecute = $false
foreach ($argument in @("-NoLogo", "-NoProfile", "-NonInteractive", "-File", (Join-Path $PSScriptRoot "drain-grandchild.ps1"))) { [void]$startInfo.ArgumentList.Add($argument) }
$child = [Diagnostics.Process]::Start($startInfo)
$deadline = [DateTime]::UtcNow.AddSeconds(2)
while (-not (Test-Path -LiteralPath $env:OXVBA_CORE_GATE_TEST_PID_PATH) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 10 }
if (-not (Test-Path -LiteralPath $env:OXVBA_CORE_GATE_TEST_PID_PATH)) { throw "drain grandchild did not publish its pid" }
Write-Output "direct-parent-exiting child=$($child.Id)"
'@
    $drainPidEnvironment = @([pscustomobject]@{ name = "OXVBA_CORE_GATE_TEST_PID_PATH"; action = "set"; value = $drainPidPath })
    Write-TestManifest -Root $drainRoot -Manifest (New-TestManifest -Gates @((New-TestGate -Order 1 -Id "descendant-drain" -Command "scripts/spawn-drain-grandchild.ps1" -Environment $drainPidEnvironment -TimeoutSeconds 10)))
    Initialize-FixtureGit -Root $drainRoot
    $drain = Invoke-Runner -Root $drainRoot -Arguments @("-RepositoryRoot", $drainRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "descendant-drain") -TimeoutSeconds 90
    Assert-Equal $drain.exit_code 0 "short-lived descendant drain was not accepted: $($drain.stderr)"
    $drainRun = Get-RunManifest -Root $drainRoot -RunId "descendant-drain"
    Assert-Equal $drainRun.results[0].status "passed" "short-lived descendant did not pass inside the bounded drain window"
    Assert-Equal $drainRun.results[0].exit_code 0 "short-lived descendant exit code drifted"
    Assert-Equal $drainRun.results[0].tree_cleanup "complete" "short-lived descendant tree did not empty"
    Assert-True (Test-Path -LiteralPath $drainPidPath -PathType Leaf) "drain grandchild pid was not recorded"
    $drainGrandchildPid = [int][IO.File]::ReadAllText($drainPidPath, $utf8)
    Assert-ProcessGone -ProcessId $drainGrandchildPid -Owner "short-lived descendant"
    Write-Host "core-profile-gates bounded descendant drain: ok"

    $ambientRoot = New-FixtureRoot -Name "ambient-descendant"
    $ambientPidPath = Join-Path $ambientRoot "test-output/ambient.pid"
    $ambientImageName = "vctip.exe"
    $ambientSource = Join-Path $tempBase "ambient-sleeper.rs"
    $ambientBinary = Join-Path $ambientRoot (Join-Path "scripts" $ambientImageName)
    Write-Utf8Text -Path $ambientSource -Text @'
fn main() {
    std::thread::sleep(std::time::Duration::from_secs(30));
}
'@
    $rustc = (Get-Command rustc -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
    $ambientCompile = @(& $rustc --edition=2021 $ambientSource -o $ambientBinary 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "could not compile ambient-descendant fixture:`n$($ambientCompile -join "`n")" }
    if ($IsLinux) { & /usr/bin/chmod +x $ambientBinary; if ($LASTEXITCODE -ne 0) { throw "could not make ambient fixture executable" } }
    Write-Utf8Text -Path (Join-Path $ambientRoot "scripts/spawn-ambient.ps1") -Text @'
$image = Join-Path $PSScriptRoot "vctip.exe"
$startInfo = [Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName = $image
$startInfo.UseShellExecute = $false
$child = [Diagnostics.Process]::Start($startInfo)
[void](New-Item -ItemType Directory -Path (Split-Path -Parent $env:OXVBA_CORE_GATE_TEST_PID_PATH) -Force)
[IO.File]::WriteAllText($env:OXVBA_CORE_GATE_TEST_PID_PATH, [string]$child.Id, [Text.UTF8Encoding]::new($false))
Write-Output "direct-parent-exiting ambient=$($child.Id)"
'@
    $ambientEnvironment = @([pscustomobject]@{ name = "OXVBA_CORE_GATE_TEST_PID_PATH"; action = "set"; value = $ambientPidPath })
    Write-TestManifest -Root $ambientRoot -Manifest (New-TestManifest -Gates @((New-TestGate -Order 1 -Id "ambient-descendant" -Command "scripts/spawn-ambient.ps1" -Environment $ambientEnvironment -TimeoutSeconds 10)) -AmbientDescendantNames @("vctip.exe", "conhost.exe"))
    Initialize-FixtureGit -Root $ambientRoot
    $ambient = Invoke-Runner -Root $ambientRoot -Arguments @("-RepositoryRoot", $ambientRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "ambient-descendant") -TimeoutSeconds 90
    Assert-Equal $ambient.exit_code 0 "ambient-declared descendant was not accepted: $($ambient.stderr)"
    $ambientRun = Get-RunManifest -Root $ambientRoot -RunId "ambient-descendant"
    Assert-Equal $ambientRun.results[0].status "passed" "ambient-declared descendant did not pass"
    Assert-Equal $ambientRun.results[0].tree_cleanup "complete" "ambient-declared descendant tree did not empty"
    Assert-Matches ([string]($ambientRun.results[0].ambient_descendants -join " ")) 'vctip\.exe' "ambient residual identity was not recorded"
    Assert-True ([int64]$ambientRun.results[0].duration_ms -le 12000) "ambient-descendant gate exceeded the bounded drain plus admission/cleanup overhead"
    Assert-True (Test-Path -LiteralPath $ambientPidPath -PathType Leaf) "ambient descendant pid was not recorded"
    $ambientPid = [int][IO.File]::ReadAllText($ambientPidPath, $utf8)
    Assert-ProcessGone -ProcessId $ambientPid -Owner "ambient-declared descendant"
    Write-Host "core-profile-gates ambient toolchain descendant exemption: ok"

    if ($IsLinux) {
        $escapedRoot = New-FixtureRoot -Name "escaped-session"
        $escapedPidPath = Join-Path $escapedRoot "test-output/escaped.pid"
        Write-Utf8Text -Path (Join-Path $escapedRoot "scripts/spawn-escaped-session.ps1") -Text @'
$startInfo = [Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName = "/usr/bin/setsid"
$startInfo.UseShellExecute = $false
foreach ($argument in @(
    "/usr/bin/bash", "-c",
    'printf "%s" "$$" >"$OXVBA_CORE_GATE_TEST_PID_PATH"; exec /usr/bin/sleep 30'
)) { [void]$startInfo.ArgumentList.Add($argument) }
$escaped = [Diagnostics.Process]::Start($startInfo)
$deadline = [DateTime]::UtcNow.AddSeconds(2)
while (-not (Test-Path -LiteralPath $env:OXVBA_CORE_GATE_TEST_PID_PATH) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 10 }
if (-not (Test-Path -LiteralPath $env:OXVBA_CORE_GATE_TEST_PID_PATH)) { throw "escaped session did not publish its pid" }
Start-Sleep -Milliseconds 1200
Write-Output "direct-parent-exiting escaped=$($escaped.Id)"
'@
        $escapedEnvironment = @([pscustomobject]@{ name = "OXVBA_CORE_GATE_TEST_PID_PATH"; action = "set"; value = $escapedPidPath })
        Write-TestManifest -Root $escapedRoot -Manifest (New-TestManifest -Gates @(
                (New-TestGate -Order 1 -Id "escaped-session" -Command "scripts/spawn-escaped-session.ps1" `
                    -Environment $escapedEnvironment -TimeoutSeconds 10)))
        Initialize-FixtureGit -Root $escapedRoot
        $escapedResult = Invoke-Runner -Root $escapedRoot -Arguments @("-RepositoryRoot", $escapedRoot,
            "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "escaped-session")
        Assert-NoSuccessOutput -Result $escapedResult -Owner "escaped Linux session descendant"
        $escapedRun = Get-RunManifest -Root $escapedRoot -RunId "escaped-session"
        Assert-Equal $escapedRun.results[0].reason "descendant-processes-remained-after-direct-exit" "escaped session failure reason drifted"
        Assert-Equal $escapedRun.results[0].tree_cleanup "complete" "escaped session cleanup was incomplete"
        Assert-Equal $escapedRun.results[0].escaped_descendants_observed $true "escaped session was not identified outside the root process group"
        $escapedPid = [int][IO.File]::ReadAllText($escapedPidPath, $utf8)
        Assert-ProcessGone -ProcessId $escapedPid -Owner "escaped-session descendant"
        Write-Host "core-profile-gates escaped-session subreaper containment: ok"
    }

    Invoke-TamperCase -Name "plan" -Script '[IO.File]::WriteAllText($env:OXVBA_CORE_GATE_PLAN_PATH, "{", [Text.UTF8Encoding]::new($false))'
    Invoke-TamperCase -Name "run-status" -Script @'
$path = Join-Path $env:OXVBA_CORE_GATE_EVIDENCE_ROOT "run-manifest.json"
$value = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json -Depth 40
$value.status = "passed"
[IO.File]::WriteAllText($path, (ConvertTo-Json -InputObject $value -Depth 40) + "`n", [Text.UTF8Encoding]::new($false))
'@
    Invoke-TamperCase -Name "summary" -Script '[IO.File]::WriteAllText((Join-Path $env:OXVBA_CORE_GATE_EVIDENCE_ROOT "summary.txt"), "forged`n", [Text.UTF8Encoding]::new($false))'
    Invoke-TamperCase -Name "prior-log" -AfterPassedGate $true -Script '[IO.File]::AppendAllText((Join-Path $env:OXVBA_CORE_GATE_EVIDENCE_ROOT "commands/001-prior-pass/stdout.log"), "forged", [Text.UTF8Encoding]::new($false))'
    Invoke-TamperCase -Name "prior-result" -AfterPassedGate $true -Script @'
$path = Join-Path $env:OXVBA_CORE_GATE_EVIDENCE_ROOT "commands/001-prior-pass/result.json"
$value = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json -Depth 40
$value.status = "failed"
[IO.File]::WriteAllText($path, (ConvertTo-Json -InputObject $value -Depth 40) + "`n", [Text.UTF8Encoding]::new($false))
'@
    Invoke-TamperCase -Name "consistent-result-run" -AfterPassedGate $true -Script @'
$resultPath = Join-Path $env:OXVBA_CORE_GATE_EVIDENCE_ROOT "commands/001-prior-pass/result.json"
$result = Get-Content -LiteralPath $resultPath -Raw | ConvertFrom-Json -Depth 40
$result.status = "failed"
[IO.File]::WriteAllText($resultPath, (ConvertTo-Json -InputObject $result -Depth 40) + "`n", [Text.UTF8Encoding]::new($false))
$runPath = Join-Path $env:OXVBA_CORE_GATE_EVIDENCE_ROOT "run-manifest.json"
$run = Get-Content -LiteralPath $runPath -Raw | ConvertFrom-Json -Depth 40
$run.status = "failed"; $run.results = @($result)
[IO.File]::WriteAllText($runPath, (ConvertTo-Json -InputObject $run -Depth 40) + "`n", [Text.UTF8Encoding]::new($false))
'@

    $dirtyRoot = New-FixtureRoot -Name "dirty-source"
    Write-TestManifest -Root $dirtyRoot -Manifest (New-TestManifest -Gates @((New-TestGate -Order 1 -Id "pass")))
    Initialize-FixtureGit -Root $dirtyRoot
    [IO.File]::AppendAllText((Join-Path $dirtyRoot "scripts/pass.ps1"), "`n# dirty`n", $utf8)
    $dirty = Invoke-Runner -Root $dirtyRoot -Arguments @("-RepositoryRoot", $dirtyRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "dirty")
    Assert-NoSuccessOutput -Result $dirty -Owner "dirty tracked source"
    Assert-Matches "$($dirty.stdout)`n$($dirty.stderr)" 'source checkout must be clean' "dirty source failed for the wrong reason"

    $replaceRoot = New-FixtureRoot -Name "replace-command"
    Write-Utf8Text -Path (Join-Path $replaceRoot "scripts/replace.ps1") -Text '[IO.File]::AppendAllText((Join-Path (Split-Path -Parent $PSScriptRoot) "scripts/second.ps1"), "# replaced", [Text.UTF8Encoding]::new($false))'
    Write-Utf8Text -Path (Join-Path $replaceRoot "scripts/second.ps1") -Text "Write-Output 'second'`n"
    Write-TestManifest -Root $replaceRoot -Manifest (New-TestManifest -Gates @(
        (New-TestGate -Order 1 -Id "replace" -Command "scripts/replace.ps1"),
        (New-TestGate -Order 2 -Id "second" -Command "scripts/second.ps1")
    ))
    Initialize-FixtureGit -Root $replaceRoot
    $replace = Invoke-Runner -Root $replaceRoot -Arguments @("-RepositoryRoot", $replaceRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "replace")
    Assert-NoSuccessOutput -Result $replace -Owner "mid-run command replacement"
    $replaceRun = Get-RunManifest -Root $replaceRoot -RunId "replace"
    Assert-Matches $replaceRun.failure 'source checkout must be clean|identity changed' "mid-run command replacement was not explicit"
    Assert-Equal $replaceRun.results[1].status "not-run" "replacement target command ran"

    $manifestDriftRoot = New-FixtureRoot -Name "manifest-drift"
    Write-Utf8Text -Path (Join-Path $manifestDriftRoot "scripts/manifest-drift.ps1") -Text @'
$blocked = $false
try { [IO.File]::AppendAllText($env:OXVBA_CORE_GATE_MANIFEST_PATH, " ", [Text.UTF8Encoding]::new($false)) }
catch { $blocked = $true; Write-Output "manifest-write-blocked" }
if (-not $blocked) { throw "admitted manifest instance remained writable" }
'@
    Write-TestManifest -Root $manifestDriftRoot -Manifest (New-TestManifest -Gates @((New-TestGate -Order 1 -Id "manifest-drift" -Command "scripts/manifest-drift.ps1")))
    Initialize-FixtureGit -Root $manifestDriftRoot
    $manifestDrift = Invoke-Runner -Root $manifestDriftRoot -Arguments @("-RepositoryRoot", $manifestDriftRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "manifest-drift")
    Assert-Equal $manifestDrift.exit_code 0 "admitted manifest write was not blocked: $($manifestDrift.stderr)"
    $manifestDriftOutput = Get-Content -LiteralPath (Join-Path (Get-RunRoot -Root $manifestDriftRoot -RunId "manifest-drift") "commands/001-manifest-drift/stdout.log") -Raw
    Assert-Matches $manifestDriftOutput 'manifest-write-blocked' "admitted manifest write did not report the immutable-input boundary"
    Write-Host "core-profile-gates committed source/command/manifest seal and immutable manifest input: ok"

    }
    if ($Phase -in @("All", "Extended")) {

    $identityToolDirectory = Join-Path $tempBase "identity-tool-bin"
    $hostileToolDirectory = Join-Path $tempBase "identity-tool-hostile"
    [void](New-Item -ItemType Directory -Path $identityToolDirectory)
    [void](New-Item -ItemType Directory -Path $hostileToolDirectory)
    $identityCargoName = if ($IsWindows) { "cargo.exe" } else { "cargo" }
    $identityCargo = Join-Path $identityToolDirectory $identityCargoName
    $hostileCargo = Join-Path $hostileToolDirectory $identityCargoName
    $identitySource = Join-Path $tempBase "identity-cargo.rs"
    $hostileSource = Join-Path $tempBase "identity-cargo-hostile.rs"
    Write-Utf8Text -Path $identitySource -Text @'
use std::env;
fn main() {
    if env::args().any(|arg| arg == "--version") { println!("cargo 1.94.1 (identity-bound fixture)"); }
    else { println!("ORIGINAL-EXECUTABLE"); }
}
'@
    Write-Utf8Text -Path $hostileSource -Text @'
use std::env;
fn main() {
    if env::args().any(|arg| arg == "--version") { println!("cargo 9.99.9 (hostile fixture)"); }
    else { println!("HOSTILE-EXECUTABLE"); }
}
'@
    $rustc = (Get-Command rustc -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
    $identityCompile = @(& $rustc --edition=2021 $identitySource -o $identityCargo 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "could not compile identity-bound executable fixture:`n$($identityCompile -join "`n")" }
    $hostileCompile = @(& $rustc --edition=2021 $hostileSource -o $hostileCargo 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "could not compile hostile executable fixture:`n$($hostileCompile -join "`n")" }
    if ($IsLinux) {
        & /usr/bin/chmod +x $identityCargo $hostileCargo
        if ($LASTEXITCODE -ne 0) { throw "could not make identity-bound executable fixtures executable" }
    }

    $identityExecutableRoot = New-FixtureRoot -Name "identity-bound-executable"
    Write-TestManifest -Root $identityExecutableRoot -Manifest (New-TestManifest -Gates @(
            (New-TestGate -Order 1 -Id "executable-identity" -Kind "cargo" -Command "cargo" -Arguments @("--identity-gate") -CargoWorkspace $true)))
    Initialize-FixtureGit -Root $identityExecutableRoot
    $identityReady = Join-Path $tempBase "identity-executable.ready"
    $identityRelease = Join-Path $tempBase "identity-executable.release"
    $unrelatedPath = Join-Path $tempBase "identity-unrelated.txt"
    $unrelatedMoved = Join-Path $tempBase "identity-unrelated.moved.txt"
    Write-Utf8Text -Path $unrelatedPath -Text "unrelated`n"
    $sentinelStart = [Diagnostics.ProcessStartInfo]::new()
    $sentinelStart.FileName = if ($IsWindows) { $pwsh } else { "/usr/bin/sleep" }
    $sentinelStart.UseShellExecute = $false
    if ($IsWindows) {
        # Admission performs several owned tool/source probes before the gate-
        # unique pause. Keep the unrelated sentinel beyond loaded-host probe
        # time; the finally block still terminates only this exact owned PID.
        foreach ($argument in @("-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "Start-Sleep -Seconds 180")) {
            [void]$sentinelStart.ArgumentList.Add($argument)
        }
    }
    else { [void]$sentinelStart.ArgumentList.Add("180") }
    $identitySentinel = [Diagnostics.Process]::Start($sentinelStart)
    $identityHandle = $null
    try {
        $identityHandle = New-RunnerProcess -Root $identityExecutableRoot -Arguments @("-RepositoryRoot", $identityExecutableRoot,
            "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "identity-executable") `
            -Environment @{
                PATH = "$identityToolDirectory$([IO.Path]::PathSeparator)$env:PATH"
                # The manifest is admitted only by the delivery gate, unlike
                # Cargo which is also admitted by its version probe. Pausing on
                # this gate-unique input proves the Cargo instance and every
                # other gate input have already been retained.
                OXVBA_CORE_GATE_TEST_INPUT_ADMISSION_MATCH = (Join-Path $identityExecutableRoot "ci/core-profile/gates-v1.json")
                OXVBA_CORE_GATE_TEST_INPUT_ADMISSION_READY = $identityReady
                OXVBA_CORE_GATE_TEST_INPUT_ADMISSION_RELEASE = $identityRelease
            }
        try { Wait-TestFile -Path $identityReady -TimeoutSeconds 60 }
        catch {
            if (-not (Test-Path -LiteralPath $identityRelease)) { Write-Utf8Text -Path $identityRelease -Text "release`n" }
            $earlyIdentityResult = Complete-RunnerProcess -Handle $identityHandle -TimeoutSeconds 60
            $identityHandle = $null
            throw "identity-bound executable runner did not reach the gate-unique manifest admission; stdout=$($earlyIdentityResult.stdout); stderr=$($earlyIdentityResult.stderr)"
        }
        if ($IsWindows) {
            $writeBlocked = $false
            try { [IO.File]::WriteAllText($identityCargo, "hostile", $utf8) } catch { $writeBlocked = $true }
            $fileRenameBlocked = $false
            try { Move-Item -LiteralPath $identityCargo -Destination "$identityCargo.moved" -ErrorAction Stop } catch { $fileRenameBlocked = $true }
            $directoryRenameBlocked = $false
            try { [IO.Directory]::Move($identityToolDirectory, "$identityToolDirectory.moved") } catch { $directoryRenameBlocked = $true }
            Assert-True ($writeBlocked -and $fileRenameBlocked -and $directoryRenameBlocked) "Windows admitted executable or ancestor remained replaceable"
        }
        else {
            Move-Item -LiteralPath $identityToolDirectory -Destination "$identityToolDirectory.admitted" -ErrorAction Stop
            [void](New-Item -ItemType SymbolicLink -Path $identityToolDirectory -Target $hostileToolDirectory)
        }
        Move-Item -LiteralPath $unrelatedPath -Destination $unrelatedMoved -ErrorAction Stop
        Write-Utf8Text -Path $identityRelease -Text "release`n"
        $identityResult = Complete-RunnerProcess -Handle $identityHandle -TimeoutSeconds 60
        $identityHandle = $null
        if ($IsWindows) { Assert-Equal $identityResult.exit_code 0 "Windows identity-bound executable run failed: $($identityResult.stderr)" }
        else {
            Assert-NoSuccessOutput -Result $identityResult -Owner "Linux post-admission executable replacement"
            Assert-Matches "$($identityResult.stdout)`n$($identityResult.stderr)" "tool 'cargo' identity changed" "Linux executable replacement did not fail closed after exact execution"
        }
        $identityOutput = Get-Content -LiteralPath (Join-Path (Get-RunRoot -Root $identityExecutableRoot -RunId "identity-executable") "commands/001-executable-identity/stdout.log") -Raw
        Assert-Matches $identityOutput 'ORIGINAL-EXECUTABLE' "admitted executable bytes were not consumed"
        Assert-True ($identityOutput -notmatch 'HOSTILE-EXECUTABLE') "replacement executable bytes were consumed"
        Assert-True (Test-Path -LiteralPath $unrelatedMoved -PathType Leaf) "unrelated file rename was redirected or blocked"
        Assert-True (-not $identitySentinel.HasExited) "identity-bound cleanup terminated an unrelated sentinel"
    }
    finally {
        if (-not (Test-Path -LiteralPath $identityRelease)) { Write-Utf8Text -Path $identityRelease -Text "release`n" }
        if ($null -ne $identityHandle) { try { [void](Complete-RunnerProcess -Handle $identityHandle -TimeoutSeconds 60) } catch {} }
        if (-not $identitySentinel.HasExited) { $identitySentinel.Kill($true); [void]$identitySentinel.WaitForExit(5000) }
        $identitySentinel.Dispose()
    }
    Write-Host "core-profile-gates identity-bound executable and ancestor replacement: ok"

    $identityCommandRoot = New-FixtureRoot -Name "identity-bound-command"
    $identityCommand = Join-Path $identityCommandRoot "scripts/identity-command.ps1"
    Write-Utf8Text -Path $identityCommand -Text "Write-Output 'ORIGINAL-COMMAND'`n"
    $hostileCommandDirectory = Join-Path $tempBase "identity-command-hostile"
    [void](New-Item -ItemType Directory -Path $hostileCommandDirectory)
    Write-Utf8Text -Path (Join-Path $hostileCommandDirectory "identity-command.ps1") -Text "Write-Output 'HOSTILE-COMMAND'`n"
    Write-TestManifest -Root $identityCommandRoot -Manifest (New-TestManifest -Gates @(
            (New-TestGate -Order 1 -Id "command-identity" -Command "scripts/identity-command.ps1")))
    Initialize-FixtureGit -Root $identityCommandRoot
    $commandReady = Join-Path $tempBase "identity-command.ready"
    $commandRelease = Join-Path $tempBase "identity-command.release"
    $commandEnvironment = @{
        OXVBA_CORE_GATE_TEST_INPUT_ADMISSION_MATCH = $identityCommand
        OXVBA_CORE_GATE_TEST_INPUT_ADMISSION_READY = $commandReady
        OXVBA_CORE_GATE_TEST_INPUT_ADMISSION_RELEASE = $commandRelease
    }
    if ($IsLinux) { $commandEnvironment.OXVBA_CORE_GATE_TEST_PROBE_UNRELATED_INHERITANCE = "1" }
    $commandHandle = New-RunnerProcess -Root $identityCommandRoot -Arguments @("-RepositoryRoot", $identityCommandRoot,
        "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "identity-command") `
        -Environment $commandEnvironment
    try {
        Wait-TestFile -Path $commandReady -TimeoutSeconds 60
        if ($IsWindows) {
            $commandRenameBlocked = $false
            try { Move-Item -LiteralPath $identityCommand -Destination "$identityCommand.moved" -ErrorAction Stop } catch { $commandRenameBlocked = $true }
            $commandAncestorBlocked = $false
            try { [IO.Directory]::Move((Join-Path $identityCommandRoot "scripts"), (Join-Path $identityCommandRoot "scripts.moved")) } catch { $commandAncestorBlocked = $true }
            Assert-True ($commandRenameBlocked -and $commandAncestorBlocked) "Windows admitted command or ancestor remained replaceable"
        }
        else {
            Move-Item -LiteralPath (Join-Path $identityCommandRoot "scripts") -Destination (Join-Path $identityCommandRoot "scripts.admitted") -ErrorAction Stop
            [void](New-Item -ItemType SymbolicLink -Path (Join-Path $identityCommandRoot "scripts") -Target $hostileCommandDirectory)
        }
        Write-Utf8Text -Path $commandRelease -Text "release`n"
        $commandResult = Complete-RunnerProcess -Handle $commandHandle -TimeoutSeconds 60
        $commandHandle = $null
        if ($IsWindows) { Assert-Equal $commandResult.exit_code 0 "Windows identity-bound command run failed: $($commandResult.stderr)" }
        else {
            Assert-NoSuccessOutput -Result $commandResult -Owner "Linux post-admission command-ancestor replacement"
            Assert-Matches "$($commandResult.stdout)`n$($commandResult.stderr)" 'source checkout must be clean|identity changed|disappeared' "Linux command-ancestor replacement did not fail closed"
        }
        $commandOutput = Get-Content -LiteralPath (Join-Path (Get-RunRoot -Root $identityCommandRoot -RunId "identity-command") "commands/001-command-identity/stdout.log") -Raw
        Assert-Matches $commandOutput 'ORIGINAL-COMMAND' "admitted command bytes were not consumed"
        Assert-True ($commandOutput -notmatch 'HOSTILE-COMMAND') "replacement command bytes were consumed"
    }
    finally {
        if (-not (Test-Path -LiteralPath $commandRelease)) { Write-Utf8Text -Path $commandRelease -Text "release`n" }
        if ($null -ne $commandHandle) { try { [void](Complete-RunnerProcess -Handle $commandHandle -TimeoutSeconds 60) } catch {} }
    }
    Write-Host "core-profile-gates identity-bound command/interpreter handoff: ok"

    if ($IsWindows) {
        $handleRoot = New-FixtureRoot -Name "handle-allowlist"
        Write-Utf8Text -Path (Join-Path $handleRoot "scripts/check-handle-allowlist.ps1") -Text @'
$source = @"
using System;
using System.Runtime.InteropServices;
public static class CoreGateHandleProbe {
    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool SetEvent(IntPtr handle);
}
"@
Add-Type -TypeDefinition $source
$handle = [IntPtr][int64]$env:OXVBA_CORE_GATE_TEST_SENTINEL_HANDLE
$set = [CoreGateHandleProbe]::SetEvent($handle)
$error = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
Write-Output "sentinel-set-attempted result=$set error=$error handle=$handle"
'@
        Write-TestManifest -Root $handleRoot -Manifest (New-TestManifest -Gates @(
                (New-TestGate -Order 1 -Id "handle-allowlist" -Command "scripts/check-handle-allowlist.ps1")))
        Initialize-FixtureGit -Root $handleRoot
        $handleResult = Invoke-Runner -Root $handleRoot -Arguments @("-RepositoryRoot", $handleRoot,
            "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "handle-allowlist") `
            -Environment @{ OXVBA_CORE_GATE_TEST_SENTINEL = "1" }
        Assert-Equal $handleResult.exit_code 0 "STARTUPINFOEX handle allowlist failed: $($handleResult.stderr)"
        $handleRun = Get-RunManifest -Root $handleRoot -RunId "handle-allowlist"
        Assert-Equal $handleRun.results[0].containment "windows-job-object-v2" "Windows handle-list containment was not recorded"
        Assert-Matches (Get-Content -LiteralPath (Join-Path (Get-RunRoot -Root $handleRoot -RunId "handle-allowlist") "commands/001-handle-allowlist/stdout.log") -Raw) `
            'sentinel-set-attempted' "sentinel probe did not execute"
        Write-Host "core-profile-gates STARTUPINFOEX handle allowlist: ok"
    }

    if ($IsLinux) {
        $bashRoot = New-FixtureRoot -Name "exact-bash"
        $fakeBashDirectory = Join-Path $tempBase "hostile-bash-bin"
        [void](New-Item -ItemType Directory -Path $fakeBashDirectory)
        $fakeBashMarker = Join-Path $bashRoot "test-output/fake-bash-ran.txt"
        foreach ($name in @("bash")) {
            Write-Utf8Text -Path (Join-Path $fakeBashDirectory $name) -Text "#!/usr/bin/bash`nprintf '%s' '$name' >>'$fakeBashMarker'`nexit 97`n"
            & /usr/bin/chmod +x (Join-Path $fakeBashDirectory $name)
        }
        if ($LASTEXITCODE -ne 0) { throw "could not make hostile fake Bash executable" }
        Write-TestManifest -Root $bashRoot -Manifest (New-TestManifest -Gates @(
                (New-TestGate -Order 1 -Id "exact-bash")))
        Initialize-FixtureGit -Root $bashRoot
        $bashResult = Invoke-Runner -Root $bashRoot -Arguments @("-RepositoryRoot", $bashRoot,
            "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "exact-bash") `
            -Environment @{ PATH = "$fakeBashDirectory$([IO.Path]::PathSeparator)$env:PATH" }
        Assert-Equal $bashResult.exit_code 0 "hostile PATH Bash displaced exact /usr/bin/bash: $($bashResult.stderr)"
        Assert-True (-not (Test-Path -LiteralPath $fakeBashMarker)) "hostile PATH Bash was executed"
        $bashRun = Get-RunManifest -Root $bashRoot -RunId "exact-bash"
        $bashIdentity = @($bashRun.tools | Where-Object { $_.id -ceq "bash" })
        Assert-Equal $bashIdentity.Count 1 "exact Bash identity was not recorded once"
        Assert-Equal $bashIdentity[0].path "/usr/bin/bash" "exact Bash identity path drifted"
        Assert-Matches $bashIdentity[0].sha256 '^[0-9a-f]{64}$' "exact Bash identity digest is absent"
        Write-Host "core-profile-gates exact Bash seal and hostile PATH rejection: ok"
    }

    $mutableToolRoot = New-FixtureRoot -Name "mutable-tool"
    Write-Utf8Text -Path (Join-Path $mutableToolRoot "scripts/mutate-tool.ps1") -Text '[IO.File]::WriteAllBytes($env:OXVBA_CORE_GATE_TEST_TOOL_PATH, ([IO.File]::ReadAllBytes($env:OXVBA_CORE_GATE_TEST_TOOL_PATH) + [byte[]]@(0)))'
    $fakeToolDirectory = Join-Path $tempBase "mutable-tool-bin"
    [void](New-Item -ItemType Directory -Path $fakeToolDirectory)
    $fakeCargoName = if ($IsWindows) { "cargo.exe" } else { "cargo" }
    $fakeCargo = Join-Path $fakeToolDirectory $fakeCargoName
    [IO.File]::Copy($cargo, $fakeCargo, $false)
    if ($IsLinux) { & /usr/bin/chmod +x $fakeCargo; if ($LASTEXITCODE -ne 0) { throw "could not make fake cargo executable" } }
    $toolEnvironment = @([pscustomobject]@{ name = "OXVBA_CORE_GATE_TEST_TOOL_PATH"; action = "set"; value = $fakeCargo })
    Write-TestManifest -Root $mutableToolRoot -Manifest (New-TestManifest -Gates @((New-TestGate -Order 1 -Id "mutate-tool" -Command "scripts/mutate-tool.ps1" -Environment $toolEnvironment)))
    Initialize-FixtureGit -Root $mutableToolRoot
    $toolPath = "$fakeToolDirectory$([IO.Path]::PathSeparator)$env:PATH"
    $mutableTool = Invoke-Runner -Root $mutableToolRoot -Arguments @("-RepositoryRoot", $mutableToolRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "mutable-tool") -Environment @{ PATH = $toolPath }
    Assert-NoSuccessOutput -Result $mutableTool -Owner "mutable exact tool"
    Assert-Matches (Get-RunManifest -Root $mutableToolRoot -RunId "mutable-tool").failure "tool 'cargo' identity changed" "tool drift was not explicit"
    Write-Host "core-profile-gates exact tool identity seal: ok"

    $missingToolRoot = New-FixtureRoot -Name "missing-tool"
    Write-TestManifest -Root $missingToolRoot -Manifest (New-TestManifest -Gates @((New-TestGate -Order 1 -Id "pass")))
    Initialize-FixtureGit -Root $missingToolRoot
    $gitOnlyPath = Split-Path -Parent $git
    $missingTool = Invoke-Runner -Root $missingToolRoot -Arguments @("-RepositoryRoot", $missingToolRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "missing-tool") -Environment @{ PATH = $gitOnlyPath }
    Assert-NoSuccessOutput -Result $missingTool -Owner "missing Cargo tool"
    Assert-Matches "$($missingTool.stdout)`n$($missingTool.stderr)" "required tool 'cargo' is unavailable" "missing Cargo failed for the wrong reason"

    $pipeRoot = New-FixtureRoot -Name "probe-pipe-drain"
    Write-TestManifest -Root $pipeRoot -Manifest (New-TestManifest -Gates @((New-TestGate -Order 1 -Id "pass")))
    Initialize-FixtureGit -Root $pipeRoot
    $pipeToolDirectory = Join-Path $tempBase "probe-pipe-bin"
    [void](New-Item -ItemType Directory -Path $pipeToolDirectory)
    $pipeSource = Join-Path $pipeToolDirectory "pipe_holder.rs"
    $pipeCargo = Join-Path $pipeToolDirectory $(if ($IsWindows) { "cargo.exe" } else { "cargo" })
    $pipePidPath = Join-Path $tempBase "probe-pipe-holder.pid"
    Write-Utf8Text -Path $pipeSource -Text @'
use std::{env, fs, process::{self, Command, Stdio}, thread, time::Duration};
fn main() {
    if env::var("OXVBA_CORE_GATE_TEST_PIPE_CHILD").ok().as_deref() == Some("1") {
        fs::write(env::var("OXVBA_CORE_GATE_TEST_PIPE_PID_PATH").unwrap(), process::id().to_string()).unwrap();
        thread::sleep(Duration::from_secs(30));
        return;
    }
    let executable = env::current_exe().unwrap();
    Command::new(executable)
        .arg("--pipe-holder")
        .env("OXVBA_CORE_GATE_TEST_PIPE_CHILD", "1")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    println!("cargo 1.94.1 (controlled probe)");
}
'@
    $rustc = (Get-Command rustc -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
    $rustcOutput = @(& $rustc --edition=2021 $pipeSource -o $pipeCargo 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "could not compile retained-pipe probe helper:`n$($rustcOutput -join "`n")" }
    if ($IsLinux) { & /usr/bin/chmod +x $pipeCargo; if ($LASTEXITCODE -ne 0) { throw "could not make retained-pipe probe executable" } }
    $sentinelStart = [Diagnostics.ProcessStartInfo]::new()
    $sentinelStart.FileName = $pwsh
    $sentinelStart.WorkingDirectory = $pipeRoot
    $sentinelStart.UseShellExecute = $false
    foreach ($argument in @("-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "Start-Sleep -Seconds 30")) {
        [void]$sentinelStart.ArgumentList.Add($argument)
    }
    $sentinel = [Diagnostics.Process]::Start($sentinelStart)
    try {
        $pipeResult = Invoke-Runner -Root $pipeRoot -Arguments @("-RepositoryRoot", $pipeRoot,
            "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "pipe-drain") `
            -Environment @{
                PATH = "$pipeToolDirectory$([IO.Path]::PathSeparator)$env:PATH"
                OXVBA_CORE_GATE_TEST_PIPE_PID_PATH = $pipePidPath
            } -TimeoutSeconds 30
        Assert-NoSuccessOutput -Result $pipeResult -Owner "tool probe retained output pipe"
        Assert-Matches "$($pipeResult.stdout)`n$($pipeResult.stderr)" 'descendant-processes-remained-after-direct-exit' "retained probe pipe failed for the wrong reason"
        Assert-True ($pipeResult.duration_ms -le 12000) "retained probe pipe exceeded its bounded containment deadline (tool probe admission plus bounded drain window plus overhead)"
        Assert-True (Test-Path -LiteralPath $pipePidPath -PathType Leaf) "retained-pipe descendant did not publish its pid"
        $pipePid = [int][IO.File]::ReadAllText($pipePidPath, $utf8)
        Assert-ProcessGone -ProcessId $pipePid -Owner "retained-pipe descendant"
        Assert-True (-not $sentinel.HasExited) "tool-probe containment terminated an unrelated sentinel"
    }
    finally {
        if (-not $sentinel.HasExited) { $sentinel.Kill($true); [void]$sentinel.WaitForExit(5000) }
        $sentinel.Dispose()
    }
    Write-Host "core-profile-gates owned tool-probe descendant cleanup: ok"

    $reparseRoot = New-FixtureRoot -Name "reparse-command"
    $reparseTarget = Join-Path $tempBase "reparse-target"
    [void](New-Item -ItemType Directory -Path $reparseTarget)
    Write-Utf8Text -Path (Join-Path $reparseTarget "pass.ps1") -Text "Write-Output 'linked'`n"
    $linkPath = Join-Path $reparseRoot "scripts/link"
    if ($IsWindows) { [void](New-Item -ItemType Junction -Path $linkPath -Target $reparseTarget) }
    else { [void](New-Item -ItemType SymbolicLink -Path $linkPath -Target $reparseTarget) }
    Write-TestManifest -Root $reparseRoot -Manifest (New-TestManifest -Gates @((New-TestGate -Order 1 -Id "linked" -Command "scripts/link/pass.ps1")))
    $reparse = Invoke-Runner -Root $reparseRoot -Arguments @("-RepositoryRoot", $reparseRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "ValidateManifest")
    Assert-NoSuccessOutput -Result $reparse -Owner "reparse command path"
    Assert-Matches "$($reparse.stdout)`n$($reparse.stderr)" 'reparse/symlink' "reparse command failed for the wrong reason"
    Write-Host "core-profile-gates reparse/symlink rejection: ok"

    $rootSwapRoot = New-FixtureRoot -Name "evidence-root-swap"
    $externalEvidence = Join-Path $tempBase "external-evidence-target"
    [void](New-Item -ItemType Directory -Path $externalEvidence)
    Write-Utf8Text -Path (Join-Path $rootSwapRoot "scripts/swap-evidence-root.ps1") -Text @'
$root = $env:OXVBA_CORE_GATE_EVIDENCE_ROOT
$moved = "$root.owned"
Move-Item -LiteralPath $root -Destination $moved
if ($IsWindows) { [void](New-Item -ItemType Junction -Path $root -Target $env:OXVBA_CORE_GATE_TEST_EXTERNAL_EVIDENCE) }
else { [void](New-Item -ItemType SymbolicLink -Path $root -Target $env:OXVBA_CORE_GATE_TEST_EXTERNAL_EVIDENCE) }
Write-Output "evidence-root-replaced"
'@
    $externalEnvironment = @([pscustomobject]@{
            name = "OXVBA_CORE_GATE_TEST_EXTERNAL_EVIDENCE"; action = "set"; value = $externalEvidence
        })
    Write-TestManifest -Root $rootSwapRoot -Manifest (New-TestManifest -Gates @(
            (New-TestGate -Order 1 -Id "root-swap" -Command "scripts/swap-evidence-root.ps1" `
                -Environment $externalEnvironment)))
    Initialize-FixtureGit -Root $rootSwapRoot
    $rootSwap = Invoke-Runner -Root $rootSwapRoot -Arguments @("-RepositoryRoot", $rootSwapRoot,
        "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "root-swap")
    Assert-NoSuccessOutput -Result $rootSwap -Owner "evidence-root reparse swap"
    Assert-Matches "$($rootSwap.stdout)`n$($rootSwap.stderr)" 'reparse/symlink' "evidence-root swap was not rejected at an evidence boundary"
    Assert-Equal @(Get-ChildItem -LiteralPath $externalEvidence -Force).Count 0 "external reparse target received terminal evidence"
    $linkedRunRoot = Get-RunRoot -Root $rootSwapRoot -RunId "root-swap"
    $movedRunRoot = "$linkedRunRoot.owned"
    if (Test-Path -LiteralPath $linkedRunRoot) { Remove-Item -LiteralPath $linkedRunRoot -Force }
    if (Test-Path -LiteralPath $movedRunRoot) { Move-Item -LiteralPath $movedRunRoot -Destination $linkedRunRoot }
    Write-Host "core-profile-gates evidence-root swap confinement: ok"

    Assert-ManifestFailure -Name "unknown-root-key" -Pattern 'properties must be exactly|unexpected or mis-cased' -Mutate { param($m, $r) $m | Add-Member extra "forbidden" }
    Assert-ManifestFailure -Name "string-version" -Pattern 'version must be integer 1' -Mutate { param($m, $r) $m.version = "1" }
    Assert-ManifestFailure -Name "unsupported-platform-set" -Pattern 'supported_platforms must be exactly' -Mutate { param($m, $r) $m.supported_platforms = @("windows-x64") }
    Assert-ManifestFailure -Name "scalar-supported-platforms" -Pattern 'supported_platforms must be a JSON array' -Mutate { param($m, $r) $m.supported_platforms = "windows-x64" }
    Assert-ManifestFailure -Name "scalar-gates" -Pattern 'manifest.gates must be a JSON array' -Mutate { param($m, $r) $m.gates = $m.gates[0] }
    Assert-ManifestFailure -Name "order-gap" -Pattern 'contiguous integer 1' -Mutate { param($m, $r) $m.gates[0].order = 2 }
    Assert-ManifestFailure -Name "unknown-gate-platform" -Pattern 'unknown or duplicate platform' -Mutate { param($m, $r) $m.gates[0].platforms = @("macos-x64") }
    Assert-ManifestFailure -Name "scalar-gate-platforms" -Pattern 'gates\[0\]\.platforms must be a JSON array' -Mutate { param($m, $r) $m.gates[0].platforms = "windows-x64" }
    Assert-ManifestFailure -Name "scalar-gate-arguments" -Pattern 'gates\[0\]\.arguments must be a JSON array' -Mutate { param($m, $r) $m.gates[0].arguments = "-NoArtifacts" }
    Assert-ManifestFailure -Name "scalar-gate-environment" -Pattern 'gates\[0\]\.environment must be a JSON array' -Mutate { param($m, $r) $m.gates[0].environment = [pscustomobject]@{ name = "RUST_BACKTRACE"; action = "set"; value = "1" } }
    Assert-ManifestFailure -Name "missing-command" -Pattern 'command is missing' -Mutate { param($m, $r) $m.gates[0].command = "scripts/missing.ps1" }
    Assert-ManifestFailure -Name "path-escape" -Pattern 'scripts/\*\.ps1 path|repository-relative path' -Mutate { param($m, $r) $m.gates[0].command = "../outside.ps1" }
    Assert-ManifestFailure -Name "zero-timeout" -Pattern 'timeout_seconds must be an integer' -Mutate { param($m, $r) $m.gates[0].timeout_seconds = 0 }
    Assert-ManifestFailure -Name "null-cargo-boolean" -Pattern 'cargo_workspace must be a JSON boolean' -Mutate { param($m, $r) $m.gates[0].cargo_workspace = $null }
    Assert-ManifestFailure -Name "unlocked-cargo-command" -Pattern 'must mark every cargo command as cargo_workspace=true' -Mutate { param($m, $r) $m.gates[0].kind = "cargo"; $m.gates[0].command = "cargo"; $m.gates[0].cargo_workspace = $false }
    Assert-ManifestFailure -Name "wrong-evidence-path" -Pattern 'evidence_path must be the unique exact path' -Mutate { param($m, $r) $m.gates[0].evidence_path = "../outside" }
    Assert-ManifestFailure -Name "forbidden-environment" -Pattern 'not an allowed unique gate variable' -Mutate { param($m, $r) $m.gates[0].environment = @([pscustomobject]@{ name = "PATH"; action = "set"; value = "mutable" }) }
    Assert-ManifestFailure -Name "forbidden-snapshot-mutation" -Pattern 'forbidden snapshot mutation surface' -Mutate { param($m, $r) $m.gates[0].arguments = @("--bless") }
    Assert-ManifestFailure -Name "duplicate-gate-id" -Pattern 'unique lowercase kebab-case identity' -Mutate { param($m, $r) $m.gates = @((New-TestGate -Order 1 -Id "duplicate"), (New-TestGate -Order 2 -Id "duplicate")) }
    Assert-ManifestFailure -Name "no-linux-lane" -Pattern 'no explicit gate lane for linux-x64' -Mutate { param($m, $r) $m.gates[0].platforms = @("windows-x64") }
    Assert-ManifestFailure -Name "missing-digest-path" -Pattern 'properties must be exactly|unexpected or mis-cased' -Mutate { param($m, $r) $m.evidence.PSObject.Properties.Remove("run_manifest_digest_path") }
    Assert-ManifestFailure -Name "bad-cleanup-reserve" -Pattern 'cleanup_reserve_ms must be an integer' -Mutate { param($m, $r) $m.supervision.cleanup_reserve_ms = 1 }
    Assert-ManifestFailure -Name "bad-windows-transport" -Pattern 'windows_transport must be' -Mutate { param($m, $r) $m.supervision.windows_transport = "unowned" }
    Assert-ManifestFailure -Name "bad-linux-transport" -Pattern 'linux_transport must be' -Mutate { param($m, $r) $m.supervision.linux_transport = "unowned" }
    Assert-ManifestFailure -Name "bad-linux-bash" -Pattern 'linux_bash_path must be' -Mutate { param($m, $r) $m.supervision.linux_bash_path = "/tmp/bash" }
    Assert-ManifestFailure -Name "scalar-ambient-descendant-names" -Pattern 'ambient_descendant_names must be a JSON array' -Mutate { param($m, $r) $m.supervision.ambient_descendant_names = "vctip.exe" }
    Assert-ManifestFailure -Name "bad-ambient-descendant-name" -Pattern 'not a plain executable image' -Mutate { param($m, $r) $m.supervision.ambient_descendant_names = @("vctip") }
    Assert-ManifestFailure -Name "too-many-ambient-descendant-names" -Pattern 'at most 16' -Mutate { param($m, $r) $m.supervision.ambient_descendant_names = @(1..17 | ForEach-Object { "ambient$_.exe" }) }
    Assert-ManifestFailure -Name "duplicate-json-key" -Pattern "duplicate JSON property 'plan_id'" -Mutate {
        param($m, $r); Write-TestManifest -Root $r -Manifest $m; $path = Join-Path $r "ci/core-profile/gates-v1.json"
        $text = [IO.File]::ReadAllText($path, $utf8).Replace('  "plan_id":', "  `"plan_id`": `"duplicate`",`n  `"plan_id`":", [StringComparison]::Ordinal)
        Write-Utf8Text -Path $path -Text $text
    }
    Assert-ManifestFailure -Name "bare-carriage-return" -Pattern 'bare carriage return' -Mutate {
        param($m, $r); Write-TestManifest -Root $r -Manifest $m; $path = Join-Path $r "ci/core-profile/gates-v1.json"
        [byte[]]$bytes = [IO.File]::ReadAllBytes($path); [byte[]]$combined = [byte[]]::new($bytes.Length + 1)
        [Array]::Copy($bytes, $combined, $bytes.Length); $combined[$bytes.Length] = 13; [IO.File]::WriteAllBytes($path, $combined)
    }

    }
    if ($Phase -in @("All", "Extended", "Concurrency")) {

    $abandonedRoot = New-FixtureRoot -Name "abandoned-cargo-lock"
    $abandonedReady = Join-Path $abandonedRoot "test-output/mutex-holder.ready"
    Write-Utf8Text -Path (Join-Path $abandonedRoot "scripts/hold-mutex.ps1") -Text @'
param([string]$Name, [string]$ReadyPath)
$mutex = [Threading.Mutex]::new($false, $Name)
if (-not $mutex.WaitOne(5000)) { throw "could not acquire controlled mutex" }
[void](New-Item -ItemType Directory -Path (Split-Path -Parent $ReadyPath) -Force)
[IO.File]::WriteAllText($ReadyPath, "owned", [Text.UTF8Encoding]::new($false))
Start-Sleep -Seconds 30
'@
    Write-TestManifest -Root $abandonedRoot -Manifest (New-TestManifest -Gates @(
            (New-TestGate -Order 1 -Id "abandoned-lock" -CargoWorkspace $true)))
    Initialize-FixtureGit -Root $abandonedRoot
    $mutexName = Get-TestCargoMutexName -Root $abandonedRoot
    $holderStart = [Diagnostics.ProcessStartInfo]::new()
    $holderStart.FileName = $pwsh
    $holderStart.WorkingDirectory = $abandonedRoot
    $holderStart.UseShellExecute = $false
    foreach ($argument in @("-NoLogo", "-NoProfile", "-NonInteractive", "-File",
            (Join-Path $abandonedRoot "scripts/hold-mutex.ps1"), "-Name", $mutexName,
            "-ReadyPath", $abandonedReady)) { [void]$holderStart.ArgumentList.Add([string]$argument) }
    $holder = $null
    $abandonedHandle = $null
    try {
        $holder = [Diagnostics.Process]::Start($holderStart)
        $holderReadyDeadline = [DateTime]::UtcNow.AddSeconds(5)
        while (-not (Test-Path -LiteralPath $abandonedReady) -and [DateTime]::UtcNow -lt $holderReadyDeadline) { Start-Sleep -Milliseconds 10 }
        Assert-True (Test-Path -LiteralPath $abandonedReady) "controlled mutex holder did not acquire the lock"
        $abandonedHandle = New-RunnerProcess -Root $abandonedRoot -Arguments @("-RepositoryRoot", $abandonedRoot,
            "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "abandoned")
        $abandonedGateRoot = Join-Path (Get-RunRoot -Root $abandonedRoot -RunId "abandoned") "commands/001-abandoned-lock"
        # Admission now runs exact tool/source probes under owned containment.
        # Keep this test-only watchdog outside the product timeout large enough
        # for those fail-closed checks on a loaded host.
        $waiterDeadline = [DateTime]::UtcNow.AddSeconds(45)
        while (-not (Test-Path -LiteralPath $abandonedGateRoot) -and -not $abandonedHandle.process.HasExited -and
            [DateTime]::UtcNow -lt $waiterDeadline) { Start-Sleep -Milliseconds 10 }
        Assert-True (Test-Path -LiteralPath $abandonedGateRoot) "runner did not reach the controlled Cargo mutex wait"
        # The command evidence directory is created immediately before the mutex
        # wait, but a heavily loaded Windows host can still deschedule the runner
        # between those operations. Keep the holder alive long enough to prove a
        # real blocked waiter without changing any product gate deadline.
        Start-Sleep -Milliseconds 1500
        $holder.Kill($true)
        [void]$holder.WaitForExit(5000)
        $holder.Dispose()
        $holder = $null
        $abandonedResult = Complete-RunnerProcess -Handle $abandonedHandle
        $abandonedHandle = $null
        Assert-Equal $abandonedResult.exit_code 0 "abandoned Cargo mutex recovery failed: $($abandonedResult.stderr)"
        $abandonedRun = Get-RunManifest -Root $abandonedRoot -RunId "abandoned"
        Assert-Equal $abandonedRun.results[0].cargo_lock_abandoned_recovered $true "abandoned Cargo mutex was not explicitly recovered"
        Assert-True ([int64]$abandonedRun.results[0].cargo_lock_wait_ms -ge 1000) "abandoned mutex recovery did not observe the waiting owner"
    }
    finally {
        if ($null -ne $holder) {
            if (-not $holder.HasExited) { $holder.Kill($true); [void]$holder.WaitForExit(5000) }
            $holder.Dispose()
        }
        if ($null -ne $abandonedHandle) {
            if (-not $abandonedHandle.process.HasExited) {
                $abandonedHandle.process.Kill($true)
                [void]$abandonedHandle.process.WaitForExit(5000)
            }
            $abandonedHandle.process.Dispose()
        }
    }
    Write-Host "core-profile-gates abandoned Cargo mutex recovery: ok"

    $concurrentRoot = New-FixtureRoot -Name "cargo-concurrency"
    $timeline = Join-Path $concurrentRoot "test-output/timeline.txt"
    Write-Utf8Text -Path (Join-Path $concurrentRoot "scripts/locked.ps1") -Text @'
[void](New-Item -ItemType Directory -Path (Split-Path -Parent $env:OXVBA_CORE_GATE_TEST_TIMELINE) -Force)
$start = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
Add-Content -LiteralPath $env:OXVBA_CORE_GATE_TEST_TIMELINE -Value "start|$env:OXVBA_CORE_GATE_RUN_ID|$start" -Encoding utf8
Start-Sleep -Milliseconds 700
$finish = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
Add-Content -LiteralPath $env:OXVBA_CORE_GATE_TEST_TIMELINE -Value "end|$env:OXVBA_CORE_GATE_RUN_ID|$finish" -Encoding utf8
'@
    $timelineEnvironment = @([pscustomobject]@{ name = "OXVBA_CORE_GATE_TEST_TIMELINE"; action = "set"; value = $timeline })
    Write-TestManifest -Root $concurrentRoot -Manifest (New-TestManifest -Gates @((New-TestGate -Order 1 -Id "cargo-serialized" -Command "scripts/locked.ps1" -Environment $timelineEnvironment -CargoWorkspace $true -TimeoutSeconds 10)))
    Initialize-FixtureGit -Root $concurrentRoot
    $concurrentArgsA = @("-RepositoryRoot", $concurrentRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "concurrent-a")
    $concurrentArgsB = @("-RepositoryRoot", $concurrentRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "concurrent-b")
    $concurrentAHandle = New-RunnerProcess -Root $concurrentRoot -Arguments $concurrentArgsA
    $concurrentBHandle = New-RunnerProcess -Root $concurrentRoot -Arguments $concurrentArgsB
    $concurrentA = Complete-RunnerProcess -Handle $concurrentAHandle
    $concurrentB = Complete-RunnerProcess -Handle $concurrentBHandle
    Assert-Equal $concurrentA.exit_code 0 "first concurrent runner failed: $($concurrentA.stderr)"
    Assert-Equal $concurrentB.exit_code 0 "second concurrent runner failed: $($concurrentB.stderr)"
    $events = @(Get-Content -LiteralPath $timeline | ForEach-Object { $parts = $_ -split '\|'; [pscustomobject]@{ event = $parts[0]; run = $parts[1]; millis = [int64]$parts[2] } })
    Assert-Equal $events.Count 4 "Cargo serialization timeline is incomplete"
    $aStart = ($events | Where-Object { $_.run -eq "concurrent-a" -and $_.event -eq "start" }).millis
    $aEnd = ($events | Where-Object { $_.run -eq "concurrent-a" -and $_.event -eq "end" }).millis
    $bStart = ($events | Where-Object { $_.run -eq "concurrent-b" -and $_.event -eq "start" }).millis
    $bEnd = ($events | Where-Object { $_.run -eq "concurrent-b" -and $_.event -eq "end" }).millis
    Assert-True (($aEnd -le $bStart) -or ($bEnd -le $aStart)) "workspace Cargo gates overlapped"
    $concurrentARun = Get-RunManifest -Root $concurrentRoot -RunId "concurrent-a"
    $concurrentBRun = Get-RunManifest -Root $concurrentRoot -RunId "concurrent-b"
    $maximumWait = [Math]::Max([int64]$concurrentARun.results[0].cargo_lock_wait_ms, [int64]$concurrentBRun.results[0].cargo_lock_wait_ms)
    Assert-True ($maximumWait -ge 400) "concurrent runner evidence does not show serialized Cargo lock wait"
    Write-Host "core-profile-gates Cargo serialization: ok (max_wait_ms=$maximumWait)"

    }

    if ($Phase -eq "All") {
        $descendantCases = if ($IsLinux) { 3 } else { 2 }
        $sourceToolSealCases = if ($IsLinux) { 6 } else { 5 }
        Write-Host "test-core-profile-gates: ok (phase=All x64=1 exact-success=1 failures=1 timeouts=1 descendants=$descendantCases ambient-descendants=1 evidence-tamper=6 source-tool-seals=$sourceToolSealCases path-confinement=2 manifest-mutations=30 cargo-concurrency=2)"
    }
    else { Write-Host "test-core-profile-gates: ok (phase=$Phase)" }
}
finally {
    if (Test-Path -LiteralPath $tempBase -PathType Container) {
        $resolved = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $tempBase).Path)
        $comparison = if ($IsWindows) { [StringComparison]::OrdinalIgnoreCase } else { [StringComparison]::Ordinal }
        if (-not $resolved.StartsWith($systemTemp + [IO.Path]::DirectorySeparatorChar, $comparison) -or
            -not ([IO.Path]::GetFileName($resolved)).StartsWith("oxvba-core-profile-gates-", [StringComparison]::Ordinal)) {
            throw "refusing unsafe Core profile gate test cleanup: $resolved"
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
