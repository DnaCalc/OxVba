Sub Main()
Dim seed As Long
Dim afterByVal As Long
Dim byValResult As Long
Dim byRefObserved As Long
Dim optionalObserved As Long
Dim packObserved As Variant
Dim propSource As Long
Dim propertyObserved As Long
seed = 5
byValResult = TakeByVal(seed)
afterByVal = seed
Call Mutate(seed)
byRefObserved = seed
optionalObserved = Defaulted()
Call Capture(packObserved, 5, 7, 9)
propSource = 8
Value = propSource
propertyObserved = propSource
End Sub

Function TakeByVal(ByVal value As Long) As Long
value = value + 10
TakeByVal = value
End Function

Sub Mutate(ByRef target As Long)
target = target + 3
End Sub

Function Defaulted(Optional ByVal value As Long = 7) As Long
Defaulted = value
End Function

Sub Capture(ByRef target As Variant, ParamArray items() As Variant)
target = UBound(items)
End Sub

Property Let Value(ByRef newValue As Long)
newValue = newValue + 100
End Property
