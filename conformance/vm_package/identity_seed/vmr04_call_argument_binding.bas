Sub Main()
Dim seed As Long
Dim byRefAlias As Long
Dim byRefExpressionUnchanged As Long
Dim byValCoerced As Double
Dim byValType As Long
Dim defaultedObserved As Long
Dim missingObserved As Long
Dim emptyPack As Variant
Dim packedPack As Variant
seed = 4
Call Mutate(seed)
byRefAlias = seed
seed = 4
Call Mutate(seed + 2)
byRefExpressionUnchanged = seed
byValCoerced = TakeDouble(seed, byValType)
defaultedObserved = Defaulted()
missingObserved = MaybeVariantTag()
Call Capture(emptyPack)
Call Capture(packedPack, 10, 11)
End Sub

Sub Mutate(ByRef target As Long)
target = target + 5
End Sub

Function TakeDouble(ByVal value As Double, ByRef observedType As Long) As Double
observedType = VarType(value)
value = value + 0.5
TakeDouble = value
End Function

Function Defaulted(Optional ByVal value As Long = 7) As Long
Defaulted = value
End Function

Function MaybeVariantTag(Optional ByVal value As Variant) As Long
MaybeVariantTag = VarType(value)
End Function

Sub Capture(ByRef target As Variant, ParamArray items() As Variant)
target = UBound(items)
End Sub
