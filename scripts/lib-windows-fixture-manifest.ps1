Set-StrictMode -Version Latest

function Get-WindowsFixtureManifestHeader {
    return @(
        "matrix_id",
        "row_id",
        "fixture_id",
        "recipe_id",
        "target_arch",
        "office_bitness",
        "process_shape",
        "apartment_shape",
        "exact_signature",
        "execution_recipe",
        "cleanup_recipe",
        "source_recipe_state",
        "source_recipe_paths",
        "source_recipe_hash",
        "source_recipe_owner_bead",
        "built_artifact_id",
        "built_artifact_class",
        "built_artifact_root",
        "built_artifact_name",
        "built_artifact_type",
        "built_artifact_components",
        "built_artifact_state",
        "built_artifact_path",
        "built_artifact_hash",
        "built_artifact_owner_bead",
        "environment_id",
        "environment_role",
        "environment_profile",
        "environment_target_arch",
        "environment_office_bitness",
        "environment_evidence_state",
        "environment_capture_root",
        "environment_capture_name",
        "environment_capture_schema",
        "environment_state",
        "environment_capture_path",
        "environment_hash",
        "environment_owner_bead",
        "result_expectation",
        "err_expectation",
        "side_effect_expectation",
        "lifecycle_order_expectation",
        "transport_expectation",
        "balance_expectation",
        "capability_credit",
        "notes"
    )
}

function Get-WindowsFixtureMatrixContracts {
    return [ordered]@{
        "WIN-COM-CLIENT" = "docs/validation/WINDOWS_JIT_COM_CLIENT_MATRIX_V1.csv"
        "WIN-COM-EVENTS" = "docs/validation/WINDOWS_JIT_COM_EVENTS_MATRIX_V1.csv"
        "WIN-COM-SERVER" = "docs/validation/WINDOWS_JIT_COM_SERVER_MATRIX_V1.csv"
        "WIN-NATIVE-IMPORT" = "docs/validation/WINDOWS_JIT_NATIVE_IMPORT_MATRIX_V1.csv"
        "WIN-NATIVE-EXPORT" = "docs/validation/WINDOWS_NATIVE_EXPORT_AND_PACKAGING_MATRIX_V1.csv"
        "WIN-ABI-CARRIER" = "docs/validation/WINDOWS_ABI_CARRIER_MATRIX_V1.csv"
    }
}

function Get-WindowsFixtureCurrentSourcePathMap {
    $dispatch = "crates/oxvba-com/src/fixtures/windows_test_dispatch.rs"
    $comCommon = "crates/oxvba-host/tests/com_matrix_common.rs"
    $comMethods = "crates/oxvba-host/tests/com_matrix_methods.rs"
    $comProperties = "crates/oxvba-host/tests/com_matrix_properties.rs"
    $comObjects = "crates/oxvba-host/tests/com_matrix_objects.rs"
    $comCollections = "crates/oxvba-host/tests/com_matrix_collections.rs"
    $comTypes = "crates/oxvba-host/tests/com_matrix_types.rs"
    $vtableFixture = "crates/oxvba-com/tests/com_vtable_fixture.rs"
    $dispCallFixture = "crates/oxvba-com/tests/disp_call_func_fixture.rs"
    $wrappedServer = "crates/oxvba-build/tests/wrapped_com_server_smoke.rs"
    $comHost = "crates/oxvba-comhost/src/lib.rs"
    $buildCompile = "crates/oxvba-build/src/compile.rs"
    $comDescriptor = "crates/oxvba-build/src/com_descriptor.rs"

    return @{
        "WIN-COM-CLIENT|WCC-PLAN-LATE" = @($dispatch, $comCommon, $comMethods, $comProperties)
        "WIN-COM-CLIENT|WCC-LATE-ARGS" = @($dispatch, $comCommon, $comMethods, $comProperties)
        "WIN-COM-CLIENT|WCC-LATE-STRUCTURAL" = @($dispatch, $comCommon, $comObjects, $comCollections, $comTypes)
        "WIN-COM-CLIENT|WCC-PLAN-EARLY" = @($dispatch, $dispCallFixture, $vtableFixture)
        "WIN-COM-CLIENT|WCC-EARLY-COMPLEX" = @($dispatch, $vtableFixture)
        "WIN-COM-CLIENT|WCC-EARLY-CUSTOM" = @($dispatch, $vtableFixture)

        "WIN-COM-EVENTS|WCE-INCOMING-LIFECYCLE" = @(
            $dispatch,
            "crates/oxvba-host/tests/com_matrix_events.rs",
            "tools/OxVba.TestEventServer/OxVba.TestEventServer.csproj",
            "tools/OxVba.TestEventServer/TestEventServer.cs",
            "tools/OxVba.TestEventServer/register.ps1"
        )

        "WIN-COM-SERVER|WCS-LATE-INPROC" = @($wrappedServer, $comHost, $buildCompile, $comDescriptor)
        "WIN-COM-SERVER|WCS-LATE-COMPLEX" = @($wrappedServer, $comHost, $buildCompile, $comDescriptor)
        "WIN-COM-SERVER|WCS-DUAL-INPROC" = @($wrappedServer, $comHost, $buildCompile, $comDescriptor)
        "WIN-COM-SERVER|WCS-IMPLEMENTS-CUSTOM" = @($wrappedServer, $comHost, $buildCompile, $comDescriptor)

        "WIN-NATIVE-EXPORT|WNE-PLAN-WRAPPED" = @($wrappedServer, $comHost, $buildCompile, $comDescriptor)

        "WIN-ABI-CARRIER|WAC-BSTR-LAYOUT" = @(
            "crates/oxvba-runtime/src/bstr.rs",
            "crates/oxvba-runtime/src/live_counters.rs"
        )
        "WIN-ABI-CARRIER|WAC-VARIANT-LAYOUT" = @(
            "crates/oxvba-runtime/src/variant.rs",
            "crates/oxvba-runtime/src/bstr.rs",
            "crates/oxvba-runtime/src/safe_array.rs",
            "crates/oxvba-runtime/src/object_ref.rs",
            "crates/oxvba-runtime/src/com_record.rs"
        )
        "WIN-ABI-CARRIER|WAC-SAFEARRAY-LAYOUT" = @(
            "crates/oxvba-runtime/src/safe_array.rs",
            "crates/oxvba-runtime/src/variant.rs"
        )
        "WIN-ABI-CARRIER|WAC-IUNKNOWN-IDENTITY" = @(
            "crates/oxvba-runtime/src/object_ref.rs",
            $dispatch
        )
        "WIN-ABI-CARRIER|WAC-NUMERIC-LONGPTR" = @(
            "crates/oxvba-runtime/src/value_types.rs",
            "crates/oxvba-runtime/src/decimal.rs",
            "crates/oxvba-runtime/src/vba_date.rs",
            "crates/oxvba-runtime/src/pointer_helpers.rs"
        )
        "WIN-ABI-CARRIER|WAC-INTERFACE-ARRAY" = @(
            "crates/oxvba-runtime/src/safe_array.rs",
            "crates/oxvba-runtime/src/object_ref.rs",
            $dispatch
        )
        "WIN-ABI-CARRIER|WAC-VT-RECORD" = @(
            "crates/oxvba-runtime/src/com_record.rs",
            "crates/oxvba-runtime/src/vba_record.rs",
            "crates/oxvba-runtime/src/safe_array.rs",
            $dispatch
        )
        "WIN-ABI-CARRIER|WAC-TYPELIB-METADATA" = @(
            "crates/oxvba-com/src/windows_typelib_loader.rs",
            "crates/oxvba-com/src/typelib.rs",
            "crates/oxvba-com/src/typelib_catalog.rs",
            "scripts/lib-com-testeventserver-alt-project.ps1",
            "scripts/run-com-testeventserver-versioned-typelib-probe.ps1",
            "tools/OxVba.TestEventServer/OxVba.TestEventServer.csproj",
            "tools/OxVba.TestEventServer/TestEventServer.cs",
            "tools/OxVba.TestEventServer/register.ps1"
        )
    }
}

function New-WindowsFixtureArtifactContract {
    param(
        [Parameter(Mandatory = $true)][string]$MatrixId,
        [Parameter(Mandatory = $true)][string]$RowId,
        [Parameter(Mandatory = $true)][string]$ArtifactClass,
        [Parameter(Mandatory = $true)][string]$Components,
        [AllowEmptyString()][string]$ComponentConstraints = "n/a"
    )

    if ([string]::IsNullOrWhiteSpace($ComponentConstraints)) {
        $ComponentConstraints = "n/a"
    }

    $root = "artifacts/windows-x64/controlled-fixtures/v1/$($MatrixId.ToLowerInvariant())/$($RowId.ToLowerInvariant())"
    $name = ""
    $type = ""
    switch ($ArtifactClass) {
        "pe-dll-x64" {
            $name = "fixture.dll"
            $type = "pe32plus-amd64-dll"
            if ($Components -ne "n/a") {
                throw "Direct PE DLL artifact '$MatrixId|$RowId' cannot declare bundle components"
            }
            if ($ComponentConstraints -ne "n/a") {
                throw "Direct PE DLL artifact '$MatrixId|$RowId' cannot declare component constraints"
            }
        }
        "pe-exe-x64" {
            $name = "fixture.exe"
            $type = "pe32plus-amd64-exe"
            if ($Components -ne "n/a") {
                throw "Direct PE EXE artifact '$MatrixId|$RowId' cannot declare bundle components"
            }
            if ($ComponentConstraints -ne "n/a") {
                throw "Direct PE EXE artifact '$MatrixId|$RowId' cannot declare component constraints"
            }
        }
        "fixture-bundle-json-v1" {
            $name = "fixture-bundle.json"
            $type = "oxvba-windows-x64-fixture-bundle-v1"
            if ([string]::IsNullOrWhiteSpace($Components) -or $Components -eq "n/a") {
                throw "Bundle artifact '$MatrixId|$RowId' must declare exact component types"
            }
            if ($ComponentConstraints -ne "n/a") {
                $componentKinds = @($Components -split '\|')
                $constraints = @($ComponentConstraints -split '\|')
                if ($constraints.Count -ne $componentKinds.Count -or
                    @($constraints | Where-Object { $_ -ne "n/a" -and $_ -notmatch '^sha256:[0-9a-f]{64}$' }).Count -gt 0) {
                    throw "Bundle artifact '$MatrixId|$RowId' has malformed ordered component constraints"
                }
            }
        }
        default {
            throw "Unknown Windows fixture artifact class '$ArtifactClass' for '$MatrixId|$RowId'"
        }
    }
    return [pscustomobject]@{
        Class = $ArtifactClass
        Root = $root
        Name = $name
        Type = $type
        Components = $Components
        ComponentConstraints = $ComponentConstraints
    }
}

