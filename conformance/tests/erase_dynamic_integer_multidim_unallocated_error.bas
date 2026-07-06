Sub Main()
Dim x As Integer
Dim a() As Integer
ReDim a(1 To 2, 1 To 3)
a(2, 3) = 44%
Erase a
x = a(2, 3)
End Sub
