Option Explicit

Sub Main()
Dim x As Long
Dim y As Long
Dim z As Long
x = Utils.SafeAdd(3, 2)
On Error Resume Next
Err.Raise 77
y = Err.Number
Err.Clear
z = Utils.SafeAdd(4, 6)
End Sub
