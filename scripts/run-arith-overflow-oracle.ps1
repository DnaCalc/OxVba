# Captures Excel/VBA arithmetic-overflow ground truth for bd-0d1y.
#
# Single Excel session: injects a probe module, runs each case under On Error Resume Next, and
# records either the error number or `TypeName(r):value`. The OxVba VM is asserted against these
# outcomes by crates/oxvba-vm/tests/vm_feature_coverage.rs (overflow_* tests). Re-run to refresh
# the ground truth in docs/evidence/conformance/OVERFLOW_ARITHMETIC_ORACLE_*.md.
#
# Requires Excel with "Trust access to the VBA project object model" (AccessVBOM=1).
param(
    [string]$OutFile = ""
)
$ErrorActionPreference = "Stop"
if (-not $IsWindows) { throw "arithmetic-overflow oracle runner is Windows-only" }

$vba = @'
Function Rz(e As Long, val As Variant) As String
  If e <> 0 Then
    Rz = "ERR" & e
  Else
    Rz = TypeName(val) & ":" & CStr(val)
  End If
End Function

Public Function RunProbes() As String
  Dim ai As Integer, al As Long, ab As Byte, v As Variant, r As Variant, out As String
  On Error Resume Next
  Err.Clear: ai = 32767: ai = ai + 1
  out = out & "INT_ADD_ASSIGN=" & Rz(Err.Number, ai) & vbLf
  Err.Clear: ai = 32767: r = ai + 1
  out = out & "INT_ADD_EXPR=" & Rz(Err.Number, r) & vbLf
  Err.Clear: ai = 100: r = ai + 1
  out = out & "INT_NOOVF=" & Rz(Err.Number, r) & vbLf
  Err.Clear: ai = -32768: ai = ai - 1
  out = out & "INT_SUB_ASSIGN=" & Rz(Err.Number, ai) & vbLf
  Err.Clear: ai = 1000: r = ai * 1000
  out = out & "INT_MUL_EXPR=" & Rz(Err.Number, r) & vbLf
  Err.Clear: ai = -32768: r = -ai
  out = out & "INT_NEG_EXPR=" & Rz(Err.Number, r) & vbLf
  Err.Clear: al = 2000000000: al = al + 2000000000
  out = out & "LNG_ADD_ASSIGN=" & Rz(Err.Number, al) & vbLf
  Err.Clear: al = 2000000000: r = al + al
  out = out & "LNG_ADD_EXPR=" & Rz(Err.Number, r) & vbLf
  Err.Clear: al = 5: r = al + 1
  out = out & "LNG_NOOVF=" & Rz(Err.Number, r) & vbLf
  Err.Clear: al = 50000: r = al * 50000
  out = out & "LNG_MUL_EXPR=" & Rz(Err.Number, r) & vbLf
  Err.Clear: al = 2000000000: r = (al + al) Mod 7
  out = out & "LNG_MOD_INTERMEDIATE=" & Rz(Err.Number, r) & vbLf
  Err.Clear: ab = 200: ab = ab + 100
  out = out & "BYTE_ADD_ASSIGN=" & Rz(Err.Number, ab) & vbLf
  Err.Clear: ab = 200: r = ab + 100
  out = out & "BYTE_ADD_EXPR=" & Rz(Err.Number, r) & vbLf
  Err.Clear: v = 5: r = v + 1
  out = out & "VAR_INT_NOOVF=" & Rz(Err.Number, r) & vbLf
  Err.Clear: v = 32767: r = v + 1
  out = out & "VAR_INT_WIDEN=" & Rz(Err.Number, r) & vbLf
  Err.Clear: v = 2000000000: r = v + v
  out = out & "VAR_LNG_WIDEN=" & Rz(Err.Number, r) & vbLf
  Err.Clear: v = 50000: r = v * 50000
  out = out & "VAR_MUL_WIDEN=" & Rz(Err.Number, r) & vbLf
  RunProbes = out
End Function
'@

$excel = $null; $wb = $null
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false; $excel.DisplayAlerts = $false
    $wb = $excel.Workbooks.Add()
    $mod = $wb.VBProject.VBComponents.Add(1)
    $mod.Name = "M"
    [void]$mod.CodeModule.AddFromString($vba)
    $res = [string]$excel.Run("M.RunProbes")
    Write-Output $res
    if ($OutFile) { Set-Content -Path $OutFile -Value $res }
} finally {
    if ($wb) { $wb.Close($false); [void][Runtime.InteropServices.Marshal]::ReleaseComObject($wb) }
    if ($excel) { $excel.Quit(); [void][Runtime.InteropServices.Marshal]::ReleaseComObject($excel) }
}
