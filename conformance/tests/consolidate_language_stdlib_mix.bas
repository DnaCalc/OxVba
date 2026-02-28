Sub Main()
Dim a
Dim b
a = DateDiff(1, DateSerial(2026, 2, 28), DateAdd(1, 3, DateSerial(2026, 2, 28)))
b = DispatchInvoke(CreateObject(2), 3, a)
End Sub
Sub Class_Initialize()
On Error Resume Next
Err.Raise 5
End Sub
