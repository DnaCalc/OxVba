using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using Microsoft.Win32.SafeHandles;

public sealed class OxVbaCoreGateWindowsJob : IDisposable
{
    private const uint CREATE_SUSPENDED = 0x00000004;
    private const uint CREATE_NO_WINDOW = 0x08000000;
    private const uint CREATE_UNICODE_ENVIRONMENT = 0x00000400;
    private const uint EXTENDED_STARTUPINFO_PRESENT = 0x00080000;
    private const uint STARTF_USESTDHANDLES = 0x00000100;
    private const int PROC_THREAD_ATTRIBUTE_HANDLE_LIST = 0x00020002;
    private const uint GENERIC_READ = 0x80000000;
    private const uint GENERIC_WRITE = 0x40000000;
    private const uint FILE_SHARE_READ = 0x00000001;
    private const uint FILE_SHARE_WRITE = 0x00000002;
    private const uint FILE_SHARE_DELETE = 0x00000004;
    private const uint CREATE_ALWAYS = 2;
    private const uint OPEN_EXISTING = 3;
    private const uint FILE_ATTRIBUTE_NORMAL = 0x00000080;
    private const uint HANDLE_FLAG_INHERIT = 0x00000001;
    private const uint JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x00002000;
    private const uint WAIT_OBJECT_0 = 0;
    private const uint WAIT_TIMEOUT = 258;

    private IntPtr _job;
    private IntPtr _process;
    private IntPtr _stdout;
    private IntPtr _stderr;
    private IntPtr _stdin;
    private IntPtr _sentinel;
    private bool _disposed;

    public int ProcessId { get; private set; }

    private OxVbaCoreGateWindowsJob() { }

