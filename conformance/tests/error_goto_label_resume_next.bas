Sub Main()
    Dim x
    Dim y
    On Error GoTo Handler
    x = 10
    Err.Raise 42
    x = 20
    GoTo Done
Handler:
    y = Err.Number
    Resume Next
Done:
End Sub
