Sub Main()
Dim x
Call Capture(x)
End Sub

Sub Capture(ByRef target, ParamArray items() As Variant)
target = UBound(items)
End Sub
