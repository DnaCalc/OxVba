Sub Main()
Dim x
x = 3
End Sub
Sub Class_Initialize()
On Error Resume Next
Err.Raise 5
End Sub
Sub Class_Terminate()
On Error Resume Next
Err.Raise 7
End Sub
