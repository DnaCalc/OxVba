Sub Main()
Dim x As Byte
Dim a() As Byte
ReDim a(1 To 2, 1 To 3, 1 To 4)
a(2, 3, 4) = CByte(7)
Erase a
x = a(2, 3, 4)
End Sub
