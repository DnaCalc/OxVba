Public x As Long

Sub Main()
x = 5
Call Mutate(x)
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = 123&
End Sub
