param(
    [string]$OutputDir = "",
    [string[]]$IncludePattern = @(),
    [string[]]$ExcludePattern = @(),
    [switch]$DisableDialogGuardian,
    [int]$TestTimeoutMs = 15000,
    [switch]$SkipRegistryCheck
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $workspaceRoot = (Resolve-Path ".").Path
    $stamp = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    $runDir = if ($OutputDir) {
        if ([System.IO.Path]::IsPathRooted($OutputDir)) { $OutputDir }
        else { Join-Path $workspaceRoot $OutputDir }
    } else {
        Join-Path $workspaceRoot "docs/evidence/conformance/oracle_captures/conformance_oracle_$stamp"
    }
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    # ── pre-flight ────────────────────────────────────────────────────────
    if (-not $SkipRegistryCheck) {
        $regPath = "HKCU:\Software\Microsoft\Office\16.0\Excel\Security"
        $accessVBOM = $null
        try {
            $accessVBOM = (Get-ItemProperty -Path $regPath -Name "AccessVBOM" -ErrorAction Stop).AccessVBOM
        } catch {}
        if ($accessVBOM -ne 1) {
            Write-Warning "AccessVBOM registry value is not set to 1. VBProject access will fail."
            Write-Warning "Enable via: Excel > File > Options > Trust Center > Trust Center Settings > Macro Settings > Trust access to the VBA project object model"
        }
    }

    # ── golden reference ──────────────────────────────────────────────────
    $goldenFile = "conformance/golden/smoke.csv"
    if (-not (Test-Path $goldenFile)) {
        throw "Missing golden file: $goldenFile"
    }
    $golden = Import-Csv $goldenFile
    $goldenMap = @{}
    foreach ($g in $golden) {
        if ($g.file -and $g.status) {
            $goldenMap[$g.file] = $g
        }
    }

    # ── helper: non-portable detection ────────────────────────────────────
    function Test-NonPortable {
        param([string]$Source)
        $patterns = @(
            '\bDispatchInvoke\b',
            '\bCreateObject\s*\(\s*\d',
            '\bCollectionAdd\b',
            '\bCollectionItem\b',
            '\bCollectionCount\b',
            '\bCollectionRemove\b',
            'Declare\s+.*\s+Lib\s+"host"',
            '\bShell\s*\(\s*\d',
            '\bEnviron\s*\(\s*\d',
            '\bDir\s*\(\s*\d'
        )
        foreach ($p in $patterns) {
            if ($Source -match $p) { return $true }
        }
        $false
    }

    # ── EncodeSlot VBA helper (injected into every harness module) ────────
    # Constants mirror crates/oxvba-runtime/src/value_tags.rs:
    #   EMPTY_TAG = 0, NULL_TAG = -1, ERROR_TAG_BASE = -900000000
    $encodeSlotCode = @'
Private Function EncodeSlot(ByVal v As Variant) As String
    If IsEmpty(v) Then
        EncodeSlot = "0"
    ElseIf IsNull(v) Then
        EncodeSlot = "-1"
    ElseIf IsError(v) Then
        Dim ec As Long
        ec = CLng(v)
        If ec < 0 Then ec = -ec
        EncodeSlot = CStr(CLng(-900000000) + ec)
    ElseIf VarType(v) = vbBoolean Then
        If v Then EncodeSlot = "1" Else EncodeSlot = "0"
    Else
        On Error Resume Next
        EncodeSlot = CStr(CLng(v))
        If Err.Number <> 0 Then
            Err.Clear
            EncodeSlot = "0"
        End If
        On Error GoTo 0
    End If
End Function
'@

    # ── harness generator ─────────────────────────────────────────────────
    function ConvertTo-OracleHarness {
        param([string]$Source)

        $lines = $Source -split "`r?`n"
        $mainStart = -1
        $mainEnd = -1

        for ($i = 0; $i -lt $lines.Count; $i++) {
            if ($lines[$i] -match '^\s*Sub\s+Main\s*\(\s*\)') {
                $mainStart = $i
                break
            }
        }
        if ($mainStart -lt 0) { return $null }

        for ($i = $mainStart + 1; $i -lt $lines.Count; $i++) {
            if ($lines[$i] -match '^\s*End\s+Sub\b') {
                $mainEnd = $i
                break
            }
        }
        if ($mainEnd -lt 0) { return $null }

        # Module-level code: everything outside Sub Main…End Sub
        $moduleLevel = @()
        for ($i = 0; $i -lt $mainStart; $i++) { $moduleLevel += $lines[$i] }
        for ($i = $mainEnd + 1; $i -lt $lines.Count; $i++) { $moduleLevel += $lines[$i] }

        # Main body (between Sub Main and End Sub, exclusive)
        $body = @()
        for ($i = $mainStart + 1; $i -lt $mainEnd; $i++) { $body += $lines[$i] }

        # Extract Dim variable names in declaration order
        $dimVars = @()
        foreach ($line in $body) {
            if ($line -match '^\s*Dim\s+(\w+)') {
                $dimVars += $Matches[1]
            }
        }

        # Build slot capture expression
        if ($dimVars.Count -gt 0) {
            $parts = $dimVars | ForEach-Object { "EncodeSlot($_)" }
            $slotExpr = "    RunProbe = " + ($parts -join ' & "," & ')
        } else {
            $slotExpr = '    RunProbe = ""'
        }

        # Assemble harness
        # RunProbe uses On Error GoTo to catch runtime errors that would
        # otherwise open the VBE in break mode and block Application.Run.
        # Runtime errors return "RTMERR:<number>" which the oracle interprets
        # as status = "error".
        $harness = @()
        $moduleTrimmed = ($moduleLevel -join "`n").Trim()
        if ($moduleTrimmed) {
            $harness += $moduleTrimmed
            $harness += ""
        }
        $harness += $encodeSlotCode
        $harness += ""
        $harness += "Public Function RunProbe() As String"
        $harness += "    On Error GoTo RunProbeErr"
        foreach ($line in $body) { $harness += $line }
        $harness += $slotExpr
        $harness += "    Exit Function"
        $harness += "RunProbeErr:"
        $harness += '    RunProbe = "RTMERR:" & Err.Number'
        $harness += "End Function"

        ($harness -join "`n")
    }

    # ── COM helpers ───────────────────────────────────────────────────────
    function Add-StdModule {
        param($Workbook, [string]$ModuleName, [string]$Code)
        $component = $Workbook.VBProject.VBComponents.Add(1)
        $component.Name = $ModuleName
        $null = $component.CodeModule.AddFromString($Code)
    }

    function Invoke-OracleTest {
        param($Excel, [string]$HarnessCode, [int]$TimeoutMs)

        $wb = $null
        try {
            $wb = $Excel.Workbooks.Add()
            Add-StdModule -Workbook $wb -ModuleName "OracleHarness" -Code $HarnessCode
            # Write deadline so the VBA handler can kill Excel on timeout
            $deadlineTicks = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs).Ticks
            Set-Content -Path $deadlineFile -Value $deadlineTicks -Force
            try {
                $result = [string]$Excel.Run("RunProbe")
                # Check for runtime error marker injected by On Error GoTo
                if ($result -match '^RTMERR:') {
                    @{ status = "error"; slots = ""; message = "VBA runtime error $result" }
                } else {
                    @{ status = "ok"; slots = $result }
                }
            } catch {
                @{ status = "error"; slots = ""; message = $_.Exception.Message }
            } finally {
                # Clear deadline regardless of success/failure
                if (Test-Path $deadlineFile) { Remove-Item -Force $deadlineFile -ErrorAction SilentlyContinue }
            }
        } catch {
            @{ status = "error"; slots = ""; message = $_.Exception.Message }
        } finally {
            if ($null -ne $wb) {
                try { $wb.Close($false) } catch {}
                try { [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb) } catch {}
            }
        }
    }

    # ── VBA error dialog handler script (inline) ─────────────────────────
    # Handles compile error / runtime error dialogs that Application.Run
    # triggers when injected VBA code is invalid.  The security guardian
    # only handles trust-center dialogs; this covers the VBE modal dialogs
    # that would otherwise block the COM call indefinitely.
    $vbaDialogHandlerScript = @'
