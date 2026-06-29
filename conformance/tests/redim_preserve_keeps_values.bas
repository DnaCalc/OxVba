Sub Main()
Dim a()
Dim x
ReDim a(1)
a(0) = 7
ReDim Preserve a(3)
x = a(0)
End Sub
