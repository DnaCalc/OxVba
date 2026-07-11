param(
    [string]$RepositoryRoot = ""
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
$runner = Join-Path $repoRoot "scripts/run-core-profile-gates.ps1"
$pwsh = (Get-Command pwsh -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source

function Assert-True {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )

    if (-not $Condition) { throw "core profile gate test: $Message" }
}

function Assert-Equal {
    param(
        [AllowNull()]$Actual,
        [AllowNull()]$Expected,
        [Parameter(Mandatory = $true)][string]$Message
    )

    if ([string]$Actual -cne [string]$Expected) {
        throw "core profile gate test: $Message; expected '$Expected', found '$Actual'"
    }
}

function Assert-Matches {
    param(
        [AllowNull()][string]$Actual,
        [Parameter(Mandatory = $true)][string]$Pattern,
        [Parameter(Mandatory = $true)][string]$Message
    )

    if ([string]$Actual -notmatch $Pattern) {
        throw "core profile gate test: $Message; output was:`n$Actual"
    }
}

function Write-Utf8Text {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text
    )

    $parent = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        [void](New-Item -ItemType Directory -Path $parent -Force)
    }
    [IO.File]::WriteAllText($Path, $Text, $utf8)
}

function Write-TestJson {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )

    Write-Utf8Text -Path $Path -Text ((ConvertTo-Json -InputObject $Value -Depth 30) + "`n")
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
        order = $Order
        id = $Id
        description = "Controlled test gate $Id"
        platforms = @($Platforms)
        kind = $Kind
        command = $Command
        arguments = @($Arguments)
        environment = @($Environment)
        timeout_seconds = $TimeoutSeconds
        cargo_workspace = $CargoWorkspace
        evidence_path = "commands/{0:D3}-{1}" -f $Order, $Id
    }
}

function New-TestManifest {
    param(
        [Parameter(Mandatory = $true)][object[]]$Gates,
        [int]$CargoLockSeconds = 10
    )

    return [pscustomobject][ordered]@{
        schema_id = "oxvba-core-profile-gate-plan-v1"
        plan_id = "core-profile-portable-gates-v1"
        version = 1
        profile = "core"
        supported_platforms = @("windows-x64", "linux-x64")
        evidence = [pscustomobject][ordered]@{
            no_artifact_root = "temp/no-artifacts/core-profile-gates"
            plan_path = "plan.json"
            run_manifest_path = "run-manifest.json"
            summary_path = "summary.txt"
        }
        cargo_lock = [pscustomobject][ordered]@{
            name_prefix = "oxvba-core-profile-cargo-v1"
            acquire_timeout_seconds = $CargoLockSeconds
        }
        gates = @($Gates)
    }
}

function New-FixtureRoot {
    param([Parameter(Mandatory = $true)][string]$Name)

    $root = Join-Path $tempBase $Name
    [void](New-Item -ItemType Directory -Path (Join-Path $root "scripts") -Force)
    [void](New-Item -ItemType Directory -Path (Join-Path $root "ci/core-profile") -Force)
    Write-Utf8Text -Path (Join-Path $root "scripts/pass.ps1") -Text @'
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
foreach ($name in @(
    "OXVBA_CORE_GATE_RUN_ID", "OXVBA_CORE_GATE_ID", "OXVBA_CORE_GATE_EVIDENCE_ROOT",
    "OXVBA_CORE_GATE_PLAN_PATH", "OXVBA_CORE_GATE_PLAN_SHA256", "OXVBA_CORE_GATE_MANIFEST_SHA256",
    "OXVBA_CORE_GATE_MANIFEST_PATH"
)) {
    $value = [Environment]::GetEnvironmentVariable($name)
    if ([string]::IsNullOrWhiteSpace($value)) { throw "missing propagated gate variable $name" }
}
if (-not (Test-Path -LiteralPath $env:OXVBA_CORE_GATE_PLAN_PATH -PathType Leaf)) {
    throw "propagated plan path is missing"
}
foreach ($name in @("OXVBA_BLESS_JIT_SCOPE", "OXVBA_BLESS_GOLDEN", "OXVBA_SNAPSHOT_UPDATE", "INSTA_UPDATE")) {
    if ($null -ne [Environment]::GetEnvironmentVariable($name)) {
        throw "hostile inherited environment reached child: $name"
    }
}
Write-Output "propagated=$env:OXVBA_CORE_GATE_RUN_ID/$env:OXVBA_CORE_GATE_ID"
'@
    return $root
}

