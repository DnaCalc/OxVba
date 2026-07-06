Sub Main()
Dim x As Currency
Dim a() As Currency
ReDim a(1 To 2, 1 To 3)
a(2, 3) = 12.3456@
Erase a
x = a(2, 3)
End Sub
