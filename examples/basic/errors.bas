Sub Main()
    Dim beforeClear
    Dim afterClear
    On Error Resume Next
    Error 7
    beforeClear = Err.Number
    Err.Clear
    afterClear = Err.Number
End Sub
