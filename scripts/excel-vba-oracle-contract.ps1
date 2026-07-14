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
    foreach ($field in @("number", "source", "description", "help_file", "help_context", "erl")) {
        if ($err.PSObject.Properties.Name -notcontains $field) {
            throw "excel-vba-oracle-contract: runtime Err payload is missing '$field'"
        }
    }
    return [pscustomobject]@{
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

function Get-ExcelOracleSha256 {
    param([Parameter(Mandatory = $true)][string]$Text)

    $bytes = [Text.UTF8Encoding]::new($false).GetBytes(($Text -replace "`r`n", "`n"))
    $hash = [Security.Cryptography.SHA256]::HashData($bytes)
    return "sha256:$([Convert]::ToHexString($hash).ToLowerInvariant())"
}

function Enter-ExcelOracleRunClaim {
    param(
        [Parameter(Mandatory = $true)][string]$OutputBase,
        [Parameter(Mandatory = $true)][string]$RunId
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
        $stream.Dispose()
        Remove-Item -LiteralPath $claimPath -Force -ErrorAction SilentlyContinue
        throw
    }
}

function Exit-ExcelOracleRunClaim {
    param([Parameter(Mandatory = $true)]$Claim)
    try { $Claim.stream.Dispose() }
    finally { Remove-Item -LiteralPath ([string]$Claim.claim_path) -Force -ErrorAction SilentlyContinue }
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

function Resolve-ExcelOraclePostCleanupResult {
    param(
        [Parameter(Mandatory = $true)][AllowNull()]$Results,
        [Parameter(Mandatory = $true)][AllowNull()]$ExcelLedger,
        [Parameter(Mandatory = $true)][AllowNull()]$HelperLedger,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$ExpectedCaseIds,
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)][int]$ExpectedWorkerPid,
        [Parameter(Mandatory = $true)][string]$ExpectedContainmentToken,
        [Parameter(Mandatory = $true)][bool]$ExpectedDiagnosticOnly,
        [Parameter(Mandatory = $true)][int]$WorkerExitCode,
        [Parameter(Mandatory = $true)][bool]$WorkerQuiesced,
        [Parameter(Mandatory = $true)][bool]$WorkerTimedOut
    )

    $errors = [Collections.Generic.List[string]]::new()
    $disposition = "invalid"
    $transport = $null
    if ($ExpectedCaseIds.Count -eq 0 -or @($ExpectedCaseIds | Where-Object { [string]::IsNullOrWhiteSpace($_) }).Count -gt 0 -or
        @($ExpectedCaseIds | Select-Object -Unique).Count -ne $ExpectedCaseIds.Count) {
        $errors.Add("expected case identity sequence is invalid")
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
    if ($null -eq $Results) {
        $errors.Add("results document is missing")
        return [pscustomobject]@{ valid = $false; disposition = $disposition; transport_error = $transport; errors = @($errors) }
    }

    $requiredDocumentFields = @("schema", "run_id", "generated_utc", "worker_pid", "containment_token", "containment_authority", "diagnostic_only", "cases", "passed")
    $actualDocumentFields = @($Results.PSObject.Properties.Name)
    if ((@($actualDocumentFields | Sort-Object) -join "`n") -cne (@($requiredDocumentFields | Sort-Object) -join "`n")) {
        $errors.Add("results document field set is invalid")
        return [pscustomobject]@{ valid = $false; disposition = $disposition; transport_error = $transport; errors = @($errors) }
    }
    if ($Results.schema -isnot [string] -or $Results.run_id -isnot [string] -or
        [string]$Results.schema -cne "oxvba.excel-vba-oracle-results.v1" -or [string]$Results.run_id -cne $RunId) {
        $errors.Add("results schema or run identity is invalid")
    }
    try {
        if ($Results.generated_utc -isnot [string]) { throw "not a string" }
        [void][DateTime]::Parse([string]$Results.generated_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind)
    }
    catch { $errors.Add("results generated_utc is invalid") }
    if (($Results.worker_pid -isnot [long] -and $Results.worker_pid -isnot [int]) -or [int]$Results.worker_pid -ne $ExpectedWorkerPid) {
        $errors.Add("results worker_pid is not the exact worker")
    }
    if ($Results.containment_token -isnot [string] -or [string]$Results.containment_token -cne $ExpectedContainmentToken) { $errors.Add("results containment token is invalid") }
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
        $authority.worker_job_membership_verified -isnot [bool] -or -not [bool]$authority.worker_job_membership_verified -or
        [string]::IsNullOrWhiteSpace([string]$authority.worker_process_start_utc) -or
        [string]::IsNullOrWhiteSpace([string]$authority.worker_executable_path) -or
        [string]::IsNullOrWhiteSpace([string]$authority.published_utc)) {
        $errors.Add("results containment authority is invalid")
    }
    else {
        try {
            [void][DateTime]::Parse([string]$authority.worker_process_start_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind)
            [void][DateTime]::Parse([string]$authority.published_utc, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::RoundtripKind)
        }
        catch { $errors.Add("results containment authority timestamps are invalid") }
    }

    $requiredCaseFields = @(
        "schema", "id", "purpose", "passed", "owned_excel_pid", "observed_excel_pid", "excel_ownership_recorded", "module_path", "module_sha256",
        "compile_status", "expected_compile_status", "compile_command", "compile_execution", "compile_context", "post_dismiss_selection_diagnostic_only",
        "compile_dialogs", "compile_window_observations", "run_procedure", "run_status", "expected_run_status", "run_value", "runtime_err",
        "macro_failure_disposition", "runtime_measurement", "transport_error", "run_dialogs", "evidence_status", "cleanup_status",
        "cleanup_authority_errors", "bootstrap_workbook", "defect_declaration"
    )
    $cases = @($Results.cases)
    $caseFieldSetInvalid = $false
    for ($index = 0; $index -lt $cases.Count; $index++) {
        $case = $cases[$index]
        if ($null -eq $case -or
            (@($case.PSObject.Properties.Name | Sort-Object) -join "`n") -cne (@($requiredCaseFields | Sort-Object) -join "`n")) {
            $errors.Add("case result $index field set is invalid")
            $caseFieldSetInvalid = $true
            continue
        }
        $requiredStringFields = @("schema", "id", "purpose", "module_path", "module_sha256", "compile_status", "expected_compile_status", "run_procedure", "run_status", "expected_run_status", "cleanup_status")
        if (@($requiredStringFields | Where-Object { $case.$_ -isnot [string] -or [string]::IsNullOrWhiteSpace([string]$case.$_) }).Count -gt 0 -or
            [string]$case.schema -cne "oxvba.excel-vba-oracle-case-result.v1" -or [string]$case.module_sha256 -notmatch '^sha256:[0-9a-f]{64}$') {
            $errors.Add("case result $index scalar identity is invalid")
        }
        if ($case.passed -isnot [bool] -or $case.excel_ownership_recorded -isnot [bool]) {
            $errors.Add("case result $index Boolean status is invalid")
        }
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
        if ($null -ne $case.transport_error -and $case.transport_error -isnot [string]) { $errors.Add("case result $index transport type is invalid") }
    }
    if ($caseFieldSetInvalid) {
        return [pscustomobject]@{ valid = $false; disposition = $disposition; transport_error = $transport; errors = @($errors) }
    }
    if ($Results.passed -is [bool] -and ([bool]$Results.passed -ne (@($cases | Where-Object { $_.passed -isnot [bool] -or -not [bool]$_.passed }).Count -eq 0))) {
        $errors.Add("aggregate passed status disagrees with case results")
    }

    [object[]]$excelRecords = [object[]]::new(0)
    [object[]]$helperRecords = [object[]]::new(0)
    if ($null -ne $ExcelLedger -and $ExcelLedger.PSObject.Properties.Name -contains "records") { $excelRecords = [object[]]@($ExcelLedger.records) }
    if ($null -ne $HelperLedger -and $HelperLedger.PSObject.Properties.Name -contains "records") { $helperRecords = [object[]]@($HelperLedger.records) }
    $caseIds = @($cases | ForEach-Object { [string]$_.id })
    $specialTransport = $cases.Count -eq 1 -and $ExpectedCaseIds.Count -gt 0 -and
        [string]$cases[0].id -ceq [string]$ExpectedCaseIds[0] -and
        $cases[0].passed -is [bool] -and -not [bool]$cases[0].passed -and
        $cases[0].excel_ownership_recorded -is [bool] -and -not [bool]$cases[0].excel_ownership_recorded -and
        $null -eq $cases[0].owned_excel_pid -and
        [string]$cases[0].compile_status -ceq "harness-error" -and [string]$cases[0].run_status -ceq "not-run" -and
        [string]$cases[0].cleanup_status -in @("not-run", "owned-process-zero", "job-contained-preownership") -and
        -not [string]::IsNullOrWhiteSpace([string]$cases[0].transport_error) -and
        $excelRecords.Count -eq 0 -and $helperRecords.Count -eq 0

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
        if (($caseIds -join "`n") -cne ($ExpectedCaseIds -join "`n")) { $errors.Add("case result order does not match the selected case sequence") }
        if ((@($excelRecords | ForEach-Object { [string]$_.case_id }) -join "`n") -cne ($ExpectedCaseIds -join "`n") -or
            (@($helperRecords | ForEach-Object { [string]$_.case_id }) -join "`n") -cne ($ExpectedCaseIds -join "`n")) {
            $errors.Add("ownership ledger order does not match the selected case sequence")
        }
        for ($index = 0; $index -lt [Math]::Min($cases.Count, $excelRecords.Count); $index++) {
            if ($cases[$index].excel_ownership_recorded -isnot [bool] -or -not [bool]$cases[$index].excel_ownership_recorded -or
                ($cases[$index].owned_excel_pid -isnot [long] -and $cases[$index].owned_excel_pid -isnot [int]) -or
                [int]$cases[$index].owned_excel_pid -ne [int]$excelRecords[$index].pid) {
                $errors.Add("case result $index does not bind to its durable Excel ownership record")
            }
        }
        $expectedExitCode = if ($Results.passed -is [bool] -and [bool]$Results.passed) { 0 } else { 1 }
        if ($WorkerExitCode -ne $expectedExitCode) { $errors.Add("worker exit code disagrees with aggregate result") }
        if ($errors.Count -eq 0) {
            if ([bool]$Results.passed) { $disposition = "complete-success" }
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
        try { $event = $line | ConvertFrom-Json }
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
