Sub Main()
Dim x As LongPtr
Dim after As LongPtr
x = 0
Call Mutate(x)
after = x
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = CLngPtr(5000000013.5#)
End Sub
