Sub Main()
Dim x
Call Capture(x, 5, 7, 9)
End Sub

Sub Capture(ByRef target, ParamArray items() As Variant)
target = UBound(items)
End Sub
