Sub Main()
    Dim x
    Dim y
    x = 0
    On Error Resume Next
    GoSub AddTen
    Err.Raise 77
    y = Err.Number
    GoTo Done
AddTen:
    x = x + 10
    Return
Done:
End Sub
