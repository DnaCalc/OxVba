Sub Main()
Dim x As Long
Dim d4 As Integer
Dim a() As Long
d4 = 4
ReDim a(1 To 2, 4 To 6, 7 To 9)
x = UBound(a, d4)
End Sub