function Get-WindowsFixtureArtifactContractMap {
    $specs = @'
matrix_id,row_id,artifact_class,components,component_constraints
WIN-COM-CLIENT,WCC-PLAN-LATE,pe-dll-x64,n/a
WIN-COM-CLIENT,WCC-LATE-ARGS,pe-dll-x64,n/a
WIN-COM-CLIENT,WCC-LATE-STRUCTURAL,pe-dll-x64,n/a
WIN-COM-CLIENT,WCC-LATE-OUTPROC-ERROR,pe-exe-x64,n/a
WIN-COM-CLIENT,WCC-PLAN-EARLY,pe-dll-x64,n/a
WIN-COM-CLIENT,WCC-EARLY-COMPLEX,pe-dll-x64,n/a
WIN-COM-CLIENT,WCC-EARLY-CUSTOM,pe-dll-x64,n/a
WIN-COM-CLIENT,WCC-EARLY-OUTPROC,pe-exe-x64,n/a
WIN-COM-CLIENT,WCC-EXCEL-AUTHORITY,fixture-bundle-json-v1,pe-dll-x64|pe-exe-x64|vba-source-utf8-v1
WIN-COM-EVENTS,WCE-PLAN-INCOMING,pe-dll-x64,n/a
WIN-COM-EVENTS,WCE-INCOMING-COMPLEX,pe-dll-x64,n/a
WIN-COM-EVENTS,WCE-INCOMING-CUSTOM,pe-dll-x64,n/a
WIN-COM-EVENTS,WCE-INCOMING-APARTMENT,pe-exe-x64,n/a
WIN-COM-EVENTS,WCE-INCOMING-LIFECYCLE,pe-dll-x64,n/a
WIN-COM-EVENTS,WCE-PLAN-OUTGOING,pe-dll-x64,n/a
WIN-COM-EVENTS,WCE-OUTGOING-COMPLEX,fixture-bundle-json-v1,pe-dll-x64|pe-exe-x64|vba-source-utf8-v1
WIN-COM-SERVER,WCS-LATE-INPROC,pe-dll-x64,n/a
WIN-COM-SERVER,WCS-LATE-LOCALSERVER,pe-exe-x64,n/a
WIN-COM-SERVER,WCS-LATE-COMPLEX,fixture-bundle-json-v1,pe-dll-x64|pe-exe-x64
WIN-COM-SERVER,WCS-DUAL-INPROC,pe-dll-x64,n/a
WIN-COM-SERVER,WCS-IMPLEMENTS-CUSTOM,pe-dll-x64,n/a
WIN-COM-SERVER,WCS-EARLY-OUTPROC,pe-exe-x64,n/a
WIN-COM-SERVER,WCS-SERVER-SAFETY,fixture-bundle-json-v1,pe-dll-x64|pe-exe-x64
WIN-NATIVE-IMPORT,WNI-PLAN-DECLARE,pe-dll-x64,n/a
WIN-NATIVE-IMPORT,WNI-DECLARE-STRINGS,pe-dll-x64,n/a
WIN-NATIVE-IMPORT,WNI-DECLARE-STRUCTURAL,pe-dll-x64,n/a
WIN-NATIVE-IMPORT,WNI-DECLARE-LOADER-ERROR,pe-dll-x64,n/a
WIN-NATIVE-IMPORT,WNI-POINTER-HELPERS,pe-exe-x64,n/a
WIN-NATIVE-IMPORT,WNI-CALLBACK-SYNC,pe-dll-x64,n/a
WIN-NATIVE-IMPORT,WNI-PLAN-CALLBACK,pe-dll-x64,n/a
WIN-NATIVE-IMPORT,WNI-CALLBACK-NESTED,fixture-bundle-json-v1,pe-dll-x64|pe-dll-x64
WIN-NATIVE-EXPORT,WNE-WRAPPER-EXE,pe-exe-x64,n/a
WIN-NATIVE-EXPORT,WNE-WRAPPER-LIBRARY,pe-dll-x64,n/a
WIN-NATIVE-EXPORT,WNE-PLAN-WRAPPED,fixture-bundle-json-v1,pe-dll-x64|pe-exe-x64
WIN-NATIVE-EXPORT,WNE-PLAN-NATIVE,pe-dll-x64,n/a
WIN-NATIVE-EXPORT,WNE-NATIVE-EXE,pe-exe-x64,n/a
WIN-NATIVE-EXPORT,WNE-NATIVE-ABI-BREADTH,pe-dll-x64,n/a
WIN-NATIVE-EXPORT,WNE-NATIVE-REPRO-DEPLOY,fixture-bundle-json-v1,pe-dll-x64|pe-exe-x64
WIN-NATIVE-EXPORT,WNE-PROFILE-TOOL-TERMINAL,fixture-bundle-json-v1,pe-dll-x64|pe-exe-x64
WIN-ABI-CARRIER,WAC-BSTR-LAYOUT,pe-exe-x64,n/a
WIN-ABI-CARRIER,WAC-VARIANT-LAYOUT,pe-exe-x64,n/a
WIN-ABI-CARRIER,WAC-SAFEARRAY-LAYOUT,pe-exe-x64,n/a
WIN-ABI-CARRIER,WAC-IUNKNOWN-IDENTITY,pe-exe-x64,n/a
WIN-ABI-CARRIER,WAC-NUMERIC-LONGPTR,pe-exe-x64,n/a
WIN-ABI-CARRIER,WAC-INTERFACE-ARRAY,pe-exe-x64,n/a
WIN-ABI-CARRIER,WAC-VT-RECORD,pe-exe-x64,n/a
WIN-ABI-CARRIER,WAC-CARRIER-EXCEL-ROUNDTRIP,fixture-bundle-json-v1,pe-dll-x64|vba-source-utf8-v1
WIN-ABI-CARRIER,WAC-SAFETY-MUTATION,pe-exe-x64,n/a
WIN-ABI-CARRIER,WAC-TARGET-DEV-ENV,fixture-bundle-json-v1,pe-exe-x64
WIN-ABI-CARRIER,WAC-TYPELIB-METADATA,fixture-bundle-json-v1,pe-dll-x64|msft-tlb-v1,n/a|sha256:9bdbc6a597d233296bd39adba69db9765552fdcce0261c6102b2e19d4d4c1a12
WIN-ABI-CARRIER,WAC-VERIFIED-INTEROP-PLAN,pe-exe-x64,n/a
WIN-ABI-CARRIER,WAC-WINDOWS-DESCRIPTORS,pe-exe-x64,n/a
WIN-ABI-CARRIER,WAC-CLEAN-CERT-ENV,fixture-bundle-json-v1,pe-exe-x64
WIN-ABI-CARRIER,WAC-RELEASE-CERT,fixture-bundle-json-v1,pe-dll-x64|pe-exe-x64
WIN-ABI-CARRIER,WAC-EXCEL-COM-CERT,fixture-bundle-json-v1,pe-dll-x64|pe-exe-x64|vba-source-utf8-v1
WIN-ABI-CARRIER,WAC-EXCEL-NATIVE-CERT,fixture-bundle-json-v1,pe-dll-x64|pe-exe-x64|vba-source-utf8-v1
WIN-ABI-CARRIER,WAC-PROFILE-TERMINAL,fixture-bundle-json-v1,pe-dll-x64|pe-exe-x64
'@ | ConvertFrom-Csv

    if ($specs.Count -ne 57) {
        throw "Windows fixture artifact contract must contain exactly 57 rows, found $($specs.Count)"
    }
    $contracts = @{}
    foreach ($spec in $specs) {
        $key = "$([string]$spec.matrix_id)|$([string]$spec.row_id)"
        if ($contracts.ContainsKey($key)) {
            throw "Duplicate Windows fixture artifact contract '$key'"
        }
        $contracts[$key] = New-WindowsFixtureArtifactContract `
            -MatrixId ([string]$spec.matrix_id) `
            -RowId ([string]$spec.row_id) `
            -ArtifactClass ([string]$spec.artifact_class) `
            -Components ([string]$spec.components) `
            -ComponentConstraints ([string]$spec.component_constraints)
    }
    return $contracts
}

function Get-WindowsFixtureEnvironmentCaptureContract {
    param(
        [Parameter(Mandatory = $true)]$Environment
    )

    $environmentId = [string]$Environment.environment_id
    if ([string]$Environment.profile -ne "windows-x64" -or
        [string]$Environment.role -notin @("dev-oracle", "certification-vm")) {
        throw "Environment '$environmentId' is not a controlled Windows x64 role"
    }
    return [pscustomobject]@{
        Root = "artifacts/windows-x64/controlled-environments/v1/$($environmentId.ToLowerInvariant())"
        Name = "environment-capture.json"
        Schema = "oxvba-windows-x64-environment-capture-v1"
    }
}

function ConvertTo-WindowsFixtureNormalizedRelativePaths {
    param(
        [Parameter(Mandatory = $true)][string[]]$Paths
    )

    $normalized = [Collections.Generic.SortedSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($path in $Paths) {
        $candidate = ([string]$path).Trim().Replace('\', '/')
        if ([string]::IsNullOrWhiteSpace($candidate)) {
            throw "Windows fixture source path cannot be blank"
        }
        Assert-IdealRelativePath -Path $candidate -Owner "Windows fixture source path"
        [void]$normalized.Add($candidate)
    }
    return @($normalized)
}

function ConvertTo-WindowsFixtureCanonicalText {
    param(
        [Parameter(Mandatory = $true)][byte[]]$Bytes,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $strictUtf8 = [Text.UTF8Encoding]::new($false, $true)
    try {
        $text = $strictUtf8.GetString($Bytes)
    }
    catch {
        throw "$Owner is not valid UTF-8 text and cannot be a source/recipe input"
    }
    if ($text.Length -gt 0 -and $text[0] -eq [char]0xFEFF) {
        $text = $text.Substring(1)
    }
    return ($text.Replace("`r`n", "`n").Replace("`r", "`n"))
}

function Get-WindowsFixtureSha256Text {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text
    )

    $bytes = [Text.UTF8Encoding]::new($false).GetBytes($Text)
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $digest = $sha.ComputeHash($bytes)
    }
    finally {
        $sha.Dispose()
    }
    return "sha256:$([Convert]::ToHexString($digest).ToLowerInvariant())"
}

function Get-WindowsFixtureCanonicalSourceFileHash {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath
    )

    $absolutePath = Resolve-IdealRepoPath -RepoRoot $RepositoryRoot -Path $RelativePath
    if (-not (Test-Path -LiteralPath $absolutePath -PathType Leaf)) {
        throw "Windows fixture source '$RelativePath' does not resolve"
    }
    $bytes = [IO.File]::ReadAllBytes($absolutePath)
    $text = ConvertTo-WindowsFixtureCanonicalText -Bytes $bytes -Owner "Windows fixture source '$RelativePath'"
    return Get-WindowsFixtureSha256Text -Text $text
}

function Get-WindowsFixtureRawFileHash {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath
    )

    $absolutePath = Resolve-IdealRepoPath -RepoRoot $RepositoryRoot -Path $RelativePath
    if (-not (Test-Path -LiteralPath $absolutePath -PathType Leaf)) {
        throw "Windows fixture artifact '$RelativePath' does not resolve"
    }
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $stream = [IO.File]::OpenRead($absolutePath)
        try {
            $digest = $sha.ComputeHash($stream)
        }
        finally {
            $stream.Dispose()
        }
    }
    finally {
        $sha.Dispose()
    }
    return "sha256:$([Convert]::ToHexString($digest).ToLowerInvariant())"
}

function Test-WindowsFixturePathWithin {
    param(
        [Parameter(Mandatory = $true)][string]$Candidate,
        [Parameter(Mandatory = $true)][string]$Root
    )

    $candidateFull = [IO.Path]::GetFullPath($Candidate).TrimEnd('\', '/')
    $rootFull = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    return $candidateFull.Equals($rootFull, [StringComparison]::OrdinalIgnoreCase) -or
        $candidateFull.StartsWith($rootFull + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)
}

function Test-WindowsFixtureMutableIdentity {
    param([AllowEmptyString()][string]$Value)

    return $Value -match '(?i)(?:^|[-_./:@])(?:latest|current|rolling|mutable|nightly|head|tip)(?:$|[-_./:@])'
}

function Assert-WindowsFixtureContainedPath {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath,
        [Parameter(Mandatory = $true)][string]$ControlledRoot,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    Assert-IdealRelativePath -Path $RelativePath -Owner $Owner
    Assert-IdealRelativePath -Path $ControlledRoot -Owner "$Owner controlled root"
    $repoFull = [IO.Path]::GetFullPath($RepositoryRoot).TrimEnd('\', '/')
    $rootFull = [IO.Path]::GetFullPath((Join-Path $repoFull $ControlledRoot)).TrimEnd('\', '/')
    $candidateFull = [IO.Path]::GetFullPath((Join-Path $repoFull $RelativePath))
    if (-not (Test-WindowsFixturePathWithin -Candidate $rootFull -Root $repoFull)) {
        throw "$Owner controlled root escapes the repository"
    }
    if (-not (Test-WindowsFixturePathWithin -Candidate $candidateFull -Root $rootFull)) {
        throw "$Owner path '$RelativePath' escapes controlled root '$ControlledRoot'"
    }
    if (-not (Test-Path -LiteralPath $rootFull -PathType Container) -or
        -not (Test-Path -LiteralPath $candidateFull -PathType Leaf)) {
        throw "$Owner path '$RelativePath' does not resolve to a controlled file"
    }

    $relativeFromRepo = [IO.Path]::GetRelativePath($repoFull, $candidateFull)
    $cursor = $repoFull
    foreach ($segment in @($relativeFromRepo -split '[\\/]')) {
        $cursor = Join-Path $cursor $segment
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "$Owner path '$RelativePath' crosses a reparse point and is not controlled"
            }
        }
    }

    $resolvedRoot = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $rootFull).Path)
    $resolvedCandidate = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $candidateFull).Path)
    if (-not (Test-WindowsFixturePathWithin -Candidate $resolvedCandidate -Root $resolvedRoot)) {
        throw "$Owner resolved path escapes controlled root '$ControlledRoot'"
    }
    return $resolvedCandidate
}

