Public text As String

Sub Main()
Dim marker
text = "before"
Call Mutate(target := marker, (text))
Dim after
after = text
End Sub

Sub Mutate(ByRef target, ParamArray items() As Variant)
items(0) = "named-global"
target = UBound(items) + 101
End Sub
