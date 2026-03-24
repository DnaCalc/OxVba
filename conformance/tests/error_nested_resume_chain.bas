Sub Main()
    Dim x
    Dim y
    x = 5
    On Error Resume Next
    x = Outer()
    y = Err.Number
End Sub

Function Outer()
    Outer = Inner()
End Function

Function Inner()
    Err.Raise 55
    Inner = 99
End Function
