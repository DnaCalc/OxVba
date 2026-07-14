Set-StrictMode -Version Latest

$script:IdealEnvironmentManifestPath = "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv"
$script:IdealWindowsFixtureManifestPath = "docs/validation/IDEAL_WINDOWS_X64_FIXTURE_MANIFEST_V1.csv"
$script:IdealEnvironmentCaptureSchema = "oxvba-windows-x64-environment-capture-v1"
$script:IdealCertificationEnvironmentPlanSchema = "oxvba-windows-x64-certification-environment-plan-v1"
$script:IdealCertificationEnvironmentPlanSealSchema = "oxvba-windows-x64-certification-environment-plan-seal-v1"

function ConvertTo-IdealCaptureCanonicalJson {
    param([Parameter(Mandatory = $true)]$Value)

    return ($Value | ConvertTo-Json -Depth 32 -Compress)
}

function Get-IdealCaptureSha256Text {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)

    return Get-WindowsFixtureSha256Text -Text $Text
}

function Get-IdealCaptureObjectHash {
    param([Parameter(Mandatory = $true)]$Value)

    return Get-IdealCaptureSha256Text -Text (ConvertTo-IdealCaptureCanonicalJson -Value $Value)
}

function Get-IdealCaptureRegistryValues {
    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey(
        [Microsoft.Win32.RegistryHive]::LocalMachine,
        [Microsoft.Win32.RegistryView]::Registry64
    )
    try {
        $windowsKey = $base.OpenSubKey("SOFTWARE\Microsoft\Windows NT\CurrentVersion", $false)
        $officeKey = $base.OpenSubKey("SOFTWARE\Microsoft\Office\ClickToRun\Configuration", $false)
        $excelKey = $base.OpenSubKey("SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\excel.exe", $false)
        try {
            if ($null -eq $windowsKey -or $null -eq $officeKey -or $null -eq $excelKey) {
                throw "capture-ideal-environment: required 64-bit Windows/Office registry facts are unavailable"
            }
            return [ordered]@{
                windows = [ordered]@{
                    current_build = [string]$windowsKey.GetValue("CurrentBuildNumber", "")
                    update_build_revision = [int64]$windowsKey.GetValue("UBR", -1)
                    display_version = [string]$windowsKey.GetValue("DisplayVersion", "")
                    edition_id = [string]$windowsKey.GetValue("EditionID", "")
                }
                office = [ordered]@{
                    version_to_report = [string]$officeKey.GetValue("VersionToReport", "")
                    client_version_to_report = [string]$officeKey.GetValue("ClientVersionToReport", "")
                    platform = [string]$officeKey.GetValue("Platform", "")
                    client_culture = [string]$officeKey.GetValue("ClientCulture", "")
                    product_release_ids = [string]$officeKey.GetValue("ProductReleaseIds", "")
                    update_channel = [string]$officeKey.GetValue("UpdateChannel", "")
                    cdn_base_url = [string]$officeKey.GetValue("CDNBaseUrl", "")
                }
                excel = [ordered]@{
                    executable_path = [string]$excelKey.GetValue("", "")
                }
            }
        }
        finally {
            if ($null -ne $windowsKey) { $windowsKey.Dispose() }
            if ($null -ne $officeKey) { $officeKey.Dispose() }
            if ($null -ne $excelKey) { $excelKey.Dispose() }
        }
    }
    finally {
        $base.Dispose()
    }
}

function Get-IdealCapturePeMachine {
    param([Parameter(Mandatory = $true)][string]$Path)

    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
    try {
        $reader = [IO.BinaryReader]::new($stream, [Text.Encoding]::ASCII, $true)
        try {
            if ($stream.Length -lt 0x40) {
                throw "capture-ideal-environment: Excel image is too short to be PE"
            }
            $stream.Position = 0x3c
            $peOffset = $reader.ReadInt32()
            if ($peOffset -lt 0 -or [int64]$peOffset + 6 -gt $stream.Length) {
                throw "capture-ideal-environment: Excel image has an invalid PE offset"
            }
            $stream.Position = $peOffset
            if ($reader.ReadUInt32() -ne 0x00004550) {
                throw "capture-ideal-environment: Excel image lacks a PE signature"
            }
            $machine = $reader.ReadUInt16()
            switch ($machine) {
                0x8664 { return "AMD64" }
                default { return "0x$($machine.ToString('x4'))" }
            }
        }
        finally {
            $reader.Dispose()
        }
    }
    finally {
        $stream.Dispose()
    }
}

function Get-IdealCaptureOfficeChannel {
    param([Parameter(Mandatory = $true)][string]$UpdateChannel)

    $guid = ($UpdateChannel.TrimEnd('/') -split '/')[-1].ToLowerInvariant()
    switch ($guid) {
        "492350f6-3a01-4f97-b9c0-c7c6ddf67d60" { return "Current Channel" }
        default { throw "capture-ideal-environment: unsupported Office update-channel identity '$guid'" }
    }
}

function Invoke-IdealCaptureNativeRead {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [string[]]$Arguments = @(),
        [ValidateRange(100, 120000)][int]$TimeoutMilliseconds = 10000
    )

    try {
        $applications = @(Get-Command -Name $Command -CommandType Application -ErrorAction Stop)
    }
    catch {
        throw "capture-ideal-environment: read-only observation executable '$Command' does not resolve"
    }
    if ($applications.Count -eq 0) {
        throw "capture-ideal-environment: read-only observation executable '$Command' does not resolve"
    }
    $commandPath = [string]$applications[0].Source
    if (-not [IO.Path]::IsPathRooted($commandPath) -or -not (Test-Path -LiteralPath $commandPath -PathType Leaf)) {
        throw "capture-ideal-environment: read-only observation executable '$Command' did not resolve to a file"
    }
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $commandPath
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $Arguments) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $started = $false
    $ownedPid = 0
    try {
        if (-not $process.Start()) {
            throw "capture-ideal-environment: failed to start read-only observation '$commandPath'"
        }
        $started = $true
        $ownedPid = $process.Id
        $standardOutputTask = $process.StandardOutput.ReadToEndAsync()
        $standardErrorTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutMilliseconds)) {
            $process.Kill($true)
            if (-not $process.WaitForExit(5000)) {
                throw "capture-ideal-environment: timed-out owned observation PID $ownedPid could not be reaped"
            }
            throw "capture-ideal-environment: read-only observation '$commandPath' timed out after $TimeoutMilliseconds ms; owned PID $ownedPid was terminated and reaped"
        }
        $standardOutput = $standardOutputTask.GetAwaiter().GetResult()
        $standardError = $standardErrorTask.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0) {
            throw "capture-ideal-environment: read-only '$commandPath $($Arguments -join ' ')' failed with exit code $($process.ExitCode): $($standardError.Trim())"
        }
        return $standardOutput.Trim()
    }
    finally {
        $cleanupFailed = $false
        if ($started -and -not $process.HasExited) {
            try {
                $process.Kill($true)
                $cleanupFailed = -not $process.WaitForExit(5000)
            }
            catch {
                $cleanupFailed = $true
            }
        }
        $process.Dispose()
        if ($cleanupFailed) {
            throw "capture-ideal-environment: owned observation PID $ownedPid could not be balanced"
        }
    }
}

