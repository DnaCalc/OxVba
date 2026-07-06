Sub Main()
Dim x As Double
Dim a() As Double
ReDim a(1 To 2, 1 To 3)
a(2, 3) = 2.5#
Erase a
x = a(2, 3)
End Sub
