Sub Main()
Dim lower
Dim upper
Dim emptyType
Dim emptyName
Dim second
Call Capture(lower, upper, emptyType, emptyName, second, , 7)
End Sub

Sub Capture(ByRef lower, ByRef upper, ByRef emptyType, ByRef emptyName, ByRef second, ParamArray items() As Variant)
lower = LBound(items)
upper = UBound(items)
emptyType = VarType(items(0))
emptyName = TypeName(items(0))
second = items(1)
End Sub
