Sub Main()
Dim target As Long
Dim namedObserved As Long
Dim packedUpper As Long

target = 1
Call Fill(value := 9, target := target)
namedObserved = target

target = 5
Call Capture(target := target, 3, 4)
packedUpper = target
End Sub

Sub Fill(ByRef target As Long, ByVal value As Long)
target = value
End Sub

Sub Capture(ByRef target As Long, ParamArray items() As Variant)
target = UBound(items)
End Sub