function Assert-WindowsFixtureExactJsonProperties {
    param(
        [Parameter(Mandatory = $true)]$Value,
        [Parameter(Mandatory = $true)][string[]]$Expected,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ($null -eq $Value -or $Value -is [Array] -or @($Value.PSObject.Properties).Count -eq 0) {
        throw "$Owner must be a JSON object"
    }
    $actualSet = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($propertyName in @($Value.PSObject.Properties.Name)) {
        [void]$actualSet.Add([string]$propertyName)
    }
    $expectedSet = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($propertyName in $Expected) {
        [void]$expectedSet.Add($propertyName)
    }
    if (-not $actualSet.SetEquals($expectedSet)) {
        $actual = @($actualSet | Sort-Object -CaseSensitive)
        $expectedSorted = @($expectedSet | Sort-Object -CaseSensitive)
        throw "$Owner does not match its exact case-sensitive schema (expected=$($expectedSorted -join '|'); actual=$($actual -join '|'))"
    }
}

function Assert-WindowsFixtureJsonPropertyUniqueness {
    param(
        [Parameter(Mandatory = $true)][Text.Json.JsonElement]$Element,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    switch ($Element.ValueKind) {
        ([Text.Json.JsonValueKind]::Object) {
            $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
            foreach ($property in $Element.EnumerateObject()) {
                if (-not $seen.Add($property.Name)) {
                    throw "$Owner contains duplicate JSON property '$($property.Name)'"
                }
                Assert-WindowsFixtureJsonPropertyUniqueness `
                    -Element $property.Value `
                    -Owner "$Owner.$($property.Name)"
            }
        }
        ([Text.Json.JsonValueKind]::Array) {
            $index = 0
            foreach ($item in $Element.EnumerateArray()) {
                Assert-WindowsFixtureJsonPropertyUniqueness -Element $item -Owner "$Owner[$index]"
                $index++
            }
        }
    }
}

function ConvertFrom-WindowsFixtureAuditedJson {
    param(
        [Parameter(Mandatory = $true)][byte[]]$Bytes,
        [Parameter(Mandatory = $true)][string]$Owner,
        [Parameter(Mandatory = $true)][string]$FormatName
    )

    $options = [Text.Json.JsonDocumentOptions]::new()
    $options.AllowTrailingCommas = $false
    $options.CommentHandling = [Text.Json.JsonCommentHandling]::Disallow
    try {
        $memory = [ReadOnlyMemory[byte]]::new($Bytes)
        $document = [Text.Json.JsonDocument]::Parse($memory, $options)
    }
    catch {
        throw "$Owner is not valid $FormatName JSON"
    }
    try {
        if ($document.RootElement.ValueKind -ne [Text.Json.JsonValueKind]::Object) {
            throw "$Owner must be a $FormatName JSON object"
        }
        Assert-WindowsFixtureJsonPropertyUniqueness -Element $document.RootElement -Owner $Owner
    }
    finally {
        $document.Dispose()
    }

    $text = ConvertTo-WindowsFixtureCanonicalText -Bytes $Bytes -Owner $Owner
    try {
        return $text | ConvertFrom-Json
    }
    catch {
        throw "$Owner could not be materialized after strict $FormatName JSON validation"
    }
}

function Assert-WindowsFixtureJsonStringProperties {
    param(
        [Parameter(Mandatory = $true)]$Value,
        [Parameter(Mandatory = $true)][string[]]$Properties,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    foreach ($property in $Properties) {
        if ($Value.$property -isnot [string]) {
            throw "$Owner property '$property' must be a JSON string"
        }
    }
}

function Test-WindowsFixtureJsonInteger {
    param($Value)

    return $Value -is [sbyte] -or $Value -is [byte] -or
        $Value -is [int16] -or $Value -is [uint16] -or
        $Value -is [int32] -or $Value -is [uint32] -or
        $Value -is [int64] -or $Value -is [uint64]
}

function Read-WindowsFixtureUInt16 {
    param(
        [Parameter(Mandatory = $true)][byte[]]$Bytes,
        [Parameter(Mandatory = $true)][int]$Offset,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ($Offset -lt 0 -or $Offset + 2 -gt $Bytes.Length) {
        throw "$Owner has a truncated 16-bit field at offset $Offset"
    }
    return [BitConverter]::ToUInt16($Bytes, $Offset)
}

function Read-WindowsFixtureUInt32 {
    param(
        [Parameter(Mandatory = $true)][byte[]]$Bytes,
        [Parameter(Mandatory = $true)][int]$Offset,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ($Offset -lt 0 -or $Offset + 4 -gt $Bytes.Length) {
        throw "$Owner has a truncated 32-bit field at offset $Offset"
    }
    return [BitConverter]::ToUInt32($Bytes, $Offset)
}

function Read-WindowsFixtureInt16 {
    param(
        [Parameter(Mandatory = $true)][byte[]]$Bytes,
        [Parameter(Mandatory = $true)][int]$Offset,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ($Offset -lt 0 -or $Offset + 2 -gt $Bytes.Length) {
        throw "$Owner has a truncated signed 16-bit field at offset $Offset"
    }
    return [BitConverter]::ToInt16($Bytes, $Offset)
}

function Read-WindowsFixtureInt32 {
    param(
        [Parameter(Mandatory = $true)][byte[]]$Bytes,
        [Parameter(Mandatory = $true)][int]$Offset,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ($Offset -lt 0 -or $Offset + 4 -gt $Bytes.Length) {
        throw "$Owner has a truncated signed 32-bit field at offset $Offset"
    }
    return [BitConverter]::ToInt32($Bytes, $Offset)
}

function Test-WindowsFixturePowerOfTwo {
    param([uint64]$Value)

    return $Value -gt 0 -and ($Value -band ($Value - 1)) -eq 0
}

function Get-WindowsFixtureAlignedValue {
    param(
        [Parameter(Mandatory = $true)][uint64]$Value,
        [Parameter(Mandatory = $true)][uint64]$Alignment,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if (-not (Test-WindowsFixturePowerOfTwo -Value $Alignment)) {
        throw "$Owner alignment '$Alignment' is not a power of two"
    }
    $remainder = $Value % $Alignment
    if ($remainder -eq 0) {
        return $Value
    }
    $increment = $Alignment - $remainder
    if ($Value -gt [uint64]::MaxValue - $increment) {
        throw "$Owner alignment calculation overflows"
    }
    return [uint64]($Value + $increment)
}

function Initialize-WindowsFixtureNativeProbe {
    if ($null -eq ("OxVbaFixtureAdmissionNativeV1" -as [type])) {
        [void](Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class OxVbaFixtureAdmissionNativeV1
{
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr LoadLibraryExW(string path, IntPtr file, uint flags);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool FreeLibrary(IntPtr module);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateFileW(
        string path, uint access, uint share, IntPtr security, uint creation,
        uint attributes, IntPtr template);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateFileMappingW(
        IntPtr file, IntPtr security, uint protect, uint maximumHigh,
        uint maximumLow, string name);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool CloseHandle(IntPtr handle);

    [DllImport("oleaut32.dll", CharSet = CharSet.Unicode, PreserveSig = true)]
    public static extern int LoadTypeLibEx(string path, int registrationKind, out IntPtr typeLib);
}
'@)
    }
}

function Assert-WindowsFixtureWindowsPeLoadable {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if (-not [Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([Runtime.InteropServices.OSPlatform]::Windows)) {
        return
    }
    Initialize-WindowsFixtureNativeProbe

    $module = [OxVbaFixtureAdmissionNativeV1]::LoadLibraryExW($Path, [IntPtr]::Zero, 0x00000001)
    if ($module -eq [IntPtr]::Zero) {
        $errorCode = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        throw "$Owner is structurally PE32+ but Windows LoadLibraryExW(DONT_RESOLVE_DLL_REFERENCES) rejected it (Win32=$errorCode)"
    }
    [void][OxVbaFixtureAdmissionNativeV1]::FreeLibrary($module)

    $genericRead = [Convert]::ToUInt32("80000000", 16)
    $fileHandle = [OxVbaFixtureAdmissionNativeV1]::CreateFileW(
        $Path, $genericRead, 0x00000005, [IntPtr]::Zero, 3, 0x00000080, [IntPtr]::Zero)
    if ($fileHandle -eq [IntPtr](-1)) {
        $errorCode = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        throw "$Owner could not be opened for SEC_IMAGE_NO_EXECUTE validation (Win32=$errorCode)"
    }
    try {
        $mapping = [OxVbaFixtureAdmissionNativeV1]::CreateFileMappingW(
            $fileHandle, [IntPtr]::Zero, 0x11000002, 0, 0, $null)
        if ($mapping -eq [IntPtr]::Zero) {
            $errorCode = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
            throw "$Owner was rejected by Windows SEC_IMAGE_NO_EXECUTE mapping (Win32=$errorCode)"
        }
        [void][OxVbaFixtureAdmissionNativeV1]::CloseHandle($mapping)
    }
    finally {
        [void][OxVbaFixtureAdmissionNativeV1]::CloseHandle($fileHandle)
    }
}

function Assert-WindowsFixturePeFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][ValidateSet("pe-dll-x64", "pe-exe-x64")][string]$ExpectedKind,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $expectedExtension = if ($ExpectedKind -eq "pe-dll-x64") { ".dll" } else { ".exe" }
    if (-not [IO.Path]::GetExtension($Path).Equals($expectedExtension, [StringComparison]::OrdinalIgnoreCase)) {
        throw "$Owner must use extension '$expectedExtension' for type '$ExpectedKind'"
    }
    $fileLength = [uint64](Get-Item -LiteralPath $Path).Length
    if ($fileLength -lt 512 -or $fileLength -gt [uint32]::MaxValue) {
        throw "$Owner PE file length is outside the bounded 32-bit image envelope"
    }

    $stream = [IO.File]::OpenRead($Path)
    $reader = $null
    try {
        try {
            $reader = [Reflection.PortableExecutable.PEReader]::new($stream)
            $headers = $reader.PEHeaders
        }
        catch {
            throw "$Owner is not a structurally valid bounded PE image"
        }
        if ($null -eq $headers) {
            throw "$Owner is not a structurally valid bounded PE image"
        }
        if ($headers.IsCoffOnly -or $null -eq $headers.PEHeader) {
            throw "$Owner is a COFF object rather than a mapped PE image"
        }
        $coff = $headers.CoffHeader
        $pe = $headers.PEHeader
        if ($coff.Machine -ne [Reflection.PortableExecutable.Machine]::Amd64) {
            throw "$Owner PE machine must be AMD64/x64 (0x8664), found '$($coff.Machine)'"
        }
        if ($pe.Magic -ne [Reflection.PortableExecutable.PEMagic]::PE32Plus) {
            throw "$Owner must be PE32+ x64, found '$($pe.Magic)'"
        }
        if (($coff.Characteristics -band [Reflection.PortableExecutable.Characteristics]::ExecutableImage) -eq 0) {
            throw "$Owner PE is not marked executable"
        }
        $isDll = ($coff.Characteristics -band [Reflection.PortableExecutable.Characteristics]::Dll) -ne 0
        if (($ExpectedKind -eq "pe-dll-x64" -and -not $isDll) -or
            ($ExpectedKind -eq "pe-exe-x64" -and $isDll)) {
            throw "$Owner PE DLL/EXE characteristics do not match '$ExpectedKind'"
        }

        # Controlled fixtures are desktop user-mode images. Bits below 0x20
        # are reserved/obsolete in current PE32+ images; AppContainer and WDM
        # describe process roles that this fixture contract does not admit.
        $dllCharacteristics = [uint16][int]$pe.DllCharacteristics
        if (($dllCharacteristics -band 0x001F) -ne 0) {
            throw ("$Owner PE DllCharacteristics contains reserved bits (0x{0:X4})" -f $dllCharacteristics)
        }
        if (($dllCharacteristics -band 0x0080) -ne 0) {
            throw "$Owner PE FORCE_INTEGRITY requires verified Authenticode admission, which this fixture gate does not perform"
        }
        if (($dllCharacteristics -band 0x1000) -ne 0 -or
            ($dllCharacteristics -band 0x2000) -ne 0) {
            throw "$Owner PE DllCharacteristics declares an unsupported AppContainer or WDM-driver role"
        }
        if (($ExpectedKind -eq "pe-dll-x64" -and
                $pe.Subsystem -ne [Reflection.PortableExecutable.Subsystem]::WindowsGui) -or
            ($ExpectedKind -eq "pe-exe-x64" -and
                $pe.Subsystem -notin @(
                    [Reflection.PortableExecutable.Subsystem]::WindowsGui,
                    [Reflection.PortableExecutable.Subsystem]::WindowsCui
                ))) {
            throw "$Owner PE subsystem '$($pe.Subsystem)' is not valid for controlled desktop '$ExpectedKind' role"
        }

        $fileAlignment = [uint64]$pe.FileAlignment
        $sectionAlignment = [uint64]$pe.SectionAlignment
        if (-not (Test-WindowsFixturePowerOfTwo -Value $fileAlignment) -or
            $fileAlignment -lt 512 -or $fileAlignment -gt 65536 -or
            -not (Test-WindowsFixturePowerOfTwo -Value $sectionAlignment) -or
            $sectionAlignment -lt $fileAlignment -or
            ($sectionAlignment -lt 4096 -and $sectionAlignment -ne $fileAlignment)) {
            throw "$Owner PE file/section alignments are invalid"
        }
        if ($pe.ImageBase % 65536 -ne 0 -or $pe.NumberOfRvaAndSizes -ne 16) {
            throw "$Owner PE image base or data-directory count is invalid"
        }
        if ($coff.NumberOfSections -le 0 -or $coff.NumberOfSections -ne $headers.SectionHeaders.Length) {
            throw "$Owner PE section count is invalid"
        }
        $sectionTableEnd = [uint64]$headers.CoffHeaderStartOffset + 20 +
            [uint64]$coff.SizeOfOptionalHeader + ([uint64]$coff.NumberOfSections * 40)
        $sizeOfHeaders = [uint64]$pe.SizeOfHeaders
        $sizeOfImage = [uint64]$pe.SizeOfImage
        if ($sizeOfHeaders -lt $sectionTableEnd -or $sizeOfHeaders -gt $fileLength -or
            $sizeOfHeaders % $fileAlignment -ne 0 -or $sizeOfImage -le $sizeOfHeaders -or
            $sizeOfImage % $sectionAlignment -ne 0) {
            throw "$Owner PE header/image sizes or alignment are invalid"
        }

        $rawRanges = [Collections.Generic.List[object]]::new()
        $virtualRanges = [Collections.Generic.List[object]]::new()
        $mappedRanges = [Collections.Generic.List[object]]::new()
        $sectionNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        $highestVirtualEnd = $sizeOfHeaders
        $computedCodeSize = [uint64]0
        $computedInitializedSize = [uint64]0
        $hasExecutableCode = $false
        foreach ($section in $headers.SectionHeaders) {
            if ([string]::IsNullOrWhiteSpace($section.Name) -or -not $sectionNames.Add($section.Name) -or
                $section.VirtualAddress -lt 0 -or $section.VirtualSize -lt 0 -or
                $section.PointerToRawData -lt 0 -or $section.SizeOfRawData -lt 0) {
                throw "$Owner PE has a blank/duplicate section name or negative section field"
            }
            $virtualAddress = [uint64]$section.VirtualAddress
            $virtualSize = [uint64]$section.VirtualSize
            $rawPointer = [uint64]$section.PointerToRawData
            $rawSize = [uint64]$section.SizeOfRawData
            $mappedSize = [Math]::Max($virtualSize, $rawSize)
            if ($mappedSize -eq 0 -or $virtualAddress % $sectionAlignment -ne 0 -or
                $virtualAddress -lt (Get-WindowsFixtureAlignedValue -Value $sizeOfHeaders -Alignment $sectionAlignment -Owner $Owner)) {
                throw "$Owner PE section '$($section.Name)' has invalid virtual mapping"
            }
            $virtualEnd = $virtualAddress + $mappedSize
            if ($virtualEnd -gt $sizeOfImage -or $virtualEnd -lt $virtualAddress) {
                throw "$Owner PE section '$($section.Name)' escapes SizeOfImage"
            }
            $virtualRanges.Add([pscustomobject]@{ Start = $virtualAddress; End = $virtualEnd; Name = $section.Name })
            $mappedRanges.Add([pscustomobject]@{ Start = $virtualAddress; End = $virtualEnd; Name = $section.Name })
            $alignedVirtualEnd = Get-WindowsFixtureAlignedValue -Value $virtualEnd -Alignment $sectionAlignment -Owner $Owner
            if ($alignedVirtualEnd -gt $highestVirtualEnd) {
                $highestVirtualEnd = $alignedVirtualEnd
            }

            if ($rawSize -gt 0) {
                $rawEnd = $rawPointer + $rawSize
                if ($rawPointer -lt $sizeOfHeaders -or $rawPointer % $fileAlignment -ne 0 -or
                    $rawSize % $fileAlignment -ne 0 -or $rawEnd -gt $fileLength -or $rawEnd -lt $rawPointer) {
                    throw "$Owner PE section '$($section.Name)' has invalid or out-of-file raw data"
                }
                $rawRanges.Add([pscustomobject]@{ Start = $rawPointer; End = $rawEnd; Name = $section.Name })
            }

            $sectionFlags = $section.SectionCharacteristics
            if (($sectionFlags -band [Reflection.PortableExecutable.SectionCharacteristics]::ContainsCode) -ne 0) {
                $computedCodeSize += $rawSize
            }
            if (($sectionFlags -band [Reflection.PortableExecutable.SectionCharacteristics]::ContainsInitializedData) -ne 0) {
                $computedInitializedSize += $rawSize
            }
            if (($sectionFlags -band [Reflection.PortableExecutable.SectionCharacteristics]::ContainsCode) -ne 0 -and
                ($sectionFlags -band [Reflection.PortableExecutable.SectionCharacteristics]::MemExecute) -ne 0 -and
                $rawSize -gt 0) {
                $hasExecutableCode = $true
            }
        }
        foreach ($ranges in @($rawRanges, $virtualRanges)) {
            $ordered = @($ranges | Sort-Object Start, End)
            for ($index = 1; $index -lt $ordered.Count; $index++) {
                if ([uint64]$ordered[$index].Start -lt [uint64]$ordered[$index - 1].End) {
                    throw "$Owner PE sections '$($ordered[$index - 1].Name)' and '$($ordered[$index].Name)' overlap"
                }
            }
        }
        if (-not $hasExecutableCode -or $highestVirtualEnd -ne $sizeOfImage -or
            [uint64]$pe.SizeOfCode -ne $computedCodeSize -or
            [uint64]$pe.SizeOfInitializedData -ne $computedInitializedSize) {
            throw "$Owner PE executable mapping or aggregate section sizes are inconsistent"
        }

        $entryPoint = [uint64]$pe.AddressOfEntryPoint
        if (($ExpectedKind -eq "pe-exe-x64" -and $entryPoint -eq 0) -or $entryPoint -ge $sizeOfImage) {
            throw "$Owner PE entry point is invalid for '$ExpectedKind'"
        }
        if ($entryPoint -ne 0) {
            $entrySection = @($virtualRanges | Where-Object {
                $entryPoint -ge [uint64]$_.Start -and $entryPoint -lt [uint64]$_.End
            })
            $executableSections = @($headers.SectionHeaders | Where-Object {
                ($_.SectionCharacteristics -band [Reflection.PortableExecutable.SectionCharacteristics]::MemExecute) -ne 0
            } | ForEach-Object Name)
            if ($entrySection.Count -ne 1 -or $entrySection[0].Name -notin $executableSections) {
                throw "$Owner PE entry point is not contained by one executable section"
            }
        }

        $directories = @(
            @{ Name = "export"; Entry = $pe.ExportTableDirectory },
            @{ Name = "import"; Entry = $pe.ImportTableDirectory },
            @{ Name = "resource"; Entry = $pe.ResourceTableDirectory },
            @{ Name = "exception"; Entry = $pe.ExceptionTableDirectory },
            @{ Name = "base-relocation"; Entry = $pe.BaseRelocationTableDirectory },
            @{ Name = "debug"; Entry = $pe.DebugTableDirectory },
            @{ Name = "copyright"; Entry = $pe.CopyrightTableDirectory },
            @{ Name = "global-pointer"; Entry = $pe.GlobalPointerTableDirectory },
            @{ Name = "tls"; Entry = $pe.ThreadLocalStorageTableDirectory },
            @{ Name = "load-config"; Entry = $pe.LoadConfigTableDirectory },
            @{ Name = "bound-import"; Entry = $pe.BoundImportTableDirectory },
            @{ Name = "iat"; Entry = $pe.ImportAddressTableDirectory },
            @{ Name = "delay-import"; Entry = $pe.DelayImportTableDirectory },
            @{ Name = "clr"; Entry = $pe.CorHeaderTableDirectory }
        )
        foreach ($directory in $directories) {
            $rva = [uint64]$directory.Entry.RelativeVirtualAddress
            $size = [uint64]$directory.Entry.Size
            if (($rva -eq 0) -xor ($size -eq 0)) {
                throw "$Owner PE $($directory.Name) directory has a partial range"
            }
            if ($size -eq 0) {
                continue
            }
            $end = $rva + $size
            $owners = if ($rva -lt $sizeOfHeaders -and $end -le $sizeOfHeaders) {
                @("headers")
            }
            else {
                @($mappedRanges | Where-Object { $rva -ge [uint64]$_.Start -and $end -le [uint64]$_.End })
            }
            if ($end -gt $sizeOfImage -or $end -lt $rva -or $owners.Count -ne 1) {
                throw "$Owner PE $($directory.Name) directory is not contained by one mapped range"
            }
        }
        $certificateOffset = [uint64]$pe.CertificateTableDirectory.RelativeVirtualAddress
        $certificateSize = [uint64]$pe.CertificateTableDirectory.Size
        if (($certificateOffset -eq 0) -xor ($certificateSize -eq 0)) {
            throw "$Owner PE certificate directory has a partial file range"
        }
        if ($certificateSize -gt 0 -and
            ($certificateOffset % 8 -ne 0 -or $certificateOffset + $certificateSize -gt $fileLength)) {
            throw "$Owner PE certificate directory escapes or is misaligned in the file"
        }
    }
    finally {
        if ($null -ne $reader) {
            $reader.Dispose()
        }
        $stream.Dispose()
    }

    Assert-WindowsFixtureWindowsPeLoadable -Path $Path -Owner $Owner
}

function Assert-WindowsFixtureMsftOwnedRecords {
    param(
        [Parameter(Mandatory = $true)][byte[]]$Bytes,
        [Parameter(Mandatory = $true)][object[]]$Segments,
        [Parameter(Mandatory = $true)][object[]]$TypeInfos,
        [Parameter(Mandatory = $true)][uint64]$NameCount,
        [Parameter(Mandatory = $true)][uint64]$NameCharacters,
        [Parameter(Mandatory = $true)][uint64]$ImportInfoCount,
        [Parameter(Mandatory = $true)][uint32]$VarFlags,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $fileLength = [uint64]$Bytes.Length
    $segmentDataEnd = [uint64](@($Segments | Measure-Object -Property End -Maximum)[0].Maximum)
    $typeInfoOffsets = [Collections.Generic.HashSet[int32]]::new()
    foreach ($typeInfo in $TypeInfos) {
        [void]$typeInfoOffsets.Add([int32]$typeInfo.RelativeOffset)
    }

    $assertFixedReference = {
        param(
            [int32]$Value,
            [int]$SegmentIndex,
            [int]$RecordSize,
            [int]$Alignment,
            [bool]$AllowMissing,
            [string]$ReferenceOwner
        )
        if ($Value -eq -1 -and $AllowMissing) {
            return
        }
        if ($Value -lt 0) {
            throw "$Owner MSFT $ReferenceOwner uses an invalid negative segment offset"
        }
        $segment = $Segments[$SegmentIndex]
        $offset = [uint64]$Value
        if ($segment.Length -eq 0 -or $offset % [uint64]$Alignment -ne 0 -or
            $offset + [uint64]$RecordSize -gt [uint64]$segment.Length) {
            throw "$Owner MSFT $ReferenceOwner does not identify a complete record in segment $SegmentIndex"
        }
    }

    # Parse the complete variable-length name table first. Every later name
    # reference must equal an entry start, not merely land somewhere in it.
    $nameOffsets = [Collections.Generic.HashSet[int32]]::new()
    $nameEntries = [Collections.Generic.List[object]]::new()
    $namePosition = [uint64]0
    $observedNameCharacters = [uint64]0
    $strictUtf8 = [Text.UTF8Encoding]::new($false, $true)
    while ($namePosition -lt [uint64]$Segments[7].Length) {
        if ($namePosition + 12 -gt [uint64]$Segments[7].Length) {
            throw "$Owner MSFT name table ends with a partial entry header"
        }
        $absolute = [int]([uint64]$Segments[7].Offset + $namePosition)
        $nameLength = [uint64]$Bytes[$absolute + 8]
        $paddedNameLength = $nameLength + ((4 - ($nameLength % 4)) % 4)
        $entryEnd = $namePosition + 12 + $paddedNameLength
        if ($entryEnd -gt [uint64]$Segments[7].Length) {
            throw "$Owner MSFT name table contains a truncated entry"
        }
        try {
            [void]$strictUtf8.GetString($Bytes, $absolute + 12, [int]$nameLength)
        }
        catch {
            throw "$Owner MSFT name table contains non-UTF-8 identifier bytes"
        }
        [void]$nameOffsets.Add([int32]$namePosition)
        $nameEntries.Add([pscustomobject]@{
            Offset = [int32]$namePosition
            Href = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset $absolute -Owner $Owner
            NextHash = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($absolute + 4) -Owner $Owner
        })
        $observedNameCharacters += $nameLength
        $namePosition = $entryEnd
    }
    if ([uint64]$nameOffsets.Count -ne $NameCount -or $observedNameCharacters -ne $NameCharacters) {
        throw "$Owner MSFT name table entries/characters do not match the bounded header counts"
    }

    $assertNameReference = {
        param([int32]$Value, [bool]$AllowMissing, [string]$ReferenceOwner)
        if ($Value -eq -1 -and $AllowMissing) {
            return
        }
        if ($Value -lt 0 -or -not $nameOffsets.Contains($Value)) {
            throw "$Owner MSFT $ReferenceOwner does not identify a complete name-table entry"
        }
    }

    # The string table uses 2-byte lengths and 4-byte aligned entry starts.
    $stringOffsets = [Collections.Generic.HashSet[int32]]::new()
    $stringPosition = [uint64]0
    while ($stringPosition -lt [uint64]$Segments[8].Length) {
        if ($stringPosition + 2 -gt [uint64]$Segments[8].Length) {
            throw "$Owner MSFT string table ends with a partial length"
        }
        $absolute = [int]([uint64]$Segments[8].Offset + $stringPosition)
        $stringLength = [uint64](Read-WindowsFixtureUInt16 -Bytes $Bytes -Offset $absolute -Owner $Owner)
        $rawSize = [uint64]2 + $stringLength
        $entrySize = $rawSize + ((4 - ($rawSize % 4)) % 4)
        if ($stringPosition + $entrySize -gt [uint64]$Segments[8].Length) {
            throw "$Owner MSFT string table contains a truncated entry"
        }
        try {
            [void]$strictUtf8.GetString($Bytes, $absolute + 2, [int]$stringLength)
        }
        catch {
            throw "$Owner MSFT string table contains non-UTF-8 bytes"
        }
        [void]$stringOffsets.Add([int32]$stringPosition)
        $stringPosition += $entrySize
    }
    $assertStringReference = {
        param([int32]$Value, [bool]$AllowMissing, [string]$ReferenceOwner)
        if ($Value -eq -1 -and $AllowMissing) {
            return
        }
        if ($Value -lt 0 -or -not $stringOffsets.Contains($Value)) {
            throw "$Owner MSFT $ReferenceOwner does not identify a complete string-table entry"
        }
    }

    # Import-file entries are variable length; record their exact starts so
    # ImpInfo cannot point into a filename or its padding.
    $importFileOffsets = [Collections.Generic.HashSet[int32]]::new()
    $importFilePosition = [uint64]0
    while ($importFilePosition -lt [uint64]$Segments[2].Length) {
        if ($importFilePosition + 14 -gt [uint64]$Segments[2].Length) {
            throw "$Owner MSFT import-file table ends with a partial entry"
        }
        $absolute = [int]([uint64]$Segments[2].Offset + $importFilePosition)
        $sizeField = Read-WindowsFixtureUInt16 -Bytes $Bytes -Offset ($absolute + 12) -Owner $Owner
        $fileNameLength = [uint64]($sizeField -shr 2)
        $rawSize = [uint64]14 + $fileNameLength
        $entrySize = $rawSize + ((4 - ($rawSize % 4)) % 4)
        if ($importFilePosition + $entrySize -gt [uint64]$Segments[2].Length) {
            throw "$Owner MSFT import-file table contains a truncated filename"
        }
        try {
            [void]$strictUtf8.GetString($Bytes, $absolute + 14, [int]$fileNameLength)
        }
        catch {
            throw "$Owner MSFT import-file table contains a non-UTF-8 filename"
        }
        [void]$importFileOffsets.Add([int32]$importFilePosition)
        & $assertFixedReference `
            (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset $absolute -Owner $Owner) `
            5 24 24 $false "import-file GUID"
        $importFilePosition += $entrySize
    }

    for ($index = 0; $index -lt $ImportInfoCount; $index++) {
        $absolute = [int]([uint64]$Segments[1].Offset + ([uint64]$index * 12))
        $fileOffset = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($absolute + 4) -Owner $Owner
        if ($fileOffset -lt 0 -or -not $importFileOffsets.Contains($fileOffset)) {
            throw "$Owner MSFT ImpInfo[$index] does not identify a complete import-file entry"
        }
        & $assertFixedReference `
            (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($absolute + 8) -Owner $Owner) `
            5 24 24 $false "ImpInfo[$index] GUID"
    }

    $assertHref = {
        param([int32]$Value, [string]$ReferenceOwner)
        if ($Value -lt 0) {
            throw "$Owner MSFT $ReferenceOwner uses a negative hreftype"
        }
        $effective = [int64]([uint32]$Value)
        if (($effective -band 0x01000000) -ne 0) {
            $effective = $effective -band 0xFEFFFFFF
        }
        if ($effective -le [int32]::MaxValue -and $typeInfoOffsets.Contains([int32]$effective)) {
            return
        }
        $importOffset = [int64]($effective -band 0xFFFFFFFC)
        if ($importOffset -gt [int32]::MaxValue) {
            throw "$Owner MSFT $ReferenceOwner hreftype is outside internal/import tables"
        }
        & $assertFixedReference ([int32]$importOffset) 1 12 12 $false $ReferenceOwner
    }

    # Validate hash-table buckets and every complete GUID/name hash link.
    for ($offset = 0; $offset -lt [int]$Segments[4].Length; $offset += 4) {
        & $assertFixedReference `
            (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ([int]$Segments[4].Offset + $offset) -Owner $Owner) `
            5 24 24 $true "GUID hash bucket[$($offset / 4)]"
    }
    for ($offset = 0; $offset -lt [int]$Segments[5].Length; $offset += 24) {
        & $assertFixedReference `
            (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ([int]$Segments[5].Offset + $offset + 20) -Owner $Owner) `
            5 24 24 $true "GUID entry[$($offset / 24)] hash link"
    }
    for ($offset = 0; $offset -lt [int]$Segments[6].Length; $offset += 4) {
        & $assertNameReference `
            (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ([int]$Segments[6].Offset + $offset) -Owner $Owner) `
            $true "name hash bucket[$($offset / 4)]"
    }
    foreach ($entry in $nameEntries) {
        & $assertNameReference ([int32]$entry.NextHash) $true "name hash link at $($entry.Offset)"
        if ([int32]$entry.Href -ge 0 -and -not $typeInfoOffsets.Contains([int32]$entry.Href)) {
            throw "$Owner MSFT name entry at $($entry.Offset) has an invalid TypeInfo owner"
        }
    }

    $assertTypeDesc = $null
    $assertArrayDesc = $null
    $activeTypeDescs = [Collections.Generic.HashSet[int32]]::new()
    $assertArrayDesc = {
        param([int32]$Value, [string]$ReferenceOwner)
        & $assertFixedReference $Value 10 8 4 $false $ReferenceOwner
        $absolute = [int]([uint64]$Segments[10].Offset + [uint64]$Value)
        $dimensions = [uint64](Read-WindowsFixtureUInt16 -Bytes $Bytes -Offset ($absolute + 4) -Owner $Owner)
        if ($dimensions -gt 64 -or [uint64]$Value + 8 + ($dimensions * 8) -gt [uint64]$Segments[10].Length) {
            throw "$Owner MSFT $ReferenceOwner array descriptor is truncated or unbounded"
        }
        $elementType = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset $absolute -Owner $Owner
        if ($elementType -ge 0) {
            & $assertTypeDesc $elementType "$ReferenceOwner element type"
        }
    }
    $assertTypeDesc = {
        param([int32]$Value, [string]$ReferenceOwner)
        & $assertFixedReference $Value 9 8 8 $false $ReferenceOwner
        if (-not $activeTypeDescs.Add($Value)) {
            return
        }
        try {
            $absolute = [int]([uint64]$Segments[9].Offset + [uint64]$Value)
            $vt = (Read-WindowsFixtureUInt32 -Bytes $Bytes -Offset $absolute -Owner $Owner) -band 0xFFFF
            $extra = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($absolute + 4) -Owner $Owner
            switch ($vt) {
                26 { if ($extra -ge 0) { & $assertTypeDesc $extra "$ReferenceOwner pointed type" } }
                27 { if ($extra -ge 0) { & $assertTypeDesc $extra "$ReferenceOwner SAFEARRAY element type" } }
                28 { if ($extra -ge 0) { & $assertArrayDesc $extra "$ReferenceOwner CARRAY descriptor" } }
                29 { & $assertHref $extra "$ReferenceOwner user-defined type" }
            }
        }
        finally {
            [void]$activeTypeDescs.Remove($Value)
        }
    }
    for ($offset = 0; $offset -lt [int]$Segments[9].Length; $offset += 8) {
        & $assertTypeDesc ([int32]$offset) "TypeDesc[$($offset / 8)]"
    }

    # Segment 11 is a packed sequence of VARTYPE-tagged values. Parse the
    # complete table once so later references must identify a record start,
    # rather than merely point at bytes that happen to fit inside the segment.
    $customDataValueStarts = [Collections.Generic.HashSet[int32]]::new()
    $customDataValueIntervals = [Collections.Generic.List[object]]::new()
    $customDataPosition = [uint64]0
    while ($customDataPosition -lt [uint64]$Segments[11].Length) {
        if ($customDataPosition % 4 -ne 0 -or
            $customDataPosition + 2 -gt [uint64]$Segments[11].Length) {
            throw "$Owner MSFT custom-data segment has a misaligned or trailing partial record"
        }
        $absolute = [int]([uint64]$Segments[11].Offset + $customDataPosition)
        $vt = Read-WindowsFixtureUInt16 -Bytes $Bytes -Offset $absolute -Owner $Owner
        $payloadSize = switch ($vt) {
            { $_ -in @(0, 1, 24) } { [uint64]2; break }
            { $_ -in @(2, 11) } { [uint64]4; break }
            { $_ -in @(5, 6, 7, 20, 21, 64) } { [uint64]10; break }
            8 {
                if ($customDataPosition + 6 -gt [uint64]$Segments[11].Length) {
                    throw "$Owner MSFT custom-data segment has a truncated BSTR record header"
                }
                [uint64]6 + [uint64](Read-WindowsFixtureUInt32 -Bytes $Bytes -Offset ($absolute + 2) -Owner $Owner)
                break
            }
            14 { [uint64]18; break }
            default { [uint64]6 }
        }
        if ($payloadSize -lt 2 -or
            $customDataPosition + $payloadSize -lt $customDataPosition -or
            $customDataPosition + $payloadSize -gt [uint64]$Segments[11].Length) {
            throw "$Owner MSFT custom-data segment contains a truncated value record"
        }
        $recordSize = $payloadSize + ((4 - ($payloadSize % 4)) % 4)
        $recordEnd = $customDataPosition + $recordSize
        if ($recordEnd -lt $customDataPosition -or $recordEnd -gt [uint64]$Segments[11].Length) {
            throw "$Owner MSFT custom-data segment ends with a trailing partial aligned record"
        }
        [void]$customDataValueStarts.Add([int32]$customDataPosition)
        $customDataValueIntervals.Add([pscustomobject]@{
            Start = [uint64]$customDataPosition
            PayloadEnd = [uint64]($customDataPosition + $payloadSize)
            End = [uint64]$recordEnd
        })
        $customDataPosition = $recordEnd
    }
    $expectedCustomDataStart = [uint64]0
    foreach ($interval in $customDataValueIntervals) {
        if ([uint64]$interval.Start -ne $expectedCustomDataStart -or
            [uint64]$interval.PayloadEnd -gt [uint64]$interval.End -or
            [uint64]$interval.End -le [uint64]$interval.Start) {
            throw "$Owner MSFT custom-data record intervals contain a gap or overlap"
        }
        $expectedCustomDataStart = [uint64]$interval.End
    }
    if ($expectedCustomDataStart -ne [uint64]$Segments[11].Length) {
        throw "$Owner MSFT custom-data segment has unowned trailing bytes"
    }

    $assertCustValue = {
        param([int32]$Value, [string]$ReferenceOwner)
        if ($Value -lt 0 -or -not $customDataValueStarts.Contains($Value)) {
            throw "$Owner MSFT $ReferenceOwner does not identify an exact custom-data record start"
        }
    }

    $customDataEntries = @{}
    for ($offset = 0; $offset -lt [int]$Segments[12].Length; $offset += 12) {
        $absolute = [int]$Segments[12].Offset + $offset
        $entry = [pscustomobject]@{
            Offset = [int32]$offset
            Guid = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset $absolute -Owner $Owner
            Data = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($absolute + 4) -Owner $Owner
            Next = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($absolute + 8) -Owner $Owner
        }
        $customDataEntries[[string]$offset] = $entry
        & $assertFixedReference $entry.Guid 5 24 24 $false "custom-data GUID[$($offset / 12)]"
        & $assertCustValue $entry.Data "custom-data value[$($offset / 12)]"
        & $assertFixedReference $entry.Next 12 12 12 $true "custom-data next[$($offset / 12)]"
    }
    $assertCustomDataChain = {
        param([int32]$Value, [bool]$AllowMissing, [string]$ReferenceOwner)
        if ($Value -eq -1 -and $AllowMissing) {
            return
        }
        & $assertFixedReference $Value 12 12 12 $false $ReferenceOwner
        $seen = [Collections.Generic.HashSet[int32]]::new()
        $next = $Value
        while ($next -ne -1) {
            if (-not $seen.Add($next)) {
                throw "$Owner MSFT $ReferenceOwner custom-data chain contains a cycle"
            }
            $next = [int32]$customDataEntries[[string]$next].Next
        }
    }

    # Validate every reference-table entry, then verify each coclass chain
    # starts at datatype1 and contains exactly cImplTypes complete records.
    $referenceEntries = @{}
    for ($offset = 0; $offset -lt [int]$Segments[3].Length; $offset += 16) {
        $absolute = [int]$Segments[3].Offset + $offset
        $entry = [pscustomobject]@{
            RefType = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset $absolute -Owner $Owner
            Next = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($absolute + 12) -Owner $Owner
        }
        $referenceEntries[[string]$offset] = $entry
        & $assertHref $entry.RefType "reference-table hreftype[$($offset / 16)]"
        & $assertFixedReference $entry.Next 3 16 16 $true "reference-table next[$($offset / 16)]"
    }

    & $assertFixedReference `
        (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset 0x08 -Owner $Owner) `
        5 24 24 $false "library GUID"
    & $assertNameReference `
        (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset 0x38 -Owner $Owner) `
        $false "library name"
    & $assertStringReference `
        (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset 0x24 -Owner $Owner) `
        $true "library help string"
    & $assertStringReference `
        (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset 0x3C -Owner $Owner) `
        $true "library help file"
    & $assertCustomDataChain `
        (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset 0x40 -Owner $Owner) `
        $true "library custom data"
    if (($VarFlags -band 0x100) -ne 0) {
        $helpDllOffsetPosition = 0x54 + ($TypeInfos.Count * 4)
        & $assertStringReference `
            (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset $helpDllOffsetPosition -Owner $Owner) `
            $true "library help DLL"
    }

    $memberIntervals = [Collections.Generic.List[object]]::new()
    foreach ($typeInfo in $TypeInfos) {
        $recordOffset = [int]$typeInfo.RecordOffset
        & $assertFixedReference `
            (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($recordOffset + 0x2C) -Owner $Owner) `
            5 24 24 $true "TypeInfo[$($typeInfo.Index)] GUID"
        & $assertNameReference `
            (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($recordOffset + 0x34) -Owner $Owner) `
            $false "TypeInfo[$($typeInfo.Index)] name"
        & $assertStringReference `
            (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($recordOffset + 0x3C) -Owner $Owner) `
            $true "TypeInfo[$($typeInfo.Index)] doc string"
        & $assertCustomDataChain `
            (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($recordOffset + 0x48) -Owner $Owner) `
            $true "TypeInfo[$($typeInfo.Index)] custom data"

        if ([uint32]$typeInfo.TypeKind -eq 6) {
            $aliasType = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($recordOffset + 0x54) -Owner $Owner
            if ($aliasType -ge 0) {
                & $assertTypeDesc $aliasType "TypeInfo[$($typeInfo.Index)] alias type"
            }
        }
        if ([uint32]$typeInfo.TypeKind -eq 5) {
            $expectedReferences = [int]$typeInfo.ImplementationCount
            $next = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($recordOffset + 0x54) -Owner $Owner
            $seen = [Collections.Generic.HashSet[int32]]::new()
            for ($referenceIndex = 0; $referenceIndex -lt $expectedReferences; $referenceIndex++) {
                & $assertFixedReference $next 3 16 16 $false "TypeInfo[$($typeInfo.Index)] implemented reference[$referenceIndex]"
                if (-not $seen.Add($next)) {
                    throw "$Owner MSFT TypeInfo[$($typeInfo.Index)] implemented-reference chain contains a cycle"
                }
                $next = [int32]$referenceEntries[[string]$next].Next
            }
            if ($next -ne -1) {
                throw "$Owner MSFT TypeInfo[$($typeInfo.Index)] implemented-reference count does not terminate its chain"
            }
        }

        $memberCount = [uint64]$typeInfo.FunctionCount + [uint64]$typeInfo.VariableCount
        $memberOffset = [int64]$typeInfo.MemberOffset
        if ($memberCount -eq 0) {
            if ($memberOffset -ne -1 -and
                ([uint64]$memberOffset -lt $segmentDataEnd -or [uint64]$memberOffset -gt $fileLength)) {
                throw "$Owner MSFT TypeInfo[$($typeInfo.Index)] empty member-data offset is outside the post-segment file range"
            }
            continue
        }
        if ($memberOffset -lt 0 -or [uint64]$memberOffset -lt $segmentDataEnd -or
            [uint64]$memberOffset + 4 -gt $fileLength) {
            throw "$Owner MSFT TypeInfo[$($typeInfo.Index)] member-data offset does not identify a complete post-segment block"
        }
        $blockSize = [uint64](Read-WindowsFixtureUInt32 -Bytes $Bytes -Offset ([int]$memberOffset) -Owner $Owner)
        $recordsStart = [uint64]$memberOffset + 4
        $recordsEnd = $recordsStart + $blockSize
        $auxiliarySize = $memberCount * 12
        $memberEnd = $recordsEnd + $auxiliarySize
        if ($recordsEnd -lt $recordsStart -or $memberEnd -lt $recordsEnd -or $memberEnd -gt $fileLength) {
            throw "$Owner MSFT TypeInfo[$($typeInfo.Index)] member records/auxiliary arrays exceed fileLength"
        }

        $recordStarts = [Collections.Generic.List[uint32]]::new()
        $position = $recordsStart
        for ($functionIndex = 0; $functionIndex -lt [int]$typeInfo.FunctionCount; $functionIndex++) {
            if ($position + 24 -gt $recordsEnd) {
                throw "$Owner MSFT TypeInfo[$($typeInfo.Index)] function[$functionIndex] record is truncated"
            }
            $recordSize = [uint64]((Read-WindowsFixtureUInt32 -Bytes $Bytes -Offset ([int]$position) -Owner $Owner) -band 0xFFFF)
            $argumentCount = Read-WindowsFixtureInt16 -Bytes $Bytes -Offset ([int]$position + 0x14) -Owner $Owner
            $optionalCount = Read-WindowsFixtureInt16 -Bytes $Bytes -Offset ([int]$position + 0x16) -Owner $Owner
            $fkccic = Read-WindowsFixtureUInt32 -Bytes $Bytes -Offset ([int]$position + 0x10) -Owner $Owner
            $argumentCustomSize = if (($fkccic -band 0x1000) -ne 0) { [uint64]$argumentCount * 4 } else { [uint64]0 }
            if ($recordSize -lt 24 -or $recordSize % 4 -ne 0 -or $position + $recordSize -gt $recordsEnd -or
                $argumentCount -lt 0 -or $argumentCount -gt 1024 -or
                $optionalCount -lt -1 -or $optionalCount -gt $argumentCount -or
                $recordSize -lt 24 + ([uint64]$argumentCount * 12) + $argumentCustomSize) {
                throw "$Owner MSFT TypeInfo[$($typeInfo.Index)] function[$functionIndex] layout is invalid or unbounded"
            }
            $attributeBytes = $recordSize - 24 - ([uint64]$argumentCount * 12) - $argumentCustomSize
            if ($attributeBytes % 4 -ne 0 -or $attributeBytes / 4 -gt 6) {
                throw "$Owner MSFT TypeInfo[$($typeInfo.Index)] function[$functionIndex] attribute layout is invalid"
            }
            [void]$recordStarts.Add([uint32]($position - $recordsStart))
            $dataType = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ([int]$position + 4) -Owner $Owner
            if ($dataType -ge 0) { & $assertTypeDesc $dataType "function[$functionIndex] return type" }
            $attributeCount = [int]($attributeBytes / 4)
            if ($attributeCount -gt 1) {
                & $assertStringReference `
                    (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ([int]$position + 0x1C) -Owner $Owner) `
                    $true "function[$functionIndex] help string"
            }
            if ($attributeCount -gt 3) {
                & $assertNameReference `
                    (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ([int]$position + 0x24) -Owner $Owner) `
                    $true "function[$functionIndex] name"
            }
            if ($attributeCount -gt 5) {
                & $assertCustomDataChain `
                    (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ([int]$position + 0x2C) -Owner $Owner) `
                    $true "function[$functionIndex] custom data"
            }
            $argumentCustomStart = $position + 24 + $attributeBytes
            if (($fkccic -band 0x1000) -ne 0) {
                for ($argumentIndex = 0; $argumentIndex -lt $argumentCount; $argumentIndex++) {
                    & $assertCustomDataChain `
                        (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ([int]($argumentCustomStart + ($argumentIndex * 4))) -Owner $Owner) `
                        $true "function[$functionIndex] argument[$argumentIndex] custom data"
                }
            }
            $parametersStart = $position + $recordSize - ([uint64]$argumentCount * 12)
            for ($argumentIndex = 0; $argumentIndex -lt $argumentCount; $argumentIndex++) {
                $parameterOffset = [int]($parametersStart + ([uint64]$argumentIndex * 12))
                $parameterType = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset $parameterOffset -Owner $Owner
                if ($parameterType -ge 0) { & $assertTypeDesc $parameterType "function[$functionIndex] argument[$argumentIndex] type" }
                & $assertNameReference `
                    (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ($parameterOffset + 4) -Owner $Owner) `
                    $true "function[$functionIndex] argument[$argumentIndex] name"
            }
            $position += $recordSize
        }
        for ($variableIndex = 0; $variableIndex -lt [int]$typeInfo.VariableCount; $variableIndex++) {
            if ($position + 20 -gt $recordsEnd) {
                throw "$Owner MSFT TypeInfo[$($typeInfo.Index)] variable[$variableIndex] record is truncated"
            }
            $recordSize = [uint64]((Read-WindowsFixtureUInt32 -Bytes $Bytes -Offset ([int]$position) -Owner $Owner) -band 0xFFFF)
            if ($recordSize -lt 20 -or $recordSize % 4 -ne 0 -or $position + $recordSize -gt $recordsEnd -or
                ($recordSize - 20) / 4 -gt 5) {
                throw "$Owner MSFT TypeInfo[$($typeInfo.Index)] variable[$variableIndex] layout is invalid"
            }
            [void]$recordStarts.Add([uint32]($position - $recordsStart))
            $dataType = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ([int]$position + 4) -Owner $Owner
            if ($dataType -ge 0) { & $assertTypeDesc $dataType "variable[$variableIndex] type" }
            $variableKind = Read-WindowsFixtureInt16 -Bytes $Bytes -Offset ([int]$position + 0x0C) -Owner $Owner
            if ($variableKind -lt 0 -or $variableKind -gt 3) {
                throw "$Owner MSFT TypeInfo[$($typeInfo.Index)] variable[$variableIndex] kind is invalid"
            }
            if ($variableKind -eq 2) {
                $valueOffset = Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ([int]$position + 0x10) -Owner $Owner
                if ($valueOffset -ge 0) { & $assertCustValue $valueOffset "variable[$variableIndex] constant value" }
            }
            $attributeCount = [int](($recordSize - 20) / 4)
            if ($attributeCount -gt 1) {
                & $assertStringReference `
                    (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ([int]$position + 0x18) -Owner $Owner) `
                    $true "variable[$variableIndex] help string"
            }
            if ($attributeCount -gt 3) {
                & $assertCustomDataChain `
                    (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ([int]$position + 0x20) -Owner $Owner) `
                    $true "variable[$variableIndex] custom data"
            }
            $position += $recordSize
        }
        if ($position -ne $recordsEnd) {
            throw "$Owner MSFT TypeInfo[$($typeInfo.Index)] block_size does not equal its complete member records"
        }

        $auxiliaryStart = $recordsEnd
        $functionCount = [uint64]$typeInfo.FunctionCount
        $variableCount = [uint64]$typeInfo.VariableCount
        for ($memberIndex = 0; $memberIndex -lt [int]$memberCount; $memberIndex++) {
            $nameArrayIndex = if ($memberIndex -lt $functionCount) {
                $functionCount + $variableCount + [uint64]$memberIndex
            }
            else {
                (2 * $functionCount) + $variableCount + ([uint64]$memberIndex - $functionCount)
            }
            & $assertNameReference `
                (Read-WindowsFixtureInt32 -Bytes $Bytes -Offset ([int]($auxiliaryStart + ($nameArrayIndex * 4))) -Owner $Owner) `
                $false "TypeInfo[$($typeInfo.Index)] member[$memberIndex] auxiliary name"
            $recordArrayIndex = (2 * $functionCount) + (2 * $variableCount) + [uint64]$memberIndex
            $recordRelative = Read-WindowsFixtureUInt32 `
                -Bytes $Bytes `
                -Offset ([int]($auxiliaryStart + ($recordArrayIndex * 4))) `
                -Owner $Owner
            if ($recordRelative -ne $recordStarts[$memberIndex]) {
                throw "$Owner MSFT TypeInfo[$($typeInfo.Index)] member[$memberIndex] auxiliary record offset is not a record start"
            }
        }
        $memberIntervals.Add([pscustomobject]@{
            Start = [uint64]$memberOffset
            End = [uint64]$memberEnd
            TypeInfo = [int]$typeInfo.Index
        })
    }
    $orderedMemberIntervals = @($memberIntervals | Sort-Object Start, End)
    for ($index = 1; $index -lt $orderedMemberIntervals.Count; $index++) {
        if ([uint64]$orderedMemberIntervals[$index].Start -lt [uint64]$orderedMemberIntervals[$index - 1].End) {
            throw "$Owner MSFT TypeInfo member-data blocks overlap"
        }
    }
}

function Assert-WindowsFixtureMsftTypeLib {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if (-not [IO.Path]::GetExtension($Path).Equals(".tlb", [StringComparison]::OrdinalIgnoreCase)) {
        throw "$Owner MSFT typelib must use extension '.tlb'"
    }
    $fileLength = (Get-Item -LiteralPath $Path).Length
    if ($fileLength -lt 1024 -or $fileLength -gt 16777216) {
        throw "$Owner MSFT type library length is outside the controlled 1 KiB..16 MiB envelope"
    }
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 0x148 -or
        (Read-WindowsFixtureUInt32 -Bytes $bytes -Offset 0 -Owner $Owner) -ne 0x5446534D -or
        (Read-WindowsFixtureUInt32 -Bytes $bytes -Offset 4 -Owner $Owner) -ne 0x00010002) {
        throw "$Owner is not a complete MSFT-format type library"
    }
    $varFlags = Read-WindowsFixtureUInt32 -Bytes $bytes -Offset 0x14 -Owner $Owner
    if (($varFlags -band 0x0F) -ne 3) {
        throw "$Owner MSFT type library must declare SYS_WIN64"
    }
    $typeInfoCount = [uint64](Read-WindowsFixtureUInt32 -Bytes $bytes -Offset 0x20 -Owner $Owner)
    $nameCount = [uint64](Read-WindowsFixtureUInt32 -Bytes $bytes -Offset 0x30 -Owner $Owner)
    $nameCharacters = [uint64](Read-WindowsFixtureUInt32 -Bytes $bytes -Offset 0x34 -Owner $Owner)
    $importInfoCount = [uint64](Read-WindowsFixtureUInt32 -Bytes $bytes -Offset 0x50 -Owner $Owner)
    if ($typeInfoCount -lt 1 -or $typeInfoCount -gt 4096 -or
        $nameCount -lt 1 -or $nameCount -gt 65536 -or
        $nameCharacters -lt 1 -or $nameCharacters -gt 16777216 -or
        $importInfoCount -gt 65536) {
        throw "$Owner MSFT header counts are invalid or unbounded"
    }
    $baseDirectoryOffset = [uint64]0x54 + ($typeInfoCount * 4)
    $directoryOffset = $baseDirectoryOffset
    if (($varFlags -band 0x100) -ne 0) {
        $directoryOffset += 4
    }

    $validLayouts = [Collections.Generic.List[object]]::new()
    foreach ($directoryOffsetValue in @($directoryOffset)) {
        $directoryOffset = [uint64]$directoryOffsetValue
        $directoryEnd = $directoryOffset + (15 * 16)
        if ($directoryEnd -gt [uint64]$bytes.Length) {
            continue
        }
        $segments = [Collections.Generic.List[object]]::new()
        $layoutValid = $true
        for ($segmentIndex = 0; $segmentIndex -lt 15; $segmentIndex++) {
            $entryOffset = [int]($directoryOffset + ($segmentIndex * 16))
            $segmentOffset = Read-WindowsFixtureUInt32 -Bytes $bytes -Offset $entryOffset -Owner $Owner
            $segmentLength = Read-WindowsFixtureUInt32 -Bytes $bytes -Offset ($entryOffset + 4) -Owner $Owner
            $reserved1 = Read-WindowsFixtureUInt32 -Bytes $bytes -Offset ($entryOffset + 8) -Owner $Owner
            $reserved2 = Read-WindowsFixtureUInt32 -Bytes $bytes -Offset ($entryOffset + 12) -Owner $Owner
            if ($reserved1 -ne [uint32]::MaxValue -or $reserved2 -ne 0x0F) {
                $layoutValid = $false
                break
            }
            if ($segmentOffset -eq [uint32]::MaxValue) {
                if ($segmentLength -ne 0) {
                    $layoutValid = $false
                    break
                }
                $segments.Add([pscustomobject]@{ Index = $segmentIndex; Offset = [uint64]0; Length = [uint64]0; End = [uint64]0 })
                continue
            }
            $offset64 = [uint64]$segmentOffset
            $length64 = [uint64]$segmentLength
            $end64 = $offset64 + $length64
            if ($length64 -eq 0 -or $offset64 -lt $directoryEnd -or $offset64 % 4 -ne 0 -or
                $length64 % 4 -ne 0 -or $end64 -gt [uint64]$bytes.Length -or $end64 -lt $offset64) {
                $layoutValid = $false
                break
            }
            $segments.Add([pscustomobject]@{ Index = $segmentIndex; Offset = $offset64; Length = $length64; End = $end64 })
        }
        if (-not $layoutValid -or $segments.Count -ne 15) {
            continue
        }
        $nonEmpty = @($segments | Where-Object Length -gt 0 | Sort-Object Offset, End)
        for ($index = 1; $index -lt $nonEmpty.Count; $index++) {
            if ([uint64]$nonEmpty[$index].Offset -lt [uint64]$nonEmpty[$index - 1].End) {
                $layoutValid = $false
                break
            }
        }
        if ($layoutValid) {
            $validLayouts.Add([pscustomobject]@{
                DirectoryOffset = $directoryOffset
                DirectoryEnd = $directoryEnd
                Segments = @($segments)
            })
        }
    }
    if ($validLayouts.Count -ne 1) {
        throw "$Owner MSFT segment directory is missing, ambiguous, overlapping, or out of bounds"
    }
    $layout = $validLayouts[0]
    $segments = @($layout.Segments)
    $typeInfoSegment = $segments[0]
    if ($typeInfoSegment.Length -ne $typeInfoCount * 100) {
        throw "$Owner MSFT TypeInfo segment does not match the header count"
    }
    $seenTypeInfoOffsets = [Collections.Generic.HashSet[uint32]]::new()
    $typeInfos = [Collections.Generic.List[object]]::new()
    for ($index = 0; $index -lt $typeInfoCount; $index++) {
        $offset = Read-WindowsFixtureUInt32 -Bytes $bytes -Offset (0x54 + ($index * 4)) -Owner $Owner
        if ($offset % 100 -ne 0 -or [uint64]$offset + 100 -gt [uint64]$typeInfoSegment.Length -or
            -not $seenTypeInfoOffsets.Add($offset)) {
            throw "$Owner MSFT TypeInfo offset[$index] is duplicate, misaligned, or out of bounds"
        }
        $recordOffset = [int]([uint64]$typeInfoSegment.Offset + [uint64]$offset)
        $typeKind = (Read-WindowsFixtureUInt32 -Bytes $bytes -Offset $recordOffset -Owner $Owner) -band 0x0F
        $memberOffset = Read-WindowsFixtureInt32 -Bytes $bytes -Offset ($recordOffset + 4) -Owner $Owner
        $elementCount = Read-WindowsFixtureUInt32 -Bytes $bytes -Offset ($recordOffset + 0x18) -Owner $Owner
        $functionCount = [uint32]($elementCount -band 0xFFFF)
        $variableCount = [uint32]($elementCount -shr 16)
        $implementationCount = Read-WindowsFixtureInt16 -Bytes $bytes -Offset ($recordOffset + 0x4C) -Owner $Owner
        if ($typeKind -gt 8 -or $implementationCount -lt 0 -or $implementationCount -gt 4096 -or
            ($memberOffset -lt -1 -or [int64]$memberOffset -gt [int64]$bytes.Length) -or
            (Read-WindowsFixtureInt32 -Bytes $bytes -Offset ($recordOffset + 0x5C) -Owner $Owner) -ne 0 -or
            (Read-WindowsFixtureInt32 -Bytes $bytes -Offset ($recordOffset + 0x60) -Owner $Owner) -ne -1) {
            throw "$Owner MSFT TypeInfo record[$index] has an invalid kind or member-data offset"
        }
        $typeInfos.Add([pscustomobject]@{
            Index = $index
            RelativeOffset = [uint32]$offset
            RecordOffset = $recordOffset
            TypeKind = [uint32]$typeKind
            MemberOffset = [int32]$memberOffset
            FunctionCount = [uint32]$functionCount
            VariableCount = [uint32]$variableCount
            ImplementationCount = [int32]$implementationCount
        })
    }

    if ($segments[4].Length -ne 0x80 -or
        $segments[5].Length -le 0 -or $segments[5].Length % 24 -ne 0 -or
        $segments[6].Length -ne 0x200 -or
        $segments[7].Length -le 0 -or
        $segments[3].Length % 16 -ne 0 -or
        $segments[9].Length % 8 -ne 0 -or
        $segments[12].Length % 12 -ne 0 -or
        $segments[13].Length -ne 0 -or $segments[14].Length -ne 0) {
        throw "$Owner MSFT GUID/name/string tables are missing or malformed"
    }
    if (($importInfoCount -eq 0 -and $segments[1].Length -ne 0) -or
        ($importInfoCount -gt 0 -and $segments[1].Length -ne $importInfoCount * 12)) {
        throw "$Owner MSFT import-info table does not match the header count"
    }

    $guidOffset = Read-WindowsFixtureUInt32 -Bytes $bytes -Offset 0x08 -Owner $Owner
    $nameOffset = Read-WindowsFixtureUInt32 -Bytes $bytes -Offset 0x38 -Owner $Owner
    foreach ($reference in @(
        @{ Name = "library GUID"; Value = $guidOffset; Segment = $segments[5] },
        @{ Name = "library name"; Value = $nameOffset; Segment = $segments[7] }
    )) {
        if ($reference.Value -eq [uint32]::MaxValue -or
            [uint64]$reference.Value -ge [uint64]$reference.Segment.Length) {
            throw "$Owner MSFT $($reference.Name) reference is out of bounds"
        }
    }
    if ($guidOffset % 24 -ne 0 -or [uint64]$guidOffset + 24 -gt [uint64]$segments[5].Length) {
        throw "$Owner MSFT library GUID entry is misaligned or truncated"
    }
    $readName = {
        param([uint32]$Offset, [string]$NameOwner)

        if ([uint64]$Offset + 12 -gt [uint64]$segments[7].Length) {
            throw "$Owner MSFT $NameOwner name entry is truncated"
        }
        $absoluteOffset = [int]([uint64]$segments[7].Offset + [uint64]$Offset)
        $length = [uint64]$bytes[$absoluteOffset + 8]
        if ([uint64]$Offset + 12 + $length -gt [uint64]$segments[7].Length) {
            throw "$Owner MSFT $NameOwner name entry is truncated"
        }
        try {
            return [Text.UTF8Encoding]::new($false, $true).GetString($bytes, $absoluteOffset + 12, [int]$length)
        }
        catch {
            throw "$Owner MSFT $NameOwner name entry is not strict UTF-8"
        }
    }
    [void](& $readName $nameOffset "library")
    foreach ($offsetField in @(0x24, 0x3C)) {
        $stringOffset = Read-WindowsFixtureUInt32 -Bytes $bytes -Offset $offsetField -Owner $Owner
        if ($stringOffset -eq [uint32]::MaxValue) {
            continue
        }
        if ([uint64]$stringOffset + 2 -gt [uint64]$segments[8].Length) {
            throw "$Owner MSFT string reference is out of bounds"
        }
        $absoluteStringOffset = [int]([uint64]$segments[8].Offset + [uint64]$stringOffset)
        $stringLength = Read-WindowsFixtureUInt16 -Bytes $bytes -Offset $absoluteStringOffset -Owner $Owner
        if ([uint64]$stringOffset + 2 + [uint64]$stringLength -gt [uint64]$segments[8].Length) {
            throw "$Owner MSFT string entry is truncated"
        }
    }

    Assert-WindowsFixtureMsftOwnedRecords `
        -Bytes $bytes `
        -Segments $segments `
        -TypeInfos @($typeInfos) `
        -NameCount $nameCount `
        -NameCharacters $nameCharacters `
        -ImportInfoCount $importInfoCount `
        -VarFlags $varFlags `
        -Owner $Owner

    if ([Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([Runtime.InteropServices.OSPlatform]::Windows)) {
        Initialize-WindowsFixtureNativeProbe
        $typeLib = [IntPtr]::Zero
        try {
            $hresult = [OxVbaFixtureAdmissionNativeV1]::LoadTypeLibEx($Path, 2, [ref]$typeLib)
            if ($hresult -lt 0 -or $typeLib -eq [IntPtr]::Zero) {
                $hresultBits = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int32]$hresult), 0)
                throw ("$Owner was rejected by LoadTypeLibEx(REGKIND_NONE) (HRESULT=0x{0:X8})" -f $hresultBits)
            }
        }
        finally {
            if ($typeLib -ne [IntPtr]::Zero) {
                [void][Runtime.InteropServices.Marshal]::Release($typeLib)
            }
        }
    }
}

function Assert-WindowsFixtureVbaSource {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ([IO.Path]::GetExtension($Path) -notin @(".bas", ".cls", ".frm")) {
        throw "$Owner VBA source must use .bas, .cls, or .frm"
    }
    $bytes = [IO.File]::ReadAllBytes($Path)
    $text = ConvertTo-WindowsFixtureCanonicalText -Bytes $bytes -Owner $Owner
    if ([string]::IsNullOrWhiteSpace($text)) {
        throw "$Owner VBA source cannot be blank"
    }
}

function Assert-WindowsFixtureBundleArtifact {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath,
        [Parameter(Mandatory = $true)][string]$ArtifactRoot,
        [Parameter(Mandatory = $true)][string]$MatrixId,
        [Parameter(Mandatory = $true)][string]$RowId,
        [Parameter(Mandatory = $true)][string]$FixtureId,
        [Parameter(Mandatory = $true)][string]$ArtifactId,
        [Parameter(Mandatory = $true)][string]$ExpectedComponents,
        [Parameter(Mandatory = $true)][string]$ExpectedComponentConstraints,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $path = Assert-WindowsFixtureContainedPath -RepositoryRoot $RepositoryRoot -RelativePath $RelativePath -ControlledRoot $ArtifactRoot -Owner $Owner
    $bytes = [IO.File]::ReadAllBytes($path)
    $bundle = ConvertFrom-WindowsFixtureAuditedJson `
        -Bytes $bytes `
        -Owner $Owner `
        -FormatName "fixture-bundle"
    Assert-WindowsFixtureExactJsonProperties -Value $bundle -Expected @(
        "schema_id", "schema_version", "matrix_id", "row_id", "fixture_id",
        "artifact_id", "target_arch", "artifact_class", "components"
    ) -Owner $Owner
    Assert-WindowsFixtureJsonStringProperties -Value $bundle -Properties @(
        "schema_id", "matrix_id", "row_id", "fixture_id", "artifact_id",
        "target_arch", "artifact_class"
    ) -Owner $Owner
    if (-not (Test-WindowsFixtureJsonInteger -Value $bundle.schema_version) -or
        [int64]$bundle.schema_version -ne 1 -or $bundle.components -isnot [Array]) {
        throw "$Owner schema_version must be JSON integer 1 and components must be a JSON array"
    }
    if ([string]$bundle.schema_id -ne "oxvba-windows-x64-fixture-bundle-v1" -or
        [string]$bundle.matrix_id -ne $MatrixId -or
        [string]$bundle.row_id -ne $RowId -or
        [string]$bundle.fixture_id -ne $FixtureId -or
        [string]$bundle.artifact_id -ne $ArtifactId -or
        [string]$bundle.target_arch -ne "x64" -or
        [string]$bundle.artifact_class -ne "fixture-bundle-json-v1") {
        throw "$Owner identity/schema does not match its controlled Windows row"
    }

    $components = @($bundle.components)
    $expectedKinds = @($ExpectedComponents -split '\|')
    $expectedConstraints = if ($ExpectedComponentConstraints -eq "n/a") {
        @($expectedKinds | ForEach-Object { "n/a" })
    }
    else {
        @($ExpectedComponentConstraints -split '\|')
    }
    if ($components.Count -ne $expectedKinds.Count) {
        throw "$Owner component count must be $($expectedKinds.Count), found $($components.Count)"
    }
    if ($expectedConstraints.Count -ne $expectedKinds.Count -or
        @($expectedConstraints | Where-Object { $_ -ne "n/a" -and $_ -notmatch '^sha256:[0-9a-f]{64}$' }).Count -gt 0) {
        throw "$Owner controller-owned component constraints are malformed"
    }
    $seenIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seenPaths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    for ($index = 0; $index -lt $components.Count; $index++) {
        $component = $components[$index]
        Assert-WindowsFixtureExactJsonProperties -Value $component -Expected @(
            "component_id", "kind", "relative_path", "sha256"
        ) -Owner "$Owner component[$index]"
        Assert-WindowsFixtureJsonStringProperties -Value $component -Properties @(
            "component_id", "kind", "relative_path", "sha256"
        ) -Owner "$Owner component[$index]"
        $componentId = [string]$component.component_id
        $kind = [string]$component.kind
        $componentRelative = ([string]$component.relative_path).Replace('\', '/')
        if ($kind -ne $expectedKinds[$index]) {
            throw "$Owner component[$index] kind must be '$($expectedKinds[$index])', found '$kind'"
        }
        if ($componentId -notmatch '^[a-z0-9]+(?:-[a-z0-9]+)*-v[1-9][0-9]*$' -or
            (Test-WindowsFixtureMutableIdentity -Value $componentId) -or -not $seenIds.Add($componentId)) {
            throw "$Owner component[$index] has mutable, malformed, or duplicate component_id '$componentId'"
        }
        $extension = switch ($kind) {
            "pe-dll-x64" { ".dll" }
            "pe-exe-x64" { ".exe" }
            "msft-tlb-v1" { ".tlb" }
            "vba-source-utf8-v1" { ".bas" }
            default { throw "$Owner component[$index] has unsupported kind '$kind'" }
        }
        $expectedRelative = "components/$componentId$extension"
        if ($componentRelative -cne $expectedRelative -or
            (Test-WindowsFixtureMutableIdentity -Value $componentRelative) -or -not $seenPaths.Add($componentRelative)) {
            throw "$Owner component[$index] path must be immutable controlled name '$expectedRelative'"
        }
        $componentRepoPath = "$ArtifactRoot/$componentRelative"
        $componentPath = Assert-WindowsFixtureContainedPath -RepositoryRoot $RepositoryRoot -RelativePath $componentRepoPath -ControlledRoot $ArtifactRoot -Owner "$Owner component[$index]"
        $actualHash = Get-WindowsFixtureRawFileHash -RepositoryRoot $RepositoryRoot -RelativePath $componentRepoPath
        if ([string]$component.sha256 -notmatch '^sha256:[0-9a-f]{64}$' -or
            [string]$component.sha256 -cne $actualHash) {
            throw "$Owner component[$index] hash is malformed, forged, or stale"
        }
        if ($expectedConstraints[$index] -ne "n/a" -and
            $actualHash -cne $expectedConstraints[$index]) {
            throw "$Owner component[$index] bytes do not match the controller-owned digest constraint"
        }
        switch ($kind) {
            "pe-dll-x64" { Assert-WindowsFixturePeFile -Path $componentPath -ExpectedKind $kind -Owner "$Owner component[$index]" }
            "pe-exe-x64" { Assert-WindowsFixturePeFile -Path $componentPath -ExpectedKind $kind -Owner "$Owner component[$index]" }
            "msft-tlb-v1" { Assert-WindowsFixtureMsftTypeLib -Path $componentPath -Owner "$Owner component[$index]" }
            "vba-source-utf8-v1" { Assert-WindowsFixtureVbaSource -Path $componentPath -Owner "$Owner component[$index]" }
        }
    }
}

function Test-WindowsFixturePlaceholder {
    param([AllowEmptyString()][string]$Value)

    return [string]::IsNullOrWhiteSpace($Value) -or
        $Value -match '(?i)(?:^|[-_:@])(?:planned|pending|latest|current|mutable|unknown|unresolved)(?:$|[-_:@])' -or
        $Value -match '(?i)no-clean-snapshot'
}

function Get-WindowsFixtureResetPolicyHash {
    param([Parameter(Mandatory = $true)][string]$ResetPolicy)

    return Get-WindowsFixtureSha256Text -Text ($ResetPolicy.Replace("`r`n", "`n").Replace("`r", "`n"))
}

function Assert-WindowsFixtureEnvironmentCapture {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath,
        [Parameter(Mandatory = $true)][string]$CaptureRoot,
        [Parameter(Mandatory = $true)]$Environment,
        [Parameter(Mandatory = $true)][string]$ExpectedSchema,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $path = Assert-WindowsFixtureContainedPath -RepositoryRoot $RepositoryRoot -RelativePath $RelativePath -ControlledRoot $CaptureRoot -Owner $Owner
    $bytes = [IO.File]::ReadAllBytes($path)
    $capture = ConvertFrom-WindowsFixtureAuditedJson `
        -Bytes $bytes `
        -Owner $Owner `
        -FormatName "environment-capture"
    Assert-WindowsFixtureEnvironmentCaptureValue `
        -Capture $capture `
        -Environment $Environment `
        -ExpectedSchema $ExpectedSchema `
        -Owner $Owner
}

function Assert-WindowsFixtureEnvironmentCaptureValue {
    param(
        [Parameter(Mandatory = $true)]$Capture,
        [Parameter(Mandatory = $true)]$Environment,
        [Parameter(Mandatory = $true)][string]$ExpectedSchema,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $capture = $Capture
    Assert-WindowsFixtureExactJsonProperties -Value $capture -Expected @(
        "schema_id", "schema_version", "capture_id", "environment_id", "role",
        "profile", "target_arch", "os_build", "office_product", "office_version",
        "office_build", "office_channel", "office_bitness", "locale",
        "snapshot_or_image", "reset_policy", "reset_policy_hash", "evidence_state",
        "certification_authority", "noncertifying"
    ) -Owner $Owner
    Assert-WindowsFixtureJsonStringProperties -Value $capture -Properties @(
        "schema_id", "capture_id", "environment_id", "role", "profile",
        "target_arch", "os_build", "office_product", "office_version",
        "office_build", "office_channel", "office_bitness", "locale",
        "snapshot_or_image", "reset_policy", "reset_policy_hash",
        "evidence_state"
    ) -Owner $Owner
    if (-not (Test-WindowsFixtureJsonInteger -Value $capture.schema_version) -or
        [int64]$capture.schema_version -ne 1 -or
        $capture.certification_authority -isnot [bool] -or
        $capture.noncertifying -isnot [bool]) {
        throw "$Owner schema_version must be JSON integer 1 and authority flags must be JSON booleans"
    }

    $environmentId = [string]$Environment.environment_id
    if ([string]$capture.schema_id -ne $ExpectedSchema) {
        throw "$Owner schema must be '$ExpectedSchema' version 1"
    }
    if ([string]$capture.environment_id -ne $environmentId -or
        [string]$capture.capture_id -ne "$environmentId-capture-v1") {
        throw "$Owner environment/capture identity does not match '$environmentId'"
    }
    if ([string]$capture.role -ne [string]$Environment.role) {
        throw "$Owner role does not match environment manifest role '$($Environment.role)'"
    }
    if ([string]$capture.target_arch -ne "x64" -or [string]$capture.office_bitness -ne "64") {
        throw "$Owner must bind x64 and Office64 facts"
    }
    foreach ($field in @(
        "profile", "target_arch", "os_build", "office_product", "office_version",
        "office_build", "office_channel", "office_bitness", "locale",
        "snapshot_or_image", "reset_policy", "evidence_state"
    )) {
        if ([string]$capture.$field -ne [string]$Environment.$field) {
            throw "$Owner field '$field' differs from IDEAL_ENVIRONMENT_MANIFEST_V1.csv"
        }
    }
    if ([string]$Environment.profile -ne "windows-x64" -or
        [string]$Environment.target_arch -ne "x64" -or
        [string]$Environment.office_bitness -ne "64" -or
        [string]$Environment.office_product -eq "n/a") {
        throw "$Owner canonical environment is not Windows x64 with Office64"
    }
    foreach ($field in @("os_build", "office_version", "office_build", "office_channel", "locale")) {
        if (Test-WindowsFixturePlaceholder -Value ([string]$Environment.$field)) {
            throw "$Owner canonical environment retains mutable or placeholder '$field=$($Environment.$field)'"
        }
    }
    if ((Test-WindowsFixturePlaceholder -Value $environmentId) -or
        [string]($Environment.snapshot_or_image) -notmatch '^(?:[A-Za-z0-9._-]+@)?sha256:[0-9a-f]{64}$') {
        throw "$Owner environment/image identity is mutable or lacks an immutable SHA-256 identity"
    }
    $expectedResetHash = Get-WindowsFixtureResetPolicyHash -ResetPolicy ([string]$Environment.reset_policy)
    if ([string]$capture.reset_policy_hash -cne $expectedResetHash) {
        throw "$Owner reset_policy_hash does not bind the canonical reset policy"
    }

    switch ([string]$Environment.role) {
        "dev-oracle" {
            if ([string]$Environment.evidence_state -ne "characterized-noncertifying" -or
                [bool]$capture.certification_authority -or -not [bool]$capture.noncertifying) {
                throw "$Owner dev-oracle capture must remain explicitly noncertifying"
            }
        }
        "certification-vm" {
            if ([string]$Environment.evidence_state -ne "verified" -or
                -not [bool]$capture.certification_authority -or [bool]$capture.noncertifying -or
                "$($Environment.snapshot_or_image) $($Environment.reset_policy)" -notmatch '(?i)(pinned|snapshot|image).*(revert|reset)|(revert|reset).*(pinned|snapshot|image)') {
                throw "$Owner certification capture must bind a verified pinned resettable image and certification authority"
            }
        }
        default {
            throw "$Owner has unsupported Windows environment role '$($Environment.role)'"
        }
    }
}

function Get-WindowsFixtureSourceRecipeHash {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)]$Row,
        [Parameter(Mandatory = $true)][string[]]$SourcePaths
    )

    $paths = @(ConvertTo-WindowsFixtureNormalizedRelativePaths -Paths $SourcePaths)
    $fieldNames = @(
        "matrix_id",
        "row_id",
        "fixture_id",
        "recipe_id",
        "target_arch",
        "office_bitness",
        "process_shape",
        "apartment_shape",
        "exact_signature",
        "execution_recipe",
        "cleanup_recipe",
        "result_expectation",
        "err_expectation",
        "side_effect_expectation",
        "lifecycle_order_expectation",
        "transport_expectation",
        "balance_expectation"
    )
    $payload = [Text.StringBuilder]::new()
    [void]$payload.Append("windows-x64-source-recipe-v1`n")
    foreach ($fieldName in $fieldNames) {
        $value = ([string]$Row.$fieldName).Replace("`r`n", "`n").Replace("`r", "`n")
        [void]$payload.Append($fieldName).Append(':').Append($value.Length).Append(':').Append($value).Append("`n")
    }
    foreach ($path in $paths) {
        $fileHash = Get-WindowsFixtureCanonicalSourceFileHash -RepositoryRoot $RepositoryRoot -RelativePath $path
        [void]$payload.Append("source_path:").Append($path.Length).Append(':').Append($path).Append("`n")
        [void]$payload.Append("source_hash:").Append($fileHash.Length).Append(':').Append($fileHash).Append("`n")
    }
    return Get-WindowsFixtureSha256Text -Text $payload.ToString()
}

function Test-WindowsFixtureSha256 {
    param([AllowEmptyString()][string]$Value)

    return $Value -match '^sha256:[0-9a-f]{64}$'
}

function New-WindowsFixtureExecutionRecipe {
    param(
        [Parameter(Mandatory = $true)][string]$MatrixId,
        [Parameter(Mandatory = $true)]$MatrixRow
    )

    $driver = switch ($MatrixId) {
        "WIN-COM-CLIENT" { "build controlled x64 COM server and x64 VM3/JIT clients; activate and invoke through the declared binding and transport" }
        "WIN-COM-EVENTS" { "build controlled x64 connection-point source and sink; Advise; fire synchronously; capture callback/writeback; Unadvise" }
        "WIN-COM-SERVER" { "build the controlled verified OxImage served object; register only owned x64 COM identities; invoke from the declared native or VBA client" }
        "WIN-NATIVE-IMPORT" { "build a controlled x64 native fixture DLL; execute the declared PtrSafe/LongPtr call, pointer, or callback route under VM3 and JIT" }
        "WIN-NATIVE-EXPORT" { "build the declared x64 output class; invoke it from an external x64 client while preserving wrapper-versus-native identity" }
        "WIN-ABI-CARRIER" { "run the controlled x64 layout, metadata, environment, or certification probe for the declared signature" }
        default { throw "Unknown Windows fixture matrix '$MatrixId'" }
    }
    return "target=x64; office-bitness=$([string]$MatrixRow.office_bitness); fixture=$([string]$MatrixRow.fixture_id); process=$([string]$MatrixRow.process_shape); apartment=$([string]$MatrixRow.apartment_shape); $driver; capture=six-axis(result,full-Err,side-effects,lifecycle-order,transport,balance); ownership=record-before-mutation; fallback=forbidden"
}

function New-WindowsFixtureCleanupRecipe {
    param(
        [Parameter(Mandatory = $true)][string]$MatrixId
    )

    switch ($MatrixId) {
        "WIN-COM-CLIENT" { "release owned COM interfaces and marshalling temporaries; revoke owned registrations; stop only recorded fixture PIDs; remove only owned files and HKCU keys; assert reference/transport balance" }
        "WIN-COM-EVENTS" { "Unadvise every owned cookie; release owned source/sink interfaces; drain only the owned apartment pump; stop only recorded PIDs; remove owned files/keys; assert event/reference balance" }
        "WIN-COM-SERVER" { "revoke owned class objects; release factories and served objects; request normal owned server shutdown; remove only owned x64 registration keys/files; stop only recorded PIDs; assert unload/reference balance" }
        "WIN-NATIVE-IMPORT" { "release owned callback thunks and buffers; unload owned DLL handles; stop only recorded message pumps/PIDs; remove only owned files; assert pin/callback/handle balance" }
        "WIN-NATIVE-EXPORT" { "request normal owned session shutdown; unload owned modules/servers outside loader lock; remove only owned registration/temp artifacts; stop only recorded PIDs; assert unload/resource balance" }
        "WIN-ABI-CARRIER" { "clear and release owned carriers, pins, typeinfo and probes; restore only the owned snapshot when applicable; stop only recorded PIDs; remove only owned files/keys; assert carrier/resource balance" }
        default { throw "Unknown Windows fixture matrix '$MatrixId'" }
    }
}

function New-WindowsFixtureManifestRows {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [string]$EnvironmentManifestPath = "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv"
    )

    $sourcePathMap = Get-WindowsFixtureCurrentSourcePathMap
    $artifactContracts = Get-WindowsFixtureArtifactContractMap
    $environmentRows = @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $RepositoryRoot -Path $EnvironmentManifestPath))
    $environmentById = @{}
    foreach ($environment in $environmentRows) {
        $environmentKey = [string]$environment.environment_id
        if ($environmentById.ContainsKey($environmentKey)) {
            throw "Duplicate environment '$environmentKey' in environment manifest"
        }
        $environmentById[$environmentKey] = $environment
    }

    $rows = [Collections.Generic.List[object]]::new()
    foreach ($matrixEntry in (Get-WindowsFixtureMatrixContracts).GetEnumerator()) {
        $matrixId = [string]$matrixEntry.Key
        $matrixPath = [string]$matrixEntry.Value
        $matrixRows = @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $RepositoryRoot -Path $matrixPath))
        foreach ($matrixRow in $matrixRows) {
            $rowId = [string]$matrixRow.row_id
            $key = "$matrixId|$rowId"
            $fixtureId = [string]$matrixRow.fixture_id
            $residualOwner = [string]$matrixRow.residual_owner_bead
            if (-not $artifactContracts.ContainsKey($key)) {
                throw "Windows fixture row '$key' has no explicit artifact contract"
            }
            $artifactContract = $artifactContracts[$key]
            $environmentId = [string]$matrixRow.environment_id
            if (-not $environmentById.ContainsKey($environmentId)) {
                throw "Windows fixture row '$key' references unknown environment '$environmentId'"
            }
            $environment = $environmentById[$environmentId]
            $environmentContract = Get-WindowsFixtureEnvironmentCaptureContract -Environment $environment

            [string[]]$sourcePaths = @()
            if ($sourcePathMap.ContainsKey($key)) {
                $sourcePaths = @(ConvertTo-WindowsFixtureNormalizedRelativePaths -Paths @($sourcePathMap[$key]))
            }
            $sourceState = if ($sourcePaths.Count -gt 0) { "current" } else { "pending" }
            $sourcePathsText = if ($sourceState -eq "current") { $sourcePaths -join '|' } else { "pending" }
            $sourceOwner = if ($sourceState -eq "current") {
                "n/a"
            }
            else {
                if ([string]::IsNullOrWhiteSpace($residualOwner)) {
                    throw "Windows fixture row '$key' has pending source/recipe but no delivery owner"
                }
                $residualOwner
            }

            $fixtureHash = [string]$matrixRow.fixture_hash
            $builtState = if (Test-WindowsFixtureSha256 -Value $fixtureHash) { "current" } elseif ($fixtureHash -eq "pending") { "pending" } else { throw "Windows fixture row '$key' has malformed matrix fixture_hash '$fixtureHash'" }
            $builtPath = if ($builtState -eq "current") { "$($artifactContract.Root)/$($artifactContract.Name)" } else { "pending" }
            $builtOwner = if ($builtState -eq "current") {
                "n/a"
            }
            else {
                if ([string]::IsNullOrWhiteSpace($residualOwner)) {
                    throw "Windows fixture row '$key' has pending built artifact but no delivery owner"
                }
                $residualOwner
            }

            $environmentHash = [string]$matrixRow.environment_hash
            $environmentState = if (Test-WindowsFixtureSha256 -Value $environmentHash) { "current" } elseif ($environmentHash -eq "pending") { "pending" } else { throw "Windows fixture row '$key' has malformed matrix environment_hash '$environmentHash'" }
            $environmentCapturePath = if ($environmentState -eq "current") { "$($environmentContract.Root)/$($environmentContract.Name)" } else { "pending" }
            $environmentOwner = if ($environmentState -eq "current") { "n/a" } else { [string]$environment.owner_bead }

            $manifestRow = [pscustomobject][ordered]@{
                matrix_id = $matrixId
                row_id = $rowId
                fixture_id = $fixtureId
                recipe_id = "x64-$fixtureId-recipe-v1"
                target_arch = [string]$matrixRow.target_arch
                office_bitness = [string]$matrixRow.office_bitness
                process_shape = [string]$matrixRow.process_shape
                apartment_shape = [string]$matrixRow.apartment_shape
                exact_signature = [string]$matrixRow.exact_signature
                execution_recipe = New-WindowsFixtureExecutionRecipe -MatrixId $matrixId -MatrixRow $matrixRow
                cleanup_recipe = New-WindowsFixtureCleanupRecipe -MatrixId $matrixId
                source_recipe_state = $sourceState
                source_recipe_paths = $sourcePathsText
                source_recipe_hash = ""
                source_recipe_owner_bead = $sourceOwner
                built_artifact_id = "x64-$fixtureId-artifact-v1"
                built_artifact_class = [string]$artifactContract.Class
                built_artifact_root = [string]$artifactContract.Root
                built_artifact_name = [string]$artifactContract.Name
                built_artifact_type = [string]$artifactContract.Type
                built_artifact_components = [string]$artifactContract.Components
                # Controller-owned, non-serialized admission policy. The V1
                # canonical CSV schema stays unchanged; the validator obtains
                # this value from the explicit source contract above.
                built_artifact_component_constraints = [string]$artifactContract.ComponentConstraints
                built_artifact_state = $builtState
                built_artifact_path = $builtPath
                built_artifact_hash = $fixtureHash
                built_artifact_owner_bead = $builtOwner
                environment_id = $environmentId
                environment_role = [string]$environment.role
                environment_profile = [string]$environment.profile
                environment_target_arch = [string]$environment.target_arch
                environment_office_bitness = [string]$environment.office_bitness
                environment_evidence_state = [string]$environment.evidence_state
                environment_capture_root = [string]$environmentContract.Root
                environment_capture_name = [string]$environmentContract.Name
                environment_capture_schema = [string]$environmentContract.Schema
                environment_state = $environmentState
                environment_capture_path = $environmentCapturePath
                environment_hash = $environmentHash
                environment_owner_bead = $environmentOwner
                result_expectation = [string]$matrixRow.result_expectation
                err_expectation = [string]$matrixRow.err_expectation
                side_effect_expectation = [string]$matrixRow.side_effect_expectation
                lifecycle_order_expectation = [string]$matrixRow.lifecycle_order_expectation
                transport_expectation = [string]$matrixRow.transport_expectation
                balance_expectation = [string]$matrixRow.balance_expectation
                capability_credit = "none"
                notes = if ($sourceState -eq "current") {
                    "Current controlled source/recipe bytes only; built artifact and environment hashes remain independent; no capability or certification credit"
                }
                else {
                    "Explicit recipe retained while dedicated source and built artifact remain pending under exact owners; no capability or certification credit"
                }
            }
            $manifestRow.source_recipe_hash = if ($sourceState -eq "current") {
                Get-WindowsFixtureSourceRecipeHash -RepositoryRoot $RepositoryRoot -Row $manifestRow -SourcePaths $sourcePaths
            }
            else {
                "pending"
            }
            $rows.Add($manifestRow)
        }
    }
    return @($rows | Sort-Object matrix_id, row_id)
}

function ConvertTo-WindowsFixtureManifestCsv {
    param(
        [Parameter(Mandatory = $true)][object[]]$Rows
    )

    $header = @(Get-WindowsFixtureManifestHeader)
    $selected = @($Rows | Select-Object -Property $header)
    $lines = @($selected | ConvertTo-Csv -NoTypeInformation -UseQuotes Always)
    return (($lines -join "`n") + "`n")
}

function ConvertTo-WindowsFixtureComparableText {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text
    )

    if ($Text.Length -gt 0 -and $Text[0] -eq [char]0xFEFF) {
        $Text = $Text.Substring(1)
    }
    return $Text.Replace("`r`n", "`n").Replace("`r", "`n")
}
