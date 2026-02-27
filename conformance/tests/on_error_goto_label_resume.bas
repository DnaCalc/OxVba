Sub Main()
Dim x
x = 1
On Error GoTo handler
Error 5
x = 99

If Err.Number = -1 Then
handler:
x = Err.Number
Resume Next
End If

x = x + 1
End Sub
