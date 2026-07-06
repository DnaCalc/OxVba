Sub Main()
Dim x As Date
Dim after As Date
x = CDate(0#)
Call Mutate(x)
after = x
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = CDate(36527#)
End Sub
