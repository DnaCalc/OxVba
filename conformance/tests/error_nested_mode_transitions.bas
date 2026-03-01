Sub Main()
Dim a
Dim b
Dim c
Dim d
On Error Resume Next
Error 5
a = Err.Number
Resume Next
b = Err.Number
On Error GoTo Handler
Error 6
c = Err.Number
GoTo Done
Handler:
d = Err.Number
Resume Next
Done:
End Sub
