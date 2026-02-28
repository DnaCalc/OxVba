Sub Main()
    Dim x
    On Error Resume Next
    Error 7
    Err.Clear
    x = Err.Number
End Sub