Sub Main()
Dim x
Call Capture(target := x, items := 5)
End Sub

Sub Capture(ByRef target, ParamArray items() As Variant)
target = UBound(items)
End Sub
