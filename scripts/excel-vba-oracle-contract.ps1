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

Public Function ExistingProbe() As String
    ExistingProbe = "existing-ok"
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
            run_procedure = "OracleSelfTest.MissingMacro"
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

    if ([string]$Record.run_id -ne $RunId -or [string]$Record.ownership -ne "owned-new-instance") {
        return $false
    }
    $pidValue = [int]$Record.pid
    return $pidValue -gt 0 -and $pidValue -notin $BaselineExcelPids
}
