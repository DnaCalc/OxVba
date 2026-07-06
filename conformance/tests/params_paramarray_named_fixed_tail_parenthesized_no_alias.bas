Sub Main()
Dim item
Dim marker
item = 5
Call Mutate(target := marker, (item))
Dim after
after = item
End Sub

Sub Mutate(ByRef target, ParamArray items() As Variant)
items(0) = CLng(17)
target = UBound(items) + 101
End Sub
