param(
    [Parameter(Mandatory = $true)][string]$RunId,
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [Parameter(Mandatory = $true)][string]$OwnershipFile,
    [Parameter(Mandatory = $true)][string]$HelperOwnershipFile,
    [Parameter(Mandatory = $true)][string]$ContainmentReadyFile,
    [Parameter(Mandatory = $true)][string]$ContainmentToken,
    [Parameter(Mandatory = $true)][string]$SelectedCaseDescriptorFile,
    [Parameter(Mandatory = $true)][string]$SelectedCaseDescriptorDigest,
    [ValidateRange(5, 600)][int]$CaseTimeoutSeconds = 90
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "excel-vba-oracle-contract.ps1")
. (Join-Path $PSScriptRoot "excel-vba-oracle-job.ps1")
. (Join-Path $PSScriptRoot "excel-vba-oracle-bootstrap.ps1")

if (-not ([System.Management.Automation.PSTypeName]'ExcelOracleNativeMethods').Type) {
    Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public sealed class ExcelOracleWindowCandidate
{
    public long Hwnd { get; set; }
    public long TopLevelHwnd { get; set; }
    public bool IsTopLevel { get; set; }
    public uint ProcessId { get; set; }
    public string ClassName { get; set; }
    public string Title { get; set; }
    public bool Visible { get; set; }
}

public sealed class ExcelOracleWindowEnumeration
{
    public ExcelOracleWindowCandidate[] Windows { get; set; }
    public bool Truncated { get; set; }
    public int Limit { get; set; }
    public bool Succeeded { get; set; }
    public int ErrorCode { get; set; }
}

public static class ExcelOracleNativeMethods
{
    private delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr state);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr state);

    [DllImport("user32.dll")]
    private static extern bool EnumChildWindows(IntPtr parent, EnumWindowsProc callback, IntPtr state);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetClassName(IntPtr hwnd, StringBuilder className, int maxCount);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr hwnd, StringBuilder title, int maxCount);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr hwnd);

    [DllImport("oleacc.dll")]
    private static extern int AccessibleObjectFromWindow(
        IntPtr hwnd,
        uint objectId,
        ref Guid interfaceId,
        [MarshalAs(UnmanagedType.Interface)] out object nativeObject);

    private static string ReadClassName(IntPtr hwnd)
    {
        StringBuilder text = new StringBuilder(257);
        int count = GetClassName(hwnd, text, text.Capacity);
        return count > 0 ? text.ToString(0, count) : String.Empty;
    }

    private static string ReadWindowTitle(IntPtr hwnd)
    {
        StringBuilder text = new StringBuilder(513);
        int count = GetWindowText(hwnd, text, text.Capacity);
        return count > 0 ? text.ToString(0, count) : String.Empty;
    }

    private static ExcelOracleWindowCandidate DescribeWindow(IntPtr hwnd, IntPtr topLevel, bool isTopLevel, uint processId)
    {
        return new ExcelOracleWindowCandidate {
            Hwnd = hwnd.ToInt64(),
            TopLevelHwnd = topLevel.ToInt64(),
            IsTopLevel = isTopLevel,
            ProcessId = processId,
            ClassName = ReadClassName(hwnd),
            Title = ReadWindowTitle(hwnd),
            Visible = IsWindowVisible(hwnd)
        };
    }

    public static ExcelOracleWindowEnumeration EnumerateOwnedWindows(uint expectedProcessId)
    {
        const int WindowLimit = 512;
        List<ExcelOracleWindowCandidate> windows = new List<ExcelOracleWindowCandidate>();
        bool truncated = false;
        bool completed = EnumWindows(delegate(IntPtr topLevel, IntPtr ignored) {
            uint topLevelProcessId;
            GetWindowThreadProcessId(topLevel, out topLevelProcessId);
            if (topLevelProcessId != expectedProcessId) return true;
            if (windows.Count >= WindowLimit) { truncated = true; return false; }
            windows.Add(DescribeWindow(topLevel, topLevel, true, topLevelProcessId));
            EnumChildWindows(topLevel, delegate(IntPtr child, IntPtr childState) {
                uint childProcessId;
                GetWindowThreadProcessId(child, out childProcessId);
                if (childProcessId == expectedProcessId) {
                    if (windows.Count >= WindowLimit) { truncated = true; return false; }
                    windows.Add(DescribeWindow(child, topLevel, false, childProcessId));
                }
                return true;
            }, IntPtr.Zero);
            return !truncated;
        }, IntPtr.Zero);
        int errorCode = completed ? 0 : Marshal.GetLastWin32Error();
        return new ExcelOracleWindowEnumeration {
            Windows = windows.ToArray(), Truncated = truncated, Limit = WindowLimit,
            Succeeded = completed && !truncated, ErrorCode = truncated ? 0 : errorCode
        };
    }

    public static int TryGetNativeObjectFromWindow(IntPtr hwnd, out object nativeObject)
    {
        const uint OBJID_NATIVEOM = 0xFFFFFFF0;
        Guid dispatch = new Guid("00020400-0000-0000-C000-000000000046");
        return AccessibleObjectFromWindow(hwnd, OBJID_NATIVEOM, ref dispatch, out nativeObject);
    }
}
'@
}

function Wait-ContainmentAuthority {
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $ContainmentReadyFile) {
            try {
                $ready = Get-Content -Raw -LiteralPath $ContainmentReadyFile | ConvertFrom-Json
                if ([string]$ready.schema -ne "oxvba.excel-vba-oracle-containment-ready.v1" -or
                    [string]$ready.run_id -ne $RunId -or
                    [string]$ready.containment_token -ne $ContainmentToken -or
                    [int]$ready.worker_pid -ne $PID -or
                    $ready.worker_job_membership_verified -isnot [bool] -or -not [bool]$ready.worker_job_membership_verified) {
                    throw "containment ready identity mismatch"
                }
                return $ready
            }
            catch { throw "excel-vba-oracle-worker: invalid containment authority: $($_.Exception.Message)" }
        }
        Start-Sleep -Milliseconds 25
    }
    throw "excel-vba-oracle-worker: containment authority was not published before mutation"
}

function Get-ExcelExecutablePath {
    foreach ($keyPath in @(
        "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\excel.exe",
        "HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\excel.exe"
    )) {
        if (-not (Test-Path -LiteralPath $keyPath)) { continue }
        $candidate = [string](Get-Item -LiteralPath $keyPath).GetValue("")
        if (-not [string]::IsNullOrWhiteSpace($candidate) -and (Test-Path -LiteralPath $candidate)) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }
    $command = Get-Command excel.exe -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($command -and (Test-Path -LiteralPath $command.Source)) { return (Resolve-Path -LiteralPath $command.Source).Path }
    throw "excel-vba-oracle-worker: Excel executable was not found"
}

function Write-ExcelAttachmentDiagnostic {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$ProcessIdentity,
        [Parameter(Mandatory = $true)][int]$AttemptCount,
        [Parameter(Mandatory = $true)][int]$TruncatedObservationCount,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Observations,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$LastWindows,
        [Parameter(Mandatory = $true)][bool]$WindowEnumerationTruncated,
        [Parameter(Mandatory = $true)][bool]$WindowEnumerationSucceeded,
        [Parameter(Mandatory = $true)][int]$WindowEnumerationErrorCode,
        [Parameter(Mandatory = $true)][ValidateSet("attached", "blocked-owned-window", "process-exited", "deadline", "window-enumeration-truncated", "window-enumeration-invalid")][string]$Outcome
    )
    $temporary = "$Path.$PID.tmp"
    [ordered]@{
        schema = "oxvba.excel-vba-oracle-attachment-diagnostic.v1"
        run_id = $RunId
        generated_utc = [DateTime]::UtcNow.ToString("o")
        outcome = $Outcome
        process = $ProcessIdentity
        attempt_count = $AttemptCount
        observation_limit = 256
        window_enumeration_limit = 512
        window_enumeration_truncated = $WindowEnumerationTruncated
        window_enumeration_succeeded = $WindowEnumerationSucceeded
        window_enumeration_error_code = $WindowEnumerationErrorCode
        last_window_diagnostic_limit = 128
        observation_count = @($Observations).Count
        truncated_observation_count = $TruncatedObservationCount
        observations = @($Observations)
        last_owned_windows = @($LastWindows)
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $temporary -Encoding utf8NoBOM
    Move-Item -LiteralPath $temporary -Destination $Path -Force
}

