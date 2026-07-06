Sub Main()
Dim a(0 To 1)
Dim after0
Dim after1
a(0) = 5
a(1) = 9
Call Mutate(a(0), a(1))
after0 = a(0)
after1 = a(1)
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = 11
items(1) = 13
End Sub
