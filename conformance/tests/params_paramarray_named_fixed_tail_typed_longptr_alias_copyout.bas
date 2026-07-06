Sub Main()
Dim value As LongPtr
Dim marker
Dim after As LongPtr
value = CLngPtr(17#)
Call Mutate(target := marker, value)
after = value
End Sub

Sub Mutate(ByRef target, ParamArray items() As Variant)
items(0) = CLngPtr(5000000013.5#)
target = UBound(items) + 101
End Sub
