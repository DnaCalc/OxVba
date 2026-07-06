Sub Main()
Dim x As Date
Dim a() As Date
ReDim a(1 To 2, 1 To 3)
a(2, 3) = CDate(36527#)
Erase a
x = a(2, 3)
End Sub