function Write-TestManifest {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)]$Manifest
    )

    Write-TestJson -Path (Join-Path $Root "ci/core-profile/gates-v1.json") -Value $Manifest
}

function New-RunnerProcess {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [hashtable]$Environment = @{}
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $pwsh
    $startInfo.WorkingDirectory = $Root
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.StandardOutputEncoding = $utf8
    $startInfo.StandardErrorEncoding = $utf8
    foreach ($argument in @("-NoLogo", "-NoProfile", "-NonInteractive", "-File", $runner) + $Arguments) {
        [void]$startInfo.ArgumentList.Add([string]$argument)
    }
    foreach ($name in $Environment.Keys) {
        $startInfo.Environment[[string]$name] = [string]$Environment[$name]
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) { throw "could not start runner test process" }
    return [pscustomobject]@{
        process = $process
        stdout_task = $process.StandardOutput.ReadToEndAsync()
        stderr_task = $process.StandardError.ReadToEndAsync()
        stopwatch = [Diagnostics.Stopwatch]::StartNew()
    }
}

function Complete-RunnerProcess {
    param(
        [Parameter(Mandatory = $true)]$Handle,
        [int]$TimeoutSeconds = 30
    )

    if (-not $Handle.process.WaitForExit($TimeoutSeconds * 1000)) {
        try { $Handle.process.Kill($true) } catch {}
        try { $Handle.process.WaitForExit() } catch {}
        throw "core profile gate test: runner process exceeded the test bound of $TimeoutSeconds seconds"
    }
    $Handle.process.WaitForExit()
    $Handle.stopwatch.Stop()
    $result = [pscustomobject]@{
        exit_code = [int]$Handle.process.ExitCode
        stdout = [string]$Handle.stdout_task.GetAwaiter().GetResult()
        stderr = [string]$Handle.stderr_task.GetAwaiter().GetResult()
        duration_ms = [int64]$Handle.stopwatch.ElapsedMilliseconds
    }
    $Handle.process.Dispose()
    return $result
}

function Invoke-Runner {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [hashtable]$Environment = @{},
        [int]$TimeoutSeconds = 30
    )

    return Complete-RunnerProcess -Handle (New-RunnerProcess -Root $Root -Arguments $Arguments -Environment $Environment) -TimeoutSeconds $TimeoutSeconds
}

function Get-RunManifest {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$RunId
    )

    return Get-Content -LiteralPath (Join-Path $Root "temp/no-artifacts/core-profile-gates/$RunId/run-manifest.json") -Raw | ConvertFrom-Json -Depth 30
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
    if (-not (Test-Path -LiteralPath (Join-Path $root "ci/core-profile/gates-v1.json") -PathType Leaf)) {
        Write-TestManifest -Root $root -Manifest $manifest
    }
    $result = Invoke-Runner -Root $root -Arguments @(
        "-RepositoryRoot", $root, "-ManifestPath", "ci/core-profile/gates-v1.json",
        "-Mode", "ValidateManifest"
    )
    Assert-True ($result.exit_code -ne 0) "mutated manifest '$Name' unexpectedly passed"
    Assert-Matches "$($result.stdout)`n$($result.stderr)" $Pattern "mutated manifest '$Name' failed for the wrong reason"
    Write-Host "core-profile-gates mutation: ok ($Name)"
}

$systemTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
$tempBase = Join-Path $systemTemp ("oxvba-core-profile-gates-" + [guid]::NewGuid().ToString("N"))
[void](New-Item -ItemType Directory -Path $tempBase)