function Get-IdealCaptureExcelProcessIds {
    return @(
        Get-Process -Name EXCEL -ErrorAction SilentlyContinue |
            Sort-Object Id |
            ForEach-Object { [int64]$_.Id }
    )
}

function Get-IdealCaptureHostObservation {
    param([Parameter(Mandatory = $true)]$RegistryValues)

    $excelPath = [string]$RegistryValues.excel.executable_path
    if ([string]::IsNullOrWhiteSpace($excelPath) -or -not (Test-Path -LiteralPath $excelPath -PathType Leaf)) {
        throw "capture-ideal-environment: the registered 64-bit Excel executable does not resolve"
    }
    $versionInfo = (Get-Item -LiteralPath $excelPath).VersionInfo
    $officeBuild = [string]$versionInfo.ProductVersion
    if ($officeBuild -notmatch '^(?<major>[0-9]+)\.(?<minor>[0-9]+)\.') {
        throw "capture-ideal-environment: Excel product version '$officeBuild' is not versioned"
    }
    $officeVersion = "$($Matches.major).$($Matches.minor)"
    $build = [string]$RegistryValues.windows.current_build
    $ubr = [int64]$RegistryValues.windows.update_build_revision
    if ($build -notmatch '^[0-9]+$' -or $ubr -lt 0) {
        throw "capture-ideal-environment: Windows build facts are incomplete"
    }
    $peMachine = Get-IdealCapturePeMachine -Path $excelPath
    if ([string]$RegistryValues.office.platform -ne "x64" -or $peMachine -ne "AMD64") {
        throw "capture-ideal-environment: Office/Excel is not a proved x64 installation"
    }
    if ([string]$RegistryValues.office.version_to_report -ne $officeBuild -or
        [string]$RegistryValues.office.client_version_to_report -ne $officeBuild) {
        throw "capture-ideal-environment: Excel file and Click-to-Run versions disagree"
    }

    return [ordered]@{
        os = [ordered]@{
            build = "10.0.$build.$ubr"
            display_version = [string]$RegistryValues.windows.display_version
            edition_id = [string]$RegistryValues.windows.edition_id
        }
        office = [ordered]@{
            product = "Microsoft Excel"
            version = $officeVersion
            build = $officeBuild
            channel = Get-IdealCaptureOfficeChannel -UpdateChannel ([string]$RegistryValues.office.update_channel)
            channel_identity = ([string]$RegistryValues.office.update_channel).TrimEnd('/').Split('/')[-1].ToLowerInvariant()
            bitness = "64"
            excel_pe_machine = $peMachine
            client_culture = [string]$RegistryValues.office.client_culture
            product_release_ids = [string]$RegistryValues.office.product_release_ids
        }
        locale = [ordered]@{
            current_culture = (Get-Culture).Name
            current_ui_culture = (Get-UICulture).Name
            system_locale = (Get-WinSystemLocale).Name
            ansi_codepage = [int64][Globalization.CultureInfo]::CurrentCulture.TextInfo.ANSICodePage
            oem_codepage = [int64][Globalization.CultureInfo]::CurrentCulture.TextInfo.OEMCodePage
            console_input_codepage = [int64][Console]::InputEncoding.CodePage
            console_output_codepage = [int64][Console]::OutputEncoding.CodePage
        }
        toolchain = [ordered]@{
            rustc_verbose = Invoke-IdealCaptureNativeRead -Command "rustc" -Arguments @("-Vv")
            cargo_version = Invoke-IdealCaptureNativeRead -Command "cargo" -Arguments @("-V")
            rustup_active_toolchain = Invoke-IdealCaptureNativeRead -Command "rustup" -Arguments @("show", "active-toolchain")
        }
    }
}

function Get-IdealCaptureFixtureFacts {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$EnvironmentId,
        [string]$FixtureManifestPath = $script:IdealWindowsFixtureManifestPath
    )

    $absolutePath = [IO.Path]::GetFullPath((Join-Path $RepositoryRoot $FixtureManifestPath))
    if (-not (Test-Path -LiteralPath $absolutePath -PathType Leaf)) {
        throw "capture-ideal-environment: canonical fixture manifest '$FixtureManifestPath' does not resolve"
    }
    $rows = @(Import-Csv -LiteralPath $absolutePath)
    if ($rows.Count -eq 0) {
        throw "capture-ideal-environment: canonical fixture manifest is empty"
    }
    $environmentRows = @($rows | Where-Object environment_id -ceq $EnvironmentId)
    if ($environmentRows.Count -eq 0) {
        throw "capture-ideal-environment: no fixture row binds environment '$EnvironmentId'"
    }
    $environmentRecords = @(
        $environmentRows |
            ForEach-Object {
                ConvertTo-IdealCaptureLengthPrefixedRecord -Fields ([ordered]@{
                    matrix_id = [string]$_.matrix_id
                    row_id = [string]$_.row_id
                    environment_capture_root = [string]$_.environment_capture_root
                    environment_capture_name = [string]$_.environment_capture_name
                    environment_capture_schema = [string]$_.environment_capture_schema
                })
            }
    )
    return [ordered]@{
        manifest_path = $FixtureManifestPath
        row_count = [int64]$rows.Count
        controlled_artifact_root_contract_sha256 = Get-IdealCaptureFixtureRootContractHash -Rows $rows
        environment_row_count = [int64]$environmentRows.Count
        environment_capture_root_contract_sha256 = Get-IdealCaptureRecordSetHash -Schema "oxvba-environment-capture-root-contract-v1" -Records $environmentRecords
    }
}

function Get-IdealCaptureFixtureRootContractHash {
    param([Parameter(Mandatory = $true)][object[]]$Rows)

    $records = @(
        $Rows |
            ForEach-Object {
                ConvertTo-IdealCaptureLengthPrefixedRecord -Fields ([ordered]@{
                    matrix_id = [string]$_.matrix_id
                    row_id = [string]$_.row_id
                    fixture_id = [string]$_.fixture_id
                    built_artifact_root = [string]$_.built_artifact_root
                    built_artifact_name = [string]$_.built_artifact_name
                    built_artifact_class = [string]$_.built_artifact_class
                })
            }
    )
    return Get-IdealCaptureRecordSetHash -Schema "oxvba-controlled-artifact-root-contract-v1" -Records $records
}

