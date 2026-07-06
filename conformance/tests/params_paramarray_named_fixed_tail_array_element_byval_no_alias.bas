Sub Main()
Dim values(0 To 0)
Dim marker
values(0) = CLng(5)
Call Mutate(target := marker, ByVal values(0))
Dim after
after = values(0)
End Sub

Sub Mutate(ByRef target, ParamArray items() As Variant)
items(0) = CLng(17)
target = UBound(items) + 101
End Sub
