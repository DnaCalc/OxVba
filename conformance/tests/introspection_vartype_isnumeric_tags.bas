Sub Main()
Dim a
Dim b
Dim c
Dim d
Dim e
Dim f
Dim g
Dim h
a = VarType(vbNullString)
b = VarType(Null)
c = VarType(CVErr(9))
d = VarType(7)
e = IsNumeric(vbNullString)
f = IsNumeric(Null)
g = IsNumeric(CVErr(9))
h = IsNumeric(7)
End Sub
