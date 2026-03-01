Sub Main()
    Dim x
    On Error Resume Next
    Error 5
    Resume Next
    x = Err.Number
End Sub
