Option Base 1

Sub Main()
Dim lower
Dim upper
Call Capture(lower, upper, 10, 20, 30)
End Sub

Sub Capture(ByRef lower, ByRef upper, ParamArray items() As Variant)
lower = LBound(items)
upper = UBound(items)
End Sub
