# Captures Excel/VBA Class_Terminate timing ground truth (Phase 2 of project-object lifetime).
#
# Two probes, each logging ordered markers into a module string:
#   A: `Set a = New Foo : log"1" : Set a = Nothing : log"2"`  -> shows Terminate fires AT the
#      statement that drops the last reference (between "1" and "2").  Expected: 1T2
#   B: `s = MakeFoo().Tag & Mark()`  -> shows an expression TEMPORARY is held to the end of the
#      statement, not released after its last sub-use.  Markers: g=Tag read, M=Mark called,
#      T=Terminate.  Expected: gMT  (end-of-statement). Intra-statement release would be gTM.
#
# Requires Excel with "Trust access to the VBA project object model" (AccessVBOM=1).
param([string]$OutFile = "")
$ErrorActionPreference = "Stop"
if (-not $IsWindows) { throw "Class_Terminate timing oracle runner is Windows-only" }

$cls = @'
Private Sub Class_Terminate()
  Append "T"
End Sub
Public Function Tag() As String
  Append "g"
  Tag = "x"
End Function
'@
$std = @'
Public gLog As String
Public Sub Append(ByVal s As String)
  gLog = gLog & s
End Sub
Public Function MakeFoo() As Foo
  Set MakeFoo = New Foo
End Function
Public Function Mark() As String
  Append "M"
  Mark = ""
End Function
Public Function RunProbeA() As String
  gLog = ""
  Dim a As Foo
  Set a = New Foo
  Append "1"
  Set a = Nothing
  Append "2"
  RunProbeA = gLog
End Function
Public Function RunProbeB() As String
  gLog = ""
  Dim s As String
  s = MakeFoo().Tag & Mark()
  RunProbeB = gLog & "|after=" & s
End Function
Public Function RunProbes() As String
  RunProbes = "A=" & RunProbeA() & "  B=" & RunProbeB()
End Function
'@

$excel = $null; $wb = $null
try {
  $excel = New-Object -ComObject Excel.Application
  $excel.Visible = $false; $excel.DisplayAlerts = $false
  $wb = $excel.Workbooks.Add()
  $foo = $wb.VBProject.VBComponents.Add(2)   # class module
  $foo.Name = "Foo"
  [void]$foo.CodeModule.AddFromString($cls)
  $m = $wb.VBProject.VBComponents.Add(1)     # standard module
  $m.Name = "MainModule"
  [void]$m.CodeModule.AddFromString($std)
  $res = [string]$excel.Run("RunProbes")
  Write-Output $res
  if ($OutFile) { Set-Content -Path $OutFile -Value $res }
} finally {
  if ($wb) { $wb.Close($false); [void][Runtime.InteropServices.Marshal]::ReleaseComObject($wb) }
  if ($excel) { $excel.Quit(); [void][Runtime.InteropServices.Marshal]::ReleaseComObject($excel) }
}
