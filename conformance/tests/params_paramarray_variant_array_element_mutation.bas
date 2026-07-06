Sub Main()
Dim v
Dim after
v = Array(5)
Call Mutate(v)
after = v(0)
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0)(0) = 99
End Sub
