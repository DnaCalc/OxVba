Sub Main()
    Dim x
    On Error Resume Next
    x = 1
    Error 2
    x = x + 1
End Sub