function Start-OwnedExcelApplication {
    param(
        [Parameter(Mandatory = $true)][string]$ExcelExecutable,
        [Parameter(Mandatory = $true)]$BootstrapWorkbook,
        [Parameter(Mandatory = $true)][string]$DiagnosticPath
    )
    $startInfo = New-ExcelOracleProcessStartInfo -ExcelExecutable $ExcelExecutable -BootstrapWorkbook $BootstrapWorkbook
    $process = [Diagnostics.Process]::Start($startInfo)
    if ($null -eq $process) { throw "excel-vba-oracle-worker: direct Excel bootstrap launch returned no process" }
    $processIdentity = [ordered]@{
        pid = $process.Id
        process_start_utc = $process.StartTime.ToUniversalTime().ToString("o")
        process_name = [string]$process.ProcessName
        executable_path = $ExcelExecutable
        launch_argv = @("/x", [string]$BootstrapWorkbook.path)
        bootstrap_workbook = $BootstrapWorkbook
    }
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    $attemptCount = 0
    $truncatedObservationCount = 0
    $observations = [Collections.Generic.List[object]]::new()
    $lastWindows = @()
    while ([DateTime]::UtcNow -lt $deadline) {
        $attemptCount++
        if ($process.HasExited) {
            Write-ExcelAttachmentDiagnostic -Path $DiagnosticPath -ProcessIdentity $processIdentity -AttemptCount $attemptCount -TruncatedObservationCount $truncatedObservationCount -Observations @($observations) -LastWindows @($lastWindows) -WindowEnumerationTruncated $false -WindowEnumerationSucceeded $false -WindowEnumerationErrorCode 0 -Outcome process-exited
            throw "excel-vba-oracle-worker: directly launched Excel exited before automation attachment; diagnostic '$DiagnosticPath'"
        }
        $process.Refresh()
        $enumeration = [ExcelOracleNativeMethods]::EnumerateOwnedWindows([uint32]$process.Id)
        $ownedWindows = @($enumeration.Windows |
            Sort-Object @{ Expression = { if ([string]$_.ClassName -eq "EXCEL7") { 0 } elseif ([bool]$_.IsTopLevel) { 1 } else { 2 } } }, Hwnd)
        $lastWindows = @($ownedWindows | Select-Object -First 128)
        if (-not (Test-ExcelOracleWindowEnumerationAuthority -Enumeration $enumeration -ExpectedProcessId $process.Id)) {
            $enumerationTruncated = $enumeration.Truncated -is [bool] -and [bool]$enumeration.Truncated
            $enumerationSucceeded = $enumeration.Succeeded -is [bool] -and [bool]$enumeration.Succeeded
            $enumerationError = if ($enumeration.ErrorCode -is [int] -or $enumeration.ErrorCode -is [long]) { [int]$enumeration.ErrorCode } else { -1 }
            $enumerationOutcome = if ($enumerationTruncated) { "window-enumeration-truncated" } else { "window-enumeration-invalid" }
            Write-ExcelAttachmentDiagnostic -Path $DiagnosticPath -ProcessIdentity $processIdentity -AttemptCount $attemptCount -TruncatedObservationCount $truncatedObservationCount -Observations @($observations) -LastWindows @($lastWindows) -WindowEnumerationTruncated $enumerationTruncated -WindowEnumerationSucceeded $enumerationSucceeded -WindowEnumerationErrorCode $enumerationError -Outcome $enumerationOutcome
            throw "excel-vba-oracle-worker: exact-PID owned-window enumeration was incomplete or invalid; refusing partial attachment authority; diagnostic '$DiagnosticPath'"
        }
        $blockingWindows = @($ownedWindows | Where-Object { Test-ExcelStartupBlockingWindow -Window $_ })
        foreach ($window in $ownedWindows) {
            $nativeWindow = $null
            $application = $null
            $applicationPid = $null
            $adjudication = $null
            $hr = [ExcelOracleNativeMethods]::TryGetNativeObjectFromWindow([IntPtr][int64]$window.Hwnd, [ref]$nativeWindow)
            $result = "attachment-candidate-unadjudicated"
            try {
                if ($null -ne $nativeWindow) { $application = $nativeWindow.Application }
                if ($null -ne $application) { $applicationPid = Get-ExcelPidFromApplication -Application $application }
                $adjudication = Resolve-ExcelOracleAttachmentCandidate -Enumeration $enumeration -ExpectedProcessId $process.Id -Candidate $window `
                    -HResult $hr -NativeObjectPresent ($null -ne $nativeWindow) -ApplicationPresent ($null -ne $application) -ApplicationPid $applicationPid
                $result = [string]$adjudication.disposition
            }
            catch { $result = "application-error:$($_.Exception.GetType().FullName):$($_.Exception.Message)" }
            $observation = [pscustomobject]@{
                observed_utc = [DateTime]::UtcNow.ToString("o")
                attempt = $attemptCount
                pid = [int]$window.ProcessId
                hwnd = [string]$window.Hwnd
                top_level_hwnd = [string]$window.TopLevelHwnd
                is_top_level = [bool]$window.IsTopLevel
                class_name = [string]$window.ClassName
                title = [string]$window.Title
                visible = [bool]$window.Visible
                hresult = ("0x{0:X8}" -f $hr)
                result = $result
            }
            if ($observations.Count -lt 256) { $observations.Add($observation) } else { $truncatedObservationCount++ }
            if ($null -ne $adjudication -and [bool]$adjudication.attached) {
                Write-ExcelAttachmentDiagnostic -Path $DiagnosticPath -ProcessIdentity $processIdentity -AttemptCount $attemptCount -TruncatedObservationCount $truncatedObservationCount -Observations @($observations) -LastWindows @($lastWindows) -WindowEnumerationTruncated $false -WindowEnumerationSucceeded $true -WindowEnumerationErrorCode 0 -Outcome attached
                return [pscustomobject]@{ process = $process; application = $application; native_window = $nativeWindow }
            }
            Release-ComObject $application
            Release-ComObject $nativeWindow
        }
        if ($blockingWindows.Count -gt 0) {
            Write-ExcelAttachmentDiagnostic -Path $DiagnosticPath -ProcessIdentity $processIdentity -AttemptCount $attemptCount -TruncatedObservationCount $truncatedObservationCount -Observations @($observations) -LastWindows @($lastWindows) -WindowEnumerationTruncated $false -WindowEnumerationSucceeded $true -WindowEnumerationErrorCode 0 -Outcome blocked-owned-window
            $blocked = @($blockingWindows | ForEach-Object { "HWND=$($_.Hwnd) class='$($_.ClassName)' title='$($_.Title)'" }) -join "; "
            throw "excel-vba-oracle-worker: directly launched Excel exposed an owned visible startup/modal window; refusing broad interaction: $blocked; diagnostic '$DiagnosticPath'"
        }
        Start-Sleep -Milliseconds 100
    }
    Write-ExcelAttachmentDiagnostic -Path $DiagnosticPath -ProcessIdentity $processIdentity -AttemptCount $attemptCount -TruncatedObservationCount $truncatedObservationCount -Observations @($observations) -LastWindows @($lastWindows) -WindowEnumerationTruncated $false -WindowEnumerationSucceeded $true -WindowEnumerationErrorCode 0 -Outcome deadline
    throw "excel-vba-oracle-worker: directly launched Excel did not expose exact-PID OBJID_NATIVEOM through an owned window; diagnostic '$DiagnosticPath'"
}

function Get-ExcelProcessIds {
    return @(Get-Process -Name EXCEL -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
}

function Get-ExcelPidFromApplication {
    param([Parameter(Mandatory = $true)]$Application)
    $processId = [uint32]0
    [void][ExcelOracleNativeMethods]::GetWindowThreadProcessId([IntPtr][int64]$Application.Hwnd, [ref]$processId)
    if ($processId -eq 0) { throw "excel-vba-oracle-worker: Excel Hwnd did not resolve to a process" }
    return [int]$processId
}

function Add-OwnershipRecord {
    param(
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][int[]]$BeforePids,
        [Parameter(Mandatory = $true)][string]$CaseId
    )
    $record = [ordered]@{
        schema = "oxvba.excel-vba-oracle-owned-process.v1"
        run_id = $RunId
        case_id = $CaseId
        pid = $Process.Id
        process_name = [string]$Process.ProcessName
        process_start_utc = $Process.StartTime.ToUniversalTime().ToString("o")
        executable_path = [string]$Process.Path
        before_excel_pids = @($BeforePids)
        ownership = "owned-new-instance"
        acquired_utc = [DateTime]::UtcNow.ToString("o")
    }
    ($record | ConvertTo-Json -Compress -Depth 5) | Add-Content -LiteralPath $OwnershipFile -Encoding utf8NoBOM
    return [pscustomobject]$record
}

function Add-HelperOwnershipRecord {
    param(
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)][string]$CaseId
    )
    $record = [ordered]@{
        schema = "oxvba.excel-vba-oracle-owned-helper.v1"
        run_id = $RunId
        case_id = $CaseId
        role = "guardian"
        pid = $Process.Id
        process_name = [string]$Process.ProcessName
        process_start_utc = $Process.StartTime.ToUniversalTime().ToString("o")
        executable_path = [string]$Process.Path
        ownership = "owned-helper-process"
        acquired_utc = [DateTime]::UtcNow.ToString("o")
    }
    ($record | ConvertTo-Json -Compress -Depth 5) | Add-Content -LiteralPath $HelperOwnershipFile -Encoding utf8NoBOM
    return [pscustomobject]$record
}

function Set-GuardianControl {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$CaseId,
        [Parameter(Mandatory = $true)][string]$OperationId,
        [Parameter(Mandatory = $true)][ValidateSet("compile", "run", "cleanup")][string]$Phase,
        [Parameter(Mandatory = $true)][bool]$AllowDismiss
    )
    $script:GuardianControlSequence++
    $temporary = "$Path.$PID.tmp"
    [ordered]@{
        schema = "oxvba.excel-vba-oracle-guardian-control.v2"
        run_id = $RunId
        case_id = $CaseId
        operation_id = $OperationId
        sequence = $script:GuardianControlSequence
        phase = $Phase
        allow_dismiss = $AllowDismiss
        written_utc = [DateTime]::UtcNow.ToString("o")
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $temporary -Encoding utf8NoBOM
    Move-Item -Force -LiteralPath $temporary -Destination $Path
    return [pscustomobject]@{ case_id = $CaseId; operation_id = $OperationId; phase = $Phase; sequence = $script:GuardianControlSequence }
}

function Get-GuardianEvents {
    param(
        [Parameter(Mandatory = $true)][string]$EventsFile,
        [string]$OperationId
    )
    $lines = if (Test-Path -LiteralPath $EventsFile) { @(Get-Content -LiteralPath $EventsFile) } else { @() }
    $ledger = ConvertFrom-ExcelOracleGuardianEventLedger -Lines $lines -RunId $RunId -ExpectedCaseIds $script:SelectedCaseIds
    if (@($ledger.errors).Count -gt 0) {
        throw "excel-vba-oracle-worker: invalid guardian event ledger: $($ledger.errors -join '; ')"
    }
    return @($ledger.records | Where-Object {
        [string]::IsNullOrWhiteSpace($OperationId) -or [string]$_.operation_id -eq $OperationId
    })
}

function Wait-GuardianReady {
    param(
        [Parameter(Mandatory = $true)][string]$ReadyFile,
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process
    )
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $ReadyFile) {
            try {
                $ready = Get-Content -Raw -LiteralPath $ReadyFile | ConvertFrom-Json
                if ([string]$ready.schema -ne "oxvba.excel-vba-oracle-guardian-ready.v1" -or [int]$ready.guardian_pid -ne $Process.Id) {
                    throw "guardian ready schema/PID mismatch"
                }
                if (-not (Test-ExcelOracleProcessIdentity -Record $ready -Process $Process -ExpectedProcessName $Process.ProcessName -RunId $RunId)) {
                    throw "guardian ready PID/start/name/executable identity mismatch"
                }
                return $ready
            }
            catch {
                throw "excel-vba-oracle-worker: invalid guardian ready record: $($_.Exception.Message)"
            }
        }
        if ($Process.HasExited) { throw "excel-vba-oracle-worker: guardian exited before becoming ready (exit $($Process.ExitCode))" }
        Start-Sleep -Milliseconds 50
    }
    throw "excel-vba-oracle-worker: guardian did not become ready"
}

function Assert-GuardianLive {
    param(
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)]$ReadyRecord,
        [Parameter(Mandatory = $true)][string]$Phase
    )
    if ($Process.HasExited -or -not (Test-ExcelOracleProcessIdentity -Record $ReadyRecord -Process $Process -ExpectedProcessName $Process.ProcessName -RunId $RunId)) {
        throw "excel-vba-oracle-worker: guardian is not live with its exact identity immediately before $Phase"
    }
}

function Wait-GuardianEventFlush {
    param(
        [Parameter(Mandatory = $true)][string]$EventsFile,
        [Parameter(Mandatory = $true)][string]$OperationId,
        [Parameter(Mandatory = $true)]$ArmRecord,
        [Parameter(Mandatory = $true)][DateTime]$InvocationCompletedUtc
    )
    $deadline = [DateTime]::UtcNow.AddMilliseconds(2500)
    do {
        $events = @(Get-GuardianEvents -EventsFile $EventsFile -OperationId $OperationId)
        if (Test-ExcelOracleGuardianOperationCoverage -Events $events -OperationId $OperationId -ControlSequence ([long]$ArmRecord.control_sequence) -InvocationCompletedUtc $InvocationCompletedUtc) { return $events }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "excel-vba-oracle-worker: guardian heartbeat did not span operation '$OperationId' through evidence flush"
}

function Wait-GuardianOperationArmed {
    param(
        [Parameter(Mandatory = $true)][string]$EventsFile,
        [Parameter(Mandatory = $true)]$Control,
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)]$ReadyRecord
    )
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        Assert-GuardianLive -Process $Process -ReadyRecord $ReadyRecord -Phase "operation arm acknowledgement"
        $events = @(Get-GuardianEvents -EventsFile $EventsFile -OperationId $Control.operation_id)
        $armed = @($events | Where-Object {
            [string]$_.event_type -eq "operation-armed" -and
            [string]$_.case_id -eq [string]$Control.case_id -and
            [string]$_.phase -eq [string]$Control.phase -and
            [long]$_.control_sequence -eq [long]$Control.sequence
        })
        if ($armed.Count -eq 1) { return $armed[0] }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "excel-vba-oracle-worker: guardian did not acknowledge operation '$($Control.operation_id)'"
}

function Get-VbeCompileControl {
    param([Parameter(Mandatory = $true)]$Vbe)
    try {
        $found = $Vbe.CommandBars.FindControl($null, 578, $null, $null, $true)
        if ($null -ne $found) { return $found }
    }
    catch { }

    foreach ($bar in @($Vbe.CommandBars)) {
        foreach ($control in @($bar.Controls)) {
            try {
                if ([int]$control.Id -eq 578) { return $control }
                foreach ($child in @($control.Controls)) {
                    if ([int]$child.Id -eq 578) { return $child }
                }
            }
            catch { }
        }
    }
    return $null
}

function Get-VbeSelectionFromCom {
    param([Parameter(Mandatory = $true)]$Vbe)
    try {
        $pane = $Vbe.ActiveCodePane
        if ($null -eq $pane) { return $null }
        $startLine = 0
        $startColumn = 0
        $endLine = 0
        $endColumn = 0
        $pane.GetSelection([ref]$startLine, [ref]$startColumn, [ref]$endLine, [ref]$endColumn)
        $line = [string]$pane.CodeModule.Lines($startLine, 1)
        $token = $null
        if ($endLine -eq $startLine -and $endColumn -gt $startColumn -and $startColumn -gt 0) {
            $offset = $startColumn - 1
            $length = [Math]::Min($line.Length - $offset, $endColumn - $startColumn)
            if ($offset -ge 0 -and $length -gt 0) { $token = $line.Substring($offset, $length) }
        }
        return [pscustomobject]@{
            schema = "oxvba.excel-vba-oracle-vbe-selection.v1"
            selected_token = if ($token) { $token.Trim() } else { $null }
            expanded_line = $line.Trim("`r", "`n")
        }
    }
    catch { return $null }
}

function Test-ComObjectIdentity {
    param($Left, $Right)
    if ($null -eq $Left -or $null -eq $Right) { return $false }
    $leftIdentity = [IntPtr]::Zero
    $rightIdentity = [IntPtr]::Zero
    try {
        $leftIdentity = [Runtime.InteropServices.Marshal]::GetIUnknownForObject($Left)
        $rightIdentity = [Runtime.InteropServices.Marshal]::GetIUnknownForObject($Right)
        return $leftIdentity -eq $rightIdentity
    }
    finally {
        if ($leftIdentity -ne [IntPtr]::Zero) { [void][Runtime.InteropServices.Marshal]::Release($leftIdentity) }
        if ($rightIdentity -ne [IntPtr]::Zero) { [void][Runtime.InteropServices.Marshal]::Release($rightIdentity) }
    }
}

function Get-ProjectFileName {
    param($Project)
    try { return [string]$Project.FileName }
    catch { return $null }
}

function Get-CompileAuthoritySnapshot {
    param(
        [Parameter(Mandatory = $true)]$Vbe,
        [Parameter(Mandatory = $true)]$Project,
        [Parameter(Mandatory = $true)]$Component,
        [Parameter(Mandatory = $true)][string]$ExpectedSourceSha256,
        [Parameter(Mandatory = $true)][string]$Stage
    )
    $activeProject = $null
    $activePane = $null
    $activeModule = $null
    $injectedPane = $null
    $activeProject = $Vbe.ActiveVBProject
    $activePane = $Vbe.ActiveCodePane
    $activeModule = if ($activePane) { $activePane.CodeModule } else { $null }
    $injectedPane = $Component.CodeModule.CodePane
    $source = [string]$Component.CodeModule.Lines(1, $Component.CodeModule.CountOfLines)
    $snapshot = [pscustomobject]@{
        schema = "oxvba.excel-vba-oracle-compile-authority-snapshot.v1"
        stage = $Stage
        captured_utc = [DateTime]::UtcNow.ToString("o")
        active_project_is_injected_project = Test-ComObjectIdentity -Left $activeProject -Right $Project
        active_module_is_injected_module = if ($activeModule) { Test-ComObjectIdentity -Left $activeModule -Right $Component.CodeModule } else { $false }
        active_code_pane_is_injected_code_pane = if ($activePane -and $injectedPane) { Test-ComObjectIdentity -Left $activePane -Right $injectedPane } else { $false }
        active_project_name = if ($activeProject) { [string]$activeProject.Name } else { $null }
        active_module_name = if ($activeModule) { [string]$activeModule.Parent.Name } else { $null }
        injected_source_sha256 = Get-ExcelOracleSha256 -Text $source
        expected_source_sha256 = $ExpectedSourceSha256
    }
    if (-not $snapshot.active_project_is_injected_project -or -not $snapshot.active_module_is_injected_module -or
        -not $snapshot.active_code_pane_is_injected_code_pane -or $snapshot.injected_source_sha256 -cne $ExpectedSourceSha256) {
        throw "compile authority mismatch at $Stage"
    }
    # Active project/module/pane values are borrowed aliases and may share RCWs
    # with caller-owned objects. Never FinalRelease them inside this snapshot.
    return $snapshot
}

function Release-ComObject {
    param($Value)
    if ($null -ne $Value -and [Runtime.InteropServices.Marshal]::IsComObject($Value)) {
        try { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($Value) }
        catch { }
    }
}

function Get-ExcelOracleEvidenceFailureTransport {
    param(
        [Parameter(Mandatory = $true)]$Descriptor,
        [Parameter(Mandatory = $true)][bool]$GuardianHealthy,
        [Parameter(Mandatory = $true)][bool]$AuthoritativeEvidencePassed
    )

    if (-not $GuardianHealthy -and -not $AuthoritativeEvidencePassed) {
        return "observed VBA behavior matched the sealed case, but the final guardian authority was unhealthy and authoritative evidence did not satisfy '$([string]$Descriptor.evidence_contract)'"
    }
    if (-not $GuardianHealthy) {
        return "observed VBA behavior matched the sealed case, but the final guardian authority was not healthy and exact"
    }
    return "observed VBA behavior matched the sealed case, but authoritative guardian evidence did not satisfy '$([string]$Descriptor.evidence_contract)'"
}

function Invoke-HarnessCase {
    param([Parameter(Mandatory = $true)]$Descriptor)

    if (-not (Test-ExcelOracleSelectedCaseDescriptor -Descriptor $Descriptor)) {
        throw "excel-vba-oracle-worker: attempted to execute an invalid selected descriptor"
    }

    $caseDirectory = Join-Path $OutputDirectory $Descriptor.id
    New-Item -ItemType Directory -Force -Path $caseDirectory | Out-Null
    $modulePath = Join-Path $caseDirectory "$($Descriptor.module_name).bas"
    Set-Content -LiteralPath $modulePath -Value $Descriptor.module_source -Encoding utf8NoBOM

    $beforePids = @(Get-ExcelProcessIds)
    $excel = $null
    $excelNativeWindow = $null
    $workbook = $null
    $project = $null
    $component = $null
    $compileControl = $null
    $guardian = $null
    $guardianOwnershipRecord = $null
    $ownedExcelProcess = $null
    $excelOwnershipRecord = $null
    $excelPid = $null
    $controlFile = Join-Path $caseDirectory "guardian-control.json"
    $eventsFile = Join-Path $caseDirectory "guardian-events.jsonl"
    $readyFile = Join-Path $caseDirectory "guardian-ready.json"
    $stopFile = Join-Path $caseDirectory "guardian-stop"
    $excelIdentityFile = Join-Path $caseDirectory "excel-process-identity.json"
    $excelAttachmentDiagnosticFile = Join-Path $caseDirectory "excel-attachment-diagnostic.json"
    $bootstrapWorkbookPath = Join-Path $caseDirectory "oracle-bootstrap.xlsx"
    $guardianStdout = Join-Path $caseDirectory "guardian.stdout.txt"
    $guardianStderr = Join-Path $caseDirectory "guardian.stderr.txt"
    $compileEvents = @()
    $compileDialogs = @()
    $compileCommand = $null
    $compileExecution = $null
    $compileContext = $null
    $postDismissSelectionDiagnostic = $null
    $runtimeMeasurement = $null
    $runEvents = @()
    $compileStatus = "not-run"
    $runStatus = "not-run"
    $runValue = $null
    $runtimeErr = $null
    $errorMessage = $null
    $runtimeFailureMessage = $null
    $macroDisposition = $null
    $passed = $false
    $behaviorPassed = $false
    $guardianHealthy = $false
    $evidenceStatus = $null
    $cleanupStatus = "not-run"
    $cleanupAuthorityErrors = [Collections.Generic.List[string]]::new()
    $bootstrapWorkbook = $null
    $bootstrapSha256After = $null
    $preOwnershipFailurePhase = "bootstrap-construction"

    try {
        $bootstrapWorkbook = New-ExcelOracleBootstrapWorkbook -Path $bootstrapWorkbookPath
        if (-not (Test-ExcelOracleBootstrapWorkbook -Descriptor $bootstrapWorkbook)) {
            throw "excel-vba-oracle-worker: controlled bootstrap workbook is missing, modified, or has invalid OPC relationship closure before launch"
        }
        $preOwnershipFailurePhase = "excel-attachment"
        $launchedExcel = Start-OwnedExcelApplication -ExcelExecutable $script:ExcelExecutablePath -BootstrapWorkbook $bootstrapWorkbook -DiagnosticPath $excelAttachmentDiagnosticFile
        $preOwnershipFailurePhase = "excel-ownership"
        $ownedExcelProcess = $launchedExcel.process
        $excel = $launchedExcel.application
        $excelNativeWindow = $launchedExcel.native_window
        $excel.Visible = $true
        $excel.DisplayAlerts = $false
        $excel.AutomationSecurity = 1
        $excelPid = Get-ExcelPidFromApplication -Application $excel
        if ($excelPid -ne $ownedExcelProcess.Id) {
            throw "excel-vba-oracle-worker: OBJID_NATIVEOM application PID does not match the directly launched contained Excel process"
        }
        if ($excelPid -in $beforePids) {
            throw "excel-vba-oracle-worker: Excel PID $excelPid existed before this case; refusing ownership"
        }
        $excelOwnershipRecord = Add-OwnershipRecord -Process $ownedExcelProcess -BeforePids $beforePids -CaseId $Descriptor.id
        $preOwnershipFailurePhase = $null
        $excelOwnershipRecord | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $excelIdentityFile -Encoding utf8NoBOM

        $guardianArguments = @(
            "-NoLogo", "-NoProfile", "-NonInteractive", "-STA", "-File", (Join-Path $PSScriptRoot "excel-vba-oracle-guardian.ps1"),
            "-ExcelPid", [string]$excelPid,
            "-ExcelIdentityFile", $excelIdentityFile,
            "-RunId", $RunId,
            "-CaseId", [string]$Descriptor.id,
            "-ControlFile", $controlFile,
            "-EventsFile", $eventsFile,
            "-ReadyFile", $readyFile,
            "-StopFile", $stopFile,
            "-MaxSeconds", [string]$CaseTimeoutSeconds
        )
        $guardian = Start-Process -FilePath (Join-Path $PSHOME "pwsh.exe") -ArgumentList $guardianArguments -PassThru -WindowStyle Hidden -RedirectStandardOutput $guardianStdout -RedirectStandardError $guardianStderr
        $guardianOwnershipRecord = Add-HelperOwnershipRecord -Process $guardian -CaseId $Descriptor.id
        $guardianReady = Wait-GuardianReady -ReadyFile $readyFile -Process $guardian

        if ([int]$excel.Workbooks.Count -ne 1) {
            throw "excel-vba-oracle-worker: directly launched Excel did not open exactly one controlled bootstrap workbook"
        }
        $workbook = $excel.Workbooks.Item(1)
        $actualBootstrapPath = [IO.Path]::GetFullPath([string]$workbook.FullName)
        if (-not [string]::Equals($actualBootstrapPath, [string]$bootstrapWorkbook.path, [StringComparison]::OrdinalIgnoreCase)) {
            throw "excel-vba-oracle-worker: attached Excel workbook does not match the controlled bootstrap path"
        }
        $workbook.Activate()
        $project = $workbook.VBProject
        $component = $project.VBComponents.Add(1)
        $component.Name = $Descriptor.module_name
        [void]$component.CodeModule.AddFromString($Descriptor.module_source)
        $excel.VBE.MainWindow.Visible = $true
        $component.CodeModule.CodePane.Show()
        try { $excel.VBE.MainWindow.SetFocus() } catch { }
        Start-Sleep -Milliseconds 250

        $injectedSource = [string]$component.CodeModule.Lines(1, $component.CodeModule.CountOfLines)
        $injectedSourceSha256 = Get-ExcelOracleSha256 -Text $injectedSource
        $selectedSourceSha256 = Get-ExcelOracleSha256 -Text ([string]$Descriptor.module_source)
        if ($injectedSourceSha256 -cne $selectedSourceSha256) {
            throw "excel-vba-oracle-worker: injected module source does not match the selected case before compile authority"
        }
        $compileContext = [pscustomobject]@{
            schema = "oxvba.excel-vba-oracle-compile-context.v1"
            injected_project_name = [string]$project.Name
            injected_project_file_name = Get-ProjectFileName -Project $project
            injected_module_name = [string]$component.Name
            selection_before_execute = Get-VbeSelectionFromCom -Vbe $excel.VBE
            injected_source = $injectedSource
            injected_source_sha256 = $injectedSourceSha256
            selected_source_sha256 = $selectedSourceSha256
            authority_before_execute = $null
            authority_after_execute = $null
        }

        $compileControl = Get-VbeCompileControl -Vbe $excel.VBE
        if ($null -eq $compileControl) { throw "excel-vba-oracle-worker: VBE compile command ID 578 was not found" }
        $compileCommand = [ordered]@{
            schema = "oxvba.excel-vba-oracle-compile-command.v1"
            id = [int]$compileControl.Id
            caption = [string]$compileControl.Caption
            enabled_before = [bool]$compileControl.Enabled
            enabled_after = $null
        }
        if (-not $compileCommand.enabled_before) { throw "excel-vba-oracle-worker: VBE compile command ID 578 is disabled for the active code pane" }
        $compileOperation = "$($Descriptor.id)-compile"
        $compileControlRecord = Set-GuardianControl -Path $controlFile -CaseId $Descriptor.id -OperationId $compileOperation -Phase compile -AllowDismiss $true
        $compileArm = Wait-GuardianOperationArmed -EventsFile $eventsFile -Control $compileControlRecord -Process $guardian -ReadyRecord $guardianReady
        Assert-GuardianLive -Process $guardian -ReadyRecord $guardianReady -Phase "forced VBE compile after control publication"
        $compileContext.authority_before_execute = Get-CompileAuthoritySnapshot -Vbe $excel.VBE -Project $project -Component $component -ExpectedSourceSha256 $selectedSourceSha256 -Stage "immediately-before-execute"
        $executeException = $null
        $executeReturn = $null
        try { $executeReturn = $compileControl.Execute() }
        catch {
            $executeException = [pscustomobject]@{
                schema = "oxvba.excel-vba-oracle-compile-exception.v1"
                message = $_.Exception.Message
                hresult = "0x$($_.Exception.HResult.ToString('x8'))"
                type = $_.Exception.GetType().FullName
            }
        }
        $compileInvocationCompletedUtc = [DateTime]::UtcNow
        $compileContext.authority_after_execute = Get-CompileAuthoritySnapshot -Vbe $excel.VBE -Project $project -Component $component -ExpectedSourceSha256 $selectedSourceSha256 -Stage "immediately-after-execute"
        $compileEvents = @(Wait-GuardianEventFlush -EventsFile $eventsFile -OperationId $compileOperation -ArmRecord $compileArm -InvocationCompletedUtc $compileInvocationCompletedUtc)
        $compileDialogs = @($compileEvents | Where-Object { [string]$_.event_type -eq "dialog-observation" })
        # Post-dismiss COM state is diagnostic only. It can never repair or satisfy
        # the immutable guardian observation that authorized dismissal.
        $postDismissSelectionDiagnostic = Get-VbeSelectionFromCom -Vbe $excel.VBE
        $compileContext | Add-Member -NotePropertyName selection_after_execute_diagnostic_only -NotePropertyValue $postDismissSelectionDiagnostic
        $compileCommand.enabled_after = [bool]$compileControl.Enabled
        $compileExecution = [pscustomobject]@{
            schema = "oxvba.excel-vba-oracle-compile-execution.v1"
            return_value = if ($null -eq $executeReturn) { $null } else { [string]$executeReturn }
            exception = $executeException
        }
        $compileKinds = @($compileDialogs | Select-Object -ExpandProperty classification)
        if ($compileKinds -contains "security-or-trust" -or $compileKinds -contains "unrecognized-modal") {
            throw "excel-vba-oracle-worker: compile was blocked by a security/trust or unrecognized owned modal"
        }
        if ($executeException) { $compileStatus = "harness-error" }
        elseif ($compileKinds -contains "compile-error") { $compileStatus = "compile-error" }
        elseif (-not [bool]$compileCommand.enabled_after) { $compileStatus = "ok" }
        else { $compileStatus = "no-dialog-unverified" }

        $runtimeMeasurement = Get-VbomRuntimeMeasurement -Project $project -Excel $excel -InvocationEntry $Descriptor.run_procedure -MacroProbeTarget $Descriptor.macro_probe_target -CompileStatus $compileStatus
        if ($compileStatus -eq "ok" -and -not [string]::IsNullOrWhiteSpace([string]$Descriptor.run_procedure)) {
            $runOperation = "$($Descriptor.id)-run"
            $runControlRecord = Set-GuardianControl -Path $controlFile -CaseId $Descriptor.id -OperationId $runOperation -Phase run -AllowDismiss $true
            $runArm = Wait-GuardianOperationArmed -EventsFile $eventsFile -Control $runControlRecord -Process $guardian -ReadyRecord $guardianReady
            Assert-GuardianLive -Process $guardian -ReadyRecord $guardianReady -Phase "runtime invocation after control publication"
            try {
                $qualifiedName = "'$($workbook.Name)'!$($Descriptor.run_procedure)"
                $runValue = $excel.Run($qualifiedName)
                $observationPrefix = [string]$Descriptor.invocation_observation_prefix
                if (-not [string]::IsNullOrWhiteSpace($observationPrefix)) {
                    if ([string]$runValue -like "$observationPrefix*") {
                        $runtimeMeasurement.invocation_entry_observed = $true
                        $runtimeMeasurement.invocation_observation = "case-specific-return-sentinel"
                        $runtimeMeasurement.macros_runnable_entry = $true
                    }
                }
                else {
                    $runtimeMeasurement.invocation_entry_observed = $true
                    $runtimeMeasurement.invocation_observation = "qualified-entry-returned"
                    $runtimeMeasurement.macros_runnable_entry = $true
                }
                if ($Descriptor.id -eq "runtime-full-err") {
                    $runtimeErr = ConvertFrom-ExcelOracleRuntimeErr -Json ([string]$runValue)
                    $runStatus = "runtime-err-captured"
                }
                else {
                    $runStatus = "ok"
                }
            }
            catch {
                $runtimeFailureMessage = $_.Exception.Message
                $macroDisposition = Get-ExcelOracleMacroFailureDisposition `
                    -Message $runtimeFailureMessage `
                    -CompileStatus $compileStatus `
                    -AccessVbom ([bool]$runtimeMeasurement.access_vbom) `
                    -RunnableEntryObserved ([bool]$runtimeMeasurement.invocation_entry_observed) `
                    -TargetExists ([bool]$runtimeMeasurement.macro_probe_target_exists)
                $runStatus = $macroDisposition
            }
            $runInvocationCompletedUtc = [DateTime]::UtcNow
            $runEvents = @(Wait-GuardianEventFlush -EventsFile $eventsFile -OperationId $runOperation -ArmRecord $runArm -InvocationCompletedUtc $runInvocationCompletedUtc)
            if ($Descriptor.id -eq "ambiguous-macro-failure" -and $runStatus -eq "ok") {
                $observationPrefix = [string]$Descriptor.invocation_observation_prefix
                if (-not [bool]$runtimeMeasurement.invocation_entry_observed -or -not ([string]$runValue).StartsWith($observationPrefix, [StringComparison]::Ordinal)) {
                    throw "excel-vba-oracle-worker: ambiguous macro probe did not return its entry-observation sentinel"
                }
                $runtimeFailureMessage = ([string]$runValue).Substring($observationPrefix.Length)
                $macroDisposition = Get-ExcelOracleMacroFailureDisposition `
                    -Message $runtimeFailureMessage `
                    -CompileStatus $compileStatus `
                    -AccessVbom ([bool]$runtimeMeasurement.access_vbom) `
                    -RunnableEntryObserved ([bool]$runtimeMeasurement.invocation_entry_observed) `
                    -TargetExists ([bool]$runtimeMeasurement.macro_probe_target_exists)
                $runStatus = $macroDisposition
            }
            if ($Descriptor.id -eq "runtime-unhandled-modal" -and (Test-RuntimeErrorEvidence -Events $runEvents)) {
                $runtimeMeasurement.invocation_entry_observed = $true
                $runtimeMeasurement.invocation_observation = "owned-runtime-error-modal"
                $runtimeMeasurement.macros_runnable_entry = $true
                $runStatus = "runtime-error-modal"
            }
        }

        $guardianHealthy = $guardian -and -not $guardian.HasExited
        $compileOperationHealthy = Test-GuardianOperationHealthy -Events $compileEvents
        $runOperationHealthy = Test-GuardianOperationHealthy -Events $runEvents
        $compileErrorEvidence = if ($Descriptor.expected_compile_status -eq "compile-error") {
            Test-CompileErrorEvidence -Events $compileEvents -InjectedSource $injectedSource -ExpectedToken ([string]$Descriptor.expected_selected_token) -ExpectedLine ([string]$Descriptor.expected_expanded_line)
        } else { $false }
        $ambiguousMacroEvidence = Test-AmbiguousMacroEvidence -Events $runEvents
        $runtimeErrorEvidence = Test-RuntimeErrorEvidence -Events $runEvents
        $behaviorPassed = $compileStatus -eq $Descriptor.expected_compile_status -and $runStatus -eq $Descriptor.expected_run_status
        if ($behaviorPassed -and $Descriptor.expected_value) { $behaviorPassed = [string]$runValue -eq [string]$Descriptor.expected_value }
        if ($behaviorPassed -and $Descriptor.id -eq "runtime-full-err") {
            $expectedErr = Get-ExcelOracleExpectedRuntimeErr
            foreach ($field in @("number", "source", "description", "help_file", "help_context", "erl")) {
                if (-not (Test-ExcelOracleRuntimeErrFieldEqual -Left $runtimeErr.$field -Right $expectedErr.$field)) { $behaviorPassed = $false }
            }
        }
        if ($behaviorPassed) { $errorMessage = $null }
        elseif ([string]::IsNullOrWhiteSpace($errorMessage)) {
            $errorMessage = if (-not [string]::IsNullOrWhiteSpace($runtimeFailureMessage)) { $runtimeFailureMessage }
                else { "observed compile '$compileStatus' and run '$runStatus' did not satisfy the sealed case contract" }
        }
        $authoritativeEvidencePassed = switch ($Descriptor.id) {
            { $_ -in @("compile-failure", "intrinsic-shadow") } { $compileOperationHealthy -and $compileErrorEvidence; break }
            "ambiguous-macro-failure" { $compileOperationHealthy -and (Test-NoDialogObservations -Events $compileEvents) -and $runOperationHealthy -and $ambiguousMacroEvidence; break }
            "runtime-unhandled-modal" { $compileOperationHealthy -and (Test-NoDialogObservations -Events $compileEvents) -and $runOperationHealthy -and $runtimeErrorEvidence; break }
            default { $compileOperationHealthy -and (Test-NoDialogObservations -Events $compileEvents) -and $runOperationHealthy -and (Test-NoDialogObservations -Events $runEvents); break }
        }
        $passed = $behaviorPassed -and $guardianHealthy -and $authoritativeEvidencePassed
        $evidenceStatus = [pscustomobject]@{
            schema = "oxvba.excel-vba-oracle-evidence-status.v1"
            guardian_healthy_before_cleanup = $guardianHealthy
            compile_operation_healthy = $compileOperationHealthy
            run_operation_healthy = $runOperationHealthy
            compile_error_modal_complete = $compileErrorEvidence
            ambiguous_macro_modal_and_dismissal_complete = $ambiguousMacroEvidence
            runtime_error_modal_and_dismissal_complete = $runtimeErrorEvidence
            authoritative_evidence_passed = $authoritativeEvidencePassed
        }
    }
    catch {
        $errorMessage = $_.Exception.Message
        $passed = $false
        if ($compileStatus -eq "not-run") { $compileStatus = "harness-error" }
    }
    finally {
        if ($guardian) {
            New-Item -ItemType File -Force -Path $stopFile | Out-Null
            if (-not $guardian.WaitForExit(3000)) {
                if ($guardianOwnershipRecord) {
                    $guardianTermination = Invoke-ExcelOracleRetainedProcessTermination -Record $guardianOwnershipRecord -ExpectedProcessName ([string]$guardianOwnershipRecord.process_name) -RunId $RunId
                    if ([string]$guardianTermination.state -eq "same-instance-conflict") { $cleanupAuthorityErrors.Add("guardian retained identity conflict") }
                }
                else { $cleanupAuthorityErrors.Add("guardian cleanup lacks its written ownership record") }
            }
            if ($guardian.HasExited -and $guardianOwnershipRecord -and (Test-Path -LiteralPath $eventsFile)) {
                try {
                    $finalGuardianEvents = @(Get-GuardianEvents -EventsFile $eventsFile)
                    $compileEvents = @($finalGuardianEvents | Where-Object { $_.PSObject.Properties.Name -contains "operation_id" -and [string]$_.operation_id -ceq "$($Descriptor.id)-compile" })
                    $compileDialogs = @($compileEvents | Where-Object { [string]$_.event_type -ceq "dialog-observation" })
                    $runEvents = @($finalGuardianEvents | Where-Object { $_.PSObject.Properties.Name -contains "operation_id" -and [string]$_.operation_id -ceq "$($Descriptor.id)-run" })
                    $finalState = @($finalGuardianEvents | Where-Object { [string]$_.event_type -ceq "guardian-stopped" })
                    $guardianHealthy = $finalState.Count -eq 1 -and [bool]$finalState[0].controlled_stop_observed -and
                        [bool]$finalState[0].excel_identity_live_at_stop -and [string]$finalState[0].exit_reason -ceq "controlled-stop" -and
                        [int]$finalState[0].excel_pid -eq [int]$excelOwnershipRecord.pid -and
                        [int]$finalState[0].guardian_pid -eq [int]$guardianOwnershipRecord.pid -and
                        [string]$finalState[0].process_name -ceq [string]$guardianOwnershipRecord.process_name -and
                        [string]$finalState[0].process_start_utc -ceq [string]$guardianOwnershipRecord.process_start_utc -and
                        [StringComparer]::OrdinalIgnoreCase.Equals([string]$finalState[0].executable_path, [string]$guardianOwnershipRecord.executable_path)
                    if (-not $guardianHealthy) { $cleanupAuthorityErrors.Add("guardian final state is missing, unhealthy, or not bound to its exact process identity") }
                    if ($null -ne $evidenceStatus) {
                        $compileOperationHealthy = Test-GuardianOperationHealthy -Events $compileEvents
                        $runOperationHealthy = Test-GuardianOperationHealthy -Events $runEvents
                        $compileErrorEvidence = if ($Descriptor.expected_compile_status -eq "compile-error") {
                            Test-CompileErrorEvidence -Events $compileEvents -InjectedSource $injectedSource -ExpectedToken ([string]$Descriptor.expected_selected_token) -ExpectedLine ([string]$Descriptor.expected_expanded_line)
                        } else { $false }
                        $ambiguousMacroEvidence = Test-AmbiguousMacroEvidence -Events $runEvents
                        $runtimeErrorEvidence = Test-RuntimeErrorEvidence -Events $runEvents
                        $authoritativeEvidencePassed = switch ($Descriptor.id) {
                            { $_ -in @("compile-failure", "intrinsic-shadow") } { $compileOperationHealthy -and $compileErrorEvidence; break }
                            "ambiguous-macro-failure" { $compileOperationHealthy -and (Test-NoDialogObservations -Events $compileEvents) -and $runOperationHealthy -and $ambiguousMacroEvidence; break }
                            "runtime-unhandled-modal" { $compileOperationHealthy -and (Test-NoDialogObservations -Events $compileEvents) -and $runOperationHealthy -and $runtimeErrorEvidence; break }
                            default { $compileOperationHealthy -and (Test-NoDialogObservations -Events $compileEvents) -and $runOperationHealthy -and (Test-NoDialogObservations -Events $runEvents); break }
                        }
                        $finalAuthorityPassed = $guardianHealthy -and $authoritativeEvidencePassed
                        if ($behaviorPassed -and -not $finalAuthorityPassed -and [string]::IsNullOrWhiteSpace($errorMessage)) {
                            $errorMessage = Get-ExcelOracleEvidenceFailureTransport -Descriptor $Descriptor -GuardianHealthy $guardianHealthy -AuthoritativeEvidencePassed $authoritativeEvidencePassed
                        }
                        $passed = $behaviorPassed -and $finalAuthorityPassed -and [string]::IsNullOrWhiteSpace($errorMessage)
                        $evidenceStatus = [pscustomobject]@{
                            schema = "oxvba.excel-vba-oracle-evidence-status.v1"; guardian_healthy_before_cleanup = $guardianHealthy
                            compile_operation_healthy = $compileOperationHealthy; run_operation_healthy = $runOperationHealthy
                            compile_error_modal_complete = $compileErrorEvidence; ambiguous_macro_modal_and_dismissal_complete = $ambiguousMacroEvidence
                            runtime_error_modal_and_dismissal_complete = $runtimeErrorEvidence; authoritative_evidence_passed = $authoritativeEvidencePassed
                        }
                    }
                }
                catch { $cleanupAuthorityErrors.Add("supervisor-bound final guardian evidence could not be sealed: $($_.Exception.Message)"); $passed = $false }
            }
        }
        if ($workbook) {
            try { $workbook.Close($false) } catch { }
        }
        if ($excel) {
            try { $excel.Quit() } catch { }
        }
        Release-ComObject $compileControl
        Release-ComObject $component
        Release-ComObject $project
        Release-ComObject $workbook
        Release-ComObject $excel
        Release-ComObject $excelNativeWindow
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
        if ($ownedExcelProcess) {
            if (-not $ownedExcelProcess.WaitForExit(2000)) {
                if ($excelOwnershipRecord) {
                    $excelTermination = Invoke-ExcelOracleRetainedProcessTermination -Record $excelOwnershipRecord -ExpectedProcessName "EXCEL" -RunId $RunId
                    if ([string]$excelTermination.state -eq "same-instance-conflict") { $cleanupAuthorityErrors.Add("Excel retained identity conflict") }
                }
                else {
                    # The supervisor-owned kill-on-close Job is the durable
                    # authority for this exact pre-ledger process. Do not invent
                    # a PID-only cleanup record; defer it to Job termination.
                    $cleanupStatus = "job-contained-preownership"
                }
            }
            if ($ownedExcelProcess.HasExited) { $cleanupStatus = "owned-process-zero" }
            elseif ($cleanupStatus -ne "job-contained-preownership") { $cleanupStatus = "owned-process-remains" }
        }
        if ($bootstrapWorkbook) {
            if (-not (Test-ExcelOracleBootstrapWorkbook -Descriptor $bootstrapWorkbook)) {
                $cleanupAuthorityErrors.Add("controlled bootstrap workbook is missing, modified, or has invalid OPC relationship closure after close-without-save")
            }
            elseif (Test-Path -LiteralPath $bootstrapWorkbook.path) {
                $bootstrapSha256After = "sha256:$((Get-FileHash -LiteralPath $bootstrapWorkbook.path -Algorithm SHA256).Hash.ToLowerInvariant())"
            }
        }
        if ($cleanupAuthorityErrors.Count -gt 0) {
            $passed = $false
            $cleanupStatus = "cleanup-authority-error"
            $cleanupText = $cleanupAuthorityErrors -join "; "
            $errorMessage = if ([string]::IsNullOrWhiteSpace($errorMessage)) { $cleanupText } else { "$errorMessage; $cleanupText" }
        }
    }

    return [pscustomobject]@{
        schema = "oxvba.excel-vba-oracle-case-result.v1"
        id = $Descriptor.id
        purpose = $Descriptor.purpose
        passed = $passed
        owned_excel_pid = if ($excelOwnershipRecord) { [int]$excelOwnershipRecord.pid } else { $null }
        observed_excel_pid = $excelPid
        excel_ownership_recorded = $null -ne $excelOwnershipRecord
        preownership_failure_phase = if ($null -eq $excelOwnershipRecord) { $preOwnershipFailurePhase } else { $null }
        selected_case_descriptor_sha256 = [string]$Descriptor.descriptor_sha256
        module_name = [string]$Descriptor.module_name
        module_path = $modulePath
        module_sha256 = [string]$Descriptor.module_sha256
        case_diagnostic_only = [bool]$Descriptor.diagnostic_only
        evidence_contract = [string]$Descriptor.evidence_contract
        compile_status = $compileStatus
        expected_compile_status = $Descriptor.expected_compile_status
        compile_command = $compileCommand
        compile_execution = $compileExecution
        compile_context = $compileContext
        post_dismiss_selection_diagnostic_only = $postDismissSelectionDiagnostic
        compile_dialogs = @($compileDialogs)
        compile_window_observations = @($compileEvents)
        run_procedure = $Descriptor.run_procedure
        run_status = $runStatus
        expected_run_status = $Descriptor.expected_run_status
        run_value = if ($null -eq $runValue) { $null } else { [string]$runValue }
        runtime_err = $runtimeErr
        macro_failure_disposition = $macroDisposition
        runtime_measurement = $runtimeMeasurement
        transport_error = $errorMessage
        run_dialogs = @($runEvents)
        evidence_status = $evidenceStatus
        cleanup_status = $cleanupStatus
        cleanup_authority_errors = @($cleanupAuthorityErrors)
        bootstrap_workbook = if ($bootstrapWorkbook) {
            [ordered]@{
                schema = $bootstrapWorkbook.schema
                path = $bootstrapWorkbook.path
                sha256 = $bootstrapWorkbook.sha256
                sha256_after = $bootstrapSha256After
                package_parts = @($bootstrapWorkbook.package_parts)
                macro_free = [bool]$bootstrapWorkbook.macro_free
            }
        } else { $null }
        defect_declaration = if ($Descriptor.id -eq "intrinsic-shadow") { "Public Function Shadowed(ByVal Fix As Double) As Double" } else { $null }
    }
}

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$ownershipParent = Split-Path -Parent $OwnershipFile
if ($ownershipParent) { New-Item -ItemType Directory -Force -Path $ownershipParent | Out-Null }
$helperOwnershipParent = Split-Path -Parent $HelperOwnershipFile
if ($helperOwnershipParent) { New-Item -ItemType Directory -Force -Path $helperOwnershipParent | Out-Null }

$descriptorEnvelope = Read-ExcelOracleSelectedCaseDescriptorEnvelope -Path $SelectedCaseDescriptorFile -ExpectedAggregateSha256 $SelectedCaseDescriptorDigest
$selectedCaseDescriptors = @($descriptorEnvelope.descriptors)
$diagnosticOnly = $selectedCaseDescriptors.Count -eq 1 -and [bool]$selectedCaseDescriptors[0].diagnostic_only
if (($diagnosticOnly -and @($selectedCaseDescriptors | Where-Object { -not [bool]$_.diagnostic_only }).Count -gt 0) -or
    (-not $diagnosticOnly -and @($selectedCaseDescriptors | Where-Object { [bool]$_.diagnostic_only }).Count -gt 0)) {
    throw "excel-vba-oracle-worker: selected descriptor sequence mixes diagnostic and ordinary cases"
}
$script:SelectedCaseIds = @($selectedCaseDescriptors | ForEach-Object { [string]$_.id })
$containmentAuthority = Wait-ContainmentAuthority
$script:ExcelExecutablePath = Get-ExcelExecutablePath
$script:GuardianControlSequence = 0L
$results = [Collections.Generic.List[object]]::new()
foreach ($descriptor in $selectedCaseDescriptors) {
    $caseResult = Invoke-HarnessCase -Descriptor $descriptor
    $results.Add($caseResult)
    if (Test-ExcelOracleShouldStopAfterCase -CaseResult $caseResult) {
        # A pre-ownership infrastructure failure (bootstrap, exact attachment,
        # or authority setup) is common to the remaining cases. Do not multiply
        # owned Excel launches after that boundary has already failed closed.
        break
    }
}

$document = [ordered]@{
    schema = "oxvba.excel-vba-oracle-results.v1"
    run_id = $RunId
    generated_utc = [DateTime]::UtcNow.ToString("o")
    worker_pid = $PID
    containment_token = $ContainmentToken
    containment_authority = $containmentAuthority
    selected_case_descriptor_digest = [string]$descriptorEnvelope.aggregate_sha256
    diagnostic_only = $diagnosticOnly
    cases = @($results)
    passed = @($results | Where-Object { -not $_.passed }).Count -eq 0
}
$document | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath (Join-Path $OutputDirectory "results.json") -Encoding utf8NoBOM
if (-not $document.passed) { exit 1 }
