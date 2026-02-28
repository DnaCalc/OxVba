Sub Main()
Dim x
GoSub marker
If Err.Number = -1 Then
marker:
x = 1
Return
marker:
x = 2
Return
End If
End Sub
