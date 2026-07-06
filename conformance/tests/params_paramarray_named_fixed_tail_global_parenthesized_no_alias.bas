Public item As Long

Sub Main()
Dim marker
item = 5
Call Mutate(target := marker, (item))
Dim after
after = item
End Sub

Sub Mutate(ByRef target, ParamArray items() As Variant)
items(0) = 123&
target = UBound(items) + 101
End Sub
