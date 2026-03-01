Sub Main()
Dim a
Dim b
Dim c
Dim d
Dim e
Dim f
a = IsError(CVErr(0))
b = IsError(CVErr(65535))
c = IsError(CVErr(70000))
d = IsError(CVErr(-70000))
e = IsNumeric(CVErr(0))
f = VarType(CVErr(70000))
End Sub
