Sub Main()
Dim a(0 To 3)
Dim x
a(3) = 9
ReDim Preserve a(0 To 1)
ReDim Preserve a(0 To 3)
x = a(3)
End Sub
