Sub Main()
    Dim x
    x = 2
    Obj = x
End Sub

Property Set Obj(ByRef target)
    target = target + 5
End Property
