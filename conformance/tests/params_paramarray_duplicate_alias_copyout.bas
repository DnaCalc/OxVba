Sub Main()
Dim first
first = 5
Call Mutate(first, first)
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = 11
items(1) = 13
End Sub
