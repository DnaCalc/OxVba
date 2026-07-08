Public result As Long

Sub Main()
    Dim i As Long
    Dim n As Long
    Dim x As Long

    On Error Resume Next
    For i = 1 To 5000
        x = 1 / 0
        If Err.Number <> 0 Then
            n = n + Err.Number
            Err.Clear
        End If
    Next i

    result = n
End Sub
