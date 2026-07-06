Public text As String * 3

Sub Main()
Dim marker
text = "abc"
Call Mutate(target := marker, ByVal text)
Dim afterText As String
afterText = text
End Sub

Sub Mutate(ByRef target, ParamArray items() As Variant)
items(0) = "abcdef"
target = UBound(items) + 101
End Sub
