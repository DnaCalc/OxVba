Sub Main()
Dim x As Boolean
Dim a() As Boolean
ReDim a(1 To 2, 1 To 3)
a(2, 3) = True
Erase a
x = a(2, 3)
End Sub
