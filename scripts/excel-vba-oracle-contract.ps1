Set-StrictMode -Version Latest

function Get-ExcelOracleIntrinsicShadowNames {
    return @("Fix", "Date", "Time", "Name", "Error", "Left", "Right", "Len", "Val", "Format")
}

function Test-ExcelOracleIntrinsicShadowName {
    param([Parameter(Mandatory = $true)][string]$Name)

    return $Name -in @(Get-ExcelOracleIntrinsicShadowNames)
}

function Get-ExcelOracleDialogPolicy {
    param(
        [Parameter(Mandatory = $true)][ValidateSet("compile", "run", "cleanup")][string]$Phase,
        [AllowEmptyString()][string]$WindowTitle = "",
        [AllowEmptyCollection()][string[]]$Texts = @(),
        [AllowEmptyCollection()][string[]]$Buttons = @()
    )

    $signal = (@($WindowTitle) + @($Texts) + @($Buttons) -join " ").ToLowerInvariant()
    if ($signal -match 'security warning|macros in this project are disabled|content has been disabled|enable content|enable macros|trust center|publisher') {
        return [pscustomobject]@{
            kind = "security-or-trust"
            disposition = "block-no-dismiss"
            preferred_buttons = @()
        }
    }
    if ($Phase -eq "compile" -and $signal -match 'compile error|sub or function not defined|expected:|syntax error|duplicate declaration|user-defined type not defined|expected array or user-defined type') {
        return [pscustomobject]@{
            kind = "compile-error"
            disposition = "capture-then-dismiss"
            preferred_buttons = @("OK")
        }
    }
    if ($Phase -eq "run" -and $signal -match 'run-time error|runtime error') {
        return [pscustomobject]@{
            kind = "runtime-error"
            disposition = "capture-then-dismiss"
            preferred_buttons = @("End", "OK")
        }
    }
    if ($Phase -eq "run" -and $signal -match 'cannot run the macro|macro may not be available|all macros may be disabled') {
        return [pscustomobject]@{
            kind = "ambiguous-macro-failure"
            disposition = "capture-then-dismiss"
            preferred_buttons = @("OK")
        }
    }
    return [pscustomobject]@{
        kind = "unrecognized-modal"
        disposition = "block-no-dismiss"
        preferred_buttons = @()
    }
}

function Get-ExcelOracleMacroFailureDisposition {
    param(
        [Parameter(Mandatory = $true)][string]$Message,
        [Parameter(Mandatory = $true)][string]$CompileStatus,
        [Parameter(Mandatory = $true)][bool]$AccessVbom,
        [Parameter(Mandatory = $true)][bool]$RunnableEntryObserved,
        [Parameter(Mandatory = $true)][bool]$TargetExists
    )

    $isGenericMacroFailure = $Message -match '(?i)cannot run the macro|macro may not be available|all macros may be disabled'
    if (-not $isGenericMacroFailure) {
        return "non-macro-runtime-failure"
    }
    if ($CompileStatus -ne "ok") {
        return "compile-failure"
    }
    if (-not $RunnableEntryObserved) {
        return "macros-runnable-entry-unobserved"
    }
    if ($AccessVbom -and -not $TargetExists) {
        return "missing-macro"
    }
    if ($AccessVbom -and $RunnableEntryObserved -and $TargetExists) {
        return "suspected-compile-failure"
    }
    return "ambiguous"
}

function ConvertFrom-ExcelOracleRuntimeErr {
    param([Parameter(Mandatory = $true)][string]$Json)

    $err = $Json | ConvertFrom-Json
    $fields = @("number", "source", "description", "help_file", "help_context", "erl")
    if ($null -eq $err -or (@($err.PSObject.Properties.Name | Sort-Object) -join "`n") -cne (@($fields | Sort-Object) -join "`n") -or
        @(@("number", "help_context", "erl") | Where-Object { $err.$_ -isnot [int] -and $err.$_ -isnot [long] }).Count -gt 0 -or
        @(@("source", "description", "help_file") | Where-Object { $err.$_ -isnot [string] }).Count -gt 0) {
        throw "excel-vba-oracle-contract: runtime Err payload has an invalid exact field/type shape"
    }
    return [pscustomobject]@{
        schema = "oxvba.excel-vba-oracle-runtime-err.v1"
        number = [int]$err.number
        source = [string]$err.source
        description = [string]$err.description
        help_file = [string]$err.help_file
        help_context = [int]$err.help_context
        erl = [int]$err.erl
    }
}

