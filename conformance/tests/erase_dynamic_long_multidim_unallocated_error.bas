Sub Main()
Dim x As Long
Dim a() As Long
ReDim a(1 To 2, 1 To 3)
a(2, 3) = 42&
Erase a
x = a(2, 3)
End Sub
