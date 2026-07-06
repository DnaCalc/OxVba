Sub Main()
Dim x As Double
Dim after As Double
x = 0#
Call Mutate(x)
after = x
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = 2.5#
End Sub
