Sub Main()
Dim x As Double
Dim a() As Double
ReDim a(1 To 2, 1 To 3, 1 To 4, 1 To 5)
a(2, 3, 4, 5) = 2.5#
Erase a
x = a(2, 3, 4, 5)
End Sub
