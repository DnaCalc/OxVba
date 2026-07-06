Sub Main()
Dim x
Call Capture(x, 5)
End Sub

Sub Capture(ByRef target, ParamArray items() As Variant)
target = UBound(items, 2)
End Sub
