Sub Main()
    Dim x
    x = 1
    Value = x
End Sub

Property Let Value(ByRef target)
    target = target + 2
End Property
