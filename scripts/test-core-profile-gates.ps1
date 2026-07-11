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
    "job-object-v1:suspended-assign-resume;kill-on-close;owned-file-stdout-stderr"
}
else {
    "setsid-process-group-v1:term-kill;owned-file-stdout-stderr"
}

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
    param([Parameter(Mandatory = $true)][object[]]$Gates, [int]$CargoLockSeconds = 10)
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
            native_source_path = "scripts/core-gate-process-supervisor.cs"
            windows_transport = "job-object-v1:suspended-assign-resume;kill-on-close;owned-file-stdout-stderr"
            linux_launcher_path = "/usr/bin/setsid"
            linux_supervisor_path = "scripts/core-gate-linux-supervisor.sh"
            linux_transport = "setsid-process-group-v1:term-kill;owned-file-stdout-stderr"
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
    param([Parameter(Mandatory = $true)]$Handle, [int]$TimeoutSeconds = 40)
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
        [int]$TimeoutSeconds = 40
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
    Assert-Equal $positiveRun.source.status "clean" "source identity is not clean"
    Assert-Matches $positiveRun.source.head '^[0-9a-f]{40,64}$' "source HEAD is not recorded"
    Assert-Matches $positiveRun.source.tree '^[0-9a-f]{40,64}$' "source tree is not recorded"
    Assert-True (@($positiveRun.tools).Count -ge 3) "tool identities are incomplete"
    Assert-True (@($positiveRun.commands).Count -ge 5) "command/source identities are incomplete"
    Assert-Equal $positiveRun.supervision.transport $expectedTransport "run supervision transport drifted"
    foreach ($command in @($positivePlan.commands)) { Assert-Matches $command.command_digest '^[0-9a-f]{64}$' "command digest is absent" }
    foreach ($resultRow in @($positiveRun.results | Where-Object { $_.status -eq "passed" })) {
        Assert-Equal $resultRow.exit_code 0 "selected result exit code drifted"
        Assert-Equal $resultRow.tree_cleanup "complete" "selected result tree was not empty"
        Assert-Equal $resultRow.transport $expectedTransport "selected result transport drifted"
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
    $timeout = Invoke-Runner -Root $timeoutRoot -Arguments @("-RepositoryRoot", $timeoutRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "timeout") -TimeoutSeconds 15
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
    Write-TestManifest -Root $descendantRoot -Manifest (New-TestManifest -Gates @((New-TestGate -Order 1 -Id "descendant" -Command "scripts/spawn-grandchild.ps1" -Environment $pidEnvironment -TimeoutSeconds 5)))
    Initialize-FixtureGit -Root $descendantRoot
    $descendant = Invoke-Runner -Root $descendantRoot -Arguments @("-RepositoryRoot", $descendantRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "descendant") -TimeoutSeconds 40
    Assert-NoSuccessOutput -Result $descendant -Owner "direct exit with descendant"
    $descendantRun = Get-RunManifest -Root $descendantRoot -RunId "descendant"
    Assert-Equal $descendantRun.results[0].status "failed" "descendant result did not fail"
    Assert-Equal $descendantRun.results[0].reason "descendant-processes-remained-after-direct-exit" "descendant failure reason drifted"
    Assert-Equal $descendantRun.results[0].tree_cleanup "complete" "descendant tree did not empty"
    Assert-Equal $descendantRun.results[0].transport $expectedTransport "descendant transport was not recorded"
    Assert-True ([int64]$descendantRun.results[0].duration_ms -le 5500) "descendant cleanup exceeded its owned-tree deadline"
    Assert-True (Test-Path -LiteralPath $pidPath -PathType Leaf) "grandchild pid was not recorded"
    $grandchildPid = [int][IO.File]::ReadAllText($pidPath, $utf8)
    $grandchildGone = $false
    for ($attempt = 0; $attempt -lt 40; $attempt++) {
        try { [void][Diagnostics.Process]::GetProcessById($grandchildPid); Start-Sleep -Milliseconds 50 }
        catch [ArgumentException] { $grandchildGone = $true; break }
    }
    Assert-True $grandchildGone "owned grandchild process remained after runner completion"
    Write-Host "core-profile-gates complete process-tree ownership: ok"

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
    Write-Utf8Text -Path (Join-Path $manifestDriftRoot "scripts/manifest-drift.ps1") -Text '[IO.File]::AppendAllText($env:OXVBA_CORE_GATE_MANIFEST_PATH, " ", [Text.UTF8Encoding]::new($false))'
    Write-TestManifest -Root $manifestDriftRoot -Manifest (New-TestManifest -Gates @((New-TestGate -Order 1 -Id "manifest-drift" -Command "scripts/manifest-drift.ps1")))
    Initialize-FixtureGit -Root $manifestDriftRoot
    $manifestDrift = Invoke-Runner -Root $manifestDriftRoot -Arguments @("-RepositoryRoot", $manifestDriftRoot, "-ManifestPath", "ci/core-profile/gates-v1.json", "-Mode", "NoArtifacts", "-RunId", "manifest-drift")
    Assert-NoSuccessOutput -Result $manifestDrift -Owner "mid-run manifest drift"
    Assert-Matches (Get-RunManifest -Root $manifestDriftRoot -RunId "manifest-drift").failure 'source checkout must be clean|versioned manifest changed|identity changed' "manifest drift was not explicit"
    Write-Host "core-profile-gates committed source/command/manifest seal: ok"

    }
    if ($Phase -in @("All", "Extended")) {

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
        Write-Host "test-core-profile-gates: ok (phase=All x64=1 exact-success=1 failures=1 timeouts=1 descendants=1 evidence-tamper=6 source-tool-seals=5 reparse=1 manifest-mutations=25 cargo-concurrency=2)"
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
