Sub Main()
Dim x As Boolean
Dim after As Boolean
x = False
Call Mutate(x)
after = x
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = True
End Sub
