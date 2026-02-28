Sub Main()
Dim x
On Error Resume Next
Err.Raise 11
x = Err.Number
End Sub
