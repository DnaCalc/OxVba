Sub Main()
Dim x As String * 3
Dim after As String
x = "a"
Call Mutate(x)
after = x
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = "abcdef"
End Sub
