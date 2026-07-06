Sub Main()
Dim number As Long
Dim text As String
Dim marker
number = 5
text = "before"
Call Mutate(target := marker, (number), (text))
Dim afterNumber As Long
Dim afterText As String
afterNumber = number
afterText = text
End Sub

Sub Mutate(ByRef target, ParamArray items() As Variant)
items(0) = 99&
items(1) = "mutated"
target = UBound(items) + 101
End Sub
