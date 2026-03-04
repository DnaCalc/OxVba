Option Explicit

Sub Main()
Dim total As Long
Dim arrScore As Long
Dim host As Long
total = MathTools.SumUpTo(4)
arrScore = ArrayTools.Score()
On Error Resume Next
host = Shell(2)
total = total + 8
Call LibMath.Boost.Tap(total)
End Sub
