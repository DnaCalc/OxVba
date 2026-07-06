Sub Main()
Dim x As Long
Dim after As Long
x = 5
Call Mutate(x)
after = x
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = 99&
End Sub
