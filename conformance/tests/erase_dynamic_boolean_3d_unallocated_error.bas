Sub Main()
Dim x As Boolean
Dim a() As Boolean
ReDim a(1 To 2, 1 To 3, 1 To 4)
a(2, 3, 4) = True
Erase a
x = a(2, 3, 4)
End Sub
