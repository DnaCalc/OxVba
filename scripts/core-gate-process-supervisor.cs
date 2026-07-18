using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using Microsoft.Win32.SafeHandles;

internal static class OxVbaCoreGateInputAdmissionTestHook
{
    private static readonly object Sync = new object();
    private static int _matchingAdmissions;

    public static void WaitIfRequested(string[] paths, bool windows)
    {
        string match = Environment.GetEnvironmentVariable("OXVBA_CORE_GATE_TEST_INPUT_ADMISSION_MATCH");
        string ready = Environment.GetEnvironmentVariable("OXVBA_CORE_GATE_TEST_INPUT_ADMISSION_READY");
        string release = Environment.GetEnvironmentVariable("OXVBA_CORE_GATE_TEST_INPUT_ADMISSION_RELEASE");
        if (string.IsNullOrWhiteSpace(match) || string.IsNullOrWhiteSpace(ready) || string.IsNullOrWhiteSpace(release)) return;
        StringComparison comparison = windows ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal;
        if (!paths.Any(path => string.Equals(Path.GetFullPath(path), Path.GetFullPath(match), comparison))) return;
        int requested = 1;
        string occurrenceText = Environment.GetEnvironmentVariable("OXVBA_CORE_GATE_TEST_INPUT_ADMISSION_OCCURRENCE");
        if (!string.IsNullOrWhiteSpace(occurrenceText) &&
            (!int.TryParse(occurrenceText, NumberStyles.None, CultureInfo.InvariantCulture, out requested) || requested < 1))
            throw new InvalidOperationException("input-admission test occurrence must be a positive integer");
        int observed;
        lock (Sync) { observed = ++_matchingAdmissions; }
        if (observed != requested) return;
        File.WriteAllText(ready, "ready\n", new UTF8Encoding(false));
        var timer = Stopwatch.StartNew();
        while (!File.Exists(release) && timer.ElapsedMilliseconds < 10000) Thread.Sleep(2);
        if (!File.Exists(release)) throw new TimeoutException("bound-input race test release was not observed");
    }
}

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
    private const uint FILE_ATTRIBUTE_DIRECTORY = 0x00000010;
    private const uint FILE_ATTRIBUTE_REPARSE_POINT = 0x00000400;
    private const uint FILE_FLAG_BACKUP_SEMANTICS = 0x02000000;
    private const uint FILE_FLAG_OPEN_REPARSE_POINT = 0x00200000;
    private const uint HANDLE_FLAG_INHERIT = 0x00000001;
    private const uint JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x00002000;
    private const uint WAIT_OBJECT_0 = 0;
    private const uint WAIT_TIMEOUT = 258;
    private const int ProcessQueryLimitedInformation = 0x1000;

    private IntPtr _job;
    private IntPtr _process;
    private IntPtr _stdout;
    private IntPtr _stderr;
    private IntPtr _stdin;
    private IntPtr _sentinel;
    private readonly List<IntPtr> _boundFiles = new List<IntPtr>();
    private readonly List<IntPtr> _boundDirectories = new List<IntPtr>();
    private bool _disposed;

    public int ProcessId { get; private set; }

    private OxVbaCoreGateWindowsJob() { }

    public static OxVbaCoreGateWindowsJob Start(
        string executable,
        string[] arguments,
        string workingDirectory,
        IDictionary<string, string> environment,
        string stdoutPath,
        string stderrPath,
        string[] boundInputPaths,
        string[] boundInputSha256)
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
            owner.BindInputs(executable, boundInputPaths, boundInputSha256);
            MaybeWaitAfterInputAdmission(boundInputPaths);
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

    // Returns "pid:image-path" for every process currently assigned to the job.
    // Used only after the direct child exits to distinguish manifest-declared
    // ambient toolchain helpers (for example MSVC vctip.exe) from a genuine
    // surviving descendant. A member whose image cannot be resolved is recorded
    // as "pid:?" so it is judged (and fails) rather than silently dropped.
    public string[] GetMemberImageNames()
    {
        EnsureLive();
        const int headerSize = 8; // JOBOBJECT_BASIC_PROCESS_ID_LIST: two UInt32 counters then ULONG_PTR[]
        int capacity = 16;
        while (true)
        {
            int size = headerSize + IntPtr.Size * capacity;
            IntPtr buffer = Marshal.AllocHGlobal(size);
            try
            {
                Marshal.WriteInt32(buffer, 0, 0);
                Marshal.WriteInt32(buffer, 4, capacity);
                // JobObjectBasicProcessIdList = 3 (verified empirically on
                // Windows x64: class 2 is rejected with ERROR_BAD_LENGTH).
                if (!QueryInformationJobObject(_job, 3, buffer, (uint)size, IntPtr.Zero))
                    ThrowLastError("QueryInformationJobObject(ProcessIdList)");
                int assigned = Marshal.ReadInt32(buffer, 0);
                int returned = Marshal.ReadInt32(buffer, 4);
                if (assigned > capacity) { capacity = assigned * 2; continue; }
                var names = new List<string>();
                for (int index = 0; index < returned; index++)
                {
                    long pidValue = Marshal.ReadIntPtr(buffer, headerSize + index * IntPtr.Size).ToInt64();
                    string image = QueryImageName((int)pidValue);
                    // An unresolvable member (protected image, exit race) is
                    // recorded as "pid:?" so it can never vanish from the
                    // ambient-declaration judgment; "?" matches no declared name.
                    names.Add(pidValue.ToString(CultureInfo.InvariantCulture) + ":" + (image ?? "?"));
                }
                return names.ToArray();
            }
            finally { Marshal.FreeHGlobal(buffer); }
        }
    }

    private static string QueryImageName(int processId)
    {
        IntPtr process = OpenProcess(ProcessQueryLimitedInformation, false, processId);
        if (process == IntPtr.Zero) return null;
        try
        {
            var builder = new StringBuilder(1024);
            int length = builder.Capacity;
            if (!QueryFullProcessImageNameW(process, 0, builder, ref length)) return null;
            return builder.ToString();
        }
        finally { CloseHandle(process); }
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
        foreach (IntPtr handle in _boundFiles) if (handle != IntPtr.Zero && handle != new IntPtr(-1)) CloseHandle(handle);
        foreach (IntPtr handle in _boundDirectories) if (handle != IntPtr.Zero && handle != new IntPtr(-1)) CloseHandle(handle);
        _boundFiles.Clear();
        _boundDirectories.Clear();
    }

    public int BoundInputHandleCount { get { return _boundFiles.Count; } }

    private void BindInputs(string executable, string[] paths, string[] sha256)
    {
        if (paths == null || sha256 == null || paths.Length == 0 || paths.Length != sha256.Length)
            throw new ArgumentException("Windows bound input paths and digests must be non-empty parallel arrays");
        var admitted = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        bool executableBound = false;
        for (int index = 0; index < paths.Length; index++)
        {
            string path = Path.GetFullPath(paths[index]);
            string expected = sha256[index] ?? "";
            if (!admitted.Add(path)) continue;
            BindAncestorDirectories(path);
            IntPtr linkHandle = CreateFileW(path, 0, FILE_SHARE_READ, IntPtr.Zero,
                OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT, IntPtr.Zero);
            ThrowIfInvalid(linkHandle, "CreateFileW(bound input entry)");
            IntPtr dataHandle = IntPtr.Zero;
            try
            {
                BY_HANDLE_FILE_INFORMATION entryInformation;
                if (!GetFileInformationByHandle(linkHandle, out entryInformation)) ThrowLastError("GetFileInformationByHandle(bound input entry)");
                if ((entryInformation.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0)
                    throw new InvalidOperationException("Windows bound input must not be a directory: " + path);
                bool reparse = (entryInformation.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;

                dataHandle = CreateFileW(path, GENERIC_READ, FILE_SHARE_READ, IntPtr.Zero,
                    OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, IntPtr.Zero);
                ThrowIfInvalid(dataHandle, "CreateFileW(bound input target)");
                BY_HANDLE_FILE_INFORMATION dataInformation;
                if (!GetFileInformationByHandle(dataHandle, out dataInformation)) ThrowLastError("GetFileInformationByHandle(bound input target)");
                if ((dataInformation.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)) != 0)
                    throw new InvalidOperationException("Windows bound input target must be a regular file: " + path);
                string targetPath = GetFinalPath(dataHandle);
                BindAncestorDirectories(targetPath);
                if (!string.Equals(GetFinalPath(dataHandle), targetPath, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidOperationException("Windows bound input target changed while ancestor locks were acquired: " + path);
                string actual = HashHandle(dataHandle);
                if (!string.Equals(actual, expected, StringComparison.Ordinal))
                    throw new InvalidOperationException("Windows bound input bytes differ from admitted digest: " + path);
                if (reparse)
                {
                    _boundFiles.Add(linkHandle);
                    linkHandle = IntPtr.Zero;
                }
                _boundFiles.Add(dataHandle);
                dataHandle = IntPtr.Zero;
                if (string.Equals(path, Path.GetFullPath(executable), StringComparison.OrdinalIgnoreCase)) executableBound = true;
            }
            finally
            {
                if (linkHandle != IntPtr.Zero && linkHandle != new IntPtr(-1)) CloseHandle(linkHandle);
                if (dataHandle != IntPtr.Zero && dataHandle != new IntPtr(-1)) CloseHandle(dataHandle);
            }
        }
        if (!executableBound) throw new InvalidOperationException("Windows executable is not present in the bound input set");
    }

    private void BindAncestorDirectories(string filePath)
    {
        var existing = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (IntPtr handle in _boundDirectories)
        {
            string final = GetFinalPath(handle);
            if (!string.IsNullOrEmpty(final)) existing.Add(final);
        }
        var ancestors = new Stack<string>();
        DirectoryInfo directory = Directory.GetParent(filePath);
        while (directory != null)
        {
            ancestors.Push(directory.FullName);
            directory = directory.Parent;
        }
        while (ancestors.Count > 0)
        {
            string path = ancestors.Pop();
            string normalized = NormalizeFinalPath(path);
            if (existing.Contains(normalized)) continue;
            IntPtr handle = CreateFileW(path, 0, FILE_SHARE_READ | FILE_SHARE_WRITE, IntPtr.Zero,
                OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT, IntPtr.Zero);
            ThrowIfInvalid(handle, "CreateFileW(bound ancestor directory)");
            try
            {
                BY_HANDLE_FILE_INFORMATION information;
                if (!GetFileInformationByHandle(handle, out information)) ThrowLastError("GetFileInformationByHandle(bound ancestor directory)");
                if ((information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) == 0 ||
                    (information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0)
                    throw new InvalidOperationException("Windows bound input ancestor must be a non-reparse directory: " + path);
                string final = GetFinalPath(handle);
                if (!string.Equals(final, normalized, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidOperationException("Windows bound input ancestor resolved to an unexpected directory: " + path);
                _boundDirectories.Add(handle);
                existing.Add(final);
                handle = IntPtr.Zero;
            }
            finally { if (handle != IntPtr.Zero && handle != new IntPtr(-1)) CloseHandle(handle); }
        }
    }

    private static string HashHandle(IntPtr handle)
    {
        using (var safe = new SafeFileHandle(handle, false))
        using (var stream = new FileStream(safe, FileAccess.Read, 65536, false))
        using (var hash = SHA256.Create())
            return string.Concat(hash.ComputeHash(stream).Select(value => value.ToString("x2", CultureInfo.InvariantCulture)));
    }

    private static string GetFinalPath(IntPtr handle)
    {
        var buffer = new StringBuilder(32768);
        uint length = GetFinalPathNameByHandleW(handle, buffer, (uint)buffer.Capacity, 0);
        if (length == 0 || length >= buffer.Capacity) ThrowLastError("GetFinalPathNameByHandleW(bound input)");
        return NormalizeFinalPath(buffer.ToString());
    }

    private static string NormalizeFinalPath(string path)
    {
        string full = Path.GetFullPath(path);
        if (full.StartsWith("\\\\?\\UNC\\", StringComparison.OrdinalIgnoreCase)) return "\\\\" + full.Substring(8).TrimEnd('\\');
        if (full.StartsWith("\\\\?\\", StringComparison.OrdinalIgnoreCase)) full = full.Substring(4);
        return full.TrimEnd('\\');
    }

    private static void MaybeWaitAfterInputAdmission(string[] paths)
    {
        OxVbaCoreGateInputAdmissionTestHook.WaitIfRequested(paths, true);
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
    private struct BY_HANDLE_FILE_INFORMATION
    {
        public uint dwFileAttributes;
        public System.Runtime.InteropServices.ComTypes.FILETIME ftCreationTime;
        public System.Runtime.InteropServices.ComTypes.FILETIME ftLastAccessTime;
        public System.Runtime.InteropServices.ComTypes.FILETIME ftLastWriteTime;
        public uint dwVolumeSerialNumber;
        public uint nFileSizeHigh;
        public uint nFileSizeLow;
        public uint nNumberOfLinks;
        public uint nFileIndexHigh;
        public uint nFileIndexLow;
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
    private static extern IntPtr CreateFileW(string fileName, uint access, uint share, IntPtr security,
        uint creation, uint attributes, IntPtr template);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetFileInformationByHandle(IntPtr handle, out BY_HANDLE_FILE_INFORMATION information);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern uint GetFinalPathNameByHandleW(IntPtr handle, StringBuilder path, uint length, uint flags);

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

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern IntPtr OpenProcess(int access, bool inheritHandle, int processId);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool QueryFullProcessImageNameW(IntPtr process, int flags, StringBuilder name, ref int length);
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
/// Linux direct-child launcher whose working directory and every executable or
/// readable launch input are retained as CLOEXEC descriptors before spawn.
/// posix_spawn file actions duplicate only the required authority to reserved
/// child descriptors; no parent-wide inheritance window exists. The shell and
/// gate never reopen an admitted executable, supervisor, manifest, or command
/// through its pathname.
/// </summary>
public sealed class OxVbaCoreGatePosixChild : IDisposable
{
    private const long SysOpenAt2X64 = 437;
    private const long SysMemFdCreateX64 = 319;
    private const int AtFdcwd = -100;
    private const int OReadOnly = 0;
    private const int OReadWrite = 2;
    private const int OCreate = 0x40;
    private const int OExclusive = 0x80;
    private const int OTruncate = 0x200;
    private const int OCloseOnExec = 0x80000;
    private const int ODirectory = 0x10000;
    private const int ONoFollow = 0x20000;
    private const int OPath = 0x200000;
    private const int FGetFd = 1;
    private const int FdCloseOnExec = 1;
    private const int FAddSeals = 1033;
    private const int FGetSeals = 1034;
    private const int FSealSeal = 0x0001;
    private const int FSealShrink = 0x0002;
    private const int FSealGrow = 0x0004;
    private const int FSealWrite = 0x0008;
    private const uint MemFdCloseOnExec = 0x0001;
    private const uint MemFdAllowSealing = 0x0002;
    private const int WaitNoHang = 1;
    private const int SeekSet = 0;
    private const ulong ResolveNoMagicLinks = 0x02;
    private const ulong ResolveNoSymlinks = 0x04;
    private const ulong ResolveBeneath = 0x08;
    private const uint SIfmt = 0xF000;
    private const uint SIfreg = 0x8000;
    private const uint SIfdir = 0x4000;
    private const int AtSymlinkNoFollow = 0x100;

    private sealed class BoundFile
    {
        public string Path;
        public string Sha256;
        public int Fd;
        public int SourceFd;
        public string ProcPath { get { return "/proc/self/fd/" + Fd.ToString(CultureInfo.InvariantCulture); } }
    }

    private sealed class Utf8Array : IDisposable
    {
        private readonly List<IntPtr> _strings = new List<IntPtr>();
        public IntPtr Pointer { get; private set; }

        public Utf8Array(IEnumerable<string> values)
        {
            string[] rows = values.ToArray();
            Pointer = Marshal.AllocHGlobal((rows.Length + 1) * IntPtr.Size);
            for (int index = 0; index < rows.Length; index++)
            {
                byte[] bytes = Encoding.UTF8.GetBytes(rows[index] + "\0");
                IntPtr text = Marshal.AllocHGlobal(bytes.Length);
                Marshal.Copy(bytes, 0, text, bytes.Length);
                _strings.Add(text);
                Marshal.WriteIntPtr(Pointer, index * IntPtr.Size, text);
            }
            Marshal.WriteIntPtr(Pointer, rows.Length * IntPtr.Size, IntPtr.Zero);
        }

        public void Dispose()
        {
            foreach (IntPtr text in _strings) Marshal.FreeHGlobal(text);
            _strings.Clear();
            if (Pointer != IntPtr.Zero) Marshal.FreeHGlobal(Pointer);
            Pointer = IntPtr.Zero;
        }
    }

    // The pinned Linux profile is Debian glibc x64. Its public
    // posix_spawn_file_actions_t ABI is 80 bytes (two ints, one pointer and
    // sixteen padding ints). Keeping the opaque storage here avoids a native
    // helper while still using libc's initializer/destructor for ownership.
    private sealed class SpawnFileActions : IDisposable
    {
        private const int GlibcX64FileActionsSize = 80;
        private bool _initialized;
        public IntPtr Pointer { get; private set; }

        public SpawnFileActions()
        {
            Pointer = Marshal.AllocHGlobal(GlibcX64FileActionsSize);
            Marshal.Copy(new byte[GlibcX64FileActionsSize], 0, Pointer, GlibcX64FileActionsSize);
            int error = posix_spawn_file_actions_init(Pointer);
            if (error != 0)
            {
                Marshal.FreeHGlobal(Pointer);
                Pointer = IntPtr.Zero;
                throw new Win32Exception(error, "posix_spawn_file_actions_init(fd-bound launch)");
            }
            _initialized = true;
        }

        public void AddDup2(int sourceFd, int childFd)
        {
            int error = posix_spawn_file_actions_adddup2(Pointer, sourceFd, childFd);
            if (error != 0) throw new Win32Exception(error, "posix_spawn_file_actions_adddup2(fd-bound launch)");
        }

        public void Dispose()
        {
            int error = 0;
            if (_initialized) error = posix_spawn_file_actions_destroy(Pointer);
            _initialized = false;
            if (Pointer != IntPtr.Zero) Marshal.FreeHGlobal(Pointer);
            Pointer = IntPtr.Zero;
            if (error != 0) throw new Win32Exception(error, "posix_spawn_file_actions_destroy(fd-bound launch)");
        }
    }

    private sealed class UnrelatedInheritanceProbe : IDisposable
    {
        private readonly ManualResetEventSlim _release = new ManualResetEventSlim(false);
        private readonly ManualResetEventSlim _entered = new ManualResetEventSlim(false);
        private readonly Thread _thread;
        private readonly string _bashProcPath;
        private readonly int _candidateFd;
        private readonly string _expectedFirstLine;
        private Exception _failure;
        private bool _disposed;

        private UnrelatedInheritanceProbe(string bashProcPath, int candidateFd, string expectedFirstLine)
        {
            _bashProcPath = bashProcPath;
            _candidateFd = candidateFd;
            _expectedFirstLine = expectedFirstLine;
            _thread = new Thread(Run) { IsBackground = true, Name = "oxvba-unrelated-inheritance-probe" };
            _thread.Start();
        }

        public static UnrelatedInheritanceProbe CreateIfRequested(
            string bashProcPath, int candidateFd, string expectedFirstLine)
        {
            return string.Equals(Environment.GetEnvironmentVariable(
                    "OXVBA_CORE_GATE_TEST_PROBE_UNRELATED_INHERITANCE"), "1", StringComparison.Ordinal)
                ? new UnrelatedInheritanceProbe(bashProcPath, candidateFd, expectedFirstLine)
                : null;
        }

        public void ReleaseAtLaunchBoundary()
        {
            _release.Set();
            if (!_entered.Wait(5000))
                throw new TimeoutException("unrelated inheritance probe did not reach the concurrent launch boundary");
        }

        private void Run()
        {
            try
            {
                _release.Wait();
                _entered.Set();
                const string script =
                    "candidate=$1; expected=$2; " +
                    "if [[ -r \"$candidate\" ]]; then " +
                    "IFS= read -r observed < \"$candidate\" || true; " +
                    "if [[ \"$observed\" == \"$expected\" ]]; then exit 97; fi; fi; exit 0";
                using (var argv = new Utf8Array(new[] {
                    "bash", "--noprofile", "--norc", "-c", script, "inheritance-probe",
                    ProcPath(_candidateFd), _expectedFirstLine }))
                using (var envp = new Utf8Array(new[] { "LC_ALL=C", "PATH=/usr/bin:/bin" }))
                {
                    int pid;
                    int error = posix_spawn(out pid, _bashProcPath, IntPtr.Zero, IntPtr.Zero,
                        argv.Pointer, envp.Pointer);
                    if (error != 0) throw new Win32Exception(error, "posix_spawn(unrelated inheritance probe)");
                    int status;
                    int waited;
                    do { waited = waitpid(pid, out status, 0); }
                    while (waited < 0 && Marshal.GetLastWin32Error() == 4);
                    if (waited != pid)
                        throw new Win32Exception(Marshal.GetLastWin32Error(), "waitpid(unrelated inheritance probe)");
                    int exitCode = DecodeExitCode(status);
                    if (exitCode == 97)
                        throw new InvalidOperationException("unrelated concurrent child inherited an admitted gate descriptor");
                    if (exitCode != 0)
                        throw new InvalidOperationException("unrelated inheritance probe exited " +
                            exitCode.ToString(CultureInfo.InvariantCulture));
                }
            }
            catch (Exception error) { _failure = error; }
        }

        public void Dispose()
        {
            if (_disposed) return;
            _disposed = true;
            _release.Set();
            if (!_thread.Join(10000))
                throw new TimeoutException("unrelated inheritance probe did not terminate");
            _release.Dispose();
            _entered.Dispose();
            if (_failure != null) throw new InvalidOperationException(
                "unrelated concurrent child inheritance sentinel failed", _failure);
        }
    }

    private readonly List<BoundFile> _inputs = new List<BoundFile>();
    private int _rootFd = -1;
    private int _workingDirectoryFd = -1;
    private int _gateDirectoryFd = -1;
    private int _readyFd = -1;
    private int _ackFd = -1;
    private int _stdoutFd = -1;
    private int _stderrFd = -1;
    private int _launchPidFd = -1;
    private string _readyName;
    private string _ackName;
    private int _processId;
    private int _waitStatus;
    private string _libcIdentity;
    private bool _exited;
    private bool _disposed;

    private OxVbaCoreGatePosixChild() { }

    public static OxVbaCoreGatePosixChild Start(
        string launcherPath,
        string bashPath,
        string supervisorPath,
        string executablePath,
        string[] arguments,
        string workingDirectory,
        IDictionary<string, string> environment,
        string readyPath,
        string ackPath,
        string nonce,
        string stdoutPath,
        string stderrPath,
        string[] boundInputPaths,
        string[] boundInputSha256)
    {
        if (!RuntimeInformation.IsOSPlatform(OSPlatform.Linux) || RuntimeInformation.ProcessArchitecture != Architecture.X64)
            throw new PlatformNotSupportedException("fd-bound Linux gate launch is Linux x64-only");
        if (boundInputPaths == null || boundInputSha256 == null || boundInputPaths.Length == 0 ||
            boundInputPaths.Length != boundInputSha256.Length)
            throw new ArgumentException("Linux bound input paths and digests must be non-empty parallel arrays");

        string libcIdentity = RequirePinnedLibcIdentity();
        var child = new OxVbaCoreGatePosixChild { _libcIdentity = libcIdentity };
        try
        {
            child._rootFd = OpenPathFromRoot("/", OPath | ODirectory | OCloseOnExec, true);
            string workingAbsolute = Path.GetFullPath(workingDirectory);
            child._workingDirectoryFd = child.OpenDirectoryExact(workingAbsolute);

            var byPath = new Dictionary<string, BoundFile>(StringComparer.Ordinal);
            for (int index = 0; index < boundInputPaths.Length; index++)
            {
                string path = Path.GetFullPath(boundInputPaths[index]);
                string digest = boundInputSha256[index] ?? "";
                BoundFile existing;
                if (byPath.TryGetValue(path, out existing))
                {
                    if (!string.Equals(existing.Sha256, digest, StringComparison.Ordinal))
                        throw new InvalidOperationException("Linux bound input has conflicting admitted digests: " + path);
                    continue;
                }
                int sourceFd = child.OpenRegularInput(path, workingAbsolute);
                string actual = HashFd(sourceFd);
                if (!string.Equals(actual, digest, StringComparison.Ordinal))
                {
                    close(sourceFd);
                    throw new InvalidOperationException("Linux bound input bytes differ from admitted digest: " + path);
                }
                int fd = CreateSealedSnapshot(sourceFd, path);
                if (!string.Equals(HashFd(fd), digest, StringComparison.Ordinal))
                {
                    close(fd);
                    close(sourceFd);
                    throw new InvalidOperationException("Linux sealed bound-input snapshot differs from admitted digest: " + path);
                }
                lseek(sourceFd, 0, SeekSet);
                lseek(fd, 0, SeekSet);
                var bound = new BoundFile { Path = path, Sha256 = digest, Fd = fd, SourceFd = sourceFd };
                child._inputs.Add(bound);
                byPath.Add(path, bound);
            }

            BoundFile launcher = RequireBound(byPath, launcherPath, "setsid launcher");
            BoundFile bash = RequireBound(byPath, bashPath, "Bash executable");
            BoundFile supervisor = RequireBound(byPath, supervisorPath, "Bash supervisor");
            BoundFile executable = RequireBound(byPath, executablePath, "gate executable");

            string gateDirectory = Path.GetDirectoryName(Path.GetFullPath(stdoutPath));
            if (!string.Equals(gateDirectory, Path.GetDirectoryName(Path.GetFullPath(stderrPath)), StringComparison.Ordinal) ||
                !string.Equals(gateDirectory, Path.GetDirectoryName(Path.GetFullPath(readyPath)), StringComparison.Ordinal) ||
                !string.Equals(gateDirectory, Path.GetDirectoryName(Path.GetFullPath(ackPath)), StringComparison.Ordinal))
                throw new InvalidOperationException("Linux gate transport files must share one admitted directory");
            child._gateDirectoryFd = child.OpenDirectoryExact(gateDirectory);
            child._readyName = Path.GetFileName(readyPath);
            child._ackName = Path.GetFileName(ackPath);
            child._readyFd = OpenAt(child._gateDirectoryFd, child._readyName,
                OReadWrite | OCreate | OExclusive | OCloseOnExec | ONoFollow, 0x180);
            child._ackFd = OpenAt(child._gateDirectoryFd, child._ackName,
                OReadWrite | OCreate | OExclusive | OCloseOnExec | ONoFollow, 0x180);
            child._stdoutFd = OpenAt(child._gateDirectoryFd, Path.GetFileName(stdoutPath),
                OReadWrite | OTruncate | OCloseOnExec | ONoFollow, 0);
            child._stderrFd = OpenAt(child._gateDirectoryFd, Path.GetFileName(stderrPath),
                OReadWrite | OTruncate | OCloseOnExec | ONoFollow, 0);
            RequireRegular(child._stdoutFd, "Linux gate stdout");
            RequireRegular(child._stderrFd, "Linux gate stderr");

            MaybeWaitAfterInputAdmission(boundInputPaths, false);

            int[] childSources = child._inputs.Select(input => input.Fd)
                .Concat(new[] { child._workingDirectoryFd, child._readyFd, child._ackFd,
                    child._stdoutFd, child._stderrFd })
                .Distinct().ToArray();
            RequireCloseOnExec(childSources
                    .Concat(child._inputs.Select(input => input.SourceFd))
                    .Concat(new[] { child._rootFd, child._gateDirectoryFd }),
                "Linux admitted parent descriptor");
            Dictionary<int, int> childFds = AllocateChildDescriptors(childSources);

            string Rewrite(string value)
            {
                if (string.IsNullOrEmpty(value)) return value;
                BoundFile match;
                return byPath.TryGetValue(Path.GetFullPath(value), out match)
                    ? ProcPath(childFds[match.Fd])
                    : value;
            }

            var launchArguments = new List<string>
            {
                Path.GetFullPath(launcherPath),
                ProcPath(childFds[bash.Fd]),
                ProcPath(childFds[supervisor.Fd]),
                ProcPath(childFds[child._readyFd]),
                ProcPath(childFds[child._ackFd]),
                nonce,
                ProcPath(childFds[child._stdoutFd]),
                ProcPath(childFds[child._stderrFd]),
                ProcPath(childFds[child._workingDirectoryFd]),
                ProcPath(childFds[executable.Fd]),
                Path.GetFullPath(executablePath)
            };
            launchArguments.AddRange(arguments.Select(Rewrite));
            var environmentRows = environment.OrderBy(pair => pair.Key, StringComparer.Ordinal)
                .Select(pair => pair.Key + "=" + Rewrite(pair.Value)).ToArray();

            using (var argv = new Utf8Array(launchArguments))
            using (var envp = new Utf8Array(environmentRows))
            using (var fileActions = new SpawnFileActions())
            {
                foreach (KeyValuePair<int, int> descriptor in childFds.OrderBy(pair => pair.Value))
                    fileActions.AddDup2(descriptor.Key, descriptor.Value);

                UnrelatedInheritanceProbe probe = UnrelatedInheritanceProbe.CreateIfRequested(
                    bash.ProcPath, supervisor.Fd, ReadFirstLine(supervisor.Fd));
                try
                {
                    if (probe != null) probe.ReleaseAtLaunchBoundary();
                    int error = posix_spawn(out child._processId, launcher.ProcPath,
                        fileActions.Pointer, IntPtr.Zero, argv.Pointer, envp.Pointer);
                    if (error != 0) throw new Win32Exception(error, "posix_spawn(fd-bound setsid)");
                    child._launchPidFd = OxVbaCoreGatePosix.TryOpenPidFd(child._processId);
                    if (child._launchPidFd < 0)
                    {
                        child.ReapExactSpawnedChildWithoutSignal(1000);
                        throw new InvalidOperationException("fd-bound Linux child exited before its launch pidfd could be retained");
                    }
                    RequireCloseOnExec(new[] { child._launchPidFd }, "Linux launch pidfd");
                    string forcedPidPath = Environment.GetEnvironmentVariable("OXVBA_CORE_GATE_TEST_FORCED_POST_SPAWN_PID_PATH");
                    if (!string.IsNullOrWhiteSpace(forcedPidPath))
                        File.WriteAllText(forcedPidPath, child._processId.ToString(CultureInfo.InvariantCulture) + "\n", new UTF8Encoding(false));
                    if (string.Equals(Environment.GetEnvironmentVariable("OXVBA_CORE_GATE_TEST_FORCE_POST_SPAWN_FAILURE"), "1", StringComparison.Ordinal))
                        throw new InvalidOperationException("forced fd-bound post-spawn failure after pidfd retention");
                }
                finally { if (probe != null) probe.Dispose(); }
            }
            return child;
        }
        catch
        {
            try { child.AbortSpawnedChildOnStartFailure(); }
            finally { child.Dispose(); }
            throw;
        }
    }

    public int ProcessId { get { EnsureLive(); return _processId; } }
    public int BoundInputFdCount { get { EnsureLive(); return _inputs.Count; } }
    public string LibcIdentity { get { EnsureLive(); return _libcIdentity; } }
    public static string RuntimeLibcIdentity { get { return RequirePinnedLibcIdentity(); } }
    public bool HasExited { get { EnsureLive(); Poll(); return _exited; } }
    public int RetainedLaunchPidFdCount { get { EnsureLive(); return _launchPidFd >= 0 ? 1 : 0; } }
    public static int CountOpenDescriptorsForTest()
    {
        return Directory.EnumerateFileSystemEntries("/proc/self/fd").ToArray().Length;
    }
    public int ExitCode
    {
        get
        {
            EnsureLive();
            Poll();
            if (!_exited) throw new InvalidOperationException("Linux fd-bound child has not exited");
            return DecodeExitCode(_waitStatus);
        }
    }

    public bool WaitForExit(int milliseconds)
    {
        EnsureLive();
        var timer = Stopwatch.StartNew();
        do
        {
            Poll();
            if (_exited) return true;
            if (milliseconds == 0) break;
            Thread.Sleep(2);
        }
        while (timer.ElapsedMilliseconds < milliseconds);
        Poll();
        return _exited;
    }

    public byte[] ReadReadyBytes() { EnsureLive(); return ReadFdBytes(_readyFd); }
    public byte[] ReadStdoutBytes() { EnsureLive(); return ReadFdBytes(_stdoutFd); }
    public byte[] ReadStderrBytes() { EnsureLive(); return ReadFdBytes(_stderrFd); }

    public void WriteAcknowledgement(byte[] bytes)
    {
        EnsureLive();
        if (ftruncate(_ackFd, 0) != 0 || lseek(_ackFd, 0, SeekSet) < 0)
            throw new Win32Exception(Marshal.GetLastWin32Error(), "prepare Linux ownership acknowledgement fd");
        int offset = 0;
        GCHandle pinned = GCHandle.Alloc(bytes, GCHandleType.Pinned);
        try
        {
            while (offset < bytes.Length)
            {
                long written = write(_ackFd, IntPtr.Add(pinned.AddrOfPinnedObject(), offset), new UIntPtr((uint)(bytes.Length - offset)));
                if (written <= 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "write Linux ownership acknowledgement fd");
                offset += checked((int)written);
            }
        }
        finally { pinned.Free(); }
        fsync(_ackFd);
    }

    public void Dispose()
    {
        if (_disposed) return;
        Exception abortFailure = null;
        if (_processId > 0 && !_exited)
        {
            try { AbortSpawnedChildOnStartFailure(); }
            catch (Exception error) { abortFailure = error; }
        }
        _disposed = true;
        try
        {
            TryUnlinkSame(_gateDirectoryFd, _readyName, _readyFd);
            TryUnlinkSame(_gateDirectoryFd, _ackName, _ackFd);
        }
        finally
        {
            CloseFd(ref _readyFd);
            CloseFd(ref _ackFd);
            CloseFd(ref _stdoutFd);
            CloseFd(ref _stderrFd);
            CloseFd(ref _gateDirectoryFd);
            CloseFd(ref _workingDirectoryFd);
            foreach (BoundFile input in _inputs)
            {
                int fd = input.Fd;
                int sourceFd = input.SourceFd;
                input.Fd = -1;
                input.SourceFd = -1;
                if (fd >= 0) close(fd);
                if (sourceFd >= 0) close(sourceFd);
            }
            _inputs.Clear();
            CloseFd(ref _rootFd);
            try { CloseLaunchPidFd(); }
            catch (Exception error) { if (abortFailure == null) abortFailure = error; }
        }
        if (abortFailure != null)
            throw new InvalidOperationException("fd-bound Linux child cleanup failed", abortFailure);
    }

    private int OpenDirectoryExact(string path)
    {
        string canonical = RealPath(path);
        int fd = OpenAt2(_rootFd, canonical.TrimStart('/'), OPath | ODirectory | OCloseOnExec,
            ResolveBeneath | ResolveNoMagicLinks | ResolveNoSymlinks);
        RequireDirectory(fd, "Linux bound directory");
        return fd;
    }

    private int OpenRegularInput(string path, string workingDirectory)
    {
        string relative = Path.GetRelativePath(workingDirectory, path);
        bool beneath = relative.Length > 0 && relative != "." && relative != ".." &&
            !relative.StartsWith("../", StringComparison.Ordinal);
        int fd;
        if (beneath)
        {
            fd = OpenAt2(_workingDirectoryFd, relative, OReadOnly | OCloseOnExec,
                ResolveBeneath | ResolveNoMagicLinks | ResolveNoSymlinks);
        }
        else
        {
            string canonical = RealPath(path);
            fd = OpenAt2(_rootFd, canonical.TrimStart('/'), OReadOnly | OCloseOnExec,
                ResolveBeneath | ResolveNoMagicLinks | ResolveNoSymlinks);
        }
        RequireRegular(fd, "Linux bound input");
        return fd;
    }

    private static BoundFile RequireBound(Dictionary<string, BoundFile> rows, string path, string owner)
    {
        BoundFile result;
        if (!rows.TryGetValue(Path.GetFullPath(path), out result))
            throw new InvalidOperationException(owner + " is not present in the Linux bound input set");
        return result;
    }

    private static string HashFd(int fd)
    {
        if (lseek(fd, 0, SeekSet) < 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "lseek(bound input)");
        using (var safe = new SafeFileHandle(new IntPtr(fd), false))
        using (var stream = new FileStream(safe, FileAccess.Read, 65536, false))
        using (var hash = SHA256.Create())
            return string.Concat(hash.ComputeHash(stream).Select(value => value.ToString("x2", CultureInfo.InvariantCulture)));
    }

    private static int CreateSealedSnapshot(int sourceFd, string sourcePath)
    {
        string name = "oxvba-core-gate-" + Path.GetFileName(sourcePath);
        long created = syscall_memfd_create(SysMemFdCreateX64, name, MemFdCloseOnExec | MemFdAllowSealing);
        if (created < 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "memfd_create(bound input snapshot)");
        int writable = checked((int)created);
        try
        {
            if (lseek(sourceFd, 0, SeekSet) < 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "lseek(bound input source)");
            using (var sourceSafe = new SafeFileHandle(new IntPtr(sourceFd), false))
            using (var targetSafe = new SafeFileHandle(new IntPtr(writable), false))
            using (var source = new FileStream(sourceSafe, FileAccess.Read, 65536, false))
            using (var target = new FileStream(targetSafe, FileAccess.Write, 65536, false))
            {
                source.CopyTo(target);
                target.Flush();
            }
            if (fchmod(writable, 0x1c0) != 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "fchmod(bound input snapshot)");
            int required = FSealSeal | FSealShrink | FSealGrow | FSealWrite;
            if (fcntl(writable, FAddSeals, required) != 0)
                throw new Win32Exception(Marshal.GetLastWin32Error(), "fcntl(F_ADD_SEALS, bound input snapshot)");
            if ((fcntl(writable, FGetSeals, 0) & required) != required)
                throw new InvalidOperationException("Linux bound-input snapshot did not retain all required seals");
            int readOnly = open(ProcPath(writable), OReadOnly | OCloseOnExec, 0);
            if (readOnly < 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "open(read-only sealed bound input snapshot)");
            close(writable);
            writable = -1;
            return readOnly;
        }
        finally { if (writable >= 0) close(writable); }
    }

    private static byte[] ReadFdBytes(int fd)
    {
        Stat stat;
        if (fstat(fd, out stat) != 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "fstat(fd-bound evidence)");
        if (stat.Size < 0 || stat.Size > 16 * 1024 * 1024) throw new InvalidOperationException("fd-bound evidence length is invalid");
        byte[] bytes = new byte[checked((int)stat.Size)];
        int offset = 0;
        GCHandle pinned = GCHandle.Alloc(bytes, GCHandleType.Pinned);
        try
        {
            while (offset < bytes.Length)
            {
                long readCount = pread(fd, IntPtr.Add(pinned.AddrOfPinnedObject(), offset),
                    new UIntPtr((uint)(bytes.Length - offset)), offset);
                if (readCount < 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "pread(fd-bound evidence)");
                if (readCount == 0) break;
                offset += checked((int)readCount);
            }
        }
        finally { pinned.Free(); }
        if (offset == bytes.Length) return bytes;
        Array.Resize(ref bytes, offset);
        return bytes;
    }

    private static string ReadFirstLine(int fd)
    {
        byte[] bytes = ReadFdBytes(fd);
        int length = Array.IndexOf(bytes, (byte)'\n');
        if (length < 0) length = bytes.Length;
        if (length > 4096)
            throw new InvalidOperationException("unrelated inheritance probe marker line exceeds 4096 bytes");
        if (length > 0 && bytes[length - 1] == (byte)'\r') length--;
        string value = new UTF8Encoding(false, true).GetString(bytes, 0, length);
        if (value.IndexOf('\0') >= 0)
            throw new InvalidOperationException("unrelated inheritance probe marker line contains NUL");
        return value;
    }

    private void Poll()
    {
        if (_exited || _processId <= 0) return;
        int result = waitpid(_processId, out _waitStatus, WaitNoHang);
        if (result == _processId) { _exited = true; CloseLaunchPidFd(); return; }
        if (result == 0) return;
        int error = Marshal.GetLastWin32Error();
        if (error == 10) { _exited = true; CloseLaunchPidFd(); return; }
        throw new Win32Exception(error, "waitpid(fd-bound direct child, WNOHANG)");
    }

    private void AbortSpawnedChildOnStartFailure()
    {
        if (_processId <= 0 || _exited) { CloseLaunchPidFd(); return; }
        if (_launchPidFd >= 0)
        {
            OxVbaCoreGatePosix.SignalPidFd(_launchPidFd, OxVbaCoreGatePosix.SignalStop);
            OxVbaCoreGatePosix.SignalPidFd(_launchPidFd, OxVbaCoreGatePosix.SignalKill);
        }
        ReapExactSpawnedChildWithoutSignal(_launchPidFd >= 0 ? 5000 : 6500);
        if (!_exited)
            throw new InvalidOperationException("fd-bound Linux child could not be reaped after launch failure");
        CloseLaunchPidFd();
    }

    private void ReapExactSpawnedChildWithoutSignal(int milliseconds)
    {
        var timer = Stopwatch.StartNew();
        do
        {
            int result = waitpid(_processId, out _waitStatus, WaitNoHang);
            if (result == _processId) { _exited = true; return; }
            if (result < 0)
            {
                int error = Marshal.GetLastWin32Error();
                if (error == 10) { _exited = true; return; }
                throw new Win32Exception(error, "waitpid(fd-bound launch-failure child, WNOHANG)");
            }
            Thread.Sleep(2);
        }
        while (timer.ElapsedMilliseconds < milliseconds);
    }

    private void CloseLaunchPidFd()
    {
        int owned = _launchPidFd;
        _launchPidFd = -1;
        if (owned >= 0) OxVbaCoreGatePosix.ClosePidFd(owned);
    }

    private static int DecodeExitCode(int status)
    {
        int signal = status & 0x7f;
        if (signal == 0) return (status >> 8) & 0xff;
        return 128 + signal;
    }

    private static void MaybeWaitAfterInputAdmission(string[] paths, bool windows)
    {
        OxVbaCoreGateInputAdmissionTestHook.WaitIfRequested(paths, windows);
    }

    private static string ProcPath(int fd) { return "/proc/self/fd/" + fd.ToString(CultureInfo.InvariantCulture); }

    private static string RealPath(string path)
    {
        IntPtr pointer = realpath(Path.GetFullPath(path), IntPtr.Zero);
        if (pointer == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error(), "realpath(bound input)");
        try { return Marshal.PtrToStringAnsi(pointer); }
        finally { free(pointer); }
    }

    private static string RequirePinnedLibcIdentity()
    {
        IntPtr pointer;
        try { pointer = gnu_get_libc_version(); }
        catch (EntryPointNotFoundException error)
        {
            throw new PlatformNotSupportedException(
                "fd-bound posix_spawn file actions require the pinned Debian glibc x64 ABI", error);
        }
        catch (DllNotFoundException error)
        {
            throw new PlatformNotSupportedException(
                "fd-bound posix_spawn file actions require the pinned Debian glibc x64 ABI", error);
        }
        if (pointer == IntPtr.Zero)
            throw new PlatformNotSupportedException("gnu_get_libc_version returned no pinned glibc identity");
        var bytes = new List<byte>();
        for (int index = 0; index < 64; index++)
        {
            byte value = Marshal.ReadByte(pointer, index);
            if (value == 0) break;
            bytes.Add(value);
        }
        if (bytes.Count == 0 || bytes.Count == 64)
            throw new PlatformNotSupportedException("gnu_get_libc_version returned an empty or unbounded identity");
        string version;
        try { version = new UTF8Encoding(false, true).GetString(bytes.ToArray()); }
        catch (DecoderFallbackException error)
        {
            throw new PlatformNotSupportedException("gnu_get_libc_version returned a non-UTF-8 identity", error);
        }
        Version parsed;
        if (!Version.TryParse(version, out parsed) || parsed.Major < 1 || parsed.Minor < 0 ||
            version.Any(value => !(value == '.' || (value >= '0' && value <= '9'))))
            throw new PlatformNotSupportedException("gnu_get_libc_version returned an unsupported identity: " + version);
        return "glibc-" + version + "-x64";
    }

    private static int OpenPathFromRoot(string path, int flags, bool root)
    {
        int fd = open(path, flags, 0);
        if (fd < 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "open(fd-bound root)");
        return fd;
    }

    private static int OpenAt2(int directoryFd, string path, int flags, ulong resolve)
    {
        var how = new OpenHow { Flags = checked((ulong)flags), Mode = 0, Resolve = resolve };
        long result = syscall_openat2(SysOpenAt2X64, directoryFd, path, ref how, new UIntPtr((uint)Marshal.SizeOf<OpenHow>()));
        if (result < 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "openat2(fd-bound input)");
        return checked((int)result);
    }

    private static int OpenAt(int directoryFd, string name, int flags, uint mode)
    {
        int fd = openat(directoryFd, name, flags, mode);
        if (fd < 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "openat(fd-bound transport)");
        return fd;
    }

    private static void RequireRegular(int fd, string owner)
    {
        Stat stat;
        if (fstat(fd, out stat) != 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "fstat(" + owner + ")");
        if ((stat.Mode & SIfmt) != SIfreg) throw new InvalidOperationException(owner + " must be a regular file");
    }

    private static void RequireDirectory(int fd, string owner)
    {
        Stat stat;
        if (fstat(fd, out stat) != 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "fstat(" + owner + ")");
        if ((stat.Mode & SIfmt) != SIfdir) throw new InvalidOperationException(owner + " must be a directory");
    }

    private static Dictionary<int, int> AllocateChildDescriptors(IEnumerable<int> sources)
    {
        int[] rows = sources.Distinct().OrderBy(value => value).ToArray();
        var sourceSet = new HashSet<int>(rows);
        long openMax = sysconf(4); // _SC_OPEN_MAX on Linux/glibc.
        if (openMax <= 0 || openMax > int.MaxValue)
            throw new InvalidOperationException("Linux child descriptor limit is unavailable or invalid");
        int candidate = 200;
        var result = new Dictionary<int, int>();
        foreach (int source in rows)
        {
            while (sourceSet.Contains(candidate)) candidate++;
            if (candidate >= openMax)
                throw new InvalidOperationException("Linux child descriptor reservation exceeds _SC_OPEN_MAX");
            result.Add(source, candidate++);
        }
        return result;
    }

    private static void RequireCloseOnExec(IEnumerable<int> descriptors, string owner)
    {
        foreach (int fd in descriptors.Distinct())
        {
            int flags = fcntl(fd, FGetFd, 0);
            if (flags < 0) throw new Win32Exception(Marshal.GetLastWin32Error(), "fcntl(F_GETFD, " + owner + ")");
            if ((flags & FdCloseOnExec) == 0)
                throw new InvalidOperationException(owner + " is ambiently inheritable: fd " +
                    fd.ToString(CultureInfo.InvariantCulture));
        }
    }

    private static void TryUnlinkSame(int directoryFd, string name, int fd)
    {
        if (directoryFd < 0 || fd < 0 || string.IsNullOrEmpty(name)) return;
        Stat held;
        Stat named;
        if (fstat(fd, out held) != 0) return;
        if (fstatat(directoryFd, name, out named, AtSymlinkNoFollow) != 0) return;
        if (held.Device == named.Device && held.Inode == named.Inode) unlinkat(directoryFd, name, 0);
    }

    private static void CloseFd(ref int fd)
    {
        int owned = fd;
        fd = -1;
        if (owned >= 0) close(owned);
    }

    private void EnsureLive()
    {
        if (_disposed) throw new ObjectDisposedException(nameof(OxVbaCoreGatePosixChild));
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct OpenHow
    {
        public ulong Flags;
        public ulong Mode;
        public ulong Resolve;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Timespec
    {
        public long Seconds;
        public long Nanoseconds;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Stat
    {
        public ulong Device;
        public ulong Inode;
        public ulong LinkCount;
        public uint Mode;
        public uint UserId;
        public uint GroupId;
        public int Padding;
        public ulong RDevice;
        public long Size;
        public long BlockSize;
        public long Blocks;
        public Timespec AccessTime;
        public Timespec ModifyTime;
        public Timespec ChangeTime;
        public long Reserved0;
        public long Reserved1;
        public long Reserved2;
    }

    [DllImport("libc", SetLastError = true)]
    private static extern int open(string path, int flags, uint mode);

    [DllImport("libc", SetLastError = true)]
    private static extern int openat(int directoryFd, string path, int flags, uint mode);

    [DllImport("libc", SetLastError = true)]
    private static extern int close(int fd);

    [DllImport("libc", SetLastError = true)]
    private static extern long lseek(int fd, long offset, int origin);

    [DllImport("libc", SetLastError = true)]
    private static extern int ftruncate(int fd, long length);

    [DllImport("libc", SetLastError = true)]
    private static extern int fchmod(int fd, uint mode);

    [DllImport("libc", SetLastError = true)]
    private static extern int fsync(int fd);

    [DllImport("libc", SetLastError = true)]
    private static extern long write(int fd, IntPtr buffer, UIntPtr count);

    [DllImport("libc", SetLastError = true)]
    private static extern long pread(int fd, IntPtr buffer, UIntPtr count, long fileOffset);

    [DllImport("libc", SetLastError = true)]
    private static extern int fcntl(int fd, int command, int value);

    [DllImport("libc", SetLastError = true)]
    private static extern long sysconf(int name);

    [DllImport("libc", SetLastError = true)]
    private static extern int fstat(int fd, out Stat stat);

    [DllImport("libc", SetLastError = true)]
    private static extern int fstatat(int directoryFd, string path, out Stat stat, int flags);

    [DllImport("libc", SetLastError = true)]
    private static extern int unlinkat(int directoryFd, string path, int flags);

    [DllImport("libc", SetLastError = true)]
    private static extern int waitpid(int pid, out int status, int options);

    [DllImport("libc", SetLastError = true)]
    private static extern IntPtr realpath(string path, IntPtr resolvedPath);

    [DllImport("libc")]
    private static extern void free(IntPtr pointer);

    [DllImport("libc", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr gnu_get_libc_version();

    [DllImport("libc", SetLastError = true)]
    private static extern int posix_spawn(out int pid, string path, IntPtr fileActions, IntPtr attributes, IntPtr argv, IntPtr envp);

    [DllImport("libc", SetLastError = true)]
    private static extern int posix_spawn_file_actions_init(IntPtr fileActions);

    [DllImport("libc", SetLastError = true)]
    private static extern int posix_spawn_file_actions_adddup2(IntPtr fileActions, int sourceFd, int childFd);

    [DllImport("libc", SetLastError = true)]
    private static extern int posix_spawn_file_actions_destroy(IntPtr fileActions);

    [DllImport("libc", EntryPoint = "syscall", SetLastError = true, CharSet = CharSet.Ansi)]
    private static extern long syscall_openat2(long number, int directoryFd, string path, ref OpenHow how, UIntPtr size);

    [DllImport("libc", EntryPoint = "syscall", SetLastError = true, CharSet = CharSet.Ansi)]
    private static extern long syscall_memfd_create(long number, string name, uint flags);
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

    // Returns "pid:comm" for every live owned descendant. Used only after the
    // direct child exits to distinguish manifest-declared ambient toolchain
    // helpers from a genuine surviving descendant, matching the Windows
    // GetMemberImageNames surface.
    public List<string> GetLiveProcessNames()
    {
        var names = new List<string>();
        foreach (ProcRecord record in Refresh())
        {
            string comm = null;
            try { comm = File.ReadAllText("/proc/" + record.Pid.ToString(CultureInfo.InvariantCulture) + "/comm").Trim(); }
            catch { /* exited between discovery and read */ }
            names.Add(record.Pid.ToString(CultureInfo.InvariantCulture) + ":" + (comm ?? "?"));
        }
        return names;
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