function ConvertTo-IdealCaptureLengthPrefixedRecord {
    param([Parameter(Mandatory = $true)][Collections.IDictionary]$Fields)

    $builder = [Text.StringBuilder]::new()
    foreach ($name in $Fields.Keys) {
        $value = ([string]$Fields[$name]).Replace("`r`n", "`n").Replace("`r", "`n")
        [void]$builder.Append($name).Append(':').Append($value.Length).Append(':').Append($value).Append("`n")
    }
    return $builder.ToString()
}

function Get-IdealCaptureRecordSetHash {
    param(
        [Parameter(Mandatory = $true)][string]$Schema,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Records
    )

    $orderedRecords = [string[]]@($Records)
    [Array]::Sort($orderedRecords, [StringComparer]::Ordinal)
    $builder = [Text.StringBuilder]::new()
    [void]$builder.Append($Schema).Append("`n")
    foreach ($record in $orderedRecords) {
        [void]$builder.Append("record:").Append($record.Length).Append(':').Append($record).Append("`n")
    }
    return Get-IdealCaptureSha256Text -Text $builder.ToString()
}

function Get-IdealCaptureEnvironmentRow {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$EnvironmentId,
        [string]$EnvironmentManifestPath = $script:IdealEnvironmentManifestPath
    )

    $path = [IO.Path]::GetFullPath((Join-Path $RepositoryRoot $EnvironmentManifestPath))
    $matches = @(Import-Csv -LiteralPath $path | Where-Object environment_id -ceq $EnvironmentId)
    if ($matches.Count -ne 1) {
        throw "capture-ideal-environment: expected one canonical environment '$EnvironmentId', found $($matches.Count)"
    }
    return $matches[0]
}

function Assert-IdealCaptureCertificationCaseContract {
    param(
        [Parameter(Mandatory = $true)]$Case,
        [Parameter(Mandatory = $true)]$Environment,
        [Parameter(Mandatory = $true)]$FixtureRow,
        [Parameter(Mandatory = $true)]$Contract,
        [string]$Owner = "certification case"
    )

    $environmentId = [string]$Environment.environment_id
    if ([string]$Environment.role -ne "certification-vm" -or `
        [string]$Case.fixture.source_environment_id -cne $environmentId -or `
        [string]$Case.environment_gate.environment_id -cne $environmentId -or `
        [string]$Case.environment_gate.current_evidence_state -cne [string]$Environment.evidence_state -or `
        [string]$Case.environment_gate.owner_bead -cne [string]$Environment.owner_bead -or `
        [string]$Case.environment_gate.required_evidence_state -ne "verified" -or `
        [string]$Case.execution.target_arch -ne "x64" -or `
        [string]$Case.execution.office_bitness -ne "64") {
        throw "$Owner is not bound to its canonical x64/Office64 certification environment"
    }
    if ([string]$FixtureRow.matrix_id -cne [string]$Case.matrix_id -or `
        [string]$FixtureRow.row_id -cne [string]$Case.row_id -or `
        [string]$FixtureRow.environment_id -cne $environmentId -or `
        [string]$FixtureRow.environment_capture_root -cne [string]$Contract.Root -or `
        [string]$FixtureRow.environment_capture_name -cne [string]$Contract.Name -or `
        [string]$FixtureRow.environment_capture_schema -cne [string]$Contract.Schema) {
        throw "$Owner differs from the canonical fixture capture contract"
    }
    $expectedPath = "$($Contract.Root)/$($Contract.Name)"
    $artifacts = @($Case.artifacts | Where-Object kind -ceq "environment-capture")
    if ($artifacts.Count -ne 1 -or [string]$artifacts[0].path -cne $expectedPath) {
        throw "$Owner does not bind the exact controlled environment-capture path"
    }
    return $expectedPath
}

function ConvertTo-IdealCertificationEnvironmentPlanCanonicalValue {
    param([Parameter(Mandatory = $true)]$Plan)

    return [pscustomobject][ordered]@{
        schema_id = [string]$Plan.schema_id
        schema_version = [int64]$Plan.schema_version
        plan_id = [string]$Plan.plan_id
        case_id = [string]$Plan.case_id
        matrix_id = [string]$Plan.matrix_id
        row_id = [string]$Plan.row_id
        environment_id = [string]$Plan.environment_id
        role = [string]$Plan.role
        profile = [string]$Plan.profile
        target_arch = [string]$Plan.target_arch
        os_build = [string]$Plan.os_build
        office_product = [string]$Plan.office_product
        office_version = [string]$Plan.office_version
        office_build = [string]$Plan.office_build
        office_channel = [string]$Plan.office_channel
        office_bitness = [string]$Plan.office_bitness
        locale = [string]$Plan.locale
        default_locale = [string]$Plan.default_locale
        locale_policy = [string]$Plan.locale_policy
        ansi_codepage = [int64]$Plan.ansi_codepage
        oem_codepage = [int64]$Plan.oem_codepage
        snapshot_or_image = [string]$Plan.snapshot_or_image
        reset_policy = [string]$Plan.reset_policy
        fixture_manifest = [string]$Plan.fixture_manifest
        fixture_root_contract_sha256 = [string]$Plan.fixture_root_contract_sha256
        fixture_recipe_sha256 = [string]$Plan.fixture_recipe_sha256
        fixture_artifact_sha256 = [string]$Plan.fixture_artifact_sha256
        environment_capture_path = [string]$Plan.environment_capture_path
        environment_capture_schema = [string]$Plan.environment_capture_schema
        owned_process_policy = [string]$Plan.owned_process_policy
        uia_modal_policy = [string]$Plan.uia_modal_policy
        required_attestation_schema = [string]$Plan.required_attestation_schema
        environment_evidence_state = [string]$Plan.environment_evidence_state
        certification_authority = [bool]$Plan.certification_authority
        noncertifying = [bool]$Plan.noncertifying
    }
}

