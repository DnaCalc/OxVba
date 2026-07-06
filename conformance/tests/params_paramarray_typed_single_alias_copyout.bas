Sub Main()
Dim x As Single
Dim after As Single
x = 0!
Call Mutate(x)
after = x
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = 1.25!
End Sub
