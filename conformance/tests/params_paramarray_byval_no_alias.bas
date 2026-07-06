Sub Main()
Dim first
Dim second
first = 5
second = 9
Call Mutate(ByVal first, ByVal second)
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = 11
items(1) = 13
End Sub