function Assert-IdealCertificationEnvironmentPlanValue {
    param(
        [Parameter(Mandatory = $true)]$Plan,
        [Parameter(Mandatory = $true)]$Environment,
        [Parameter(Mandatory = $true)]$Case,
        [Parameter(Mandatory = $true)]$FixtureRow,
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)]$FixtureFacts,
        [string]$Owner = "certification environment plan"
    )

    Assert-WindowsFixtureExactJsonProperties -Value $Plan -Expected @(
        "schema_id", "schema_version", "plan_id", "case_id", "matrix_id", "row_id",
        "environment_id", "role", "profile", "target_arch", "os_build", "office_product",
        "office_version", "office_build", "office_channel", "office_bitness", "locale",
        "default_locale", "locale_policy", "ansi_codepage", "oem_codepage", "snapshot_or_image", "reset_policy",
        "fixture_manifest", "fixture_root_contract_sha256", "fixture_recipe_sha256",
        "fixture_artifact_sha256", "environment_capture_path", "environment_capture_schema",
        "owned_process_policy", "uia_modal_policy", "required_attestation_schema",
        "environment_evidence_state", "certification_authority", "noncertifying"
    ) -Owner $Owner
    Assert-WindowsFixtureJsonStringProperties -Value $Plan -Properties @(
        "schema_id", "plan_id", "case_id", "matrix_id", "row_id", "environment_id", "role",
        "profile", "target_arch", "os_build", "office_product", "office_version", "office_build",
        "office_channel", "office_bitness", "locale", "default_locale", "locale_policy", "snapshot_or_image",
        "reset_policy", "fixture_manifest", "fixture_root_contract_sha256", "fixture_recipe_sha256",
        "fixture_artifact_sha256", "environment_capture_path", "environment_capture_schema",
        "owned_process_policy", "uia_modal_policy", "required_attestation_schema",
        "environment_evidence_state"
    ) -Owner $Owner
    if (-not (Test-WindowsFixtureJsonInteger -Value $Plan.schema_version) -or
        [int64]$Plan.schema_version -ne 1 -or
        -not (Test-WindowsFixtureJsonInteger -Value $Plan.ansi_codepage) -or
        [int64]$Plan.ansi_codepage -le 0 -or
        -not (Test-WindowsFixtureJsonInteger -Value $Plan.oem_codepage) -or
        [int64]$Plan.oem_codepage -le 0 -or
        $Plan.certification_authority -isnot [bool] -or
        $Plan.noncertifying -isnot [bool]) {
        throw "$Owner schema/version/codepage/authority field types are invalid"
    }
    foreach ($field in @("ansi_codepage", "oem_codepage")) {
        try {
            $encoding = [Text.Encoding]::GetEncoding([int]$Plan.$field)
            if ([int]$encoding.CodePage -ne [int]$Plan.$field) {
                throw "resolved codepage differs"
            }
        }
        catch {
            throw "$Owner $field is not a supported Windows code page"
        }
    }

    $environmentId = [string]$Environment.environment_id
    $expectedCapturePath = Assert-IdealCaptureCertificationCaseContract `
        -Case $Case `
        -Environment $Environment `
        -FixtureRow $FixtureRow `
        -Contract $Contract `
        -Owner $Owner
    $expected = [ordered]@{
        schema_id = $script:IdealCertificationEnvironmentPlanSchema
        plan_id = "$environmentId-certification-plan-v1"
        case_id = [string]$Case.case_id
        matrix_id = [string]$Case.matrix_id
        row_id = [string]$Case.row_id
        environment_id = $environmentId
        role = [string]$Environment.role
        profile = [string]$Environment.profile
        target_arch = [string]$Environment.target_arch
        os_build = [string]$Environment.os_build
        office_product = [string]$Environment.office_product
        office_version = [string]$Environment.office_version
        office_build = [string]$Environment.office_build
        office_channel = [string]$Environment.office_channel
        office_bitness = [string]$Environment.office_bitness
        locale = [string]$Environment.locale
        locale_policy = "explicit-non-default"
        snapshot_or_image = [string]$Environment.snapshot_or_image
        reset_policy = [string]$Environment.reset_policy
        fixture_manifest = [string]$Environment.fixture_manifest
        fixture_root_contract_sha256 = [string]$FixtureFacts.controlled_artifact_root_contract_sha256
        fixture_recipe_sha256 = [string]$FixtureRow.source_recipe_hash
        fixture_artifact_sha256 = [string]$FixtureRow.built_artifact_hash
        environment_capture_path = $expectedCapturePath
        environment_capture_schema = [string]$Contract.Schema
        owned_process_policy = [string]$Environment.owned_process_policy
        uia_modal_policy = [string]$Environment.uia_modal_policy
        required_attestation_schema = "oxvba-windows-x64-pinned-image-restore-session-attestation-v1"
        environment_evidence_state = [string]$Environment.evidence_state
    }
    foreach ($field in $expected.Keys) {
        if ([string]$Plan.$field -cne [string]$expected[$field]) {
            throw "$Owner field '$field' differs from its canonical producer input"
        }
    }
    if ([string]$Environment.role -ne "certification-vm" -or
        [string]$Environment.profile -ne "windows-x64" -or
        [string]$Environment.target_arch -ne "x64" -or
        [string]$Environment.office_bitness -ne "64") {
        throw "$Owner is not a Windows x64/Office64 certification plan"
    }
    foreach ($field in @(
        "environment_id", "os_build", "office_version", "office_build", "office_channel", "locale",
        "snapshot_or_image", "fixture_manifest", "fixture_hash"
    )) {
        if (Test-WindowsFixturePlaceholder -Value ([string]$Environment.$field)) {
            throw "$Owner canonical environment retains placeholder '$field=$($Environment.$field)'"
        }
    }
    foreach ($field in @("locale", "default_locale")) {
        if (Test-WindowsFixturePlaceholder -Value ([string]$Plan.$field)) {
            throw "$Owner $field must be a concrete locale identity"
        }
        try {
            $culture = [Globalization.CultureInfo]::GetCultureInfo([string]$Plan.$field)
            if ($culture.IsNeutralCulture -or [string]$culture.Name -cne [string]$Plan.$field) {
                throw "locale identity is neutral or not canonical"
            }
        }
        catch {
            throw "$Owner $field is not a canonical supported locale identity"
        }
    }
    if ([string]$Plan.locale -ceq [string]$Plan.default_locale) {
        throw "$Owner locale must differ from the declared default_locale"
    }
    if ([string]$Environment.snapshot_or_image -notmatch '^(?:[A-Za-z0-9._-]+@)?sha256:[0-9a-f]{64}$') {
        throw "$Owner snapshot_or_image lacks an immutable SHA-256 identity"
    }
    foreach ($field in @("fixture_root_contract_sha256", "fixture_recipe_sha256", "fixture_artifact_sha256")) {
        if ([string]$Plan.$field -notmatch '^sha256:[0-9a-f]{64}$') {
            throw "$Owner $field lacks an immutable SHA-256 identity"
        }
    }
    if ([string]$Environment.fixture_manifest -cne [string]$FixtureFacts.manifest_path -or
        [string]$Environment.fixture_hash -cne [string]$FixtureFacts.controlled_artifact_root_contract_sha256) {
        throw "$Owner fixture manifest/root hash differs from the controlled fixture authority"
    }
    if ([string]$FixtureRow.fixture_id -cne [string]$Case.fixture.fixture_id -or
        [string]$FixtureRow.recipe_id -cne [string]$Case.fixture.recipe_id -or
        [string]$FixtureRow.built_artifact_id -cne [string]$Case.fixture.built_artifact_id) {
        throw "$Owner fixture identity differs from the certification case"
    }
    if ([string]$Environment.reset_policy -notmatch '(?i)(revert|reset).*(pinned|snapshot|image)|(pinned|snapshot|image).*(revert|reset)') {
        throw "$Owner reset policy does not require a pinned-image reset/revert"
    }
    if ([string]$Environment.owned_process_policy -notmatch '(?i)owned' -or
        [string]$Environment.owned_process_policy -notmatch '(?i)(never-kill-unowned|kill-owned)') {
        throw "$Owner owned-process policy does not confine cleanup to recorded owned processes"
    }
    if ([string]$Environment.uia_modal_policy -notmatch '(?i)(UIA|UI Automation).*(modal|dialog)') {
        throw "$Owner UIA policy does not require owned Excel/VBE modal interception"
    }
    if ([bool]$Plan.certification_authority -or -not [bool]$Plan.noncertifying) {
        throw "$Owner must remain noncertifying until trusted restore/session attestation is verified"
    }
    $canonicalPlan = ConvertTo-IdealCertificationEnvironmentPlanCanonicalValue -Plan $Plan
    return Get-IdealCaptureObjectHash -Value $canonicalPlan
}

function New-IdealCertificationEnvironmentPlanValue {
    param(
        [Parameter(Mandatory = $true)]$Environment,
        [Parameter(Mandatory = $true)]$Case,
        [Parameter(Mandatory = $true)]$FixtureRow,
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)]$FixtureFacts,
        [Parameter(Mandatory = $true)][string]$DefaultLocale,
        [Parameter(Mandatory = $true)][int64]$AnsiCodepage,
        [Parameter(Mandatory = $true)][int64]$OemCodepage
    )

    $environmentId = [string]$Environment.environment_id
    $capturePath = "$($Contract.Root)/$($Contract.Name)"
    $plan = [pscustomobject][ordered]@{
        schema_id = $script:IdealCertificationEnvironmentPlanSchema
        schema_version = 1
        plan_id = "$environmentId-certification-plan-v1"
        case_id = [string]$Case.case_id
        matrix_id = [string]$Case.matrix_id
        row_id = [string]$Case.row_id
        environment_id = $environmentId
        role = [string]$Environment.role
        profile = [string]$Environment.profile
        target_arch = [string]$Environment.target_arch
        os_build = [string]$Environment.os_build
        office_product = [string]$Environment.office_product
        office_version = [string]$Environment.office_version
        office_build = [string]$Environment.office_build
        office_channel = [string]$Environment.office_channel
        office_bitness = [string]$Environment.office_bitness
        locale = [string]$Environment.locale
        default_locale = $DefaultLocale
        locale_policy = "explicit-non-default"
        ansi_codepage = $AnsiCodepage
        oem_codepage = $OemCodepage
        snapshot_or_image = [string]$Environment.snapshot_or_image
        reset_policy = [string]$Environment.reset_policy
        fixture_manifest = [string]$Environment.fixture_manifest
        fixture_root_contract_sha256 = [string]$FixtureFacts.controlled_artifact_root_contract_sha256
        fixture_recipe_sha256 = [string]$FixtureRow.source_recipe_hash
        fixture_artifact_sha256 = [string]$FixtureRow.built_artifact_hash
        environment_capture_path = $capturePath
        environment_capture_schema = [string]$Contract.Schema
        owned_process_policy = [string]$Environment.owned_process_policy
        uia_modal_policy = [string]$Environment.uia_modal_policy
        required_attestation_schema = "oxvba-windows-x64-pinned-image-restore-session-attestation-v1"
        environment_evidence_state = [string]$Environment.evidence_state
        certification_authority = $false
        noncertifying = $true
    }
    [void](Assert-IdealCertificationEnvironmentPlanValue `
        -Plan $plan `
        -Environment $Environment `
        -Case $Case `
        -FixtureRow $FixtureRow `
        -Contract $Contract `
        -FixtureFacts $FixtureFacts)
    return $plan
}

function Assert-IdealCertificationEnvironmentPlanSealValue {
    param(
        [Parameter(Mandatory = $true)]$Seal,
        [Parameter(Mandatory = $true)]$Plan,
        [Parameter(Mandatory = $true)]$Environment,
        [Parameter(Mandatory = $true)]$Case,
        [Parameter(Mandatory = $true)]$FixtureRow,
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)]$FixtureFacts,
        [string]$Owner = "certification environment plan seal"
    )

    Assert-WindowsFixtureExactJsonProperties -Value $Seal -Expected @(
        "schema_id", "schema_version", "seal_id", "plan_id", "plan_sha256", "environment_id",
        "case_id", "environment_capture_path", "required_attestation_schema", "attestation_state",
        "certification_authority", "noncertifying"
    ) -Owner $Owner
    Assert-WindowsFixtureJsonStringProperties -Value $Seal -Properties @(
        "schema_id", "seal_id", "plan_id", "plan_sha256", "environment_id", "case_id",
        "environment_capture_path", "required_attestation_schema", "attestation_state"
    ) -Owner $Owner
    if (-not (Test-WindowsFixtureJsonInteger -Value $Seal.schema_version) -or
        [int64]$Seal.schema_version -ne 1 -or
        $Seal.certification_authority -isnot [bool] -or
        $Seal.noncertifying -isnot [bool]) {
        throw "$Owner schema/version/authority field types are invalid"
    }
    $planHash = Assert-IdealCertificationEnvironmentPlanValue `
        -Plan $Plan `
        -Environment $Environment `
        -Case $Case `
        -FixtureRow $FixtureRow `
        -Contract $Contract `
        -FixtureFacts $FixtureFacts `
        -Owner "$Owner plan"
    $expected = [ordered]@{
        schema_id = $script:IdealCertificationEnvironmentPlanSealSchema
        seal_id = "$($Plan.plan_id)-seal-v1"
        plan_id = [string]$Plan.plan_id
        plan_sha256 = $planHash
        environment_id = [string]$Plan.environment_id
        case_id = [string]$Plan.case_id
        environment_capture_path = [string]$Plan.environment_capture_path
        required_attestation_schema = [string]$Plan.required_attestation_schema
        attestation_state = "required-unavailable"
    }
    foreach ($field in $expected.Keys) {
        if ([string]$Seal.$field -cne [string]$expected[$field]) {
            throw "$Owner field '$field' differs from the immutable plan binding"
        }
    }
    if ([bool]$Seal.certification_authority -or -not [bool]$Seal.noncertifying) {
        throw "$Owner is only a plan seal and cannot claim certification authority"
    }
    return $planHash
}

