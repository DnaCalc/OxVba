Sub Main()
    Dim x
    On Error Resume Next
    Error 7
    Call Worker
    x = Err.Number
End Sub

Sub Worker()
    Dim y
    y = 1
End Sub
