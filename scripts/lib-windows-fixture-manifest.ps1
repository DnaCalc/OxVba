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
        "built_artifact_state",
        "built_artifact_path",
        "built_artifact_hash",
        "built_artifact_owner_bead",
        "environment_id",
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

function Get-WindowsFixtureCurrentArtifactPathMap {
    # Empty by design for WIN-0. Historical binaries and ad-hoc build outputs are
    # not current proof. A producer must add an immutable controlled artifact path
    # here in the same change that replaces a matrix `pending` hash.
    return @{}
}

function Get-WindowsFixtureCurrentEnvironmentCapturePathMap {
    # Empty until the environment-owner beads produce immutable captures. The
    # characterized development host is useful but is not a hashed release image.
    return @{}
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
    $artifactPathMap = Get-WindowsFixtureCurrentArtifactPathMap
    $environmentCapturePathMap = Get-WindowsFixtureCurrentEnvironmentCapturePathMap
    $environmentRows = @(Import-Csv -LiteralPath (Resolve-IdealRepoPath -RepoRoot $RepositoryRoot -Path $EnvironmentManifestPath))
    $environmentById = @{}
    foreach ($environment in $environmentRows) {
        $environmentById[[string]$environment.environment_id] = $environment
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
            $environmentId = [string]$matrixRow.environment_id
            if (-not $environmentById.ContainsKey($environmentId)) {
                throw "Windows fixture row '$key' references unknown environment '$environmentId'"
            }
            $environment = $environmentById[$environmentId]

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
            if ($builtState -eq "current" -and -not $artifactPathMap.ContainsKey($key)) {
                throw "Windows fixture row '$key' has a current artifact hash but no controlled current artifact path"
            }
            $builtPath = if ($builtState -eq "current") { [string]$artifactPathMap[$key] } else { "pending" }
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
            if ($environmentState -eq "current" -and -not $environmentCapturePathMap.ContainsKey($environmentId)) {
                throw "Windows fixture row '$key' has a current environment hash but no immutable environment capture path"
            }
            $environmentCapturePath = if ($environmentState -eq "current") { [string]$environmentCapturePathMap[$environmentId] } else { "pending" }
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
                built_artifact_state = $builtState
                built_artifact_path = $builtPath
                built_artifact_hash = $fixtureHash
                built_artifact_owner_bead = $builtOwner
                environment_id = $environmentId
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
