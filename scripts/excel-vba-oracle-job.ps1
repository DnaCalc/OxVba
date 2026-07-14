Set-StrictMode -Version Latest

if (-not ([Management.Automation.PSTypeName]'ExcelOracleJob').Type) {
    Add-Type @'
using System;
using System.ComponentModel;
using System.Diagnostics;
using Microsoft.Win32.SafeHandles;
using System.Runtime.InteropServices;
using System.Text;

public sealed class ExcelOracleJob : IDisposable
{
    private IntPtr handle;

    [StructLayout(LayoutKind.Sequential)]
    private struct IO_COUNTERS
    {
        public ulong ReadOperationCount, WriteOperationCount, OtherOperationCount;
        public ulong ReadTransferCount, WriteTransferCount, OtherTransferCount;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct JOBOBJECT_BASIC_LIMIT_INFORMATION
    {
        public long PerProcessUserTimeLimit;
        public long PerJobUserTimeLimit;
        public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize;
        public UIntPtr MaximumWorkingSetSize;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass;
        public uint SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION
    {
        public JOBOBJECT_BASIC_LIMIT_INFORMATION BasicLimitInformation;
        public IO_COUNTERS IoInfo;
        public UIntPtr ProcessMemoryLimit;
        public UIntPtr JobMemoryLimit;
        public UIntPtr PeakProcessMemoryUsed;
        public UIntPtr PeakJobMemoryUsed;
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr CreateJobObject(IntPtr attributes, string name);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool SetInformationJobObject(IntPtr job, int infoClass, IntPtr info, uint length);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateJobObject(IntPtr job, uint exitCode);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool IsProcessInJob(IntPtr process, IntPtr job, out bool result);

    [DllImport("kernel32.dll")]
    private static extern bool CloseHandle(IntPtr handle);

    private const uint JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x00002000;
    private const int JobObjectExtendedLimitInformation = 9;

    public ExcelOracleJob(string name)
    {
        handle = CreateJobObject(IntPtr.Zero, name);
        if (handle == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error(), "CreateJobObject failed");

        var limits = new JOBOBJECT_EXTENDED_LIMIT_INFORMATION();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        int size = Marshal.SizeOf<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>();
        IntPtr buffer = Marshal.AllocHGlobal(size);
        try
        {
            Marshal.StructureToPtr(limits, buffer, false);
            if (!SetInformationJobObject(handle, JobObjectExtendedLimitInformation, buffer, (uint)size))
                throw new Win32Exception(Marshal.GetLastWin32Error(), "SetInformationJobObject failed");
        }
        catch
        {
            CloseHandle(handle);
            handle = IntPtr.Zero;
            throw;
        }
        finally { Marshal.FreeHGlobal(buffer); }
    }

    public void AssignProcess(IntPtr processHandle)
    {
        if (handle == IntPtr.Zero) throw new ObjectDisposedException(nameof(ExcelOracleJob));
        if (!AssignProcessToJobObject(handle, processHandle))
            throw new Win32Exception(Marshal.GetLastWin32Error(), "AssignProcessToJobObject failed");
    }

    public bool ContainsProcess(IntPtr processHandle)
    {
        if (handle == IntPtr.Zero) throw new ObjectDisposedException(nameof(ExcelOracleJob));
        bool result;
        if (!IsProcessInJob(processHandle, handle, out result))
            throw new Win32Exception(Marshal.GetLastWin32Error(), "IsProcessInJob failed");
        return result;
    }

    public void Terminate()
    {
        if (handle == IntPtr.Zero) throw new ObjectDisposedException(nameof(ExcelOracleJob));
        if (!TerminateJobObject(handle, 1))
            throw new Win32Exception(Marshal.GetLastWin32Error(), "TerminateJobObject failed");
    }

    public void Dispose()
    {
        if (handle != IntPtr.Zero)
        {
            CloseHandle(handle);
            handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }

    ~ExcelOracleJob() { Dispose(); }
}

public sealed class ExcelOracleRetainedProcess : IDisposable
{
    private const uint PROCESS_TERMINATE = 0x0001;
    private const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
    private const uint SYNCHRONIZE = 0x00100000;
    private const uint WAIT_OBJECT_0 = 0;
    private const uint WAIT_TIMEOUT = 258;

    private SafeProcessHandle handle;

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern SafeProcessHandle OpenProcess(uint access, bool inheritHandle, int processId);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool QueryFullProcessImageName(SafeProcessHandle process, uint flags, StringBuilder path, ref uint size);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetProcessTimes(SafeProcessHandle process, out long creation, out long exit, out long kernel, out long user);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateProcess(SafeProcessHandle process, uint exitCode);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern uint WaitForSingleObject(SafeProcessHandle process, uint milliseconds);

    private ExcelOracleRetainedProcess(SafeProcessHandle retainedHandle)
    {
        handle = retainedHandle;
    }

    public static ExcelOracleRetainedProcess Open(int processId)
    {
        SafeProcessHandle retained = OpenProcess(PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, false, processId);
        if (retained == null || retained.IsInvalid)
        {
            int error = Marshal.GetLastWin32Error();
            if (retained != null) retained.Dispose();
            if (error == 87 || error == 1168) return null;
            throw new Win32Exception(error, "OpenProcess failed");
        }
        return new ExcelOracleRetainedProcess(retained);
    }

    public string ExecutablePath
    {
        get
        {
            var path = new StringBuilder(32768);
            uint size = (uint)path.Capacity;
            if (!QueryFullProcessImageName(handle, 0, path, ref size))
                throw new Win32Exception(Marshal.GetLastWin32Error(), "QueryFullProcessImageName failed");
            return path.ToString();
        }
    }

    public DateTime StartTimeUtc
    {
        get
        {
            long creation, exit, kernel, user;
            if (!GetProcessTimes(handle, out creation, out exit, out kernel, out user))
                throw new Win32Exception(Marshal.GetLastWin32Error(), "GetProcessTimes failed");
            return DateTime.FromFileTimeUtc(creation);
        }
    }

    public void TerminateAndWait(int milliseconds)
    {
        if (!TerminateProcess(handle, 1))
        {
            int error = Marshal.GetLastWin32Error();
            if (error != 5) throw new Win32Exception(error, "TerminateProcess failed");
        }
        uint result = WaitForSingleObject(handle, checked((uint)milliseconds));
        if (result == WAIT_TIMEOUT) throw new TimeoutException("retained process did not terminate before timeout");
        if (result != WAIT_OBJECT_0) throw new Win32Exception(Marshal.GetLastWin32Error(), "WaitForSingleObject failed");
    }

    public void Dispose()
    {
        if (handle != null) { handle.Dispose(); handle = null; }
        GC.SuppressFinalize(this);
    }
}
'@
}

function Get-ExcelOracleRetainedProcessIdentityState {
    param(
        [Parameter(Mandatory = $true)]$Record,
        [Parameter(Mandatory = $true)][string]$ExpectedProcessName,
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)][ExcelOracleRetainedProcess]$RetainedProcess
    )

    foreach ($field in @("run_id", "pid", "process_name", "process_start_utc", "executable_path")) {
        if ($Record.PSObject.Properties.Name -notcontains $field -or [string]::IsNullOrWhiteSpace([string]$Record.$field)) {
            return "same-instance-conflict"
        }
    }
    try {
        $recordedStart = [DateTime]::Parse(
            [string]$Record.process_start_utc,
            [Globalization.CultureInfo]::InvariantCulture,
            [Globalization.DateTimeStyles]::RoundtripKind
        ).ToUniversalTime()
        $actualStart = $RetainedProcess.StartTimeUtc
        if ($recordedStart.ToFileTimeUtc() -ne $actualStart.ToFileTimeUtc()) { return "pid-reused" }
        $recordedPath = [IO.Path]::GetFullPath([string]$Record.executable_path)
        $actualPath = [IO.Path]::GetFullPath($RetainedProcess.ExecutablePath)
        $actualName = [IO.Path]::GetFileNameWithoutExtension($actualPath)
        if ([string]$Record.run_id -ne $RunId -or
            -not [StringComparer]::OrdinalIgnoreCase.Equals([string]$Record.process_name, $ExpectedProcessName) -or
            -not [StringComparer]::OrdinalIgnoreCase.Equals($actualName, $ExpectedProcessName) -or
            -not [StringComparer]::OrdinalIgnoreCase.Equals($recordedPath, $actualPath)) {
            return "same-instance-conflict"
        }
        return "exact"
    }
    catch { return "same-instance-conflict" }
}

function Invoke-ExcelOracleRetainedProcessTermination {
    param(
        [Parameter(Mandatory = $true)]$Record,
        [Parameter(Mandatory = $true)][string]$ExpectedProcessName,
        [Parameter(Mandatory = $true)][string]$RunId,
        [ValidateRange(1, 60000)][int]$TimeoutMilliseconds = 5000
    )

    $retained = $null
    try {
        $retained = [ExcelOracleRetainedProcess]::Open([int]$Record.pid)
        if ($null -eq $retained) { return [pscustomobject]@{ state = "missing"; terminated = $false } }
        # Identity query, termination, and wait deliberately use this one retained
        # SafeProcessHandle. A PID cannot be rebound between authority and action.
        $state = Get-ExcelOracleRetainedProcessIdentityState -Record $Record -ExpectedProcessName $ExpectedProcessName -RunId $RunId -RetainedProcess $retained
        if ($state -eq "exact") {
            $retained.TerminateAndWait($TimeoutMilliseconds)
            return [pscustomobject]@{ state = $state; terminated = $true }
        }
        return [pscustomobject]@{ state = $state; terminated = $false }
    }
    finally {
        if ($null -ne $retained) { $retained.Dispose() }
    }
}

function Start-ExcelOracleContainedProcess {
    param(
        [Parameter(Mandatory = $true)][string]$JobName,
        [Parameter(Mandatory = $true)][string]$RunId,
        [Parameter(Mandatory = $true)][scriptblock]$StartProcess
    )
    $job = $null
    $process = $null
    try {
        $job = [ExcelOracleJob]::new($JobName)
        $process = & $StartProcess
        if ($process -isnot [Diagnostics.Process]) {
            throw "process start callback did not return a Diagnostics.Process"
        }
        $job.AssignProcess($process.Handle)
        if (-not $job.ContainsProcess($process.Handle)) {
            throw "started process is not a member of the kill-on-close Job after assignment"
        }
        return [pscustomobject]@{ job = $job; process = $process }
    }
    catch {
        $failure = $_.Exception.Message
        if ($null -ne $job) {
            try { $job.Terminate() } catch { $failure = "$failure; Job termination failed: $($_.Exception.Message)" }
            finally { $job.Dispose() }
        }
        if ($process -is [Diagnostics.Process] -and -not $process.HasExited) {
            try {
                $record = [pscustomobject]@{
                    run_id = $RunId
                    pid = $process.Id
                    process_name = [string]$process.ProcessName
                    process_start_utc = $process.StartTime.ToUniversalTime().ToString("o")
                    executable_path = [string]$process.Path
                }
                [void](Invoke-ExcelOracleRetainedProcessTermination -Record $record -ExpectedProcessName ([string]$process.ProcessName) -RunId $RunId -TimeoutMilliseconds 10000)
            }
            catch { $failure = "$failure; retained process cleanup failed: $($_.Exception.Message)" }
        }
        throw "excel-vba-oracle-job: contained process start failed deterministically: $failure"
    }
}