    public static OxVbaCoreGateWindowsJob Start(
        string executable,
        string[] arguments,
        string workingDirectory,
        IDictionary<string, string> environment,
        string stdoutPath,
        string stderrPath)
    {
        if (!RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
            throw new PlatformNotSupportedException("Windows Job Objects require Windows");

        var owner = new OxVbaCoreGateWindowsJob();
        IntPtr thread = IntPtr.Zero;
        IntPtr environmentBlock = IntPtr.Zero;
        IntPtr attributeList = IntPtr.Zero;
        IntPtr inheritedHandles = IntPtr.Zero;
        try
        {
            owner._job = CreateJobObjectW(IntPtr.Zero, null);
            ThrowIfInvalid(owner._job, "CreateJobObjectW");
            var limits = new JOBOBJECT_EXTENDED_LIMIT_INFORMATION();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            int limitsLength = Marshal.SizeOf<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>();
            IntPtr limitsPointer = Marshal.AllocHGlobal(limitsLength);
            try
            {
                Marshal.StructureToPtr(limits, limitsPointer, false);
                if (!SetInformationJobObject(owner._job, 9, limitsPointer, (uint)limitsLength))
                    ThrowLastError("SetInformationJobObject");
            }
            finally { Marshal.FreeHGlobal(limitsPointer); }

            var security = new SECURITY_ATTRIBUTES
            {
                nLength = Marshal.SizeOf<SECURITY_ATTRIBUTES>(),
                bInheritHandle = true,
                lpSecurityDescriptor = IntPtr.Zero
            };
            owner._stdout = CreateFileW(stdoutPath, GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                ref security, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, IntPtr.Zero);
            ThrowIfInvalid(owner._stdout, "CreateFileW(stdout)");
            owner._stderr = CreateFileW(stderrPath, GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                ref security, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, IntPtr.Zero);
            ThrowIfInvalid(owner._stderr, "CreateFileW(stderr)");
            owner._stdin = CreateFileW("NUL", GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE,
                ref security, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, IntPtr.Zero);
            ThrowIfInvalid(owner._stdin, "CreateFileW(NUL)");

            // The test sentinel is intentionally inheritable but deliberately
            // excluded from PROC_THREAD_ATTRIBUTE_HANDLE_LIST. It proves that
            // bInheritHandles=true cannot leak ambient inheritable handles.
            var childEnvironment = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (var pair in environment) childEnvironment[pair.Key] = pair.Value;
            string sentinelRequested = Environment.GetEnvironmentVariable("OXVBA_CORE_GATE_TEST_SENTINEL");
            if (string.Equals(sentinelRequested, "1", StringComparison.Ordinal))
            {
                owner._sentinel = CreateEventW(ref security, true, false, null);
                ThrowIfInvalid(owner._sentinel, "CreateEventW(inheritance sentinel)");
                childEnvironment["OXVBA_CORE_GATE_TEST_SENTINEL_HANDLE"] =
                    owner._sentinel.ToInt64().ToString(CultureInfo.InvariantCulture);
            }

            var startup = new STARTUPINFOEX();
            startup.StartupInfo.cb = Marshal.SizeOf<STARTUPINFOEX>();
            startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
            startup.StartupInfo.hStdInput = owner._stdin;
            startup.StartupInfo.hStdOutput = owner._stdout;
            startup.StartupInfo.hStdError = owner._stderr;

            UIntPtr attributeBytes = UIntPtr.Zero;
            InitializeProcThreadAttributeList(IntPtr.Zero, 1, 0, ref attributeBytes);
            int initializationError = Marshal.GetLastWin32Error();
            if (attributeBytes == UIntPtr.Zero || (initializationError != 0 && initializationError != 122))
                ThrowLastError("InitializeProcThreadAttributeList(size)");
            attributeList = Marshal.AllocHGlobal(checked((int)attributeBytes.ToUInt64()));
            if (!InitializeProcThreadAttributeList(attributeList, 1, 0, ref attributeBytes))
                ThrowLastError("InitializeProcThreadAttributeList");
            startup.lpAttributeList = attributeList;

            inheritedHandles = Marshal.AllocHGlobal(checked(IntPtr.Size * 3));
            Marshal.WriteIntPtr(inheritedHandles, 0, owner._stdin);
            Marshal.WriteIntPtr(inheritedHandles, IntPtr.Size, owner._stdout);
            Marshal.WriteIntPtr(inheritedHandles, IntPtr.Size * 2, owner._stderr);
            if (!UpdateProcThreadAttribute(attributeList, 0,
                new IntPtr(PROC_THREAD_ATTRIBUTE_HANDLE_LIST), inheritedHandles,
                new UIntPtr(checked((uint)(IntPtr.Size * 3))), IntPtr.Zero, IntPtr.Zero))
                ThrowLastError("UpdateProcThreadAttribute(handle list)");

            string commandLine = QuoteWindowsArgument(executable) +
                (arguments.Length == 0 ? "" : " " + string.Join(" ", arguments.Select(QuoteWindowsArgument)));
            environmentBlock = Marshal.StringToHGlobalUni(BuildEnvironmentBlock(childEnvironment));
            PROCESS_INFORMATION processInfo;
            bool created = CreateProcessW(
                executable,
                new StringBuilder(commandLine),
                IntPtr.Zero,
                IntPtr.Zero,
                true,
                CREATE_SUSPENDED | CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
                environmentBlock,
                workingDirectory,
                ref startup,
                out processInfo);
            if (!created) ThrowLastError("CreateProcessW");
            owner._process = processInfo.hProcess;
            thread = processInfo.hThread;
            owner.ProcessId = checked((int)processInfo.dwProcessId);

            if (!AssignProcessToJobObject(owner._job, owner._process))
                ThrowLastError("AssignProcessToJobObject");
            if (ResumeThread(thread) == uint.MaxValue)
                ThrowLastError("ResumeThread");
            CloseHandle(thread);
            thread = IntPtr.Zero;
            return owner;
        }
        catch
        {
            if (owner._process != IntPtr.Zero && owner._process != new IntPtr(-1))
                TerminateProcess(owner._process, 127);
            owner.Dispose();
            throw;
        }
        finally
        {
            if (thread != IntPtr.Zero) CloseHandle(thread);
            if (environmentBlock != IntPtr.Zero) Marshal.FreeHGlobal(environmentBlock);
            if (attributeList != IntPtr.Zero)
            {
                DeleteProcThreadAttributeList(attributeList);
                Marshal.FreeHGlobal(attributeList);
            }
            if (inheritedHandles != IntPtr.Zero) Marshal.FreeHGlobal(inheritedHandles);
        }
    }

    public bool DirectExited
    {
        get
        {
            EnsureLive();
            uint result = WaitForSingleObject(_process, 0);
            if (result == WAIT_OBJECT_0) return true;
            if (result == WAIT_TIMEOUT) return false;
            ThrowLastError("WaitForSingleObject");
            return false;
        }
    }

    public uint ActiveProcesses
    {
        get
        {
            EnsureLive();
            int length = Marshal.SizeOf<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>();
            IntPtr pointer = Marshal.AllocHGlobal(length);
            try
            {
                if (!QueryInformationJobObject(_job, 1, pointer, (uint)length, IntPtr.Zero))
                    ThrowLastError("QueryInformationJobObject");
                return Marshal.PtrToStructure<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>(pointer).ActiveProcesses;
            }
            finally { Marshal.FreeHGlobal(pointer); }
        }
    }

    public int ExitCode
    {
        get
        {
            EnsureLive();
            if (!GetExitCodeProcess(_process, out uint code)) ThrowLastError("GetExitCodeProcess");
            return unchecked((int)code);
        }
    }

    public bool TestSentinelWasSignaled
    {
        get
        {
            EnsureLive();
            if (_sentinel == IntPtr.Zero || _sentinel == new IntPtr(-1)) return false;
            uint result = WaitForSingleObject(_sentinel, 0);
            if (result == WAIT_OBJECT_0) return true;
            if (result == WAIT_TIMEOUT) return false;
            ThrowLastError("WaitForSingleObject(test sentinel)");
            return false;
        }
    }

    public void Terminate(uint exitCode)
    {
        EnsureLive();
        if (!TerminateJobObject(_job, exitCode)) ThrowLastError("TerminateJobObject");
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Close(ref _job);
        Close(ref _process);
        Close(ref _stdout);
        Close(ref _stderr);
        Close(ref _stdin);
        Close(ref _sentinel);
    }

    private void EnsureLive()
    {
        if (_disposed) throw new ObjectDisposedException(nameof(OxVbaCoreGateWindowsJob));
    }

    private static void Close(ref IntPtr handle)
    {
        if (handle != IntPtr.Zero && handle != new IntPtr(-1)) CloseHandle(handle);
        handle = IntPtr.Zero;
    }

    private static string BuildEnvironmentBlock(IDictionary<string, string> environment)
    {
        var builder = new StringBuilder();
        foreach (var pair in environment.OrderBy(p => p.Key, StringComparer.OrdinalIgnoreCase))
        {
            builder.Append(pair.Key).Append('=').Append(pair.Value).Append('\0');
        }
        builder.Append('\0');
        return builder.ToString();
    }

    private static string QuoteWindowsArgument(string value)
    {
        if (value.Length > 0 && value.All(c => !char.IsWhiteSpace(c) && c != '"')) return value;
        var result = new StringBuilder("\"");
        int backslashes = 0;
        foreach (char character in value)
        {
            if (character == '\\') { backslashes++; continue; }
            if (character == '"')
            {
                result.Append('\\', backslashes * 2 + 1).Append('"');
                backslashes = 0;
                continue;
            }
            result.Append('\\', backslashes).Append(character);
            backslashes = 0;
        }
        result.Append('\\', backslashes * 2).Append('"');
        return result.ToString();
    }

    private static void ThrowIfInvalid(IntPtr handle, string operation)
    {
        if (handle == IntPtr.Zero || handle == new IntPtr(-1)) ThrowLastError(operation);
    }

    private static void ThrowLastError(string operation)
    {
        throw new Win32Exception(Marshal.GetLastWin32Error(), operation);
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct SECURITY_ATTRIBUTES
    {
        public int nLength;
        public IntPtr lpSecurityDescriptor;
        [MarshalAs(UnmanagedType.Bool)] public bool bInheritHandle;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct STARTUPINFO
    {
        public int cb;
        public string lpReserved;
        public string lpDesktop;
        public string lpTitle;
        public int dwX;
        public int dwY;
        public int dwXSize;
        public int dwYSize;
        public int dwXCountChars;
        public int dwYCountChars;
        public int dwFillAttribute;
        public uint dwFlags;
        public short wShowWindow;
        public short cbReserved2;
        public IntPtr lpReserved2;
        public IntPtr hStdInput;
        public IntPtr hStdOutput;
        public IntPtr hStdError;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct STARTUPINFOEX
    {
        public STARTUPINFO StartupInfo;
        public IntPtr lpAttributeList;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct PROCESS_INFORMATION
    {
        public IntPtr hProcess;
        public IntPtr hThread;
        public uint dwProcessId;
        public uint dwThreadId;
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
    private struct IO_COUNTERS
    {
        public ulong ReadOperationCount;
        public ulong WriteOperationCount;
        public ulong OtherOperationCount;
        public ulong ReadTransferCount;
        public ulong WriteTransferCount;
        public ulong OtherTransferCount;
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

    [StructLayout(LayoutKind.Sequential)]
    private struct JOBOBJECT_BASIC_ACCOUNTING_INFORMATION
    {
        public long TotalUserTime;
        public long TotalKernelTime;
        public long ThisPeriodTotalUserTime;
        public long ThisPeriodTotalKernelTime;
        public uint TotalPageFaultCount;
        public uint TotalProcesses;
        public uint ActiveProcesses;
        public uint TotalTerminatedProcesses;
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr CreateJobObjectW(IntPtr attributes, string name);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool SetInformationJobObject(IntPtr job, int infoClass, IntPtr info, uint length);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool QueryInformationJobObject(IntPtr job, int infoClass, IntPtr info, uint length, IntPtr returnLength);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool TerminateJobObject(IntPtr job, uint exitCode);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool TerminateProcess(IntPtr process, uint exitCode);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool CreateProcessW(
        string applicationName,
        StringBuilder commandLine,
        IntPtr processAttributes,
        IntPtr threadAttributes,
        [MarshalAs(UnmanagedType.Bool)] bool inheritHandles,
        uint creationFlags,
        IntPtr environment,
        string currentDirectory,
        ref STARTUPINFOEX startupInfo,
        out PROCESS_INFORMATION processInformation);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool InitializeProcThreadAttributeList(
        IntPtr attributeList,
        int attributeCount,
        uint flags,
        ref UIntPtr size);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool UpdateProcThreadAttribute(
        IntPtr attributeList,
        uint flags,
        IntPtr attribute,
        IntPtr value,
        UIntPtr size,
        IntPtr previousValue,
        IntPtr returnSize);

    [DllImport("kernel32.dll")]
    private static extern void DeleteProcThreadAttributeList(IntPtr attributeList);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr CreateFileW(string fileName, uint access, uint share, ref SECURITY_ATTRIBUTES security,
        uint creation, uint attributes, IntPtr template);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr CreateEventW(ref SECURITY_ATTRIBUTES security, [MarshalAs(UnmanagedType.Bool)] bool manualReset,
        [MarshalAs(UnmanagedType.Bool)] bool initialState, string name);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern uint ResumeThread(IntPtr thread);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetExitCodeProcess(IntPtr process, out uint exitCode);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool CloseHandle(IntPtr handle);
}

public static class OxVbaCoreGatePosix
{
    private const int PrSetChildSubreaper = 36;
    private const int PrGetChildSubreaper = 37;
    private const long SysPidfdSendSignalX64 = 424;
    private const long SysPidfdOpenX64 = 434;
    public const int SignalZero = 0;
    public const int SignalStop = 19;
    public const int SignalKill = 9;
    private const int WaitNoHang = 1;

    public static int CurrentProcessId
    {
        get { return getpid(); }
    }

    public static void EnableChildSubreaper()
    {
        if (!RuntimeInformation.IsOSPlatform(OSPlatform.Linux))
            throw new PlatformNotSupportedException("Linux subreaper containment requires Linux");
        if (prctl_value(PrSetChildSubreaper, new UIntPtr(1), UIntPtr.Zero, UIntPtr.Zero, UIntPtr.Zero) != 0)
            throw new Win32Exception(Marshal.GetLastWin32Error(), "prctl(PR_SET_CHILD_SUBREAPER)");
        int enabled;
        if (prctl_pointer(PrGetChildSubreaper, out enabled, UIntPtr.Zero, UIntPtr.Zero, UIntPtr.Zero) != 0)
            throw new Win32Exception(Marshal.GetLastWin32Error(), "prctl(PR_GET_CHILD_SUBREAPER)");
        if (enabled != 1) throw new InvalidOperationException("Linux child subreaper did not remain enabled");
    }

    public static int TryOpenPidFd(int processId)
    {
        if (RuntimeInformation.ProcessArchitecture != Architecture.X64)
            throw new PlatformNotSupportedException("Linux pidfd containment is x64-only");
        long result = syscall_pidfd_open(SysPidfdOpenX64, processId, 0);
        if (result < 0)
        {
            int error = Marshal.GetLastWin32Error();
            if (error == 3) return -1; // ordinary exit between discovery and retain
            throw new Win32Exception(error, "pidfd_open");
        }
        return checked((int)result);
    }

    public static bool SignalPidFd(int pidFd, int signal)
    {
        long result = syscall_pidfd_send_signal(SysPidfdSendSignalX64, pidFd, signal, IntPtr.Zero, 0);
        if (result == 0) return true;
        int error = Marshal.GetLastWin32Error();
        if (error == 3) return false; // exact task already exited
        throw new Win32Exception(error, "pidfd_send_signal");
    }

    public static void ClosePidFd(int pidFd)
    {
        if (pidFd >= 0 && close(pidFd) != 0)
            throw new Win32Exception(Marshal.GetLastWin32Error(), "close(pidfd)");
    }

    public static bool ReapOwnedProcess(int processId)
    {
        int status;
        int result = waitpid(processId, out status, WaitNoHang);
        if (result == processId) return true;
        if (result == 0) return false;
        int error = Marshal.GetLastWin32Error();
        if (error == 3 || error == 10) return false; // ESRCH or ECHILD
        throw new Win32Exception(error, "waitpid(owned process, WNOHANG)");
    }

    [DllImport("libc", SetLastError = true)]
    private static extern int getpid();

    [DllImport("libc", SetLastError = true)]
    private static extern int close(int fd);

    [DllImport("libc", SetLastError = true)]
    private static extern int waitpid(int pid, out int status, int options);

    [DllImport("libc", EntryPoint = "prctl", SetLastError = true)]
    private static extern int prctl_value(int option, UIntPtr arg2, UIntPtr arg3, UIntPtr arg4, UIntPtr arg5);

    [DllImport("libc", EntryPoint = "prctl", SetLastError = true)]
    private static extern int prctl_pointer(int option, out int arg2, UIntPtr arg3, UIntPtr arg4, UIntPtr arg5);

    [DllImport("libc", EntryPoint = "syscall", SetLastError = true)]
    private static extern long syscall_pidfd_open(long number, int pid, uint flags);

    [DllImport("libc", EntryPoint = "syscall", SetLastError = true)]
    private static extern long syscall_pidfd_send_signal(long number, int pidFd, int signal, IntPtr info, uint flags);
}

/// <summary>
/// Linux gate containment that survives setsid/double-fork daemonization.
/// The PowerShell runner is first made a child subreaper. Processes that leave
/// the launcher's session are consequently adopted by the runner instead of
/// PID 1. Every admitted process is retained through a pidfd after its
/// ancestry and /proc start-time identity are revalidated. Signals use only
/// those stable kernel handles, never a numeric PID or process-group ID.
/// </summary>
public sealed class OxVbaCoreGatePosixOwnedTree : IDisposable
{
    private sealed class ProcRecord
    {
        public int Pid;
        public int ParentPid;
        public int ProcessGroup;
        public int Session;
        public ulong StartTicks;
        public char State;

        public string Identity
        {
            get { return Pid.ToString(CultureInfo.InvariantCulture) + ":" + StartTicks.ToString(CultureInfo.InvariantCulture); }
        }
    }

    private sealed class OwnedRecord : IDisposable
    {
        public int Pid;
        public int ParentPid;
        public ulong StartTicks;
        public int PidFd = -1;

        public string Identity
        {
            get { return Pid.ToString(CultureInfo.InvariantCulture) + ":" + StartTicks.ToString(CultureInfo.InvariantCulture); }
        }

        public void Dispose()
        {
            if (PidFd < 0) return;
            int owned = PidFd;
            PidFd = -1;
            OxVbaCoreGatePosix.ClosePidFd(owned);
        }
    }

    private readonly int _runnerPid;
    private readonly HashSet<string> _baseline;
    private readonly Dictionary<int, OwnedRecord> _owned = new Dictionary<int, OwnedRecord>();
    private int _rootPid;
    private ulong _rootStartTicks;
    private bool _armed;
    private bool _rootConfirmed;
    private bool _escapedSessionObserved;
    private bool _disposed;

    public OxVbaCoreGatePosixOwnedTree()
    {
        OxVbaCoreGatePosix.EnableChildSubreaper();
        _runnerPid = OxVbaCoreGatePosix.CurrentProcessId;
        _baseline = new HashSet<string>(ReadSnapshot().Values
            .Where(record => record.ParentPid == _runnerPid)
            .Select(record => record.Identity), StringComparer.Ordinal);
    }

    public int RunnerProcessId { get { return _runnerPid; } }
    public int RootProcessId { get { return _rootPid; } }
    public ulong RootStartTicks { get { return _rootStartTicks; } }
    public int RetainedPidFdCount { get { Refresh(); return _owned.Count; } }
    public bool EscapedSessionObserved
    {
        get
        {
            Refresh();
            return _escapedSessionObserved;
        }
    }

    public ulong ArmRoot(int processId)
    {
        return ArmRootCore(processId, false);
    }

    // Cross-platform gate tests call this only on Linux to prove that a
    // confirmation failure after pidfd retention still has exact abort
    // authority. Production launch uses ArmRoot above.
    public ulong ArmRootWithForcedConfirmationFailureForTest(int processId)
    {
        return ArmRootCore(processId, true);
    }

    private ulong ArmRootCore(int processId, bool forceConfirmationFailure)
    {
        EnsureLive();
        if (_armed) throw new InvalidOperationException("Linux ownership root is already armed");
        int pidFd = OxVbaCoreGatePosix.TryOpenPidFd(processId);
        if (pidFd < 0)
            throw new InvalidOperationException("Linux ownership root exited before its pidfd could be retained");
        var retained = new OwnedRecord { Pid = processId, PidFd = pidFd };
        _rootPid = processId;
        _owned[processId] = retained;
        _armed = true;

        // Retain exact kernel authority before every fallible /proc or parent
        // confirmation. If any check below fails, TerminateAll still owns the
        // precise task through this pidfd and can abort it without a numeric
        // PID signal or a Process.Kill fallback.
        if (forceConfirmationFailure)
            throw new InvalidOperationException("forced Linux ownership-root confirmation failure after pidfd retention");
        ProcRecord root = ReadProcRecord(processId);
        if (root == null)
            throw new InvalidOperationException("Linux ownership root could not be confirmed after its pidfd was retained");
        if (root.ParentPid != _runnerPid)
            throw new InvalidOperationException("Linux ownership root is not a direct child of the subreaper runner");
        if (!OxVbaCoreGatePosix.SignalPidFd(retained.PidFd, OxVbaCoreGatePosix.SignalZero))
            throw new InvalidOperationException("Linux ownership root exited while its retained pidfd was being confirmed");
        retained.ParentPid = root.ParentPid;
        retained.StartTicks = root.StartTicks;
        _rootStartTicks = root.StartTicks;
        _rootConfirmed = true;
        return root.StartTicks;
    }

    public int LiveProcessCount
    {
        get { return Refresh().Count; }
    }

    public bool TerminateAll(int deadlineMilliseconds)
    {
        EnsureLive();
        if (!_armed) return true;
        if (!_rootConfirmed)
        {
            // The shell supervisor is contractually child-free before the
            // readiness acknowledgement. Abort the exact retained root even
            // when /proc/parent confirmation failed; the caller then reaps its
            // Process handle and verifies that this pidfd has reached ESRCH.
            OwnedRecord unconfirmed;
            if (_owned.TryGetValue(_rootPid, out unconfirmed))
            {
                OxVbaCoreGatePosix.SignalPidFd(unconfirmed.PidFd, OxVbaCoreGatePosix.SignalStop);
                OxVbaCoreGatePosix.SignalPidFd(unconfirmed.PidFd, OxVbaCoreGatePosix.SignalKill);
            }
            return true;
        }
        var timer = Stopwatch.StartNew();
        int deadline = Math.Max(0, deadlineMilliseconds);
        string stableStoppedSet = null;
        int stableStoppedPasses = 0;

        do
        {
            List<ProcRecord> before = Refresh();
            if (before.Count == 0) return true;

            // Freeze parents first. A process may fork before SIGSTOP is
            // delivered, so discovery repeats until the exact stopped set is
            // stable across two passes. Any child created in the race is then
            // retained through its own pidfd before cleanup proceeds.
            foreach (ProcRecord record in OrderParentFirst(before))
                SignalRetained(record.Pid, OxVbaCoreGatePosix.SignalStop);
            if (deadline > 0) Thread.Sleep(1);

            List<ProcRecord> stopped = Refresh();
            string stoppedSet = string.Join("|", stopped.OrderBy(item => item.Pid).Select(item => item.Identity));
            bool allStopped = stopped.All(item => item.State == 'T' || item.State == 't');
            if (allStopped && string.Equals(stoppedSet, stableStoppedSet, StringComparison.Ordinal)) stableStoppedPasses++;
            else stableStoppedPasses = 0;
            stableStoppedSet = stoppedSet;

            if (stableStoppedPasses >= 1 || timer.ElapsedMilliseconds >= deadline)
            {
                foreach (ProcRecord record in OrderParentFirst(stopped))
                    SignalRetained(record.Pid, OxVbaCoreGatePosix.SignalKill);
                stableStoppedSet = null;
                stableStoppedPasses = 0;
                if (deadline > 0) Thread.Sleep(1);
            }
        }
        while (timer.ElapsedMilliseconds < deadline);

        // Even at a spent deadline, make one last exact-handle kill pass. It
        // cannot target a reused numeric PID; failure to observe an empty tree
        // remains explicit to the caller.
        foreach (ProcRecord record in OrderParentFirst(Refresh()))
            SignalRetained(record.Pid, OxVbaCoreGatePosix.SignalKill);
        return Refresh().Count == 0;
    }

    private List<ProcRecord> Refresh()
    {
        EnsureLive();
        if (!_armed) return new List<ProcRecord>();
        if (!_rootConfirmed)
        {
            OwnedRecord unconfirmed;
            if (!_owned.TryGetValue(_rootPid, out unconfirmed)) return new List<ProcRecord>();
            if (!OxVbaCoreGatePosix.SignalPidFd(unconfirmed.PidFd, OxVbaCoreGatePosix.SignalZero))
            {
                unconfirmed.Dispose();
                _owned.Remove(_rootPid);
                return new List<ProcRecord>();
            }
            return new List<ProcRecord> { new ProcRecord { Pid = _rootPid, State = '?' } };
        }
        var snapshot = ReadSnapshot();
        var current = new HashSet<int>();

        foreach (var identity in _owned.ToArray())
        {
            ProcRecord record;
            if (snapshot.TryGetValue(identity.Key, out record) && record.StartTicks == identity.Value.StartTicks)
            {
                current.Add(identity.Key);
                identity.Value.ParentPid = record.ParentPid;
            }
            else
            {
                identity.Value.Dispose();
                _owned.Remove(identity.Key);
            }
        }

        bool changed;
        do
        {
            changed = false;
            foreach (var record in snapshot.Values)
            {
                bool fromKnownParent = current.Contains(record.ParentPid);
                bool newlyAdopted = record.ParentPid == _runnerPid &&
                    record.Pid != _rootPid && !_baseline.Contains(record.Identity);
                if (!fromKnownParent && !newlyAdopted) continue;
                OwnedRecord retained = RetainExact(record, delegate(ProcRecord after)
                {
                    return IsRetainedParentExact(after.ParentPid) ||
                        (after.ParentPid == _runnerPid && after.Pid != _rootPid && !_baseline.Contains(after.Identity));
                });
                if (retained != null && current.Add(record.Pid))
                {
                    _owned[record.Pid] = retained;
                    changed = true;
                    if (retained.ParentPid == _runnerPid && record.Pid != _rootPid) _escapedSessionObserved = true;
                }
                else if (retained != null) retained.Dispose();
            }
        }
        while (changed);

        var live = new List<ProcRecord>();
        foreach (int pid in current)
        {
            ProcRecord record;
            if (!snapshot.TryGetValue(pid, out record)) continue;
            if (record.ProcessGroup != _rootPid || record.Session != _rootPid)
                _escapedSessionObserved = true;
            if (record.State == 'Z' || record.State == 'X')
            {
                // The direct root remains owned by System.Diagnostics.Process,
                // which must retain responsibility for its exit status. Exact
                // adopted descendants have no Process object; reap only those
                // PID/start-time-validated zombies so they cannot remain in
                // /proc after cleanup reports an empty owned tree.
                if (record.Pid != _rootPid && OxVbaCoreGatePosix.ReapOwnedProcess(record.Pid))
                {
                    OwnedRecord reaped;
                    if (_owned.TryGetValue(record.Pid, out reaped)) reaped.Dispose();
                    _owned.Remove(record.Pid);
                }
                continue;
            }
            live.Add(record);
        }
        return live;
    }

    private OwnedRecord RetainExact(ProcRecord expected, Func<ProcRecord, bool> relation)
    {
        int pidFd = OxVbaCoreGatePosix.TryOpenPidFd(expected.Pid);
        if (pidFd < 0) return null;
        try
        {
            ProcRecord after = ReadProcRecord(expected.Pid);
            if (after == null || after.StartTicks != expected.StartTicks || !relation(after)) return null;
            var retained = new OwnedRecord
            {
                Pid = after.Pid,
                ParentPid = after.ParentPid,
                StartTicks = after.StartTicks,
                PidFd = pidFd
            };
            pidFd = -1;
            return retained;
        }
        finally
        {
            if (pidFd >= 0) OxVbaCoreGatePosix.ClosePidFd(pidFd);
        }
    }

    private void SignalRetained(int pid, int signal)
    {
        OwnedRecord retained;
        if (_owned.TryGetValue(pid, out retained))
            OxVbaCoreGatePosix.SignalPidFd(retained.PidFd, signal);
    }

    private bool IsRetainedParentExact(int parentPid)
    {
        OwnedRecord parent;
        if (!_owned.TryGetValue(parentPid, out parent)) return false;
        ProcRecord current = ReadProcRecord(parentPid);
        return current != null && current.StartTicks == parent.StartTicks &&
            OxVbaCoreGatePosix.SignalPidFd(parent.PidFd, OxVbaCoreGatePosix.SignalZero);
    }

    private IEnumerable<ProcRecord> OrderParentFirst(IEnumerable<ProcRecord> records)
    {
        var rows = records.ToDictionary(record => record.Pid);
        return rows.Values.OrderBy(record => GetDepth(record, rows)).ThenBy(record => record.Pid);
    }

    private int GetDepth(ProcRecord record, Dictionary<int, ProcRecord> rows)
    {
        int depth = record.Pid == _rootPid ? 0 : 1;
        int parent = record.ParentPid;
        var seen = new HashSet<int>();
        while (parent != _runnerPid && seen.Add(parent))
        {
            ProcRecord parentRecord;
            if (!rows.TryGetValue(parent, out parentRecord)) break;
            depth++;
            parent = parentRecord.ParentPid;
        }
        return depth;
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        foreach (OwnedRecord record in _owned.Values) record.Dispose();
        _owned.Clear();
    }

    private void EnsureLive()
    {
        if (_disposed) throw new ObjectDisposedException(nameof(OxVbaCoreGatePosixOwnedTree));
    }

    private static Dictionary<int, ProcRecord> ReadSnapshot()
    {
        var result = new Dictionary<int, ProcRecord>();
        foreach (string directory in Directory.EnumerateDirectories("/proc"))
        {
            int pid;
            if (!int.TryParse(Path.GetFileName(directory), NumberStyles.None, CultureInfo.InvariantCulture, out pid))
                continue;
            try
            {
                string text = File.ReadAllText(Path.Combine(directory, "stat"));
                int close = text.LastIndexOf(')');
                if (close < 1 || close + 2 >= text.Length) continue;
                string[] fields = text.Substring(close + 2).Split(new[] { ' ' }, StringSplitOptions.RemoveEmptyEntries);
                if (fields.Length < 20) continue;
                var record = new ProcRecord
                {
                    Pid = pid,
                    State = fields[0][0],
                    ParentPid = int.Parse(fields[1], CultureInfo.InvariantCulture),
                    ProcessGroup = int.Parse(fields[2], CultureInfo.InvariantCulture),
                    Session = int.Parse(fields[3], CultureInfo.InvariantCulture),
                    StartTicks = ulong.Parse(fields[19], CultureInfo.InvariantCulture)
                };
                result[pid] = record;
            }
            catch (IOException) { }
            catch (UnauthorizedAccessException) { }
            catch (FormatException) { }
        }
        return result;
    }

    private static ProcRecord ReadProcRecord(int pid)
    {
        try
        {
            string text = File.ReadAllText("/proc/" + pid.ToString(CultureInfo.InvariantCulture) + "/stat");
            int close = text.LastIndexOf(')');
            if (close < 1 || close + 2 >= text.Length) return null;
            string[] fields = text.Substring(close + 2).Split(new[] { ' ' }, StringSplitOptions.RemoveEmptyEntries);
            if (fields.Length < 20) return null;
            return new ProcRecord
            {
                Pid = pid,
                State = fields[0][0],
                ParentPid = int.Parse(fields[1], CultureInfo.InvariantCulture),
                ProcessGroup = int.Parse(fields[2], CultureInfo.InvariantCulture),
                Session = int.Parse(fields[3], CultureInfo.InvariantCulture),
                StartTicks = ulong.Parse(fields[19], CultureInfo.InvariantCulture)
            };
        }
        catch (IOException) { return null; }
        catch (UnauthorizedAccessException) { return null; }
        catch (FormatException) { return null; }
    }
}
