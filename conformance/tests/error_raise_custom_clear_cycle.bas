Sub Main()
    Dim a
    Dim b
    Dim c
    On Error Resume Next
    Err.Raise 100
    a = Err.Number
    Err.Clear
    b = Err.Number
    Err.Raise 200
    c = Err.Number
End Sub
