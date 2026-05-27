Sub Main()
Dim x As Long
x = 1
Call Capture(target := x, items := 5)
End Sub

Sub Capture(ByRef target As Long, ParamArray items() As Variant)
target = UBound(items)
End Sub
