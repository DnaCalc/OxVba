Option Base 1
Sub Main()
Dim x
Dim y
    ProbeOptionBaseOne x, y
End Sub

Private Sub ProbeOptionBaseOne(ByRef x, ByRef y)
Dim a(3)
a(1) = 4
a(3) = 9
x = a(1)
y = a(3)
End Sub