function New-IdealCertificationEnvironmentPlanSealValue {
    param(
        [Parameter(Mandatory = $true)]$Plan,
        [Parameter(Mandatory = $true)]$Environment,
        [Parameter(Mandatory = $true)]$Case,
        [Parameter(Mandatory = $true)]$FixtureRow,
        [Parameter(Mandatory = $true)]$Contract,
        [Parameter(Mandatory = $true)]$FixtureFacts
    )

    $planHash = Assert-IdealCertificationEnvironmentPlanValue `
        -Plan $Plan `
        -Environment $Environment `
        -Case $Case `
        -FixtureRow $FixtureRow `
        -Contract $Contract `
        -FixtureFacts $FixtureFacts
    $seal = [pscustomobject][ordered]@{
        schema_id = $script:IdealCertificationEnvironmentPlanSealSchema
        schema_version = 1
        seal_id = "$($Plan.plan_id)-seal-v1"
        plan_id = [string]$Plan.plan_id
        plan_sha256 = $planHash
        environment_id = [string]$Plan.environment_id
        case_id = [string]$Plan.case_id
        environment_capture_path = [string]$Plan.environment_capture_path
        required_attestation_schema = [string]$Plan.required_attestation_schema
        attestation_state = "required-unavailable"
        certification_authority = $false
        noncertifying = $true
    }
    [void](Assert-IdealCertificationEnvironmentPlanSealValue `
        -Seal $seal `
        -Plan $Plan `
        -Environment $Environment `
        -Case $Case `
        -FixtureRow $FixtureRow `
        -Contract $Contract `
        -FixtureFacts $FixtureFacts)
    return $seal
}

function Get-IdealDevHostFingerprint {
    param(
        [Parameter(Mandatory = $true)]$Environment,
        [Parameter(Mandatory = $true)]$Observation
    )

    $input = [ordered]@{
        schema_id = "oxvba-windows-x64-dev-host-fingerprint-v1"
        schema_version = 1
        environment_id = [string]$Environment.environment_id
        profile = [string]$Environment.profile
        target_arch = [string]$Environment.target_arch
        os = $Observation.os
        office = $Observation.office
        locale = $Observation.locale
        toolchain = $Observation.toolchain
        reset_policy = [string]$Environment.reset_policy
        owned_process_policy = [string]$Environment.owned_process_policy
        uia_modal_policy = [string]$Environment.uia_modal_policy
    }
    return [pscustomobject]@{
        Input = $input
        Hash = Get-IdealCaptureObjectHash -Value $input
    }
}

function Assert-IdealDevHostFingerprintPreimage {
    param(
        [Parameter(Mandatory = $true)]$Preimage,
        [Parameter(Mandatory = $true)]$Environment,
        [string]$Owner = "dev host fingerprint preimage"
    )

    Assert-WindowsFixtureExactJsonProperties -Value $Preimage -Expected @(
        "schema_id", "schema_version", "environment_id", "profile", "target_arch",
        "os", "office", "locale", "toolchain", "reset_policy",
        "owned_process_policy", "uia_modal_policy"
    ) -Owner $Owner
    Assert-WindowsFixtureJsonStringProperties -Value $Preimage -Properties @(
        "schema_id", "environment_id", "profile", "target_arch", "reset_policy",
        "owned_process_policy", "uia_modal_policy"
    ) -Owner $Owner
    if ([string]$Preimage.schema_id -ne "oxvba-windows-x64-dev-host-fingerprint-v1" -or `
        -not (Test-WindowsFixtureJsonInteger -Value $Preimage.schema_version) -or `
        [int64]$Preimage.schema_version -ne 1) {
        throw "$Owner schema identity must be oxvba-windows-x64-dev-host-fingerprint-v1 version 1"
    }

    Assert-WindowsFixtureExactJsonProperties -Value $Preimage.os -Expected @(
        "build", "display_version", "edition_id"
    ) -Owner "$Owner.os"
    Assert-WindowsFixtureJsonStringProperties -Value $Preimage.os -Properties @(
        "build", "display_version", "edition_id"
    ) -Owner "$Owner.os"
    Assert-WindowsFixtureExactJsonProperties -Value $Preimage.office -Expected @(
        "product", "version", "build", "channel", "channel_identity", "bitness",
        "excel_pe_machine", "client_culture", "product_release_ids"
    ) -Owner "$Owner.office"
    Assert-WindowsFixtureJsonStringProperties -Value $Preimage.office -Properties @(
        "product", "version", "build", "channel", "channel_identity", "bitness",
        "excel_pe_machine", "client_culture", "product_release_ids"
    ) -Owner "$Owner.office"
    Assert-WindowsFixtureExactJsonProperties -Value $Preimage.locale -Expected @(
        "current_culture", "current_ui_culture", "system_locale", "ansi_codepage",
        "oem_codepage", "console_input_codepage", "console_output_codepage"
    ) -Owner "$Owner.locale"
    Assert-WindowsFixtureJsonStringProperties -Value $Preimage.locale -Properties @(
        "current_culture", "current_ui_culture", "system_locale"
    ) -Owner "$Owner.locale"
    foreach ($field in @("ansi_codepage", "oem_codepage", "console_input_codepage", "console_output_codepage")) {
        if (-not (Test-WindowsFixtureJsonInteger -Value $Preimage.locale.$field) -or [int64]$Preimage.locale.$field -le 0) {
            throw "$Owner.locale.$field must be a positive JSON integer"
        }
    }
    Assert-WindowsFixtureExactJsonProperties -Value $Preimage.toolchain -Expected @(
        "rustc_verbose", "cargo_version", "rustup_active_toolchain"
    ) -Owner "$Owner.toolchain"
    Assert-WindowsFixtureJsonStringProperties -Value $Preimage.toolchain -Properties @(
        "rustc_verbose", "cargo_version", "rustup_active_toolchain"
    ) -Owner "$Owner.toolchain"
    foreach ($group in @(
        @($Preimage, @("environment_id", "profile", "target_arch", "reset_policy", "owned_process_policy", "uia_modal_policy")),
        @($Preimage.os, @("build", "display_version", "edition_id")),
        @($Preimage.office, @("product", "version", "build", "channel", "channel_identity", "bitness", "excel_pe_machine", "client_culture", "product_release_ids")),
        @($Preimage.locale, @("current_culture", "current_ui_culture", "system_locale")),
        @($Preimage.toolchain, @("rustc_verbose", "cargo_version", "rustup_active_toolchain"))
    )) {
        foreach ($field in $group[1]) {
            if ([string]::IsNullOrWhiteSpace([string]$group[0].$field)) {
                throw "$Owner.$field must not be blank"
            }
        }
    }

    $comparisons = [ordered]@{
        environment_id = [string]$Environment.environment_id
        profile = [string]$Environment.profile
        target_arch = [string]$Environment.target_arch
        reset_policy = [string]$Environment.reset_policy
        owned_process_policy = [string]$Environment.owned_process_policy
        uia_modal_policy = [string]$Environment.uia_modal_policy
    }
    foreach ($field in $comparisons.Keys) {
        if ([string]$Preimage.$field -cne [string]$comparisons[$field]) {
            throw "$Owner.$field differs from the canonical environment"
        }
    }
    $nestedComparisons = @(
        @("os.build", [string]$Preimage.os.build, [string]$Environment.os_build),
        @("office.product", [string]$Preimage.office.product, [string]$Environment.office_product),
        @("office.version", [string]$Preimage.office.version, [string]$Environment.office_version),
        @("office.build", [string]$Preimage.office.build, [string]$Environment.office_build),
        @("office.channel", [string]$Preimage.office.channel, [string]$Environment.office_channel),
        @("office.bitness", [string]$Preimage.office.bitness, [string]$Environment.office_bitness),
        @("locale.current_culture", [string]$Preimage.locale.current_culture, [string]$Environment.locale)
    )
    foreach ($comparison in $nestedComparisons) {
        if ([string]$comparison[1] -cne [string]$comparison[2]) {
            throw "$Owner.$($comparison[0]) differs from the canonical environment"
        }
    }
    if ([string]$Preimage.target_arch -ne "x64" -or `
        [string]$Preimage.office.bitness -ne "64" -or `
        [string]$Preimage.office.excel_pe_machine -ne "AMD64" -or `
        [string]$Preimage.office.channel_identity -notmatch '^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$' -or `
        [string]$Preimage.toolchain.rustc_verbose -notmatch '(?m)^host: x86_64-pc-windows-msvc$' -or `
        [string]$Preimage.toolchain.rustup_active_toolchain -notmatch '^stable-x86_64-pc-windows-msvc(?: |$)') {
        throw "$Owner does not prove the x64 Office64 MSVC toolchain profile"
    }
    return Get-IdealCaptureObjectHash -Value $Preimage
}

