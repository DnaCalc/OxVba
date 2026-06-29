Sub Main()
Dim m()
Dim x
ReDim m(1 To 2, 1 To 2)
m(1, 1) = 7
ReDim Preserve m(1 To 2, 1 To 3)
x = m(1, 1)
End Sub
