Sub Main()
Dim x As Long
Dim a() As Long
ReDim a(1 To 2, 1 To 3, 1 To 4)
a(2, 3, 4) = 42&
Erase a
x = a(2, 3, 4)
End Sub
