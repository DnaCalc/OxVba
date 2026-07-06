Sub Main()
Dim text As String * 3
Dim marker
text = "abc"
Call Mutate(target := marker, (text))
Dim afterText As String
afterText = text
End Sub

Sub Mutate(ByRef target, ParamArray items() As Variant)
items(0) = "abcdef"
target = UBound(items) + 101
End Sub
