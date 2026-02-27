Sub Main()
On Error GoTo handler
On Error GoTo 0
Error 4

If Err.Number = -1 Then
handler:
Resume Next
End If
End Sub