try {
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

    $manifestOnly = Invoke-Runner -Root $repoRoot -Arguments @(
        "-RepositoryRoot", $repoRoot, "-Mode", "ValidateManifest"
    )
    Assert-Equal $manifestOnly.exit_code 0 "canonical manifest validation failed"
    Assert-Matches $manifestOnly.stdout 'manifest ok' "manifest validation did not report its distinct mode"
    Write-Host "core-profile-gates deterministic plan: ok"

    $positiveRoot = New-FixtureRoot -Name "positive"
    $positiveManifest = New-TestManifest -Gates @(
        (New-TestGate -Order 1 -Id "propagation"),
        (New-TestGate -Order 2 -Id "serialized-pass" -CargoWorkspace $true)
    )
    Write-TestManifest -Root $positiveRoot -Manifest $positiveManifest
    $positiveValidation = Invoke-Runner -Root $positiveRoot -Arguments @(
        "-RepositoryRoot", $positiveRoot, "-ManifestPath", "ci/core-profile/gates-v1.json",
        "-Mode", "ValidateManifest"
    )
    Assert-Equal $positiveValidation.exit_code 0 "test manifest validation failed"
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $positiveRoot "temp/no-artifacts"))) "manifest validation wrote no-artifact evidence"
    $positive = Invoke-Runner -Root $positiveRoot -Arguments @(
        "-RepositoryRoot", $positiveRoot, "-ManifestPath", "ci/core-profile/gates-v1.json",
        "-Mode", "NoArtifacts", "-RunId", "positive"
    ) -Environment @{
        OXVBA_BLESS_JIT_SCOPE = "1"
        OXVBA_BLESS_GOLDEN = "1"
        OXVBA_SNAPSHOT_UPDATE = "1"
        INSTA_UPDATE = "always"
    }
    Assert-Equal $positive.exit_code 0 "positive no-artifact run failed; stdout=$($positive.stdout); stderr=$($positive.stderr)"
    $positiveRun = Get-RunManifest -Root $positiveRoot -RunId "positive"
    Assert-Equal $positiveRun.status "passed" "positive run manifest did not pass"
    Assert-Equal @($positiveRun.results).Count 2 "positive run result count drifted"
    $propagationOutput = Get-Content -LiteralPath (Join-Path $positiveRoot "temp/no-artifacts/core-profile-gates/positive/commands/001-propagation/stdout.log") -Raw
    Assert-Matches $propagationOutput 'propagated=positive/propagation' "evidence/plan environment was not propagated to the command"
    foreach ($relative in @("plan.json", "run-manifest.json", "summary.txt")) {
        Assert-True (Test-Path -LiteralPath (Join-Path $positiveRoot "temp/no-artifacts/core-profile-gates/positive/$relative") -PathType Leaf) "positive evidence lacks $relative"
    }
    $stale = Invoke-Runner -Root $positiveRoot -Arguments @(
        "-RepositoryRoot", $positiveRoot, "-ManifestPath", "ci/core-profile/gates-v1.json",
        "-Mode", "NoArtifacts", "-RunId", "positive"
    )
    Assert-True ($stale.exit_code -ne 0) "pre-existing evidence root was silently reused"
    Assert-Matches "$($stale.stdout)`n$($stale.stderr)" 'refusing stale evidence' "stale evidence root failed for the wrong reason"
    Write-Host "core-profile-gates evidence propagation: ok"

    $failureRoot = New-FixtureRoot -Name "failure"
    Write-Utf8Text -Path (Join-Path $failureRoot "scripts/fail.ps1") -Text "Write-Error 'controlled failure'`nexit 7`n"
    $failureManifest = New-TestManifest -Gates @(
        (New-TestGate -Order 1 -Id "controlled-failure" -Command "scripts/fail.ps1"),
        (New-TestGate -Order 2 -Id "must-not-run")
    )
    Write-TestManifest -Root $failureRoot -Manifest $failureManifest
    $failure = Invoke-Runner -Root $failureRoot -Arguments @(
        "-RepositoryRoot", $failureRoot, "-ManifestPath", "ci/core-profile/gates-v1.json",
        "-Mode", "NoArtifacts", "-RunId", "failure"
    )
    Assert-True ($failure.exit_code -ne 0) "nonzero command was not propagated"
    $failureRun = Get-RunManifest -Root $failureRoot -RunId "failure"
    Assert-Equal $failureRun.status "failed" "failure run manifest did not fail"
    Assert-Equal $failureRun.results[0].exit_code 7 "command exit code was not preserved"
    Assert-Equal $failureRun.results[1].status "not-run" "later command ran after failure"
    Write-Host "core-profile-gates command failure: ok"

    $timeoutRoot = New-FixtureRoot -Name "timeout"
    Write-Utf8Text -Path (Join-Path $timeoutRoot "scripts/sleep.ps1") -Text "Start-Sleep -Seconds 5`nWrite-Output 'late'`n"
    $timeoutManifest = New-TestManifest -Gates @(
        (New-TestGate -Order 1 -Id "controlled-timeout" -Command "scripts/sleep.ps1" -TimeoutSeconds 1)
    )
    Write-TestManifest -Root $timeoutRoot -Manifest $timeoutManifest
    $timeout = Invoke-Runner -Root $timeoutRoot -Arguments @(
        "-RepositoryRoot", $timeoutRoot, "-ManifestPath", "ci/core-profile/gates-v1.json",
        "-Mode", "NoArtifacts", "-RunId", "timeout"
    ) -TimeoutSeconds 12
    Assert-True ($timeout.exit_code -ne 0) "timed-out command was not propagated"
    Assert-True ($timeout.duration_ms -lt 8000) "command timeout did not bound execution"
    $timeoutRun = Get-RunManifest -Root $timeoutRoot -RunId "timeout"
    Assert-Equal $timeoutRun.results[0].status "timeout" "timeout status was not preserved"
    Write-Host "core-profile-gates timeout: ok"

    $tamperRoot = New-FixtureRoot -Name "evidence-tamper"
    Write-Utf8Text -Path (Join-Path $tamperRoot "scripts/tamper.ps1") -Text @'
