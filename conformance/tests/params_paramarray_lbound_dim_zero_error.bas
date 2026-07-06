Sub Main()
Dim x
Call Capture(x, 5)
End Sub

Sub Capture(ByRef target, ParamArray items() As Variant)
target = LBound(items, 0)
End Sub
