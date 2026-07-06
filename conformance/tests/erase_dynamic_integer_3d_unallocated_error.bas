Sub Main()
Dim x As Integer
Dim a() As Integer
ReDim a(1 To 2, 1 To 3, 1 To 4)
a(2, 3, 4) = 44%
Erase a
x = a(2, 3, 4)
End Sub
