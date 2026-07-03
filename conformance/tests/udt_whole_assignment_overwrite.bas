Type Pair
    A As Integer
    B As Integer
End Type

Sub Main()
Dim xa
Dim xb
Dim ya
Dim yb
ProbePairWholeOverwrite xa, xb, ya, yb
End Sub

Private Sub ProbePairWholeOverwrite(ByRef xa, ByRef xb, ByRef ya, ByRef yb)
Dim x As Pair
Dim y As Pair
x.A = 1
x.B = 2
y.A = 9
y.B = 8
y = x
x.A = 7
x.B = 6
y = x
xa = x.A
xb = x.B
ya = y.A
yb = y.B
End Sub
