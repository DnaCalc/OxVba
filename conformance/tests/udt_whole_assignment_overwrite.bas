Type Pair
    A
    B
End Type

Sub Main()
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
End Sub
