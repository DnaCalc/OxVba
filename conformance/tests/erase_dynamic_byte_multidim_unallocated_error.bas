Sub Main()
Dim x As Byte
Dim a() As Byte
ReDim a(1 To 2, 1 To 3)
a(2, 3) = CByte(7)
Erase a
x = a(2, 3)
End Sub