function Assert-IdealCaptureObservedEnvironment {
    param(
        [Parameter(Mandatory = $true)]$Environment,
        [Parameter(Mandatory = $true)]$Observation,
        [Parameter(Mandatory = $true)]$FixtureFacts
    )

    $environmentId = [string]$Environment.environment_id
    if ([string]$Environment.profile -ne "windows-x64" -or
        [string]$Environment.target_arch -ne "x64" -or
        [string]$Environment.office_bitness -ne "64") {
        throw "capture-ideal-environment: '$environmentId' is not Windows x64 with Office64"
    }
    $comparisons = [ordered]@{
        os_build = [string]$Observation.os.build
        office_product = [string]$Observation.office.product
        office_version = [string]$Observation.office.version
        office_build = [string]$Observation.office.build
        office_channel = [string]$Observation.office.channel
        office_bitness = [string]$Observation.office.bitness
        locale = [string]$Observation.locale.current_culture
    }
    foreach ($field in $comparisons.Keys) {
        if ([string]$Environment.$field -cne [string]$comparisons[$field]) {
            throw "capture-ideal-environment: observed $field='$($comparisons[$field])' differs from canonical '$($Environment.$field)'"
        }
    }
    if ([string]$Environment.fixture_manifest -cne [string]$FixtureFacts.manifest_path -or
        [string]$Environment.fixture_hash -cne [string]$FixtureFacts.controlled_artifact_root_contract_sha256) {
        throw "capture-ideal-environment: canonical fixture manifest path/hash does not match the captured fixture authority"
    }
    if ([string]$Environment.role -eq "dev-oracle") {
        $fingerprint = Get-IdealDevHostFingerprint -Environment $Environment -Observation $Observation
        $expected = "dev-host-fingerprint-v1@$($fingerprint.Hash)"
        if ([string]$Environment.snapshot_or_image -cne $expected) {
            throw "capture-ideal-environment: dev host fingerprint differs; expected snapshot_or_image '$expected'"
        }
        if ([string]$Environment.evidence_state -ne "characterized-noncertifying") {
            throw "capture-ideal-environment: dev host must remain characterized-noncertifying"
        }
    }
    else {
        throw "capture-ideal-environment: certification capture is disabled until a trusted pinned-image restore/session attestation is implemented"
    }
}

