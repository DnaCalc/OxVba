param(
    [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
    [string]$IssuesPath = ".beads/issues.jsonl",
    [string]$RepositoryRoot = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}
else {
    (Resolve-Path $RepositoryRoot).Path
}
. (Join-Path $PSScriptRoot "lib-ideal-program-validation.ps1")

function Assert-ExactStringSet {
    param(
        [Parameter(Mandatory = $true)][string[]]$Actual,
        [Parameter(Mandatory = $true)][string[]]$Expected,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $difference = @(Compare-Object -ReferenceObject @($Expected | Sort-Object -Unique) -DifferenceObject @($Actual | Sort-Object -Unique))
    if ($difference.Count -gt 0) {
        $missing = @($difference | Where-Object SideIndicator -eq '<=' | ForEach-Object InputObject)
        $unexpected = @($difference | Where-Object SideIndicator -eq '=>' | ForEach-Object InputObject)
        throw "validate-windows-x64-control-surfaces: $Owner differs (missing=$($missing -join '|'); unexpected=$($unexpected -join '|'))"
    }
}

function Assert-ExactClauseSet {
    param(
        [Parameter(Mandatory = $true)]$Row,
        [Parameter(Mandatory = $true)][string[]]$Expected,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $actual = @(Get-IdealContractClauseIds -Text ([string]$Row.contract_clauses))
    Assert-ExactStringSet -Actual $actual -Expected $Expected -Owner "$Owner contract clauses"
}

function Test-BeadInEpicScope {
    param(
        [Parameter(Mandatory = $true)][string]$BeadId,
        [Parameter(Mandatory = $true)][string]$EpicId,
        [Parameter(Mandatory = $true)][hashtable]$ParentIdsById
    )

    $pending = [Collections.Generic.Queue[string]]::new()
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $pending.Enqueue($BeadId)
    while ($pending.Count -gt 0) {
        $candidate = $pending.Dequeue()
        if (-not $seen.Add($candidate)) {
            continue
        }
        if ($candidate -eq $EpicId) {
            return $true
        }
        if ($ParentIdsById.ContainsKey($candidate)) {
            foreach ($parentId in @($ParentIdsById[$candidate])) {
                $pending.Enqueue($parentId)
            }
        }
    }
    return $false
}

$expectedMatrices = [ordered]@{
    "WIN-COM-CLIENT" = [pscustomobject]@{ Path = "docs/validation/WINDOWS_JIT_COM_CLIENT_MATRIX_V1.csv"; Role = "primary"; OwnerEpic = "bd-59co.3.4"; Prefix = "WCC"; RowCount = 9 }
    "WIN-COM-EVENTS" = [pscustomobject]@{ Path = "docs/validation/WINDOWS_JIT_COM_EVENTS_MATRIX_V1.csv"; Role = "primary"; OwnerEpic = "bd-59co.3.6"; Prefix = "WCE"; RowCount = 7 }
    "WIN-COM-SERVER" = [pscustomobject]@{ Path = "docs/validation/WINDOWS_JIT_COM_SERVER_MATRIX_V1.csv"; Role = "primary"; OwnerEpic = "bd-59co.3.7"; Prefix = "WCS"; RowCount = 7 }
    "WIN-NATIVE-IMPORT" = [pscustomobject]@{ Path = "docs/validation/WINDOWS_JIT_NATIVE_IMPORT_MATRIX_V1.csv"; Role = "primary"; OwnerEpic = "bd-59co.3.10"; Prefix = "WNI"; RowCount = 8 }
    "WIN-NATIVE-EXPORT" = [pscustomobject]@{ Path = "docs/validation/WINDOWS_NATIVE_EXPORT_AND_PACKAGING_MATRIX_V1.csv"; Role = "primary"; OwnerEpic = "bd-59co.3.13"; Prefix = "WNE"; RowCount = 8 }
    "WIN-ABI-CARRIER" = [pscustomobject]@{ Path = "docs/validation/WINDOWS_ABI_CARRIER_MATRIX_V1.csv"; Role = "quality"; OwnerEpic = "bd-59co.3.2"; Prefix = "WAC"; RowCount = 18 }
}

$expectedRoutes = @'
matrix_id,row_id,owner_epic,evidence_owner_bead,residual_owner_bead
WIN-COM-CLIENT,WCC-PLAN-LATE,bd-59co.3.4,bd-59co.3.4.1,bd-59co.3.4
WIN-COM-CLIENT,WCC-LATE-ARGS,bd-59co.3.4,bd-59co.3.4.1,bd-59co.3.4
WIN-COM-CLIENT,WCC-LATE-STRUCTURAL,bd-59co.3.4,bd-59co.3.4.1,bd-59co.3.4
WIN-COM-CLIENT,WCC-LATE-OUTPROC-ERROR,bd-59co.3.4,bd-59co.3.4.1,bd-59co.3.4
WIN-COM-CLIENT,WCC-PLAN-EARLY,bd-59co.3.5,bd-59co.3.5.1,bd-59co.3.5
WIN-COM-CLIENT,WCC-EARLY-COMPLEX,bd-59co.3.5,bd-59co.3.5.1,bd-59co.3.5
WIN-COM-CLIENT,WCC-EARLY-CUSTOM,bd-59co.3.5,bd-59co.3.5.1,bd-59co.3.5
WIN-COM-CLIENT,WCC-EARLY-OUTPROC,bd-59co.3.5,bd-59co.3.5.1,bd-59co.3.5
WIN-COM-CLIENT,WCC-EXCEL-AUTHORITY,bd-59co.3.15,bd-59co.3.15.32,bd-59co.3.15.32
WIN-COM-EVENTS,WCE-PLAN-INCOMING,bd-59co.3.6,bd-59co.3.6.1,bd-59co.3.6.4
WIN-COM-EVENTS,WCE-INCOMING-COMPLEX,bd-59co.3.6,bd-59co.3.6.1,bd-59co.3.6.4
WIN-COM-EVENTS,WCE-INCOMING-CUSTOM,bd-59co.3.6,bd-59co.3.6.1,bd-59co.3.6.4
WIN-COM-EVENTS,WCE-INCOMING-APARTMENT,bd-59co.3.6,bd-59co.3.6.1,bd-59co.3.6.4
WIN-COM-EVENTS,WCE-INCOMING-LIFECYCLE,bd-59co.3.6,bd-59co.3.6.1,bd-59co.3.6.4
WIN-COM-EVENTS,WCE-PLAN-OUTGOING,bd-59co.3.9,bd-59co.3.9.1,bd-59co.3.9
WIN-COM-EVENTS,WCE-OUTGOING-COMPLEX,bd-59co.3.9,bd-59co.3.9.1,bd-59co.3.9
WIN-COM-SERVER,WCS-LATE-INPROC,bd-59co.3.7,bd-59co.3.7.1,bd-59co.3.7
WIN-COM-SERVER,WCS-LATE-LOCALSERVER,bd-59co.3.7,bd-59co.3.7.1,bd-59co.3.7
WIN-COM-SERVER,WCS-LATE-COMPLEX,bd-59co.3.7,bd-59co.3.7.1,bd-59co.3.7
WIN-COM-SERVER,WCS-DUAL-INPROC,bd-59co.3.8,bd-59co.3.8.1,bd-59co.3.8
WIN-COM-SERVER,WCS-IMPLEMENTS-CUSTOM,bd-59co.3.8,bd-59co.3.8.1,bd-59co.3.8
WIN-COM-SERVER,WCS-EARLY-OUTPROC,bd-59co.3.8,bd-59co.3.8.1,bd-59co.3.8
WIN-COM-SERVER,WCS-SERVER-SAFETY,bd-59co.3.7,bd-59co.3.7.1,bd-59co.3.7
WIN-NATIVE-IMPORT,WNI-PLAN-DECLARE,bd-59co.3.10,bd-59co.3.10.1,bd-59co.3.10
WIN-NATIVE-IMPORT,WNI-DECLARE-STRINGS,bd-59co.3.10,bd-59co.3.10.1,bd-59co.3.10
WIN-NATIVE-IMPORT,WNI-DECLARE-STRUCTURAL,bd-59co.3.10,bd-59co.3.10.1,bd-59co.3.10
WIN-NATIVE-IMPORT,WNI-DECLARE-LOADER-ERROR,bd-59co.3.10,bd-59co.3.10.1,bd-59co.3.10
WIN-NATIVE-IMPORT,WNI-POINTER-HELPERS,bd-59co.3.11,bd-59co.3.11.1,bd-59co.3.11
WIN-NATIVE-IMPORT,WNI-CALLBACK-SYNC,bd-59co.3.11,bd-59co.3.11.1,bd-59co.3.11
WIN-NATIVE-IMPORT,WNI-PLAN-CALLBACK,bd-59co.3.11,bd-59co.3.11.1,bd-59co.3.11.4
WIN-NATIVE-IMPORT,WNI-CALLBACK-NESTED,bd-59co.3.11,bd-59co.3.11.1,bd-59co.3.11.4
WIN-NATIVE-EXPORT,WNE-WRAPPER-EXE,bd-59co.3.12,bd-59co.3.12.1,bd-59co.3.12
WIN-NATIVE-EXPORT,WNE-WRAPPER-LIBRARY,bd-59co.3.12,bd-59co.3.12.1,bd-59co.3.12
WIN-NATIVE-EXPORT,WNE-PLAN-WRAPPED,bd-59co.3.12,bd-59co.3.12.1,bd-59co.3.12
WIN-NATIVE-EXPORT,WNE-PLAN-NATIVE,bd-59co.3.13,bd-59co.3.13.1,bd-59co.3.13
WIN-NATIVE-EXPORT,WNE-NATIVE-EXE,bd-59co.3.13,bd-59co.3.13.1,bd-59co.3.13
WIN-NATIVE-EXPORT,WNE-NATIVE-ABI-BREADTH,bd-59co.3.13,bd-59co.3.13.1,bd-59co.3.13
WIN-NATIVE-EXPORT,WNE-NATIVE-REPRO-DEPLOY,bd-59co.3.13,bd-59co.3.13.1,bd-59co.3.13
WIN-NATIVE-EXPORT,WNE-PROFILE-TOOL-TERMINAL,bd-59co.3.16,bd-59co.3.16.1,bd-59co.3.16.1
WIN-ABI-CARRIER,WAC-BSTR-LAYOUT,bd-59co.3.2,bd-59co.3.2.1,bd-59co.3.2
WIN-ABI-CARRIER,WAC-VARIANT-LAYOUT,bd-59co.3.2,bd-59co.3.2.1,bd-59co.3.2
WIN-ABI-CARRIER,WAC-SAFEARRAY-LAYOUT,bd-59co.3.2,bd-59co.3.2.1,bd-59co.3.2
WIN-ABI-CARRIER,WAC-IUNKNOWN-IDENTITY,bd-59co.3.2,bd-59co.3.2.1,bd-59co.3.2
WIN-ABI-CARRIER,WAC-NUMERIC-LONGPTR,bd-59co.3.2,bd-59co.3.2.1,bd-59co.3.2
WIN-ABI-CARRIER,WAC-INTERFACE-ARRAY,bd-59co.3.2,bd-59co.3.2.1,bd-59co.3.2
WIN-ABI-CARRIER,WAC-VT-RECORD,bd-59co.3.2,bd-59co.3.2.1,bd-59co.3.2
WIN-ABI-CARRIER,WAC-CARRIER-EXCEL-ROUNDTRIP,bd-59co.3.2,bd-59co.3.2.1,bd-59co.3.2
WIN-ABI-CARRIER,WAC-SAFETY-MUTATION,bd-59co.3.14,bd-59co.3.14.2,bd-59co.3.14.2
WIN-ABI-CARRIER,WAC-TARGET-DEV-ENV,bd-59co.3.1,bd-59co.3.1.2,bd-59co.3.1.2
WIN-ABI-CARRIER,WAC-TYPELIB-METADATA,bd-59co.3.2,bd-59co.3.2.1,bd-59co.3.2
WIN-ABI-CARRIER,WAC-VERIFIED-INTEROP-PLAN,bd-59co.3.3,bd-59co.3.3.1,bd-59co.3.3.1
WIN-ABI-CARRIER,WAC-WINDOWS-DESCRIPTORS,bd-59co.3.3,bd-59co.3.3.1,bd-59co.3.3.1
WIN-ABI-CARRIER,WAC-CLEAN-CERT-ENV,bd-59co.3.15,bd-59co.3.15.3,bd-59co.3.15.3
WIN-ABI-CARRIER,WAC-RELEASE-CERT,bd-59co.3.15,bd-59co.3.15.2,bd-59co.3.15.2
WIN-ABI-CARRIER,WAC-EXCEL-COM-CERT,bd-59co.3.15,bd-59co.3.15.32,bd-59co.3.15.32
WIN-ABI-CARRIER,WAC-EXCEL-NATIVE-CERT,bd-59co.3.15,bd-59co.3.15.33,bd-59co.3.15.33
WIN-ABI-CARRIER,WAC-PROFILE-TERMINAL,bd-59co.3.16,bd-59co.3.16.1,bd-59co.3.16.1
'@ | ConvertFrom-Csv

if ($expectedRoutes.Count -ne 57) {
    throw "validate-windows-x64-control-surfaces: internal route contract must contain exactly 57 rows"
}

Push-Location $repoRoot
try {
    $manifest = (Read-IdealProgramManifest -RepoRoot $repoRoot -ManifestPath $ManifestPath).Manifest
    $issues = Read-IdealIssues -RepoRoot $repoRoot -IssuesPath $IssuesPath
    $issueById = $issues.IssueById
    $parentIdsById = @{}
    foreach ($issue in @($issues.Issues)) {
        $parentIdsById[[string]$issue.id] = @(Get-IdealParentIds -Issue $issue)
    }

    $epicProfileById = @{}
    foreach ($epic in @(Get-IdealExpectedEpicRecords -Manifest $manifest)) {
        $epicProfileById[$epic.EpicId] = $epic.Profile
    }

    # Exclude target architectures without rejecting the conventional x64
    # target spellings x86_64 and x86-64.
    $forbiddenTargetPattern = '(?i)(?<![A-Z0-9])x86(?![_-]64(?:$|[^A-Z0-9]))(?![A-Z0-9])|(?<![A-Z0-9])i686(?![A-Z0-9])|(?<![A-Z0-9])WOW64(?![A-Z0-9])|(?<![A-Z0-9])(?:ARM64(?:EC)?|AARCH64)(?![A-Z0-9])|(?<![A-Z0-9])32[- ]?bit[- ]*(?:Windows|Office|Excel|process|artifact|binary|target|host)(?![A-Z0-9])|(?<![A-Z0-9])(?:Windows|Office|Excel|process|artifact|binary|target|host)[- ]*32[- ]?bit(?![A-Z0-9])|(?<![A-Z0-9])(?:Office32|Excel32|process32|artifact32)(?![A-Z0-9])'
    $allowedProcessShapes = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($shape in @(
        "clean-certification-VM",
        "clean-certification-VM with controlled COM fixtures",
        "development-oracle-host",
        "inproc-and-localserver",
        "inproc-and-localserver-served-object",
        "inproc-and-localserver-wrapped-COM",
        "inproc-and-out-of-proc",
        "inproc-client-and-server",
        "inproc-COM-server",
        "inproc-native-and-COM-cycle",
        "inproc-native-DLL",
        "inproc-native-DLL-and-message-pump",
        "inproc-served-object",
        "inproc-source",
        "inproc-wrapper-library",
        "native-DLL",
        "native-DLL-and-EXE",
        "native-EXE",
        "out-of-proc-COM-server",
        "out-of-proc-event-source",
        "out-of-proc-LocalServer32",
        "OxVba-and-Excel-COM-boundary",
        "OxVba-process",
        "release-profile",
        "standalone",
        "wrapped-and-native-DLL-EXE-profile"
    )) {
        [void]$allowedProcessShapes.Add($shape)
    }

    $ownershipPath = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path ([string]$manifest.matrix_ownership)
    $allOwnership = @(Import-Csv -LiteralPath $ownershipPath)
    foreach ($owner in $allOwnership) {
        $looksWindows = [string]$owner.matrix_id -like "WIN-*" -or
            ([string]$owner.path).Replace('\', '/') -match '(?i)(?:^|/)WINDOWS_' -or
            [string]$owner.profile -match '(?i)(windows|x86|wow64|arm64|aarch64)'
        if ($looksWindows -and [string]$owner.profile -ne "windows-x64") {
            throw "validate-windows-x64-control-surfaces: Windows-like matrix '$($owner.matrix_id)' must use profile windows-x64"
        }
        if ($looksWindows) {
            $ownerText = @($owner.PSObject.Properties | ForEach-Object { [string]$_.Value }) -join ' '
            if ($ownerText -match $forbiddenTargetPattern) {
                throw "validate-windows-x64-control-surfaces: matrix ownership '$($owner.matrix_id)' contains an excluded non-x64 artifact"
            }
        }
    }
    $windowsOwnership = @($allOwnership | Where-Object { [string]$_.profile -eq "windows-x64" })
    Assert-ExactStringSet `
        -Actual @($windowsOwnership.matrix_id) `
        -Expected @($expectedMatrices.Keys) `
        -Owner "Windows matrix inventory"

    $ownershipById = @{}
    foreach ($owner in $windowsOwnership) {
        $matrixId = [string]$owner.matrix_id
        if ($ownershipById.ContainsKey($matrixId)) {
            throw "validate-windows-x64-control-surfaces: duplicate Windows ownership row '$matrixId'"
        }
        $ownershipById[$matrixId] = $owner
    }

    $routesByKey = @{}
    foreach ($route in $expectedRoutes) {
        $key = "$([string]$route.matrix_id)|$([string]$route.row_id)"
        if ($routesByKey.ContainsKey($key)) {
            throw "validate-windows-x64-control-surfaces: duplicate internal route '$key'"
        }
        $routesByKey[$key] = $route
    }

    $officeBitnessNotApplicable = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($key in @(
        "WIN-NATIVE-EXPORT|WNE-WRAPPER-EXE",
        "WIN-NATIVE-EXPORT|WNE-WRAPPER-LIBRARY",
        "WIN-NATIVE-EXPORT|WNE-PLAN-NATIVE",
        "WIN-NATIVE-EXPORT|WNE-NATIVE-EXE",
        "WIN-NATIVE-EXPORT|WNE-NATIVE-ABI-BREADTH",
        "WIN-NATIVE-EXPORT|WNE-NATIVE-REPRO-DEPLOY",
        "WIN-NATIVE-EXPORT|WNE-PROFILE-TOOL-TERMINAL",
        "WIN-ABI-CARRIER|WAC-BSTR-LAYOUT",
        "WIN-ABI-CARRIER|WAC-VARIANT-LAYOUT",
        "WIN-ABI-CARRIER|WAC-SAFEARRAY-LAYOUT",
        "WIN-ABI-CARRIER|WAC-IUNKNOWN-IDENTITY",
        "WIN-ABI-CARRIER|WAC-NUMERIC-LONGPTR",
        "WIN-ABI-CARRIER|WAC-INTERFACE-ARRAY",
        "WIN-ABI-CARRIER|WAC-VT-RECORD",
        "WIN-ABI-CARRIER|WAC-SAFETY-MUTATION",
        "WIN-ABI-CARRIER|WAC-VERIFIED-INTEROP-PLAN",
        "WIN-ABI-CARRIER|WAC-WINDOWS-DESCRIPTORS"
    )) {
        [void]$officeBitnessNotApplicable.Add($key)
    }

    $defaultAuthority = @(
        "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md",
        "docs/spec/OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md",
        "docs/worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md"
    )
    $authorityOverrides = @{
        "WIN-COM-CLIENT|WCC-EXCEL-AUTHORITY" = @(
            "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md",
            "docs/spec/OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md",
            "docs/memory/EXCEL_VBA_ORACLE_MODAL_HANDLING.md"
        )
        "WIN-ABI-CARRIER|WAC-TARGET-DEV-ENV" = @(
            "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md",
            "docs/spec/OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md",
            "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv"
        )
        "WIN-ABI-CARRIER|WAC-VERIFIED-INTEROP-PLAN" = @(
            "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md",
            "docs/spec/OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md"
        )
        "WIN-ABI-CARRIER|WAC-WINDOWS-DESCRIPTORS" = @(
            "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md",
            "docs/spec/OXVBA_WINDOWS_INTEROP_ARCHITECTURE_V1.md"
        )
        "WIN-ABI-CARRIER|WAC-CLEAN-CERT-ENV" = @(
            "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md",
            "docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv"
        )
        "WIN-ABI-CARRIER|WAC-RELEASE-CERT" = @(
            "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md",
            "docs/worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md"
        )
        "WIN-ABI-CARRIER|WAC-EXCEL-COM-CERT" = @(
            "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md",
            "docs/worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md"
        )
        "WIN-ABI-CARRIER|WAC-EXCEL-NATIVE-CERT" = @(
            "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md",
            "docs/worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md"
        )
        "WIN-ABI-CARRIER|WAC-PROFILE-TERMINAL" = @(
            "docs/spec/OXVBA_SYSTEM_CONTRACT_V1.md",
            "docs/worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md"
        )
    }

    $matrixRowsByKey = @{}
    $seenGlobalRowIds = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $activeStatuses = @("open", "in_progress", "blocked")
    $truthStates = @("planned", "in-progress", "implemented-subset", "implemented-full", "verified")

    foreach ($matrixEntry in $expectedMatrices.GetEnumerator()) {
        $matrixId = [string]$matrixEntry.Key
        $expected = $matrixEntry.Value
        $owner = $ownershipById[$matrixId]

        $actualPath = ([string]$owner.path).Replace('\', '/')
        if ($actualPath -ne [string]$expected.Path -or
            [string]$owner.role -ne [string]$expected.Role -or
            [string]$owner.owner_epic -ne [string]$expected.OwnerEpic -or
            [string]$owner.row_id_prefix -ne [string]$expected.Prefix -or
            -not (ConvertFrom-IdealBoolean -Value ([string]$owner.required_for_terminal) -Owner "$matrixId required_for_terminal")) {
            throw "validate-windows-x64-control-surfaces: matrix '$matrixId' ownership contract drifted"
        }

        $matrixPath = Resolve-IdealRepoPath -RepoRoot $repoRoot -Path $actualPath
        $rows = @(Import-Csv -LiteralPath $matrixPath)
        if ($rows.Count -ne [int]$expected.RowCount) {
            throw "validate-windows-x64-control-surfaces: matrix '$matrixId' expected $($expected.RowCount) rows, found $($rows.Count)"
        }
        $expectedRowIds = @($expectedRoutes | Where-Object { [string]$_.matrix_id -eq $matrixId } | ForEach-Object { [string]$_.row_id })
        Assert-ExactStringSet -Actual @($rows.row_id) -Expected $expectedRowIds -Owner "matrix '$matrixId' row identity set"

        foreach ($row in $rows) {
            $rowId = [string]$row.row_id
            $key = "$matrixId|$rowId"
            if (-not $seenGlobalRowIds.Add($rowId)) {
                throw "validate-windows-x64-control-surfaces: duplicate Windows row_id '$rowId'"
            }
            if (-not $routesByKey.ContainsKey($key)) {
                throw "validate-windows-x64-control-surfaces: row '$key' has no exact route contract"
            }
            $route = $routesByKey[$key]
            $matrixRowsByKey[$key] = $row

            if ([string]$row.profile -ne "windows-x64" -or
                -not (ConvertFrom-IdealBoolean -Value ([string]$row.required) -Owner "$key required") -or
                [string]$row.truth_role -ne [string]$expected.Role -or
                -not $rowId.StartsWith("$($expected.Prefix)-", [StringComparison]::OrdinalIgnoreCase)) {
                throw "validate-windows-x64-control-surfaces: row '$key' is outside its required Windows ownership envelope"
            }
            foreach ($field in @("target_arch", "office_bitness", "process_shape")) {
                if ($row.PSObject.Properties.Name -notcontains $field -or [string]::IsNullOrWhiteSpace([string]$row.$field)) {
                    throw "validate-windows-x64-control-surfaces: row '$key' must carry nonblank '$field'"
                }
            }
            if ([string]$row.target_arch -ne "x64") {
                throw "validate-windows-x64-control-surfaces: row '$key' target_arch must be x64"
            }
            if (-not $allowedProcessShapes.Contains([string]$row.process_shape)) {
                throw "validate-windows-x64-control-surfaces: row '$key' has unrecognized x64 process_shape '$($row.process_shape)'"
            }
            $expectedOfficeBitness = if ($officeBitnessNotApplicable.Contains($key)) { "n/a" } else { "64" }
            if ([string]$row.office_bitness -ne $expectedOfficeBitness) {
                throw "validate-windows-x64-control-surfaces: row '$key' office_bitness must be $expectedOfficeBitness"
            }
            if ([string]$row.truth_state -notin $truthStates) {
                throw "validate-windows-x64-control-surfaces: required row '$key' has invalid truth_state '$($row.truth_state)'"
            }
            $rowText = @($row.PSObject.Properties | ForEach-Object { [string]$_.Value }) -join ' '
            if ($rowText -match $forbiddenTargetPattern) {
                throw "validate-windows-x64-control-surfaces: row '$key' contains an excluded non-x64 target, Office, process or artifact token"
            }

            if (-not (Test-IdealEvidenceReferences -RepoRoot $repoRoot -Text ([string]$row.authority_refs))) {
                throw "validate-windows-x64-control-surfaces: row '$key' has unresolved authority references"
            }
            $authorityTokens = @(([string]$row.authority_refs -split '[;|]') | ForEach-Object { $_.Trim().Replace('\', '/') } | Where-Object { $_ })
            $expectedAuthority = if ($authorityOverrides.ContainsKey($key)) { @($authorityOverrides[$key]) } else { $defaultAuthority }
            Assert-ExactStringSet -Actual $authorityTokens -Expected $expectedAuthority -Owner "row '$key' authority route"

            foreach ($field in @("owner_epic", "evidence_owner_bead")) {
                $actual = [string]$row.$field
                $routeField = if ($field -eq "owner_epic") { "owner_epic" } else { "evidence_owner_bead" }
                $routeValue = [string]$route.$routeField
                if ($actual -ne $routeValue) {
                    throw "validate-windows-x64-control-surfaces: row '$key' $field must be '$routeValue', found '$actual'"
                }
                if (-not $issueById.ContainsKey($actual)) {
                    throw "validate-windows-x64-control-surfaces: row '$key' $field '$actual' is outside the current program"
                }
            }
            $ownerEpic = [string]$row.owner_epic
            if (-not $epicProfileById.ContainsKey($ownerEpic) -or [string]$epicProfileById[$ownerEpic] -ne "windows-x64") {
                throw "validate-windows-x64-control-surfaces: row '$key' owner '$ownerEpic' is not a Windows execution epic"
            }
            if (-not (Test-BeadInEpicScope -BeadId ([string]$row.evidence_owner_bead) -EpicId $ownerEpic -ParentIdsById $parentIdsById)) {
                throw "validate-windows-x64-control-surfaces: row '$key' evidence owner is outside owner epic '$ownerEpic'"
            }

            if ([string]$row.truth_state -eq "verified") {
                if (-not [string]::IsNullOrWhiteSpace([string]$row.residual_owner_bead) -or [string]$row.residual_disposition -eq "remaining-accepted-scope") {
                    throw "validate-windows-x64-control-surfaces: verified row '$key' retains accepted residual ownership"
                }
            }
            else {
                $expectedResidual = [string]$route.residual_owner_bead
                $actualResidual = [string]$row.residual_owner_bead
                if ($actualResidual -ne $expectedResidual) {
                    throw "validate-windows-x64-control-surfaces: row '$key' residual_owner_bead must be '$expectedResidual', found '$actualResidual'"
                }
                if ([string]$row.residual_disposition -ne "remaining-accepted-scope") {
                    throw "validate-windows-x64-control-surfaces: non-verified required row '$key' must retain remaining-accepted-scope"
                }
                if (-not $issueById.ContainsKey($actualResidual) -or
                    -not (Test-BeadInEpicScope -BeadId $actualResidual -EpicId $ownerEpic -ParentIdsById $parentIdsById) -or
                    [string]$issueById[$actualResidual].status -notin $activeStatuses) {
                    throw "validate-windows-x64-control-surfaces: row '$key' residual owner '$actualResidual' is not active in owner epic '$ownerEpic'"
                }
            }
        }
    }

    if ($matrixRowsByKey.Count -ne 57) {
        throw "validate-windows-x64-control-surfaces: expected exactly 57 routed Windows rows, found $($matrixRowsByKey.Count)"
    }

    $compatibilityTerminal = $matrixRowsByKey["WIN-ABI-CARRIER|WAC-PROFILE-TERMINAL"]
    $toolingTerminal = $matrixRowsByKey["WIN-NATIVE-EXPORT|WNE-PROFILE-TOOL-TERMINAL"]
    Assert-ExactClauseSet `
        -Row $compatibilityTerminal `
        -Expected @("CONF-DONE-001", "DOC-AUTH-001", "DOC-TRACE-001", "PROFILE-WIN-001") `
        -Owner "Windows compatibility terminal"
    Assert-ExactClauseSet `
        -Row $toolingTerminal `
        -Expected @("PROFILE-TOOL-001") `
        -Owner "standalone native-output terminal"
    if ([string]$compatibilityTerminal.capability -ne "Windows x64 profile terminal" -or
        [string]$toolingTerminal.capability -ne "VB-universe Windows tooling profile terminal" -or
        [string]$toolingTerminal.output_class -ne "PROFILE-TOOL-001") {
        throw "validate-windows-x64-control-surfaces: Windows compatibility and standalone tooling terminals have collapsed into one claim gate"
    }

    $expectedOutputClasses = [ordered]@{
        "WNE-WRAPPER-EXE" = [pscustomobject]@{ Class = "WrapperExe"; Backend = "JIT-session"; Clauses = @("BUILD-CLASS-001", "BUILD-PACKAGE-001") }
        "WNE-WRAPPER-LIBRARY" = [pscustomobject]@{ Class = "WrapperLibrary"; Backend = "JIT-session"; Clauses = @("BUILD-CLASS-001", "BUILD-PACKAGE-001") }
        "WNE-PLAN-WRAPPED" = [pscustomobject]@{ Class = "WrappedComServer"; Backend = "JIT-session"; Clauses = @("BUILD-CLASS-001", "BUILD-PACKAGE-001", "PROFILE-TOOL-001") }
        "WNE-PLAN-NATIVE" = [pscustomobject]@{ Class = "NativeDll"; Backend = "Cranelift-object"; Clauses = @("BUILD-NATIVE-001", "JIT-AOT-001", "PROFILE-TOOL-001") }
        "WNE-NATIVE-EXE" = [pscustomobject]@{ Class = "NativeExe"; Backend = "Cranelift-object"; Clauses = @("BUILD-NATIVE-001", "JIT-AOT-001") }
        "WNE-NATIVE-ABI-BREADTH" = [pscustomobject]@{ Class = "NativeDll"; Backend = "Cranelift-object"; Clauses = @("BUILD-NATIVE-001", "JIT-AOT-001") }
        "WNE-NATIVE-REPRO-DEPLOY" = [pscustomobject]@{ Class = "NativeDll-and-NativeExe"; Backend = "Cranelift-object-and-linker"; Clauses = @("BUILD-NATIVE-001", "DEBUG-MAP-001", "JIT-AOT-001") }
    }
    foreach ($entry in $expectedOutputClasses.GetEnumerator()) {
        $row = $matrixRowsByKey["WIN-NATIVE-EXPORT|$($entry.Key)"]
        if ([string]$row.output_class -ne [string]$entry.Value.Class -or [string]$row.backend -ne [string]$entry.Value.Backend) {
            throw "validate-windows-x64-control-surfaces: output row '$($entry.Key)' changed wrapper/native class or backend"
        }
        Assert-ExactClauseSet -Row $row -Expected @($entry.Value.Clauses) -Owner "output row '$($entry.Key)'"
        $clauses = @(Get-IdealContractClauseIds -Text ([string]$row.contract_clauses))
        $isWrapper = [string]$entry.Key -in @("WNE-WRAPPER-EXE", "WNE-WRAPPER-LIBRARY", "WNE-PLAN-WRAPPED")
        if ($isWrapper -and ($clauses -notcontains "BUILD-PACKAGE-001" -or $clauses -contains "BUILD-NATIVE-001")) {
            throw "validate-windows-x64-control-surfaces: wrapper row '$($entry.Key)' is not an honest package-backed wrapper"
        }
        if (-not $isWrapper -and ($clauses -notcontains "BUILD-NATIVE-001" -or $clauses -notcontains "JIT-AOT-001" -or $clauses -contains "BUILD-PACKAGE-001")) {
            throw "validate-windows-x64-control-surfaces: native row '$($entry.Key)' is not a distinct genuine native-output claim"
        }
    }

    Write-Host "validate-windows-x64-control-surfaces: ok (program=$($manifest.program_id) matrices=6 required_rows=57 target=x64 gates=compatibility+native-output)"
}
finally {
    Pop-Location
}