function Get-ExcelOracleHarnessCases {
    $successSource = @'
Option Explicit

Public Function RunProbe() As String
    RunProbe = "success-ok"
End Function
'@
    $compileFailureSource = @'
Option Explicit

Public Function RunProbe() As String
    RunProbe = MissingOracleSymbol(1)
End Function
'@
    $ambiguousMacroSource = @'
Option Explicit

Public Function RunProbe() As String
    Dim capturedDescription As String

    On Error GoTo Handler
    Application.Run "OracleSelfTest.MissingMacro"
    RunProbe = "unexpected-success"
    Exit Function

Handler:
    capturedDescription = Err.Description
    MsgBox capturedDescription, vbOKOnly, "Microsoft Excel"
    RunProbe = "oracle-ambiguous-entry-observed:" & capturedDescription
End Function
'@
    $intrinsicShadowSource = @'
Option Explicit

Public Function Shadowed(ByVal Fix As Double) As Double
    Shadowed = Fix(Fix)
End Function

Public Function RunProbe() As String
    RunProbe = CStr(Shadowed(1.5))
End Function
'@
    $runtimeErrSource = @'
Option Explicit

Public Function RunProbe() As String
    Dim capturedNumber As Long
    Dim capturedSource As String
    Dim capturedDescription As String
    Dim capturedHelpFile As String
    Dim capturedHelpContext As Long
    Dim capturedErl As Long

    On Error GoTo Handler
100 Err.Raise 513, "OracleRuntimeSource", "oracle-runtime-error", "oracle-help.chm", 42
    RunProbe = "unexpected-success"
    Exit Function

Handler:
    capturedNumber = Err.Number
    capturedSource = Err.Source
    capturedDescription = Err.Description
    capturedHelpFile = Err.HelpFile
    capturedHelpContext = Err.HelpContext
    capturedErl = Erl
    RunProbe = "{""number"":" & CStr(capturedNumber) & _
        ",""source"":""" & capturedSource & """" & _
        ",""description"":""" & capturedDescription & """" & _
        ",""help_file"":""" & capturedHelpFile & """" & _
        ",""help_context"":" & CStr(capturedHelpContext) & _
        ",""erl"":" & CStr(capturedErl) & "}"
End Function
'@
    $runtimeUnhandledSource = @'
Option Explicit

Public Sub RunProbe()
100 Err.Raise 13, "OracleUnhandledRuntime", "oracle-unhandled-runtime"
End Sub
'@

    $cases = @(
        [pscustomobject]@{
            id = "success"
            purpose = "clean VBE compile followed by successful runtime invocation"
            module_name = "OracleSelfTest"
            module_source = $successSource
            expected_compile_status = "ok"
            run_procedure = "OracleSelfTest.RunProbe"
            target_exists = $true
            expected_run_status = "ok"
            expected_value = "success-ok"
        },
        [pscustomobject]@{
            id = "compile-failure"
            purpose = "forced VBE compile error with selected token and expanded source line"
            module_name = "OracleSelfTest"
            module_source = $compileFailureSource
            expected_compile_status = "compile-error"
            run_procedure = $null
            target_exists = $true
            expected_run_status = "not-run"
            expected_value = $null
            expected_selected_token = "MissingOracleSymbol"
            expected_expanded_line = "RunProbe = MissingOracleSymbol(1)"
        },
        [pscustomobject]@{
            id = "ambiguous-macro-failure"
            purpose = "generic Application.Run macro failure classified only after a clean forced compile"
            module_name = "OracleSelfTest"
            module_source = $ambiguousMacroSource
            expected_compile_status = "ok"
            run_procedure = "OracleSelfTest.RunProbe"
            target_exists = $false
            macro_probe_target = "OracleSelfTest.MissingMacro"
            invocation_observation_prefix = "oracle-ambiguous-entry-observed:"
            expected_run_status = "missing-macro"
            expected_value = $null
        },
        [pscustomobject]@{
            id = "intrinsic-shadow"
            purpose = "Fix parameter shadowing surfaces at the call site while the declaration remains the defect candidate"
            module_name = "OracleSelfTest"
            module_source = $intrinsicShadowSource
            expected_compile_status = "compile-error"
            run_procedure = $null
            target_exists = $true
            expected_run_status = "not-run"
            expected_value = $null
            expected_selected_token = "Fix"
            expected_expanded_line = "Shadowed = Fix(Fix)"
        },
        [pscustomobject]@{
            id = "runtime-full-err"
            purpose = "clean compile and caught runtime failure returning complete Err state"
            module_name = "OracleSelfTest"
            module_source = $runtimeErrSource
            expected_compile_status = "ok"
            run_procedure = "OracleSelfTest.RunProbe"
            target_exists = $true
            expected_run_status = "runtime-err-captured"
            expected_value = $null
        },
        [pscustomobject]@{
            id = "runtime-unhandled-modal"
            purpose = "clean compile followed by an unhandled VBA error captured as an owned runtime modal"
            module_name = "OracleSelfTest"
            module_source = $runtimeUnhandledSource
            expected_compile_status = "ok"
            run_procedure = "OracleSelfTest.RunProbe"
            target_exists = $true
            expected_run_status = "runtime-error-modal"
            expected_value = $null
            diagnostic_only = $true
        }
    )
    foreach ($case in $cases) {
        if ($case.PSObject.Properties.Name -notcontains "diagnostic_only") {
            $case | Add-Member -NotePropertyName diagnostic_only -NotePropertyValue $false
        }
        if ($case.PSObject.Properties.Name -notcontains "macro_probe_target") {
            $case | Add-Member -NotePropertyName macro_probe_target -NotePropertyValue $case.run_procedure
        }
        if ($case.PSObject.Properties.Name -notcontains "expected_selected_token") {
            $case | Add-Member -NotePropertyName expected_selected_token -NotePropertyValue $null
            $case | Add-Member -NotePropertyName expected_expanded_line -NotePropertyValue $null
        }
        if ($case.PSObject.Properties.Name -notcontains "invocation_observation_prefix") {
            $case | Add-Member -NotePropertyName invocation_observation_prefix -NotePropertyValue $null
        }
    }
    return $cases
}

function Get-ExcelOracleExpectedRuntimeErr {
    return [pscustomobject]@{
        number = 513
        source = "OracleRuntimeSource"
        description = "oracle-runtime-error"
        help_file = "oracle-help.chm"
        help_context = 42
        erl = 100
    }
}

function Test-ExcelOracleRuntimeErrFieldEqual {
    param($Left, $Right)

    if ($Left -is [string] -or $Right -is [string]) {
        return $Left -is [string] -and $Right -is [string] -and
            [string]::Equals([string]$Left, [string]$Right, [StringComparison]::Ordinal)
    }
    if (($Left -is [int] -or $Left -is [long]) -and ($Right -is [int] -or $Right -is [long])) {
        return [long]$Left -eq [long]$Right
    }
    return $false
}

function Get-ExcelOracleSha256 {
    param([Parameter(Mandatory = $true)][string]$Text)

    $bytes = [Text.UTF8Encoding]::new($false).GetBytes(($Text -replace "`r`n", "`n"))
    $hash = [Security.Cryptography.SHA256]::HashData($bytes)
    return "sha256:$([Convert]::ToHexString($hash).ToLowerInvariant())"
}

function Get-ExcelOracleExactTextSha256 {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)

    $bytes = [Text.UTF8Encoding]::new($false).GetBytes($Text)
    return Get-ExcelOracleBytesSha256 -Bytes $bytes
}

function Get-ExcelOracleBytesSha256 {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][byte[]]$Bytes)

    $hash = [Security.Cryptography.SHA256]::HashData($Bytes)
    return "sha256:$([Convert]::ToHexString($hash).ToLowerInvariant())"
}

function Get-ExcelOracleObjectSequenceDigest {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Values)

    $json = @($Values | ForEach-Object { $_ | ConvertTo-Json -Compress -Depth 20 }) -join "`n"
    return Get-ExcelOracleExactTextSha256 -Text $json
}

function Get-ExcelOracleEvidenceUtcValues {
    param([AllowNull()]$Value, [string]$Path = "evidence")

    $values = [Collections.Generic.List[object]]::new()
    if ($null -eq $Value) { return @($values) }
    if ($Value -is [array]) {
        for ($index = 0; $index -lt $Value.Count; $index++) {
            foreach ($nested in @(Get-ExcelOracleEvidenceUtcValues -Value $Value[$index] -Path "$Path[$index]")) { $values.Add($nested) }
        }
        return @($values)
    }
    if ($Value -is [string] -or $Value -is [ValueType]) { return @($values) }
    foreach ($property in @($Value.PSObject.Properties)) {
        $propertyPath = "$Path.$($property.Name)"
        if ([string]$property.Name -match '(?i)_utc$') {
            $values.Add([pscustomobject]@{ path = $propertyPath; value = $property.Value })
        }
        else {
            foreach ($nested in @(Get-ExcelOracleEvidenceUtcValues -Value $property.Value -Path $propertyPath)) { $values.Add($nested) }
        }
    }
    return @($values)
}

function Get-ExcelOracleCaseEvidenceContract {
    param([Parameter(Mandatory = $true)][string]$CaseId)
    switch ($CaseId) {
        "success" { return "clean-compile-run-no-dialog-v1" }
        "compile-failure" { return "compile-error-token-line-dismissal-v1" }
        "ambiguous-macro-failure" { return "clean-compile-ambiguous-macro-dismissal-v1" }
        "intrinsic-shadow" { return "compile-error-token-line-dismissal-v1" }
        "runtime-full-err" { return "clean-compile-full-err-no-dialog-v1" }
        "runtime-unhandled-modal" { return "clean-compile-runtime-error-dismissal-v1" }
        default { throw "excel-vba-oracle-contract: unknown case evidence contract '$CaseId'" }
    }
}

function Get-ExcelOracleSelectedCaseDescriptorPayload {
    param([Parameter(Mandatory = $true)]$Descriptor)
    return [ordered]@{
        schema = "oxvba.excel-vba-oracle-selected-case.v1"
        id = [string]$Descriptor.id
        purpose = [string]$Descriptor.purpose
        module_name = [string]$Descriptor.module_name
        module_source = [string]$Descriptor.module_source
        module_sha256 = [string]$Descriptor.module_sha256
        expected_compile_status = [string]$Descriptor.expected_compile_status
        expected_run_status = [string]$Descriptor.expected_run_status
        run_procedure = if ($null -eq $Descriptor.run_procedure) { $null } else { [string]$Descriptor.run_procedure }
        diagnostic_only = [bool]$Descriptor.diagnostic_only
        expected_value = if ($null -eq $Descriptor.expected_value) { $null } else { [string]$Descriptor.expected_value }
        expected_selected_token = if ($null -eq $Descriptor.expected_selected_token) { $null } else { [string]$Descriptor.expected_selected_token }
        expected_expanded_line = if ($null -eq $Descriptor.expected_expanded_line) { $null } else { [string]$Descriptor.expected_expanded_line }
        macro_probe_target = if ($null -eq $Descriptor.macro_probe_target) { $null } else { [string]$Descriptor.macro_probe_target }
        invocation_observation_prefix = if ($null -eq $Descriptor.invocation_observation_prefix) { $null } else { [string]$Descriptor.invocation_observation_prefix }
        evidence_contract = [string]$Descriptor.evidence_contract
        expected_runtime_err = $Descriptor.expected_runtime_err
    }
}

function New-ExcelOracleSelectedCaseDescriptors {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Cases)
    $descriptors = [Collections.Generic.List[object]]::new()
    foreach ($case in $Cases) {
        $descriptor = [pscustomobject](Get-ExcelOracleSelectedCaseDescriptorPayload -Descriptor ([pscustomobject]@{
            id = $case.id
            purpose = $case.purpose
            module_name = $case.module_name
            module_source = $case.module_source
            module_sha256 = Get-ExcelOracleSha256 -Text ([string]$case.module_source)
            expected_compile_status = $case.expected_compile_status
            expected_run_status = $case.expected_run_status
            run_procedure = $case.run_procedure
            diagnostic_only = $case.diagnostic_only
            expected_value = $case.expected_value
            expected_selected_token = $case.expected_selected_token
            expected_expanded_line = $case.expected_expanded_line
            macro_probe_target = $case.macro_probe_target
            invocation_observation_prefix = $case.invocation_observation_prefix
            evidence_contract = Get-ExcelOracleCaseEvidenceContract -CaseId ([string]$case.id)
            expected_runtime_err = if ([string]$case.id -eq "runtime-full-err") { Get-ExcelOracleExpectedRuntimeErr } else { $null }
        }))
        $payloadJson = (Get-ExcelOracleSelectedCaseDescriptorPayload -Descriptor $descriptor) | ConvertTo-Json -Compress -Depth 8
        $descriptor | Add-Member -NotePropertyName descriptor_sha256 -NotePropertyValue (Get-ExcelOracleSha256 -Text $payloadJson)
        $descriptors.Add($descriptor)
    }
    return @($descriptors)
}

function Test-ExcelOracleSelectedCaseDescriptor {
    param([Parameter(Mandatory = $true)][AllowNull()]$Descriptor)
    $expectedFields = @(
        "schema", "id", "purpose", "module_name", "module_source", "module_sha256", "expected_compile_status", "expected_run_status",
        "run_procedure", "diagnostic_only", "expected_value", "expected_selected_token", "expected_expanded_line", "macro_probe_target",
        "invocation_observation_prefix", "evidence_contract", "expected_runtime_err", "descriptor_sha256"
    )
    if ($null -eq $Descriptor -or
        (@($Descriptor.PSObject.Properties.Name | Sort-Object) -join "`n") -cne (@($expectedFields | Sort-Object) -join "`n")) {
        return $false
    }
    try { $expectedEvidenceContract = Get-ExcelOracleCaseEvidenceContract -CaseId ([string]$Descriptor.id) }
    catch { return $false }
    if (
        $Descriptor.schema -isnot [string] -or [string]$Descriptor.schema -cne "oxvba.excel-vba-oracle-selected-case.v1" -or
        $Descriptor.id -isnot [string] -or [string]::IsNullOrWhiteSpace([string]$Descriptor.id) -or
        $Descriptor.purpose -isnot [string] -or [string]::IsNullOrWhiteSpace([string]$Descriptor.purpose) -or
        $Descriptor.module_name -isnot [string] -or [string]::IsNullOrWhiteSpace([string]$Descriptor.module_name) -or
        $Descriptor.module_source -isnot [string] -or $Descriptor.module_sha256 -isnot [string] -or
        [string]$Descriptor.module_sha256 -cne (Get-ExcelOracleSha256 -Text ([string]$Descriptor.module_source)) -or
        $Descriptor.expected_compile_status -isnot [string] -or $Descriptor.expected_run_status -isnot [string] -or
        $Descriptor.diagnostic_only -isnot [bool] -or $Descriptor.evidence_contract -isnot [string] -or
        [string]$Descriptor.evidence_contract -cne $expectedEvidenceContract -or
        $Descriptor.descriptor_sha256 -isnot [string]) {
        return $false
    }
    foreach ($nullableString in @("run_procedure", "expected_value", "expected_selected_token", "expected_expanded_line", "macro_probe_target", "invocation_observation_prefix")) {
        if ($null -ne $Descriptor.$nullableString -and $Descriptor.$nullableString -isnot [string]) { return $false }
    }
    if ([string]$Descriptor.id -eq "runtime-full-err") {
        $expectedErr = Get-ExcelOracleExpectedRuntimeErr
        if ($null -eq $Descriptor.expected_runtime_err -or
            @(@("number", "source", "description", "help_file", "help_context", "erl") | Where-Object {
                $Descriptor.expected_runtime_err.PSObject.Properties.Name -notcontains $_ -or
                -not (Test-ExcelOracleRuntimeErrFieldEqual -Left $Descriptor.expected_runtime_err.$_ -Right $expectedErr.$_)
            }).Count -gt 0) {
            return $false
        }
    }
    elseif ($null -ne $Descriptor.expected_runtime_err) { return $false }
    $payloadJson = (Get-ExcelOracleSelectedCaseDescriptorPayload -Descriptor $Descriptor) | ConvertTo-Json -Compress -Depth 8
    return [string]$Descriptor.descriptor_sha256 -cne "" -and
        [string]$Descriptor.descriptor_sha256 -ceq (Get-ExcelOracleSha256 -Text $payloadJson)
}

function Get-ExcelOracleSelectedCaseDescriptorSequenceDigest {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Descriptors)
    if ($Descriptors.Count -eq 0 -or
        @($Descriptors | Where-Object { -not (Test-ExcelOracleSelectedCaseDescriptor -Descriptor $_) }).Count -gt 0) {
        throw "excel-vba-oracle-contract: cannot digest an invalid selected descriptor sequence"
    }
    $canonical = @($Descriptors | ForEach-Object {
        $payload = Get-ExcelOracleSelectedCaseDescriptorPayload -Descriptor $_
        $payload["descriptor_sha256"] = [string]$_.descriptor_sha256
        [pscustomobject]$payload
    })
    return Get-ExcelOracleSha256 -Text ($canonical | ConvertTo-Json -Compress -Depth 12)
}

function New-ExcelOracleSelectedCaseDescriptorEnvelope {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Descriptors)
    $digest = Get-ExcelOracleSelectedCaseDescriptorSequenceDigest -Descriptors $Descriptors
    return [pscustomobject][ordered]@{
        schema = "oxvba.excel-vba-oracle-selected-case-sequence.v1"
        descriptor_count = $Descriptors.Count
        aggregate_sha256 = $digest
        descriptors = @($Descriptors)
    }
}

function Read-ExcelOracleSelectedCaseDescriptorEnvelope {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ExpectedAggregateSha256
    )
    try { $envelope = Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json -DateKind String }
    catch { throw "excel-vba-oracle-contract: selected descriptor envelope cannot be read: $($_.Exception.Message)" }
    $fields = @("schema", "descriptor_count", "aggregate_sha256", "descriptors")
    if ($null -eq $envelope -or
        (@($envelope.PSObject.Properties.Name | Sort-Object) -join "`n") -cne (@($fields | Sort-Object) -join "`n") -or
        $envelope.schema -isnot [string] -or [string]$envelope.schema -cne "oxvba.excel-vba-oracle-selected-case-sequence.v1" -or
        ($envelope.descriptor_count -isnot [int] -and $envelope.descriptor_count -isnot [long]) -or
        $envelope.descriptors -isnot [array] -or [int]$envelope.descriptor_count -ne @($envelope.descriptors).Count -or
        $envelope.aggregate_sha256 -isnot [string] -or [string]$envelope.aggregate_sha256 -cne $ExpectedAggregateSha256) {
        throw "excel-vba-oracle-contract: selected descriptor envelope shape/digest declaration is invalid"
    }
    $descriptors = @($envelope.descriptors)
    $actualDigest = Get-ExcelOracleSelectedCaseDescriptorSequenceDigest -Descriptors $descriptors
    if ($actualDigest -cne $ExpectedAggregateSha256) {
        throw "excel-vba-oracle-contract: selected descriptor sequence changed after supervisor sealing"
    }
    return [pscustomobject]@{ descriptors = $descriptors; aggregate_sha256 = $actualDigest }
}

function Enter-ExcelOracleRunClaim {
    param(
        [Parameter(Mandatory = $true)][string]$OutputBase,
        [Parameter(Mandatory = $true)][string]$RunId,
        [scriptblock]$RemoveClaim = { param($Path) Remove-Item -LiteralPath $Path -Force -ErrorAction Stop }
    )

    if ([string]::IsNullOrWhiteSpace($RunId) -or $RunId.IndexOfAny([IO.Path]::GetInvalidFileNameChars()) -ge 0 -or
        $RunId.Contains([IO.Path]::DirectorySeparatorChar) -or $RunId.Contains([IO.Path]::AltDirectorySeparatorChar)) {
        throw "excel-vba-oracle-contract: RunId is not a single safe path segment"
    }
    [void][IO.Directory]::CreateDirectory($OutputBase)
    $claimPath = Join-Path $OutputBase ".$RunId.run-claim"
    try {
        $stream = [IO.File]::Open($claimPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    }
    catch {
        throw "excel-vba-oracle-contract: atomic run claim already exists for '$RunId': $($_.Exception.Message)"
    }
    $outputDirectory = Join-Path $OutputBase $RunId
    try {
        if ([IO.Directory]::Exists($outputDirectory) -or [IO.File]::Exists($outputDirectory)) {
            throw "run directory already exists; refusing stale state: $outputDirectory"
        }
        [void][IO.Directory]::CreateDirectory($outputDirectory)
        $claimDocument = [Text.UTF8Encoding]::new($false).GetBytes("$RunId`n$PID`n$([DateTime]::UtcNow.ToString('o'))`n")
        $stream.Write($claimDocument, 0, $claimDocument.Length)
        $stream.Flush($true)
        return [pscustomobject]@{
            run_id = $RunId
            output_directory = $outputDirectory
            claim_path = $claimPath
            stream = $stream
        }
    }
    catch {
        $primaryFailure = $_.Exception
        $failedClaim = [pscustomobject]@{ run_id = $RunId; output_directory = $outputDirectory; claim_path = $claimPath; stream = $stream }
        Exit-ExcelOracleRunClaim -Claim $failedClaim -PrimaryFailure $primaryFailure -RemoveClaim $RemoveClaim
        throw $primaryFailure
    }
}

function Exit-ExcelOracleRunClaim {
    param(
        [Parameter(Mandatory = $true)]$Claim,
        [AllowNull()][Exception]$PrimaryFailure = $null,
        [scriptblock]$RemoveClaim = { param($Path) Remove-Item -LiteralPath $Path -Force -ErrorAction Stop }
    )
    $cleanupErrors = [Collections.Generic.List[string]]::new()
    try { $Claim.stream.Dispose() }
    catch { $cleanupErrors.Add("claim stream dispose failed: $($_.Exception.Message)") }
    try { & $RemoveClaim ([string]$Claim.claim_path) }
    catch { $cleanupErrors.Add("claim marker deletion failed: $($_.Exception.Message)") }
    if (Test-Path -LiteralPath ([string]$Claim.claim_path)) {
        $cleanupErrors.Add("claim marker remains after deletion attempt: $([string]$Claim.claim_path)")
    }
    if ($cleanupErrors.Count -gt 0) {
        $cleanupFailure = "excel-vba-oracle-contract: run claim cleanup failed: $($cleanupErrors -join '; ')"
        if ($null -ne $PrimaryFailure) {
            throw "excel-vba-oracle-contract: primary failure: $($PrimaryFailure.Message); $cleanupFailure"
        }
        throw $cleanupFailure
    }
}

function ConvertFrom-ExcelOracleGuardianControl {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Json,
        [Parameter(Mandatory = $true)][string]$RunId
    )

    $errors = [Collections.Generic.List[string]]::new()
    try { $control = $Json | ConvertFrom-Json }
    catch { return [pscustomobject]@{ control = $null; errors = @("malformed control JSON") } }
    if ($null -eq $control) { return [pscustomobject]@{ control = $null; errors = @("null control") } }
    $required = @("schema", "run_id", "case_id", "operation_id", "sequence", "phase", "allow_dismiss", "written_utc")
    $unexpected = @($control.PSObject.Properties.Name | Where-Object { $_ -notin $required })
    $missing = @($required | Where-Object { $control.PSObject.Properties.Name -notcontains $_ })
    if ($missing.Count -gt 0) { $errors.Add("missing control fields: $($missing -join ',')") }
    if ($unexpected.Count -gt 0) { $errors.Add("unexpected control fields: $($unexpected -join ',')") }
    if ($control.PSObject.Properties.Name -contains "schema" -and [string]$control.schema -ne "oxvba.excel-vba-oracle-guardian-control.v2") { $errors.Add("invalid control schema") }
    if ($control.PSObject.Properties.Name -contains "run_id" -and [string]$control.run_id -ne $RunId) { $errors.Add("foreign control run_id") }
    foreach ($field in @("case_id", "operation_id")) {
        if ($control.PSObject.Properties.Name -contains $field -and [string]::IsNullOrWhiteSpace([string]$control.$field)) { $errors.Add("blank control $field") }
    }
    if ($control.PSObject.Properties.Name -contains "phase" -and [string]$control.phase -notin @("compile", "run", "cleanup")) { $errors.Add("invalid control phase") }
    if ($control.PSObject.Properties.Name -contains "allow_dismiss" -and $control.allow_dismiss -isnot [bool]) { $errors.Add("control allow_dismiss is not a JSON boolean") }
    if ($control.PSObject.Properties.Name -contains "sequence" -and ($control.sequence -isnot [long] -and $control.sequence -isnot [int] -or [long]$control.sequence -le 0)) { $errors.Add("control sequence is not a positive integer") }
    if ($control.PSObject.Properties.Name -contains "written_utc") {
        try { [void][DateTime]::Parse([string]$control.written_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind) }
        catch { $errors.Add("invalid control written_utc") }
    }
    return [pscustomobject]@{ control = $control; errors = @($errors) }
}

function Test-ExcelOracleLedgerCaseBinding {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Records,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$ExpectedCaseIds
    )
    $actual = @($Records | ForEach-Object {
        if ($_.PSObject.Properties.Name -contains "case_id") { [string]$_.case_id }
        elseif ($_.PSObject.Properties.Name -contains "id") { [string]$_.id }
        else { "" }
    })
    return $actual.Count -eq $ExpectedCaseIds.Count -and
        @($actual | Select-Object -Unique).Count -eq $actual.Count -and
        (@($actual | Sort-Object) -join "`n") -ceq (@($ExpectedCaseIds | Sort-Object) -join "`n")
}

function Test-ExcelOracleGuardianOperationCoverage {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events,
        [Parameter(Mandatory = $true)][string]$OperationId,
        [Parameter(Mandatory = $true)][long]$ControlSequence,
        [Parameter(Mandatory = $true)][DateTime]$InvocationCompletedUtc
    )
    $armed = @($Events | Where-Object {
        [string]$_.event_type -eq "operation-armed" -and [string]$_.operation_id -eq $OperationId -and [long]$_.control_sequence -eq $ControlSequence
    })
    if ($armed.Count -ne 1) { return $false }
    return @($Events | Where-Object {
        [string]$_.event_type -eq "guardian-heartbeat" -and [string]$_.operation_id -eq $OperationId -and
        [long]$_.control_sequence -eq $ControlSequence -and [long]$_.event_sequence -gt [long]$armed[0].event_sequence -and
        [DateTime]::Parse([string]$_.observed_utc).ToUniversalTime() -ge $InvocationCompletedUtc.ToUniversalTime()
    }).Count -gt 0
}

function Test-ExcelOracleOwnedProcessRecord {
    param(
        [Parameter(Mandatory = $true)]$Record,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][int[]]$BaselineExcelPids,
        [Parameter(Mandatory = $true)][string]$RunId
    )

    foreach ($field in @("run_id", "case_id", "ownership", "pid", "process_name", "process_start_utc", "executable_path", "acquired_utc")) {
        if ($Record.PSObject.Properties.Name -notcontains $field -or [string]::IsNullOrWhiteSpace([string]$Record.$field)) {
            return $false
        }
    }
    if ([string]$Record.run_id -ne $RunId -or [string]$Record.ownership -ne "owned-new-instance") {
        return $false
    }
    if (-not [StringComparer]::OrdinalIgnoreCase.Equals([string]$Record.process_name, "EXCEL") -or
        -not [StringComparer]::OrdinalIgnoreCase.Equals([IO.Path]::GetFileName([string]$Record.executable_path), "EXCEL.EXE")) {
        return $false
    }
    try {
        if ($Record.pid -isnot [long] -and $Record.pid -isnot [int]) { return $false }
        $pidValue = [int]$Record.pid
        $started = [DateTime]::Parse([string]$Record.process_start_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind)
        $acquired = [DateTime]::Parse([string]$Record.acquired_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind)
        if ($acquired.ToUniversalTime() -lt $started.ToUniversalTime()) { return $false }
    }
    catch { return $false }
    return $pidValue -gt 0 -and $pidValue -notin $BaselineExcelPids
}

function Test-ExcelOracleProcessIdentity {
    param(
        [Parameter(Mandatory = $true)]$Record,
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)][string]$ExpectedProcessName,
        [Parameter(Mandatory = $true)][string]$RunId
    )

    return (Get-ExcelOracleProcessIdentityState -Record $Record -Process $Process -ExpectedProcessName $ExpectedProcessName -RunId $RunId) -eq "exact"
}

function Get-ExcelOracleProcessIdentityState {
    param(
        [Parameter(Mandatory = $true)]$Record,
        [Parameter(Mandatory = $true)][AllowNull()][Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)][string]$ExpectedProcessName,
        [Parameter(Mandatory = $true)][string]$RunId
    )

    if ($null -eq $Process) { return "missing" }
    foreach ($field in @("run_id", "pid", "process_name", "process_start_utc", "executable_path")) {
        if ($Record.PSObject.Properties.Name -notcontains $field -or [string]::IsNullOrWhiteSpace([string]$Record.$field)) {
            return "same-instance-conflict"
        }
    }
    try { $recordedPid = [int]$Record.pid }
    catch { return "same-instance-conflict" }
    if ([string]$Record.run_id -ne $RunId -or $recordedPid -ne $Process.Id) { return "same-instance-conflict" }
    try {
        $recordedStart = [DateTime]::Parse(
            [string]$Record.process_start_utc,
            [Globalization.CultureInfo]::InvariantCulture,
            [Globalization.DateTimeStyles]::RoundtripKind
        ).ToUniversalTime()
        $actualStart = $Process.StartTime.ToUniversalTime()
        $actualPath = [IO.Path]::GetFullPath([string]$Process.Path)
        $recordedPath = [IO.Path]::GetFullPath([string]$Record.executable_path)
        if ($recordedStart.Ticks -ne $actualStart.Ticks) { return "pid-reused" }
        if (-not [StringComparer]::OrdinalIgnoreCase.Equals([string]$Record.process_name, $ExpectedProcessName) -or
            -not [StringComparer]::OrdinalIgnoreCase.Equals([string]$Process.ProcessName, $ExpectedProcessName) -or
            -not [StringComparer]::OrdinalIgnoreCase.Equals($recordedPath, $actualPath)) {
            return "same-instance-conflict"
        }
        return "exact"
    }
    catch { return "same-instance-conflict" }
}

function Test-ExcelOracleHelperProcessRecord {
    param(
        [Parameter(Mandatory = $true)]$Record,
        [Parameter(Mandatory = $true)][string]$RunId
    )
    foreach ($field in @("run_id", "case_id", "ownership", "role", "pid", "process_name", "process_start_utc", "executable_path", "acquired_utc")) {
        if ($Record.PSObject.Properties.Name -notcontains $field -or [string]::IsNullOrWhiteSpace([string]$Record.$field)) {
            return $false
        }
    }
    try {
        if ($Record.pid -isnot [long] -and $Record.pid -isnot [int]) { return $false }
        $recordedPid = [int]$Record.pid
        $started = [DateTime]::Parse([string]$Record.process_start_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind)
        $acquired = [DateTime]::Parse([string]$Record.acquired_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind)
        if ($acquired.ToUniversalTime() -lt $started.ToUniversalTime()) { return $false }
    }
    catch { return $false }
    return [string]$Record.run_id -eq $RunId -and
        [string]$Record.ownership -eq "owned-helper-process" -and
        [string]$Record.role -eq "guardian" -and
        [string]$Record.process_name -in @("pwsh", "powershell") -and
        [StringComparer]::OrdinalIgnoreCase.Equals(
            [IO.Path]::GetFileName([string]$Record.executable_path),
            "$(if ([string]$Record.process_name -eq 'pwsh') { 'pwsh' } else { 'powershell' }).exe"
        ) -and
        $recordedPid -gt 0
}

function ConvertFrom-ExcelOracleOwnershipLedger {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Lines,
        [Parameter(Mandatory = $true)][ValidateSet("excel", "guardian")][string]$Kind,
        [Parameter(Mandatory = $true)][string]$RunId,
        [AllowEmptyCollection()][int[]]$BaselineExcelPids = @(),
        [AllowEmptyCollection()][string[]]$ExpectedCaseIds = @()
    )

    $records = [Collections.Generic.List[object]]::new()
    $errors = [Collections.Generic.List[string]]::new()
    $keys = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    for ($index = 0; $index -lt $Lines.Count; $index++) {
        $line = [string]$Lines[$index]
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        try { $record = $line | ConvertFrom-Json }
        catch {
            $errors.Add("line $($index + 1): malformed JSON")
            continue
        }
        if ($null -eq $record) {
            $errors.Add("line $($index + 1): null ownership record")
            continue
        }
        $expectedSchema = if ($Kind -eq "excel") { "oxvba.excel-vba-oracle-owned-process.v1" } else { "oxvba.excel-vba-oracle-owned-helper.v1" }
        if ($record.PSObject.Properties.Name -notcontains "schema" -or [string]$record.schema -ne $expectedSchema) {
            $errors.Add("line $($index + 1): expected schema $expectedSchema")
            continue
        }
        $valid = if ($Kind -eq "excel") {
            Test-ExcelOracleOwnedProcessRecord -Record $record -BaselineExcelPids $BaselineExcelPids -RunId $RunId
        }
        else {
            Test-ExcelOracleHelperProcessRecord -Record $record -RunId $RunId
        }
        if (-not $valid) {
            $errors.Add("line $($index + 1): invalid $Kind ownership identity")
            continue
        }
        if ($ExpectedCaseIds.Count -gt 0 -and [string]$record.case_id -notin $ExpectedCaseIds) {
            $errors.Add("line $($index + 1): ownership record has an unselected case_id")
            continue
        }
        $key = "$([string]$record.run_id)|$([string]$record.pid)|$([string]$record.process_start_utc)"
        if (-not $keys.Add($key)) {
            $errors.Add("line $($index + 1): duplicate $Kind ownership identity")
            continue
        }
        $records.Add($record)
    }
    $duplicateCases = @($records | Group-Object { [string]$_.case_id } | Where-Object Count -gt 1)
    foreach ($duplicate in $duplicateCases) { $errors.Add("duplicate $Kind ownership case_id '$($duplicate.Name)'") }
    return [pscustomobject]@{ records = @($records); errors = @($errors) }
}

function Test-ExcelOracleShouldStopAfterCase {
    param([Parameter(Mandatory = $true)]$CaseResult)
    return $CaseResult.PSObject.Properties.Name -contains "excel_ownership_recorded" -and
        $CaseResult.excel_ownership_recorded -is [bool] -and
        -not [bool]$CaseResult.excel_ownership_recorded -and
        [string]$CaseResult.compile_status -eq "harness-error"
}

function Test-ExcelOracleWindowEnumerationAuthority {
    param(
        [Parameter(Mandatory = $true)][AllowNull()]$Enumeration,
        [Parameter(Mandatory = $true)][int]$ExpectedProcessId
    )
    if ($null -eq $Enumeration -or
        $Enumeration.PSObject.Properties.Name -notcontains "Windows" -or
        $Enumeration.PSObject.Properties.Name -notcontains "Truncated" -or
        $Enumeration.PSObject.Properties.Name -notcontains "Limit" -or
        $Enumeration.PSObject.Properties.Name -notcontains "Succeeded" -or
        $Enumeration.PSObject.Properties.Name -notcontains "ErrorCode" -or
        $Enumeration.Truncated -isnot [bool] -or [bool]$Enumeration.Truncated -or
        $Enumeration.Succeeded -isnot [bool] -or -not [bool]$Enumeration.Succeeded -or
        ($Enumeration.Limit -isnot [int] -and $Enumeration.Limit -isnot [long]) -or [int]$Enumeration.Limit -ne 512 -or
        ($Enumeration.ErrorCode -isnot [int] -and $Enumeration.ErrorCode -isnot [long]) -or [int]$Enumeration.ErrorCode -ne 0 -or
        @($Enumeration.Windows).Count -gt 512) {
        return $false
    }
    return @($Enumeration.Windows | Where-Object {
        $_.PSObject.Properties.Name -notcontains "ProcessId" -or
        ($_.ProcessId -isnot [int] -and $_.ProcessId -isnot [long] -and $_.ProcessId -isnot [uint32]) -or
        [int]$_.ProcessId -ne $ExpectedProcessId
    }).Count -eq 0
}

function Test-ExcelStartupBlockingWindow {
    param([Parameter(Mandatory = $true)]$Window)
    if (-not [bool]$Window.IsTopLevel -or -not [bool]$Window.Visible) { return $false }
    return [string]$Window.ClassName -notin @("XLMAIN", "MSOSPLASH", "MsoSplash")
}

function Resolve-ExcelOracleAttachmentCandidate {
    param(
        [Parameter(Mandatory = $true)]$Enumeration,
        [Parameter(Mandatory = $true)][int]$ExpectedProcessId,
        [Parameter(Mandatory = $true)][AllowNull()]$Candidate,
        [Parameter(Mandatory = $true)][int]$HResult,
        [Parameter(Mandatory = $true)][bool]$NativeObjectPresent,
        [Parameter(Mandatory = $true)][bool]$ApplicationPresent,
        [AllowNull()]$ApplicationPid
    )
    if (-not (Test-ExcelOracleWindowEnumerationAuthority -Enumeration $Enumeration -ExpectedProcessId $ExpectedProcessId)) {
        return [pscustomobject]@{ attached = $false; disposition = "window-enumeration-invalid" }
    }
    if (@($Enumeration.Windows | Where-Object { Test-ExcelStartupBlockingWindow -Window $_ }).Count -gt 0) {
        return [pscustomobject]@{ attached = $false; disposition = "blocked-owned-window" }
    }
    if ($null -eq $Candidate) { return [pscustomobject]@{ attached = $false; disposition = "no-candidate" } }
    foreach ($field in @("Hwnd", "ProcessId", "ClassName")) {
        if ($Candidate.PSObject.Properties.Name -notcontains $field) {
            return [pscustomobject]@{ attached = $false; disposition = "candidate-shape-invalid" }
        }
    }
    $matches = @($Enumeration.Windows | Where-Object {
        [string]$_.Hwnd -ceq [string]$Candidate.Hwnd -and
        [string]$_.ProcessId -ceq [string]$Candidate.ProcessId -and
        [string]$_.ClassName -ceq [string]$Candidate.ClassName
    })
    if ($matches.Count -ne 1) { return [pscustomobject]@{ attached = $false; disposition = "candidate-not-enumerated" } }
    if (($Candidate.ProcessId -isnot [int] -and $Candidate.ProcessId -isnot [long] -and $Candidate.ProcessId -isnot [uint32]) -or
        [int]$Candidate.ProcessId -ne $ExpectedProcessId) {
        return [pscustomobject]@{ attached = $false; disposition = "candidate-pid-mismatch" }
    }
    if ($HResult -lt 0) { return [pscustomobject]@{ attached = $false; disposition = "hresult-failure" } }
    if (-not $NativeObjectPresent) { return [pscustomobject]@{ attached = $false; disposition = "null-native-object" } }
    if (-not $ApplicationPresent) { return [pscustomobject]@{ attached = $false; disposition = "null-application" } }
    if ([string]$Candidate.ClassName -cne "EXCEL7") { return [pscustomobject]@{ attached = $false; disposition = "non-excel7-candidate" } }
    if (($ApplicationPid -isnot [int] -and $ApplicationPid -isnot [long]) -or [int]$ApplicationPid -ne $ExpectedProcessId) {
        return [pscustomobject]@{ attached = $false; disposition = "application-pid-mismatch" }
    }
    return [pscustomobject]@{ attached = $true; disposition = "attached-exact-process-excel7" }
}

function Test-GuardianOperationHealthy {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events)
    $armed = @($Events | Where-Object { [string]$_.event_type -eq "operation-armed" })
    $heartbeats = @($Events | Where-Object { [string]$_.event_type -eq "guardian-heartbeat" })
    if ($armed.Count -ne 1 -or @($heartbeats | Where-Object { [long]$_.event_sequence -gt [long]$armed[0].event_sequence }).Count -eq 0) { return $false }
    $observations = @($Events | Where-Object { [string]$_.event_type -in @("dialog-observation", "ignored-top-level-window") })
    if ($observations.Count -eq 0) { return $false }
    return @($observations | Where-Object {
        [string]$_.event_type -eq "dialog-observation" -and [string]$_.classification -in @("security-or-trust", "unrecognized-modal")
    }).Count -eq 0
}

function Test-LinkedSuccessfulDismissal {
    param(
        [Parameter(Mandatory = $true)]$Observation,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events
    )
    return @($Events | Where-Object {
        [string]$_.event_type -eq "dismissal-result" -and
        [string]$_.observation_id -ceq [string]$Observation.observation_id -and
        [string]$_.operation_id -ceq [string]$Observation.operation_id -and
        $_.succeeded -is [bool] -and [bool]$_.succeeded -and
        -not [string]::IsNullOrWhiteSpace([string]$_.dismissed_button)
    }).Count -eq 1
}

function Test-CompileErrorEvidence {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events,
        [Parameter(Mandatory = $true)][string]$InjectedSource,
        [Parameter(Mandatory = $true)][string]$ExpectedToken,
        [Parameter(Mandatory = $true)][string]$ExpectedLine
    )
    $sourceLines = @($InjectedSource -split "`r?`n" | ForEach-Object { $_.Trim() })
    $dialogs = @($Events | Where-Object { [string]$_.event_type -eq "dialog-observation" })
    if ($dialogs.Count -eq 0 -or @($dialogs | Where-Object { [string]$_.classification -ne "compile-error" }).Count -gt 0) { return $false }
    foreach ($observation in $dialogs) {
        if (-not [string]::IsNullOrWhiteSpace((@($observation.dialog_text) -join " / ")) -and
            [string]$observation.selected_token -ceq $ExpectedToken -and
            [string]$observation.expanded_line.Trim() -ceq $ExpectedLine -and
            [string]$observation.expanded_line.Trim() -in $sourceLines -and
            (Test-LinkedSuccessfulDismissal -Observation $observation -Events $Events)) { return $true }
    }
    return $false
}

function Test-RuntimeErrorEvidence {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events)
    $dialogs = @($Events | Where-Object { [string]$_.event_type -eq "dialog-observation" })
    return $dialogs.Count -eq 1 -and [string]$dialogs[0].classification -eq "runtime-error" -and
        -not [string]::IsNullOrWhiteSpace((@($dialogs[0].dialog_text) -join " / ")) -and
        (Test-LinkedSuccessfulDismissal -Observation $dialogs[0] -Events $Events)
}

function Test-AmbiguousMacroEvidence {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events)
    $dialogs = @($Events | Where-Object { [string]$_.event_type -eq "dialog-observation" })
    if ($dialogs.Count -eq 0 -or @($dialogs | Where-Object { [string]$_.classification -ne "ambiguous-macro-failure" }).Count -gt 0) { return $false }
    foreach ($observation in $dialogs) {
        if (-not [string]::IsNullOrWhiteSpace((@($observation.dialog_text) -join " / ")) -and
            (Test-LinkedSuccessfulDismissal -Observation $observation -Events $Events)) { return $true }
    }
    return $false
}

function Test-NoDialogObservations {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events)
    return @($Events | Where-Object { [string]$_.event_type -eq "dialog-observation" }).Count -eq 0
}

function ConvertTo-ExcelOracleValidatedGuardianEvents {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Events,
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)][string]$CaseId
    )
    if (@($Events | Where-Object { $null -eq $_ }).Count -gt 0) {
        return [pscustomobject]@{ records = @(); errors = @("guardian event collection contains null") }
    }
    try { $lines = @($Events | ForEach-Object { $_ | ConvertTo-Json -Compress -Depth 20 }) }
    catch { return [pscustomobject]@{ records = @(); errors = @("guardian event collection cannot be serialized") } }
    return ConvertFrom-ExcelOracleGuardianEventLedger -Lines $lines -RunId $RunId -ExpectedCaseIds @($CaseId)
}

function Resolve-ExcelOraclePostCleanupResult {
    param(
        [Parameter(Mandatory = $true)][AllowNull()]$Results,
        [Parameter(Mandatory = $true)][AllowNull()]$ExcelLedger,
        [Parameter(Mandatory = $true)][AllowNull()]$HelperLedger,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$SupervisorGuardianEvidence,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$SelectedCaseDescriptors,
        [Parameter(Mandatory = $true)][string]$ExpectedOutputDirectory,
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)][int]$ExpectedWorkerPid,
        [Parameter(Mandatory = $true)][string]$ExpectedWorkerStartUtc,
        [Parameter(Mandatory = $true)][string]$ExpectedWorkerExecutablePath,
        [Parameter(Mandatory = $true)][string]$ExpectedContainmentToken,
        [Parameter(Mandatory = $true)][bool]$ExpectedDiagnosticOnly,
        [Parameter(Mandatory = $true)][int]$WorkerExitCode,
        [Parameter(Mandatory = $true)][bool]$WorkerQuiesced,
        [Parameter(Mandatory = $true)][bool]$WorkerTimedOut
    )

    $errors = [Collections.Generic.List[string]]::new()
    $disposition = "invalid"
    $transport = $null
    if ($SelectedCaseDescriptors.Count -eq 0 -or
        @($SelectedCaseDescriptors | Where-Object { -not (Test-ExcelOracleSelectedCaseDescriptor -Descriptor $_) }).Count -gt 0) {
        $errors.Add("selected case descriptor sequence is invalid")
        return [pscustomobject]@{ valid = $false; disposition = $disposition; transport_error = $transport; errors = @($errors) }
    }
    $expectedOutputFullPath = $null
    try {
        if (-not [IO.Path]::IsPathFullyQualified($ExpectedOutputDirectory)) { throw "not fully qualified" }
        $expectedOutputFullPath = [IO.Path]::GetFullPath($ExpectedOutputDirectory)
        if (-not [StringComparer]::OrdinalIgnoreCase.Equals($expectedOutputFullPath, $ExpectedOutputDirectory)) { throw "not canonical" }
    }
    catch {
        $errors.Add("expected output directory is not a canonical fully qualified path")
        return [pscustomobject]@{ valid = $false; disposition = $disposition; transport_error = $transport; errors = @($errors) }
    }
    $expectedCaseIds = @($SelectedCaseDescriptors | ForEach-Object { [string]$_.id })
    if (@($expectedCaseIds | Select-Object -Unique).Count -ne $expectedCaseIds.Count) {
        $errors.Add("selected case descriptor identities are not unique")
    }
    if (-not $WorkerQuiesced -or $WorkerTimedOut) { $errors.Add("worker exit envelope is not quiesced/non-timeout") }
    foreach ($ledgerEntry in @(@("excel", $ExcelLedger), @("guardian", $HelperLedger))) {
        $name = [string]$ledgerEntry[0]
        $ledger = $ledgerEntry[1]
        if ($null -eq $ledger -or $ledger.PSObject.Properties.Name -notcontains "records" -or $ledger.PSObject.Properties.Name -notcontains "errors") {
            $errors.Add("$name ledger result shape is invalid")
            continue
        }
        if ($ledger.records -isnot [array] -or $ledger.errors -isnot [array]) { $errors.Add("$name ledger collections are not arrays") }
        if (@($ledger.errors).Count -gt 0) { $errors.Add("$name ledger has authority errors") }
    }
    [object[]]$excelRecords = [object[]]::new(0)
    [object[]]$helperRecords = [object[]]::new(0)
    if ($null -ne $ExcelLedger -and $ExcelLedger.PSObject.Properties.Name -contains "records") { $excelRecords = [object[]]@($ExcelLedger.records) }
    if ($null -ne $HelperLedger -and $HelperLedger.PSObject.Properties.Name -contains "records") { $helperRecords = [object[]]@($HelperLedger.records) }

    $supervisorGuardianByCase = @{}
    foreach ($guardianEvidence in @($SupervisorGuardianEvidence)) {
        $requiredGuardianEvidenceFields = @("schema", "case_id", "ledger_path", "ledger_sha256", "raw_base64")
        if ($null -eq $guardianEvidence -or
            (@($guardianEvidence.PSObject.Properties.Name | Sort-Object) -join "`n") -cne (@($requiredGuardianEvidenceFields | Sort-Object) -join "`n") -or
            $guardianEvidence.schema -isnot [string] -or [string]$guardianEvidence.schema -cne "oxvba.excel-vba-oracle-supervisor-guardian-ledger.v1" -or
            $guardianEvidence.case_id -isnot [string] -or $guardianEvidence.ledger_path -isnot [string] -or
            $guardianEvidence.ledger_sha256 -isnot [string] -or $guardianEvidence.raw_base64 -isnot [string]) {
            $errors.Add("supervisor guardian evidence shape is invalid")
            continue
        }
        $caseId = [string]$guardianEvidence.case_id
        if (@($expectedCaseIds | Where-Object { [string]$_ -ceq $caseId }).Count -ne 1 -or $supervisorGuardianByCase.ContainsKey($caseId)) {
            $errors.Add("supervisor guardian evidence case identity is unselected or duplicated")
            continue
        }
        $expectedLedgerPath = [IO.Path]::GetFullPath((Join-Path (Join-Path $expectedOutputFullPath $caseId) "guardian-events.jsonl"))
        if (-not [StringComparer]::OrdinalIgnoreCase.Equals([string]$guardianEvidence.ledger_path, $expectedLedgerPath)) {
            $errors.Add("supervisor guardian evidence path is not the exact case ledger path")
            continue
        }
        try {
            $guardianBytes = [Convert]::FromBase64String([string]$guardianEvidence.raw_base64)
            if ([string]$guardianEvidence.ledger_sha256 -cne (Get-ExcelOracleBytesSha256 -Bytes $guardianBytes)) {
                throw "digest mismatch"
            }
            $guardianRawText = [Text.UTF8Encoding]::new($false, $true).GetString($guardianBytes)
            [string[]]$guardianLines = [string[]]@($guardianRawText -split "`r?`n" | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) })
            $parsedGuardian = ConvertFrom-ExcelOracleGuardianEventLedger -Lines $guardianLines -RunId $RunId -ExpectedCaseIds @($caseId)
            if (@($parsedGuardian.errors).Count -gt 0) { throw "invalid ledger: $(@($parsedGuardian.errors) -join '; ')" }
            $supervisorGuardianByCase[$caseId] = [pscustomobject]@{
                ledger_path = $expectedLedgerPath
                ledger_sha256 = [string]$guardianEvidence.ledger_sha256
                records = @($parsedGuardian.records)
            }
        }
        catch { $errors.Add("supervisor guardian evidence for '$caseId' is invalid: $($_.Exception.Message)") }
    }
    if ($null -eq $Results) {
        $errors.Add("results document is missing")
        return [pscustomobject]@{ valid = $false; disposition = $disposition; transport_error = $transport; errors = @($errors) }
    }

    $requiredDocumentFields = @("schema", "run_id", "generated_utc", "worker_pid", "containment_token", "containment_authority", "selected_case_descriptor_digest", "diagnostic_only", "cases", "passed")
    $actualDocumentFields = @($Results.PSObject.Properties.Name)
    if ((@($actualDocumentFields | Sort-Object) -join "`n") -cne (@($requiredDocumentFields | Sort-Object) -join "`n")) {
        $errors.Add("results document field set is invalid")
        return [pscustomobject]@{ valid = $false; disposition = $disposition; transport_error = $transport; errors = @($errors) }
    }
    if ($Results.schema -isnot [string] -or $Results.run_id -isnot [string] -or
        [string]$Results.schema -cne "oxvba.excel-vba-oracle-results.v1" -or [string]$Results.run_id -cne $RunId) {
        $errors.Add("results schema or run identity is invalid")
    }
    $generatedUtc = $null
    try {
        if ($Results.generated_utc -isnot [string]) { throw "not a string" }
        $generatedUtc = [DateTime]::Parse([string]$Results.generated_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind).ToUniversalTime()
    }
    catch { $errors.Add("results generated_utc is invalid") }
    if (($Results.worker_pid -isnot [long] -and $Results.worker_pid -isnot [int]) -or [int]$Results.worker_pid -ne $ExpectedWorkerPid) {
        $errors.Add("results worker_pid is not the exact worker")
    }
    if ($Results.containment_token -isnot [string] -or [string]$Results.containment_token -cne $ExpectedContainmentToken) { $errors.Add("results containment token is invalid") }
    $expectedDescriptorDigest = Get-ExcelOracleSelectedCaseDescriptorSequenceDigest -Descriptors $SelectedCaseDescriptors
    if ($Results.selected_case_descriptor_digest -isnot [string] -or [string]$Results.selected_case_descriptor_digest -cne $expectedDescriptorDigest) {
        $errors.Add("results selected descriptor aggregate digest is invalid")
    }
    if ($Results.diagnostic_only -isnot [bool] -or [bool]$Results.diagnostic_only -ne $ExpectedDiagnosticOnly) {
        $errors.Add("results diagnostic_only is not the expected JSON Boolean")
    }
    if ($Results.passed -isnot [bool]) { $errors.Add("results passed is not a JSON Boolean") }
    if ($Results.cases -isnot [array]) { $errors.Add("results cases is not a JSON array") }

    $authority = $Results.containment_authority
    $requiredAuthorityFields = @("schema", "run_id", "containment_token", "worker_pid", "worker_process_start_utc", "worker_executable_path", "worker_job_membership_verified", "published_utc")
    if ($null -eq $authority -or
        (@($authority.PSObject.Properties.Name | Sort-Object) -join "`n") -cne (@($requiredAuthorityFields | Sort-Object) -join "`n") -or
        $authority.schema -isnot [string] -or $authority.run_id -isnot [string] -or $authority.containment_token -isnot [string] -or
        $authority.worker_process_start_utc -isnot [string] -or $authority.worker_executable_path -isnot [string] -or $authority.published_utc -isnot [string] -or
        [string]$authority.schema -cne "oxvba.excel-vba-oracle-containment-ready.v1" -or
        [string]$authority.run_id -cne $RunId -or [string]$authority.containment_token -cne $ExpectedContainmentToken -or
        ($authority.worker_pid -isnot [long] -and $authority.worker_pid -isnot [int]) -or [int]$authority.worker_pid -ne $ExpectedWorkerPid -or
        [string]$authority.worker_process_start_utc -cne $ExpectedWorkerStartUtc -or
        -not [StringComparer]::OrdinalIgnoreCase.Equals([string]$authority.worker_executable_path, $ExpectedWorkerExecutablePath) -or
        $authority.worker_job_membership_verified -isnot [bool] -or -not [bool]$authority.worker_job_membership_verified -or
        [string]::IsNullOrWhiteSpace([string]$authority.worker_process_start_utc) -or
        [string]::IsNullOrWhiteSpace([string]$authority.worker_executable_path) -or
        [string]::IsNullOrWhiteSpace([string]$authority.published_utc)) {
        $errors.Add("results containment authority is invalid")
    }
    else {
        try {
            $expectedWorkerStart = [DateTime]::Parse($ExpectedWorkerStartUtc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind).ToUniversalTime()
            $authorityWorkerStart = [DateTime]::Parse([string]$authority.worker_process_start_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind).ToUniversalTime()
            $authorityPublished = [DateTime]::Parse([string]$authority.published_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind).ToUniversalTime()
            if ($authorityWorkerStart -ne $expectedWorkerStart -or $authorityPublished -lt $authorityWorkerStart -or $null -eq $generatedUtc -or $generatedUtc -le $authorityPublished) {
                throw "worker start/publish/result timestamp order is invalid"
            }
        }
        catch { $errors.Add("results containment authority timestamps are invalid") }
    }

    $requiredCaseFields = @(
        "schema", "id", "purpose", "passed", "owned_excel_pid", "observed_excel_pid", "excel_ownership_recorded",
        "selected_case_descriptor_sha256", "module_name", "module_path", "module_sha256", "case_diagnostic_only", "evidence_contract",
        "compile_status", "expected_compile_status", "compile_command", "compile_execution", "compile_context", "post_dismiss_selection_diagnostic_only",
        "compile_dialogs", "compile_window_observations", "run_procedure", "run_status", "expected_run_status", "run_value", "runtime_err",
        "macro_failure_disposition", "runtime_measurement", "transport_error", "run_dialogs", "evidence_status", "cleanup_status",
        "cleanup_authority_errors", "bootstrap_workbook", "defect_declaration"
    )
    $cases = @($Results.cases)
    $caseFieldSetInvalid = $false
    $derivedCasePasses = [Collections.Generic.List[bool]]::new()
    for ($index = 0; $index -lt $cases.Count; $index++) {
        $case = $cases[$index]
        if ($null -eq $case -or
            (@($case.PSObject.Properties.Name | Sort-Object) -join "`n") -cne (@($requiredCaseFields | Sort-Object) -join "`n")) {
            $errors.Add("case result $index field set is invalid")
            $caseFieldSetInvalid = $true
            continue
        }
        $descriptor = if ($index -lt $SelectedCaseDescriptors.Count) { $SelectedCaseDescriptors[$index] } else { $null }
        if ($null -eq $descriptor) {
            $errors.Add("case result $index has no selected descriptor")
            $derivedCasePasses.Add($false)
            continue
        }
        $expectedCaseDirectory = [IO.Path]::GetFullPath((Join-Path $expectedOutputFullPath ([string]$descriptor.id)))
        $expectedModulePath = [IO.Path]::GetFullPath((Join-Path $expectedCaseDirectory "$([string]$descriptor.module_name).bas"))
        $expectedBootstrapPath = [IO.Path]::GetFullPath((Join-Path $expectedCaseDirectory "oracle-bootstrap.xlsx"))
        $requiredStringFields = @("schema", "id", "purpose", "selected_case_descriptor_sha256", "module_name", "module_path", "module_sha256", "evidence_contract", "compile_status", "expected_compile_status", "run_status", "expected_run_status", "cleanup_status")
        if (@($requiredStringFields | Where-Object { $case.$_ -isnot [string] -or [string]::IsNullOrWhiteSpace([string]$case.$_) }).Count -gt 0 -or
            [string]$case.schema -cne "oxvba.excel-vba-oracle-case-result.v1" -or [string]$case.module_sha256 -notmatch '^sha256:[0-9a-f]{64}$') {
            $errors.Add("case result $index scalar identity is invalid")
        }
        if (-not [StringComparer]::OrdinalIgnoreCase.Equals([string]$case.module_path, $expectedModulePath)) {
            $errors.Add("case result $index module path is not the exact supervisor-derived case path")
        }
        if ($case.passed -isnot [bool] -or $case.excel_ownership_recorded -isnot [bool] -or $case.case_diagnostic_only -isnot [bool]) {
            $errors.Add("case result $index Boolean status is invalid")
        }
        if ($null -ne $case.run_procedure -and $case.run_procedure -isnot [string]) { $errors.Add("case result $index run procedure type is invalid") }
        if ($case.compile_dialogs -isnot [array] -or $case.compile_window_observations -isnot [array] -or
            $case.run_dialogs -isnot [array] -or $case.cleanup_authority_errors -isnot [array] -or
            @($case.compile_dialogs).Count -ne @($case.compile_dialogs | Where-Object { $null -ne $_ }).Count -or
            @($case.compile_window_observations).Count -ne @($case.compile_window_observations | Where-Object { $null -ne $_ }).Count -or
            @($case.run_dialogs).Count -ne @($case.run_dialogs | Where-Object { $null -ne $_ }).Count -or
            @($case.cleanup_authority_errors | Where-Object { $_ -isnot [string] }).Count -gt 0) {
            $errors.Add("case result $index collection type is invalid")
        }
        if (@($case.cleanup_authority_errors).Count -gt 0) { $errors.Add("case result $index reports cleanup authority errors") }
        if ([bool]$case.excel_ownership_recorded) {
            if (($case.owned_excel_pid -isnot [long] -and $case.owned_excel_pid -isnot [int]) -or [int]$case.owned_excel_pid -le 0) {
                $errors.Add("case result $index durable Excel PID is invalid")
            }
            $bootstrap = $case.bootstrap_workbook
            $requiredBootstrapFields = @("schema", "path", "sha256", "sha256_after", "package_parts", "macro_free")
            if ($null -eq $bootstrap -or
                (@($bootstrap.PSObject.Properties.Name | Sort-Object) -join "`n") -cne (@($requiredBootstrapFields | Sort-Object) -join "`n") -or
                $bootstrap.schema -isnot [string] -or $bootstrap.path -isnot [string] -or $bootstrap.sha256 -isnot [string] -or $bootstrap.sha256_after -isnot [string] -or
                [string]$bootstrap.schema -cne "oxvba.excel-vba-oracle-bootstrap-workbook.v1" -or
                [string]::IsNullOrWhiteSpace([string]$bootstrap.path) -or
                -not [StringComparer]::OrdinalIgnoreCase.Equals([string]$bootstrap.path, $expectedBootstrapPath) -or
                [string]$bootstrap.sha256 -notmatch '^sha256:[0-9a-f]{64}$' -or
                [string]$bootstrap.sha256_after -cne [string]$bootstrap.sha256 -or
                $bootstrap.macro_free -isnot [bool] -or -not [bool]$bootstrap.macro_free -or $bootstrap.package_parts -isnot [array] -or
                (@($bootstrap.package_parts) -join "`n") -cne (@("[Content_Types].xml", "_rels/.rels", "xl/workbook.xml", "xl/_rels/workbook.xml.rels", "xl/worksheets/sheet1.xml") -join "`n") -or
                [string]$case.cleanup_status -cne "owned-process-zero") {
                $errors.Add("case result $index bootstrap persistence or cleanup status is invalid")
            }
        }
        elseif ($null -ne $case.owned_excel_pid) {
            $errors.Add("case result $index exposes an owned Excel PID without a durable ownership record")
        }
        if ($null -ne $case.observed_excel_pid -and (($case.observed_excel_pid -isnot [long] -and $case.observed_excel_pid -isnot [int]) -or [int]$case.observed_excel_pid -le 0)) {
            $errors.Add("case result $index observed Excel PID is invalid")
        }
        if ($null -ne $case.transport_error -and ($case.transport_error -isnot [string] -or [string]::IsNullOrWhiteSpace([string]$case.transport_error))) { $errors.Add("case result $index transport type is invalid") }
        if ($null -ne $case.run_value -and $case.run_value -isnot [string]) { $errors.Add("case result $index run_value type is invalid") }
        if ($null -ne $case.macro_failure_disposition -and $case.macro_failure_disposition -isnot [string]) { $errors.Add("case result $index macro failure disposition type is invalid") }

        $sameRunProcedure = ($null -eq $descriptor.run_procedure -and $null -eq $case.run_procedure) -or
            ($null -ne $descriptor.run_procedure -and $null -ne $case.run_procedure -and [string]$case.run_procedure -ceq [string]$descriptor.run_procedure)
        if ([string]$case.id -cne [string]$descriptor.id -or
            [string]$case.purpose -cne [string]$descriptor.purpose -or
            [string]$case.selected_case_descriptor_sha256 -cne [string]$descriptor.descriptor_sha256 -or
            [string]$case.module_name -cne [string]$descriptor.module_name -or
            [string]$case.module_sha256 -cne [string]$descriptor.module_sha256 -or
            [string]$case.expected_compile_status -cne [string]$descriptor.expected_compile_status -or
            [string]$case.expected_run_status -cne [string]$descriptor.expected_run_status -or
            -not $sameRunProcedure -or
            $case.case_diagnostic_only -isnot [bool] -or [bool]$case.case_diagnostic_only -ne [bool]$descriptor.diagnostic_only -or
            [string]$case.evidence_contract -cne [string]$descriptor.evidence_contract) {
            $errors.Add("case result $index does not bind exactly to its immutable selected descriptor")
        }

        $compileCommandValid = $false
        if ($null -ne $case.compile_command) {
            $fields = @("schema", "id", "caption", "enabled_before", "enabled_after")
            $compileCommandValid = (@($case.compile_command.PSObject.Properties.Name | Sort-Object) -join "`n") -ceq (@($fields | Sort-Object) -join "`n") -and
                $case.compile_command.schema -is [string] -and [string]$case.compile_command.schema -ceq "oxvba.excel-vba-oracle-compile-command.v1" -and
                ($case.compile_command.id -is [int] -or $case.compile_command.id -is [long]) -and [int]$case.compile_command.id -eq 578 -and
                $case.compile_command.caption -is [string] -and -not [string]::IsNullOrWhiteSpace([string]$case.compile_command.caption) -and
                $case.compile_command.enabled_before -is [bool] -and [bool]$case.compile_command.enabled_before -and
                $case.compile_command.enabled_after -is [bool]
            if (-not $compileCommandValid) { $errors.Add("case result $index compile_command schema or types are invalid") }
        }

        $compileExecutionValid = $false
        if ($null -ne $case.compile_execution) {
            $fields = @("schema", "return_value", "exception")
            $exceptionValid = $null -eq $case.compile_execution.exception
            if ($null -ne $case.compile_execution.exception) {
                $exceptionFields = @("schema", "message", "hresult", "type")
                $exceptionValid = (@($case.compile_execution.exception.PSObject.Properties.Name | Sort-Object) -join "`n") -ceq (@($exceptionFields | Sort-Object) -join "`n") -and
                    [string]$case.compile_execution.exception.schema -ceq "oxvba.excel-vba-oracle-compile-exception.v1" -and
                    @(@("message", "hresult", "type") | Where-Object { $case.compile_execution.exception.$_ -isnot [string] -or [string]::IsNullOrWhiteSpace([string]$case.compile_execution.exception.$_) }).Count -eq 0
            }
            $compileExecutionValid = (@($case.compile_execution.PSObject.Properties.Name | Sort-Object) -join "`n") -ceq (@($fields | Sort-Object) -join "`n") -and
                [string]$case.compile_execution.schema -ceq "oxvba.excel-vba-oracle-compile-execution.v1" -and
                ($null -eq $case.compile_execution.return_value -or $case.compile_execution.return_value -is [string]) -and $exceptionValid
            if (-not $compileExecutionValid) { $errors.Add("case result $index compile_execution schema or types are invalid") }
        }

        $compileContextValid = $false
        if ($null -ne $case.compile_context) {
            $compileContextFields = @("schema", "injected_project_name", "injected_project_file_name", "injected_module_name", "selection_before_execute", "injected_source", "injected_source_sha256", "selected_source_sha256", "authority_before_execute", "authority_after_execute", "selection_after_execute_diagnostic_only")
            $authorityFields = @("schema", "stage", "captured_utc", "active_project_is_injected_project", "active_module_is_injected_module", "active_code_pane_is_injected_code_pane", "active_project_name", "active_module_name", "injected_source_sha256", "expected_source_sha256")
            $contextFieldsValid = (@($case.compile_context.PSObject.Properties.Name | Sort-Object) -join "`n") -ceq (@($compileContextFields | Sort-Object) -join "`n")
            $authorityValid = $contextFieldsValid
            $authorityEntries = if ($contextFieldsValid) { @(@("immediately-before-execute", $case.compile_context.authority_before_execute), @("immediately-after-execute", $case.compile_context.authority_after_execute)) } else { @() }
            foreach ($authorityEntry in $authorityEntries) {
                $expectedStage = [string]$authorityEntry[0]
                $authoritySnapshot = $authorityEntry[1]
                if ($null -eq $authoritySnapshot -or
                    (@($authoritySnapshot.PSObject.Properties.Name | Sort-Object) -join "`n") -cne (@($authorityFields | Sort-Object) -join "`n") -or
                    [string]$authoritySnapshot.schema -cne "oxvba.excel-vba-oracle-compile-authority-snapshot.v1" -or
                    [string]$authoritySnapshot.stage -cne $expectedStage -or
                    $authoritySnapshot.captured_utc -isnot [string] -or
                    @(@("active_project_is_injected_project", "active_module_is_injected_module", "active_code_pane_is_injected_code_pane") | Where-Object { $authoritySnapshot.$_ -isnot [bool] -or -not [bool]$authoritySnapshot.$_ }).Count -gt 0 -or
                    $authoritySnapshot.active_project_name -isnot [string] -or [string]::IsNullOrWhiteSpace([string]$authoritySnapshot.active_project_name) -or
                    $authoritySnapshot.active_module_name -isnot [string] -or [string]$authoritySnapshot.active_module_name -cne [string]$case.module_name -or
                    [string]$authoritySnapshot.injected_source_sha256 -cne [string]$case.module_sha256 -or
                    [string]$authoritySnapshot.expected_source_sha256 -cne [string]$case.module_sha256) {
                    $authorityValid = $false
                }
                else {
                    try { [void][DateTime]::Parse([string]$authoritySnapshot.captured_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind) }
                    catch { $authorityValid = $false }
                }
            }
            $compileContextValid = $contextFieldsValid -and
                [string]$case.compile_context.schema -ceq "oxvba.excel-vba-oracle-compile-context.v1" -and
                $case.compile_context.injected_project_name -is [string] -and -not [string]::IsNullOrWhiteSpace([string]$case.compile_context.injected_project_name) -and
                $case.compile_context.injected_project_file_name -is [string] -and
                [StringComparer]::OrdinalIgnoreCase.Equals([string]$case.compile_context.injected_project_file_name, $expectedBootstrapPath) -and
                $case.compile_context.injected_module_name -is [string] -and [string]$case.compile_context.injected_module_name -ceq [string]$case.module_name -and
                $case.compile_context.injected_source -is [string] -and [string]$case.compile_context.injected_source -ceq [string]$descriptor.module_source -and
                [string]$case.compile_context.injected_source_sha256 -ceq [string]$case.module_sha256 -and
                [string]$case.compile_context.selected_source_sha256 -ceq [string]$case.module_sha256 -and $authorityValid
            if (-not $compileContextValid) { $errors.Add("case result $index compile_context schema or source identity is invalid") }
        }

        $runtimeMeasurementValid = $false
        if ($null -ne $case.runtime_measurement) {
            $requiredRuntimeFields = @("schema", "measured_utc", "access_vbom", "invocation_entry", "invocation_entry_exists", "macro_probe_target", "macro_probe_target_exists", "automation_security", "macros_configured_for_automation", "invocation_entry_observed", "invocation_observation", "macros_runnable_entry")
            $runtimeMeasurementValid = (@($case.runtime_measurement.PSObject.Properties.Name | Sort-Object) -join "`n") -ceq (@($requiredRuntimeFields | Sort-Object) -join "`n") -and
                [string]$case.runtime_measurement.schema -ceq "oxvba.excel-vba-oracle-runtime-measurement.v1" -and
                @(@("access_vbom", "invocation_entry_exists", "macro_probe_target_exists", "macros_configured_for_automation", "invocation_entry_observed", "macros_runnable_entry") | Where-Object { $case.runtime_measurement.$_ -isnot [bool] }).Count -eq 0 -and
                ($null -eq $case.runtime_measurement.invocation_entry -or $case.runtime_measurement.invocation_entry -is [string]) -and
                ($null -eq $case.runtime_measurement.macro_probe_target -or $case.runtime_measurement.macro_probe_target -is [string]) -and
                ($null -eq $case.runtime_measurement.invocation_observation -or $case.runtime_measurement.invocation_observation -is [string]) -and
                ($case.runtime_measurement.automation_security -is [int] -or $case.runtime_measurement.automation_security -is [long]) -and
                $case.runtime_measurement.measured_utc -is [string]
            if ($runtimeMeasurementValid) {
                try { [void][DateTime]::Parse([string]$case.runtime_measurement.measured_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind) }
                catch { $runtimeMeasurementValid = $false }
            }
            if (-not $runtimeMeasurementValid) { $errors.Add("case result $index runtime_measurement schema or types are invalid") }
        }

        $runtimeErrValid = $null -eq $case.runtime_err
        if ($null -ne $case.runtime_err) {
            $runtimeErrFields = @("schema", "number", "source", "description", "help_file", "help_context", "erl")
            $runtimeErrValid = (@($case.runtime_err.PSObject.Properties.Name | Sort-Object) -join "`n") -ceq (@($runtimeErrFields | Sort-Object) -join "`n") -and
                [string]$case.runtime_err.schema -ceq "oxvba.excel-vba-oracle-runtime-err.v1" -and
                @(@("number", "help_context", "erl") | Where-Object { $case.runtime_err.$_ -isnot [int] -and $case.runtime_err.$_ -isnot [long] }).Count -eq 0 -and
                @(@("source", "description", "help_file") | Where-Object { $case.runtime_err.$_ -isnot [string] }).Count -eq 0
            if (-not $runtimeErrValid) { $errors.Add("case result $index runtime_err schema or types are invalid") }
        }

        $workerCompileLedger = ConvertTo-ExcelOracleValidatedGuardianEvents -Events @($case.compile_window_observations) -RunId $RunId -CaseId ([string]$case.id)
        $workerCompileDialogs = ConvertTo-ExcelOracleValidatedGuardianEvents -Events @($case.compile_dialogs) -RunId $RunId -CaseId ([string]$case.id)
        $workerRunLedger = ConvertTo-ExcelOracleValidatedGuardianEvents -Events @($case.run_dialogs) -RunId $RunId -CaseId ([string]$case.id)
        if (@($workerCompileLedger.errors).Count -gt 0) { $errors.Add("case result $index embedded compile guardian evidence is invalid: $(@($workerCompileLedger.errors) -join '; ')") }
        if (@($workerCompileDialogs.errors).Count -gt 0) { $errors.Add("case result $index embedded compile dialog evidence is invalid: $(@($workerCompileDialogs.errors) -join '; ')") }
        if (@($workerRunLedger.errors).Count -gt 0) { $errors.Add("case result $index embedded run guardian evidence is invalid: $(@($workerRunLedger.errors) -join '; ')") }

        $supervisorGuardianRecords = @()
        if ($supervisorGuardianByCase.ContainsKey([string]$case.id)) {
            $supervisorGuardianRecords = @($supervisorGuardianByCase[[string]$case.id].records)
        }
        elseif ([bool]$case.excel_ownership_recorded) {
            $errors.Add("case result $index lacks its supervisor-retained guardian event ledger")
        }
        $compileEvents = @($supervisorGuardianRecords | Where-Object {
            $_.PSObject.Properties.Name -contains "operation_id" -and [string]$_.operation_id -ceq "$([string]$case.id)-compile"
        })
        $runEvents = @($supervisorGuardianRecords | Where-Object {
            $_.PSObject.Properties.Name -contains "operation_id" -and [string]$_.operation_id -ceq "$([string]$case.id)-run"
        })
        $supervisorCompileDialogs = @($compileEvents | Where-Object { [string]$_.event_type -ceq "dialog-observation" })
        if ((Get-ExcelOracleObjectSequenceDigest -Values @($workerCompileLedger.records)) -cne (Get-ExcelOracleObjectSequenceDigest -Values $compileEvents) -or
            (Get-ExcelOracleObjectSequenceDigest -Values @($workerCompileDialogs.records)) -cne (Get-ExcelOracleObjectSequenceDigest -Values $supervisorCompileDialogs) -or
            (Get-ExcelOracleObjectSequenceDigest -Values @($workerRunLedger.records)) -cne (Get-ExcelOracleObjectSequenceDigest -Values $runEvents)) {
            $errors.Add("case result $index embedded guardian evidence does not exactly match the supervisor-retained ledger records")
        }
        $guardianFinalEvents = @($supervisorGuardianRecords | Where-Object { [string]$_.event_type -ceq "guardian-stopped" })
        $caseHelperRecords = @($helperRecords | Where-Object { [string]$_.case_id -ceq [string]$case.id })
        $guardianFinalHealthy = $guardianFinalEvents.Count -eq 1 -and $caseHelperRecords.Count -eq 1 -and
            [bool]$guardianFinalEvents[0].controlled_stop_observed -and [bool]$guardianFinalEvents[0].excel_identity_live_at_stop -and
            [string]$guardianFinalEvents[0].exit_reason -ceq "controlled-stop" -and
            [int]$guardianFinalEvents[0].guardian_pid -eq [int]$caseHelperRecords[0].pid -and
            [string]$guardianFinalEvents[0].process_name -ceq [string]$caseHelperRecords[0].process_name -and
            [string]$guardianFinalEvents[0].process_start_utc -ceq [string]$caseHelperRecords[0].process_start_utc -and
            [StringComparer]::OrdinalIgnoreCase.Equals([string]$guardianFinalEvents[0].executable_path, [string]$caseHelperRecords[0].executable_path)
        if ([bool]$case.excel_ownership_recorded -and -not $guardianFinalHealthy) {
            $errors.Add("case result $index lacks an exact healthy guardian final state bound to its supervisor-retained helper identity")
        }
        foreach ($timestamp in @(
                @(Get-ExcelOracleEvidenceUtcValues -Value $case -Path "case[$index]") +
                @(Get-ExcelOracleEvidenceUtcValues -Value $supervisorGuardianRecords -Path "supervisor_guardian[$index]")
            )) {
            try {
                if ($timestamp.value -isnot [string]) { throw "not a JSON string" }
                $evidenceUtc = [DateTime]::Parse([string]$timestamp.value, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind).ToUniversalTime()
                if ($null -eq $generatedUtc -or $evidenceUtc -ge $generatedUtc) { throw "not earlier than results generation" }
            }
            catch { $errors.Add("case result $index evidence timestamp '$([string]$timestamp.path)' is invalid or later than generated_utc") }
        }
        $compileHealthy = Test-GuardianOperationHealthy -Events $compileEvents
        $runHealthy = Test-GuardianOperationHealthy -Events $runEvents
        $compileErrorComplete = if ($null -ne $descriptor -and $null -ne $descriptor.expected_selected_token -and $null -ne $descriptor.expected_expanded_line) {
            Test-CompileErrorEvidence -Events $compileEvents -InjectedSource ([string]$descriptor.module_source) -ExpectedToken ([string]$descriptor.expected_selected_token) -ExpectedLine ([string]$descriptor.expected_expanded_line)
        } else { $false }
        $compileErrorDialogs = @($compileEvents | Where-Object { [string]$_.event_type -eq "dialog-observation" })
        $compileErrorObserved = $compileErrorDialogs.Count -gt 0 -and
            @($compileErrorDialogs | Where-Object { [string]$_.classification -cne "compile-error" -or -not (Test-LinkedSuccessfulDismissal -Observation $_ -Events $compileEvents) }).Count -eq 0
        $ambiguousComplete = Test-AmbiguousMacroEvidence -Events $runEvents
        $runtimeErrorComplete = Test-RuntimeErrorEvidence -Events $runEvents
        $authoritativeEvidencePassed = if ($null -eq $descriptor) { $false } else {
            switch ([string]$descriptor.evidence_contract) {
                "compile-error-token-line-dismissal-v1" { $compileHealthy -and $compileErrorComplete; break }
                "clean-compile-ambiguous-macro-dismissal-v1" { $compileHealthy -and (Test-NoDialogObservations -Events $compileEvents) -and $runHealthy -and $ambiguousComplete; break }
                "clean-compile-runtime-error-dismissal-v1" { $compileHealthy -and (Test-NoDialogObservations -Events $compileEvents) -and $runHealthy -and $runtimeErrorComplete; break }
                default { $compileHealthy -and (Test-NoDialogObservations -Events $compileEvents) -and $runHealthy -and (Test-NoDialogObservations -Events $runEvents); break }
            }
        }

        # Compiler outcome authority comes from the exact command, Execute
        # result, active context, and guardian ledger. An Execute exception is
        # always a harness failure; a coincident modal cannot downgrade it into
        # a language compile error without future independent evidence.
        $derivedCompileStatus = if (-not $compileCommandValid -or -not $compileExecutionValid -or -not $compileContextValid) { "harness-error" }
            elseif ($null -ne $case.compile_execution.exception) { "harness-error" }
            elseif ($compileErrorObserved) { "compile-error" }
            elseif (-not [bool]$case.compile_command.enabled_after -and (Test-NoDialogObservations -Events $compileEvents)) { "ok" }
            else { "no-dialog-unverified" }
        $shouldInvoke = $derivedCompileStatus -ceq "ok" -and $null -ne $descriptor.run_procedure

        $evidenceStatusValid = $false
        $guardianHealthyBeforeCleanup = $false
        if ($null -ne $case.evidence_status) {
            $evidenceFields = @("schema", "guardian_healthy_before_cleanup", "compile_operation_healthy", "run_operation_healthy", "compile_error_modal_complete", "ambiguous_macro_modal_and_dismissal_complete", "runtime_error_modal_and_dismissal_complete", "authoritative_evidence_passed")
            $evidenceStatusValid = (@($case.evidence_status.PSObject.Properties.Name | Sort-Object) -join "`n") -ceq (@($evidenceFields | Sort-Object) -join "`n") -and
                [string]$case.evidence_status.schema -ceq "oxvba.excel-vba-oracle-evidence-status.v1" -and
                @($evidenceFields | Where-Object { $_ -ne "schema" -and $case.evidence_status.$_ -isnot [bool] }).Count -eq 0
            if ($evidenceStatusValid) { $guardianHealthyBeforeCleanup = $guardianFinalHealthy }
            if ($evidenceStatusValid -and
                ([bool]$case.evidence_status.guardian_healthy_before_cleanup -ne $guardianFinalHealthy -or
                 [bool]$case.evidence_status.compile_operation_healthy -ne $compileHealthy -or
                 [bool]$case.evidence_status.run_operation_healthy -ne $runHealthy -or
                 [bool]$case.evidence_status.compile_error_modal_complete -ne $compileErrorComplete -or
                 [bool]$case.evidence_status.ambiguous_macro_modal_and_dismissal_complete -ne $ambiguousComplete -or
                 [bool]$case.evidence_status.runtime_error_modal_and_dismissal_complete -ne $runtimeErrorComplete -or
                 [bool]$case.evidence_status.authoritative_evidence_passed -ne $authoritativeEvidencePassed)) {
                $evidenceStatusValid = $false
            }
            if (-not $evidenceStatusValid) { $errors.Add("case result $index evidence_status does not match independently derived evidence") }
        }

        $runtimeMeasurementMatchesDescriptor = $runtimeMeasurementValid
        $entryObserved = $false
        if ($runtimeMeasurementMatchesDescriptor) {
            $expectedInvocationEntry = $descriptor.run_procedure
            $expectedProbeTarget = $descriptor.macro_probe_target
            $invocationEntryExists = $null -ne $expectedInvocationEntry
            $probeTargetExists = $null -ne $expectedProbeTarget -and $null -ne $expectedInvocationEntry -and
                [string]$expectedProbeTarget -ceq [string]$expectedInvocationEntry
            $entryObserved = [bool]$case.runtime_measurement.invocation_entry_observed
            $runtimeMeasurementMatchesDescriptor = [bool]$case.runtime_measurement.access_vbom -and
                (($null -eq $expectedInvocationEntry -and $null -eq $case.runtime_measurement.invocation_entry) -or
                 ($null -ne $expectedInvocationEntry -and [string]$case.runtime_measurement.invocation_entry -ceq [string]$expectedInvocationEntry)) -and
                [bool]$case.runtime_measurement.invocation_entry_exists -eq $invocationEntryExists -and
                (($null -eq $expectedProbeTarget -and $null -eq $case.runtime_measurement.macro_probe_target) -or
                 ($null -ne $expectedProbeTarget -and [string]$case.runtime_measurement.macro_probe_target -ceq [string]$expectedProbeTarget)) -and
                [bool]$case.runtime_measurement.macro_probe_target_exists -eq $probeTargetExists -and
                [int]$case.runtime_measurement.automation_security -eq 1 -and [bool]$case.runtime_measurement.macros_configured_for_automation -and
                [bool]$case.runtime_measurement.macros_runnable_entry -eq $entryObserved -and
                (($entryObserved -and $case.runtime_measurement.invocation_observation -is [string] -and
                    -not [string]::IsNullOrWhiteSpace([string]$case.runtime_measurement.invocation_observation)) -or
                 (-not $entryObserved -and $null -eq $case.runtime_measurement.invocation_observation))
            if (-not $runtimeMeasurementMatchesDescriptor) { $errors.Add("case result $index runtime measurement does not match the selected descriptor and observed invocation contract") }
        }

        $runValueErrMatches = $false
        if ($case.run_value -is [string] -and $null -ne $case.runtime_err -and $runtimeErrValid) {
            try {
                $parsedRunValueErr = ConvertFrom-ExcelOracleRuntimeErr -Json ([string]$case.run_value)
                $runValueErrMatches = @(@("number", "source", "description", "help_file", "help_context", "erl") | Where-Object {
                    -not (Test-ExcelOracleRuntimeErrFieldEqual -Left $parsedRunValueErr.$_ -Right $case.runtime_err.$_)
                }).Count -eq 0
            }
            catch { $runValueErrMatches = $false }
        }
        $derivedMacroDisposition = $null
        $ambiguousValueShape = $runtimeMeasurementMatchesDescriptor -and $case.run_value -is [string] -and
            -not [string]::IsNullOrWhiteSpace([string]$descriptor.invocation_observation_prefix) -and
            ([string]$case.run_value).StartsWith([string]$descriptor.invocation_observation_prefix, [StringComparison]::Ordinal)
        if ($ambiguousValueShape) {
            $macroMessage = ([string]$case.run_value).Substring(([string]$descriptor.invocation_observation_prefix).Length)
            if (-not [string]::IsNullOrWhiteSpace($macroMessage)) {
                $derivedMacroDisposition = Get-ExcelOracleMacroFailureDisposition -Message $macroMessage -CompileStatus $derivedCompileStatus `
                    -AccessVbom ([bool]$case.runtime_measurement.access_vbom) -RunnableEntryObserved $entryObserved `
                    -TargetExists ([bool]$case.runtime_measurement.macro_probe_target_exists)
            }
        }
        $noRunPayload = $runEvents.Count -eq 0 -and $null -eq $case.run_value -and $null -eq $case.runtime_err -and $null -eq $case.macro_failure_disposition
        $returnSuccessShape = $shouldInvoke -and $runHealthy -and $runtimeMeasurementMatchesDescriptor -and $entryObserved -and
            [string]$case.runtime_measurement.invocation_observation -ceq "qualified-entry-returned" -and
            (Test-NoDialogObservations -Events $runEvents) -and $case.run_value -is [string] -and $null -eq $case.runtime_err
        $fullErrShape = $shouldInvoke -and $runHealthy -and $runtimeMeasurementMatchesDescriptor -and $entryObserved -and
            [string]$case.runtime_measurement.invocation_observation -ceq "qualified-entry-returned" -and
            (Test-NoDialogObservations -Events $runEvents) -and $runValueErrMatches
        $ambiguousShape = $shouldInvoke -and $runHealthy -and $runtimeMeasurementMatchesDescriptor -and $entryObserved -and
            [string]$case.runtime_measurement.invocation_observation -ceq "case-specific-return-sentinel" -and
            $ambiguousComplete -and -not $runtimeErrorComplete -and $null -eq $case.runtime_err -and $ambiguousValueShape -and
            [string]$derivedMacroDisposition -ceq "missing-macro"
        $runtimeModalShape = $shouldInvoke -and $runHealthy -and $runtimeMeasurementMatchesDescriptor -and $entryObserved -and
            [string]$case.runtime_measurement.invocation_observation -ceq "owned-runtime-error-modal" -and
            $runtimeErrorComplete -and -not $ambiguousComplete -and $null -eq $case.run_value -and $null -eq $case.runtime_err
        if (-not $shouldInvoke -and (-not $noRunPayload -or $entryObserved)) {
            $errors.Add("case result $index compile-not-run shape contains runtime operation, payload, Err, disposition, or observation evidence")
        }
        $runtimeShapeCount = @(@($returnSuccessShape, $fullErrShape, $ambiguousShape, $runtimeModalShape) | Where-Object { $_ }).Count
        if ($shouldInvoke -and $runtimeShapeCount -ne 1) {
            $errors.Add("case result $index runtime evidence does not form exactly one admitted mutually exclusive outcome")
        }
        $derivedRunStatus = if (-not $shouldInvoke) { "not-run" }
            elseif ($runtimeModalShape) { "runtime-error-modal" }
            elseif ($ambiguousShape) { "missing-macro" }
            elseif ($fullErrShape) { "runtime-err-captured" }
            elseif ($returnSuccessShape) { "ok" }
            else { "runtime-evidence-invalid" }
        $expectedMacroDisposition = if ($ambiguousShape) { $derivedMacroDisposition }
            elseif ($runtimeModalShape) { "non-macro-runtime-failure" }
            else { $null }
        if ([string]$case.compile_status -cne $derivedCompileStatus) { $errors.Add("case result $index compile status contradicts command/execution/modal evidence") }
        if ([string]$case.run_status -cne $derivedRunStatus) { $errors.Add("case result $index run status contradicts invocation/runtime/modal evidence") }
        if (($null -eq $expectedMacroDisposition -and $null -ne $case.macro_failure_disposition) -or
            ($null -ne $expectedMacroDisposition -and [string]$case.macro_failure_disposition -cne [string]$expectedMacroDisposition)) {
            $errors.Add("case result $index macro failure disposition contradicts derived runtime evidence")
        }

        $behaviorPassed = $null -ne $descriptor -and $derivedCompileStatus -ceq [string]$descriptor.expected_compile_status -and
            $derivedRunStatus -ceq [string]$descriptor.expected_run_status
        if ($behaviorPassed -and $null -ne $descriptor.expected_value) { $behaviorPassed = [string]$case.run_value -ceq [string]$descriptor.expected_value }
        if ($behaviorPassed -and $null -ne $descriptor.expected_runtime_err) {
            $behaviorPassed = $runtimeErrValid -and $null -ne $case.runtime_err
            foreach ($field in @("number", "source", "description", "help_file", "help_context", "erl")) {
                if ($behaviorPassed -and -not (Test-ExcelOracleRuntimeErrFieldEqual -Left $case.runtime_err.$field -Right $descriptor.expected_runtime_err.$field)) { $behaviorPassed = $false }
            }
        }
        $cleanupPassed = $case.excel_ownership_recorded -is [bool] -and [bool]$case.excel_ownership_recorded -and
            [string]$case.cleanup_status -ceq "owned-process-zero" -and @($case.cleanup_authority_errors).Count -eq 0
        $casePassBeforeTransport = [bool]($behaviorPassed -and $runtimeMeasurementMatchesDescriptor -and $runtimeErrValid -and
            $evidenceStatusValid -and $guardianHealthyBeforeCleanup -and $authoritativeEvidencePassed -and $cleanupPassed)
        $transportShapeValid = if ($casePassBeforeTransport) { $null -eq $case.transport_error }
            else { $case.transport_error -is [string] -and -not [string]::IsNullOrWhiteSpace([string]$case.transport_error) }
        if (-not $transportShapeValid) { $errors.Add("case result $index transport_error does not match the derived success/failure shape") }
        $derivedCasePasses.Add([bool]($casePassBeforeTransport -and $transportShapeValid))
    }
    if ($caseFieldSetInvalid) {
        return [pscustomobject]@{ valid = $false; disposition = $disposition; transport_error = $transport; errors = @($errors) }
    }
    $caseIds = @($cases | ForEach-Object { [string]$_.id })
    $preOwnershipEvidenceEmpty = $cases.Count -eq 1 -and
        $null -eq $cases[0].compile_command -and $null -eq $cases[0].compile_execution -and $null -eq $cases[0].compile_context -and
        $null -eq $cases[0].post_dismiss_selection_diagnostic_only -and @($cases[0].compile_dialogs).Count -eq 0 -and
        @($cases[0].compile_window_observations).Count -eq 0 -and $null -eq $cases[0].run_value -and $null -eq $cases[0].runtime_err -and
        $null -eq $cases[0].macro_failure_disposition -and $null -eq $cases[0].runtime_measurement -and @($cases[0].run_dialogs).Count -eq 0 -and
        $null -eq $cases[0].evidence_status -and $null -eq $cases[0].bootstrap_workbook -and @($cases[0].cleanup_authority_errors).Count -eq 0 -and
        $supervisorGuardianByCase.Count -eq 0
    $specialTransport = $cases.Count -eq 1 -and $expectedCaseIds.Count -gt 0 -and
        [string]$cases[0].id -ceq [string]$expectedCaseIds[0] -and
        $cases[0].passed -is [bool] -and -not [bool]$cases[0].passed -and
        $cases[0].excel_ownership_recorded -is [bool] -and -not [bool]$cases[0].excel_ownership_recorded -and
        $null -eq $cases[0].owned_excel_pid -and
        [string]$cases[0].compile_status -ceq "harness-error" -and [string]$cases[0].run_status -ceq "not-run" -and
        [string]$cases[0].cleanup_status -in @("not-run", "owned-process-zero", "job-contained-preownership") -and
        -not [string]::IsNullOrWhiteSpace([string]$cases[0].transport_error) -and
        $preOwnershipEvidenceEmpty -and $excelRecords.Count -eq 0 -and $helperRecords.Count -eq 0

    if ($specialTransport) {
        if ($WorkerExitCode -ne 1 -or $Results.passed -isnot [bool] -or [bool]$Results.passed) {
            $errors.Add("pre-ownership transport has an invalid exit envelope")
        }
        if ($errors.Count -eq 0) {
            $disposition = "pre-ownership-transport"
            $transport = [string]$cases[0].transport_error
        }
    }
    else {
        if (($caseIds -join "`n") -cne ($expectedCaseIds -join "`n")) { $errors.Add("case result order does not match the selected descriptor sequence") }
        if ((@($excelRecords | ForEach-Object { [string]$_.case_id }) -join "`n") -cne ($expectedCaseIds -join "`n") -or
            (@($helperRecords | ForEach-Object { [string]$_.case_id }) -join "`n") -cne ($expectedCaseIds -join "`n")) {
            $errors.Add("ownership ledger order does not match the selected case sequence")
        }
        for ($index = 0; $index -lt [Math]::Min($cases.Count, $excelRecords.Count); $index++) {
            if ($cases[$index].excel_ownership_recorded -isnot [bool] -or -not [bool]$cases[$index].excel_ownership_recorded -or
                ($cases[$index].owned_excel_pid -isnot [long] -and $cases[$index].owned_excel_pid -isnot [int]) -or
                ($cases[$index].observed_excel_pid -isnot [long] -and $cases[$index].observed_excel_pid -isnot [int]) -or
                [int]$cases[$index].owned_excel_pid -ne [int]$excelRecords[$index].pid -or
                [int]$cases[$index].observed_excel_pid -ne [int]$excelRecords[$index].pid) {
                $errors.Add("case result $index does not bind to its durable Excel ownership record")
            }
        }
        $derivedAggregate = $derivedCasePasses.Count -eq $cases.Count -and @($derivedCasePasses | Where-Object { -not $_ }).Count -eq 0
        for ($index = 0; $index -lt [Math]::Min($cases.Count, $derivedCasePasses.Count); $index++) {
            if ($cases[$index].passed -isnot [bool] -or [bool]$cases[$index].passed -ne [bool]$derivedCasePasses[$index]) {
                $errors.Add("case result $index passed value disagrees with derived behavior/evidence")
            }
        }
        if ($Results.passed -isnot [bool] -or [bool]$Results.passed -ne $derivedAggregate) { $errors.Add("aggregate passed status disagrees with derived case results") }
        $expectedExitCode = if ($derivedAggregate) { 0 } else { 1 }
        if ($WorkerExitCode -ne $expectedExitCode) { $errors.Add("worker exit code disagrees with aggregate result") }
        if ($errors.Count -eq 0) {
            if ($derivedAggregate) { $disposition = "complete-success" }
            else {
                $disposition = "complete-case-failure"
                $transport = @($cases | Where-Object { -not [bool]$_.passed } | ForEach-Object { [string]$_.transport_error }) -join "; "
            }
        }
    }
    return [pscustomobject]@{ valid = $errors.Count -eq 0; disposition = $disposition; transport_error = $transport; errors = @($errors) }
}

function ConvertFrom-ExcelOracleGuardianEventLedger {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Lines,
        [Parameter(Mandatory = $true)][string]$RunId,
        [AllowEmptyCollection()][string[]]$ExpectedCaseIds = @()
    )

    $records = [Collections.Generic.List[object]]::new()
    $errors = [Collections.Generic.List[string]]::new()
    $observations = @{}
    $dismissals = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $lastEventSequence = 0L
    for ($index = 0; $index -lt $Lines.Count; $index++) {
        $line = [string]$Lines[$index]
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        try { $event = $line | ConvertFrom-Json -DateKind String }
        catch {
            $errors.Add("line $($index + 1): malformed guardian JSON")
            continue
        }
        if ($null -eq $event) {
            $errors.Add("line $($index + 1): null guardian event")
            continue
        }
        if ($event.PSObject.Properties.Name -notcontains "schema" -or
            $event.PSObject.Properties.Name -notcontains "event_type" -or
            $event.PSObject.Properties.Name -notcontains "run_id") {
            $errors.Add("line $($index + 1): missing guardian schema/event_type/run_id")
            continue
        }
        if ([string]$event.run_id -ne $RunId) {
            $errors.Add("line $($index + 1): foreign guardian run_id")
            continue
        }
        foreach ($timestampField in @("observed_utc", "capture_completed_utc", "attempted_utc")) {
            if ($event.PSObject.Properties.Name -notcontains $timestampField) { continue }
            try { [void][DateTime]::Parse([string]$event.$timestampField, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind) }
            catch { $errors.Add("line $($index + 1): invalid guardian $timestampField") }
        }
        if ($event.PSObject.Properties.Name -contains "event_sequence") {
            if (($event.event_sequence -isnot [long] -and $event.event_sequence -isnot [int]) -or [long]$event.event_sequence -le $lastEventSequence) {
                $errors.Add("line $($index + 1): guardian event_sequence is not a strictly increasing integer")
            }
            else { $lastEventSequence = [long]$event.event_sequence }
        }
        $lifecycleRecognized = $false
        if ([string]$event.event_type -eq "invalid-control" -and [string]$event.schema -eq "oxvba.excel-vba-oracle-control-status.v1") {
            $lifecycleRecognized = $true
            $required = @("event_sequence", "observed_utc", "control_sha256", "errors", "valid")
            $missing = @($required | Where-Object { $event.PSObject.Properties.Name -notcontains $_ })
            if ($missing.Count -gt 0 -or $event.valid -isnot [bool] -or [bool]$event.valid -or @($event.errors).Count -eq 0) {
                $errors.Add("line $($index + 1): invalid control report is incomplete or not fail-closed")
                continue
            }
        }
        elseif ([string]$event.event_type -in @("operation-armed", "guardian-heartbeat") -and [string]$event.schema -eq "oxvba.excel-vba-oracle-operation-state.v1") {
            $lifecycleRecognized = $true
            $required = @("case_id", "operation_id", "phase", "control_sequence", "event_sequence", "observed_utc")
            $missing = @($required | Where-Object { $event.PSObject.Properties.Name -notcontains $_ -or [string]::IsNullOrWhiteSpace([string]$event.$_) })
            if ($missing.Count -gt 0 -or [string]$event.phase -notin @("compile", "run", "cleanup")) {
                $errors.Add("line $($index + 1): invalid guardian operation state")
                continue
            }
            if ($ExpectedCaseIds.Count -gt 0 -and [string]$event.case_id -notin $ExpectedCaseIds) {
                $errors.Add("line $($index + 1): operation state has an unselected case_id")
                continue
            }
            if (($event.control_sequence -isnot [long] -and $event.control_sequence -isnot [int]) -or [long]$event.control_sequence -le 0) {
                $errors.Add("line $($index + 1): operation control_sequence is not a positive JSON integer")
                continue
            }
        }
        elseif ([string]$event.event_type -ceq "guardian-stopped" -and [string]$event.schema -ceq "oxvba.excel-vba-oracle-final-state.v1") {
            $lifecycleRecognized = $true
            $required = @("schema", "event_type", "run_id", "case_id", "event_sequence", "observed_utc", "guardian_pid", "process_name", "process_start_utc", "executable_path", "controlled_stop_observed", "excel_identity_live_at_stop", "exit_reason")
            if ((@($event.PSObject.Properties.Name | Sort-Object) -join "`n") -cne (@($required | Sort-Object) -join "`n") -or
                $event.case_id -isnot [string] -or [string]::IsNullOrWhiteSpace([string]$event.case_id) -or
                ($ExpectedCaseIds.Count -gt 0 -and @($ExpectedCaseIds | Where-Object { [string]$_ -ceq [string]$event.case_id }).Count -ne 1) -or
                ($event.guardian_pid -isnot [int] -and $event.guardian_pid -isnot [long]) -or [int]$event.guardian_pid -le 0 -or
                $event.process_name -isnot [string] -or [string]::IsNullOrWhiteSpace([string]$event.process_name) -or
                $event.executable_path -isnot [string] -or [string]::IsNullOrWhiteSpace([string]$event.executable_path) -or
                $event.controlled_stop_observed -isnot [bool] -or $event.excel_identity_live_at_stop -isnot [bool] -or
                $event.exit_reason -isnot [string] -or [string]$event.exit_reason -notin @("controlled-stop", "excel-identity-lost", "deadline")) {
                $errors.Add("line $($index + 1): invalid guardian final state")
                continue
            }
            try { [void][DateTime]::Parse([string]$event.process_start_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind) }
            catch { $errors.Add("line $($index + 1): invalid guardian final process start"); continue }
        }
        if ([string]$event.schema -eq "oxvba.excel-vba-oracle-window-observation.v1" -and [string]$event.event_type -in @("dialog-observation", "ignored-top-level-window")) {
            $required = @("observation_id", "case_id", "operation_id", "control_sequence", "event_sequence", "phase", "excel_pid", "observed_process_id", "observed_utc", "capture_completed_utc", "window_handle", "classification", "disposition", "considered_dialog", "is_modal")
            $missing = @($required | Where-Object { $event.PSObject.Properties.Name -notcontains $_ -or [string]::IsNullOrWhiteSpace([string]$event.$_) })
            if ($missing.Count -gt 0) {
                $errors.Add("line $($index + 1): incomplete guardian observation ($($missing -join ','))")
                continue
            }
            if ($event.considered_dialog -isnot [bool] -or $event.is_modal -isnot [bool]) {
                $errors.Add("line $($index + 1): guardian observation Boolean fields are not JSON booleans")
                continue
            }
            if ([string]$event.phase -notin @("compile", "run", "cleanup")) {
                $errors.Add("line $($index + 1): invalid guardian observation phase")
                continue
            }
            if ($ExpectedCaseIds.Count -gt 0 -and [string]$event.case_id -notin $ExpectedCaseIds) {
                $errors.Add("line $($index + 1): guardian observation has an unselected case_id")
                continue
            }
            try {
                if (($event.excel_pid -isnot [long] -and $event.excel_pid -isnot [int]) -or
                    ($event.observed_process_id -isnot [long] -and $event.observed_process_id -isnot [int]) -or
                    ($event.control_sequence -isnot [long] -and $event.control_sequence -isnot [int]) -or [long]$event.control_sequence -le 0) {
                    throw "non-integer numeric field"
                }
                $excelPid = [int]$event.excel_pid; $observedPid = [int]$event.observed_process_id
            }
            catch { $errors.Add("line $($index + 1): guardian observation PID is not an integer"); continue }
            if ($excelPid -le 0 -or $observedPid -ne $excelPid) {
                $errors.Add("line $($index + 1): guardian observation escaped its Excel PID boundary")
                continue
            }
            $classification = [string]$event.classification
            $expectedDisposition = switch ($classification) {
                { $_ -in @("compile-error", "runtime-error", "ambiguous-macro-failure") } { "capture-then-dismiss"; break }
                { $_ -in @("security-or-trust", "unrecognized-modal") } { "block-no-dismiss"; break }
                default { $null; break }
            }
            if ($null -eq $expectedDisposition -or [string]$event.disposition -ne $expectedDisposition) {
                $errors.Add("line $($index + 1): guardian classification/disposition mismatch")
                continue
            }
            $phaseClassifications = switch ([string]$event.phase) {
                "compile" { @("compile-error", "security-or-trust", "unrecognized-modal") }
                "run" { @("runtime-error", "ambiguous-macro-failure", "security-or-trust", "unrecognized-modal") }
                default { @("security-or-trust", "unrecognized-modal") }
            }
            if ($classification -notin $phaseClassifications) {
                $errors.Add("line $($index + 1): guardian classification is not allowed in this phase")
                continue
            }
            if ($classification -eq "compile-error" -and ([string]::IsNullOrWhiteSpace([string]$event.selected_token) -or [string]::IsNullOrWhiteSpace([string]$event.expanded_line))) {
                $errors.Add("line $($index + 1): compile observation lacks immutable pre-dismiss token/line capture")
                continue
            }
            try {
                if ([DateTime]::Parse([string]$event.capture_completed_utc).ToUniversalTime() -lt [DateTime]::Parse([string]$event.observed_utc).ToUniversalTime()) {
                    $errors.Add("line $($index + 1): guardian capture completed before observation")
                    continue
                }
            }
            catch { $errors.Add("line $($index + 1): guardian observation timestamps are invalid"); continue }
            if ([string]$event.event_type -eq "dialog-observation" -and -not [bool]$event.considered_dialog) {
                $errors.Add("line $($index + 1): dialog observation is not marked considered")
                continue
            }
            if ([string]$event.event_type -eq "ignored-top-level-window" -and
                ([bool]$event.considered_dialog -or $classification -ne "unrecognized-modal")) {
                $errors.Add("line $($index + 1): ignored window violates considered/classification constraints")
                continue
            }
            $observationId = [string]$event.observation_id
            if ($observations.ContainsKey($observationId)) {
                $errors.Add("line $($index + 1): duplicate guardian observation_id")
                continue
            }
            $observations[$observationId] = $event
            $records.Add($event)
            continue
        }
        if ([string]$event.schema -eq "oxvba.excel-vba-oracle-dismissal-result.v1" -and [string]$event.event_type -eq "dismissal-result") {
            $required = @("observation_id", "case_id", "operation_id", "control_sequence", "event_sequence", "phase", "excel_pid", "window_handle", "attempted_utc", "requested_buttons", "succeeded")
            $missing = @($required | Where-Object { $event.PSObject.Properties.Name -notcontains $_ -or [string]::IsNullOrWhiteSpace([string]$event.$_) })
            if ($missing.Count -gt 0) {
                $errors.Add("line $($index + 1): incomplete guardian dismissal ($($missing -join ','))")
                continue
            }
            if ($event.succeeded -isnot [bool]) {
                $errors.Add("line $($index + 1): dismissal succeeded is not a JSON boolean")
                continue
            }
            if (($event.excel_pid -isnot [long] -and $event.excel_pid -isnot [int]) -or
                ($event.control_sequence -isnot [long] -and $event.control_sequence -isnot [int]) -or [long]$event.control_sequence -le 0) {
                $errors.Add("line $($index + 1): dismissal numeric identity fields are not JSON integers")
                continue
            }
            if (@($event.requested_buttons).Count -eq 0) {
                $errors.Add("line $($index + 1): dismissal has no requested buttons")
                continue
            }
            if (@($event.requested_buttons | Where-Object { $_ -isnot [string] -or [string]::IsNullOrWhiteSpace([string]$_) }).Count -gt 0) {
                $errors.Add("line $($index + 1): dismissal requested_buttons must be nonempty JSON strings")
                continue
            }
            if ([bool]$event.succeeded -and
                ($event.PSObject.Properties.Name -notcontains "dismissed_button" -or [string]::IsNullOrWhiteSpace([string]$event.dismissed_button))) {
                $errors.Add("line $($index + 1): successful dismissal lacks dismissed_button")
                continue
            }
            if ([bool]$event.succeeded -and [string]$event.dismissed_button -notin @($event.requested_buttons)) {
                $errors.Add("line $($index + 1): dismissed_button was not requested")
                continue
            }
            $observationId = [string]$event.observation_id
            if (-not $observations.ContainsKey($observationId)) {
                $errors.Add("line $($index + 1): dismissal precedes or lacks its observation")
                continue
            }
            if (-not $dismissals.Add($observationId)) {
                $errors.Add("line $($index + 1): duplicate dismissal result")
                continue
            }
            $observation = $observations[$observationId]
            if ([string]$event.operation_id -ne [string]$observation.operation_id -or
                [string]$event.case_id -ne [string]$observation.case_id -or
                [string]$event.phase -ne [string]$observation.phase -or
                [string]$event.control_sequence -ne [string]$observation.control_sequence -or
                [string]$event.excel_pid -ne [string]$observation.excel_pid -or
                [string]$event.window_handle -ne [string]$observation.window_handle) {
                $errors.Add("line $($index + 1): dismissal link identity mismatch")
                continue
            }
            try {
                if ([DateTime]::Parse([string]$event.attempted_utc).ToUniversalTime() -lt [DateTime]::Parse([string]$observation.capture_completed_utc).ToUniversalTime()) {
                    $errors.Add("line $($index + 1): dismissal predates immutable capture completion")
                    continue
                }
            }
            catch { $errors.Add("line $($index + 1): dismissal timestamp is invalid"); continue }
            $records.Add($event)
            continue
        }
        if (-not $lifecycleRecognized) {
            $errors.Add("line $($index + 1): unknown guardian schema/event_type")
            continue
        }
        $records.Add($event)
    }
    return [pscustomobject]@{ records = @($records); errors = @($errors) }
}
