Sub Main()
Dim value
Dim lower As Long
Dim upper As Long
Dim a(2 To 4) As Long
a(2) = 7
Erase a
value = a(2)
lower = LBound(a)
upper = UBound(a)
End Sub
