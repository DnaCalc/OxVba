Attribute VB_Name = "BusinessCalc"

Public Function GrossMargin(ByVal revenue As Double, ByVal cost As Double) As Double
    GrossMargin = (revenue - cost) / revenue
End Function

Public Function ApplyDiscount(ByVal price As Double, ByVal discountRate As Double) As Double
    ApplyDiscount = price - (price * discountRate)
End Function

Public Function UnitsAfterBundle(ByVal baseUnits As Long, ByVal bundleSize As Long) As Long
    UnitsAfterBundle = baseUnits + bundleSize
End Function

Private Function InternalRoundMarker(ByVal value As Long) As Long
    InternalRoundMarker = value
End Function