function New-IdealEnvironmentCaptureValue {
    param([Parameter(Mandatory = $true)]$Environment)

    if ([string]$Environment.role -ne "dev-oracle") {
        throw "capture-ideal-environment: certification authority cannot be derived from the manifest role; trusted reset attestation is required"
    }
    return [pscustomobject][ordered]@{
        schema_id = $script:IdealEnvironmentCaptureSchema
        schema_version = 1
        capture_id = "$($Environment.environment_id)-capture-v1"
        environment_id = [string]$Environment.environment_id
        role = [string]$Environment.role
        profile = [string]$Environment.profile
        target_arch = [string]$Environment.target_arch
        os_build = [string]$Environment.os_build
        office_product = [string]$Environment.office_product
        office_version = [string]$Environment.office_version
        office_build = [string]$Environment.office_build
        office_channel = [string]$Environment.office_channel
        office_bitness = [string]$Environment.office_bitness
        locale = [string]$Environment.locale
        snapshot_or_image = [string]$Environment.snapshot_or_image
        reset_policy = [string]$Environment.reset_policy
        reset_policy_hash = Get-IdealCaptureSha256Text -Text (([string]$Environment.reset_policy).Replace("`r`n", "`n").Replace("`r", "`n"))
        evidence_state = [string]$Environment.evidence_state
        certification_authority = $false
        noncertifying = $true
    }
}

