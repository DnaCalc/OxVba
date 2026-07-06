Sub Main()
Dim lower
Dim upper
Call Capture(lower, upper, 5, 7, 9)
End Sub

Sub Capture(ByRef lower, ByRef upper, ParamArray items() As Variant)
lower = LBound(items, 1)
upper = UBound(items, 1)
End Sub