param([int]$ExcelPid, $StopFile, $LogFile, $DeadlineFile, [int]$PollMs = 200)
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class VbeWin32 {
    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool PostMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
    public const uint BM_CLICK  = 0x00F5;
    public const uint WM_CLOSE  = 0x0010;
}
"@

function Write-Log { param([string]$Msg)
    Add-Content -Path $LogFile -Value "$(Get-Date -Format o) $Msg" -Encoding UTF8
}

Write-Log "start excel_pid=$ExcelPid poll_ms=$PollMs stop_file=$StopFile"
$root = [System.Windows.Automation.AutomationElement]::RootElement

while ($true) {
    if (Test-Path $StopFile) { Write-Log "stop file observed"; break }
    if (-not (Get-Process -Id $ExcelPid -ErrorAction SilentlyContinue)) { Write-Log "excel exited"; break }

    # Deadline watchdog: kill Excel if a test exceeds its timeout.
    # The oracle writes the deadline (UTC ticks) to $DeadlineFile before
    # each Application.Run call and deletes it after the call returns.
    if ($DeadlineFile -and (Test-Path $DeadlineFile)) {
        try {
            $deadlineTicks = [long](Get-Content $DeadlineFile -Raw).Trim()
            if ([DateTime]::UtcNow.Ticks -gt $deadlineTicks) {
                Write-Log "deadline exceeded — killing excel pid=$ExcelPid"
                Stop-Process -Id $ExcelPid -Force -ErrorAction SilentlyContinue
                break
            }
        } catch {}
    }

    try {
        $pidCond  = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ExcelPid)
        $winCond  = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            [System.Windows.Automation.ControlType]::Window)
        $combined = New-Object System.Windows.Automation.AndCondition($pidCond, $winCond)
        $windows  = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $combined)

        for ($i = 0; $i -lt $windows.Count; $i++) {
            $w = $windows.Item($i)
            $title = ""
            try { $title = [string]$w.Current.Name } catch { continue }
            $sig = $title.ToLowerInvariant()

            if ($sig -notmatch 'microsoft visual basic') { continue }

            # Phase 1: find and click OK on the compile error child dialog
            # (child window named "Microsoft Visual Basic for Applications"
            #  with OK/Help buttons).
            $trueCond = [System.Windows.Automation.Condition]::TrueCondition
            $children = $w.FindAll([System.Windows.Automation.TreeScope]::Children, $trueCond)
            $clickedOK = $false
            for ($j = 0; $j -lt $children.Count; $j++) {
                $c = $children.Item($j)
                $cn = ""; try { $cn = $c.Current.Name } catch {}
                if ($cn -ne "Microsoft Visual Basic for Applications") { continue }

                $btnCond = New-Object System.Windows.Automation.PropertyCondition(
                    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
                    [System.Windows.Automation.ControlType]::Button)
                $btns = $c.FindAll([System.Windows.Automation.TreeScope]::Children, $btnCond)
                for ($k = 0; $k -lt $btns.Count; $k++) {
                    $btn = $btns.Item($k)
                    $bname = ""; try { $bname = $btn.Current.Name } catch {}
                    if ($bname -eq "OK") {
                        $bHwnd = [IntPtr]::new($btn.Current.NativeWindowHandle)
                        [void][VbeWin32]::PostMessage($bHwnd, [VbeWin32]::BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero)
                        Write-Log "clicked compile-error OK: window='$title'"
                        $clickedOK = $true
                        break
                    }
                }
                break
            }

            if ($clickedOK) {
                Start-Sleep -Milliseconds 500
            }

            # Phase 2: if VBE is now in break mode, close it via WM_CLOSE.
            # This terminates the VBA execution and makes Application.Run
            # return a COM error to the oracle runner.
            if ($sig -match '\[break\]' -or $clickedOK) {
                $vbeHwnd = [IntPtr]::new($w.Current.NativeWindowHandle)
                if ($vbeHwnd -ne [IntPtr]::Zero) {
                    [void][VbeWin32]::PostMessage($vbeHwnd, [VbeWin32]::WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
                    Write-Log "sent WM_CLOSE to VBE: window='$title'"
                }
            }
        }
    } catch {}

    Start-Sleep -Milliseconds $PollMs
}
Write-Log "exit"
'@

    # ── Excel lifecycle ──────────────────────────────────────────────────
    $dialogGuardianLog = Join-Path $runDir "excel_dialog_guardian.log"
    $dialogGuardianStop = Join-Path $runDir "excel_dialog_guardian.stop"
    $vbaDialogHandlerLog = Join-Path $runDir "vba_dialog_handler.log"
    $vbaDialogHandlerStop = Join-Path $runDir "vba_dialog_handler.stop"
    $vbaHandlerScriptFile = Join-Path $runDir "_vba_dialog_handler.ps1"
    $deadlineFile = Join-Path $runDir "_test_deadline.txt"
    Set-Content -Path $vbaHandlerScriptFile -Value $vbaDialogHandlerScript -Encoding UTF8

    if (-not ([System.Management.Automation.PSTypeName]'Win32User32PidOracle').Type) {
        Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win32User32PidOracle {
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@
    }

    function Get-WindowProcessId {
        param([int]$Hwnd)
        [uint32]$windowPid = 0
        [void][Win32User32PidOracle]::GetWindowThreadProcessId([IntPtr]::new($Hwnd), [ref]$windowPid)
        [int]$windowPid
    }

    # Holds current Excel state; closures capture $script:excel etc.
    $script:excel = $null
    $script:excelPid = 0
    $script:excelVersion = ""
    $script:guardianProc = $null
    $script:vbaHandlerProc = $null

    function Start-ExcelSession {
        # Stop previous handlers if any
        Stop-ExcelHandlers

        $script:excel = New-Object -ComObject Excel.Application
        $script:excel.Visible = $false
        $script:excel.DisplayAlerts = $false
        $script:excelVersion = $script:excel.Version
        $script:excelPid = Get-WindowProcessId -Hwnd ([int]$script:excel.Hwnd)
        Write-Host "excel: started (pid=$($script:excelPid), version=$($script:excelVersion))"

        if (-not $DisableDialogGuardian -and $script:excelPid -gt 0) {
            foreach ($sf in @($dialogGuardianStop, $vbaDialogHandlerStop)) {
                if (Test-Path $sf) { Remove-Item -Force $sf }
            }
            $guardianScript = Join-Path $PSScriptRoot "excel-dialog-guardian.ps1"
            $script:guardianProc = Start-Process -FilePath "pwsh" -WindowStyle Hidden -PassThru -ArgumentList @(
                "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass",
                "-File", $guardianScript,
                "-ExcelPid", "$($script:excelPid)",
                "-StopFile", $dialogGuardianStop,
                "-LogFile", $dialogGuardianLog,
                "-PollMs", "250", "-MaxSeconds", "1800"
            )
            $script:vbaHandlerProc = Start-Process -FilePath "pwsh" -WindowStyle Hidden -PassThru -ArgumentList @(
                "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass",
                "-File", $vbaHandlerScriptFile,
                "-ExcelPid", "$($script:excelPid)",
                "-StopFile", $vbaDialogHandlerStop,
                "-LogFile", $vbaDialogHandlerLog,
                "-DeadlineFile", $deadlineFile,
                "-PollMs", "200"
            )
            Write-Host "  guardian=$($script:guardianProc.Id) vba-handler=$($script:vbaHandlerProc.Id)"
        }
    }

    function Stop-ExcelHandlers {
        foreach ($entry in @(
            @{ proc = $script:guardianProc;    stop = $dialogGuardianStop },
            @{ proc = $script:vbaHandlerProc;  stop = $vbaDialogHandlerStop }
        )) {
            if ($null -ne $entry.proc) {
                New-Item -ItemType File -Path $entry.stop -Force | Out-Null
                Start-Sleep -Milliseconds 300
                if (-not $entry.proc.HasExited) {
                    Stop-Process -Id $entry.proc.Id -Force -ErrorAction SilentlyContinue
                }
                if (Test-Path $entry.stop) {
                    Remove-Item -Force $entry.stop -ErrorAction SilentlyContinue
                }
            }
        }
        $script:guardianProc = $null
        $script:vbaHandlerProc = $null
    }

    function Test-ExcelAlive {
        try { $null = $script:excel.Version; $true } catch { $false }
    }

    # ── start Excel ───────────────────────────────────────────────────────
    Start-ExcelSession

    try {
        # ── enumerate tests ───────────────────────────────────────────────
        $testsDir = "conformance/tests"
        if (-not (Test-Path $testsDir)) {
            throw "Missing conformance test directory: $testsDir"
        }
        $testFiles = @(Get-ChildItem -Path $testsDir -Filter *.bas | Sort-Object Name)

        if ($IncludePattern -and $IncludePattern.Count -gt 0) {
            $testFiles = @($testFiles | Where-Object {
                $name = $_.Name
                foreach ($p in $IncludePattern) {
                    if ($name -like $p) { return $true }
                }
                return $false
            })
        }

        if ($ExcludePattern -and $ExcludePattern.Count -gt 0) {
            $testFiles = @($testFiles | Where-Object {
                $name = $_.Name
                foreach ($p in $ExcludePattern) {
                    if ($name -like $p) { return $false }
                }
                return $true
            })
        }

        $rows = New-Object System.Collections.Generic.List[object]
        $totalCount = $testFiles.Count
        $index = 0

        foreach ($tf in $testFiles) {
            $index++
            $fileName = $tf.Name
            $source = Get-Content -Raw $tf.FullName

            $goldenEntry = $goldenMap[$fileName]
            $goldenStatus = if ($goldenEntry) { $goldenEntry.status } else { "" }
            $goldenSlots = if ($goldenEntry -and $goldenEntry.slots) { $goldenEntry.slots } else { "" }

            Write-Host "[$index/$totalCount] $fileName" -NoNewline

            # Skip non-portable tests
            if (Test-NonPortable $source) {
                Write-Host " SKIP (non-portable)"
                $rows.Add([PSCustomObject]@{
                    file          = $fileName
                    oracle_status = "skip"
                    oracle_slots  = ""
                    golden_status = $goldenStatus
                    golden_slots  = $goldenSlots
                    match         = ""
                    notes         = "non-portable (OxVba synthetic builtins)"
                }) | Out-Null
                continue
            }

            # Generate harness
            $harness = ConvertTo-OracleHarness $source
            if ($null -eq $harness) {
                Write-Host " SKIP (no Sub Main)"
                $rows.Add([PSCustomObject]@{
                    file          = $fileName
                    oracle_status = "skip"
                    oracle_slots  = ""
                    golden_status = $goldenStatus
                    golden_slots  = $goldenSlots
                    match         = ""
                    notes         = "harness generation failed (no Sub Main found)"
                }) | Out-Null
                continue
            }

            # Execute in Excel
            $result = Invoke-OracleTest -Excel $script:excel -HarnessCode $harness -TimeoutMs $TestTimeoutMs
            $oracleStatus = $result.status
            $oracleSlots = $result.slots

            # Restart Excel if it died or entered a corrupt state (VBE compile
            # error handling by the handler process may have killed it).
            $needsRestart = -not (Test-ExcelAlive)
            if (-not $needsRestart -and $oracleStatus -eq "error" -and $result.message -match '0x800|RPC|disconnected|E_FAIL') {
                $needsRestart = $true
            }
            if ($needsRestart) {
                Write-Host " (restarting Excel)"
                try { $script:excel.Quit() } catch {}
                try { [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($script:excel) } catch {}
                Start-Sleep -Milliseconds 1000
                Start-ExcelSession
            }

            # Determine match against golden
            $matchResult = ""
            if ($goldenEntry) {
                if ($oracleStatus -eq $goldenStatus) {
                    if ($oracleStatus -eq "ok") {
                        $matchResult = if ($oracleSlots -eq $goldenSlots) { "true" } else { "false" }
                    } elseif ($oracleStatus -eq "error") {
                        $matchResult = "true"
                    }
                } else {
                    $matchResult = "false"
                }
            }

            $statusTag = if ($matchResult -eq "true") { "MATCH" }
                         elseif ($matchResult -eq "false") { "MISMATCH" }
                         else { "?" }
            Write-Host " $oracleStatus [$statusTag]"

            $notes = ""
            if ($matchResult -eq "false") {
                $notes = "oracle=$oracleStatus/$oracleSlots golden=$goldenStatus/$goldenSlots"
            }
            if ($result.message -and $oracleStatus -eq "error") {
                if ($notes) { $notes += "; " }
                $notes += $result.message
            }

            $rows.Add([PSCustomObject]@{
                file          = $fileName
                oracle_status = $oracleStatus
                oracle_slots  = $oracleSlots
                golden_status = $goldenStatus
                golden_slots  = $goldenSlots
                match         = $matchResult
                notes         = $notes
            }) | Out-Null
        }

        # ── write results CSV ─────────────────────────────────────────────
        $csvPath = Join-Path $runDir "results.csv"
        $rows | Export-Csv -Path $csvPath -NoTypeInformation

        # ── write summary markdown ────────────────────────────────────────
        $summaryPath = Join-Path $runDir "summary.md"
        $total = $rows.Count
        $matchCount = @($rows | Where-Object { $_.match -eq "true" }).Count
        $mismatchCount = @($rows | Where-Object { $_.match -eq "false" }).Count
        $skipCount = @($rows | Where-Object { $_.oracle_status -eq "skip" }).Count
        $errorCount = @($rows | Where-Object { $_.oracle_status -eq "error" }).Count
        $okCount = @($rows | Where-Object { $_.oracle_status -eq "ok" }).Count

        $md = @()
        $md += "# Conformance Oracle Run"
        $md += ""
        $md += "- Timestamp (UTC): $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))"
        $md += "- Excel version: $excelVersion"
        $md += "- Excel process id: $excelPid"
        $md += "- Dialog guardian enabled: $([string](-not $DisableDialogGuardian))"
        if (-not $DisableDialogGuardian) {
            $md += "- Dialog guardian log: $dialogGuardianLog"
        }
        $md += "- Output CSV: $csvPath"
        $md += ""
        $md += "## Summary"
        $md += ""
        $md += "| Metric | Count |"
        $md += "|--------|-------|"
        $md += "| Total tests | $total |"
        $md += "| Oracle OK | $okCount |"
        $md += "| Oracle error | $errorCount |"
        $md += "| Skipped | $skipCount |"
        $md += "| **Match** | **$matchCount** |"
        $md += "| **Mismatch** | **$mismatchCount** |"
        $md += ""

        $mismatches = @($rows | Where-Object { $_.match -eq "false" })
        if ($mismatches.Count -gt 0) {
            $md += "## Mismatches"
            $md += ""
            $md += "| File | Oracle | Golden | Notes |"
            $md += "|------|--------|--------|-------|"
            foreach ($m in $mismatches) {
                $oracleCell = "$($m.oracle_status): ``$($m.oracle_slots)``"
                $goldenCell = "$($m.golden_status): ``$($m.golden_slots)``"
                $md += "| $($m.file) | $oracleCell | $goldenCell | $($m.notes) |"
            }
            $md += ""
        }

        $md += "## All Results"
        $md += ""
        $md += "| File | Oracle Status | Oracle Slots | Golden Status | Golden Slots | Match |"
        $md += "|------|-------------|-------------|--------------|-------------|-------|"
        foreach ($row in $rows) {
            $md += "| $($row.file) | $($row.oracle_status) | ``$($row.oracle_slots)`` | $($row.golden_status) | ``$($row.golden_slots)`` | $($row.match) |"
        }

        Set-Content -Path $summaryPath -Value ($md -join [Environment]::NewLine)

        Write-Host ""
        Write-Host "conformance-oracle: complete"
        Write-Host "  total=$total ok=$okCount error=$errorCount skip=$skipCount"
        Write-Host "  match=$matchCount mismatch=$mismatchCount"
        Write-Host "  run_dir=$runDir"
        Write-Host "  csv=$csvPath"
        Write-Host "  summary=$summaryPath"

    } finally {
        # ── teardown ──────────────────────────────────────────────────────
        Stop-ExcelHandlers
        if (Test-ExcelAlive) {
            $script:excel.Quit()
            [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($script:excel)
        }
    }
} finally {
    Pop-Location
}
