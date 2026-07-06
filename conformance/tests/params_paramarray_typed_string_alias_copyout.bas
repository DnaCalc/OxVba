Sub Main()
Dim x As String
Dim after As String
x = "before"
Call Mutate(x)
after = x
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = "mutated"
End Sub
