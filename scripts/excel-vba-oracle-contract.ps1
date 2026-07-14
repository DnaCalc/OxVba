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
    if ($signal -match 'compile error|sub or function not defined|expected:|syntax error|duplicate declaration|user-defined type not defined|expected array or user-defined type') {
        return [pscustomobject]@{
            kind = "compile-error"
            disposition = "capture-then-dismiss"
            preferred_buttons = @("OK")
        }
    }
    if ($signal -match 'run-time error|runtime error') {
        return [pscustomobject]@{
            kind = "runtime-error"
            disposition = "capture-then-dismiss"
            preferred_buttons = @("End", "OK")
        }
    }
    if ($signal -match 'cannot run the macro|macro may not be available|all macros may be disabled') {
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
        [Parameter(Mandatory = $true)][bool]$MacrosEnabled,
        [Parameter(Mandatory = $true)][bool]$TargetExists
    )

    $isGenericMacroFailure = $Message -match '(?i)cannot run the macro|macro may not be available|all macros may be disabled'
    if (-not $isGenericMacroFailure) {
        return "non-macro-runtime-failure"
    }
    if ($CompileStatus -ne "ok") {
        return "compile-failure"
    }
    if (-not $MacrosEnabled) {
        return "macros-disabled-or-policy"
    }
    if ($AccessVbom -and -not $TargetExists) {
        return "missing-macro"
    }
    if ($AccessVbom -and $MacrosEnabled -and $TargetExists) {
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
    RunProbe = capturedDescription
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

    return @(
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
        },
        [pscustomobject]@{
            id = "ambiguous-macro-failure"
            purpose = "generic Application.Run macro failure classified only after a clean forced compile"
            module_name = "OracleSelfTest"
            module_source = $ambiguousMacroSource
            expected_compile_status = "ok"
            run_procedure = "OracleSelfTest.RunProbe"
            target_exists = $false
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
        }
    )
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

function Test-ExcelOracleOwnedProcessRecord {
    param(
        [Parameter(Mandatory = $true)]$Record,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][int[]]$BaselineExcelPids,
        [Parameter(Mandatory = $true)][string]$RunId
    )

    foreach ($field in @("run_id", "ownership", "pid", "process_name", "process_start_utc", "executable_path")) {
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
    try { $pidValue = [int]$Record.pid }
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
    foreach ($field in @("run_id", "ownership", "role", "pid", "process_name", "process_start_utc", "executable_path")) {
        if ($Record.PSObject.Properties.Name -notcontains $field -or [string]::IsNullOrWhiteSpace([string]$Record.$field)) {
            return $false
        }
    }
    try { $recordedPid = [int]$Record.pid }
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
        [AllowEmptyCollection()][int[]]$BaselineExcelPids = @()
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
        $key = "$([string]$record.run_id)|$([string]$record.pid)|$([string]$record.process_start_utc)"
        if (-not $keys.Add($key)) {
            $errors.Add("line $($index + 1): duplicate $Kind ownership identity")
            continue
        }
        $records.Add($record)
    }
    return [pscustomobject]@{ records = @($records); errors = @($errors) }
}

function ConvertFrom-ExcelOracleGuardianEventLedger {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Lines,
        [Parameter(Mandatory = $true)][string]$RunId
    )

    $records = [Collections.Generic.List[object]]::new()
    $errors = [Collections.Generic.List[string]]::new()
    $observations = @{}
    $dismissals = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
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
        if ([string]$event.schema -eq "oxvba.excel-vba-oracle-window-observation.v1" -and [string]$event.event_type -in @("dialog-observation", "ignored-top-level-window")) {
            $required = @("observation_id", "operation_id", "phase", "excel_pid", "observed_process_id", "observed_utc", "window_handle", "classification", "disposition", "considered_dialog", "is_modal")
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
            try { $excelPid = [int]$event.excel_pid; $observedPid = [int]$event.observed_process_id }
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
            $required = @("observation_id", "operation_id", "excel_pid", "window_handle", "attempted_utc", "requested_buttons", "succeeded")
            $missing = @($required | Where-Object { $event.PSObject.Properties.Name -notcontains $_ -or [string]::IsNullOrWhiteSpace([string]$event.$_) })
            if ($missing.Count -gt 0) {
                $errors.Add("line $($index + 1): incomplete guardian dismissal ($($missing -join ','))")
                continue
            }
            if ($event.succeeded -isnot [bool]) {
                $errors.Add("line $($index + 1): dismissal succeeded is not a JSON boolean")
                continue
            }
            if (@($event.requested_buttons).Count -eq 0) {
                $errors.Add("line $($index + 1): dismissal has no requested buttons")
                continue
            }
            if ([bool]$event.succeeded -and
                ($event.PSObject.Properties.Name -notcontains "dismissed_button" -or [string]::IsNullOrWhiteSpace([string]$event.dismissed_button))) {
                $errors.Add("line $($index + 1): successful dismissal lacks dismissed_button")
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
                [string]$event.excel_pid -ne [string]$observation.excel_pid -or
                [string]$event.window_handle -ne [string]$observation.window_handle) {
                $errors.Add("line $($index + 1): dismissal link identity mismatch")
                continue
            }
            $records.Add($event)
            continue
        }
        $errors.Add("line $($index + 1): unknown guardian schema/event_type")
    }
    return [pscustomobject]@{ records = @($records); errors = @($errors) }
}