[IO.File]::WriteAllText($env:OXVBA_CORE_GATE_PLAN_PATH, "{", [Text.UTF8Encoding]::new($false))
Write-Output "tampered"
'@
    $tamperManifest = New-TestManifest -Gates @(
        (New-TestGate -Order 1 -Id "evidence-tamper" -Command "scripts/tamper.ps1")
    )
    Write-TestManifest -Root $tamperRoot -Manifest $tamperManifest
    $tamper = Invoke-Runner -Root $tamperRoot -Arguments @(
        "-RepositoryRoot", $tamperRoot, "-ManifestPath", "ci/core-profile/gates-v1.json",
        "-Mode", "NoArtifacts", "-RunId", "tamper"
    )
    Assert-True ($tamper.exit_code -ne 0) "malformed evidence was accepted"
    $tamperRun = Get-RunManifest -Root $tamperRoot -RunId "tamper"
    Assert-Equal $tamperRun.status "failed" "evidence tamper did not fail the run manifest"
    Assert-Matches $tamperRun.failure 'evidence validation failed' "evidence tamper failure was not explicit"
    Write-Host "core-profile-gates malformed evidence: ok"

    $manifestTamperRoot = New-FixtureRoot -Name "manifest-tamper"
    Write-Utf8Text -Path (Join-Path $manifestTamperRoot "scripts/tamper-manifest.ps1") -Text @'
