Sub Main()
Dim x As Date
Dim a() As Date
ReDim a(1 To 2, 1 To 3, 1 To 4)
a(2, 3, 4) = CDate(36527#)
Erase a
x = a(2, 3, 4)
End Sub
