Sub Main()
Dim x As Single
Dim a() As Single
ReDim a(1 To 2, 1 To 3, 1 To 4)
a(2, 3, 4) = 1.25!
Erase a
x = a(2, 3, 4)
End Sub
