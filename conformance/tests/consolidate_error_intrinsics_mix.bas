Sub Main()
Dim x
Dim y
Dim z
On Error Resume Next
Err.Raise 12
x = Err.Number
y = CVErr(4)
z = IsDate(DateSerial(2026, 2, 28))
End Sub
