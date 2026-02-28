Sub Main()
    Dim x
    x = 0
    On Error GoTo handler
    If x = 0 Then
        Error 5
    End If
    x = x + 10
handler:
    If x = 0 Then
        x = 1
        Resume
    End If
    x = x + 1
End Sub