[IO.File]::WriteAllText($env:OXVBA_CORE_GATE_MANIFEST_PATH, "{}", [Text.UTF8Encoding]::new($false))
Write-Output "manifest-tampered"
'@
    $manifestTamperManifest = New-TestManifest -Gates @(
        (New-TestGate -Order 1 -Id "manifest-tamper" -Command "scripts/tamper-manifest.ps1")
    )
    Write-TestManifest -Root $manifestTamperRoot -Manifest $manifestTamperManifest
    $manifestTamper = Invoke-Runner -Root $manifestTamperRoot -Arguments @(
        "-RepositoryRoot", $manifestTamperRoot, "-ManifestPath", "ci/core-profile/gates-v1.json",
        "-Mode", "NoArtifacts", "-RunId", "manifest-tamper"
    )
    Assert-True ($manifestTamper.exit_code -ne 0) "mid-run versioned manifest replacement was accepted"
    $manifestTamperRun = Get-RunManifest -Root $manifestTamperRoot -RunId "manifest-tamper"
    Assert-Equal $manifestTamperRun.status "failed" "manifest replacement did not fail the run manifest"
    Assert-Matches $manifestTamperRun.failure 'versioned manifest|manifest\.' "manifest replacement failure was not explicit"
    Write-Host "core-profile-gates live manifest binding: ok"

    $missingToolRoot = New-FixtureRoot -Name "missing-tool"
    $missingToolManifest = New-TestManifest -Gates @(
        (New-TestGate -Order 1 -Id "cargo-missing" -Kind "cargo" -Command "cargo" -Arguments @("--version") -CargoWorkspace $true)
    )
    Write-TestManifest -Root $missingToolRoot -Manifest $missingToolManifest
    $emptyPath = Join-Path $missingToolRoot "empty-path"
    [void](New-Item -ItemType Directory -Path $emptyPath)
    $missingTool = Invoke-Runner -Root $missingToolRoot -Arguments @(
        "-RepositoryRoot", $missingToolRoot, "-ManifestPath", "ci/core-profile/gates-v1.json",
        "-Mode", "NoArtifacts", "-RunId", "missing-tool"
    ) -Environment @{ PATH = $emptyPath }
    Assert-True ($missingTool.exit_code -ne 0) "missing Cargo tool was accepted"
    $missingToolRun = Get-RunManifest -Root $missingToolRoot -RunId "missing-tool"
    Assert-Matches $missingToolRun.failure "required tool 'cargo' is unavailable" "missing tool failure was not explicit"
    Write-Host "core-profile-gates missing tool: ok"

    Assert-ManifestFailure -Name "unknown-root-key" -Pattern 'properties must be exactly|unexpected or mis-cased' -Mutate {
        param($manifest, $root)
        $manifest | Add-Member -NotePropertyName extra -NotePropertyValue "forbidden"
    }
    Assert-ManifestFailure -Name "string-version" -Pattern 'version must be integer 1' -Mutate {
        param($manifest, $root)
        $manifest.version = "1"
    }
    Assert-ManifestFailure -Name "unsupported-platform-set" -Pattern 'supported_platforms must be exactly' -Mutate {
        param($manifest, $root)
        $manifest.supported_platforms = @("windows-x64")
    }
    Assert-ManifestFailure -Name "scalar-supported-platforms" -Pattern 'supported_platforms must be a JSON array' -Mutate {
        param($manifest, $root)
        $manifest.supported_platforms = "windows-x64"
    }
    Assert-ManifestFailure -Name "scalar-gates" -Pattern 'manifest.gates must be a JSON array' -Mutate {
        param($manifest, $root)
        $manifest.gates = $manifest.gates[0]
    }
    Assert-ManifestFailure -Name "order-gap" -Pattern 'contiguous integer 1' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].order = 2
    }
    Assert-ManifestFailure -Name "unknown-gate-platform" -Pattern 'unknown or duplicate platform' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].platforms = @("macos-x64")
    }
    Assert-ManifestFailure -Name "scalar-gate-platforms" -Pattern 'gates\[0\]\.platforms must be a JSON array' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].platforms = "windows-x64"
    }
    Assert-ManifestFailure -Name "scalar-gate-arguments" -Pattern 'gates\[0\]\.arguments must be a JSON array' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].arguments = "-NoArtifacts"
    }
    Assert-ManifestFailure -Name "scalar-gate-environment" -Pattern 'gates\[0\]\.environment must be a JSON array' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].environment = [pscustomobject]@{ name = "RUST_BACKTRACE"; action = "set"; value = "1" }
    }
    Assert-ManifestFailure -Name "missing-command" -Pattern 'command is missing' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].command = "scripts/missing.ps1"
    }
    Assert-ManifestFailure -Name "path-escape" -Pattern 'scripts/\*\.ps1 path|repository-relative path' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].command = "../outside.ps1"
    }
    Assert-ManifestFailure -Name "zero-timeout" -Pattern 'timeout_seconds must be an integer' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].timeout_seconds = 0
    }
    Assert-ManifestFailure -Name "null-cargo-boolean" -Pattern 'cargo_workspace must be a JSON boolean' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].cargo_workspace = $null
    }
    Assert-ManifestFailure -Name "unlocked-cargo-command" -Pattern 'must mark every cargo command as cargo_workspace=true' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].kind = "cargo"
        $manifest.gates[0].command = "cargo"
        $manifest.gates[0].cargo_workspace = $false
    }
    Assert-ManifestFailure -Name "wrong-evidence-path" -Pattern 'evidence_path must be the unique exact path' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].evidence_path = "../outside"
    }
    Assert-ManifestFailure -Name "forbidden-environment" -Pattern 'not an allowed unique gate variable' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].environment = @([pscustomobject]@{ name = "PATH"; action = "set"; value = "mutable" })
    }
    Assert-ManifestFailure -Name "forbidden-snapshot-mutation" -Pattern 'forbidden snapshot mutation surface' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].arguments = @("--bless")
    }
    Assert-ManifestFailure -Name "duplicate-gate-id" -Pattern 'unique lowercase kebab-case identity' -Mutate {
        param($manifest, $root)
        $manifest.gates = @(
            (New-TestGate -Order 1 -Id "duplicate"),
            (New-TestGate -Order 2 -Id "duplicate")
        )
    }
    Assert-ManifestFailure -Name "no-linux-lane" -Pattern 'no explicit gate lane for linux-x64' -Mutate {
        param($manifest, $root)
        $manifest.gates[0].platforms = @("windows-x64")
    }
    Assert-ManifestFailure -Name "duplicate-json-key" -Pattern "duplicate JSON property 'plan_id'" -Mutate {
        param($manifest, $root)
        Write-TestManifest -Root $root -Manifest $manifest
        $path = Join-Path $root "ci/core-profile/gates-v1.json"
        $text = [IO.File]::ReadAllText($path, $utf8)
        $text = $text.Replace('  "plan_id":', "  `"plan_id`": `"duplicate`",`n  `"plan_id`":", [StringComparison]::Ordinal)
        Write-Utf8Text -Path $path -Text $text
    }
    Assert-ManifestFailure -Name "bare-carriage-return" -Pattern 'bare carriage return' -Mutate {
        param($manifest, $root)
        Write-TestManifest -Root $root -Manifest $manifest
        $path = Join-Path $root "ci/core-profile/gates-v1.json"
        [byte[]]$bytes = [IO.File]::ReadAllBytes($path)
        [byte[]]$combined = [byte[]]::new($bytes.Length + 1)
        [Array]::Copy($bytes, $combined, $bytes.Length)
        $combined[$bytes.Length] = 13
        [IO.File]::WriteAllBytes($path, $combined)
    }

    $concurrentRoot = New-FixtureRoot -Name "cargo-concurrency"
    $timeline = Join-Path $concurrentRoot "timeline.txt"
    Write-Utf8Text -Path (Join-Path $concurrentRoot "scripts/locked.ps1") -Text @'
$start = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
Add-Content -LiteralPath $env:OXVBA_CORE_GATE_TEST_TIMELINE -Value "start|$env:OXVBA_CORE_GATE_RUN_ID|$start" -Encoding utf8
Start-Sleep -Milliseconds 700
$finish = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
Add-Content -LiteralPath $env:OXVBA_CORE_GATE_TEST_TIMELINE -Value "end|$env:OXVBA_CORE_GATE_RUN_ID|$finish" -Encoding utf8
'@
    $timelineEnvironment = @([pscustomobject]@{
            name = "OXVBA_CORE_GATE_TEST_TIMELINE"
            action = "set"
            value = $timeline
        })
    $concurrentManifest = New-TestManifest -Gates @(
        (New-TestGate -Order 1 -Id "cargo-serialized" -Command "scripts/locked.ps1" `
            -Environment $timelineEnvironment -CargoWorkspace $true -TimeoutSeconds 10)
    )
    Write-TestManifest -Root $concurrentRoot -Manifest $concurrentManifest
    $concurrentArgsA = @(
        "-RepositoryRoot", $concurrentRoot, "-ManifestPath", "ci/core-profile/gates-v1.json",
        "-Mode", "NoArtifacts", "-RunId", "concurrent-a"
    )
    $concurrentArgsB = @(
        "-RepositoryRoot", $concurrentRoot, "-ManifestPath", "ci/core-profile/gates-v1.json",
        "-Mode", "NoArtifacts", "-RunId", "concurrent-b"
    )
    $concurrentAHandle = New-RunnerProcess -Root $concurrentRoot -Arguments $concurrentArgsA
    $concurrentBHandle = New-RunnerProcess -Root $concurrentRoot -Arguments $concurrentArgsB
    $concurrentA = Complete-RunnerProcess -Handle $concurrentAHandle -TimeoutSeconds 30
    $concurrentB = Complete-RunnerProcess -Handle $concurrentBHandle -TimeoutSeconds 30
    Assert-Equal $concurrentA.exit_code 0 "first concurrent runner failed"
    Assert-Equal $concurrentB.exit_code 0 "second concurrent runner failed"
    $events = @(Get-Content -LiteralPath $timeline | ForEach-Object {
            $parts = $_ -split '\|'
            [pscustomobject]@{ event = $parts[0]; run = $parts[1]; millis = [int64]$parts[2] }
        })
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

    Write-Host "test-core-profile-gates: ok (deterministic-list=2 deterministic-dry-run=2 hostile-parent-env=4 positive-runs=1 failures=1 timeouts=1 evidence-tamper=1 manifest-tamper=1 missing-tool=1 manifest-mutations=22 cargo-concurrency=2)"
}
finally {
    if (Test-Path -LiteralPath $tempBase -PathType Container) {
        $resolved = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $tempBase).Path)
        if (-not $resolved.StartsWith($systemTemp + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
            -not ([IO.Path]::GetFileName($resolved)).StartsWith("oxvba-core-profile-gates-", [StringComparison]::Ordinal)) {
            throw "refusing unsafe Core profile gate test cleanup: $resolved"
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
