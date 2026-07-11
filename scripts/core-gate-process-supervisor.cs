using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;

public sealed class OxVbaCoreGateWindowsJob : IDisposable
{
    private const uint CREATE_SUSPENDED = 0x00000004;
    private const uint CREATE_NO_WINDOW = 0x08000000;
    private const uint CREATE_UNICODE_ENVIRONMENT = 0x00000400;
    private const uint STARTF_USESTDHANDLES = 0x00000100;
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

            var startup = new STARTUPINFO();
            startup.cb = Marshal.SizeOf<STARTUPINFO>();
            startup.dwFlags = STARTF_USESTDHANDLES;
            startup.hStdInput = owner._stdin;
            startup.hStdOutput = owner._stdout;
            startup.hStdError = owner._stderr;

            string commandLine = QuoteWindowsArgument(executable) +
                (arguments.Length == 0 ? "" : " " + string.Join(" ", arguments.Select(QuoteWindowsArgument)));
            environmentBlock = Marshal.StringToHGlobalUni(BuildEnvironmentBlock(environment));
            PROCESS_INFORMATION processInfo;
            bool created = CreateProcessW(
                executable,
                new StringBuilder(commandLine),
                IntPtr.Zero,
                IntPtr.Zero,
                true,
                CREATE_SUSPENDED | CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
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
        ref STARTUPINFO startupInfo,
        out PROCESS_INFORMATION processInformation);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr CreateFileW(string fileName, uint access, uint share, ref SECURITY_ATTRIBUTES security,
        uint creation, uint attributes, IntPtr template);

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
    public const int SignalZero = 0;
    public const int SignalTerm = 15;
    public const int SignalKill = 9;

    public static bool GroupExists(int processGroupId)
    {
        int result = kill(-processGroupId, SignalZero);
        if (result == 0) return true;
        int error = Marshal.GetLastWin32Error();
        if (error == 3) return false; // ESRCH
        if (error == 1) return true;  // EPERM
        throw new Win32Exception(error, "kill(group, 0)");
    }

    public static void SignalGroup(int processGroupId, int signal)
    {
        int result = kill(-processGroupId, signal);
        if (result == 0) return;
        int error = Marshal.GetLastWin32Error();
        if (error == 3) return; // already empty
        throw new Win32Exception(error, "kill(group, signal)");
    }

    public static int GetProcessGroup(int processId)
    {
        int result = getpgid(processId);
        if (result >= 0) return result;
        throw new Win32Exception(Marshal.GetLastWin32Error(), "getpgid");
    }

    [DllImport("libc", SetLastError = true)]
    private static extern int kill(int pid, int signal);

    [DllImport("libc", SetLastError = true)]
    private static extern int getpgid(int pid);
}
