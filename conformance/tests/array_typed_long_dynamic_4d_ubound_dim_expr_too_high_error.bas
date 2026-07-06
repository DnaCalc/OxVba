Sub Main()
Dim x As Long
Dim d5 As Integer
Dim a() As Long
d5 = 5
ReDim a(1 To 2, 4 To 6, 7 To 9, 10 To 12)
x = UBound(a, d5)
End Sub