function New-IdealEnvironmentCaptureMarkdown {
    param(
        [Parameter(Mandatory = $true)]$Environment,
        [Parameter(Mandatory = $true)]$Observation,
        [Parameter(Mandatory = $true)]$FixtureFacts,
        [Parameter(Mandatory = $true)]$Capture,
        [Parameter(Mandatory = $true)][string]$CaptureHash,
        [Parameter(Mandatory = $true)][string]$RegistryHash,
        [Parameter(Mandatory = $true)][string]$OutputPath
    )

    if ([string]$Environment.role -ne "dev-oracle") {
        throw "capture-ideal-environment: the development report cannot represent certification authority"
    }
    $fingerprint = Get-IdealDevHostFingerprint -Environment $Environment -Observation $Observation
    $fingerprintJson = (($fingerprint.Input | ConvertTo-Json -Depth 16).Replace("`r`n", "`n").Replace("`r", "`n"))
    $rustc = ([string]$Observation.toolchain.rustc_verbose).Replace("`n", "<br>")
    $release = if ([string]$Environment.role -eq "certification-vm") { "true" } else { "false" }
    $authority = ([bool]$Capture.certification_authority).ToString().ToLowerInvariant()
    $noncertifying = ([bool]$Capture.noncertifying).ToString().ToLowerInvariant()
    $lines = @(
        "# WIN-0 development/oracle environment capture",
        "",
        "Program period: **2026-07**. Environment: ``$($Environment.environment_id)``.",
        "",
        "This is the immutable characterization of the current development and Excel/VBA oracle host. It is explicitly ``release=$release``, ``certification_authority=$authority`` and ``noncertifying=$noncertifying``. It cannot replace the clean pinned Windows x64/64-bit Excel certification VM.",
        "",
        "## Canonical capture",
        "",
        "- Capture: ``$OutputPath``",
        "- Schema: ``$($Capture.schema_id)`` version ``$($Capture.schema_version)``",
        "- Capture SHA-256: ``$CaptureHash``",
        "- Host configuration identity: ``$($Environment.snapshot_or_image)``",
        "- Host fingerprint input SHA-256: ``$($fingerprint.Hash)``",
        "- Environment manifest: ``$script:IdealEnvironmentManifestPath``",
        "- Fixture manifest: ``$($FixtureFacts.manifest_path)``",
        "- Stable controlled-artifact root-contract SHA-256: ``$($FixtureFacts.controlled_artifact_root_contract_sha256)``",
        "",
        "The host identity hashes observed OS, Office x64, locale/codepage, Rust toolchain and operating-policy facts. It is a configuration fingerprint, not a claim that this physical host is resettable or clean.",
        "",
        "## Observed host",
        "",
        "| fact | value |",
        "|---|---|",
        "| OS build | ``$($Observation.os.build)`` (``$($Observation.os.display_version)``, ``$($Observation.os.edition_id)``) |",
        "| Excel/Office | ``$($Observation.office.product) $($Observation.office.build)``; ``$($Observation.office.channel)``; 64-bit; PE ``$($Observation.office.excel_pe_machine)`` |",
        "| Office channel identity | ``$($Observation.office.channel_identity)`` |",
        "| Office client culture | ``$($Observation.office.client_culture)`` |",
        "| Office product release IDs | ``$($Observation.office.product_release_ids)`` |",
        "| Current/UI/system locale | ``$($Observation.locale.current_culture)`` / ``$($Observation.locale.current_ui_culture)`` / ``$($Observation.locale.system_locale)`` |",
        "| ANSI/OEM codepage | ``$($Observation.locale.ansi_codepage)`` / ``$($Observation.locale.oem_codepage)`` |",
        "| Console input/output codepage | ``$($Observation.locale.console_input_codepage)`` / ``$($Observation.locale.console_output_codepage)`` |",
        "| rustc | ``$rustc`` |",
        "| Cargo | ``$($Observation.toolchain.cargo_version)`` |",
        "| rustup active | ``$($Observation.toolchain.rustup_active_toolchain)`` |",
        "",
        "## Controlled fixture roots",
        "",
        "The hashes below are ordinal, length-prefixed derivations of state-independent roots in the canonical fixture manifest. Build, source and environment state/hash transitions do not invalidate this host evidence, and no pending artifact is claimed to exist.",
        "",
        "- Rows: ``$($FixtureFacts.row_count)``; controlled-artifact root contract: ``$($FixtureFacts.controlled_artifact_root_contract_sha256)``.",
        "- Rows using this environment: ``$($FixtureFacts.environment_row_count)``; capture-root contract: ``$($FixtureFacts.environment_capture_root_contract_sha256)``.",
        "",
        "## Reset, ownership and UIA policy",
        "",
        "- Reset role: ``$($Environment.reset_policy)``. Manual recovery is noncertifying; there is no clean-snapshot claim.",
        "- Process ownership: ``$($Environment.owned_process_policy)``.",
        "- Excel/VBE UIA: ``$($Environment.uia_modal_policy)``.",
        "- The capture required no running Excel process, launched no Excel/VBE/COM/UIA automation, opened registry keys read-only and performed no Office or registry write.",
        "- The three Rust version observations ran as recorded owned child processes with bounded asynchronous stream draining and a 10-second wait, each synchronously reaped and disposed. The capture tool requested no temp path.",
        "- Mutation verdict: zero Excel/Office or registry mutation, zero residual owned process, and zero capture-owned temp path; the three version readers were the only transient processes.",
        "",
        "## Six-axis control evidence",
        "",
        "| axis | observation |",
        "|---|---|",
        "| result | Exact V1 JSON was reconstructed from the canonical manifest and matched all observed OS/Office/locale facts. Capture hash: ``$CaptureHash``. |",
        "| full Err | Not applicable: no VBA compile or execution occurred; no Err state was created or consumed. |",
        "| side effects | The only permitted persistent write was initial publication of the capture JSON and report; an identical rerun is read-only and a differing rerun is rejected. No Office, Excel, registry or fixture mutation API was used. No capture-owned temp path was requested. |",
        "| lifecycle/order | Assert no Excel process; read registry/PE/locale/toolchain/manifests; re-read registry; assert no Excel process; seal capture; publish evidence. |",
        "| transport | Read-only Win32 registry, PE metadata, locale APIs and synchronous ``rustc``/``cargo``/``rustup`` version queries. No COM, VBE, UIA or native fixture execution. |",
        "| balance | Excel PID set was empty before and after. Selected registry observation hash was ``$RegistryHash`` before and after. Each of the three owned version-observation children used bounded asynchronous drains, was awaited through exit and disposed; timeout cleanup is limited to its recorded owned PID. No broader system-process or temp-directory snapshot is claimed. |",
        "",
        "## Host fingerprint preimage",
        "",
        "The exact deterministic preimage below recomputes the SHA-256 suffix of ``$($Environment.snapshot_or_image)``.",
        "",
        "<!-- oxvba-dev-host-fingerprint-preimage-v1-begin -->",
        '```json',
        $fingerprintJson,
        '```',
        "<!-- oxvba-dev-host-fingerprint-preimage-v1-end -->",
        "",
        "This evidence supports only ``WAC-TARGET-DEV-ENV`` as characterized development/oracle infrastructure. The canonical row remains planned until the later WIN-0 reconciliation bead publishes the controlled environment root and completes the fixture/matrix handoff. Capability and release-certification credit remain ``none``."
    )
    return (($lines -join "`n") + "`n")
}
