Sub Main()
    Dim x
    Dim y
    On Error Resume Next
    Call RaiseSub()
    x = Err.Number
    Err.Clear
    y = SafeAdd(3, 4)
End Sub

Sub RaiseSub()
    Err.Raise 42
End Sub

Function SafeAdd(a, b)
    SafeAdd = a + b
End Function
