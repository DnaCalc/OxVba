Sub Main()
Dim x As Byte
Dim after As Byte
x = 5
Call Mutate(x)
after = x
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = CByte(7)
End Sub
