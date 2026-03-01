Sub Main()
Dim a
Dim b
Dim c
Dim d
Dim e
On Error Resume Next
Error 11
a = Err.Number
Resume Next
b = Err.Number
c = IsError(CVErr(-70000))
d = VarType(CVErr(70000))
e = IsNumeric(CVErr(123))
End Sub
