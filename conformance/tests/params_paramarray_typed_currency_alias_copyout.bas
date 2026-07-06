Sub Main()
Dim x As Currency
Dim after As Currency
x = 1@
Call Mutate(x)
after = x
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = 12.3456@
End Sub
